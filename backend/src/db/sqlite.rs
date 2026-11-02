use super::media::{MediaEpisode, MediaItem};
use crate::downloader::task::{DownloadState, DownloadTask};
use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, OptionalExtension, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

const TASK_SELECT_COLUMNS: &str = "id, url, original_url, filename, destination, state, progress, size,
    downloaded, speed, eta, host, category, priority, segments, retry_count,
    created_at, started_at, completed_at, wait_until, error_message,
    batch_id, batch_name, tmdb_id, media_type, media_item_id, tmdb_title, tmdb_season, tmdb_episode, info_path,
    arr_announced, arr_series_id, arr_movie_id, arr_announce_error, fshare_code,
    quality, resolution, tvdb_id, download_source, external_provider, external_id, external_instance_id, info_hash, origin_provider, origin_indexer, seeders, leechers,
    provider_id, provider_item_id, file_idx, source_filename, selected_episodes, execution_strategy, torbox_remote_state, torbox_remote_completed_at, selected_remote_file_ids, selected_artifacts";
const APP_SESSION_IDLE_TTL_SECS: i64 = 60 * 60 * 24 * 7;
pub const MAX_STREMIO_ADDONS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StremioAddonInsert {
    Inserted,
    Duplicate,
    LimitReached,
}

use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct Db {
    pool: Pool<SqliteConnectionManager>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEvent {
    pub id: i64,
    pub timestamp: String,
    pub kind: String,
    pub task_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QbitInstance {
    pub id: String,
    pub name: String,
    pub url: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password: String,
    pub enabled: bool,
    pub default_category: Option<String>,
    pub save_path: Option<String>,
    pub poll_interval_secs: u64,
    pub last_rid: i64,
    pub sync_health: String,
    pub last_sync_at: Option<String>,
    pub app_version: Option<String>,
    pub webapi_version: Option<String>,
    pub last_seen_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QbitTorrentRow {
    pub instance_id: String,
    pub hash: String,
    pub name: String,
    pub state: String,
    pub progress: f64,
    pub size: i64,
    pub downloaded: i64,
    pub uploaded: i64,
    pub dlspeed: i64,
    pub upspeed: i64,
    pub eta: i64,
    pub ratio: f64,
    pub category: String,
    pub tags: String,
    pub save_path: String,
    pub content_path: String,
    pub tracker: String,
    pub added_on: i64,
    pub completion_on: i64,
    pub amount_left: i64,
    pub availability: f64,
    pub num_seeds: i64,
    pub num_leechs: i64,
    pub seen_complete: i64,
    pub last_activity: i64,
    #[serde(skip_serializing)]
    pub raw_json: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct QbitSyncApplied {
    pub upserted: Vec<QbitTorrentRow>,
    pub removed: Vec<String>,
    pub replaced: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct QbitTorrentListQuery {
    pub page: u32,
    pub limit: u32,
    pub sort: String,
    pub order: String,
    pub search: Option<String>,
    pub status: Option<Vec<String>>,
    pub categories: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub trackers: Option<Vec<String>>,
}

fn row_source_filter_clause(source_filter: Option<&str>) -> Option<&'static str> {
    match source_filter {
        Some("torrent") => Some(
            "lower(COALESCE(download_source, external_provider, host, '')) IN ('torrent', 'qbittorrent')",
        ),
        Some("fshare") => Some(
            "lower(COALESCE(download_source, external_provider, host, '')) IN ('fshare', 'flasharr-http')",
        ),
        Some("torbox") => {
            Some("lower(COALESCE(download_source, external_provider, host, '')) = 'torbox'")
        }
        Some("direct") => Some(
            "lower(COALESCE(download_source, external_provider, host, '')) IN ('fshare', 'flasharr-http', 'torbox')",
        ),
        _ => None,
    }
}

impl Db {
    pub(super) fn connection(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
    }

    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let manager = SqliteConnectionManager::file(path).with_init(|conn| {
            conn.execute_batch(
                "PRAGMA journal_mode=WAL;
                     PRAGMA busy_timeout=5000;
                     PRAGMA synchronous=NORMAL;",
            )
        });
        let pool = Pool::builder()
            .max_size(5)
            .build(manager)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let db = Self { pool };
        db.init()?;

        tracing::info!("Database connection pool initialized (max_size: 5, WAL mode)");
        Ok(db)
    }

    fn init(&self) -> Result<()> {
        let mut conn = self
            .pool
            .get()
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
        let _ = conn.execute(
            "ALTER TABLE downloads ADD COLUMN media_type TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN media_item_id INTEGER", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN media_key TEXT", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN tvdb_id INTEGER", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN tmdb_title TEXT", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN tmdb_season INTEGER", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN tmdb_episode INTEGER", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN info_path TEXT", []);
        let _ = conn.execute("UPDATE downloads SET media_type = CASE WHEN category = 'tv' THEN 'tv' WHEN category = 'movies' THEN 'movie' ELSE media_type END WHERE tmdb_id IS NOT NULL AND media_type = ''", []);
        let _ = conn.execute("UPDATE downloads SET media_item_id = tmdb_id WHERE tmdb_id IS NOT NULL AND media_item_id IS NULL", []);
        let _ = conn.execute("UPDATE downloads SET media_key = CASE media_type WHEN 'tv' THEN CASE WHEN tvdb_id IS NOT NULL THEN 'tv:' || tvdb_id ELSE 'tv-tmdb:' || tmdb_id END WHEN 'movie' THEN 'movie:' || tmdb_id ELSE media_key END WHERE tmdb_id IS NOT NULL AND (media_key IS NULL OR media_key = '')", []);
        let _ = conn.execute("UPDATE downloads SET info_path = CASE media_type WHEN 'tv' THEN CASE WHEN tvdb_id IS NOT NULL THEN '/tv/' || tvdb_id ELSE '/tv/' || tmdb_id END WHEN 'movie' THEN '/movie/' || tmdb_id ELSE info_path END WHERE (tmdb_id IS NOT NULL OR tvdb_id IS NOT NULL) AND (info_path IS NULL OR info_path = '')", []);
        // Arr sync tracking columns (v2.1.0)
        let _ = conn.execute(
            "ALTER TABLE downloads ADD COLUMN arr_announced BOOLEAN DEFAULT 0",
            [],
        );
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN arr_series_id INTEGER", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN arr_movie_id INTEGER", []);
        let _ = conn.execute(
            "ALTER TABLE downloads ADD COLUMN arr_announce_error TEXT",
            [],
        );
        // Fshare code for duplicate detection (v2.2.0)
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN fshare_code TEXT", []);
        {
            let tx = conn.transaction()?;
            let columns = {
                let mut stmt = tx.prepare("PRAGMA table_info(downloads)")?;
                let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
                rows.collect::<Result<std::collections::HashSet<_>>>()?
            };
            if !columns.contains("provider_id") {
                tx.execute("ALTER TABLE downloads ADD COLUMN provider_id TEXT", [])?;
            }
            if !columns.contains("provider_item_id") {
                tx.execute("ALTER TABLE downloads ADD COLUMN provider_item_id TEXT", [])?;
            }
            tx.execute(
                "UPDATE downloads
                 SET provider_id = 'fshare', provider_item_id = fshare_code
                 WHERE fshare_code IS NOT NULL
                   AND (provider_id IS NULL OR provider_id = '')
                   AND (provider_item_id IS NULL OR provider_item_id = '')",
                [],
            )?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS idx_downloads_provider_item
                 ON downloads(provider_id, provider_item_id)",
                [],
            )?;
            tx.commit()?;
        }
        // Quality metadata columns (v2.3.0)
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN quality TEXT", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN resolution TEXT", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN download_source TEXT", []);
        let _ = conn.execute(
            "ALTER TABLE downloads ADD COLUMN external_provider TEXT",
            [],
        );
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN external_id TEXT", []);
        let _ = conn.execute(
            "ALTER TABLE downloads ADD COLUMN external_instance_id TEXT",
            [],
        );
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN info_hash TEXT", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN origin_provider TEXT", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN origin_indexer TEXT", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN seeders INTEGER", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN leechers INTEGER", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN file_idx INTEGER", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN source_filename TEXT", []);
        let _ = conn.execute(
            "ALTER TABLE downloads ADD COLUMN selected_episodes TEXT",
            [],
        );
        {
            let tx = conn.transaction()?;
            let columns = {
                let mut stmt = tx.prepare("PRAGMA table_info(downloads)")?;
                let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
                rows.collect::<Result<std::collections::HashSet<_>>>()?
            };
            for (name, definition) in [
                ("execution_strategy", "TEXT"),
                ("torbox_remote_state", "TEXT"),
                ("torbox_remote_completed_at", "TEXT"),
                ("selected_remote_file_ids", "TEXT"),
                ("selected_artifacts", "TEXT"),
                ("media_leaf_key", "TEXT"),
                ("superseded_by", "TEXT"),
                ("is_current", "INTEGER NOT NULL DEFAULT 1"),
            ] {
                if !columns.contains(name) {
                    tx.execute(
                        &format!("ALTER TABLE downloads ADD COLUMN {name} {definition}"),
                        [],
                    )?;
                }
            }
            tx.execute("UPDATE downloads SET media_leaf_key = CASE WHEN media_type = 'movie' AND tmdb_id IS NOT NULL THEN 'movie:' || tmdb_id WHEN media_type = 'tv' AND tvdb_id IS NOT NULL AND tmdb_season IS NOT NULL AND tmdb_episode IS NOT NULL THEN 'episode:' || tvdb_id || ':' || tmdb_season || ':' || tmdb_episode END WHERE media_leaf_key IS NULL", [])?;
            // Legacy migration winner: preserve active work first, then a usable completed
            // artifact; failed historical retries must not hide an older completed item.
            const LEGACY_WINNER_ORDER: &str = "CASE WHEN state IN ('QUEUED','STARTING','DOWNLOADING','PAUSED','WAITING','IMPORTING') THEN 0 WHEN state = 'COMPLETED' THEN 1 ELSE 2 END, created_at DESC, rowid DESC";
            tx.execute(
                &format!("UPDATE downloads SET is_current = 0, superseded_by = (SELECT newer.id FROM downloads newer WHERE newer.media_leaf_key = downloads.media_leaf_key ORDER BY {LEGACY_WINNER_ORDER} LIMIT 1) WHERE media_leaf_key IS NOT NULL AND id <> (SELECT newer.id FROM downloads newer WHERE newer.media_leaf_key = downloads.media_leaf_key ORDER BY {LEGACY_WINNER_ORDER} LIMIT 1)"),
                [],
            )?;
            tx.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_downloads_current_media_leaf ON downloads(media_leaf_key) WHERE is_current = 1 AND media_leaf_key IS NOT NULL", [])?;
            tx.commit()?;
        }

        let completed_progress_backfilled = conn.execute(
            "UPDATE downloads
             SET progress = 100.0,
                 downloaded = CASE WHEN size > 0 THEN size ELSE COALESCE(downloaded, 0) END
             WHERE state = 'COMPLETED'
               AND (progress < 100.0 OR downloaded IS NULL OR downloaded < size)",
            [],
        )?;
        if completed_progress_backfilled > 0 {
            tracing::info!(
                "[DB] Backfilled progress/downloaded for {} completed downloads",
                completed_progress_backfilled
            );
        }

        // Backfill quality/resolution from filenames for existing records
        {
            let mut stmt = conn.prepare(
                "SELECT id, filename FROM downloads WHERE quality IS NULL AND filename IS NOT NULL",
            )?;
            let rows: Vec<(String, String)> = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();

            if !rows.is_empty() {
                tracing::info!(
                    "[DB] Backfilling quality metadata for {} downloads",
                    rows.len()
                );
                let mut update_stmt = conn
                    .prepare("UPDATE downloads SET quality = ?1, resolution = ?2 WHERE id = ?3")?;
                for (id, filename) in &rows {
                    let attrs =
                        crate::utils::parser::FilenameParser::extract_quality_attributes(filename);
                    let quality = attrs.quality_name();
                    let resolution = attrs.resolution.clone();
                    let _ = update_stmt.execute(rusqlite::params![quality, resolution, id]);
                }
                tracing::info!("[DB] Quality backfill complete");
            }
        }

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS activity_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at TEXT NOT NULL,
                kind TEXT NOT NULL CHECK (kind IN ('queued', 'completed', 'failed')),
                task_id TEXT NOT NULL,
                message TEXT NOT NULL,
                UNIQUE(task_id, kind)
            );
            CREATE INDEX IF NOT EXISTS idx_activity_events_created_at
                ON activity_events(created_at DESC);",
        )?;

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
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_external ON downloads(external_provider, external_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_source_state ON downloads(download_source, state)",
            [],
        )?;
        // Index on tmdb_id for joining downloads to media_items
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_tmdb_id ON downloads(tmdb_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_media_type ON downloads(media_type)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_media_item_id ON downloads(media_item_id)",
            [],
        )?;
        self.migrate_canonical_media_tables(&conn)?;
        self.migrate_acquisition(&mut conn)?;

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

        // Additive Stremio registry; encrypted_config_url is never projected publicly.
        conn.execute_batch("CREATE TABLE IF NOT EXISTS stremio_addons (id TEXT PRIMARY KEY, name TEXT NOT NULL, origin TEXT NOT NULL, encrypted_config_url TEXT NOT NULL, credential_present INTEGER NOT NULL DEFAULT 0, enabled INTEGER NOT NULL DEFAULT 1, sort_order INTEGER NOT NULL, created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')), updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))); CREATE UNIQUE INDEX IF NOT EXISTS idx_stremio_addons_order ON stremio_addons(sort_order);")?;
        let _ = conn.execute(
            "ALTER TABLE stremio_addons ADD COLUMN stream_types TEXT NOT NULL DEFAULT '[]'",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE stremio_addons ADD COLUMN capabilities_known INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE stremio_addons ADD COLUMN version TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE stremio_addons ADD COLUMN description TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = conn.execute("ALTER TABLE stremio_addons ADD COLUMN logo TEXT", []);

        // Settings table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT,
                updated_at INTEGER DEFAULT (strftime('%s', 'now'))
            )",
            [],
        )?;

        self.migrate_qbit_tables(&conn)?;
        self.migrate_auth_tables(&conn)?;

        // Canonical relational graph plus external-content FTS search index.
        let legacy_fts = conn
            .query_row(
                "SELECT COALESCE(sql, '') LIKE '%VIRTUAL TABLE%fts5%'
                    AND COALESCE(sql, '') NOT LIKE '%content=''folder_cache_items''%'
             FROM sqlite_master WHERE name='folder_cache'",
                [],
                |row| row.get::<_, bool>(0),
            )
            .optional()?
            .unwrap_or(false);
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS folder_cache_items (
                id INTEGER PRIMARY KEY, linkcode TEXT NOT NULL UNIQUE, name TEXT NOT NULL,
                title TEXT NOT NULL, category TEXT NOT NULL, label TEXT NOT NULL,
                parent_linkcode TEXT NOT NULL DEFAULT '', fshare_url TEXT NOT NULL,
                year TEXT NOT NULL DEFAULT '', season TEXT NOT NULL DEFAULT '', episode TEXT NOT NULL DEFAULT '',
                is_series TEXT NOT NULL DEFAULT '0', is_directory TEXT NOT NULL DEFAULT '0',
                size TEXT NOT NULL DEFAULT '0', quality TEXT NOT NULL DEFAULT '', path TEXT NOT NULL DEFAULT '',
                snapshot_generation INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_folder_cache_parent ON folder_cache_items(parent_linkcode);
            CREATE INDEX IF NOT EXISTS idx_folder_cache_directory ON folder_cache_items(is_directory);
            CREATE INDEX IF NOT EXISTS idx_folder_cache_generation ON folder_cache_items(snapshot_generation);"
        )?;
        if legacy_fts {
            conn.execute_batch("INSERT OR IGNORE INTO folder_cache_items(linkcode,name,title,category,label,parent_linkcode,fshare_url,year,season,episode,is_series,is_directory,size,quality,path) SELECT linkcode,name,title,category,label,parent_linkcode,fshare_url,year,season,episode,is_series,is_directory,size,quality,path FROM folder_cache; DROP TABLE folder_cache;")?;
        }
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS folder_cache USING fts5(
                linkcode, name, title, category, label, parent_linkcode,
                content='folder_cache_items', content_rowid='id', tokenize='unicode61'
            );
            CREATE TRIGGER IF NOT EXISTS folder_cache_ai AFTER INSERT ON folder_cache_items BEGIN INSERT INTO folder_cache(rowid,linkcode,name,title,category,label,parent_linkcode) VALUES(new.id,new.linkcode,new.name,new.title,new.category,new.label,new.parent_linkcode); END;
            CREATE TRIGGER IF NOT EXISTS folder_cache_ad AFTER DELETE ON folder_cache_items BEGIN INSERT INTO folder_cache(folder_cache,rowid,linkcode,name,title,category,label,parent_linkcode) VALUES('delete',old.id,old.linkcode,old.name,old.title,old.category,old.label,old.parent_linkcode); END;
            CREATE TRIGGER IF NOT EXISTS folder_cache_au AFTER UPDATE ON folder_cache_items BEGIN INSERT INTO folder_cache(folder_cache,rowid,linkcode,name,title,category,label,parent_linkcode) VALUES('delete',old.id,old.linkcode,old.name,old.title,old.category,old.label,old.parent_linkcode); INSERT INTO folder_cache(rowid,linkcode,name,title,category,label,parent_linkcode) VALUES(new.id,new.linkcode,new.name,new.title,new.category,new.label,new.parent_linkcode); END;
            INSERT INTO folder_cache(folder_cache) VALUES('rebuild');
            CREATE TABLE IF NOT EXISTS folder_cache_meta (
                key TEXT PRIMARY KEY,
                value TEXT
            );
            CREATE TABLE IF NOT EXISTS folder_tmdb_map (
                linkcode TEXT PRIMARY KEY,
                tmdb_id INTEGER,
                media_type TEXT NOT NULL DEFAULT '',
                poster_path TEXT DEFAULT '',
                mapped_at TEXT DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_folder_tmdb_identity
                ON folder_tmdb_map(tmdb_id, media_type);
            CREATE TABLE IF NOT EXISTS folder_modified_meta (
                linkcode TEXT PRIMARY KEY,
                modified INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS folder_tmdb_negative_cache (
                cache_key TEXT PRIMARY KEY,
                checked_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS folder_cache_media_index (
                linkcode TEXT PRIMARY KEY,
                parent_linkcode TEXT NOT NULL DEFAULT '', root_linkcode TEXT NOT NULL DEFAULT '',
                normalized_title TEXT NOT NULL DEFAULT '', media_type TEXT NOT NULL DEFAULT '',
                season INTEGER, episode INTEGER, is_directory INTEGER NOT NULL,
                parse_ambiguous INTEGER NOT NULL DEFAULT 0,
                snapshot_generation INTEGER NOT NULL DEFAULT 0, parser_version INTEGER NOT NULL DEFAULT 1
            );
            CREATE INDEX IF NOT EXISTS idx_folder_media_episode ON folder_cache_media_index(season,episode);
            CREATE INDEX IF NOT EXISTS idx_folder_media_parent_root ON folder_cache_media_index(parent_linkcode,root_linkcode);
            CREATE INDEX IF NOT EXISTS idx_folder_media_title ON folder_cache_media_index(normalized_title);
            CREATE INDEX IF NOT EXISTS idx_folder_media_generation ON folder_cache_media_index(snapshot_generation,parser_version);
            INSERT OR IGNORE INTO folder_cache_media_index
                (linkcode,parent_linkcode,normalized_title,media_type,season,episode,is_directory,snapshot_generation,parser_version)
            SELECT linkcode,parent_linkcode,lower(trim(title)),CASE WHEN is_series='1' THEN 'tv' ELSE 'movie' END,
                   NULLIF(season,''),NULLIF(episode,''),is_directory='1',snapshot_generation,1 FROM folder_cache_items;",
        )?;

        Ok(())
    }

    fn migrate_qbit_tables(&self, conn: &rusqlite::Connection) -> Result<()> {
        conn.execute_batch("CREATE TABLE IF NOT EXISTS qbit_instances (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            url TEXT NOT NULL,
            username TEXT NOT NULL DEFAULT '',
            password TEXT NOT NULL DEFAULT '',
            enabled INTEGER NOT NULL DEFAULT 1,
            default_category TEXT,
            save_path TEXT,
            poll_interval_secs INTEGER NOT NULL DEFAULT 2,
            last_rid INTEGER NOT NULL DEFAULT 0,
            sync_health TEXT NOT NULL DEFAULT 'disconnected' CHECK(sync_health IN ('healthy','stale','disconnected')),
            last_sync_at TEXT,
            app_version TEXT,
            webapi_version TEXT,
            last_seen_at TEXT,
            last_error TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS qbit_torrents (
            instance_id TEXT NOT NULL,
            hash TEXT NOT NULL,
            name TEXT NOT NULL,
            state TEXT NOT NULL DEFAULT '',
            progress REAL NOT NULL DEFAULT 0,
            size INTEGER NOT NULL DEFAULT 0,
            downloaded INTEGER NOT NULL DEFAULT 0,
            uploaded INTEGER NOT NULL DEFAULT 0,
            dlspeed INTEGER NOT NULL DEFAULT 0,
            upspeed INTEGER NOT NULL DEFAULT 0,
            eta INTEGER NOT NULL DEFAULT 0,
            ratio REAL NOT NULL DEFAULT 0,
            category TEXT NOT NULL DEFAULT '',
            tags TEXT NOT NULL DEFAULT '',
            save_path TEXT NOT NULL DEFAULT '',
            content_path TEXT NOT NULL DEFAULT '',
            tracker TEXT NOT NULL DEFAULT '',
            added_on INTEGER NOT NULL DEFAULT 0,
            completion_on INTEGER NOT NULL DEFAULT 0,
            amount_left INTEGER NOT NULL DEFAULT 0,
            availability REAL NOT NULL DEFAULT 0,
            num_seeds INTEGER NOT NULL DEFAULT 0,
            num_leechs INTEGER NOT NULL DEFAULT 0,
            seen_complete INTEGER NOT NULL DEFAULT 0,
            last_activity INTEGER NOT NULL DEFAULT 0,
            raw_json TEXT NOT NULL DEFAULT '{}',
            updated_at TEXT NOT NULL,
            PRIMARY KEY(instance_id, hash)
        );
        CREATE INDEX IF NOT EXISTS idx_qbit_torrents_instance_state ON qbit_torrents(instance_id, state);
        CREATE INDEX IF NOT EXISTS idx_qbit_torrents_instance_name ON qbit_torrents(instance_id, name);")?;
        for sql in [
            "ALTER TABLE qbit_instances ADD COLUMN sync_health TEXT NOT NULL DEFAULT 'disconnected' CHECK(sync_health IN ('healthy','stale','disconnected'))",
            "ALTER TABLE qbit_instances ADD COLUMN last_sync_at TEXT",
        ] {
            if let Err(error) = conn.execute(sql, []) {
                if !error.to_string().contains("duplicate column name") { return Err(error); }
            }
        }

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM qbit_instances", [], |r| r.get(0))?;
        if count == 0 {
            let enabled = conn
                .query_row(
                    "SELECT value FROM settings WHERE key='qbittorrent_enabled'",
                    [],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .and_then(|v| v.parse::<bool>().ok())
                .unwrap_or(false);
            let url = conn
                .query_row(
                    "SELECT value FROM settings WHERE key='qbittorrent_url'",
                    [],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .unwrap_or_else(|| "http://localhost:8080".to_string());
            let username = conn
                .query_row(
                    "SELECT value FROM settings WHERE key='qbittorrent_username'",
                    [],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .unwrap_or_else(|| "admin".to_string());
            let password = conn
                .query_row(
                    "SELECT value FROM settings WHERE key='qbittorrent_password'",
                    [],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .unwrap_or_default();
            let category = conn
                .query_row(
                    "SELECT value FROM settings WHERE key='qbittorrent_category'",
                    [],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .filter(|v| !v.trim().is_empty());
            let save_path = conn
                .query_row(
                    "SELECT value FROM settings WHERE key='qbittorrent_save_path'",
                    [],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .filter(|v| !v.trim().is_empty());
            let poll = conn
                .query_row(
                    "SELECT value FROM settings WHERE key='qbittorrent_poll_interval_secs'",
                    [],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5)
                .max(2);
            if enabled || !password.is_empty() || url != "http://localhost:8080" {
                conn.execute("INSERT INTO qbit_instances (id, name, url, username, password, enabled, default_category, save_path, poll_interval_secs, created_at, updated_at) VALUES ('default', 'qBittorrent', ?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'), datetime('now'))",
                    params![url, username, password, enabled as i32, category, save_path, poll as i64])?;
            }
        }
        Ok(())
    }

    fn migrate_auth_tables(&self, conn: &rusqlite::Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS app_users (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL UNIQUE COLLATE NOCASE,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS app_sessions (
                token_hash TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
                expires_at INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                last_seen_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_app_sessions_expiry ON app_sessions(expires_at);",
        )?;
        Ok(())
    }

    fn migrate_canonical_media_tables(&self, conn: &rusqlite::Connection) -> Result<()> {
        conn.execute_batch("CREATE TABLE IF NOT EXISTS media_items (
            media_key TEXT PRIMARY KEY, tmdb_id INTEGER, media_type TEXT NOT NULL, title TEXT NOT NULL,
            original_title TEXT, year INTEGER, overview TEXT, poster_path TEXT, backdrop_path TEXT, genres TEXT,
            runtime INTEGER, total_seasons INTEGER, arr_id INTEGER, arr_type TEXT, arr_path TEXT,
            arr_monitored INTEGER DEFAULT 0, arr_status TEXT, arr_quality_profile_id INTEGER,
            arr_has_file INTEGER DEFAULT 0, arr_size_on_disk INTEGER DEFAULT 0, tvdb_id INTEGER, imdb_id TEXT,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL, arr_synced_at TEXT);
            CREATE TABLE IF NOT EXISTS media_episodes (
            id INTEGER PRIMARY KEY AUTOINCREMENT, media_key TEXT NOT NULL REFERENCES media_items(media_key) ON DELETE CASCADE,
            tmdb_id INTEGER NOT NULL, season_number INTEGER NOT NULL, episode_number INTEGER NOT NULL, title TEXT,
            overview TEXT, air_date TEXT, arr_episode_id INTEGER, arr_has_file INTEGER DEFAULT 0,
            arr_monitored INTEGER DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
            UNIQUE(media_key, season_number, episode_number));")?;
        let pk: Option<String> = conn
            .query_row(
                "SELECT name FROM pragma_table_info('media_items') WHERE pk=1 LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if pk.as_deref() != Some("media_key") {
            conn.execute_batch("PRAGMA foreign_keys=off;
            ALTER TABLE media_items RENAME TO media_items_legacy;
            ALTER TABLE media_episodes RENAME TO media_episodes_legacy;
            CREATE TABLE media_items (media_key TEXT PRIMARY KEY, tmdb_id INTEGER, media_type TEXT NOT NULL, title TEXT NOT NULL,
            original_title TEXT, year INTEGER, overview TEXT, poster_path TEXT, backdrop_path TEXT, genres TEXT,
            runtime INTEGER, total_seasons INTEGER, arr_id INTEGER, arr_type TEXT, arr_path TEXT,
            arr_monitored INTEGER DEFAULT 0, arr_status TEXT, arr_quality_profile_id INTEGER,
            arr_has_file INTEGER DEFAULT 0, arr_size_on_disk INTEGER DEFAULT 0, tvdb_id INTEGER, imdb_id TEXT,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL, arr_synced_at TEXT);
            INSERT OR REPLACE INTO media_items SELECT CASE WHEN media_type='tv' THEN CASE WHEN tvdb_id IS NOT NULL THEN 'tv:'||tvdb_id ELSE 'tv-tmdb:'||tmdb_id END ELSE 'movie:'||tmdb_id END,
            tmdb_id, media_type, title, original_title, year, overview, poster_path, backdrop_path, genres, runtime, total_seasons,
            arr_id, arr_type, arr_path, arr_monitored, arr_status, arr_quality_profile_id, arr_has_file, arr_size_on_disk, tvdb_id, imdb_id, created_at, updated_at, arr_synced_at FROM media_items_legacy;
            CREATE TABLE media_episodes (id INTEGER PRIMARY KEY AUTOINCREMENT, media_key TEXT NOT NULL REFERENCES media_items(media_key) ON DELETE CASCADE,
            tmdb_id INTEGER NOT NULL, season_number INTEGER NOT NULL, episode_number INTEGER NOT NULL, title TEXT,
            overview TEXT, air_date TEXT, arr_episode_id INTEGER, arr_has_file INTEGER DEFAULT 0,
            arr_monitored INTEGER DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
            UNIQUE(media_key, season_number, episode_number));
            INSERT OR REPLACE INTO media_episodes SELECT e.id, m.media_key, e.tmdb_id, e.season_number, e.episode_number, e.title, e.overview, e.air_date, e.arr_episode_id, e.arr_has_file, e.arr_monitored, e.created_at, e.updated_at FROM media_episodes_legacy e JOIN media_items m ON m.tmdb_id=e.tmdb_id;
            DROP TABLE media_episodes_legacy; DROP TABLE media_items_legacy; PRAGMA foreign_keys=on;")?;
        }
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_media_key ON downloads(media_key)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_media_items_tmdb_id ON media_items(tmdb_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_media_items_tvdb_id ON media_items(tvdb_id)",
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
            "CREATE INDEX IF NOT EXISTS idx_media_episodes_media_key ON media_episodes(media_key)",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_media_episodes_lookup ON media_episodes(media_key, season_number, episode_number)", [])?;
        Ok(())
    }

    fn upsert_task_in_tx(
        tx: &rusqlite::Transaction<'_>,
        task: &DownloadTask,
        leaf: Option<&str>,
        superseded_by: Option<&str>,
        is_current: i64,
    ) -> Result<()> {
        if let Err(e) = tx.execute(
            "INSERT INTO downloads (
                id, url, original_url, filename, destination, state, progress, size,
                downloaded, speed, eta, host, category, priority, segments, retry_count,
                created_at, started_at, completed_at, wait_until, error_message,
                batch_id, batch_name, tmdb_id, media_type, media_item_id, tmdb_title, tmdb_season, tmdb_episode, info_path,
                arr_announced, arr_series_id, arr_movie_id, arr_announce_error, fshare_code,
                quality, resolution, tvdb_id, download_source, external_provider, external_id, external_instance_id, info_hash, origin_provider, origin_indexer, seeders, leechers,
                provider_id, provider_item_id, file_idx, source_filename, selected_episodes, execution_strategy, torbox_remote_state, torbox_remote_completed_at, selected_remote_file_ids, selected_artifacts, media_leaf_key, superseded_by, is_current
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44, ?45, ?46, ?47, ?48, ?49, ?50, ?51, ?52, ?53, ?54, ?55, ?56, ?57, ?58, ?59, ?60)
            ON CONFLICT(id) DO UPDATE SET
                url=excluded.url, original_url=excluded.original_url, filename=excluded.filename,
                destination=excluded.destination, state=excluded.state, progress=excluded.progress,
                size=excluded.size, downloaded=excluded.downloaded, speed=excluded.speed, eta=excluded.eta,
                host=excluded.host, category=excluded.category, priority=excluded.priority,
                segments=excluded.segments, retry_count=excluded.retry_count, created_at=excluded.created_at,
                started_at=excluded.started_at, completed_at=excluded.completed_at, wait_until=excluded.wait_until,
                error_message=excluded.error_message, batch_id=excluded.batch_id, batch_name=excluded.batch_name,
                tmdb_id=excluded.tmdb_id, media_type=excluded.media_type, media_item_id=excluded.media_item_id,
                tmdb_title=excluded.tmdb_title, tmdb_season=excluded.tmdb_season, tmdb_episode=excluded.tmdb_episode,
                info_path=excluded.info_path, arr_announced=excluded.arr_announced,
                arr_series_id=excluded.arr_series_id, arr_movie_id=excluded.arr_movie_id,
                arr_announce_error=excluded.arr_announce_error, fshare_code=excluded.fshare_code,
                quality=excluded.quality, resolution=excluded.resolution, tvdb_id=excluded.tvdb_id,
                download_source=excluded.download_source, external_provider=excluded.external_provider,
                external_id=excluded.external_id, external_instance_id=excluded.external_instance_id,
                info_hash=excluded.info_hash, origin_provider=excluded.origin_provider,
                origin_indexer=excluded.origin_indexer, seeders=excluded.seeders, leechers=excluded.leechers,
                provider_id=excluded.provider_id, provider_item_id=excluded.provider_item_id,
                file_idx=excluded.file_idx, source_filename=excluded.source_filename,
                selected_episodes=excluded.selected_episodes, execution_strategy=excluded.execution_strategy,
                torbox_remote_state=excluded.torbox_remote_state,
                torbox_remote_completed_at=excluded.torbox_remote_completed_at,
                selected_remote_file_ids=excluded.selected_remote_file_ids,
                selected_artifacts=excluded.selected_artifacts, media_leaf_key=excluded.media_leaf_key",
            params![
                task.id.to_string(),
                if task.execution_strategy.as_deref() == Some("torbox_http") { task.external_id.as_deref().unwrap_or("") } else { &task.url },
                if task.execution_strategy.as_deref() == Some("torbox_http") { task.external_id.as_deref().unwrap_or("") } else { &task.original_url },
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
                task.media_type.as_deref().unwrap_or("other"),
                task.media_item_id,
                task.tmdb_title,
                task.tmdb_season.map(|s| s as i64),
                task.tmdb_episode.map(|e| e as i64),
                task.info_path,
                task.arr_announced,
                task.arr_series_id,
                task.arr_movie_id,
                task.arr_announce_error,
                Option::<String>::None,
                &task.quality,
                &task.resolution,
                task.tvdb_id,
                &task.download_source,
                &task.external_provider,
                &task.external_id,
                &task.external_instance_id,
                &task.info_hash,
                &task.origin_provider,
                &task.origin_indexer,
                task.seeders,
                task.leechers,
                &task.provider_id,
                &task.provider_item_id,
                task.file_idx.map(|i| i as i64),
                &task.source_filename,
                &task.selected_episodes,
                &task.execution_strategy,
                &task.torbox_remote_state,
                &task.torbox_remote_completed_at,
                &task.selected_remote_file_ids,
                &task.selected_artifacts,
                leaf,
                superseded_by,
                is_current,
            ],
        ) {
            tracing::error!("Failed to save task {} to DB: {}", task.id, e);
            return Err(e.into());
        }
        Ok(())
    }

    pub fn save_task(&self, task: &DownloadTask) -> Result<()> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let id = task.id.to_string();
        let leaf = task.canonical_media_leaf_key();
        let ownership = tx
            .query_row(
                "SELECT superseded_by,is_current FROM downloads WHERE id=?1",
                [&id],
                |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        let (superseded_by, is_current) = match ownership {
            Some(value) => value,
            None => {
                let occupied = match leaf.as_deref() {
                    Some(leaf) => tx.query_row("SELECT EXISTS(SELECT 1 FROM downloads WHERE media_leaf_key=?1 AND is_current=1)", [leaf], |row| row.get::<_, i64>(0))? != 0,
                    None => false,
                };
                (None, i64::from(!occupied))
            }
        };
        Self::upsert_task_in_tx(
            &tx,
            task,
            leaf.as_deref(),
            superseded_by.as_deref(),
            is_current,
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn save_task_replacing_current(&self, task: &DownloadTask) -> Result<Option<DownloadTask>> {
        let leaf = task.canonical_media_leaf_key();
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let old = match leaf.as_deref() {
            Some(leaf) => tx.query_row(&format!("SELECT {TASK_SELECT_COLUMNS} FROM downloads WHERE media_leaf_key=?1 AND is_current=1"), [leaf], Self::parse_task_from_row).optional()?,
            None => None,
        };
        let id = task.id.to_string();
        if let Some(leaf) = leaf.as_deref() {
            tx.execute("UPDATE downloads SET is_current=0,superseded_by=?1 WHERE media_leaf_key=?2 AND is_current=1 AND id<>?1", params![id, leaf])?;
        }
        Self::upsert_task_in_tx(&tx, task, leaf.as_deref(), None, 1)?;
        tx.execute(
            "UPDATE downloads SET is_current=1,superseded_by=NULL WHERE id=?1",
            [&id],
        )?;
        tx.commit()?;
        Ok(old)
    }

    pub fn get_current_tasks_for_leaf(&self, leaf: &str) -> Result<Vec<DownloadTask>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(&format!("SELECT {TASK_SELECT_COLUMNS} FROM downloads WHERE media_leaf_key = ?1 AND is_current = 1"))?;
        let rows = stmt.query_map(params![leaf], Self::parse_task_from_row)?;
        rows.collect()
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
            "IMPORTING" => DownloadState::Importing,
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
            tvdb_id: row.get::<_, Option<i64>>(37).ok().flatten(),
            tmdb_id: row.get::<_, Option<i64>>(23).ok().flatten(),
            media_type: row.get(24).ok().flatten(),
            media_item_id: row.get::<_, Option<i64>>(25).ok().flatten(),
            tmdb_title: row.get(26).ok().flatten(),
            tmdb_season: row
                .get::<_, Option<i64>>(27)
                .ok()
                .flatten()
                .map(|s| s as u32),
            tmdb_episode: row
                .get::<_, Option<i64>>(28)
                .ok()
                .flatten()
                .map(|e| e as u32),
            info_path: row.get::<_, Option<String>>(29).ok().flatten(),
            arr_announced: row
                .get::<_, Option<bool>>(30)
                .ok()
                .flatten()
                .unwrap_or(false),
            arr_series_id: row.get::<_, Option<i64>>(31).ok().flatten(),
            arr_movie_id: row.get::<_, Option<i64>>(32).ok().flatten(),
            arr_announce_error: row.get::<_, Option<String>>(33).ok().flatten(),
            fshare_code: row.get::<_, Option<String>>(34).ok().flatten(),
            provider_id: row.get::<_, Option<String>>(47).ok().flatten(),
            provider_item_id: row.get::<_, Option<String>>(48).ok().flatten(),
            file_idx: row
                .get::<_, Option<i64>>(49)
                .ok()
                .flatten()
                .map(|i| i as usize),
            source_filename: row.get::<_, Option<String>>(50).ok().flatten(),
            selected_episodes: row.get::<_, Option<String>>(51).ok().flatten(),
            execution_strategy: row
                .get::<_, Option<String>>(52)
                .ok()
                .flatten()
                .or_else(|| Some("local_qb".to_string())),
            torbox_remote_state: row
                .get::<_, Option<String>>(53)
                .ok()
                .flatten()
                .or_else(|| Some("unknown".to_string())),
            torbox_remote_completed_at: row.get::<_, Option<String>>(54).ok().flatten(),
            selected_remote_file_ids: row
                .get::<_, Option<String>>(55)
                .ok()
                .flatten()
                .or_else(|| Some("[]".to_string())),
            selected_artifacts: row
                .get::<_, Option<String>>(56)
                .ok()
                .flatten()
                .or_else(|| Some("[]".to_string())),
            quality: row.get::<_, Option<String>>(35).ok().flatten(),
            resolution: row.get::<_, Option<String>>(36).ok().flatten(),
            download_source: row.get::<_, Option<String>>(38).ok().flatten(),
            external_provider: row.get::<_, Option<String>>(39).ok().flatten(),
            external_id: row.get::<_, Option<String>>(40).ok().flatten(),
            external_instance_id: row.get::<_, Option<String>>(41).ok().flatten(),
            info_hash: row.get::<_, Option<String>>(42).ok().flatten(),
            origin_provider: row.get::<_, Option<String>>(43).ok().flatten(),
            origin_indexer: row.get::<_, Option<String>>(44).ok().flatten(),
            seeders: row.get::<_, Option<i64>>(45).ok().flatten(),
            leechers: row.get::<_, Option<i64>>(46).ok().flatten(),
            task_state: None,
            qbit_state: None,
            sync_health: None,
            last_sync_at: None,
            transfer_state: None,
            transfer_active: false,
            url_metadata: None,
            error_history: Vec::new(),
            state_obj: crate::downloader::state_machine::TaskStateFactory::get_state(state),
            cancel_token: CancellationToken::new(),
            pause_notify: Arc::new(Notify::new()),
        })
    }

    pub fn get_all_tasks(&self) -> Result<Vec<DownloadTask>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size,
             downloaded, speed, eta, host, category, priority, segments, retry_count,
             created_at, started_at, completed_at, wait_until, error_message,
             batch_id, batch_name, tmdb_id, media_type, media_item_id, tmdb_title, tmdb_season, tmdb_episode, info_path,
             arr_announced, arr_series_id, arr_movie_id, arr_announce_error, fshare_code,
             quality, resolution, tvdb_id, download_source, external_provider, external_id, external_instance_id, info_hash, origin_provider, origin_indexer, seeders, leechers, provider_id, provider_item_id, file_idx, source_filename, selected_episodes, execution_strategy, torbox_remote_state, torbox_remote_completed_at, selected_remote_file_ids, selected_artifacts
             FROM downloads WHERE is_current = 1",
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
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size,
             downloaded, speed, eta, host, category, priority, segments, retry_count,
             created_at, started_at, completed_at, wait_until, error_message,
             batch_id, batch_name, tmdb_id, media_type, media_item_id, tmdb_title, tmdb_season, tmdb_episode, info_path,
             arr_announced, arr_series_id, arr_movie_id, arr_announce_error, fshare_code,
             quality, resolution, tvdb_id, download_source, external_provider, external_id, external_instance_id, info_hash, origin_provider, origin_indexer, seeders, leechers, provider_id, provider_item_id, file_idx, source_filename, selected_episodes, execution_strategy, torbox_remote_state, torbox_remote_completed_at, selected_remote_file_ids, selected_artifacts
             FROM downloads WHERE id = ?1",
        )?;
        let mut task_iter = stmt.query_map(params![id.to_string()], |row| {
            Self::parse_task_from_row(row)
        })?;

        match task_iter.next() {
            Some(task) => Ok(Some(task?)),
            None => Ok(None),
        }
    }

    /// Get tasks with pagination, sorted by created_at DESC
    /// Returns (tasks, total_count)
    pub fn get_tasks_paginated(&self, page: u32, limit: u32) -> Result<(Vec<DownloadTask>, u64)> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        // Get total count
        let total: u64 = conn.query_row(
            "SELECT COUNT(*) FROM downloads WHERE is_current = 1",
            [],
            |row| row.get(0),
        )?;

        // Calculate offset
        let offset = (page.saturating_sub(1)) * limit;

        // Get paginated tasks (active states first, then by created_at DESC)
        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size,
             downloaded, speed, eta, host, category, priority, segments, retry_count,
             created_at, started_at, completed_at, wait_until, error_message,
             batch_id, batch_name, tmdb_id, media_type, media_item_id, tmdb_title, tmdb_season, tmdb_episode, info_path,
             arr_announced, arr_series_id, arr_movie_id, arr_announce_error, fshare_code,
             quality, resolution, tvdb_id, download_source, external_provider, external_id, external_instance_id, info_hash, origin_provider, origin_indexer, seeders, leechers, provider_id, provider_item_id, file_idx, source_filename, selected_episodes, execution_strategy, torbox_remote_state, torbox_remote_completed_at, selected_remote_file_ids, selected_artifacts
             FROM downloads WHERE is_current = 1
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
             LIMIT ?1 OFFSET ?2",
        )?;

        let task_iter = stmt.query_map(params![limit as i64, offset as i64], |row| {
            Self::parse_task_from_row(row)
        })?;

        let mut tasks = Vec::new();
        for task in task_iter {
            if let Ok(t) = task {
                tasks.push(t);
            }
        }

        Ok((tasks, total))
    }

    /// Get recent download history for a host
    pub async fn get_history_async(&self, host: &str) -> Result<Vec<DownloadTask>> {
        let db = self.pool.clone();
        let host = host.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .get()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let mut stmt = conn.prepare(
                "SELECT id, url, original_url, filename, destination, state, progress, size,
                 downloaded, speed, eta, host, category, priority, segments, retry_count,
                 created_at, started_at, completed_at, wait_until, error_message,
                 batch_id, batch_name, tmdb_id, media_type, media_item_id, tmdb_title, tmdb_season, tmdb_episode, info_path,
                 arr_announced, arr_series_id, arr_movie_id, arr_announce_error, fshare_code,
                 quality, resolution, tvdb_id, download_source, external_provider, external_id, external_instance_id, info_hash, origin_provider, origin_indexer, seeders, leechers, provider_id, provider_item_id, file_idx, source_filename, selected_episodes, execution_strategy, torbox_remote_state, torbox_remote_completed_at, selected_remote_file_ids, selected_artifacts
                 FROM downloads
                 WHERE host = ?1
                 ORDER BY created_at DESC
                 LIMIT 100",
            )?;
            let task_iter = stmt.query_map(params![host], |row| Db::parse_task_from_row(row))?;
            let mut tasks = Vec::new();
            for task in task_iter {
                if let Ok(t) = task {
                    tasks.push(t);
                }
            }
            Ok(tasks)
        })
        .await
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
    }

    /// Get the next queued task for workers to process (FIFO with priority)
    pub fn get_next_queued_task(&self) -> Result<Option<DownloadTask>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size,
             downloaded, speed, eta, host, category, priority, segments, retry_count,
             created_at, started_at, completed_at, wait_until, error_message,
             batch_id, batch_name, tmdb_id, media_type, media_item_id, tmdb_title, tmdb_season, tmdb_episode, info_path,
             arr_announced, arr_series_id, arr_movie_id, arr_announce_error, fshare_code,
             quality, resolution, tvdb_id, download_source, external_provider, external_id, external_instance_id, info_hash, origin_provider, origin_indexer, seeders, leechers, provider_id, provider_item_id, file_idx, source_filename, selected_episodes, execution_strategy, torbox_remote_state, torbox_remote_completed_at, selected_remote_file_ids, selected_artifacts
             FROM downloads
             WHERE state = 'QUEUED' AND is_current = 1
             ORDER BY priority DESC, created_at ASC
             LIMIT 1",
        )?;

        let mut task_iter = stmt.query_map([], |row| Self::parse_task_from_row(row))?;

        match task_iter.next() {
            Some(task) => Ok(Some(task?)),
            None => Ok(None),
        }
    }

    /// Get tasks waiting for retry that are past their wait_until time
    pub fn get_ready_waiting_tasks(&self) -> Result<Vec<DownloadTask>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let now = chrono::Utc::now().to_rfc3339();

        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size,
             downloaded, speed, eta, host, category, priority, segments, retry_count,
             created_at, started_at, completed_at, wait_until, error_message,
             batch_id, batch_name, tmdb_id, media_type, media_item_id, tmdb_title, tmdb_season, tmdb_episode, info_path,
             arr_announced, arr_series_id, arr_movie_id, arr_announce_error, fshare_code,
             quality, resolution, tvdb_id, download_source, external_provider, external_id, external_instance_id, info_hash, origin_provider, origin_indexer, seeders, leechers, provider_id, provider_item_id, file_idx, source_filename, selected_episodes, execution_strategy, torbox_remote_state, torbox_remote_completed_at, selected_remote_file_ids, selected_artifacts
             FROM downloads
             WHERE state = 'WAITING' AND is_current = 1 AND (wait_until IS NULL OR wait_until <= ?1)
             ORDER BY created_at ASC",
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
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "UPDATE downloads SET state = ?1 WHERE id = ?2",
            params![state, id.to_string()],
        )?;
        Ok(())
    }

    /// Update only progress-related fields (narrow update to avoid data races)
    /// This should be used during active downloads instead of save_task
    pub fn update_task_progress(
        &self,
        id: Uuid,
        downloaded: u64,
        speed: f64,
        eta: f64,
        progress: f32,
    ) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "UPDATE downloads SET downloaded = ?1, speed = ?2, eta = ?3, progress = ?4 WHERE id = ?5",
            params![downloaded as i64, speed, eta as i64, progress, id.to_string()],
        )?;
        Ok(())
    }

    /// Async version of update_task_progress
    pub async fn update_task_progress_async(
        &self,
        id: Uuid,
        downloaded: u64,
        speed: f64,
        eta: f64,
        progress: f32,
    ) -> Result<()> {
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
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM downloads WHERE state = ?1 AND is_current = 1",
            params![state],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn delete_task(&self, id: Uuid) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "DELETE FROM downloads WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }

    pub fn record_activity_event(&self, kind: &str, task_id: Uuid, message: &str) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR IGNORE INTO activity_events (created_at, kind, task_id, message)
             VALUES (?1, ?2, ?3, ?4)",
            params![Utc::now().to_rfc3339(), kind, task_id.to_string(), message],
        )?;
        Ok(())
    }

    pub async fn get_activity_events_async(&self, limit: usize) -> Result<Vec<ActivityEvent>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let mut statement = conn.prepare(
                "SELECT id, created_at, kind, task_id, message
                 FROM activity_events ORDER BY created_at DESC, id DESC LIMIT ?1",
            )?;
            let rows = statement.query_map(params![limit as i64], |row| {
                Ok(ActivityEvent {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    kind: row.get(2)?,
                    task_id: row.get(3)?,
                    message: row.get(4)?,
                })
            })?;
            rows.collect()
        })
        .await
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
    }

    // ============================================================================
    // Async Wrappers (prevent async runtime blocking)
    // ============================================================================

    /// Async version of get_tasks_paginated - uses spawn_blocking
    pub async fn get_tasks_paginated_async(
        &self,
        page: u32,
        limit: u32,
    ) -> Result<(Vec<DownloadTask>, u64)> {
        self.get_tasks_paginated_sorted_async(page, limit, "added", "desc")
            .await
    }

    /// Async version with sorting - uses spawn_blocking
    /// sort_by: "added", "status", "filename", "size", "progress", "source"
    /// sort_dir: "asc" or "desc"
    pub async fn get_tasks_paginated_sorted_async(
        &self,
        page: u32,
        limit: u32,
        sort_by: &str,
        sort_dir: &str,
    ) -> Result<(Vec<DownloadTask>, u64)> {
        self.get_tasks_paginated_sorted_filtered_async(page, limit, sort_by, sort_dir, None, None)
            .await
    }

    /// Async version with sorting and optional filters - uses spawn_blocking
    /// sort_by: "added", "status", "filename", "size", "progress"
    /// sort_dir: "asc" or "desc"
    /// status_filter: Optional status to filter by (e.g., "DOWNLOADING", "QUEUED", "COMPLETED", "FAILED")
    pub async fn get_tasks_paginated_sorted_filtered_async(
        &self,
        page: u32,
        limit: u32,
        sort_by: &str,
        sort_dir: &str,
        status_filter: Option<&str>,
        source_filter: Option<&str>,
    ) -> Result<(Vec<DownloadTask>, u64)> {
        let pool = self.pool.clone();
        let sort_by = sort_by.to_string();
        let sort_dir = sort_dir.to_string();
        let status_filter = status_filter.map(|s| s.to_uppercase());
        let source_filter = source_filter
            .map(|s| s.to_ascii_lowercase())
            .filter(|s| matches!(s.as_str(), "fshare" | "torrent" | "torbox" | "direct"));

        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            let mut where_parts = vec!["is_current = 1".to_string()];
            if let Some(status) = &status_filter {
                where_parts.push(format!("state = '{}'", status));
            }
            if let Some(source_clause) = row_source_filter_clause(source_filter.as_deref()) {
                where_parts.push(source_clause.to_string());
            }
            let where_clause = if where_parts.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", where_parts.join(" AND "))
            };

            // Get total count (with filters if specified)
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
                "source" => format!("COALESCE(download_source, external_provider, host) {}", if sort_dir == "asc" { "ASC" } else { "DESC" }),
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
                 tmdb_id, media_type, media_item_id, tmdb_title, tmdb_season, tmdb_episode, info_path, arr_announced, arr_series_id, arr_movie_id, arr_announce_error, fshare_code, quality, resolution, tvdb_id, download_source, external_provider, external_id, external_instance_id, info_hash, origin_provider, origin_indexer, seeders, leechers, provider_id, provider_item_id, file_idx, source_filename, selected_episodes, execution_strategy, torbox_remote_state, torbox_remote_completed_at, selected_remote_file_ids, selected_artifacts
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
                    let mut batch_filters = vec!["is_current = 1".to_string()];
                    if let Some(status) = &status_filter {
                        batch_filters.push(format!("state = '{}'", status));
                    }
                    if let Some(source_clause) = row_source_filter_clause(source_filter.as_deref()) {
                        batch_filters.push(source_clause.to_string());
                    }
                    let batch_query = format!(
                        "SELECT id, url, original_url, filename, destination, state, progress, size,
                         downloaded, speed, eta, host, category, priority, segments, retry_count,
                         created_at, started_at, completed_at, wait_until, error_message,
                         batch_id, batch_name, tmdb_id, media_type, media_item_id, tmdb_title, tmdb_season, tmdb_episode, info_path,
                         arr_announced, arr_series_id, arr_movie_id, arr_announce_error, fshare_code,
                         quality, resolution, tvdb_id, download_source, external_provider, external_id, external_instance_id, info_hash, origin_provider, origin_indexer, seeders, leechers, provider_id, provider_item_id, file_idx, source_filename, selected_episodes, execution_strategy, torbox_remote_state, torbox_remote_completed_at, selected_remote_file_ids, selected_artifacts
                         FROM downloads
                         WHERE batch_id = ?1{}",
                        if batch_filters.is_empty() {
                            String::new()
                        } else {
                            format!(" AND {}", batch_filters.join(" AND "))
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
        })
        .await
        .unwrap()
    }

    /// Async version of update_task_state
    pub async fn update_task_state_async(&self, id: Uuid, state: String) -> Result<()> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            conn.execute(
                "UPDATE downloads SET state = ?1 WHERE id = ?2",
                params![state, id.to_string()],
            )?;
            Ok(())
        })
        .await
        .unwrap()
    }

    /// Batch update task states atomically in a single transaction
    /// Used by pause_all, resume_all for atomic operations
    pub async fn batch_update_states_async(
        &self,
        task_ids: Vec<Uuid>,
        new_state: String,
    ) -> Result<usize> {
        if task_ids.is_empty() {
            return Ok(0);
        }

        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool
                .get()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
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
        })
        .await
        .unwrap()
    }

    /// Get all tasks matching any of the provided states (async)
    /// Used for bulk operations like pause_all, resume_all
    pub async fn get_tasks_by_states_async(
        &self,
        states: Vec<String>,
    ) -> Result<Vec<DownloadTask>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            if states.is_empty() {
                return Ok(Vec::new());
            }

            // Build IN clause with placeholders
            let placeholders: Vec<String> = states
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 1))
                .collect();
            let in_clause = placeholders.join(", ");

            let sql = format!(
                "SELECT id, url, original_url, filename, destination, state, progress, size,
                        downloaded, speed, eta, host, category, priority, segments, retry_count,
                        created_at, started_at, completed_at, wait_until, error_message,
                        batch_id, batch_name, tmdb_id, media_type, media_item_id, tmdb_title, tmdb_season, tmdb_episode, info_path,
                        arr_announced, arr_series_id, arr_movie_id, arr_announce_error, fshare_code,
                        quality, resolution, tvdb_id, download_source, external_provider, external_id, external_instance_id, info_hash, origin_provider, origin_indexer, seeders, leechers, provider_id, provider_item_id, file_idx, source_filename, selected_episodes, execution_strategy, torbox_remote_state, torbox_remote_completed_at, selected_remote_file_ids, selected_artifacts
                 FROM downloads
                 WHERE state IN ({}) AND is_current = 1
                 ORDER BY priority DESC, created_at ASC",
                in_clause
            );

            let mut stmt = conn.prepare(&sql)?;

            // Convert states to params
            let params: Vec<&dyn rusqlite::ToSql> =
                states.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

            let task_iter = stmt.query_map(params.as_slice(), |row| {
                Self::parse_task_from_row_static(row)
            })?;

            let mut tasks = Vec::new();
            for task_result in task_iter {
                tasks.push(task_result?);
            }
            Ok(tasks)
        })
        .await
        .unwrap()
    }

    /// Get counts of downloads by status from database
    pub async fn get_status_counts_async(
        &self,
    ) -> Result<std::collections::HashMap<String, usize>> {
        self.get_status_counts_filtered_async(None).await
    }

    pub async fn get_status_counts_filtered_async(
        &self,
        source_filter: Option<&str>,
    ) -> Result<std::collections::HashMap<String, usize>> {
        let pool = self.pool.clone();
        let source_filter = source_filter
            .map(|s| s.to_ascii_lowercase())
            .filter(|s| matches!(s.as_str(), "fshare" | "torrent" | "torbox" | "direct"));
        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let where_clause = row_source_filter_clause(source_filter.as_deref())
                .map(|clause| format!(" WHERE is_current = 1 AND {}", clause))
                .unwrap_or_else(|| " WHERE is_current = 1".to_string());
            let mut stmt = conn.prepare(&format!(
                "SELECT state, COUNT(*) as count FROM downloads{} GROUP BY state",
                where_clause
            ))?;

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
        })
        .await
        .unwrap()
    }

    /// Get counts of downloads by status from database (synchronous version)
    pub fn get_status_counts(&self) -> Result<std::collections::HashMap<String, usize>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT state, COUNT(*) as count FROM downloads WHERE is_current = 1 GROUP BY state",
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
    pub async fn get_tasks_by_batch_id_async(
        &self,
        batch_id: String,
        source_filter: Option<&str>,
    ) -> Result<Vec<DownloadTask>> {
        let pool = self.pool.clone();
        let source_filter = source_filter
            .map(|s| s.to_ascii_lowercase())
            .filter(|s| matches!(s.as_str(), "fshare" | "torrent" | "torbox" | "direct"));
        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let source_clause = row_source_filter_clause(source_filter.as_deref())
                .map(|clause| format!(" AND {}", clause))
                .unwrap_or_default();
            let sql = format!(
                "SELECT {} FROM downloads WHERE batch_id = ?1 AND is_current = 1{} ORDER BY created_at ASC",
                TASK_SELECT_COLUMNS, source_clause
            );
            debug_assert_eq!(TASK_SELECT_COLUMNS.split(',').count(), 57);
            let mut stmt = conn.prepare(&sql)?;

            let task_iter = stmt.query_map(params![&batch_id], |row| {
                Self::parse_task_from_row_static(row)
            })?;

            let mut tasks = Vec::new();
            for task_result in task_iter {
                tasks.push(task_result?);
            }
            Ok(tasks)
        })
        .await
        .unwrap()
    }

    /// Delete all tasks with a specific batch_id (async)
    pub async fn delete_tasks_by_batch_id_async(&self, batch_id: String) -> Result<usize> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let affected = conn.execute(
                "DELETE FROM downloads WHERE batch_id = ?1",
                params![&batch_id],
            )?;
            Ok(affected)
        })
        .await
        .unwrap()
    }

    pub async fn find_task_by_provider_item_async(
        &self,
        provider_id: &str,
        provider_item_id: &str,
    ) -> Result<Option<(String, String)>> {
        let pool = self.pool.clone();
        let provider_id = provider_id.to_string();
        let provider_item_id = provider_item_id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            conn.query_row(
                "SELECT id, state FROM downloads
                 WHERE provider_id = ?1 AND provider_item_id = ?2 AND is_current = 1 LIMIT 1",
                params![provider_id, provider_item_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
        })
        .await
        .unwrap()
    }

    /// Find an existing task by external provider/id (for torrent duplicate detection)
    pub async fn find_task_by_external_async(
        &self,
        provider: &str,
        external_id: &str,
    ) -> Result<Option<(String, String)>> {
        let pool = self.pool.clone();
        let provider = provider.to_string();
        let external_id = external_id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let result: Option<(String, String)> = conn
                .query_row(
                    "SELECT id, state FROM downloads WHERE external_provider = ?1 AND external_id = ?2 AND is_current = 1 LIMIT 1",
                    params![&provider, &external_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            Ok(result)
        })
        .await
        .unwrap()
    }

    /// Get batch_id for an existing batch by batch_name (async)
    /// Returns the batch_id if a batch with this name exists, None otherwise
    pub async fn get_batch_id_by_name_async(&self, batch_name: &str) -> Result<Option<String>> {
        let pool = self.pool.clone();
        let batch_name = batch_name.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let result: Option<String> = conn
                .query_row(
                    "SELECT batch_id FROM downloads WHERE batch_name = ?1 AND is_current = 1 LIMIT 1",
                    params![&batch_name],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(result)
        })
        .await
        .unwrap()
    }

    /// Get batch_id for an existing batch by tmdb_id (async)
    /// Returns the batch_id if an item with this tmdb_id exists in a batch, None otherwise
    pub async fn get_batch_id_by_tmdb_id_async(&self, tmdb_id: i64) -> Result<Option<String>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let result: Option<String> = conn.query_row(
                "SELECT batch_id FROM downloads WHERE tmdb_id = ?1 AND batch_id IS NOT NULL AND is_current = 1
                 ORDER BY (batch_id = printf('media:tv-tmdb:%d', ?1)) DESC, created_at ASC, batch_id ASC LIMIT 1",
                params![tmdb_id],
                |row| row.get(0)
            ).optional()?;
            Ok(result)
        }).await.unwrap()
    }

    /// Get batch_id for an existing TV batch by canonical tvdb_id (async)
    pub async fn get_batch_id_by_tvdb_id_async(&self, tvdb_id: i64) -> Result<Option<String>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let result: Option<String> = conn.query_row(
                "SELECT batch_id FROM downloads WHERE tvdb_id = ?1 AND batch_id IS NOT NULL AND is_current = 1
                 ORDER BY (batch_id = printf('media:tv:%d', ?1)) DESC, created_at ASC, batch_id ASC LIMIT 1",
                params![tvdb_id],
                |row| row.get(0)
            ).optional()?;
            Ok(result)
        }).await.unwrap()
    }

    /// Static version of parse_task_from_row for use in spawn_blocking
    fn parse_task_from_row_static(row: &rusqlite::Row) -> rusqlite::Result<DownloadTask> {
        Self::parse_task_from_row(row)
    }

    /// Get batch summaries with pagination for batch-first display.
    /// Returns page-scoped batches, standalone downloads, display row refs, orphan torrents, and unified total.
    pub async fn get_batch_summaries_paginated_async(
        &self,
        page: u32,
        limit: u32,
        status_filter: Option<&str>,
        source_filter: Option<&str>,
        sort_by: &str,
        sort_dir: &str,
    ) -> Result<(
        Vec<crate::api::downloads::BatchSummary>,
        Vec<DownloadTask>,
        Vec<crate::api::downloads::DisplayRowRef>,
        Vec<QbitTorrentRow>,
        u64,
    )> {
        let pool = self.pool.clone();
        let status_filter = status_filter.map(|s| s.to_uppercase()).filter(|s| {
            matches!(
                s.as_str(),
                "QUEUED"
                    | "STARTING"
                    | "DOWNLOADING"
                    | "PAUSED"
                    | "WAITING"
                    | "COMPLETED"
                    | "FAILED"
                    | "CANCELLED"
                    | "EXTRACTING"
                    | "SKIPPED"
                    | "IMPORTING"
            )
        });
        let source_filter = source_filter
            .map(|s| s.to_ascii_lowercase())
            .filter(|s| matches!(s.as_str(), "fshare" | "torrent" | "torbox" | "direct"));
        let sort_by = sort_by.to_string();
        let sort_dir = sort_dir.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            // Build WHERE clause for status filter
            let _where_clause = match &status_filter {
                Some(status) => format!("WHERE state = '{}'", status),
                None => String::new(),
            };
            // AND variant for queries that already have a WHERE clause (e.g. standalone query)
            let and_clause = match &status_filter {
                Some(status) => format!("AND state = '{}'", status),
                None => String::new(),
            };
            let source_matches = |source: &str| -> bool {
                let normalized = match source.to_ascii_lowercase().as_str() {
                    "qbittorrent" => "torrent".to_string(),
                    "flasharr-http" => "fshare".to_string(),
                    other => other.to_string(),
                };
                source_filter.as_ref().map_or(true, |wanted| {
                    if wanted == "direct" {
                        normalized == "fshare" || normalized == "torbox"
                    } else {
                        &normalized == wanted
                    }
                })
            };
            // ponytail: keep source normalization local; extract shared enum if more source modes arrive.

            let direction = if sort_dir == "asc" { "ASC" } else { "DESC" };
            let batch_order_by = match sort_by.as_str() {
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
                    direction
                ),
                "filename" => format!("batch_name {}", direction),
                "size" => format!("total_size {}", direction),
                "progress" => format!("progress {}", direction),
                _ => format!("created_at {}", direction),
            };
            let standalone_order_by = match sort_by.as_str() {
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
                    direction
                ),
                "filename" => format!("filename {}", direction),
                "size" => format!("size {}", direction),
                "progress" => format!("progress {}", direction),
                _ => format!("created_at {}", direction),
            };

            let mut batch_filters = vec!["batch_id IS NOT NULL".to_string(), "is_current = 1".to_string()];
            if let Some(status) = &status_filter {
                batch_filters.push(format!("state = '{}'", status));
            }
            if let Some(source_clause) = row_source_filter_clause(source_filter.as_deref()) {
                batch_filters.push(source_clause.to_string());
            }
            let batch_where_clause = batch_filters.join(" AND ");

            let batch_query = format!(
                "SELECT
                    batch_id,
                    COALESCE(MAX(batch_name), '') as batch_name,
                    COUNT(*) as total_items,
                    SUM(CASE WHEN state = 'COMPLETED' THEN 1 ELSE 0 END) as completed_items,
                    SUM(CASE WHEN state = 'FAILED' THEN 1 ELSE 0 END) as failed_items,
                    SUM(CASE WHEN state = 'DOWNLOADING' OR state = 'STARTING' THEN 1 ELSE 0 END) as downloading_items,
                    SUM(CASE WHEN state = 'PAUSED' THEN 1 ELSE 0 END) as paused_items,
                    SUM(CASE WHEN state = 'QUEUED' OR state = 'WAITING' THEN 1 ELSE 0 END) as queued_items,
                    COALESCE(SUM(size), 0) as total_size,
                    COALESCE(SUM(CASE
                        WHEN state = 'COMPLETED' THEN size
                        WHEN downloaded IS NOT NULL AND downloaded > 0 THEN MIN(downloaded, size)
                        ELSE CAST((progress / 100.0) * size AS INTEGER)
                    END), 0) as downloaded_size,
                    MAX(created_at) as created_at,
                    COALESCE(MAX(download_source), MAX(external_provider), MAX(host), '') as source
                FROM downloads
                WHERE {}
                GROUP BY batch_id
                ORDER BY {}",
                batch_where_clause,
                batch_order_by
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
                    source: row.get(11)?,
                    state,
                })
            })?;

            let mut all_batches = Vec::new();
            for batch in batch_iter {
                match batch {
                    Ok(b) => {
                        tracing::debug!("[DB] Parsed batch: {} - {}", b.batch_id, b.batch_name);
                        if source_matches(&b.source) {
                            all_batches.push(b);
                        }
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

            // Step 2: Get standalone downloads (no batch_id)
            let source_clause = row_source_filter_clause(source_filter.as_deref())
                .map(|clause| format!("AND {}", clause))
                .unwrap_or_default();
            let standalone_query = format!(
                "SELECT id, url, original_url, filename, destination, state, progress, size,
                 downloaded, speed, eta, host, category, priority, segments, retry_count,
                 created_at, started_at, completed_at, wait_until, error_message,
                 batch_id, batch_name, tmdb_id, media_type, media_item_id, tmdb_title, tmdb_season, tmdb_episode, info_path,
                 arr_announced, arr_series_id, arr_movie_id, arr_announce_error, fshare_code,
                 quality, resolution, tvdb_id, download_source, external_provider, external_id, external_instance_id, info_hash, origin_provider, origin_indexer, seeders, leechers, provider_id, provider_item_id, file_idx, source_filename, selected_episodes, execution_strategy, torbox_remote_state, torbox_remote_completed_at, selected_remote_file_ids, selected_artifacts
                 FROM downloads
                 WHERE batch_id IS NULL AND is_current = 1 {} {}
                 ORDER BY {}",
                and_clause,
                source_clause,
                standalone_order_by
            );

            let mut stmt = conn.prepare(&standalone_query)?;
            let standalone_iter = stmt.query_map([], |row| Self::parse_task_from_row_static(row))?;

            let mut all_standalone = Vec::new();
            for task in standalone_iter {
                if let Ok(t) = task {
                    let source = t
                        .download_source
                        .as_deref()
                        .or(t.external_provider.as_deref())
                        .unwrap_or(&t.host);
                    if source_matches(source) {
                        all_standalone.push(t);
                    }
                }
            }

            // qB is execution state only. Rows without a current Flasharr task are not product records.
            let all_torrents: Vec<QbitTorrentRow> = Vec::new();

            // Step 4: Apply pagination to a unified display-row list.
            #[derive(Clone, Copy)]
            enum RowRef { Batch(usize), Download(usize), Torrent(usize) }

            let mut display_rows: Vec<RowRef> = Vec::with_capacity(all_batches.len() + all_standalone.len() + all_torrents.len());
            display_rows.extend((0..all_batches.len()).map(RowRef::Batch));
            display_rows.extend((0..all_standalone.len()).map(RowRef::Download));
            display_rows.extend((0..all_torrents.len()).map(RowRef::Torrent));

            let state_rank = |state: &str| match state {
                "DOWNLOADING" => 0,
                "STARTING" => 1,
                "QUEUED" => 2,
                "PAUSED" => 3,
                "WAITING" => 4,
                "FAILED" => 5,
                "COMPLETED" => 6,
                _ => 7,
            };
            let task_state_rank = |state: DownloadState| match state {
                DownloadState::Downloading => 0,
                DownloadState::Starting => 1,
                DownloadState::Queued => 2,
                DownloadState::Paused => 3,
                DownloadState::Waiting => 4,
                DownloadState::Failed => 5,
                DownloadState::Completed => 6,
                _ => 7,
            };
            let row_name = |row: RowRef| -> String {
                match row {
                    RowRef::Batch(idx) => all_batches[idx].batch_name.clone(),
                    RowRef::Download(idx) => all_standalone[idx].filename.clone(),
                    RowRef::Torrent(idx) => all_torrents[idx].name.clone(),
                }
            };
            let row_created = |row: RowRef| -> i64 {
                match row {
                    RowRef::Batch(idx) => chrono::DateTime::parse_from_rfc3339(&all_batches[idx].created_at).map(|d| d.timestamp()).unwrap_or(0),
                    RowRef::Download(idx) => all_standalone[idx].created_at.timestamp(),
                    RowRef::Torrent(idx) => all_torrents[idx].added_on,
                }
            };
            let row_source = |row: RowRef| -> String {
                match row {
                    RowRef::Batch(idx) => all_batches[idx].source.clone(),
                    RowRef::Download(idx) => all_standalone[idx]
                        .download_source
                        .clone()
                        .or_else(|| all_standalone[idx].external_provider.clone())
                        .unwrap_or_else(|| all_standalone[idx].host.clone()),
                    RowRef::Torrent(_) => "torrent".to_string(),
                }
            };
            let row_status_rank = |row: RowRef| -> i32 {
                match row {
                    RowRef::Batch(idx) => state_rank(all_batches[idx].state.as_str()),
                    RowRef::Download(idx) => task_state_rank(all_standalone[idx].state),
                    RowRef::Torrent(idx) => task_state_rank(crate::torrent::qbittorrent::qbit_state_to_flasharr(&all_torrents[idx].state, all_torrents[idx].progress)),
                }
            };
            let row_size = |row: RowRef| -> u64 {
                match row {
                    RowRef::Batch(idx) => all_batches[idx].total_size,
                    RowRef::Download(idx) => all_standalone[idx].size,
                    RowRef::Torrent(idx) => all_torrents[idx].size.max(0) as u64,
                }
            };
            let row_progress = |row: RowRef| -> f32 {
                match row {
                    RowRef::Batch(idx) => all_batches[idx].progress,
                    RowRef::Download(idx) => all_standalone[idx].progress,
                    RowRef::Torrent(idx) => (all_torrents[idx].progress * 100.0) as f32,
                }
            };

            display_rows.sort_by(|a, b| {
                let ord = match sort_by.as_str() {
                    "status" => row_status_rank(*a).cmp(&row_status_rank(*b)).then_with(|| row_created(*a).cmp(&row_created(*b))),
                    "filename" => row_name(*a).cmp(&row_name(*b)),
                    "size" => row_size(*a).cmp(&row_size(*b)),
                    "progress" => row_progress(*a).partial_cmp(&row_progress(*b)).unwrap_or(std::cmp::Ordering::Equal),
                    "source" => row_source(*a).cmp(&row_source(*b)),
                    _ => row_created(*a).cmp(&row_created(*b)),
                };
                if sort_dir == "asc" { ord } else { ord.reverse() }
            });

            let total = display_rows.len() as u64;
            let offset = ((page.saturating_sub(1)) * limit) as usize;
            let page_rows = display_rows.into_iter().skip(offset).take(limit as usize);
            let mut batches_to_return = Vec::new();
            let mut standalone_to_return = Vec::new();
            let mut torrents_to_return = Vec::new();
            let mut rows_to_return = Vec::new();
            for row in page_rows {
                match row {
                    RowRef::Batch(idx) => {
                        rows_to_return.push(crate::api::downloads::DisplayRowRef {
                            kind: "batch".to_string(),
                            id: all_batches[idx].batch_id.clone(),
                        });
                        batches_to_return.push(all_batches[idx].clone());
                    }
                    RowRef::Download(idx) => {
                        rows_to_return.push(crate::api::downloads::DisplayRowRef {
                            kind: "download".to_string(),
                            id: all_standalone[idx].id.to_string(),
                        });
                        standalone_to_return.push(all_standalone[idx].clone());
                    }
                    RowRef::Torrent(idx) => {
                        rows_to_return.push(crate::api::downloads::DisplayRowRef {
                            kind: "torrent".to_string(),
                            id: format!("{}:{}", all_torrents[idx].instance_id, all_torrents[idx].hash),
                        });
                        torrents_to_return.push(all_torrents[idx].clone());
                    }
                }
            }

            Ok((batches_to_return, standalone_to_return, rows_to_return, torrents_to_return, total))
        }).await.unwrap()
    }

    // ============================================================================
    // Account Management
    // ============================================================================

    /// Save or update an account
    pub fn save_account(
        &self,
        email: &str,
        session_id: Option<&str>,
        token: Option<&str>,
        expires_at: Option<i64>,
    ) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO accounts (email, session_id, token, expires_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![email, session_id, token, expires_at],
        )?;
        Ok(())
    }

    /// Get account by email
    pub fn get_account(&self, email: &str) -> Result<Option<Account>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
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
        })
        .optional()
    }

    /// Get all accounts
    pub fn get_all_accounts(&self) -> Result<Vec<Account>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt =
            conn.prepare("SELECT email, session_id, token, expires_at, created_at FROM accounts")?;

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
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("DELETE FROM accounts WHERE email = ?1", params![email])?;
        Ok(())
    }

    // ============================================================================
    // qBittorrent Instance Mirror
    // ============================================================================

    fn parse_qbit_instance(row: &rusqlite::Row) -> rusqlite::Result<QbitInstance> {
        Ok(QbitInstance {
            id: row.get(0)?,
            name: row.get(1)?,
            url: row.get(2)?,
            username: row.get(3)?,
            password: row.get(4)?,
            enabled: row.get::<_, i64>(5)? != 0,
            default_category: row.get(6)?,
            save_path: row.get(7)?,
            poll_interval_secs: row.get::<_, i64>(8)?.max(2) as u64,
            last_rid: row.get(9)?,
            sync_health: row.get(10)?,
            last_sync_at: row.get(11)?,
            app_version: row.get(12)?,
            webapi_version: row.get(13)?,
            last_seen_at: row.get(14)?,
            last_error: row.get(15)?,
            created_at: row.get(16)?,
            updated_at: row.get(17)?,
        })
    }

    pub fn list_qbit_instances(&self) -> Result<Vec<QbitInstance>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare("SELECT id, name, url, username, password, enabled, default_category, save_path, poll_interval_secs, last_rid, sync_health, last_sync_at, app_version, webapi_version, last_seen_at, last_error, created_at, updated_at FROM qbit_instances ORDER BY created_at ASC")?;
        let rows = stmt.query_map([], Self::parse_qbit_instance)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_qbit_instance(&self, id: &str) -> Result<Option<QbitInstance>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.query_row("SELECT id, name, url, username, password, enabled, default_category, save_path, poll_interval_secs, last_rid, sync_health, last_sync_at, app_version, webapi_version, last_seen_at, last_error, created_at, updated_at FROM qbit_instances WHERE id = ?1", params![id], Self::parse_qbit_instance).optional()
    }

    pub fn default_qbit_instance(&self) -> Result<Option<QbitInstance>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.query_row("SELECT id, name, url, username, password, enabled, default_category, save_path, poll_interval_secs, last_rid, sync_health, last_sync_at, app_version, webapi_version, last_seen_at, last_error, created_at, updated_at FROM qbit_instances WHERE enabled = 1 ORDER BY CASE id WHEN 'default' THEN 0 ELSE 1 END, created_at ASC LIMIT 1", [], Self::parse_qbit_instance).optional()
    }

    pub fn save_qbit_instance(&self, instance: &QbitInstance) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("INSERT INTO qbit_instances (id, name, url, username, password, enabled, default_category, save_path, poll_interval_secs, last_rid, app_version, webapi_version, last_seen_at, last_error, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, COALESCE(NULLIF(?15, ''), datetime('now')), datetime('now'))
            ON CONFLICT(id) DO UPDATE SET name=excluded.name, url=excluded.url, username=excluded.username, password=excluded.password, enabled=excluded.enabled, default_category=excluded.default_category, save_path=excluded.save_path, poll_interval_secs=excluded.poll_interval_secs, updated_at=datetime('now')",
            params![instance.id, instance.name, instance.url, instance.username, instance.password, instance.enabled as i32, instance.default_category, instance.save_path, instance.poll_interval_secs as i64, instance.last_rid, instance.app_version, instance.webapi_version, instance.last_seen_at, instance.last_error, instance.created_at])?;
        Ok(())
    }

    pub fn delete_qbit_instance(&self, id: &str) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "DELETE FROM qbit_torrents WHERE instance_id = ?1",
            params![id],
        )?;
        conn.execute("DELETE FROM qbit_instances WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn update_qbit_instance_sync(
        &self,
        id: &str,
        rid: i64,
        app_version: Option<&str>,
        webapi_version: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("UPDATE qbit_instances SET last_rid = ?2, app_version = COALESCE(?3, app_version), webapi_version = COALESCE(?4, webapi_version), last_seen_at = CASE WHEN ?5 IS NULL THEN datetime('now') ELSE last_seen_at END, last_error = ?5, updated_at = datetime('now') WHERE id = ?1", params![id, rid, app_version, webapi_version, error])?;
        Ok(())
    }

    pub fn upsert_qbit_torrent(
        &self,
        instance_id: &str,
        torrent: &crate::torrent::qbittorrent::QBittorrentTorrent,
    ) -> Result<QbitTorrentRow> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let now = chrono::Utc::now().to_rfc3339();
        let raw_json = serde_json::to_string(torrent).unwrap_or_else(|_| "{}".to_string());
        conn.execute("INSERT OR REPLACE INTO qbit_torrents (instance_id, hash, name, state, progress, size, downloaded, uploaded, dlspeed, upspeed, eta, ratio, category, tags, save_path, content_path, tracker, added_on, completion_on, amount_left, availability, num_seeds, num_leechs, seen_complete, last_activity, raw_json, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27)",
            params![instance_id, torrent.hash.to_ascii_lowercase(), torrent.name, torrent.state, torrent.progress, torrent.size, torrent.downloaded, torrent.uploaded, torrent.dlspeed, torrent.upspeed, torrent.eta, torrent.ratio, torrent.category, torrent.tags, torrent.save_path, torrent.content_path, torrent.tracker, torrent.added_on, torrent.completion_on, torrent.amount_left, torrent.availability, torrent.num_seeds, torrent.num_leechs, torrent.seen_complete, torrent.last_activity, raw_json, now])?;
        self.get_qbit_torrent(instance_id, &torrent.hash)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn apply_qbit_sync(
        &self,
        instance_id: &str,
        rid: i64,
        full: bool,
        torrents: &[crate::torrent::qbittorrent::QBittorrentTorrent],
        removed: &[String],
        app: Option<&str>,
        webapi: Option<&str>,
    ) -> Result<QbitSyncApplied> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let tx = conn.transaction()?;
        let incoming: std::collections::HashSet<_> = torrents
            .iter()
            .map(|t| t.hash.to_ascii_lowercase())
            .collect();
        let mut replaced = Vec::new();
        if full {
            let mut stmt = tx.prepare("SELECT hash FROM qbit_torrents WHERE instance_id=?1")?;
            replaced = stmt
                .query_map([instance_id], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .filter(|h| !incoming.contains(h))
                .collect();
            drop(stmt);
            tx.execute(
                "DELETE FROM qbit_torrents WHERE instance_id=?1",
                [instance_id],
            )?;
        }
        let now = chrono::Utc::now().to_rfc3339();
        for t in torrents {
            let raw = serde_json::to_string(t)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            tx.execute("INSERT OR REPLACE INTO qbit_torrents (instance_id,hash,name,state,progress,size,downloaded,uploaded,dlspeed,upspeed,eta,ratio,category,tags,save_path,content_path,tracker,added_on,completion_on,amount_left,availability,num_seeds,num_leechs,seen_complete,last_activity,raw_json,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27)", params![instance_id,t.hash.to_ascii_lowercase(),t.name,t.state,t.progress,t.size,t.downloaded,t.uploaded,t.dlspeed,t.upspeed,t.eta,t.ratio,t.category,t.tags,t.save_path,t.content_path,t.tracker,t.added_on,t.completion_on,t.amount_left,t.availability,t.num_seeds,t.num_leechs,t.seen_complete,t.last_activity,raw,now])?;
        }
        for h in removed {
            tx.execute(
                "DELETE FROM qbit_torrents WHERE instance_id=?1 AND hash=?2",
                params![instance_id, h.to_ascii_lowercase()],
            )?;
        }
        tx.execute("UPDATE qbit_instances SET last_rid=?2,sync_health='healthy',last_sync_at=?3,last_seen_at=?3,last_error=NULL,app_version=COALESCE(?4,app_version),webapi_version=COALESCE(?5,webapi_version),updated_at=datetime('now') WHERE id=?1", params![instance_id,rid,now,app,webapi])?;
        let mut upserted = Vec::new();
        for t in torrents {
            upserted.push(tx.query_row("SELECT instance_id,hash,name,state,progress,size,downloaded,uploaded,dlspeed,upspeed,eta,ratio,category,tags,save_path,content_path,tracker,added_on,completion_on,amount_left,availability,num_seeds,num_leechs,seen_complete,last_activity,raw_json,updated_at FROM qbit_torrents WHERE instance_id=?1 AND hash=?2",params![instance_id,t.hash.to_ascii_lowercase()],Self::parse_qbit_torrent)?);
        }
        tx.commit()?;
        Ok(QbitSyncApplied {
            upserted,
            removed: removed.iter().map(|h| h.to_ascii_lowercase()).collect(),
            replaced,
        })
    }

    pub fn mark_qbit_sync_error(
        &self,
        instance_id: &str,
        health: &str,
        _error: &str,
    ) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("UPDATE qbit_instances SET sync_health=?2,last_error='qBittorrent sync failed',updated_at=datetime('now') WHERE id=?1",params![instance_id,if health=="disconnected"{"disconnected"}else{"stale"}])?;
        Ok(())
    }

    pub fn delete_qbit_torrent(&self, instance_id: &str, hash: &str) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "DELETE FROM qbit_torrents WHERE instance_id = ?1 AND hash = ?2",
            params![instance_id, hash.to_ascii_lowercase()],
        )?;
        Ok(())
    }

    pub fn clear_qbit_torrents(&self, instance_id: &str) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "DELETE FROM qbit_torrents WHERE instance_id = ?1",
            params![instance_id],
        )?;
        Ok(())
    }

    fn parse_qbit_torrent(row: &rusqlite::Row) -> rusqlite::Result<QbitTorrentRow> {
        Ok(QbitTorrentRow {
            instance_id: row.get(0)?,
            hash: row.get(1)?,
            name: row.get(2)?,
            state: row.get(3)?,
            progress: row.get(4)?,
            size: row.get(5)?,
            downloaded: row.get(6)?,
            uploaded: row.get(7)?,
            dlspeed: row.get(8)?,
            upspeed: row.get(9)?,
            eta: row.get(10)?,
            ratio: row.get(11)?,
            category: row.get(12)?,
            tags: row.get(13)?,
            save_path: row.get(14)?,
            content_path: row.get(15)?,
            tracker: row.get(16)?,
            added_on: row.get(17)?,
            completion_on: row.get(18)?,
            amount_left: row.get(19)?,
            availability: row.get(20)?,
            num_seeds: row.get(21)?,
            num_leechs: row.get(22)?,
            seen_complete: row.get(23)?,
            last_activity: row.get(24)?,
            raw_json: row.get(25)?,
            updated_at: row.get(26)?,
        })
    }

    pub fn get_qbit_torrent(
        &self,
        instance_id: &str,
        hash: &str,
    ) -> Result<Option<QbitTorrentRow>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.query_row("SELECT instance_id, hash, name, state, progress, size, downloaded, uploaded, dlspeed, upspeed, eta, ratio, category, tags, save_path, content_path, tracker, added_on, completion_on, amount_left, availability, num_seeds, num_leechs, seen_complete, last_activity, raw_json, updated_at FROM qbit_torrents WHERE instance_id = ?1 AND hash = ?2", params![instance_id, hash.to_ascii_lowercase()], Self::parse_qbit_torrent).optional()
    }

    pub fn list_qbit_torrents(
        &self,
        instance_id: &str,
        query: &QbitTorrentListQuery,
    ) -> Result<(Vec<QbitTorrentRow>, u64)> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let rows = conn.prepare("SELECT instance_id, hash, name, state, progress, size, downloaded, uploaded, dlspeed, upspeed, eta, ratio, category, tags, save_path, content_path, tracker, added_on, completion_on, amount_left, availability, num_seeds, num_leechs, seen_complete, last_activity, raw_json, updated_at FROM qbit_torrents WHERE instance_id = ?1")?
            .query_map(params![instance_id], Self::parse_qbit_torrent)?
            .filter_map(|row| row.ok())
            .filter(|row| qbit_row_matches(row, query))
            .collect::<Vec<_>>();
        let mut rows = rows;
        sort_qbit_rows(&mut rows, &query.sort, &query.order);
        let total = rows.len() as u64;
        let limit = query.limit.clamp(1, 2000) as usize;
        let page = query.page.max(1) as usize;
        let start = page.saturating_sub(1) * limit;
        let page_rows = rows.into_iter().skip(start).take(limit).collect();
        Ok((page_rows, total))
    }

    /// Uncapped authoritative snapshot for WebSocket reconnect replacement only.
    pub fn qbit_snapshot_full(&self) -> Result<Vec<(String, Vec<QbitTorrentRow>)>> {
        let instances = self.list_qbit_instances()?;
        let mut out = Vec::new();
        for instance in instances.into_iter().filter(|i| i.enabled) {
            let conn = self
                .pool
                .get()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let rows = conn.prepare("SELECT instance_id, hash, name, state, progress, size, downloaded, uploaded, dlspeed, upspeed, eta, ratio, category, tags, save_path, content_path, tracker, added_on, completion_on, amount_left, availability, num_seeds, num_leechs, seen_complete, last_activity, raw_json, updated_at FROM qbit_torrents WHERE instance_id=?1 ORDER BY added_on DESC")?
                .query_map(params![instance.id], Self::parse_qbit_torrent)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            out.push((instance.id, rows));
        }
        Ok(out)
    }

    /// Return qB torrent rows that have no corresponding Flasharr download task.
    /// Anti-join by info_hash OR (external_instance_id, external_id) to avoid duplicates
    /// with Flasharr-created torrent tasks already visible in the downloads list.
    pub fn list_untracked_qbit_torrents(&self) -> Result<Vec<QbitTorrentRow>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT q.instance_id, q.hash, q.name, q.state, q.progress, q.size,
                    q.downloaded, q.uploaded, q.dlspeed, q.upspeed, q.eta, q.ratio,
                    q.category, q.tags, q.save_path, q.content_path, q.tracker,
                    q.added_on, q.completion_on, q.amount_left, q.availability,
                    q.num_seeds, q.num_leechs, q.seen_complete, q.last_activity,
                    q.raw_json, q.updated_at
             FROM qbit_torrents q
             WHERE NOT EXISTS (
                 SELECT 1 FROM downloads d
                 WHERE d.is_current = 1 AND (lower(d.info_hash) = lower(q.hash)
                    OR (d.external_instance_id = q.instance_id AND lower(d.external_id) = lower(q.hash)))
             )
             ORDER BY q.added_on DESC"
        )?;
        let rows = stmt
            .query_map([], Self::parse_qbit_torrent)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    // ============================================================================
    // Browser Authentication
    // ============================================================================

    pub fn has_app_users(&self) -> Result<bool> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.query_row("SELECT EXISTS(SELECT 1 FROM app_users)", [], |row| {
            row.get(0)
        })
    }

    pub fn create_initial_app_user(
        &self,
        id: &str,
        username: &str,
        password_hash: &str,
    ) -> Result<bool> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let tx = conn.transaction()?;
        let inserted = tx.execute(
            "INSERT INTO app_users (id, username, password_hash)
             SELECT ?1, ?2, ?3 WHERE NOT EXISTS (SELECT 1 FROM app_users)",
            params![id, username, password_hash],
        )?;
        tx.commit()?;
        Ok(inserted == 1)
    }

    pub fn get_app_user_password_hash(&self, username: &str) -> Result<Option<(String, String)>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.query_row(
            "SELECT id, password_hash FROM app_users WHERE username = ?1 COLLATE NOCASE",
            params![username],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
    }

    pub fn create_app_session(
        &self,
        token_hash: &str,
        user_id: &str,
        expires_at: i64,
    ) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO app_sessions (token_hash, user_id, expires_at, created_at, last_seen_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![token_hash, user_id, expires_at, now],
        )?;
        Ok(())
    }

    pub fn validate_app_session(&self, token_hash: &str) -> Result<Option<String>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "DELETE FROM app_sessions WHERE expires_at <= ?1 OR last_seen_at <= ?2",
            params![now, now - APP_SESSION_IDLE_TTL_SECS],
        )?;
        let username = conn
            .query_row(
                "SELECT u.username FROM app_sessions s JOIN app_users u ON u.id = s.user_id WHERE s.token_hash = ?1 AND s.expires_at > ?2 AND s.last_seen_at > ?3",
                params![token_hash, now, now - APP_SESSION_IDLE_TTL_SECS],
                |row| row.get(0),
            )
            .optional()?;
        if username.is_some() {
            conn.execute(
                "UPDATE app_sessions SET last_seen_at = ?2 WHERE token_hash = ?1",
                params![token_hash, now],
            )?;
        }
        Ok(username)
    }

    pub fn delete_app_session(&self, token_hash: &str) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "DELETE FROM app_sessions WHERE token_hash = ?1",
            params![token_hash],
        )?;
        Ok(())
    }

    pub fn update_app_user_password(&self, user_id: &str, password_hash: &str) -> Result<bool> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let tx = conn.transaction()?;
        let updated = tx.execute(
            "UPDATE app_users SET password_hash = ?2 WHERE id = ?1",
            params![user_id, password_hash],
        )?;
        if updated == 1 {
            tx.execute(
                "DELETE FROM app_sessions WHERE user_id = ?1",
                params![user_id],
            )?;
        }
        tx.commit()?;
        Ok(updated == 1)
    }

    pub fn count_active_tasks_for_qbit_instance(&self, instance_id: &str) -> Result<u64> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.query_row(
            "SELECT COUNT(*) FROM downloads WHERE external_instance_id = ?1 AND is_current = 1 AND state IN ('QUEUED', 'STARTING', 'DOWNLOADING', 'PAUSED', 'WAITING', 'IMPORTING')",
            params![instance_id],
            |row| row.get(0),
        )
    }

    // ============================================================================
    // Stremio Addon Registry
    // ============================================================================

    pub fn list_stremio_addons(&self) -> Result<Vec<crate::stremio::registry::StoredAddon>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare("SELECT id,name,origin,encrypted_config_url,credential_present,enabled,sort_order,stream_types,capabilities_known,version,description,logo FROM stremio_addons ORDER BY sort_order,id")?;
        let rows = stmt.query_map([], |row| {
            Ok(crate::stremio::registry::StoredAddon {
                public: crate::stremio::registry::AddonRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    version: row.get(9)?,
                    description: row.get(10)?,
                    logo: row.get(11)?,
                    origin: row.get(2)?,
                    credential_present: row.get::<_, i64>(4)? != 0,
                    enabled: row.get::<_, i64>(5)? != 0,
                    sort_order: row.get(6)?,
                    stream_types: (row.get::<_, i64>(8)? != 0).then(|| {
                        serde_json::from_str(&row.get::<_, String>(7).unwrap_or_default())
                            .unwrap_or_default()
                    }),
                },
                encrypted_config_url: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn insert_stremio_addon(
        &self,
        addon: &mut crate::stremio::registry::StoredAddon,
    ) -> Result<StremioAddonInsert> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM stremio_addons WHERE id=?1)",
            [&addon.public.id],
            |row| row.get::<_, bool>(0),
        )? {
            return Ok(StremioAddonInsert::Duplicate);
        }
        let count: usize =
            tx.query_row("SELECT COUNT(*) FROM stremio_addons", [], |row| row.get(0))?;
        if count >= MAX_STREMIO_ADDONS {
            return Ok(StremioAddonInsert::LimitReached);
        }
        addon.public.sort_order = tx.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM stremio_addons",
            [],
            |row| row.get(0),
        )?;
        let stream_types =
            serde_json::to_string(addon.public.stream_types.as_deref().unwrap_or_default())
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        tx.execute(
            "INSERT INTO stremio_addons(id,name,origin,encrypted_config_url,credential_present,enabled,sort_order,stream_types,capabilities_known,version,description,logo) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![addon.public.id, addon.public.name, addon.public.origin, addon.encrypted_config_url, addon.public.credential_present as i64, addon.public.enabled as i64, addon.public.sort_order, stream_types, addon.public.stream_types.is_some() as i64, addon.public.version, addon.public.description, addon.public.logo],
        )?;
        tx.commit()?;
        Ok(StremioAddonInsert::Inserted)
    }

    pub fn save_stremio_addon(&self, addon: &crate::stremio::registry::StoredAddon) -> Result<()> {
        self.write_stremio_addon(addon, true).map(|_| ())
    }

    fn write_stremio_addon(
        &self,
        addon: &crate::stremio::registry::StoredAddon,
        upsert: bool,
    ) -> Result<bool> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let stream_types =
            serde_json::to_string(addon.public.stream_types.as_deref().unwrap_or_default())
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let conflict = if upsert {
            "DO UPDATE SET name=excluded.name,origin=excluded.origin,encrypted_config_url=excluded.encrypted_config_url,credential_present=excluded.credential_present,enabled=excluded.enabled,sort_order=excluded.sort_order,stream_types=excluded.stream_types,capabilities_known=excluded.capabilities_known,version=excluded.version,description=excluded.description,logo=excluded.logo,updated_at=strftime('%s','now')"
        } else {
            "DO NOTHING"
        };
        let sql = format!("INSERT INTO stremio_addons(id,name,origin,encrypted_config_url,credential_present,enabled,sort_order,stream_types,capabilities_known,version,description,logo) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(id) {conflict}");
        Ok(conn.execute(
            &sql,
            params![
                addon.public.id,
                addon.public.name,
                addon.public.origin,
                addon.encrypted_config_url,
                addon.public.credential_present as i64,
                addon.public.enabled as i64,
                addon.public.sort_order,
                stream_types,
                addon.public.stream_types.is_some() as i64,
                addon.public.version,
                addon.public.description,
                addon.public.logo
            ],
        )? == 1)
    }

    pub fn set_stremio_addon_enabled(&self, id: &str, enabled: bool) -> Result<bool> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        Ok(conn.execute(
            "UPDATE stremio_addons SET enabled=?2,updated_at=strftime('%s','now') WHERE id=?1",
            params![id, enabled as i64],
        )? == 1)
    }

    pub fn reorder_stremio_addons(&self, ids: &[String]) -> Result<()> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let tx = conn.transaction()?;
        let count: usize =
            tx.query_row("SELECT COUNT(*) FROM stremio_addons", [], |row| row.get(0))?;
        let unique = ids.iter().collect::<std::collections::HashSet<_>>();
        if ids.len() != count || unique.len() != count {
            return Err(rusqlite::Error::InvalidQuery);
        }
        tx.execute(
            "UPDATE stremio_addons SET sort_order=sort_order+1000000",
            [],
        )?;
        for (order, id) in ids.iter().enumerate() {
            if tx.execute("UPDATE stremio_addons SET sort_order=?2,updated_at=strftime('%s','now') WHERE id=?1", params![id, order as i64])? != 1 { return Err(rusqlite::Error::QueryReturnedNoRows); }
        }
        tx.commit()
    }

    pub fn delete_stremio_addon(&self, id: &str) -> Result<bool> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        Ok(conn.execute("DELETE FROM stremio_addons WHERE id=?1", params![id])? == 1)
    }

    // ============================================================================
    // Settings Management
    // ============================================================================

    /// Save or update a setting
    pub fn save_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at)
             VALUES (?1, ?2, strftime('%s', 'now'))",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn save_settings(&self, settings: &[(&str, &str)]) -> Result<()> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for (key, value) in settings {
            tx.execute(
                "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, strftime('%s', 'now'))",
                params![key, value],
            )?;
        }
        tx.commit()
    }

    /// Get setting by key
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        stmt.query_row(params![key], |row| row.get(0)).optional()
    }

    /// Get all settings as a HashMap
    pub fn get_all_settings(&self) -> Result<std::collections::HashMap<String, String>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
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
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(())
    }

    /// Check if onboarding is complete
    pub fn is_onboarding_complete(&self) -> Result<bool> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
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
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('onboarding_complete', 'true')",
            [],
        )?;
        Ok(())
    }

    /// Save Fshare credentials
    pub fn save_fshare_credentials(&self, email: &str, password: &str) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
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
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
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
    pub fn save_arr_config(
        &self,
        service: &str,
        url: &str,
        api_key: &str,
        enabled: bool,
        auto_import: bool,
    ) -> Result<()> {
        let url_key = format!("{service}_url");
        let api_key_key = format!("{service}_api_key");
        let enabled_key = format!("{service}_enabled");
        let auto_import_key = format!("{service}_auto_import");
        self.save_settings(&[
            (&url_key, url),
            (&api_key_key, api_key),
            (&enabled_key, if enabled { "true" } else { "false" }),
            (&auto_import_key, if auto_import { "true" } else { "false" }),
        ])
    }

    /// Save Jellyfin configuration
    pub fn save_jellyfin_config(&self, url: &str, api_key: &str) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
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
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
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
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('indexer_api_key', ?1)",
            params![api_key],
        )?;
        Ok(())
    }

    // ============================================================================
    // Arr Sync Tracking
    // ============================================================================

    /// Update Arr IDs only. `arr_announced` is set only after import polling confirms.
    pub fn update_download_arr_ids(
        &self,
        download_id: &str,
        series_id: Option<i32>,
        movie_id: Option<i32>,
    ) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "UPDATE downloads SET arr_series_id = COALESCE(?1, arr_series_id), arr_movie_id = COALESCE(?2, arr_movie_id) WHERE id = ?3",
            params![series_id, movie_id, download_id],
        )?;
        Ok(())
    }

    /// Update arr_series_id for all downloads with the same TMDB ID
    /// This ensures all episodes in a series batch get the series ID for auto-import
    pub fn update_arr_series_id_by_tmdb(&self, tmdb_id: i64, series_id: i64) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let updated = conn.execute(
            "UPDATE downloads SET arr_series_id = ?1 WHERE tmdb_id = ?2",
            params![series_id, tmdb_id],
        )?;

        if updated == 0 {
            tracing::warn!(
                "No downloads found with tmdb_id={} to update arr_series_id",
                tmdb_id
            );
        } else {
            tracing::info!(
                "Updated {} downloads with tmdb_id={} to arr_series_id={}",
                updated,
                tmdb_id,
                series_id
            );
        }
        Ok(())
    }

    /// Update Radarr movie ID for all downloads with given TMDB ID
    pub fn update_arr_movie_id_by_tmdb(&self, tmdb_id: i64, movie_id: i64) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let updated = conn.execute(
            "UPDATE downloads SET arr_movie_id = ?1 WHERE tmdb_id = ?2",
            params![movie_id, tmdb_id],
        )?;

        if updated == 0 {
            tracing::warn!(
                "No downloads found with tmdb_id={} to update arr_movie_id",
                tmdb_id
            );
        } else {
            tracing::info!(
                "Updated {} downloads with tmdb_id={} to arr_movie_id={}",
                updated,
                tmdb_id,
                movie_id
            );
        }
        Ok(())
    }

    /// Update Arr announcement error for a download
    pub fn update_download_arr_error(&self, download_id: &str, error: &str) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
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
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO media_items (
                media_key, tmdb_id, media_type, title, original_title, year, overview,
                poster_path, backdrop_path, genres, runtime, total_seasons,
                arr_id, arr_type, arr_path, arr_monitored, arr_status,
                arr_quality_profile_id, arr_has_file, arr_size_on_disk,
                tvdb_id, imdb_id, created_at, updated_at, arr_synced_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10, ?11,
                ?12, ?13, ?14, ?15, ?16,
                ?17, ?18, ?19,
                ?20, ?21, ?22, ?23, ?24, ?25
            ) ON CONFLICT(media_key) DO UPDATE SET
                tmdb_id=excluded.tmdb_id,
                media_type=excluded.media_type,
                title=excluded.title,
                original_title=excluded.original_title,
                year=excluded.year,
                overview=excluded.overview,
                poster_path=excluded.poster_path,
                backdrop_path=excluded.backdrop_path,
                genres=excluded.genres,
                runtime=excluded.runtime,
                total_seasons=excluded.total_seasons,
                arr_id=excluded.arr_id,
                arr_type=excluded.arr_type,
                arr_path=excluded.arr_path,
                arr_monitored=excluded.arr_monitored,
                arr_status=excluded.arr_status,
                arr_quality_profile_id=excluded.arr_quality_profile_id,
                arr_has_file=excluded.arr_has_file,
                arr_size_on_disk=excluded.arr_size_on_disk,
                tvdb_id=excluded.tvdb_id,
                imdb_id=excluded.imdb_id,
                updated_at=excluded.updated_at,
                arr_synced_at=excluded.arr_synced_at",
            params![
                item.media_key,
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
        })
        .await
        .unwrap()
    }

    /// Get a media item by TMDB ID
    pub fn get_media_item(&self, tmdb_id: i64) -> Result<Option<MediaItem>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT media_key, tmdb_id, media_type, title, original_title, year, overview,
                    poster_path, backdrop_path, genres, runtime, total_seasons,
                    arr_id, arr_type, arr_path, arr_monitored, arr_status,
                    arr_quality_profile_id, arr_has_file, arr_size_on_disk,
                    tvdb_id, imdb_id, created_at, updated_at, arr_synced_at
             FROM media_items WHERE tmdb_id = ?1",
        )?;
        stmt.query_row(params![tmdb_id], |row| Self::parse_media_item(row))
            .optional()
    }

    /// Async get media item
    pub async fn get_media_item_async(&self, tmdb_id: i64) -> Result<Option<MediaItem>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let db = Db { pool };
            db.get_media_item(tmdb_id)
        })
        .await
        .unwrap()
    }

    /// Get all media items, ordered by updated_at DESC
    pub fn get_all_media_items(&self) -> Result<Vec<MediaItem>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT media_key, tmdb_id, media_type, title, original_title, year, overview,
                    poster_path, backdrop_path, genres, runtime, total_seasons,
                    arr_id, arr_type, arr_path, arr_monitored, arr_status,
                    arr_quality_profile_id, arr_has_file, arr_size_on_disk,
                    tvdb_id, imdb_id, created_at, updated_at, arr_synced_at
             FROM media_items ORDER BY updated_at DESC",
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
        })
        .await
        .unwrap()
    }

    /// Get media items by type ("movie" or "tv")
    pub fn get_media_items_by_type(&self, media_type: &str) -> Result<Vec<MediaItem>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT media_key, tmdb_id, media_type, title, original_title, year, overview,
                    poster_path, backdrop_path, genres, runtime, total_seasons,
                    arr_id, arr_type, arr_path, arr_monitored, arr_status,
                    arr_quality_profile_id, arr_has_file, arr_size_on_disk,
                    tvdb_id, imdb_id, created_at, updated_at, arr_synced_at
             FROM media_items WHERE media_type = ?1 ORDER BY updated_at DESC",
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
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
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
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "DELETE FROM media_episodes WHERE tmdb_id = ?1",
            params![tmdb_id],
        )?;
        conn.execute(
            "DELETE FROM media_items WHERE tmdb_id = ?1",
            params![tmdb_id],
        )?;
        Ok(())
    }

    /// Get download count per media item (for library display)
    pub fn get_media_download_counts(&self) -> Result<std::collections::HashMap<i64, usize>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT tmdb_id, COUNT(*) FROM downloads WHERE tmdb_id IS NOT NULL AND is_current = 1 GROUP BY tmdb_id",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?;
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
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO media_episodes (
                media_key, tmdb_id, season_number, episode_number, title, overview, air_date,
                arr_episode_id, arr_has_file, arr_monitored, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                ep.media_key,
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
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT id, media_key, tmdb_id, season_number, episode_number, title, overview, air_date,
                    arr_episode_id, arr_has_file, arr_monitored, created_at, updated_at
             FROM media_episodes WHERE tmdb_id = ?1
             ORDER BY season_number ASC, episode_number ASC",
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
        })
        .await
        .unwrap()
    }

    /// Get a specific episode
    pub fn get_episode(
        &self,
        tmdb_id: i64,
        season: i32,
        episode: i32,
    ) -> Result<Option<MediaEpisode>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT id, media_key, tmdb_id, season_number, episode_number, title, overview, air_date,
                    arr_episode_id, arr_has_file, arr_monitored, created_at, updated_at
             FROM media_episodes WHERE tmdb_id = ?1 AND season_number = ?2 AND episode_number = ?3",
        )?;
        stmt.query_row(params![tmdb_id, season, episode], |row| {
            Self::parse_media_episode(row)
        })
        .optional()
    }

    /// Get a media item with all its associated downloads
    pub fn get_media_with_downloads(
        &self,
        tmdb_id: i64,
    ) -> Result<Option<(MediaItem, Vec<crate::downloader::task::DownloadTask>)>> {
        let item = self.get_media_item(tmdb_id)?;
        match item {
            Some(media) => {
                let conn = self
                    .pool
                    .get()
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                let mut stmt = conn.prepare(
                    "SELECT id, url, original_url, filename, destination, state, progress, size,
                             downloaded, speed, eta, host, category, priority, segments, retry_count,
                             created_at, started_at, completed_at, wait_until, error_message,
                             batch_id, batch_name, tmdb_id, media_type, media_item_id, tmdb_title, tmdb_season, tmdb_episode, info_path,
                             arr_announced, arr_series_id, arr_movie_id, arr_announce_error, fshare_code,
                             quality, resolution, tvdb_id, download_source, external_provider, external_id, external_instance_id, info_hash, origin_provider, origin_indexer, seeders, leechers, provider_id, provider_item_id, file_idx, source_filename, selected_episodes, execution_strategy, torbox_remote_state, torbox_remote_completed_at, selected_remote_file_ids, selected_artifacts
                     FROM downloads WHERE tmdb_id = ?1 AND is_current = 1
                     ORDER BY created_at DESC"
                )?;
                let task_iter =
                    stmt.query_map(params![tmdb_id], |row| Self::parse_task_from_row(row))?;
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
    pub async fn get_media_with_downloads_async(
        &self,
        tmdb_id: i64,
    ) -> Result<Option<(MediaItem, Vec<crate::downloader::task::DownloadTask>)>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let db = Db { pool };
            db.get_media_with_downloads(tmdb_id)
        })
        .await
        .unwrap()
    }

    /// Get all downloads for a given TMDB ID directly from the downloads table.
    /// Does NOT require a media_items row — works for queued downloads before library sync.
    pub fn get_downloads_by_tmdb_id(
        &self,
        tmdb_id: i64,
    ) -> Result<Vec<crate::downloader::task::DownloadTask>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT id, url, original_url, filename, destination, state, progress, size,
                     downloaded, speed, eta, host, category, priority, segments, retry_count,
                     created_at, started_at, completed_at, wait_until, error_message,
                     batch_id, batch_name, tmdb_id, media_type, media_item_id, tmdb_title, tmdb_season, tmdb_episode, info_path,
                     arr_announced, arr_series_id, arr_movie_id, arr_announce_error, fshare_code,
                     quality, resolution, tvdb_id, download_source, external_provider, external_id, external_instance_id, info_hash, origin_provider, origin_indexer, seeders, leechers, provider_id, provider_item_id, file_idx, source_filename, selected_episodes, execution_strategy, torbox_remote_state, torbox_remote_completed_at, selected_remote_file_ids, selected_artifacts
             FROM downloads WHERE tmdb_id = ?1 AND is_current = 1
             ORDER BY created_at DESC",
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
    pub async fn get_downloads_by_tmdb_id_async(
        &self,
        tmdb_id: i64,
    ) -> Result<Vec<crate::downloader::task::DownloadTask>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let db = Db { pool };
            db.get_downloads_by_tmdb_id(tmdb_id)
        })
        .await
        .unwrap()
    }

    // ── Row Parsers ───────────────────────────────────────────────────────

    fn parse_media_item(row: &rusqlite::Row) -> rusqlite::Result<MediaItem> {
        Ok(MediaItem {
            media_key: row.get(0)?,
            tmdb_id: row.get(1)?,
            media_type: row.get(2)?,
            title: row.get(3)?,
            original_title: row.get(4)?,
            year: row.get(5)?,
            overview: row.get(6)?,
            poster_path: row.get(7)?,
            backdrop_path: row.get(8)?,
            genres: row.get(9)?,
            runtime: row.get(10)?,
            total_seasons: row.get(11)?,
            arr_id: row.get(12)?,
            arr_type: row.get(13)?,
            arr_path: row.get(14)?,
            arr_monitored: row.get::<_, Option<i32>>(15)?.unwrap_or(0) != 0,
            arr_status: row.get(16)?,
            arr_quality_profile_id: row.get(17)?,
            arr_has_file: row.get::<_, Option<i32>>(18)?.unwrap_or(0) != 0,
            arr_size_on_disk: row.get::<_, Option<i64>>(19)?.unwrap_or(0),
            tvdb_id: row.get(20)?,
            imdb_id: row.get(21)?,
            created_at: row.get(22)?,
            updated_at: row.get(23)?,
            arr_synced_at: row.get(24)?,
        })
    }

    fn parse_media_episode(row: &rusqlite::Row) -> rusqlite::Result<MediaEpisode> {
        Ok(MediaEpisode {
            id: row.get(0)?,
            media_key: row.get(1)?,
            tmdb_id: row.get(2)?,
            season_number: row.get(3)?,
            episode_number: row.get(4)?,
            title: row.get(5)?,
            overview: row.get(6)?,
            air_date: row.get(7)?,
            arr_episode_id: row.get(8)?,
            arr_has_file: row.get::<_, Option<i32>>(9)?.unwrap_or(0) != 0,
            arr_monitored: row.get::<_, Option<i32>>(10)?.unwrap_or(0) != 0,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::torrent::qbittorrent::QBittorrentTorrent;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn activity_events_are_idempotent_and_newest_first() {
        let file = NamedTempFile::new().unwrap();
        let db = Db::new(file.path()).unwrap();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        db.record_activity_event("queued", first, "Queued: first.mkv")
            .unwrap();
        db.record_activity_event("queued", first, "Queued: duplicate.mkv")
            .unwrap();
        db.record_activity_event("completed", second, "Completed: second.mkv")
            .unwrap();

        let events = db.get_activity_events_async(1).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].task_id, second.to_string());
        let all = db.get_activity_events_async(10).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn phase6_real_task_db_reopen_roundtrip_is_exact_and_secret_safe() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        let signed = "https://download.example/file.mkv?token=phase6-secret";
        let mut task = DownloadTask::new(
            signed.into(),
            "Show.S01E02.mkv".into(),
            "torbox".into(),
            "tv".into(),
        );
        task.execution_strategy = Some("torbox_http".into());
        task.external_id = Some("torbox:441:91".into());
        task.provider_id = Some("torbox".into());
        task.provider_item_id = Some("441".into());
        task.torbox_remote_state = Some("ready".into());
        task.torbox_remote_completed_at = Some("2026-07-18T10:00:00+00:00".into());
        task.selected_remote_file_ids = Some("[91,92]".into());
        task.selected_artifacts = Some(r#"[{"remoteFileId":91,"filename":"Show.S01E01.mkv"},{"remoteFileId":92,"filename":"Show.S01E02.mkv"}]"#.into());
        task.info_hash = Some("0123456789abcdef0123456789abcdef01234567".into());
        task.file_idx = Some(7);
        task.source_filename = Some("Show.S01E02.mkv".into());
        task.selected_episodes =
            Some(r#"[{"season":1,"episode":1},{"season":1,"episode":2}]"#.into());
        task.completed_at = Some(
            chrono::DateTime::parse_from_rfc3339("2026-07-18T10:01:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        );
        let id = task.id;

        Db::new(&path).unwrap().save_task(&task).unwrap();
        let loaded = Db::new(&path).unwrap().get_task_by_id(id).unwrap().unwrap();
        assert_eq!(loaded.execution_strategy, task.execution_strategy);
        assert_eq!(loaded.external_id, task.external_id);
        assert_eq!(loaded.provider_item_id, task.provider_item_id);
        assert_eq!(loaded.torbox_remote_state, task.torbox_remote_state);
        assert_eq!(
            loaded.torbox_remote_completed_at,
            task.torbox_remote_completed_at
        );
        assert_eq!(
            loaded.selected_remote_file_ids,
            task.selected_remote_file_ids
        );
        assert_eq!(loaded.selected_artifacts, task.selected_artifacts);
        assert_eq!(loaded.info_hash, task.info_hash);
        assert_eq!(loaded.file_idx, task.file_idx);
        assert_eq!(loaded.source_filename, task.source_filename);
        assert_eq!(loaded.selected_episodes, task.selected_episodes);
        assert_eq!(loaded.completed_at, task.completed_at);
        let json = serde_json::to_string(&loaded).unwrap();
        assert!(!json.contains("phase6-secret"));
        assert!(!json.contains("download.example"));
        let conn = rusqlite::Connection::open(path).unwrap();
        let stored: (String, String) = conn
            .query_row(
                "SELECT url, original_url FROM downloads WHERE id=?1",
                [id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(stored, ("torbox:441:91".into(), "torbox:441:91".into()));
    }

    fn stored_addon(id: &str, order: i64) -> crate::stremio::registry::StoredAddon {
        crate::stremio::registry::StoredAddon {
            public: crate::stremio::registry::AddonRecord {
                id: id.into(),
                name: format!("Addon {id}"),
                version: "1.0.0".into(),
                description: "Addon description".into(),
                logo: Some(format!("https://cdn.example/{id}.png")),
                origin: "https://addon.example".into(),
                credential_present: true,
                enabled: true,
                sort_order: order,
                stream_types: Some(vec!["movie".into(), "series".into()]),
            },
            encrypted_config_url: format!("cipher-{id}"),
        }
    }

    #[test]
    fn stremio_addon_crud_reorders_exact_set_atomically() {
        let file = NamedTempFile::new().unwrap();
        let db = Db::new(file.path()).unwrap();
        db.save_stremio_addon(&stored_addon("a", 0)).unwrap();
        db.save_stremio_addon(&stored_addon("b", 1)).unwrap();
        assert!(db.set_stremio_addon_enabled("a", false).unwrap());
        db.reorder_stremio_addons(&["b".into(), "a".into()])
            .unwrap();
        let rows = db.list_stremio_addons().unwrap();
        assert_eq!(
            rows.iter()
                .map(|x| x.public.id.as_str())
                .collect::<Vec<_>>(),
            vec!["b", "a"]
        );
        assert!(!rows[1].public.enabled);
        assert_eq!(rows[0].public.version, "1.0.0");
        assert_eq!(rows[0].public.description, "Addon description");
        assert_eq!(
            rows[0].public.logo.as_deref(),
            Some("https://cdn.example/b.png")
        );
        assert!(db
            .reorder_stremio_addons(&["a".into(), "missing".into()])
            .is_err());
        assert_eq!(
            db.list_stremio_addons()
                .unwrap()
                .iter()
                .map(|x| x.public.id.as_str())
                .collect::<Vec<_>>(),
            vec!["b", "a"]
        );
        assert!(db.delete_stremio_addon("b").unwrap());
        assert_eq!(db.list_stremio_addons().unwrap().len(), 1);
    }

    #[test]
    fn stremio_addon_insert_enforces_registry_limit() {
        let file = NamedTempFile::new().unwrap();
        let db = Db::new(file.path()).unwrap();
        for index in 0..MAX_STREMIO_ADDONS {
            let mut addon = stored_addon(&format!("addon-{index}"), index as i64);
            assert_eq!(
                db.insert_stremio_addon(&mut addon).unwrap(),
                StremioAddonInsert::Inserted
            );
        }
        let mut overflow = stored_addon("overflow", 99);
        assert_eq!(
            db.insert_stremio_addon(&mut overflow).unwrap(),
            StremioAddonInsert::LimitReached
        );
        assert_eq!(db.list_stremio_addons().unwrap().len(), MAX_STREMIO_ADDONS);
    }

    #[test]
    fn concurrent_stremio_inserts_assign_distinct_order_and_preserve_duplicate() {
        let file = NamedTempFile::new().unwrap();
        let db = Arc::new(Db::new(file.path()).unwrap());
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let handles = ["a", "b"].map(|id| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let mut addon = stored_addon(id, 99);
                barrier.wait();
                assert_eq!(
                    db.insert_stremio_addon(&mut addon).unwrap(),
                    StremioAddonInsert::Inserted
                );
                addon.public.sort_order
            })
        });
        let mut orders = handles.map(|handle| handle.join().unwrap());
        orders.sort_unstable();
        assert_eq!(orders, [0, 1]);

        let mut duplicate = stored_addon("a", 99);
        duplicate.public.name = "Replacement".into();
        assert_eq!(
            db.insert_stremio_addon(&mut duplicate).unwrap(),
            StremioAddonInsert::Duplicate
        );
        let rows = db.list_stremio_addons().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter()
                .find(|addon| addon.public.id == "a")
                .unwrap()
                .public
                .name,
            "Addon a"
        );
    }

    #[test]
    fn legacy_stremio_capabilities_are_unknown_and_refresh_persists_known_types() {
        let file = NamedTempFile::new().unwrap();
        let db = Db::new(file.path()).unwrap();
        let conn = db.pool.get().unwrap();
        conn.execute(
            "INSERT INTO stremio_addons(id,name,origin,encrypted_config_url,enabled,sort_order,stream_types,capabilities_known) VALUES(?1,?2,?3,?4,1,0,'[]',0)",
            params!["legacy", "Legacy", "redacted", "cipher"],
        )
        .unwrap();
        drop(conn);

        let mut addon = db.list_stremio_addons().unwrap().pop().unwrap();
        assert!(addon.public.stream_types.is_none());
        assert_eq!(addon.public.logo, None);
        assert!(addon.supports("movie"));
        assert!(addon.supports("series"));

        addon.public.stream_types = Some(vec!["series".into()]);
        db.save_stremio_addon(&addon).unwrap();
        let refreshed = db.list_stremio_addons().unwrap().pop().unwrap();
        assert_eq!(refreshed.public.stream_types, Some(vec!["series".into()]));
        assert!(!refreshed.supports("movie"));
        assert!(refreshed.supports("series"));
    }

    fn identified_task(
        source: &str,
        media_type: &str,
        id: i64,
        season: Option<u32>,
        episode: Option<u32>,
    ) -> DownloadTask {
        let mut task = DownloadTask::new(
            format!("{source}://item"),
            format!("{source}.mkv"),
            source.into(),
            "other".into(),
        );
        task.download_source = Some(source.into());
        task.media_type = Some(media_type.into());
        if media_type == "movie" {
            task.tmdb_id = Some(id);
        } else {
            task.tvdb_id = Some(id);
        }
        task.tmdb_season = season;
        task.tmdb_episode = episode;
        task
    }

    #[test]
    fn phase6_durable_columns_migrate_idempotently() {
        let file = NamedTempFile::new().expect("temp db");
        Db::new(file.path()).expect("first migration");
        let db = Db::new(file.path()).expect("second migration");
        let conn = db.pool.get().expect("connection");
        let columns = conn
            .prepare("PRAGMA table_info(downloads)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<std::collections::HashSet<_>>>()
            .unwrap();
        for name in [
            "execution_strategy",
            "torbox_remote_state",
            "torbox_remote_completed_at",
            "selected_remote_file_ids",
            "selected_artifacts",
        ] {
            assert!(columns.contains(name), "missing durable column {name}");
        }
    }

    #[test]
    fn canonical_leaf_key_requires_complete_identity() {
        assert_eq!(
            identified_task("fshare", "movie", 550, None, None)
                .canonical_media_leaf_key()
                .as_deref(),
            Some("movie:550")
        );
        assert_eq!(
            identified_task("torrent", "tv", 81189, Some(2), Some(3))
                .canonical_media_leaf_key()
                .as_deref(),
            Some("episode:81189:2:3")
        );
        assert_eq!(
            identified_task("fshare", "tv", 81189, Some(2), None).canonical_media_leaf_key(),
            None
        );
    }

    #[test]
    fn cross_source_replacement_keeps_only_new_movie_current() {
        let file = NamedTempFile::new().expect("temp db");
        let db = Db::new(file.path()).expect("fresh db");
        let old = identified_task("fshare", "movie", 550, None, None);
        let new = identified_task("torrent", "movie", 550, None, None);
        assert!(db.save_task_replacing_current(&old).unwrap().is_none());
        assert_eq!(
            db.save_task_replacing_current(&new).unwrap().unwrap().id,
            old.id
        );
        assert_eq!(
            db.get_current_tasks_for_leaf("movie:550")
                .unwrap()
                .iter()
                .map(|t| t.id)
                .collect::<Vec<_>>(),
            vec![new.id]
        );
    }

    #[test]
    fn episodes_replace_independently_and_missing_ids_never_collide() {
        let file = NamedTempFile::new().expect("temp db");
        let db = Db::new(file.path()).expect("fresh db");
        let old = identified_task("fshare", "tv", 81189, Some(1), Some(2));
        let replacement = identified_task("torbox", "tv", 81189, Some(1), Some(2));
        let other_episode = identified_task("torrent", "tv", 81189, Some(1), Some(3));
        let missing_a = identified_task("fshare", "tv", 81189, Some(1), None);
        let missing_b = identified_task("torrent", "tv", 81189, Some(1), None);
        db.save_task_replacing_current(&old).unwrap();
        db.save_task_replacing_current(&replacement).unwrap();
        db.save_task_replacing_current(&other_episode).unwrap();
        db.save_task_replacing_current(&missing_a).unwrap();
        db.save_task_replacing_current(&missing_b).unwrap();
        assert_eq!(
            db.get_current_tasks_for_leaf("episode:81189:1:2").unwrap()[0].id,
            replacement.id
        );
        assert_eq!(
            db.get_current_tasks_for_leaf("episode:81189:1:3").unwrap()[0].id,
            other_episode.id
        );
        assert!(db.get_task_by_id(missing_a.id).unwrap().is_some());
        assert!(db.get_task_by_id(missing_b.id).unwrap().is_some());
    }

    #[test]
    fn late_save_of_superseded_task_does_not_resurrect_it() {
        for (old_source, new_source) in [("torrent", "fshare"), ("fshare", "torrent")] {
            let file = NamedTempFile::new().unwrap();
            let db = Db::new(file.path()).unwrap();
            let mut old = identified_task(old_source, "movie", 550, None, None);
            let new = identified_task(new_source, "movie", 550, None, None);
            db.save_task_replacing_current(&old).unwrap();
            db.save_task_replacing_current(&new).unwrap();
            old.progress = 73.0;
            db.save_task(&old).unwrap();
            assert_eq!(
                db.get_current_tasks_for_leaf("movie:550").unwrap()[0].id,
                new.id
            );
            let conn = db.pool.get().unwrap();
            let ownership: (Option<String>, i64) = conn
                .query_row(
                    "SELECT superseded_by,is_current FROM downloads WHERE id=?1",
                    [old.id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(ownership, (Some(new.id.to_string()), 0));
        }
    }

    #[test]
    fn concurrent_late_save_and_replacement_leave_replacement_current() {
        let file = NamedTempFile::new().unwrap();
        let db = Arc::new(Db::new(file.path()).unwrap());
        let old = identified_task("torrent", "movie", 42, None, None);
        let new = identified_task("fshare", "movie", 42, None, None);
        db.save_task_replacing_current(&old).unwrap();
        std::thread::scope(|scope| {
            let routine_db = db.clone();
            let routine = old.clone();
            scope.spawn(move || routine_db.save_task(&routine).unwrap());
            let replacement_db = db.clone();
            scope.spawn(move || replacement_db.save_task_replacing_current(&new).unwrap());
        });
        assert_eq!(
            db.get_current_tasks_for_leaf("movie:42").unwrap()[0]
                .download_source
                .as_deref(),
            Some("fshare")
        );
    }

    #[test]
    fn failed_replacement_rolls_back_supersession() {
        let file = NamedTempFile::new().unwrap();
        let db = Db::new(file.path()).unwrap();
        let old = identified_task("torrent", "movie", 99, None, None);
        let new = identified_task("fshare", "movie", 99, None, None);
        db.save_task_replacing_current(&old).unwrap();
        db.pool.get().unwrap().execute_batch(&format!(
            "CREATE TRIGGER reject_replacement BEFORE INSERT ON downloads WHEN NEW.id='{}' BEGIN SELECT RAISE(ABORT, 'rejected'); END;", new.id
        )).unwrap();
        assert!(db.save_task_replacing_current(&new).is_err());
        assert_eq!(
            db.get_current_tasks_for_leaf("movie:99").unwrap()[0].id,
            old.id
        );
    }

    #[test]
    fn concurrent_leaf_creation_has_one_current_winner() {
        let file = NamedTempFile::new().expect("temp db");
        let db = Arc::new(Db::new(file.path()).expect("fresh db"));
        let tasks = [
            identified_task("fshare", "movie", 42, None, None),
            identified_task("torrent", "movie", 42, None, None),
        ];
        std::thread::scope(|scope| {
            for task in &tasks {
                let db = db.clone();
                scope.spawn(move || {
                    db.save_task_replacing_current(task).unwrap();
                });
            }
        });
        assert_eq!(db.get_current_tasks_for_leaf("movie:42").unwrap().len(), 1);
        let conn = db.pool.get().expect("test DB connection");
        let history_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM downloads", [], |row| row.get(0))
            .expect("download history count");
        assert_eq!(history_count, 2);
    }

    fn task(filename: &str, source: &str) -> DownloadTask {
        let mut task = DownloadTask::new(
            "https://example.test/file".to_string(),
            filename.to_string(),
            if source == "torrent" {
                "qbittorrent"
            } else {
                "fshare"
            }
            .to_string(),
            "other".to_string(),
        );
        task.download_source = Some(source.to_string());
        if source == "torrent" {
            task.info_hash = Some("trackedhash".to_string());
            task.external_instance_id = Some("default".to_string());
            task.external_id = Some("trackedhash".to_string());
        }
        task
    }

    #[tokio::test]
    async fn canonical_batch_lookup_wins_over_legacy_and_uuid_batches() {
        let file = NamedTempFile::new().unwrap();
        let db = Db::new(file.path()).unwrap();
        for batch in ["legacy-uuid", "media:tv-tmdb:296206", "media:tv:468006"] {
            let mut task = task("lookup.mkv", "fshare");
            task.batch_id = Some(batch.to_string());
            task.tmdb_id = Some(296206);
            task.tvdb_id = Some(468006);
            task.media_type = Some("tv".to_string());
            db.save_task(&task).unwrap();
        }

        assert_eq!(
            db.get_batch_id_by_tmdb_id_async(296206)
                .await
                .unwrap()
                .as_deref(),
            Some("media:tv-tmdb:296206")
        );
        assert_eq!(
            db.get_batch_id_by_tvdb_id_async(468006)
                .await
                .unwrap()
                .as_deref(),
            Some("media:tv:468006")
        );
    }

    #[tokio::test]
    async fn batch_summaries_excludes_qbit_rows_without_flasharr_tasks() {
        let file = NamedTempFile::new().unwrap();
        let db = Db::new(file.path()).unwrap();

        db.save_task(&task("fshare.mkv", "fshare")).unwrap();
        db.save_task(&task("tracked.torrent", "torrent")).unwrap();
        db.upsert_qbit_torrent(
            "default",
            &QBittorrentTorrent {
                hash: "orphanhash".to_string(),
                name: "orphan.torrent".to_string(),
                state: "downloading".to_string(),
                progress: 0.5,
                added_on: 2,
                ..Default::default()
            },
        )
        .unwrap();
        db.upsert_qbit_torrent(
            "default",
            &QBittorrentTorrent {
                hash: "trackedhash".to_string(),
                name: "tracked duplicate".to_string(),
                state: "downloading".to_string(),
                progress: 0.5,
                added_on: 3,
                ..Default::default()
            },
        )
        .unwrap();

        let (_batches, standalone, rows, torrents, total) = db
            .get_batch_summaries_paginated_async(1, 10, None, Some("torrent"), "added", "desc")
            .await
            .unwrap();

        assert_eq!(total, 1);
        assert_eq!(rows.len(), 1);
        assert!(standalone.iter().any(|t| t.filename == "tracked.torrent"));
        assert!(torrents.is_empty());
        assert!(!torrents.iter().any(|t| t.hash == "trackedhash"));
    }

    #[tokio::test]
    async fn direct_source_filter_excludes_torrents() {
        let file = NamedTempFile::new().unwrap();
        let db = Db::new(file.path()).unwrap();

        db.save_task(&task("fshare.mkv", "fshare")).unwrap();
        db.save_task(&task("torbox.mkv", "torbox")).unwrap();
        db.save_task(&task("torrent.mkv", "torrent")).unwrap();

        let (downloads, total) = db
            .get_tasks_paginated_sorted_filtered_async(1, 10, "added", "desc", None, Some("direct"))
            .await
            .unwrap();
        let names = downloads
            .iter()
            .map(|t| t.filename.as_str())
            .collect::<Vec<_>>();
        assert_eq!(total, 2);
        assert!(names.contains(&"fshare.mkv"));
        assert!(names.contains(&"torbox.mkv"));
        assert!(!names.contains(&"torrent.mkv"));

        let counts = db
            .get_status_counts_filtered_async(Some("direct"))
            .await
            .unwrap();
        assert_eq!(counts.values().sum::<usize>(), 2);

        let (_batches, standalone, _rows, torrents, batch_total) = db
            .get_batch_summaries_paginated_async(1, 10, None, Some("direct"), "added", "desc")
            .await
            .unwrap();
        assert_eq!(batch_total, 2);
        assert_eq!(torrents.len(), 0);
        let standalone_names = standalone
            .iter()
            .map(|t| t.filename.as_str())
            .collect::<Vec<_>>();
        assert!(standalone_names.contains(&"fshare.mkv"));
        assert!(standalone_names.contains(&"torbox.mkv"));
        assert!(!standalone_names.contains(&"torrent.mkv"));
    }

    #[tokio::test]
    async fn direct_source_filter_excludes_torrent_children_from_mixed_batch() {
        let file = NamedTempFile::new().unwrap();
        let db = Db::new(file.path()).unwrap();

        let mut direct = task("show-e01.mkv", "fshare");
        direct.batch_id = Some("batch-1".to_string());
        direct.batch_name = Some("Show S01".to_string());
        db.save_task(&direct).unwrap();

        let mut torrent = task("show-e02.torrent", "torrent");
        torrent.batch_id = Some("batch-1".to_string());
        torrent.batch_name = Some("Show S01".to_string());
        db.save_task(&torrent).unwrap();

        let (batches, standalone, _rows, torrents, total) = db
            .get_batch_summaries_paginated_async(1, 10, None, Some("direct"), "added", "desc")
            .await
            .unwrap();

        assert_eq!(total, 1);
        assert!(standalone.is_empty());
        assert!(torrents.is_empty());
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].batch_id, "batch-1");
        assert_eq!(batches[0].total_items, 1);
        assert_eq!(batches[0].source, "fshare");

        let batch_items = db
            .get_tasks_by_batch_id_async("batch-1".to_string(), Some("direct"))
            .await
            .unwrap();
        assert_eq!(batch_items.len(), 1);
        assert_eq!(batch_items[0].filename, "show-e01.mkv");
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

/// Cached folder item for search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFolderItem {
    pub linkcode: String,
    pub name: String,
    pub title: String,
    pub category: String,
    pub label: String,
    pub parent_linkcode: String,
    pub fshare_url: String,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub is_series: bool,
    pub is_directory: bool,
    pub size: u64,
    pub quality: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmdb_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster_path: Option<String>,
}

/// TMDB mapping for a folder item
#[derive(Debug, Clone)]
pub struct FolderTmdbMapping {
    pub linkcode: String,
    pub tmdb_id: i64,
    pub media_type: String,
    pub poster_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderCacheSnapshotItem {
    pub name: String,
    pub parent_linkcode: String,
    pub size: u64,
    pub path: String,
    pub is_directory: bool,
}

fn qbit_row_matches(row: &QbitTorrentRow, query: &QbitTorrentListQuery) -> bool {
    if let Some(search) = query
        .search
        .as_ref()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
    {
        if !row.name.to_ascii_lowercase().contains(&search)
            && !row.hash.to_ascii_lowercase().contains(&search)
            && !row.tracker.to_ascii_lowercase().contains(&search)
        {
            return false;
        }
    }
    if let Some(status) = &query.status {
        if !status.is_empty() && !status.iter().any(|s| row.state.eq_ignore_ascii_case(s)) {
            return false;
        }
    }
    if let Some(categories) = &query.categories {
        if !categories.is_empty()
            && !categories
                .iter()
                .any(|s| row.category.eq_ignore_ascii_case(s))
        {
            return false;
        }
    }
    if let Some(tags) = &query.tags {
        if !tags.is_empty() {
            let row_tags = row.tags.to_ascii_lowercase();
            if !tags.iter().any(|tag| {
                row_tags
                    .split(',')
                    .any(|t| t.trim() == tag.to_ascii_lowercase())
            }) {
                return false;
            }
        }
    }
    if let Some(trackers) = &query.trackers {
        if !trackers.is_empty()
            && !trackers.iter().any(|s| {
                row.tracker
                    .to_ascii_lowercase()
                    .contains(&s.to_ascii_lowercase())
            })
        {
            return false;
        }
    }
    true
}

fn sort_qbit_rows(rows: &mut [QbitTorrentRow], sort: &str, order: &str) {
    rows.sort_by(|a, b| {
        let ord = match sort {
            "name" => a
                .name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase()),
            "size" => a.size.cmp(&b.size),
            "progress" => a
                .progress
                .partial_cmp(&b.progress)
                .unwrap_or(std::cmp::Ordering::Equal),
            "state" => a.state.cmp(&b.state),
            "ratio" => a
                .ratio
                .partial_cmp(&b.ratio)
                .unwrap_or(std::cmp::Ordering::Equal),
            "dlspeed" => a.dlspeed.cmp(&b.dlspeed),
            "upspeed" => a.upspeed.cmp(&b.upspeed),
            "category" => a.category.cmp(&b.category),
            "added_on" | _ => a.added_on.cmp(&b.added_on),
        };
        if order.eq_ignore_ascii_case("asc") {
            ord
        } else {
            ord.reverse()
        }
    });
}

#[cfg(test)]
mod folder_cache_graph_red_tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn item(code: &str, parent: &str, name: &str, directory: bool) -> CachedFolderItem {
        CachedFolderItem {
            linkcode: code.into(),
            name: name.into(),
            title: name.into(),
            category: "tv".into(),
            label: "source-a".into(),
            parent_linkcode: parent.into(),
            fshare_url: format!(
                "https://www.fshare.vn/{}/{}",
                if directory { "folder" } else { "file" },
                code
            ),
            year: None,
            season: None,
            episode: None,
            is_series: true,
            is_directory: directory,
            size: 0,
            quality: String::new(),
            path: format!("/{name}"),
            tmdb_id: None,
            media_type_hint: None,
            poster_path: None,
        }
    }

    #[test]
    fn graph_exact_parent_siblings_and_unbounded_cycle_safe_descendants() {
        let file = NamedTempFile::new().unwrap();
        let db = Db::new(file.path()).unwrap();
        let mut rows = vec![
            item("show", "", "Show", true),
            item("season", "show", "Season 1", true),
            item("quality", "season", "1080p", true),
            item("e01", "quality", "Show.S01E01.mkv", false),
            item("e02", "quality", "Show.S01E02.mkv", false),
        ];
        for n in 0..8 {
            let parent = if n == 0 {
                "e02".to_string()
            } else {
                format!("deep{}", n - 1)
            };
            rows.push(item(&format!("deep{n}"), &parent, "deep", true));
        }
        db.replace_folder_cache(&rows).unwrap();
        assert_eq!(
            db.get_folder_cache_by_linkcode("e01")
                .unwrap()
                .unwrap()
                .parent_linkcode,
            "quality"
        );
        assert_eq!(
            db.folder_cache_parent("e01").unwrap().unwrap().linkcode,
            "quality"
        );
        assert_eq!(
            db.folder_cache_siblings("e01")
                .unwrap()
                .iter()
                .map(|x| x.linkcode.as_str())
                .collect::<Vec<_>>(),
            vec!["e02"]
        );
        assert_eq!(
            db.folder_cache_children("season").unwrap()[0].linkcode,
            "quality"
        );
        assert_eq!(
            db.folder_cache_descendants("show").unwrap().len(),
            rows.len() - 1
        );
    }

    #[test]
    fn exhaustive_folder_search_has_no_top_200_truncation() {
        let file = NamedTempFile::new().unwrap();
        let db = Db::new(file.path()).unwrap();
        let rows = (0..251)
            .map(|n| {
                item(
                    &format!("d{n:03}"),
                    "root",
                    &format!("Show Season 1 Variant {n}"),
                    true,
                )
            })
            .collect::<Vec<_>>();
        db.replace_folder_cache(&rows).unwrap();
        assert_eq!(
            db.search_folder_cache_exhaustive("Show Season 1", true)
                .unwrap()
                .len(),
            251
        );
        assert_eq!(
            db.search_folder_cache("Show Season 1", 200).unwrap().len(),
            200
        );
    }

    #[test]
    fn migration_is_idempotent_and_preserves_legacy_fts_rows() {
        let file = NamedTempFile::new().unwrap();
        let conn = rusqlite::Connection::open(file.path()).unwrap();
        conn.execute_batch("CREATE VIRTUAL TABLE folder_cache USING fts5(linkcode,name,title,category,label,parent_linkcode,fshare_url UNINDEXED,year UNINDEXED,season UNINDEXED,episode UNINDEXED,is_series UNINDEXED,is_directory UNINDEXED,size UNINDEXED,quality UNINDEXED,path UNINDEXED); INSERT INTO folder_cache VALUES ('old','Old.S01E01.mkv','Old','tv','legacy','parent','https://www.fshare.vn/file/old','','1','1','1','0','1','1080p','/old');").unwrap();
        drop(conn);
        Db::new(file.path()).unwrap();
        let db = Db::new(file.path()).unwrap();
        assert_eq!(db.get_folder_cache_count().unwrap(), 1);
        assert_eq!(
            db.get_folder_cache_by_linkcode("old")
                .unwrap()
                .unwrap()
                .label,
            "legacy"
        );
        let conn = rusqlite::Connection::open(file.path()).unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM folder_cache_items", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}

impl Db {
    // ============================================================================
    // Folder Cache Operations
    // ============================================================================

    fn map_folder_row(row: &rusqlite::Row) -> rusqlite::Result<CachedFolderItem> {
        let text = |i| row.get::<_, String>(i);
        Ok(CachedFolderItem {
            linkcode: text(0)?,
            name: text(1)?,
            title: text(2)?,
            category: text(3)?,
            label: text(4)?,
            parent_linkcode: text(5)?,
            fshare_url: text(6)?,
            year: text(7)?.parse().ok(),
            season: text(8)?.parse().ok(),
            episode: text(9)?.parse().ok(),
            is_series: text(10)? == "1",
            is_directory: text(11)? == "1",
            size: text(12)?.parse().unwrap_or(0),
            quality: text(13)?,
            path: text(14)?,
            tmdb_id: None,
            media_type_hint: None,
            poster_path: None,
        })
    }

    fn insert_folder_media_index(
        tx: &rusqlite::Transaction<'_>,
        items: &[CachedFolderItem],
        generation: i64,
        root_linkcode: &str,
    ) -> Result<()> {
        let mut stmt = tx.prepare("INSERT OR REPLACE INTO folder_cache_media_index
            (linkcode,parent_linkcode,root_linkcode,normalized_title,media_type,season,episode,is_directory,parse_ambiguous,snapshot_generation,parser_version)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,1)")?;
        for item in items {
            let markers = item
                .name
                .to_ascii_uppercase()
                .as_bytes()
                .windows(2)
                .filter(|w| w[0] == b'E' && w[1].is_ascii_digit())
                .count();
            stmt.execute(params![
                item.linkcode,
                item.parent_linkcode,
                root_linkcode,
                item.title.trim().to_lowercase(),
                if item.is_series { "tv" } else { "movie" },
                item.season,
                item.episode,
                item.is_directory,
                markers > 1,
                generation
            ])?;
        }
        Ok(())
    }

    pub fn get_folder_cache_by_linkcode(&self, code: &str) -> Result<Option<CachedFolderItem>> {
        self.connection()?.query_row("SELECT linkcode,name,title,category,label,parent_linkcode,fshare_url,year,season,episode,is_series,is_directory,size,quality,path FROM folder_cache_items WHERE linkcode=?1", [code], Self::map_folder_row).optional()
    }

    pub fn folder_cache_children(&self, code: &str) -> Result<Vec<CachedFolderItem>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare("SELECT linkcode,name,title,category,label,parent_linkcode,fshare_url,year,season,episode,is_series,is_directory,size,quality,path FROM folder_cache_items WHERE parent_linkcode=?1 ORDER BY linkcode")?;
        let rows = stmt.query_map([code], Self::map_folder_row)?.collect();
        rows
    }

    pub fn folder_cache_parent(&self, code: &str) -> Result<Option<CachedFolderItem>> {
        let Some(item) = self.get_folder_cache_by_linkcode(code)? else {
            return Ok(None);
        };
        self.get_folder_cache_by_linkcode(&item.parent_linkcode)
    }

    pub fn folder_cache_siblings(&self, code: &str) -> Result<Vec<CachedFolderItem>> {
        let Some(item) = self.get_folder_cache_by_linkcode(code)? else {
            return Ok(Vec::new());
        };
        Ok(self
            .folder_cache_children(&item.parent_linkcode)?
            .into_iter()
            .filter(|x| x.linkcode != code)
            .collect())
    }

    pub fn folder_cache_descendants(&self, code: &str) -> Result<Vec<CachedFolderItem>> {
        self.folder_cache_descendants_batch(&[code.to_string()])
    }

    /// Expand all roots with one cycle-safe recursive query.
    pub fn folder_cache_descendants_batch(
        &self,
        roots: &[String],
    ) -> Result<Vec<CachedFolderItem>> {
        if roots.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.connection()?;
        let values = (1..=roots.len())
            .map(|i| format!("(?{i})"))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("WITH RECURSIVE roots(code) AS (VALUES {values}), descendants(linkcode) AS (
            SELECT fc.linkcode FROM folder_cache_items fc JOIN roots r ON fc.parent_linkcode=r.code
            UNION SELECT fc.linkcode FROM folder_cache_items fc JOIN descendants d ON fc.parent_linkcode=d.linkcode)
            SELECT fc.linkcode,fc.name,fc.title,fc.category,fc.label,fc.parent_linkcode,fc.fshare_url,
                   fc.year,COALESCE(CAST(mi.season AS TEXT),''),COALESCE(CAST(mi.episode AS TEXT),''),
                   fc.is_series,fc.is_directory,fc.size,fc.quality,fc.path
            FROM descendants d JOIN folder_cache_items fc ON fc.linkcode=d.linkcode
            JOIN folder_cache_media_index mi ON mi.linkcode=fc.linkcode ORDER BY fc.linkcode");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(
                rusqlite::params_from_iter(roots.iter()),
                Self::map_folder_row,
            )?
            .collect();
        rows
    }

    /// Clear the entire folder cache
    pub fn clear_folder_cache(&self) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("DELETE FROM folder_cache_items", [])?;
        Ok(())
    }

    /// Insert a batch of items into the folder cache within a transaction
    pub fn insert_folder_cache_batch(&self, items: &[CachedFolderItem]) -> Result<usize> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let tx = conn.transaction()?;

        let mut count = 0;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO folder_cache_items (
                    linkcode, name, title, category, label, parent_linkcode,
                    fshare_url, year, season, episode, is_series, is_directory,
                    size, quality, path
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            )?;

            for item in items {
                stmt.execute(params![
                    item.linkcode,
                    item.name,
                    item.title,
                    item.category,
                    item.label,
                    item.parent_linkcode,
                    item.fshare_url,
                    item.year.map(|y| y.to_string()).unwrap_or_default(),
                    item.season.map(|s| s.to_string()).unwrap_or_default(),
                    item.episode.map(|e| e.to_string()).unwrap_or_default(),
                    if item.is_series { "1" } else { "0" },
                    if item.is_directory { "1" } else { "0" },
                    item.size.to_string(),
                    item.quality,
                    item.path,
                ])?;
                count += 1;
            }
        }

        tx.commit()?;
        Ok(count)
    }

    /// Replace the folder cache atomically, preserving the previous snapshot on failure.
    pub fn replace_folder_cache(&self, items: &[CachedFolderItem]) -> Result<usize> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM folder_cache_items", [])?;
        tx.execute("DELETE FROM folder_cache_media_index", [])?;

        let mut stmt = tx.prepare(
            "INSERT INTO folder_cache_items (
                linkcode, name, title, category, label, parent_linkcode,
                fshare_url, year, season, episode, is_series, is_directory,
                size, quality, path
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        )?;
        for item in items {
            stmt.execute(params![
                item.linkcode,
                item.name,
                item.title,
                item.category,
                item.label,
                item.parent_linkcode,
                item.fshare_url,
                item.year.map(|y| y.to_string()).unwrap_or_default(),
                item.season.map(|s| s.to_string()).unwrap_or_default(),
                item.episode.map(|e| e.to_string()).unwrap_or_default(),
                if item.is_series { "1" } else { "0" },
                if item.is_directory { "1" } else { "0" },
                item.size.to_string(),
                item.quality,
                item.path,
            ])?;
        }
        drop(stmt);
        let generation = tx.query_row("SELECT COALESCE((SELECT value FROM folder_cache_meta WHERE key='sync_generation'),'0')", [], |r| r.get::<_, String>(0))?.parse().unwrap_or(0);
        Self::insert_folder_media_index(&tx, items, generation, "")?;
        tx.commit()?;
        Ok(items.len())
    }

    pub fn get_folder_cache_context(
        &self,
        root_linkcode: &str,
    ) -> Result<Option<(String, String)>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.query_row(
            "SELECT category, label FROM folder_cache
             WHERE linkcode = ?1 OR parent_linkcode = ?1
             LIMIT 1",
            params![root_linkcode],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
    }

    pub fn snapshot_folder_cache_subtree(
        &self,
        root_linkcode: &str,
    ) -> Result<std::collections::HashMap<String, FolderCacheSnapshotItem>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "WITH RECURSIVE descendants(linkcode) AS (
                SELECT linkcode FROM folder_cache_items WHERE parent_linkcode = ?1
                UNION
                SELECT fc.linkcode FROM folder_cache_items fc
                JOIN descendants d ON fc.parent_linkcode = d.linkcode
            )
            SELECT fc.linkcode, fc.name, fc.parent_linkcode, fc.size, fc.path, fc.is_directory
            FROM descendants d JOIN folder_cache_items fc ON fc.linkcode = d.linkcode",
        )?;
        let rows = stmt.query_map(params![root_linkcode], |row| {
            let size: String = row.get(3)?;
            let is_directory: String = row.get(5)?;
            Ok((
                row.get::<_, String>(0)?,
                FolderCacheSnapshotItem {
                    name: row.get(1)?,
                    parent_linkcode: row.get(2)?,
                    size: size.parse().unwrap_or(0),
                    path: row.get(4)?,
                    is_directory: is_directory == "1",
                },
            ))
        })?;
        rows.collect()
    }

    /// Replace one root-linkcode hierarchy without disturbing other sources.
    pub fn replace_folder_cache_subtree(
        &self,
        root_linkcode: &str,
        items: &[CachedFolderItem],
        generation: i64,
    ) -> Result<usize> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let tx = conn.transaction()?;
        let descendants_sql = "WITH RECURSIVE descendants(linkcode) AS (
                SELECT linkcode FROM folder_cache_items WHERE parent_linkcode = ?1
                UNION
                SELECT fc.linkcode FROM folder_cache_items fc
                JOIN descendants d ON fc.parent_linkcode = d.linkcode
            )
            SELECT linkcode FROM descendants";
        let old_codes = {
            let mut stmt = tx.prepare(descendants_sql)?;
            let rows = stmt.query_map(params![root_linkcode], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<std::collections::HashSet<_>, _>>()?
        };

        for linkcode in &old_codes {
            tx.execute(
                "DELETE FROM folder_cache_media_index WHERE linkcode = ?1",
                params![linkcode],
            )?;
            tx.execute(
                "DELETE FROM folder_cache_items WHERE linkcode = ?1",
                params![linkcode],
            )?;
        }

        {
            let mut stmt = tx.prepare(
                "INSERT INTO folder_cache_items (
                    linkcode, name, title, category, label, parent_linkcode,
                    fshare_url, year, season, episode, is_series, is_directory,
                    size, quality, path, snapshot_generation
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            )?;
            for item in items {
                stmt.execute(params![
                    item.linkcode,
                    item.name,
                    item.title,
                    item.category,
                    item.label,
                    item.parent_linkcode,
                    item.fshare_url,
                    item.year.map(|y| y.to_string()).unwrap_or_default(),
                    item.season.map(|s| s.to_string()).unwrap_or_default(),
                    item.episode.map(|e| e.to_string()).unwrap_or_default(),
                    if item.is_series { "1" } else { "0" },
                    if item.is_directory { "1" } else { "0" },
                    item.size.to_string(),
                    item.quality,
                    item.path,
                    generation,
                ])?;
            }
        }

        let new_codes: std::collections::HashSet<_> =
            items.iter().map(|item| item.linkcode.as_str()).collect();
        let mut delete_mapping = tx.prepare("DELETE FROM folder_tmdb_map WHERE linkcode = ?1")?;
        for linkcode in old_codes {
            if !new_codes.contains(linkcode.as_str()) {
                delete_mapping.execute(params![linkcode])?;
            }
        }
        drop(delete_mapping);
        Self::insert_folder_media_index(&tx, items, generation, root_linkcode)?;
        tx.commit()?;
        Ok(items.len())
    }

    /// Search the folder cache using FTS5 full-text search (LEFT JOINs TMDB map)
    pub fn search_folder_cache(&self, query: &str, limit: u32) -> Result<Vec<CachedFolderItem>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        // Build literal FTS5 prefix terms. Raw user punctuation can otherwise be
        // interpreted as MATCH syntax and turn a valid title into a query error.
        let fts_query = query
            .split_whitespace()
            .map(|word| {
                word.chars()
                    .filter(|ch| ch.is_alphanumeric())
                    .collect::<String>()
            })
            .filter(|word| !word.is_empty())
            .map(|word| format!("\"{word}\"*"))
            .collect::<Vec<_>>()
            .join(" ");
        if fts_query.is_empty() {
            return Ok(Vec::new());
        }

        let mut stmt = conn.prepare(
            "SELECT fc.linkcode, fc.name, fc.title, fc.category, fc.label, fc.parent_linkcode,
                    fc.fshare_url, fc.year, fc.season, fc.episode, fc.is_series, fc.is_directory,
                    fc.size, fc.quality, fc.path,
                    COALESCE(tm.tmdb_id, ptm.tmdb_id) as tmdb_id,
                    COALESCE(tm.media_type, ptm.media_type) as media_type,
                    COALESCE(tm.poster_path, ptm.poster_path) as poster_path
             FROM folder_cache JOIN folder_cache_items fc ON fc.id = folder_cache.rowid
             LEFT JOIN folder_tmdb_map tm ON fc.linkcode = tm.linkcode
             LEFT JOIN folder_tmdb_map ptm ON fc.parent_linkcode = ptm.linkcode
             WHERE folder_cache MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![fts_query, limit], |row| {
            let year_str: String = row.get(7)?;
            let season_str: String = row.get(8)?;
            let episode_str: String = row.get(9)?;
            let is_series_str: String = row.get(10)?;
            let is_dir_str: String = row.get(11)?;
            let size_str: String = row.get(12)?;
            let tmdb_id: Option<i64> = row.get(15)?;
            let media_type_hint: Option<String> = row.get(16)?;
            let poster_path: Option<String> = row.get(17)?;

            Ok(CachedFolderItem {
                linkcode: row.get(0)?,
                name: row.get(1)?,
                title: row.get(2)?,
                category: row.get(3)?,
                label: row.get(4)?,
                parent_linkcode: row.get(5)?,
                fshare_url: row.get(6)?,
                year: year_str.parse().ok(),
                season: season_str.parse().ok(),
                episode: episode_str.parse().ok(),
                is_series: is_series_str == "1",
                is_directory: is_dir_str == "1",
                size: size_str.parse().unwrap_or(0),
                quality: row.get(13)?,
                path: row.get(14)?,
                tmdb_id,
                media_type_hint: media_type_hint.filter(|s| !s.is_empty()),
                poster_path: poster_path.filter(|s| !s.is_empty()),
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            if let Ok(item) = row {
                results.push(item);
            }
        }
        Ok(results)
    }

    /// Exhaustive FTS search for the season planner. Unlike the public `/search`, no UI cap.
    /// `directories=true` returns only directory hits; false returns only file hits.
    pub fn search_folder_cache_exhaustive(
        &self,
        query: &str,
        directories: bool,
    ) -> Result<Vec<CachedFolderItem>> {
        let terms = query
            .split_whitespace()
            .map(|word| {
                word.chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
            })
            .filter(|word| !word.is_empty())
            .map(|word| format!("\"{word}\"*"))
            .collect::<Vec<_>>()
            .join(" ");
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT fc.linkcode,fc.name,fc.title,fc.category,fc.label,fc.parent_linkcode,
                    fc.fshare_url,fc.year,fc.season,fc.episode,fc.is_series,fc.is_directory,
                    fc.size,fc.quality,fc.path
             FROM folder_cache JOIN folder_cache_items fc ON fc.id=folder_cache.rowid
             WHERE folder_cache MATCH ?1 AND fc.is_directory=?2
             ORDER BY rank,fc.linkcode",
        )?;
        let rows = stmt
            .query_map(
                params![terms, if directories { "1" } else { "0" }],
                Self::map_folder_row,
            )?
            .collect();
        rows
    }

    /// Exhaustive folder discovery constrained by explicit canonical identity.
    /// Unmapped aggregate folders remain eligible; explicitly different folders are pruned.
    pub fn search_folder_cache_folders_for_tmdb(
        &self,
        query: &str,
        tmdb_id: i64,
    ) -> Result<Vec<CachedFolderItem>> {
        let terms = query
            .split_whitespace()
            .map(|word| {
                word.chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
            })
            .filter(|word| !word.is_empty())
            .map(|word| format!("\"{word}\"*"))
            .collect::<Vec<_>>()
            .join(" ");
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT fc.linkcode,fc.name,fc.title,fc.category,fc.label,fc.parent_linkcode,
                    fc.fshare_url,fc.year,fc.season,fc.episode,fc.is_series,fc.is_directory,
                    fc.size,fc.quality,fc.path
             FROM folder_cache
             JOIN folder_cache_items fc ON fc.id=folder_cache.rowid
             LEFT JOIN folder_tmdb_map own_tm ON own_tm.linkcode=fc.linkcode
             WHERE folder_cache MATCH ?1 AND fc.is_directory='1'
               AND (own_tm.tmdb_id IS NULL OR own_tm.tmdb_id=?2)
             ORDER BY rank,fc.linkcode",
        )?;
        let rows = stmt
            .query_map(params![terms, tmdb_id], Self::map_folder_row)?
            .collect();
        rows
    }

    /// Async search wrapper
    pub async fn search_folder_cache_async(
        &self,
        query: String,
        limit: u32,
    ) -> Result<Vec<CachedFolderItem>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let db = Db { pool };
            db.search_folder_cache(&query, limit)
        })
        .await
        .unwrap()
    }

    /// Get folder cache metadata value
    pub fn get_folder_cache_meta(&self, key: &str) -> Result<Option<String>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.query_row(
            "SELECT value FROM folder_cache_meta WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
    }

    /// Set folder cache metadata value
    pub fn set_folder_cache_meta(&self, key: &str, value: &str) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO folder_cache_meta (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_all_folder_modified_times(&self) -> Result<std::collections::HashMap<String, i64>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare("SELECT linkcode, modified FROM folder_modified_meta")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;

        let mut map = std::collections::HashMap::new();
        for row in rows {
            let (linkcode, modified) = row?;
            map.insert(linkcode, modified);
        }
        Ok(map)
    }

    pub fn replace_folder_modified_times(
        &self,
        items: &std::collections::HashMap<String, i64>,
    ) -> Result<()> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM folder_modified_meta", [])?;
        {
            let mut stmt = tx
                .prepare("INSERT INTO folder_modified_meta (linkcode, modified) VALUES (?1, ?2)")?;
            for (linkcode, modified) in items {
                stmt.execute(params![linkcode, modified])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_folder_cache_items_grouped_by_label(
        &self,
    ) -> Result<std::collections::HashMap<String, Vec<CachedFolderItem>>> {
        let items = self.search_folder_cache_all()?;
        let mut grouped = std::collections::HashMap::new();
        for item in items {
            grouped
                .entry(item.label.clone())
                .or_insert_with(Vec::new)
                .push(item);
        }
        Ok(grouped)
    }

    fn search_folder_cache_all(&self) -> Result<Vec<CachedFolderItem>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let mut stmt = conn.prepare(
            "SELECT fc.linkcode, fc.name, fc.title, fc.category, fc.label, fc.parent_linkcode,
                    fc.fshare_url, fc.year, fc.season, fc.episode, fc.is_series, fc.is_directory,
                    fc.size, fc.quality, fc.path,
                    COALESCE(tm.tmdb_id, ptm.tmdb_id) as tmdb_id,
                    COALESCE(tm.media_type, ptm.media_type) as media_type,
                    COALESCE(tm.poster_path, ptm.poster_path) as poster_path
             FROM folder_cache_items fc
             LEFT JOIN folder_tmdb_map tm ON fc.linkcode = tm.linkcode
             LEFT JOIN folder_tmdb_map ptm ON fc.parent_linkcode = ptm.linkcode
             WHERE fc.is_directory = '0'",
        )?;

        let rows = stmt.query_map([], |row| {
            let year_str: String = row.get(7)?;
            let season_str: String = row.get(8)?;
            let episode_str: String = row.get(9)?;
            let is_series_str: String = row.get(10)?;
            let is_dir_str: String = row.get(11)?;
            let size_str: String = row.get(12)?;
            let tmdb_id: Option<i64> = row.get(15)?;
            let media_type_hint: Option<String> = row.get(16)?;
            let poster_path: Option<String> = row.get(17)?;

            Ok(CachedFolderItem {
                linkcode: row.get(0)?,
                name: row.get(1)?,
                title: row.get(2)?,
                category: row.get(3)?,
                label: row.get(4)?,
                parent_linkcode: row.get(5)?,
                fshare_url: row.get(6)?,
                year: year_str.parse().ok(),
                season: season_str.parse().ok(),
                episode: episode_str.parse().ok(),
                is_series: is_series_str == "1",
                is_directory: is_dir_str == "1",
                size: size_str.parse().unwrap_or(0),
                quality: row.get(13)?,
                path: row.get(14)?,
                tmdb_id,
                media_type_hint: media_type_hint.filter(|s| !s.is_empty()),
                poster_path: poster_path.filter(|s| !s.is_empty()),
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Get total count of items in folder cache
    pub fn get_folder_cache_count(&self) -> Result<u64> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.query_row("SELECT COUNT(*) FROM folder_cache_items", [], |row| {
            row.get(0)
        })
    }

    pub fn get_folder_cache_counts(&self) -> Result<(u64, u64)> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN is_directory = '1' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN is_directory = '0' THEN 1 ELSE 0 END), 0)
             FROM folder_cache_items",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
    }

    /// Snapshot file rows from the folder cache for pre-sync diffing.
    pub fn snapshot_folder_cache_files(
        &self,
    ) -> Result<std::collections::HashMap<String, FolderCacheSnapshotItem>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn.prepare(
            "SELECT linkcode, name, parent_linkcode, size, path, is_directory
             FROM folder_cache
             WHERE is_directory = '0'",
        )?;
        let rows = stmt.query_map([], |row| {
            let size_str: String = row.get(3)?;
            let is_directory_str: String = row.get(5)?;
            Ok((
                row.get::<_, String>(0)?,
                FolderCacheSnapshotItem {
                    name: row.get(1)?,
                    parent_linkcode: row.get(2)?,
                    size: size_str.parse().unwrap_or(0),
                    path: row.get(4)?,
                    is_directory: is_directory_str == "1",
                },
            ))
        })?;

        let mut snapshot = std::collections::HashMap::new();
        for row in rows {
            let (linkcode, item) = row?;
            snapshot.insert(linkcode, item);
        }
        Ok(snapshot)
    }

    // ============================================================================
    // Folder TMDB Mapping Operations
    // ============================================================================

    pub fn folder_cache_folders_by_tmdb_id(
        &self,
        tmdb_id: i64,
        media_type: &str,
    ) -> Result<Vec<CachedFolderItem>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT folder.linkcode,folder.name,folder.title,folder.category,folder.label,
                    folder.parent_linkcode,folder.fshare_url,folder.year,folder.season,folder.episode,
                    folder.is_series,folder.is_directory,folder.size,folder.quality,folder.path
             FROM folder_tmdb_map tm
             JOIN folder_cache_items mapped ON mapped.linkcode = tm.linkcode
             JOIN folder_cache_items folder
               ON folder.linkcode = CASE
                    WHEN mapped.is_directory = '1' THEN mapped.linkcode
                    ELSE mapped.parent_linkcode
                  END
             WHERE tm.tmdb_id = ?1 AND tm.media_type = ?2 AND folder.is_directory = '1'
             ORDER BY folder.linkcode",
        )?;
        let rows = stmt
            .query_map(params![tmdb_id, media_type], Self::map_folder_row)?
            .collect();
        rows
    }

    /// Check if a linkcode already has a TMDB mapping
    pub fn has_folder_tmdb_mapping(&self, linkcode: &str) -> Result<bool> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM folder_tmdb_map WHERE linkcode = ?1",
            params![linkcode],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Set TMDB mapping for a linkcode
    pub fn set_folder_tmdb_mapping(
        &self,
        linkcode: &str,
        tmdb_id: i64,
        media_type: &str,
        poster_path: &str,
    ) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO folder_tmdb_map (linkcode, tmdb_id, media_type, poster_path, mapped_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![linkcode, tmdb_id, media_type, poster_path, chrono::Utc::now().timestamp().to_string()],
        )?;
        Ok(())
    }

    /// Batch set TMDB mappings
    pub fn set_folder_tmdb_mappings_batch(
        &self,
        mappings: &[(String, i64, String, String)],
    ) -> Result<usize> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().timestamp().to_string();
        let mut count = 0;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO folder_tmdb_map (linkcode, tmdb_id, media_type, poster_path, mapped_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)"
            )?;
            for (lc, tid, mt, pp) in mappings {
                stmt.execute(params![lc, tid, mt, pp, now])?;
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Return negative TMDB cache keys newer than `checked_after`.
    pub fn get_folder_tmdb_negative_keys(
        &self,
        checked_after: i64,
    ) -> Result<std::collections::HashSet<String>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt = conn
            .prepare("SELECT cache_key FROM folder_tmdb_negative_cache WHERE checked_at >= ?1")?;
        let rows = stmt.query_map(params![checked_after], |row| row.get(0))?;
        let mut keys = std::collections::HashSet::new();
        for row in rows {
            keys.insert(row?);
        }
        Ok(keys)
    }

    /// Record unsuccessful TMDB lookups so recurring syncs do not repeat them.
    pub fn set_folder_tmdb_negative_keys(&self, keys: &[String]) -> Result<usize> {
        if keys.is_empty() {
            return Ok(0);
        }
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let tx = conn.transaction()?;
        let checked_at = chrono::Utc::now().timestamp();
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO folder_tmdb_negative_cache (cache_key, checked_at) VALUES (?1, ?2)",
        )?;
        for key in keys {
            stmt.execute(params![key, checked_at])?;
        }
        drop(stmt);
        tx.commit()?;
        Ok(keys.len())
    }

    /// Get all existing TMDB mappings as a HashMap for fast lookup
    pub fn get_all_folder_tmdb_mappings(
        &self,
    ) -> Result<std::collections::HashMap<String, FolderTmdbMapping>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut stmt =
            conn.prepare("SELECT linkcode, tmdb_id, media_type, poster_path FROM folder_tmdb_map")?;
        let rows = stmt.query_map([], |row| {
            Ok(FolderTmdbMapping {
                linkcode: row.get(0)?,
                tmdb_id: row.get(1)?,
                media_type: row.get(2)?,
                poster_path: row.get(3)?,
            })
        })?;
        let mut map = std::collections::HashMap::new();
        for row in rows {
            if let Ok(m) = row {
                map.insert(m.linkcode.clone(), m);
            }
        }
        Ok(map)
    }
}

