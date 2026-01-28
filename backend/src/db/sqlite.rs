use rusqlite::{params, Connection, Result, OptionalExtension};
use std::path::Path;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use serde::{Deserialize, Serialize};
use crate::downloader::task::{DownloadTask, DownloadState};

pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn: Arc::new(Mutex::new(conn)) };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
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
        
        // Create indexes for performance
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_state ON downloads(state)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_created_at ON downloads(created_at)",
            [],
        )?;
        
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
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO downloads (
                id, url, original_url, filename, destination, state, progress, size, 
                downloaded, speed, eta,
                host, category, priority, segments, retry_count, created_at, 
                started_at, completed_at, wait_until, error_message
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                task.id.to_string(),
                task.url,
                task.original_url,
                task.filename,
                task.destination,
                format!("{:?}", task.state).to_uppercase(),
                task.progress,
                task.size as i64,
                None::<i64>, // downloaded - to be populated by Core Backend
                None::<f64>, // speed - to be populated by Core Backend
                None::<i64>, // eta - to be populated by Core Backend
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
            url_metadata: None,  // Not stored in DB yet, will be set on URL resolution
            error_history: Vec::new(),  // Not stored in DB, will be populated on errors
            fshare_code: None,  // Will be extracted from URL when needed
            state_obj: crate::downloader::state_machine::TaskStateFactory::get_state(state),
            cancel_token: CancellationToken::new(),
            pause_notify: Arc::new(Notify::new()),
        })
    }

    pub fn get_all_tasks(&self) -> Result<Vec<DownloadTask>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size, 
             downloaded, speed, eta,
             host, category, priority, segments, retry_count, created_at, 
             started_at, completed_at, wait_until, error_message 
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
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size, 
             downloaded, speed, eta,
             host, category, priority, segments, retry_count, created_at, 
             started_at, completed_at, wait_until, error_message 
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
        let conn = self.conn.lock().unwrap();
        
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
             started_at, completed_at, wait_until, error_message 
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
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size, 
             downloaded, speed, eta,
             host, category, priority, segments, retry_count, created_at, 
             started_at, completed_at, wait_until, error_message 
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
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        
        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size, 
             downloaded, speed, eta,
             host, category, priority, segments, retry_count, created_at, 
             started_at, completed_at, wait_until, error_message 
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
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET state = ?1 WHERE id = ?2",
            params![state, id.to_string()],
        )?;
        Ok(())
    }

    /// Get count of tasks by state
    pub fn count_tasks_by_state(&self, state: &str) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM downloads WHERE state = ?1",
            params![state],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn delete_task(&self, id: Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM downloads WHERE id = ?1", params![id.to_string()])?;
        Ok(())
    }

    // ============================================================================
    // Async Wrappers (prevent async runtime blocking)
    // ============================================================================

    /// Async version of get_tasks_paginated - uses spawn_blocking
    pub async fn get_tasks_paginated_async(&self, page: u32, limit: u32) -> Result<(Vec<DownloadTask>, u64)> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            
            // Get total count
            let total: u64 = conn.query_row(
                "SELECT COUNT(*) FROM downloads",
                [],
                |row| row.get(0),
            )?;
            
            let offset = (page.saturating_sub(1)) * limit;
            
            let mut stmt = conn.prepare(
                "SELECT id, url, original_url, filename, destination, state, progress, size, 
                 downloaded, speed, eta,
                 host, category, priority, segments, retry_count, created_at, 
                 started_at, completed_at, wait_until, error_message 
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
            
            let task_iter = stmt.query_map(params![limit as i64, offset as i64], |row| Self::parse_task_from_row_static(row))?;

            let mut tasks = Vec::new();
            for task in task_iter {
                if let Ok(t) = task {
                    tasks.push(t);
                }
            }
            
            Ok((tasks, total))
        }).await.unwrap()
    }

    /// Async version of save_task
    pub async fn save_task_async(&self, task: DownloadTask) -> Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT OR REPLACE INTO downloads (
                    id, url, original_url, filename, destination, state, progress, size,
                    downloaded, speed, eta,
                    host, category, priority, segments, retry_count, created_at, 
                    started_at, completed_at, wait_until, error_message
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
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
                ],
            )?;
            Ok(())
        }).await.unwrap()
    }

    /// Async version of update_task_state
    pub async fn update_task_state_async(&self, id: Uuid, state: String) -> Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "UPDATE downloads SET state = ?1 WHERE id = ?2",
                params![state, id.to_string()],
            )?;
            Ok(())
        }).await.unwrap()
    }

    /// Static version of parse_task_from_row for use in spawn_blocking
    fn parse_task_from_row_static(row: &rusqlite::Row) -> rusqlite::Result<DownloadTask> {
        Self::parse_task_from_row(row)
    }

    // ============================================================================
    // Account Management
    // ============================================================================

    /// Save or update an account
    pub fn save_account(&self, email: &str, session_id: Option<&str>, token: Option<&str>, expires_at: Option<i64>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO accounts (email, session_id, token, expires_at) 
             VALUES (?1, ?2, ?3, ?4)",
            params![email, session_id, token, expires_at],
        )?;
        Ok(())
    }

    /// Get account by email
    pub fn get_account(&self, email: &str) -> Result<Option<Account>> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM accounts WHERE email = ?1", params![email])?;
        Ok(())
    }

    // ============================================================================
    // Settings Management
    // ============================================================================

    /// Save or update a setting
    pub fn save_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) 
             VALUES (?1, ?2, strftime('%s', 'now'))",
            params![key, value],
        )?;
        Ok(())
    }

    /// Get setting by key
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        stmt.query_row(params![key], |row| row.get(0)).optional()
    }

    /// Get all settings as a HashMap
    pub fn get_all_settings(&self) -> Result<std::collections::HashMap<String, String>> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(())
    }
    
    /// Check if onboarding is complete
    pub fn is_onboarding_complete(&self) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('onboarding_complete', 'true')",
            [],
        )?;
        Ok(())
    }
    
    /// Save Fshare credentials
    pub fn save_fshare_credentials(&self, email: &str, password: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('indexer_api_key', ?1)",
            params![api_key],
        )?;
        Ok(())
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
