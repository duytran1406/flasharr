//! Download Orchestrator
//!
//! Coordinates between TaskManager and DownloadEngine.
//! Handles URL resolution, retry logic, and progress broadcasting.

use std::sync::Arc;
use std::time::Duration;
use std::path::PathBuf;
use tokio::sync::{broadcast, Mutex, Notify, RwLock};
use tokio::task::JoinHandle;
use chrono::Utc;

use super::config::DownloadConfig;
use super::stats::EngineStats;
use super::engine_simple::SimpleDownloadEngine;
use super::manager::DownloadTaskManager;
use super::task::{DownloadTask, DownloadState, UrlMetadata};
use super::progress::ProgressUpdate;
use crate::hosts::registry::HostRegistry;
use crate::db::Db;

/// TMDB metadata for organizing downloads into folder structures
#[derive(Debug, Clone)]
pub struct TmdbDownloadMetadata {
    /// TMDB ID
    pub tmdb_id: Option<i64>,
    /// Media type: "movie" or "tv"
    pub media_type: Option<String>,
    /// Movie/Show title
    pub title: Option<String>,
    /// Release year
    pub year: Option<String>,
    /// Collection name (for movies in a collection)
    pub collection_name: Option<String>,
    /// Season number (for TV)
    pub season: Option<i32>,
    /// Episode number (for TV)
    pub episode: Option<i32>,
}

/// Download orchestrator - coordinates all download operations
pub struct DownloadOrchestrator {
    /// Task manager (Single Source of Truth)
    task_manager: Arc<DownloadTaskManager>,
    
    /// HTTP download engine (wrapped for dynamic updates)
    download_engine: Arc<RwLock<Arc<SimpleDownloadEngine>>>,
    
    /// Host registry for URL resolution
    host_registry: Arc<HostRegistry>,
    
    /// Database connection
    db: Option<Arc<Db>>,
    
    /// Progress broadcast channel
    progress_tx: broadcast::Sender<ProgressUpdate>,
    
    /// Worker handles
    workers: Mutex<Vec<JoinHandle<()>>>,
    
    /// Configuration (wrapped for dynamic updates)
    config: Arc<RwLock<DownloadConfig>>,
    
    /// Running state
    running: Arc<std::sync::atomic::AtomicBool>,
    
    /// Task notification for workers (wake when new task added)
    task_notify: Arc<Notify>,
    
    /// *arr API client for bi-directional sync
    arr_client: Option<Arc<crate::arr::ArrClient>>,
}

impl DownloadOrchestrator {
    /// Create new orchestrator
    pub fn new(
        config: DownloadConfig,
        host_registry: Arc<HostRegistry>,
        db: Option<Arc<Db>>,
        sonarr_config: Option<crate::config::ArrConfig>,
        radarr_config: Option<crate::config::ArrConfig>,
    ) -> Self {
        let task_manager = Arc::new(DownloadTaskManager::new());
        let download_engine = Arc::new(SimpleDownloadEngine::with_config(config.clone()));
        let (progress_tx, _) = broadcast::channel(1000);
        
        // Create arr client if either Sonarr or Radarr is configured
        let arr_client = if sonarr_config.is_some() || radarr_config.is_some() {
            Some(Arc::new(crate::arr::ArrClient::new(sonarr_config, radarr_config)))
        } else {
            None
        };
        
        Self {
            task_manager,
            download_engine: Arc::new(RwLock::new(download_engine)),
            host_registry,
            db,
            progress_tx,
            workers: Mutex::new(Vec::new()),
            config: Arc::new(RwLock::new(config)),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            task_notify: Arc::new(Notify::new()),
            arr_client,
        }
    }
    