#[cfg(test)]
mod canonical_identity_tests {
    use super::*;
    use crate::downloader::config::DownloadConfig;
    use crate::downloader::DownloadOrchestrator;
    use crate::hosts::registry::HostRegistry;
    use crate::torrent::qbittorrent::QBittorrentConfig;
    use tempfile::NamedTempFile;

    #[test]
    fn websocket_snapshot_is_uncapped_while_rest_stays_capped() {
        let file = NamedTempFile::new().expect("temp db");
        let db = Db::new(file.path()).expect("database");
        let conn = db.pool.get().unwrap();
        conn.execute("INSERT INTO qbit_instances (id,name,url,username,password,enabled,poll_interval_secs,created_at,updated_at) VALUES ('q','q','http://q','','',1,5,datetime('now'),datetime('now'))", []).unwrap();
        for i in 0..2001 {
            conn.execute("INSERT INTO qbit_torrents (instance_id,hash,name,state,updated_at) VALUES ('q',?1,'x','stalledDL',datetime('now'))", [format!("{i:040x}")]).unwrap();
        }
        drop(conn);
        assert_eq!(db.qbit_snapshot_full().unwrap()[0].1.len(), 2001);
        let (page, total) = db
            .list_qbit_torrents(
                "q",
                &QbitTorrentListQuery {
                    limit: 3000,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!((page.len(), total), (2000, 2001));
    }

    #[tokio::test]
    async fn orchestrator_writes_only_canonical_provider_identity() {
        let file = NamedTempFile::new().expect("temp db");
        let db = Arc::new(Db::new(file.path()).expect("database"));
        let orchestrator = DownloadOrchestrator::new(
            DownloadConfig::default(),
            Arc::new(HostRegistry::new()),
            Some(Arc::clone(&db)),
            None,
            None,
            QBittorrentConfig::disabled(),
            None,
        );
        let code = "SAMEID";

        orchestrator
            .add_download_with_metadata(
                format!("https://www.fshare.vn/file/{code}"),
                Some("fshare.mkv".into()),
                "fshare".into(),
                "other".into(),
                None,
                None,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("add fshare task");
        orchestrator
            .add_download_with_metadata(
                "https://torbox.app/file/SAMEID".into(),
                Some("torbox.mkv".into()),
                "torbox".into(),
                "other".into(),
                None,
                None,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(code.into()),
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("same item id under torbox must not collide");

        let conn = db.pool.get().expect("connection");
        let identities = conn
            .prepare(
                "SELECT provider_id, provider_item_id, fshare_code FROM downloads ORDER BY host",
            )
            .expect("prepare identities")
            .query_map([], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .expect("query identities")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect identities");

        assert_eq!(
            identities,
            vec![
                (Some("fshare".into()), Some(code.into()), None),
                (Some("torbox".into()), Some(code.into()), None),
            ]
        );
    }

    #[tokio::test]
    async fn provider_identity_migration_is_additive_idempotent_and_isolated() {
        let file = NamedTempFile::new().expect("temp db");
        {
            let conn = rusqlite::Connection::open(file.path()).expect("open legacy db");
            conn.execute_batch(
                "CREATE TABLE downloads (
                    id TEXT PRIMARY KEY,
                    url TEXT NOT NULL,
                    original_url TEXT NOT NULL,
                    filename TEXT NOT NULL,
                    destination TEXT NOT NULL,
                    state TEXT NOT NULL,
                    progress REAL NOT NULL,
                    size INTEGER NOT NULL,
                    host TEXT NOT NULL,
                    category TEXT NOT NULL,
                    priority INTEGER NOT NULL,
                    segments INTEGER NOT NULL,
                    retry_count INTEGER NOT NULL,
                    created_at TEXT NOT NULL,
                    fshare_code TEXT
                );
                INSERT INTO downloads VALUES (
                    'legacy-fshare', 'https://www.fshare.vn/file/SAMEID',
                    'https://www.fshare.vn/file/SAMEID', 'legacy.mkv', '/downloads',
                    'QUEUED', 0, 0, 'fshare', 'other', 0, 4, 0,
                    '2026-07-16T00:00:00Z', 'SAMEID'
                );",
            )
            .expect("legacy schema");
        }

        let db = Db::new(file.path()).expect("migrate legacy db");
        let conn = db.pool.get().expect("connection");
        let migrated: (Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT provider_id, provider_item_id, fshare_code FROM downloads WHERE id = 'legacy-fshare'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("migrated row");
        assert_eq!(
            migrated,
            (
                Some("fshare".into()),
                Some("SAMEID".into()),
                Some("SAMEID".into())
            )
        );
        drop(conn);

        let reopened = Db::new(file.path()).expect("idempotent reopen");
        {
            let conn = reopened.pool.get().expect("connection");
            conn.execute(
                "INSERT INTO downloads (
                    id, url, original_url, filename, destination, state, progress, size,
                    host, category, priority, segments, retry_count, created_at,
                    provider_id, provider_item_id
                ) VALUES (
                    'torbox-same', 'torbox:SAMEID', 'torbox:SAMEID', 'torrent.mkv', '/downloads',
                    'QUEUED', 0, 0, 'torbox', 'other', 0, 4, 0,
                    '2026-07-16T00:00:00Z', 'torbox', 'SAMEID'
                )",
                [],
            )
            .expect("canonical row");
        }

        assert_eq!(
            reopened
                .find_task_by_provider_item_async("fshare", "SAMEID")
                .await
                .expect("fshare lookup"),
            Some(("legacy-fshare".into(), "QUEUED".into()))
        );
        assert_eq!(
            reopened
                .find_task_by_provider_item_async("torbox", "SAMEID")
                .await
                .expect("torbox lookup"),
            Some(("torbox-same".into(), "QUEUED".into()))
        );
    }

    fn legacy_fixture() -> NamedTempFile {
        let file = NamedTempFile::new().expect("temp db");
        let conn = rusqlite::Connection::open(file.path()).expect("open legacy db");
        conn.execute_batch(
            "CREATE TABLE media_items (
                tmdb_id INTEGER PRIMARY KEY, media_type TEXT NOT NULL, title TEXT NOT NULL,
                original_title TEXT, year INTEGER, overview TEXT, poster_path TEXT, backdrop_path TEXT,
                genres TEXT, runtime INTEGER, total_seasons INTEGER, arr_id INTEGER, arr_type TEXT,
                arr_path TEXT, arr_monitored INTEGER DEFAULT 0, arr_status TEXT,
                arr_quality_profile_id INTEGER, arr_has_file INTEGER DEFAULT 0, arr_size_on_disk INTEGER DEFAULT 0,
                tvdb_id INTEGER, imdb_id TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, arr_synced_at TEXT
            );
            CREATE TABLE media_episodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tmdb_id INTEGER NOT NULL REFERENCES media_items(tmdb_id),
                season_number INTEGER NOT NULL, episode_number INTEGER NOT NULL,
                title TEXT, overview TEXT, air_date TEXT, arr_episode_id INTEGER,
                arr_has_file INTEGER DEFAULT 0, arr_monitored INTEGER DEFAULT 1,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                UNIQUE(tmdb_id, season_number, episode_number)
            );
            INSERT INTO media_items(tmdb_id,media_type,title,total_seasons,tvdb_id,created_at,updated_at)
            VALUES (12609,'tv','Dragon Ball',6,76666,'2026-01-01','2026-01-01');
            INSERT INTO media_episodes(tmdb_id,season_number,episode_number,title,created_at,updated_at)
            VALUES (12609,2,1,'Goku Rival','2026-01-01','2026-01-01');
            INSERT INTO media_items(tmdb_id,media_type,title,year,created_at,updated_at)
            VALUES (19995,'movie','Avatar',2009,'2026-01-01','2026-01-01');"
        ).expect("seed legacy schema");
        file
    }

    #[test]
    fn legacy_tmdb_pk_migrates_to_canonical_media_key_and_preserves_episodes() {
        let file = legacy_fixture();
        let db = Db::new(file.path()).expect("migrate db");
        let conn = db.pool.get().expect("conn");

        let pk: String = conn
            .query_row(
                "SELECT name FROM pragma_table_info('media_items') WHERE pk=1",
                [],
                |r| r.get(0),
            )
            .expect("pk");
        assert_eq!(pk, "media_key");

        let dragon: (String, i64, i32, String) = conn
            .query_row(
                "SELECT media_key, tmdb_id, tvdb_id, media_type FROM media_items WHERE title='Dragon Ball'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .expect("dragon row");
        assert_eq!(
            dragon,
            ("tv:76666".to_string(), 12609, 76666, "tv".to_string())
        );

        let movie_key: String = conn
            .query_row(
                "SELECT media_key FROM media_items WHERE title='Avatar'",
                [],
                |r| r.get(0),
            )
            .expect("movie row");
        assert_eq!(movie_key, "movie:19995");

        let episode_key: String = conn
            .query_row("SELECT media_key FROM media_episodes WHERE tmdb_id=12609 AND season_number=2 AND episode_number=1", [], |r| r.get(0))
            .expect("episode row");
        assert_eq!(episode_key, "tv:76666");
    }

    #[test]
    fn media_item_upsert_does_not_cascade_delete_existing_episodes() {
        let file = legacy_fixture();
        let db = Db::new(file.path()).expect("migrate db");
        let mut item = MediaItem::new(12609, "tv", "Dragon Ball");
        item.media_key = "tv:76666".to_string();
        item.tvdb_id = Some(76666);
        item.total_seasons = Some(6);
        db.upsert_media_item(&item)
            .expect("upsert canonical tv item");

        let conn = db.pool.get().expect("conn");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM media_episodes WHERE media_key='tv:76666'",
                [],
                |r| r.get(0),
            )
            .expect("episode count");
        assert_eq!(count, 1);
    }

    #[test]
    fn save_task_persists_tvdb_id_and_canonical_tv_info_path() {
        let file = NamedTempFile::new().expect("temp db");
        let db = Db::new(file.path()).expect("fresh db");
        let mut task = DownloadTask::new(
            "https://example.invalid/file.mkv".to_string(),
            "Dragon Ball - S02E01.mkv".to_string(),
            "fshare".to_string(),
            "tv".to_string(),
        );
        task.media_type = Some("tv".to_string());
        task.tmdb_id = Some(12609);
        task.tvdb_id = Some(76666);
        task.info_path = Some("/tv/76666".to_string());
        db.save_task(&task).expect("save tv task");

        let conn = db.pool.get().expect("conn");
        let row: (String, i64, i64, String) = conn
            .query_row(
                "SELECT media_type, tmdb_id, tvdb_id, info_path FROM downloads WHERE id=?1",
                [task.id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .expect("download row");
        assert_eq!(
            row,
            ("tv".to_string(), 12609, 76666, "/tv/76666".to_string())
        );
    }

    fn seed_qbit_instance(db: &Db, id: &str) {
        db.pool.get().unwrap().execute("INSERT INTO qbit_instances(id,name,url,enabled,last_rid) VALUES(?1,?1,'http://localhost',1,1)", [id]).unwrap();
    }
    fn qbit_torrent(hash: &str, name: &str) -> crate::torrent::qbittorrent::QBittorrentTorrent {
        crate::torrent::qbittorrent::QBittorrentTorrent {
            hash: hash.into(),
            name: name.into(),
            ..Default::default()
        }
    }
    #[test]
    fn qbit_full_sync_replaces_only_target_instance_atomically() {
        let file = NamedTempFile::new().unwrap();
        let db = Db::new(file.path()).unwrap();
        seed_qbit_instance(&db, "a");
        seed_qbit_instance(&db, "b");
        db.upsert_qbit_torrent("a", &qbit_torrent("old-a", "old"))
            .unwrap();
        db.upsert_qbit_torrent("b", &qbit_torrent("old-b", "other"))
            .unwrap();
        let applied = db
            .apply_qbit_sync(
                "a",
                2,
                true,
                &[qbit_torrent("new-a", "new")],
                &[],
                None,
                None,
            )
            .unwrap();
        assert_eq!(applied.replaced, vec!["old-a"]);
        assert!(db.get_qbit_torrent("a", "old-a").unwrap().is_none());
        assert!(db.get_qbit_torrent("a", "new-a").unwrap().is_some());
        assert!(db.get_qbit_torrent("b", "old-b").unwrap().is_some());
        let instance = db.get_qbit_instance("a").unwrap().unwrap();
        assert_eq!(
            (instance.last_rid, instance.sync_health.as_str()),
            (2, "healthy")
        );
        assert!(instance.last_sync_at.is_some());
    }
    #[test]
    fn qbit_delta_failure_and_sanitized_error_preserve_authority() {
        let file = NamedTempFile::new().unwrap();
        let db = Db::new(file.path()).unwrap();
        seed_qbit_instance(&db, "a");
        db.upsert_qbit_torrent("a", &qbit_torrent("remove", "old"))
            .unwrap();
        db.apply_qbit_sync(
            "a",
            2,
            false,
            &[qbit_torrent("keep", "v1")],
            &["remove".into()],
            None,
            None,
        )
        .unwrap();
        assert!(db.get_qbit_torrent("a", "remove").unwrap().is_none());
        let conn = db.pool.get().unwrap();
        conn.execute_batch("CREATE TRIGGER fail_qbit BEFORE INSERT ON qbit_torrents WHEN NEW.name='FAIL' BEGIN SELECT RAISE(FAIL,'injected'); END;").unwrap();
        drop(conn);
        assert!(db
            .apply_qbit_sync(
                "a",
                3,
                false,
                &[qbit_torrent("keep", "v2"), qbit_torrent("bad", "FAIL")],
                &[],
                None,
                None
            )
            .is_err());
        assert_eq!(
            db.get_qbit_torrent("a", "keep").unwrap().unwrap().name,
            "v1"
        );
        assert_eq!(db.get_qbit_instance("a").unwrap().unwrap().last_rid, 2);
        db.mark_qbit_sync_error(
            "a",
            "stale",
            "GET https://user:pass@host/x?apikey=SECRET&sid=ABC token=XYZ",
        )
        .unwrap();
        let instance = db.get_qbit_instance("a").unwrap().unwrap();
        assert_eq!(
            (instance.last_rid, instance.sync_health.as_str()),
            (2, "stale")
        );
        let error = instance.last_error.unwrap().to_ascii_lowercase();
        for secret in ["https://", "secret", "abc", "xyz", "apikey", "sid", "token"] {
            assert!(!error.contains(secret));
        }
    }
}
