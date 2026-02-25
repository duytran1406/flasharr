use rusqlite::{params, Result, OptionalExtension};
use std::path::Path;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use serde::{Deserialize, Serialize};
use super::media::{MediaItem, MediaEpisode};
use crate::downloader::task::{DownloadTask, DownloadState};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

pub struct Db {
    pool: Pool<SqliteConnectionManager>,
}

impl Db {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let manager = SqliteConnectionManager::file(path);
        let pool = Pool::builder()
            .max_size(5) // 5 concurrent connections
            .build(manager)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        
        let db = Self { pool };
        db.init()?;
        
        tracing::info!("Database connection pool initialized (max_size: 5)");
        Ok(db)
    }

    fn init(&self) -> Result<()> {
        let conn = self.pool.get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        
        // Downloads table (also known as tasks table)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS downloads (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                original_url TEXT NOT NULL,
                filename TEXT NOT NULL,
                destination TEXT NOT NULL,
                state TEXT NOT NULL,
                progress REAL NOT NULL,
                size INTEGER NOT NULL,
                downloaded INTEGER,
                speed REAL,
                eta INTEGER,
                host TEXT NOT NULL,
                category TEXT NOT NULL,
                priority INTEGER NOT NULL,
                segments INTEGER NOT NULL DEFAULT 4,
                retry_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT,
                wait_until TEXT,
                error_message TEXT
            )",
            [],
        )?;
        
        // Migrate existing downloads table if needed (add new columns)
        // This is safe - ALTER TABLE ADD COLUMN is idempotent in SQLite
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN downloaded INTEGER", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN speed REAL", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN eta INTEGER", []);
        // Batch grouping columns (v2.0)
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN batch_id TEXT", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN batch_name TEXT", []);
        // TMDB metadata columns for Sonarr/Radarr matching (v2.0.1)
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN tmdb_id INTEGER", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN tmdb_title TEXT", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN tmdb_season INTEGER", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN tmdb_episode INTEGER", []);
        // Arr sync tracking columns (v2.1.0)
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN arr_announced BOOLEAN DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN arr_series_id INTEGER", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN arr_movie_id INTEGER", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN arr_announce_error TEXT", []);
        // Fshare code for duplicate detection (v2.2.0)
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN fshare_code TEXT", []);
        // Quality metadata columns (v2.3.0)
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN quality TEXT", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN resolution TEXT", []);
        
        // Backfill quality/resolution from filenames for existing records
        {
            let mut stmt = conn.prepare(
                "SELECT id, filename FROM downloads WHERE quality IS NULL AND filename IS NOT NULL"
            )?;
            let rows: Vec<(String, String)> = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?.filter_map(|r| r.ok()).collect();
            
            if !rows.is_empty() {
                tracing::info!("[DB] Backfilling quality metadata for {} downloads", rows.len());
                let mut update_stmt = conn.prepare(
                    "UPDATE downloads SET quality = ?1, resolution = ?2 WHERE id = ?3"
                )?;
                for (id, filename) in &rows {
                    let attrs = crate::utils::parser::FilenameParser::extract_quality_attributes(filename);
                    let quality = attrs.quality_name();
                    let resolution = attrs.resolution.clone();
                    let _ = update_stmt.execute(rusqlite::params![quality, resolution, id]);
                }
                tracing::info!("[DB] Quality backfill complete");
            }
        }
        
        // Create indexes for performance
        // Single-column indexes
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_state ON downloads(state)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_created_at ON downloads(created_at)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_batch_id ON downloads(batch_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_host ON downloads(host)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_category ON downloads(category)",
            [],
        )?;
        
        // Composite indexes for common query patterns
        // Query: Get active/queued downloads sorted by creation time
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_state_created ON downloads(state, created_at)",
            [],
        )?;
        // Query: Get downloads by batch sorted by creation time
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_batch_created ON downloads(batch_id, created_at)",
            [],
        )?;
        // Query: Get downloads by host and state (for monitoring)
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_host_state ON downloads(host, state)",
            [],
        )?;
        // Query: Batch summary aggregation with state filtering
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_batch_state ON downloads(batch_id, state)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_fshare_code ON downloads(fshare_code)",
            [],
        )?;
        // Index on tmdb_id for joining downloads to media_items
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_tmdb_id ON downloads(tmdb_id)",
            [],
        )?;
        
        // ── media_items table (TMDB-centric) ─────────────────────────────
        conn.execute(
            "CREATE TABLE IF NOT EXISTS media_items (
                tmdb_id INTEGER PRIMARY KEY,
                media_type TEXT NOT NULL,
                title TEXT NOT NULL,
                original_title TEXT,
                year INTEGER,
                overview TEXT,
                poster_path TEXT,
                backdrop_path TEXT,
                genres TEXT,
                runtime INTEGER,
                total_seasons INTEGER,
                arr_id INTEGER,
                arr_type TEXT,
                arr_path TEXT,
                arr_monitored INTEGER DEFAULT 0,
                arr_status TEXT,
                arr_quality_profile_id INTEGER,
                arr_has_file INTEGER DEFAULT 0,
                arr_size_on_disk INTEGER DEFAULT 0,
                tvdb_id INTEGER,
                imdb_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                arr_synced_at TEXT
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_media_items_media_type ON media_items(media_type)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_media_items_arr_id ON media_items(arr_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_media_items_tvdb_id ON media_items(tvdb_id)",
            [],
        )?;
        
        // ── media_episodes table ──────────────────────────────────────────
        conn.execute(
            "CREATE TABLE IF NOT EXISTS media_episodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tmdb_id INTEGER NOT NULL REFERENCES media_items(tmdb_id),
                season_number INTEGER NOT NULL,
                episode_number INTEGER NOT NULL,
                title TEXT,
                overview TEXT,
                air_date TEXT,
                arr_episode_id INTEGER,
                arr_has_file INTEGER DEFAULT 0,
                arr_monitored INTEGER DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(tmdb_id, season_number, episode_number)
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_media_episodes_tmdb ON media_episodes(tmdb_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_media_episodes_lookup ON media_episodes(tmdb_id, season_number, episode_number)",
            [],
        )?;
        
        tracing::info!("Database indexes created successfully");
        
        // Run ANALYZE to update query planner statistics
        // This helps SQLite choose the best indexes for queries
        conn.execute("ANALYZE", [])?;
        tracing::info!("Database statistics updated (ANALYZE complete)");
        
        // Accounts table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS accounts (
                email TEXT PRIMARY KEY,
                session_id TEXT,
                token TEXT,
                expires_at INTEGER,
                created_at INTEGER DEFAULT (strftime('%s', 'now'))
            )",
            [],
        )?;
        
        // Settings table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT,
                updated_at INTEGER DEFAULT (strftime('%s', 'now'))
            )",
            [],
        )?;
        
        Ok(())
    }

    pub fn save_task(&self, task: &DownloadTask) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO downloads (
                id, url, original_url, filename, destination, state, progress, size, 
                downloaded, speed, eta,
                host, category, priority, segments, retry_count, created_at, 
                started_at, completed_at, wait_until, error_message, batch_id, batch_name,
                tmdb_id, tmdb_title, tmdb_season, tmdb_episode, quality, resolution,
                arr_announced, arr_series_id, arr_movie_id, arr_announce_error,
                fshare_code
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34)",
            params![
                task.id.to_string(),
                task.url,
                task.original_url,
                task.filename,
                task.destination,
                format!("{:?}", task.state).to_uppercase(),
                task.progress,
                task.size as i64,
                task.downloaded as i64,
                task.speed,
                task.eta as i64,
                task.host,
                task.category,
                task.priority,
                task.segments as i64,
                task.retry_count as i64,
                task.created_at.to_rfc3339(),
                task.started_at.map(|dt| dt.to_rfc3339()),
                task.completed_at.map(|dt| dt.to_rfc3339()),
                task.wait_until.map(|dt| dt.to_rfc3339()),
                task.error_message,
                task.batch_id,
                task.batch_name,
                task.tmdb_id,
                task.tmdb_title,
                task.tmdb_season.map(|s| s as i64),
                task.tmdb_episode.map(|e| e as i64),
                &task.quality,
                &task.resolution,
                // Note: These fields are usually set by other tools but we should preserve them
                // We'll read them from the task if they are set
                task.arr_series_id.is_some() || task.arr_movie_id.is_some(), // Infer announced if we have IDs
                task.arr_series_id,
                task.arr_movie_id,
                None::<String>, // arr_announce_error - keep simple for now
                &task.fshare_code,
            ],
        )?;
        Ok(())
    }

    fn parse_task_from_row(row: &rusqlite::Row) -> rusqlite::Result<DownloadTask> {
        let id_str: String = row.get(0)?;
        let id = Uuid::parse_str(&id_str).map_err(|_| rusqlite::Error::InvalidQuery)?;
        
        let state_str: String = row.get(5)?;
        // Note: columns 8, 9, 10 are downloaded, speed, eta (skipped for now)
        let state = match state_str.as_str() {
            "QUEUED" => DownloadState::Queued,
            "STARTING" => DownloadState::Starting,
            "DOWNLOADING" => DownloadState::Downloading,
            "PAUSED" => DownloadState::Paused,
            "WAITING" => DownloadState::Waiting,
            "COMPLETED" => DownloadState::Completed,
            "FAILED" => DownloadState::Failed,
            "CANCELLED" => DownloadState::Cancelled,
            "EXTRACTING" => DownloadState::Extracting,
            "SKIPPED" => DownloadState::Skipped,
            _ => DownloadState::Queued,
        };

        let created_at_str: String = row.get(16)?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|_| rusqlite::Error::InvalidQuery)?;

        let started_at_str: Option<String> = row.get(17)?;
        let started_at = started_at_str.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .ok()
        });

        let completed_at_str: Option<String> = row.get(18)?;
        let completed_at = completed_at_str.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .ok()
        });

        let wait_until_str: Option<String> = row.get(19)?;
        let wait_until = wait_until_str.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .ok()
        });

        Ok(DownloadTask {
            id,
            url: row.get(1)?,
            original_url: row.get(2)?,
            filename: row.get(3)?,
            destination: row.get(4)?,
            state,
            progress: row.get(6)?,
            size: row.get::<_, i64>(7)? as u64,
            downloaded: row.get::<_, Option<i64>>(8)?.unwrap_or(0) as u64,
            speed: row.get::<_, Option<f64>>(9)?.unwrap_or(0.0),
            eta: row.get::<_, Option<f64>>(10)?.unwrap_or(0.0),
            host: row.get(11)?,
            category: row.get(12)?,
            priority: row.get(13)?,
            segments: row.get::<_, i64>(14)? as u32,
            retry_count: row.get::<_, i64>(15)? as u32,
            created_at,
            started_at,
            completed_at,
            wait_until,
            error_message: row.get(20)?,
            batch_id: row.get(21).ok().flatten(),
            batch_name: row.get(22).ok().flatten(),
            tmdb_id: row.get::<_, Option<i64>>(23).ok().flatten(),
            tmdb_title: row.get(24).ok().flatten(),
            tmdb_season: row.get::<_, Option<i64>>(25).ok().flatten().map(|s| s as u32),
            tmdb_episode: row.get::<_, Option<i64>>(26).ok().flatten().map(|e| e as u32),
            quality: row.get::<_, Option<String>>(27).ok().flatten(),
            resolution: row.get::<_, Option<String>>(28).ok().flatten(),
            arr_series_id: row.get::<_, Option<i64>>(29).ok().flatten(),
            arr_movie_id: row.get::<_, Option<i64>>(30).ok().flatten(),
            url_metadata: None,  // Not stored in DB yet, will be set on URL resolution
            error_history: Vec::new(),  // Not stored in DB, will be populated on errors
            fshare_code: row.get::<_, Option<String>>(31).ok().flatten(),
            state_obj: crate::downloader::state_machine::TaskStateFactory::get_state(state),
            cancel_token: CancellationToken::new(),
            pause_notify: Arc::new(Notify::new()),
        })
    }

    pub fn get_all_tasks(&self) -> Result<Vec<DownloadTask>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size, 
             downloaded, speed, eta,
             host, category, priority, segments, retry_count, created_at, 
             started_at, completed_at, wait_until, error_message, batch_id, batch_name,
             tmdb_id, tmdb_title, tmdb_season, tmdb_episode, quality, resolution, arr_series_id, arr_movie_id 
             FROM downloads"
        )?;
        let task_iter = stmt.query_map([], |row| Self::parse_task_from_row(row))?;

        let mut tasks = Vec::new();
        for task in task_iter {
            if let Ok(t) = task {
                tasks.push(t);
            }
        }
        Ok(tasks)
    }

    pub fn get_task_by_id(&self, id: Uuid) -> Result<Option<DownloadTask>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size, 
             downloaded, speed, eta,
             host, category, priority, segments, retry_count, created_at, 
             started_at, completed_at, wait_until, error_message, batch_id, batch_name,
             tmdb_id, tmdb_title, tmdb_season, tmdb_episode, quality, resolution, arr_series_id, arr_movie_id 
             FROM downloads WHERE id = ?1"
        )?;
        let mut task_iter = stmt.query_map(params![id.to_string()], |row| Self::parse_task_from_row(row))?;

        match task_iter.next() {
            Some(task) => Ok(Some(task?)),
            None => Ok(None),
        }
    }

    /// Get tasks with pagination, sorted by created_at DESC
    /// Returns (tasks, total_count)
    pub fn get_tasks_paginated(&self, page: u32, limit: u32) -> Result<(Vec<DownloadTask>, u64)> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        
        // Get total count
        let total: u64 = conn.query_row(
            "SELECT COUNT(*) FROM downloads",
            [],
            |row| row.get(0),
        )?;
        
        // Calculate offset
        let offset = (page.saturating_sub(1)) * limit;
        
        // Get paginated tasks (active states first, then by created_at DESC)
        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size, 
             downloaded, speed, eta,
             host, category, priority, segments, retry_count, created_at, 
             started_at, completed_at, wait_until, error_message, batch_id, batch_name 
             FROM downloads 
             ORDER BY 
                CASE state 
                    WHEN 'DOWNLOADING' THEN 0 
                    WHEN 'STARTING' THEN 1 
                    WHEN 'QUEUED' THEN 2 
                    WHEN 'PAUSED' THEN 3 
                    WHEN 'WAITING' THEN 4 
                    ELSE 5 
                END,
                created_at DESC
             LIMIT ?1 OFFSET ?2"
        )?;
        
        let task_iter = stmt.query_map(params![limit as i64, offset as i64], |row| Self::parse_task_from_row(row))?;

        let mut tasks = Vec::new();
        for task in task_iter {
            if let Ok(t) = task {
                tasks.push(t);
            }
        }
        
        Ok((tasks, total))
    }

    /// Get the next queued task for workers to process (FIFO with priority)
    pub fn get_next_queued_task(&self) -> Result<Option<DownloadTask>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size, 
             downloaded, speed, eta,
             host, category, priority, segments, retry_count, created_at, 
             started_at, completed_at, wait_until, error_message, batch_id, batch_name,
             tmdb_id, tmdb_title, tmdb_season, tmdb_episode, quality, resolution, arr_series_id, arr_movie_id 
             FROM downloads 
             WHERE state = 'QUEUED'
             ORDER BY priority DESC, created_at ASC
             LIMIT 1"
        )?;
        
        let mut task_iter = stmt.query_map([], |row| Self::parse_task_from_row(row))?;
        
        match task_iter.next() {
            Some(task) => Ok(Some(task?)),
            None => Ok(None),
        }
    }

    /// Get tasks waiting for retry that are past their wait_until time
    pub fn get_ready_waiting_tasks(&self) -> Result<Vec<DownloadTask>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let now = chrono::Utc::now().to_rfc3339();
        
        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size, 
             downloaded, speed, eta,
             host, category, priority, segments, retry_count, created_at, 
             started_at, completed_at, wait_until, error_message, batch_id, batch_name,
             tmdb_id, tmdb_title, tmdb_season, tmdb_episode, quality, resolution, arr_series_id, arr_movie_id 
             FROM downloads 
             WHERE state = 'WAITING' AND (wait_until IS NULL OR wait_until <= ?1)
             ORDER BY created_at ASC"
        )?;
        
        let task_iter = stmt.query_map(params![now], |row| Self::parse_task_from_row(row))?;

        let mut tasks = Vec::new();
        for task in task_iter {
            if let Ok(t) = task {
                tasks.push(t);
            }
        }
        Ok(tasks)
    }

    /// Update task state in database
    pub fn update_task_state(&self, id: Uuid, state: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "UPDATE downloads SET state = ?1 WHERE id = ?2",
            params![state, id.to_string()],
        )?;
        Ok(())
    }

    /// Update only progress-related fields (narrow update to avoid data races)
    /// This should be used during active downloads instead of save_task
    pub fn update_task_progress(&self, id: Uuid, downloaded: u64, speed: f64, eta: f64, progress: f32) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "UPDATE downloads SET downloaded = ?1, speed = ?2, eta = ?3, progress = ?4 WHERE id = ?5",
            params![downloaded as i64, speed, eta as i64, progress, id.to_string()],
        )?;
        Ok(())
    }

    /// Async version of update_task_progress
    pub async fn update_task_progress_async(&self, id: Uuid, downloaded: u64, speed: f64, eta: f64, progress: f32) -> Result<()> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            conn.execute(
                "UPDATE downloads SET downloaded = ?1, speed = ?2, eta = ?3, progress = ?4 WHERE id = ?5",
                params![downloaded as i64, speed, eta as i64, progress, id.to_string()],
            )?;
            Ok(())
        }).await.unwrap()
    }

    /// Get count of tasks by state
    pub fn count_tasks_by_state(&self, state: &str) -> Result<u64> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM downloads WHERE state = ?1",
            params![state],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn delete_task(&self, id: Uuid) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("DELETE FROM downloads WHERE id = ?1", params![id.to_string()])?;
        Ok(())
    }

    // ============================================================================
    // Async Wrappers (prevent async runtime blocking)
    // ============================================================================

    /// Async version of get_tasks_paginated - uses spawn_blocking
    pub async fn get_tasks_paginated_async(&self, page: u32, limit: u32) -> Result<(Vec<DownloadTask>, u64)> {
        self.get_tasks_paginated_sorted_async(page, limit, "added", "desc").await
    }

    /// Async version with sorting - uses spawn_blocking
    /// sort_by: "added", "status", "filename", "size", "progress"
    /// sort_dir: "asc" or "desc"
    pub async fn get_tasks_paginated_sorted_async(&self, page: u32, limit: u32, sort_by: &str, sort_dir: &str) -> Result<(Vec<DownloadTask>, u64)> {
        self.get_tasks_paginated_sorted_filtered_async(page, limit, sort_by, sort_dir, None).await
    }

    /// Async version with sorting and optional status filter - uses spawn_blocking
    /// sort_by: "added", "status", "filename", "size", "progress"
    /// sort_dir: "asc" or "desc"
    /// status_filter: Optional status to filter by (e.g., "DOWNLOADING", "QUEUED", "COMPLETED", "FAILED")
    /// 
    /// IMPORTANT: This function ensures batches are never split across pages.
    /// All downloads with the same batch_id will always appear together.
    pub async fn get_tasks_paginated_sorted_filtered_async(&self, page: u32, limit: u32, sort_by: &str, sort_dir: &str, status_filter: Option<&str>) -> Result<(Vec<DownloadTask>, u64)> {
        let pool = self.pool.clone();
        let sort_by = sort_by.to_string();
        let sort_dir = sort_dir.to_string();
        let status_filter = status_filter.map(|s| s.to_uppercase());
        
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            
            // Build WHERE clause for status filter
            let where_clause = match &status_filter {
                Some(status) => format!("WHERE state = '{}'", status),
                None => String::new(),
            };
            
            // Get total count (with filter if specified)
            let count_query = format!("SELECT COUNT(*) FROM downloads {}", where_clause);
            let total: u64 = conn.query_row(&count_query, [], |row| row.get(0))?;
            
            let offset = (page.saturating_sub(1)) * limit;
            
            // Build ORDER BY clause based on sort_by parameter
            let order_by = match sort_by.as_str() {
                "status" => format!(
                    "CASE state 
                        WHEN 'DOWNLOADING' THEN 0 
                        WHEN 'STARTING' THEN 1 
                        WHEN 'QUEUED' THEN 2 
                        WHEN 'PAUSED' THEN 3 
                        WHEN 'WAITING' THEN 4 
                        WHEN 'FAILED' THEN 5
                        WHEN 'COMPLETED' THEN 6
                        ELSE 7 
                    END {}, created_at DESC",
                    if sort_dir == "asc" { "ASC" } else { "DESC" }
                ),
                "filename" => format!("filename {}", if sort_dir == "asc" { "ASC" } else { "DESC" }),
                "size" => format!("size {}", if sort_dir == "asc" { "ASC" } else { "DESC" }),
                "progress" => format!("progress {}", if sort_dir == "asc" { "ASC" } else { "DESC" }),
                _ => format!("created_at {}", if sort_dir == "asc" { "ASC" } else { "DESC" }), // "added" default
            };
            
            // BATCH-AWARE PAGINATION:
            // Strategy: Fetch items with LIMIT/OFFSET, then include all remaining items from incomplete batches
            
            // Step 1: Get initial page of downloads
            let initial_query = format!(
                "SELECT id, url, original_url, filename, destination, state, progress, size, 
                 downloaded, speed, eta,
                 host, category, priority, segments, retry_count, created_at, 
                 started_at, completed_at, wait_until, error_message, batch_id, batch_name,
                 tmdb_id, tmdb_title, tmdb_season, tmdb_episode, quality, resolution, arr_series_id, arr_movie_id 
                 FROM downloads 
                 {}
                 ORDER BY {}
                 LIMIT ?1 OFFSET ?2",
                where_clause,
                order_by
            );
            
            let mut stmt = conn.prepare(&initial_query)?;
            let task_iter = stmt.query_map(params![limit as i64, offset as i64], |row| Self::parse_task_from_row_static(row))?;

            let mut tasks = Vec::new();
            let mut batch_ids_to_complete = std::collections::HashSet::new();
            
            for task in task_iter {
                if let Ok(t) = task {
                    // Track batch IDs that appear in this page
                    if let Some(ref batch_id) = t.batch_id {
                        batch_ids_to_complete.insert(batch_id.clone());
                    }
                    tasks.push(t);
                }
            }
            
            // Step 2: For each batch ID found, fetch ALL remaining items from that batch
            // that weren't in the initial page
            if !batch_ids_to_complete.is_empty() {
                let task_ids_in_page: std::collections::HashSet<uuid::Uuid> = 
                    tasks.iter().map(|t| t.id).collect();
                
                for batch_id in &batch_ids_to_complete {
                    // Get all downloads for this batch
                    let batch_query = format!(
                        "SELECT id, url, original_url, filename, destination, state, progress, size, 
                         downloaded, speed, eta,
                         host, category, priority, segments, retry_count, created_at, 
                         started_at, completed_at, wait_until, error_message, batch_id, batch_name,
                         tmdb_id, tmdb_title, tmdb_season, tmdb_episode, quality, resolution, arr_series_id, arr_movie_id 
                         FROM downloads 
                         WHERE batch_id = ?1
                         {}",
                        if status_filter.is_some() { 
                            format!("AND state = '{}'", status_filter.as_ref().unwrap()) 
                        } else { 
                            String::new() 
                        }
                    );
                    
                    let mut batch_stmt = conn.prepare(&batch_query)?;
                    let batch_iter = batch_stmt.query_map(params![batch_id], |row| Self::parse_task_from_row_static(row))?;
                    
                    for batch_task in batch_iter {
                        if let Ok(bt) = batch_task {
                            // Only add if not already in the page
                            if !task_ids_in_page.contains(&bt.id) {
                                tasks.push(bt);
                            }
                        }
                    }
                }
            }
            
            Ok((tasks, total))
        }).await.unwrap()
    }

    /// Async version of save_task
    pub async fn save_task_async(&self, task: DownloadTask) -> Result<()> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let db = Db { pool };
            db.save_task(&task)
        }).await.unwrap()
    }

    /// Async version of update_task_state
    pub async fn update_task_state_async(&self, id: Uuid, state: String) -> Result<()> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            conn.execute(
                "UPDATE downloads SET state = ?1 WHERE id = ?2",
                params![state, id.to_string()],
            )?;
            Ok(())
        }).await.unwrap()
    }

    /// Batch update task states atomically in a single transaction
    /// Used by pause_all, resume_all for atomic operations
    pub async fn batch_update_states_async(&self, task_ids: Vec<Uuid>, new_state: String) -> Result<usize> {
        if task_ids.is_empty() {
            return Ok(0);
        }
        
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let tx = conn.transaction()?;
            
            let mut affected = 0;
            for task_id in &task_ids {
                let rows = tx.execute(
                    "UPDATE downloads SET state = ?1 WHERE id = ?2",
                    params![&new_state, task_id.to_string()],
                )?;
                affected += rows;
            }
            
            tx.commit()?;
            Ok(affected)
        }).await.unwrap()
    }

    /// Get all tasks matching any of the provided states (async)
    /// Used for bulk operations like pause_all, resume_all
    pub async fn get_tasks_by_states_async(&self, states: Vec<String>) -> Result<Vec<DownloadTask>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            
            if states.is_empty() {
                return Ok(Vec::new());
            }
            
            // Build IN clause with placeholders
            let placeholders: Vec<String> = states.iter().enumerate()
                .map(|(i, _)| format!("?{}", i + 1))
                .collect();
            let in_clause = placeholders.join(", ");
            
            let sql = format!(
                "SELECT id, url, original_url, filename, destination, state, progress, size, 
                        downloaded, speed, eta, host, category, priority, segments, retry_count,
                        created_at, started_at, completed_at, wait_until, error_message, 
                        batch_id, batch_name, tmdb_id, tmdb_title, tmdb_season, tmdb_episode,
                        quality, resolution, arr_series_id, arr_movie_id
                 FROM downloads 
                 WHERE state IN ({})
                 ORDER BY priority DESC, created_at ASC",
                in_clause
            );
            
            let mut stmt = conn.prepare(&sql)?;
            
            // Convert states to params
            let params: Vec<&dyn rusqlite::ToSql> = states.iter()
                .map(|s| s as &dyn rusqlite::ToSql)
                .collect();
            
            let task_iter = stmt.query_map(params.as_slice(), |row| {
                Self::parse_task_from_row_static(row)
            })?;
            
            let mut tasks = Vec::new();
            for task_result in task_iter {
                tasks.push(task_result?);
            }
            Ok(tasks)
        }).await.unwrap()
    }

    /// Get counts of downloads by status from database
    pub async fn get_status_counts_async(&self) -> Result<std::collections::HashMap<String, usize>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let mut stmt = conn.prepare(
                "SELECT state, COUNT(*) as count FROM downloads GROUP BY state"
            )?;
            
            let mut counts = std::collections::HashMap::new();
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            
            for row in rows {
                if let Ok((state, count)) = row {
                    counts.insert(state, count as usize);
                }
            }
            
            Ok(counts)
        }).await.unwrap()
    }


    /// Get counts of downloads by status from database (synchronous version)
    pub fn get_status_counts(&self) -> Result<std::collections::HashMap<String, usize>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT state, COUNT(*) as count FROM downloads GROUP BY state"
        )?;
        
        let mut counts = std::collections::HashMap::new();
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        
        for row in rows {
            if let Ok((state, count)) = row {
                counts.insert(state, count as usize);
            }
        }
        
        Ok(counts)
    }

    /// Get all tasks with a specific batch_id (async)
    pub async fn get_tasks_by_batch_id_async(&self, batch_id: String) -> Result<Vec<DownloadTask>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            
            let mut stmt = conn.prepare(
                "SELECT id, url, original_url, filename, destination, state, progress, size, 
                        downloaded, speed, eta, host, category, priority, segments, retry_count,
                        created_at, started_at, completed_at, wait_until, error_message, 
                        batch_id, batch_name, tmdb_id, tmdb_title, tmdb_season, tmdb_episode,
                        quality, resolution, arr_series_id, arr_movie_id
                 FROM downloads 
                 WHERE batch_id = ?1
                 ORDER BY created_at ASC"
            )?;
            
            let task_iter = stmt.query_map(params![&batch_id], |row| {
                Self::parse_task_from_row_static(row)
            })?;
            
            let mut tasks = Vec::new();
            for task_result in task_iter {
                tasks.push(task_result?);
            }
            Ok(tasks)
        }).await.unwrap()
    }

    /// Delete all tasks with a specific batch_id (async)
    pub async fn delete_tasks_by_batch_id_async(&self, batch_id: String) -> Result<usize> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let affected = conn.execute(
                "DELETE FROM downloads WHERE batch_id = ?1",
                params![&batch_id]
            )?;
            Ok(affected)
        }).await.unwrap()
    }

    /// Find an existing task by Fshare code (for duplicate detection)
    /// Uses exact match on the dedicated fshare_code column
    /// Returns the task state and id if found, so we can decide whether to skip or replace
    pub async fn find_task_by_fshare_code_async(&self, fshare_code: &str) -> Result<Option<(String, String)>> {
        let pool = self.pool.clone();
        let code = fshare_code.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let result: Option<(String, String)> = conn.query_row(
                "SELECT id, state FROM downloads WHERE fshare_code = ?1 LIMIT 1",
                params![&code],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).optional()?;
            Ok(result)
        }).await.unwrap()
    }

    /// Get batch_id for an existing batch by batch_name (async)
    /// Returns the batch_id if a batch with this name exists, None otherwise
    pub async fn get_batch_id_by_name_async(&self, batch_name: &str) -> Result<Option<String>> {
        let pool = self.pool.clone();
        let batch_name = batch_name.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let result: Option<String> = conn.query_row(
                "SELECT batch_id FROM downloads WHERE batch_name = ?1 LIMIT 1",
                params![&batch_name],
                |row| row.get(0)
            ).optional()?;
            Ok(result)
        }).await.unwrap()
    }

    /// Static version of parse_task_from_row for use in spawn_blocking
    fn parse_task_from_row_static(row: &rusqlite::Row) -> rusqlite::Result<DownloadTask> {
        Self::parse_task_from_row(row)
    }

    /// Get batch summaries with pagination for batch-first display
    /// Returns (batches, standalone_downloads, total_batches, total_standalone)
    pub async fn get_batch_summaries_paginated_async(
        &self,
        page: u32,
        limit: u32,
        status_filter: Option<&str>,
    ) -> Result<(Vec<crate::api::downloads::BatchSummary>, Vec<DownloadTask>, u64, u64)> {
        let pool = self.pool.clone();
        let status_filter = status_filter.map(|s| s.to_uppercase());
        
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            
            // Build WHERE clause for status filter
            let where_clause = match &status_filter {
                Some(status) => format!("WHERE state = '{}'", status),
                None => String::new(),
            };
            // AND variant for queries that already have a WHERE clause (e.g. standalone query)
            let and_clause = match &status_filter {
                Some(status) => format!("AND state = '{}'", status),
                None => String::new(),
            };
            
            // Step 1: Identify batch IDs that match the filter (if any)
            let matching_batches_query = if let Some(status) = &status_filter {
                format!("SELECT DISTINCT batch_id FROM downloads WHERE batch_id IS NOT NULL AND state = '{}'", status)
            } else {
                "SELECT DISTINCT batch_id FROM downloads WHERE batch_id IS NOT NULL".to_string()
            };
            
            let batch_query = format!(
                "SELECT 
                    batch_id,
                    batch_name,
                    COUNT(*) as total_items,
                    SUM(CASE WHEN state = 'COMPLETED' THEN 1 ELSE 0 END) as completed_items,
                    SUM(CASE WHEN state = 'FAILED' THEN 1 ELSE 0 END) as failed_items,
                    SUM(CASE WHEN state = 'DOWNLOADING' OR state = 'STARTING' THEN 1 ELSE 0 END) as downloading_items,
                    SUM(CASE WHEN state = 'PAUSED' THEN 1 ELSE 0 END) as paused_items,
                    SUM(CASE WHEN state = 'QUEUED' OR state = 'WAITING' THEN 1 ELSE 0 END) as queued_items,
                    COALESCE(SUM(size), 0) as total_size,
                    COALESCE(SUM(downloaded), 0) as downloaded_size,
                    MIN(created_at) as created_at
                FROM downloads
                WHERE batch_id IN ({})
                GROUP BY batch_id, batch_name
                ORDER BY created_at DESC",
                matching_batches_query
            );
            
            tracing::info!("[DB] Executing batch query: {}", batch_query);
            
            let mut stmt = conn.prepare(&batch_query)?;
            let batch_iter = stmt.query_map([], |row| {
                let total_items: i64 = row.get(2)?;
                let completed: i64 = row.get(3)?;
                let failed: i64 = row.get(4)?;
                let downloading: i64 = row.get(5)?;
                let paused: i64 = row.get(6)?;
                let queued: i64 = row.get(7)?;
                let total_size: i64 = row.get(8)?;
                let downloaded_size: i64 = row.get(9)?;
                
                // Calculate aggregate state
                let state = if failed > 0 {
                    "FAILED".to_string()
                } else if downloading > 0 {
                    "DOWNLOADING".to_string()
                } else if paused > 0 && completed < total_items {
                    "PAUSED".to_string()
                } else if completed == total_items && total_items > 0 {
                    "COMPLETED".to_string()
                } else if queued > 0 {
                    "QUEUED".to_string()
                } else {
                    "QUEUED".to_string()
                };
                
                // Calculate progress
                let progress = if total_size > 0 {
                    (downloaded_size as f64 / total_size as f64 * 100.0) as f32
                } else {
                    0.0
                };
                
                Ok(crate::api::downloads::BatchSummary {
                    batch_id: row.get(0)?,
                    batch_name: row.get(1)?,
                    total_items: total_items as usize,
                    completed_items: completed as usize,
                    failed_items: failed as usize,
                    downloading_items: downloading as usize,
                    paused_items: paused as usize,
                    queued_items: queued as usize,
                    total_size: total_size as u64,
                    downloaded_size: downloaded_size as u64,
                    progress,
                    speed: 0.0, // Will be updated from active tasks in API layer
                    created_at: row.get(10)?,
                    state,
                })
            })?;
            
            let mut all_batches = Vec::new();
            for batch in batch_iter {
                match batch {
                    Ok(b) => {
                        tracing::debug!("[DB] Parsed batch: {} - {}", b.batch_id, b.batch_name);
                        all_batches.push(b);
                    }
                    Err(e) => {
                        tracing::error!("[DB] Failed to parse batch row: {}", e);
                    }
                }
            }
            
            tracing::info!("[DB] Batch query returned {} batches", all_batches.len());
            for batch in &all_batches {
                tracing::info!("[DB] Batch: {} - {} ({} items)", batch.batch_id, batch.batch_name, batch.total_items);
            }
            
            let total_batches = all_batches.len() as u64;
            
            // Step 2: Get standalone downloads (no batch_id)
            let standalone_query = format!(
                "SELECT id, url, original_url, filename, destination, state, progress, size, 
                 downloaded, speed, eta, host, category, priority, segments, retry_count, created_at, 
                 started_at, completed_at, wait_until, error_message, batch_id, batch_name,
                 tmdb_id, tmdb_title, tmdb_season, tmdb_episode, quality, resolution, arr_series_id, arr_movie_id
                 FROM downloads 
                 WHERE batch_id IS NULL {}
                 ORDER BY created_at DESC",
                and_clause
            );
            
            let mut stmt = conn.prepare(&standalone_query)?;
            let standalone_iter = stmt.query_map([], |row| Self::parse_task_from_row_static(row))?;
            
            let mut all_standalone = Vec::new();
            for task in standalone_iter {
                if let Ok(t) = task {
                    all_standalone.push(t);
                }
            }
            
            let total_standalone = all_standalone.len() as u64;
            
            // Step 3: Apply pagination to combined list
            // Calculate total display units
            let _total_units = total_batches + total_standalone;
            let offset = ((page.saturating_sub(1)) * limit) as u64;
            
            // Slice batches and standalone based on pagination
            let mut batches_to_return = Vec::new();
            let mut standalone_to_return = Vec::new();
            
            if offset < total_batches {
                // We're still in the batches section
                let batch_start = offset as usize;
                let batch_end = std::cmp::min((offset + limit as u64) as usize, total_batches as usize);
                batches_to_return = all_batches[batch_start..batch_end].to_vec();
                
                // If we have room for standalone items
                let remaining = limit as usize - (batch_end - batch_start);
                if remaining > 0 && !all_standalone.is_empty() {
                    let standalone_end = std::cmp::min(remaining, all_standalone.len());
                    standalone_to_return = all_standalone[0..standalone_end].to_vec();
                }
            } else {
                // We're in the standalone section
                let standalone_offset = (offset - total_batches) as usize;
                let standalone_end = std::cmp::min(standalone_offset + limit as usize, all_standalone.len());
                if standalone_offset < all_standalone.len() {
                    standalone_to_return = all_standalone[standalone_offset..standalone_end].to_vec();
                }
            }
            
            Ok((batches_to_return, standalone_to_return, total_batches, total_standalone))
        }).await.unwrap()
    }

    // ============================================================================
    // Account Management
    // ============================================================================

    /// Save or update an account
    pub fn save_account(&self, email: &str, session_id: Option<&str>, token: Option<&str>, expires_at: Option<i64>) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO accounts (email, session_id, token, expires_at) 
             VALUES (?1, ?2, ?3, ?4)",
            params![email, session_id, token, expires_at],
        )?;
        Ok(())
    }

    /// Get account by email
    pub fn get_account(&self, email: &str) -> Result<Option<Account>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT email, session_id, token, expires_at, created_at FROM accounts WHERE email = ?1"
        )?;
        
        stmt.query_row(params![email], |row| {
            Ok(Account {
                email: row.get(0)?,
                session_id: row.get(1)?,
                token: row.get(2)?,
                expires_at: row.get(3)?,
                created_at: row.get(4)?,
            })
        }).optional()
    }

    /// Get all accounts
    pub fn get_all_accounts(&self) -> Result<Vec<Account>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT email, session_id, token, expires_at, created_at FROM accounts"
        )?;
        
        let account_iter = stmt.query_map([], |row| {
            Ok(Account {
                email: row.get(0)?,
                session_id: row.get(1)?,
                token: row.get(2)?,
                expires_at: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        let mut accounts = Vec::new();
        for account in account_iter {
            if let Ok(a) = account {
                accounts.push(a);
            }
        }
        Ok(accounts)
    }

    /// Delete account by email
    pub fn delete_account(&self, email: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("DELETE FROM accounts WHERE email = ?1", params![email])?;
        Ok(())
    }

    // ============================================================================
    // Settings Management
    // ============================================================================

    /// Save or update a setting
    pub fn save_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) 
             VALUES (?1, ?2, strftime('%s', 'now'))",
            params![key, value],
        )?;
        Ok(())
    }

    /// Get setting by key
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        stmt.query_row(params![key], |row| row.get(0)).optional()
    }

    /// Get all settings as a HashMap
    pub fn get_all_settings(&self) -> Result<std::collections::HashMap<String, String>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let setting_iter = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut settings = std::collections::HashMap::new();
        for setting in setting_iter {
            if let Ok((k, v)) = setting {
                settings.insert(k, v);
            }
        }
        Ok(settings)
    }

    /// Delete setting by key
    pub fn delete_setting(&self, key: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(())
    }
    
    /// Check if onboarding is complete
    pub fn is_onboarding_complete(&self) -> Result<bool> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let result: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'onboarding_complete'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        
        Ok(result.map(|v| v == "true").unwrap_or(false))
    }
    
    /// Mark onboarding as complete
    pub fn mark_onboarding_complete(&self) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('onboarding_complete', 'true')",
            [],
        )?;
        Ok(())
    }
    
    /// Save Fshare credentials
    pub fn save_fshare_credentials(&self, email: &str, password: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('fshare_email', ?1)",
            params![email],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('fshare_password', ?1)",
            params![password],
        )?;
        Ok(())
    }
    
    /// Save download settings
    pub fn save_download_settings(&self, directory: &str, max_concurrent: u32) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('download_directory', ?1)",
            params![directory],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('max_concurrent', ?1)",
            params![max_concurrent.to_string()],
        )?;
        Ok(())
    }
    
    /// Save Arr (Sonarr/Radarr) configuration
    pub fn save_arr_config(&self, service: &str, url: &str, api_key: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            &format!("INSERT OR REPLACE INTO settings (key, value) VALUES ('{}_url', ?1)", service),
            params![url],
        )?;
        conn.execute(
            &format!("INSERT OR REPLACE INTO settings (key, value) VALUES ('{}_api_key', ?1)", service),
            params![api_key],
        )?;
        Ok(())
    }
    
    /// Save Jellyfin configuration
    pub fn save_jellyfin_config(&self, url: &str, api_key: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('jellyfin_url', ?1)",
            params![url],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('jellyfin_api_key', ?1)",
            params![api_key],
        )?;
        Ok(())
    }
    
    /// Get indexer API key (or generate if not exists)
    pub fn get_indexer_api_key(&self) -> Result<String> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let result: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'indexer_api_key'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        
        match result {
            Some(key) => Ok(key),
            None => Err(rusqlite::Error::QueryReturnedNoRows),
        }
    }
    
    /// Save indexer API key
    pub fn save_indexer_api_key(&self, api_key: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('indexer_api_key', ?1)",
            params![api_key],
        )?;
        Ok(())
    }

    // ============================================================================
    // Arr Sync Tracking
    // ============================================================================

    /// Update Arr announcement status for a download
    pub fn update_download_arr_status(
        &self,
        download_id: &str,
        announced: bool,
        series_id: Option<i32>,
        movie_id: Option<i32>,
    ) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "UPDATE downloads SET arr_announced = ?1, arr_series_id = ?2, arr_movie_id = ?3, arr_announce_error = NULL WHERE id = ?4",
            params![announced as i32, series_id, movie_id, download_id],
        )?;
        Ok(())
    }

    /// Update arr_series_id for all downloads with the same TMDB ID
    /// This ensures all episodes in a series batch get the series ID for auto-import
    pub fn update_arr_series_id_by_tmdb(&self, tmdb_id: i64, series_id: i64) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let updated = conn.execute(
            "UPDATE downloads SET arr_series_id = ?1, arr_announced = 1 WHERE tmdb_id = ?2",
            params![series_id, tmdb_id],
        )?;
        
        if updated == 0 {
            tracing::warn!("No downloads found with tmdb_id={} to update arr_series_id", tmdb_id);
        } else {
            tracing::info!("Updated {} downloads with tmdb_id={} to arr_series_id={}", updated, tmdb_id, series_id);
        }
        Ok(())
    }

    /// Update Radarr movie ID for all downloads with given TMDB ID
    pub fn update_arr_movie_id_by_tmdb(&self, tmdb_id: i64, movie_id: i64) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let updated = conn.execute(
            "UPDATE downloads SET arr_movie_id = ?1, arr_announced = 1 WHERE tmdb_id = ?2",
            params![movie_id, tmdb_id],
        )?;
        
        if updated == 0 {
            tracing::warn!("No downloads found with tmdb_id={} to update arr_movie_id", tmdb_id);
        } else {
            tracing::info!("Updated {} downloads with tmdb_id={} to arr_movie_id={}", updated, tmdb_id, movie_id);
        }
        Ok(())
    }

    /// Update Arr announcement error for a download
    pub fn update_download_arr_error(
        &self,
        download_id: &str,
        error: &str,
    ) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "UPDATE downloads SET arr_announced = 0, arr_announce_error = ?1 WHERE id = ?2",
            params![error, download_id],
        )?;
        Ok(())
    }

    // ============================================================================
    // Media Items (TMDB-Centric)
    // ============================================================================

    /// Upsert a media item (INSERT OR REPLACE by tmdb_id)
    pub fn upsert_media_item(&self, item: &MediaItem) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO media_items (
                tmdb_id, media_type, title, original_title, year, overview,
                poster_path, backdrop_path, genres, runtime, total_seasons,
                arr_id, arr_type, arr_path, arr_monitored, arr_status,
                arr_quality_profile_id, arr_has_file, arr_size_on_disk,
                tvdb_id, imdb_id, created_at, updated_at, arr_synced_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10, ?11,
                ?12, ?13, ?14, ?15, ?16,
                ?17, ?18, ?19,
                ?20, ?21, ?22, ?23, ?24
            )",
            params![
                item.tmdb_id,
                item.media_type,
                item.title,
                item.original_title,
                item.year,
                item.overview,
                item.poster_path,
                item.backdrop_path,
                item.genres,
                item.runtime,
                item.total_seasons,
                item.arr_id,
                item.arr_type,
                item.arr_path,
                item.arr_monitored as i32,
                item.arr_status,
                item.arr_quality_profile_id,
                item.arr_has_file as i32,
                item.arr_size_on_disk,
                item.tvdb_id,
                item.imdb_id,
                item.created_at,
                item.updated_at,
                item.arr_synced_at,
            ],
        )?;
        Ok(())
    }

    /// Async upsert media item
    pub async fn upsert_media_item_async(&self, item: MediaItem) -> Result<()> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let db = Db { pool };
            db.upsert_media_item(&item)
        }).await.unwrap()
    }

    /// Get a media item by TMDB ID
    pub fn get_media_item(&self, tmdb_id: i64) -> Result<Option<MediaItem>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT tmdb_id, media_type, title, original_title, year, overview,
                    poster_path, backdrop_path, genres, runtime, total_seasons,
                    arr_id, arr_type, arr_path, arr_monitored, arr_status,
                    arr_quality_profile_id, arr_has_file, arr_size_on_disk,
                    tvdb_id, imdb_id, created_at, updated_at, arr_synced_at
             FROM media_items WHERE tmdb_id = ?1"
        )?;
        stmt.query_row(params![tmdb_id], |row| Self::parse_media_item(row)).optional()
    }

    /// Async get media item
    pub async fn get_media_item_async(&self, tmdb_id: i64) -> Result<Option<MediaItem>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let db = Db { pool };
            db.get_media_item(tmdb_id)
        }).await.unwrap()
    }

    /// Get all media items, ordered by updated_at DESC
    pub fn get_all_media_items(&self) -> Result<Vec<MediaItem>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT tmdb_id, media_type, title, original_title, year, overview,
                    poster_path, backdrop_path, genres, runtime, total_seasons,
                    arr_id, arr_type, arr_path, arr_monitored, arr_status,
                    arr_quality_profile_id, arr_has_file, arr_size_on_disk,
                    tvdb_id, imdb_id, created_at, updated_at, arr_synced_at
             FROM media_items ORDER BY updated_at DESC"
        )?;
        let iter = stmt.query_map([], |row| Self::parse_media_item(row))?;
        let mut items = Vec::new();
        for item in iter {
            items.push(item?);
        }
        Ok(items)
    }

    /// Async get all media items
    pub async fn get_all_media_items_async(&self) -> Result<Vec<MediaItem>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let db = Db { pool };
            db.get_all_media_items()
        }).await.unwrap()
    }

    /// Get media items by type ("movie" or "tv")
    pub fn get_media_items_by_type(&self, media_type: &str) -> Result<Vec<MediaItem>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT tmdb_id, media_type, title, original_title, year, overview,
                    poster_path, backdrop_path, genres, runtime, total_seasons,
                    arr_id, arr_type, arr_path, arr_monitored, arr_status,
                    arr_quality_profile_id, arr_has_file, arr_size_on_disk,
                    tvdb_id, imdb_id, created_at, updated_at, arr_synced_at
             FROM media_items WHERE media_type = ?1 ORDER BY updated_at DESC"
        )?;
        let iter = stmt.query_map(params![media_type], |row| Self::parse_media_item(row))?;
        let mut items = Vec::new();
        for item in iter {
            items.push(item?);
        }
        Ok(items)
    }

    /// Update *arr integration state for a media item
    pub fn update_media_arr_state(
        &self,
        tmdb_id: i64,
        arr_id: i32,
        arr_type: &str,
        arr_path: Option<&str>,
        arr_monitored: bool,
        arr_status: Option<&str>,
        arr_quality_profile_id: Option<i32>,
    ) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE media_items SET 
                arr_id = ?1, arr_type = ?2, arr_path = ?3, arr_monitored = ?4,
                arr_status = ?5, arr_quality_profile_id = ?6,
                updated_at = ?7, arr_synced_at = ?7
             WHERE tmdb_id = ?8",
            params![
                arr_id,
                arr_type,
                arr_path,
                arr_monitored as i32,
                arr_status,
                arr_quality_profile_id,
                now,
                tmdb_id,
            ],
        )?;
        Ok(())
    }

    /// Delete a media item and its episodes
    pub fn delete_media_item(&self, tmdb_id: i64) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("DELETE FROM media_episodes WHERE tmdb_id = ?1", params![tmdb_id])?;
        conn.execute("DELETE FROM media_items WHERE tmdb_id = ?1", params![tmdb_id])?;
        Ok(())
    }

    /// Get download count per media item (for library display)
    pub fn get_media_download_counts(&self) -> Result<std::collections::HashMap<i64, usize>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT tmdb_id, COUNT(*) FROM downloads WHERE tmdb_id IS NOT NULL GROUP BY tmdb_id"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut counts = std::collections::HashMap::new();
        for row in rows {
            if let Ok((id, count)) = row {
                counts.insert(id, count as usize);
            }
        }
        Ok(counts)
    }

    // ============================================================================
    // Media Episodes
    // ============================================================================

    /// Upsert a media episode (INSERT OR REPLACE on UNIQUE constraint)
    pub fn upsert_media_episode(&self, ep: &MediaEpisode) -> Result<()> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO media_episodes (
                tmdb_id, season_number, episode_number, title, overview, air_date,
                arr_episode_id, arr_has_file, arr_monitored, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                ep.tmdb_id,
                ep.season_number,
                ep.episode_number,
                ep.title,
                ep.overview,
                ep.air_date,
                ep.arr_episode_id,
                ep.arr_has_file as i32,
                ep.arr_monitored as i32,
                ep.created_at,
                ep.updated_at,
            ],
        )?;
        Ok(())
    }

    /// Get all episodes for a series
    pub fn get_episodes_for_series(&self, tmdb_id: i64) -> Result<Vec<MediaEpisode>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT id, tmdb_id, season_number, episode_number, title, overview, air_date,
                    arr_episode_id, arr_has_file, arr_monitored, created_at, updated_at
             FROM media_episodes WHERE tmdb_id = ?1
             ORDER BY season_number ASC, episode_number ASC"
        )?;
        let iter = stmt.query_map(params![tmdb_id], |row| Self::parse_media_episode(row))?;
        let mut episodes = Vec::new();
        for ep in iter {
            episodes.push(ep?);
        }
        Ok(episodes)
    }

    /// Async get episodes for series
    pub async fn get_episodes_for_series_async(&self, tmdb_id: i64) -> Result<Vec<MediaEpisode>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let db = Db { pool };
            db.get_episodes_for_series(tmdb_id)
        }).await.unwrap()
    }

    /// Get a specific episode
    pub fn get_episode(&self, tmdb_id: i64, season: i32, episode: i32) -> Result<Option<MediaEpisode>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT id, tmdb_id, season_number, episode_number, title, overview, air_date,
                    arr_episode_id, arr_has_file, arr_monitored, created_at, updated_at
             FROM media_episodes WHERE tmdb_id = ?1 AND season_number = ?2 AND episode_number = ?3"
        )?;
        stmt.query_row(params![tmdb_id, season, episode], |row| Self::parse_media_episode(row)).optional()
    }

    /// Get a media item with all its associated downloads
    pub fn get_media_with_downloads(&self, tmdb_id: i64) -> Result<Option<(MediaItem, Vec<crate::downloader::task::DownloadTask>)>> {
        let item = self.get_media_item(tmdb_id)?;
        match item {
            Some(media) => {
                let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                let mut stmt = conn.prepare(
                    "SELECT id, url, original_url, filename, destination, state, progress, size,
                            downloaded, speed, eta, host, category, priority, segments, retry_count,
                            created_at, started_at, completed_at, wait_until, error_message,
                            batch_id, batch_name, tmdb_id, tmdb_title, tmdb_season, tmdb_episode,
                            quality, resolution, arr_series_id, arr_movie_id
                     FROM downloads WHERE tmdb_id = ?1
                     ORDER BY created_at DESC"
                )?;
                let task_iter = stmt.query_map(params![tmdb_id], |row| Self::parse_task_from_row(row))?;
                let mut tasks = Vec::new();
                for task in task_iter {
                    if let Ok(t) = task {
                        tasks.push(t);
                    }
                }
                Ok(Some((media, tasks)))
            }
            None => Ok(None),
        }
    }

    /// Async get media with downloads
    pub async fn get_media_with_downloads_async(&self, tmdb_id: i64) -> Result<Option<(MediaItem, Vec<crate::downloader::task::DownloadTask>)>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let db = Db { pool };
            db.get_media_with_downloads(tmdb_id)
        }).await.unwrap()
    }

    /// Get all downloads for a given TMDB ID directly from the downloads table.
    /// Does NOT require a media_items row — works for queued downloads before library sync.
    pub fn get_downloads_by_tmdb_id(&self, tmdb_id: i64) -> Result<Vec<crate::downloader::task::DownloadTask>> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size,
                    downloaded, speed, eta, host, category, priority, segments, retry_count,
                    created_at, started_at, completed_at, wait_until, error_message,
                    batch_id, batch_name, tmdb_id, tmdb_title, tmdb_season, tmdb_episode,
                    quality, resolution, arr_series_id, arr_movie_id
             FROM downloads WHERE tmdb_id = ?1
             ORDER BY created_at DESC"
        )?;
        let task_iter = stmt.query_map(params![tmdb_id], |row| Self::parse_task_from_row(row))?;
        let mut tasks = Vec::new();
        for task in task_iter {
            if let Ok(t) = task {
                tasks.push(t);
            }
        }
        Ok(tasks)
    }

    /// Async version of get_downloads_by_tmdb_id
    pub async fn get_downloads_by_tmdb_id_async(&self, tmdb_id: i64) -> Result<Vec<crate::downloader::task::DownloadTask>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let db = Db { pool };
            db.get_downloads_by_tmdb_id(tmdb_id)
        }).await.unwrap()
    }


    // ── Row Parsers ───────────────────────────────────────────────────────

    fn parse_media_item(row: &rusqlite::Row) -> rusqlite::Result<MediaItem> {
        Ok(MediaItem {
            tmdb_id: row.get(0)?,
            media_type: row.get(1)?,
            title: row.get(2)?,
            original_title: row.get(3)?,
            year: row.get(4)?,
            overview: row.get(5)?,
            poster_path: row.get(6)?,
            backdrop_path: row.get(7)?,
            genres: row.get(8)?,
            runtime: row.get(9)?,
            total_seasons: row.get(10)?,
            arr_id: row.get(11)?,
            arr_type: row.get(12)?,
            arr_path: row.get(13)?,
            arr_monitored: row.get::<_, Option<i32>>(14)?.unwrap_or(0) != 0,
            arr_status: row.get(15)?,
            arr_quality_profile_id: row.get(16)?,
            arr_has_file: row.get::<_, Option<i32>>(17)?.unwrap_or(0) != 0,
            arr_size_on_disk: row.get::<_, Option<i64>>(18)?.unwrap_or(0),
            tvdb_id: row.get(19)?,
            imdb_id: row.get(20)?,
            created_at: row.get(21)?,
            updated_at: row.get(22)?,
            arr_synced_at: row.get(23)?,
        })
    }

    fn parse_media_episode(row: &rusqlite::Row) -> rusqlite::Result<MediaEpisode> {
        Ok(MediaEpisode {
            id: row.get(0)?,
            tmdb_id: row.get(1)?,
            season_number: row.get(2)?,
            episode_number: row.get(3)?,
            title: row.get(4)?,
            overview: row.get(5)?,
            air_date: row.get(6)?,
            arr_episode_id: row.get(7)?,
            arr_has_file: row.get::<_, Option<i32>>(8)?.unwrap_or(0) != 0,
            arr_monitored: row.get::<_, Option<i32>>(9)?.unwrap_or(0) != 0,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        })
    }
}

/// Account data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub email: String,
    pub session_id: Option<String>,
    pub token: Option<String>,
    pub expires_at: Option<i64>,
    pub created_at: Option<i64>,
}