    /// Start orchestrator with workers
    pub async fn start(&self) {
        use std::sync::atomic::Ordering;
        
        if self.running.load(Ordering::SeqCst) {
            return;
        }
        
        self.running.store(true, Ordering::SeqCst);
        
        {
            let config_guard = self.config.read().await;
            tracing::info!("Starting download orchestrator with {} workers", config_guard.max_concurrent);
        }
        
        // Restore tasks from database
        if let Err(e) = self.restore_from_db().await {
            tracing::error!("Failed to restore tasks from database: {}", e);
        }
        
        // Spawn worker tasks
        let mut workers = self.workers.lock().await;
        // Acquire config read lock to get max_concurrent
        let max_concurrent = self.config.read().await.max_concurrent;
        for worker_id in 0..max_concurrent {
            let handle = self.spawn_worker(worker_id);
            workers.push(handle);
        }
    }
    
    /// Stop orchestrator
    pub async fn stop(&self) {
        use std::sync::atomic::Ordering;
        
        self.running.store(false, Ordering::SeqCst);
        
        // Wait for workers to finish
        let mut workers = self.workers.lock().await;
        for worker in workers.drain(..) {
            let _ = worker.await;
        }
        
        tracing::info!("Download orchestrator stopped");
    }
    
    
    /// Add a new download with duplicate detection
    pub async fn add_download(
        &self,
        url: String,
        filename: String,
        host: String,
        category: String,
    ) -> Result<DownloadTask, anyhow::Error> {
        self.add_download_with_metadata(url, Some(filename), host, category, None).await
    }
    
    /// Add a new download with TMDB metadata for organized folder structure
    pub async fn add_download_with_metadata(
        &self,
        url: String,
        filename_override: Option<String>,
        host: String,
        category: String,
        tmdb_metadata: Option<TmdbDownloadMetadata>,
    ) -> Result<DownloadTask, anyhow::Error> {
        // === DEBUG: Log orchestrator input ===
        tracing::info!("=== [ORCHESTRATOR] add_download_with_metadata called ===");
        tracing::info!("[ORCHESTRATOR] Input URL: {}", url);
        tracing::info!("[ORCHESTRATOR] Input filename_override: {:?}", filename_override);
        tracing::info!("[ORCHESTRATOR] Input host: {}", host);
        tracing::info!("[ORCHESTRATOR] Input category: {}", category);
        if let Some(ref meta) = tmdb_metadata {
            tracing::info!("[ORCHESTRATOR] TMDB: tmdb_id={:?}, media_type={:?}, title={:?}", 
                meta.tmdb_id, meta.media_type, meta.title);
        }
        
        // Extract Fshare code from URL for duplicate detection
        let fshare_code = Self::extract_fshare_code(&url);
        tracing::info!("[ORCHESTRATOR] Extracted fshare_code: {:?}", fshare_code);
        
        // Check for duplicates if we have a Fshare code
        if let Some(code) = &fshare_code {
            if let Some(existing) = self.find_task_by_fshare_code(code) {
                let filename = filename_override.unwrap_or_else(|| "unknown".to_string());
                return self.handle_duplicate(existing, url, filename, host, category, code.clone()).await;
            }
        }
        
        // Fetch real filename and size from Fshare API
        let (filename, file_size) = if let Some(handler) = self.host_registry.get_handler(&host) {
            match handler.get_file_info(&url).await {
                Ok(file_info) => {
                    tracing::info!("Fetched file info from API: name='{}', size={}", file_info.filename, file_info.size);
                    // Use filename override if provided, otherwise use API name
                    let final_name = filename_override.unwrap_or(file_info.filename);
                    (final_name, file_info.size)
                }
                Err(e) => {
                    tracing::warn!("Failed to get file info from API, using fallback: {}", e);
                    let final_name = filename_override.unwrap_or_else(|| url.split('/').last().unwrap_or("unknown").to_string());
                    (final_name, 0u64)
                }
            }
        } else {
            let final_name = filename_override.unwrap_or_else(|| url.split('/').last().unwrap_or("unknown").to_string());
            (final_name, 0u64)
        };
        
        // Build destination path based on TMDB metadata
        let download_dir = {
            self.config.read().await.download_dir.clone()
        };
        let destination = self.build_destination_path(&filename, &category, &tmdb_metadata, &download_dir);
        
        // Create new task with file size
        let mut task = DownloadTask::new(url, filename.clone(), host, category);
        task.fshare_code = fshare_code;
        task.destination = destination;
        task.size = file_size;
        
        // Add to manager (SSOT)
        self.task_manager.add_task(task.clone());
        
        // Wake idle workers
        self.task_notify.notify_waiters();
        
        // Persist to database
        if let Some(db) = &self.db {
            db.save_task(&task)?;
        }
        
        // Broadcast task added event
        let _ = self.progress_tx.send(ProgressUpdate {
            task_id: task.id.to_string(),
            downloaded_bytes: 0,
            total_bytes: 0,
            speed_bytes_per_sec: 0.0,
            eta_seconds: 0.0,
            percentage: 0.0,
            state: "QUEUED".to_string(),
        });
        
        tracing::info!("Added download: {} ({}) [code: {:?}] -> {}", task.filename, task.id, task.fshare_code, task.destination);
        Ok(task)
    }
    
    /// Build organized destination path based on TMDB metadata
    /// Movies: collection_name/movie_name (year)/file OR movie_name (year)/file
    /// TV: series_name/season_xx/file
    fn build_destination_path(&self, filename: &str, category: &str, tmdb: &Option<TmdbDownloadMetadata>, root_dir: &std::path::Path) -> String {
        let base_dir = root_dir;
        
        if let Some(meta) = tmdb {
            let media_type = meta.media_type.as_deref().unwrap_or(category);
            
            match media_type {
                "movie" => {
                    // Build: [Collection]/MovieName (Year)/filename
                    let movie_folder = if let Some(ref title) = meta.title {
                        if let Some(ref year) = meta.year {
                            format!("{} ({})", Self::sanitize_filename(title), year)
                        } else {
                            Self::sanitize_filename(title)
                        }
                    } else {
                        "Unknown Movie".to_string()
                    };
                    
                    if let Some(ref collection) = meta.collection_name {
                        base_dir.join(Self::sanitize_filename(collection))
                            .join(&movie_folder)
                            .join(filename)
                            .to_string_lossy()
                            .to_string()
                    } else {
                        base_dir.join(&movie_folder)
                            .join(filename)
                            .to_string_lossy()
                            .to_string()
                    }
                }
                "tv" => {
                    // Build: SeriesName/Season XX/filename
                    let series_folder = if let Some(ref title) = meta.title {
                        Self::sanitize_filename(title)
                    } else {
                        "Unknown Series".to_string()
                    };
                    
                    let season_folder = if let Some(season) = meta.season {
                        format!("Season {:02}", season)
                    } else {
                        "Season 01".to_string()
                    };
                    
                    base_dir.join(&series_folder)
                        .join(&season_folder)
                        .join(filename)
                        .to_string_lossy()
                        .to_string()
                }
                _ => {
                    // Default: just use base dir
                    base_dir.join(filename).to_string_lossy().to_string()
                }
            }
        } else {
            // No TMDB metadata, use simple path
            base_dir.join(filename).to_string_lossy().to_string()
        }
    }
    
    /// Sanitize a string for use as a folder/file name
    fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c,
            })
            .collect::<String>()
            .trim()
            .to_string()
    }
    
    /// Extract Fshare file code from URL
    /// Example: https://www.fshare.vn/file/8DW6WQOV5R551DL -> Some("8DW6WQOV5R551DL")
    fn extract_fshare_code(url: &str) -> Option<String> {
        if url.contains("fshare.vn/file/") {
            url.split("/file/")
                .nth(1)
                .and_then(|s| s.split('?').next())
                .map(|s| s.to_string())
        } else {
            None
        }
    }
    
    /// Find task by Fshare code
    fn find_task_by_fshare_code(&self, code: &str) -> Option<DownloadTask> {
        self.task_manager.get_tasks()
            .into_iter()
            .find(|t| t.fshare_code.as_deref() == Some(code))
    }
    
    /// Handle duplicate download based on existing task state
    async fn handle_duplicate(
        &self,
        existing: DownloadTask,
        url: String,
        filename: String,
        host: String,
        category: String,
        code: String,
    ) -> Result<DownloadTask, anyhow::Error> {
        use crate::downloader::DownloadState;
        
        match existing.state {
            // Active states - skip and return existing
            DownloadState::Queued | 
            DownloadState::Starting | 
            DownloadState::Downloading | 
            DownloadState::Paused | 
            DownloadState::Completed => {
                tracing::info!(
                    "Duplicate detected [code: {}]: Task {} already exists in state {:?}, skipping",
                    code, existing.id, existing.state
                );
                Ok(existing)
            }
            
            // Failed/Cancelled states - delete old and create new
            DownloadState::Failed | 
            DownloadState::Cancelled => {
                tracing::info!(
                    "Duplicate detected [code: {}]: Task {} is {:?}, deleting and creating new",
                    code, existing.id, existing.state
                );
                
                // Delete old task (this will also delete the file)
                self.task_manager.delete_task(existing.id);
                
                // Delete from database
                if let Some(db) = &self.db {
                    let _ = db.delete_task(existing.id);
                }
                
                // Create new task
                let mut task = DownloadTask::new(url, filename, host, category);
                task.fshare_code = Some(code);
                let download_dir = self.config.read().await.download_dir.clone();
                task.destination = download_dir.join(&task.filename).to_string_lossy().to_string();
                
                // Add to manager
                self.task_manager.add_task(task.clone());
                
                // Persist to database
                if let Some(db) = &self.db {
                    db.save_task(&task)?;
                }
                
                // Broadcast task added event
                let _ = self.progress_tx.send(ProgressUpdate {
                    task_id: task.id.to_string(),
                    downloaded_bytes: 0,
                    total_bytes: 0,
                    speed_bytes_per_sec: 0.0,
                    eta_seconds: 0.0,
                    percentage: 0.0,
                    state: "QUEUED".to_string(),
                });
                
                tracing::info!("Created new download: {} ({})", task.filename, task.id);
                Ok(task)
            }
            
            // Other states - skip and return existing
            _ => {
                tracing::info!(
                    "Duplicate detected [code: {}]: Task {} in state {:?}, skipping",
                    code, existing.id, existing.state
                );
                Ok(existing)
            }
        }
    }
    
    /// Get task manager reference
    pub fn task_manager(&self) -> &Arc<DownloadTaskManager> {
        &self.task_manager
    }

    /// Get aggregate engine statistics
    pub async fn get_stats(&self) -> EngineStats {
        self.task_manager.get_stats()
    }
    
    /// Subscribe to progress updates
    pub fn subscribe_progress(&self) -> broadcast::Receiver<ProgressUpdate> {
        self.progress_tx.subscribe()
    }
    
    /// Broadcast a task update to all WebSocket clients
    /// Used when task state changes (pause, resume, etc.)
    pub fn broadcast_task_update(&self, task: &DownloadTask) {
        let state_str = format!("{:?}", task.state).to_uppercase();
        
        let _ = self.progress_tx.send(ProgressUpdate {
            task_id: task.id.to_string(),
            downloaded_bytes: task.downloaded,
            total_bytes: task.size,
            speed_bytes_per_sec: task.speed,
            eta_seconds: task.eta,
            percentage: task.progress as f64,
            state: state_str,
        });
        
        tracing::debug!("Broadcast task update: {} -> {:?}", task.id, task.state);
    }
    
    /// Spawn a worker
    fn spawn_worker(&self, worker_id: usize) -> JoinHandle<()> {
        let running = self.running.clone();
        let task_manager = self.task_manager.clone();
        let download_engine = self.download_engine.clone();
        let host_registry = self.host_registry.clone();
        let db = self.db.clone();
        let progress_tx = self.progress_tx.clone();
        let config = self.config.clone();
        let arr_client = self.arr_client.clone();
        let task_notify = self.task_notify.clone();
        
        tokio::spawn(async move {
            tracing::debug!("Worker {} started", worker_id);
            
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                // Check if this worker should be active based on current config
                // This allows dynamic max_concurrent changes without restart
                {
                    let current_max = config.read().await.max_concurrent;
                    if worker_id >= current_max {
                        // This worker is over the limit, sleep and check again
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        continue;
                    }
                }
                
                // Get next queued task
                if let Some(task) = task_manager.pop_next_queued() {
                    tracing::info!("Worker {}: Processing task {}", worker_id, task.id);
                    
                    // Acquire locks for current config and engine
                    let (current_engine, current_config) = {
                        let engine_guard = download_engine.read().await;
                        let config_guard = config.read().await;
                        (engine_guard.clone(), config_guard.clone())
                    };

                    // Process task
                    Self::process_task_static(
                        worker_id,
                        task,
                        &task_manager,
                        &current_engine,
                        &host_registry,
                        db.as_ref(),
                        &progress_tx,
                        &current_config,
                        arr_client.as_ref(),
                    ).await;
                } else {
                    // No tasks - wait for notification or timeout
                    // Use select! to allow cancellation via running flag check
                    tokio::select! {
                        _ = task_notify.notified() => {},
                        _ = tokio::time::sleep(Duration::from_secs(2)) => {},
                    }
                }
            }
            
            tracing::debug!("Worker {} stopped", worker_id);
        })
    }
    
    /// Process a single task (static method for worker)
    async fn process_task_static(
        worker_id: usize,
        mut task: DownloadTask,
        task_manager: &Arc<DownloadTaskManager>,
        download_engine: &Arc<SimpleDownloadEngine>,
        host_registry: &Arc<HostRegistry>,
        db: Option<&Arc<Db>>,
        progress_tx: &broadcast::Sender<ProgressUpdate>,
        config: &DownloadConfig,
        arr_client: Option<&Arc<crate::arr::ArrClient>>,
    ) {
        // Update state to Starting
        task.state = DownloadState::Starting;
        task.started_at = Some(Utc::now());
        task_manager.update_task(task.clone());
        
        // Broadcast Starting state to UI
        let _ = progress_tx.send(ProgressUpdate {
            task_id: task.id.to_string(),
            downloaded_bytes: task.downloaded,
            total_bytes: task.size,
            speed_bytes_per_sec: 0.0,
            eta_seconds: 0.0,
            percentage: task.progress as f64,
            state: "STARTING".to_string(),
        });
        
        if let Some(db) = db {
            let _ = db.save_task(&task);
        }
        
        // Resolve URL using host handler
        let download_url = match host_registry.get_handler(&task.host) {
            Some(handler) => {
                tracing::info!("Worker {}: Resolving URL with {} handler", worker_id, task.host);
                match handler.resolve_download_url(&task.original_url).await {
                    Ok(resolved) => {
                        task.url = resolved.direct_url.clone();
                        task.url_metadata = Some(UrlMetadata {
                            resolved_at: Utc::now(),
                            expires_at: Utc::now() + chrono::Duration::hours(6),
                        });
                        task_manager.update_task(task.clone());
                        resolved.direct_url
                    }
                    Err(e) => {
                        tracing::error!("Worker {}: Failed to resolve URL: {}", worker_id, e);
                        task_manager.mark_failed(task.id, format!("URL resolution failed: {}", e));
                        if let Some(db) = db {
                            if let Some(failed_task) = task_manager.get_task(task.id) {
                                let _ = db.save_task(&failed_task);
                            }
                        }
                        return;
                    }
                }
            }
            None => {
                tracing::warn!("Worker {}: No handler for host '{}', using original URL", worker_id, task.host);
                task.original_url.clone()
            }
        };
        
        // Update state to Downloading
        task.state = DownloadState::Downloading;
        task_manager.update_task(task.clone());
        
        // Download the file
        let destination = PathBuf::from(&task.destination);
        let task_id = task.id;
        let cancel_token = task.cancel_token.clone();
        
        // Clone for the progress callback closure (needs to be 'static)
        let task_manager_clone = Arc::clone(task_manager);
        let progress_tx_clone = progress_tx.clone();
        
        let result = download_engine.download_file(
            &download_url,
            &destination,
            move |progress| {
                // Update ALL progress fields in one atomic operation to prevent flickering
                task_manager_clone.update_task_progress(
                    task_id,
                    progress.downloaded_bytes,
                    progress.total_bytes,
                    progress.speed_bytes_per_sec,
                    progress.eta_seconds,
                    progress.percentage as f32,
                );
                
                // Broadcast progress (WebSocket handler will just read the task, not update again)
                let _ = progress_tx_clone.send(ProgressUpdate {
                    task_id: task_id.to_string(),
                    downloaded_bytes: progress.downloaded_bytes,
                    total_bytes: progress.total_bytes,
                    speed_bytes_per_sec: progress.speed_bytes_per_sec,
                    eta_seconds: progress.eta_seconds,
                    percentage: progress.percentage,
                    state: "DOWNLOADING".to_string(),
                });
            },
            &cancel_token,
        ).await;
        
        // Handle result
        match result {
            Ok(()) => {
                tracing::info!("Worker {}: Download completed for {}", worker_id, task_id);
                task_manager.mark_completed(task_id);
                
                if let Some(db) = db {
                    if let Some(completed_task) = task_manager.get_task(task_id) {
                        let _ = db.save_task(&completed_task);
                    }
                }
                
                // Trigger *arr sync if configured
                if let Some(arr_client) = arr_client {
                    if let Some(completed_task) = task_manager.get_task(task_id) {
                        let filename = completed_task.filename.clone();
                        let destination = PathBuf::from(&completed_task.destination);
                        let arr_client = Arc::clone(arr_client);
                        
                        tokio::spawn(async move {
                            arr_client.notify_completion(&filename, &destination).await;
                        });
                    }
                }
                
                // Broadcast completion
                let _ = progress_tx.send(ProgressUpdate {
                    task_id: task_id.to_string(),
                    downloaded_bytes: 0,
                    total_bytes: 0,
                    speed_bytes_per_sec: 0.0,
                    eta_seconds: 0.0,
                    percentage: 100.0,
                    state: "COMPLETED".to_string(),
                });
            }
            Err(e) => {
                let error_string = e.to_string();
                
                // Check if this was a pause action (task state is already Paused)
                // In this case, we should NOT retry or mark as failed
                if let Some(task) = task_manager.get_task(task_id) {
                    if task.state == DownloadState::Paused {
                        tracing::info!("Worker {}: Download paused for {} (by user request)", worker_id, task_id);
                        
                        // Broadcast paused state
                        let _ = progress_tx.send(ProgressUpdate {
                            task_id: task_id.to_string(),
                            downloaded_bytes: task.downloaded,
                            total_bytes: task.size,
                            speed_bytes_per_sec: 0.0,
                            eta_seconds: 0.0,
                            percentage: task.progress as f64,
                            state: "PAUSED".to_string(),
                        });
                        
                        // Save to database
                        if let Some(db) = db {
                            let _ = db.save_task(&task);
                        }
                        return; // Don't retry or mark as failed
                    }
                }
                
                tracing::error!("Worker {}: Download failed for {}: {}", worker_id, task_id, error_string);
                
                // Check if user cancelled (not paused)
                if cancel_token.is_cancelled() {
                    // Check one more time if it was paused
                    if let Some(task) = task_manager.get_task(task_id) {
                        if task.state == DownloadState::Paused {
                            return; // Already handled above
                        }
                    }
                    
                    // True cancellation (delete/stop), not pause
                    task_manager.mark_failed(task_id, "Download cancelled".to_string());
                    
                    if let Some(db) = db {
                        if let Some(updated_task) = task_manager.get_task(task_id) {
                            let _ = db.save_task(&updated_task);
                        }
                    }
                    return;
                }
                
                // Check if should retry (only for actual failures, not pauses)
                if let Some(mut task) = task_manager.get_task(task_id) {
                    if task.retry_count < config.retry.max_retries {
                        task.retry_count += 1;
                        task.state = DownloadState::Waiting;
                        let delay = Self::calculate_retry_delay_static(task.retry_count, config);
                        task.wait_until = Some(Utc::now() + chrono::Duration::from_std(delay).unwrap());
                        task.error_message = Some(format!("Retry {}/{}: {}", task.retry_count, config.retry.max_retries, error_string));
                        task_manager.update_task(task.clone());
                        
                        tracing::warn!("Worker {}: Will retry task {} in {:?}", worker_id, task_id, delay);
                    } else {
                        task_manager.mark_failed(task_id, format!("Max retries exceeded: {}", error_string));
                    }
                    
                    if let Some(db) = db {
                        if let Some(updated_task) = task_manager.get_task(task_id) {
                            let _ = db.save_task(&updated_task);
                        }
                    }
                }
            }
        }
    }
    
    /// Calculate retry delay with exponential backoff
    fn calculate_retry_delay_static(retry_count: u32, config: &DownloadConfig) -> Duration {
        let base_delay = config.retry.base_delay_ms as u64;
        let max_delay = config.retry.max_delay_ms as u64;
        let delay = base_delay * 2u64.pow(retry_count.saturating_sub(1));
        Duration::from_millis(delay.min(max_delay))
    }
    
    /// Restore tasks from database
    async fn restore_from_db(&self) -> anyhow::Result<()> {
        if let Some(db) = &self.db {
            tracing::info!("Restoring tasks from database...");
            
            let tasks = db.get_all_tasks()?;
            let mut restored_count = 0;
            
            for task in tasks {
                match task.state {
                    DownloadState::Downloading | DownloadState::Starting => {
                        // Resume interrupted downloads as QUEUED
                        let mut resumed_task = task.clone();
                        resumed_task.state = DownloadState::Queued;
                        self.task_manager.add_task(resumed_task);
                        restored_count += 1;
                    }
                    DownloadState::Queued | DownloadState::Waiting => {
                        // Re-queue pending downloads
                        self.task_manager.add_task(task);
                        restored_count += 1;
                    }
                    _ => {
                        // Keep other states as-is
                        self.task_manager.add_task(task);
                        restored_count += 1;
                    }
                }
            }
            
            tracing::info!("Restored {} tasks from database", restored_count);
        }
        
        Ok(())
    }

    /// Get current download configuration
    pub async fn get_config(&self) -> DownloadConfig {
        self.config.read().await.clone()
    }
    
    /// Update download configuration
    pub async fn update_config(&self, new_config: DownloadConfig) {
        // Update config
        {
            let mut config_guard = self.config.write().await;
            *config_guard = new_config.clone();
        }
        
        // Recreation of engine to pick up new settings
        {
            let new_engine = Arc::new(SimpleDownloadEngine::with_config(new_config.clone()));
            let mut engine_guard = self.download_engine.write().await;
            *engine_guard = new_engine;
        }
        
        tracing::info!("Updated download configuration: max_concurrent={}, segments={}", 
            new_config.max_concurrent, new_config.segments_per_download);
            
        // Note: Changing max_concurrent currently relies on restart or we need to implement dynamic pool resizing.
        // For now, it will apply on next restart, but segments_per_download will apply immediately.
    }
}
