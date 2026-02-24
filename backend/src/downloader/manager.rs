//! Download Task Manager
//!
//! Simple task manager for in-memory task storage.
//! Note: The DownloadEngine handles the actual download logic and persistence.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use uuid::Uuid;
use chrono::Utc;

use super::task::{DownloadTask, DownloadState};
use super::stats::EngineStats;

/// Thread-safe download task manager (in-memory only)
pub struct DownloadTaskManager {
    tasks: Arc<RwLock<HashMap<Uuid, DownloadTask>>>,
    /// Set of task IDs currently being processed by workers
    /// This prevents duplicate task claiming during URL resolution and download
    processing_tasks: Arc<RwLock<HashSet<Uuid>>>,
}

impl DownloadTaskManager {
    /// Create a new task manager
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            processing_tasks: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Add a task
    pub fn add_task(&self, task: DownloadTask) {
        let mut tasks = self.tasks.write().unwrap();
        tasks.insert(task.id, task);
    }
    
    /// Restore multiple tasks from database (used on startup)
    /// Loads QUEUED/PAUSED tasks into HashMap so resume/pause operations work
    pub fn restore_tasks(&self, tasks_to_restore: Vec<DownloadTask>) -> usize {
        let mut tasks = self.tasks.write().unwrap();
        let count = tasks_to_restore.len();
        for task in tasks_to_restore {
            tasks.insert(task.id, task);
        }
        tracing::info!("Restored {} tasks from database into TaskManager", count);
        count
    }


    /// Get all tasks (deprecated - use get_active_tasks for real-time + DB for full list)
    pub fn get_tasks(&self) -> Vec<DownloadTask> {
        let tasks = self.tasks.read().unwrap();
        tasks.values().cloned().collect()
    }
    
    /// Get only active tasks (DOWNLOADING/STARTING) for real-time progress updates
    /// This is the primary method for merging real-time data with DB queries
    pub fn get_active_tasks(&self) -> Vec<DownloadTask> {
        let tasks = self.tasks.read().unwrap();
        tasks.values()
            .filter(|t| matches!(t.state, DownloadState::Downloading | DownloadState::Starting))
            .cloned()
            .collect()
    }
    
    /// Get a specific task
    pub fn get_task(&self, id: Uuid) -> Option<DownloadTask> {
        let tasks = self.tasks.read().unwrap();
        tasks.get(&id).cloned()
    }
    
    /// Update a task
    pub fn update_task(&self, task: DownloadTask) {
        let mut tasks = self.tasks.write().unwrap();
        tasks.insert(task.id, task);
    }
    
    /// Update task progress including speed, ETA, and total size
    pub fn update_task_progress(&self, id: Uuid, downloaded: u64, size: u64, speed: f64, eta: f64, progress: f32) {
        let mut tasks = self.tasks.write().unwrap();
        if let Some(task) = tasks.get_mut(&id) {
            task.downloaded = downloaded;
            task.size = size;
            task.speed = speed;
            task.eta = eta;
            task.progress = progress;
        }
    }
    
    /// Remove a task
    pub fn remove_task(&self, id: Uuid) -> Option<DownloadTask> {
        let mut tasks = self.tasks.write().unwrap();
        tasks.remove(&id)
    }
    
    /// Delete a task (alias for remove_task, returns bool)
    /// Also deletes the downloaded file if it exists
    pub fn delete_task(&self, id: Uuid) -> bool {
        if let Some(task) = self.remove_task(id) {
            // Delete the downloaded file if it exists
            // Note: task.destination is already the FULL file path including filename
            let file_path = std::path::Path::new(&task.destination);
            if file_path.exists() {
                if let Err(e) = std::fs::remove_file(&file_path) {
                    tracing::warn!("Failed to delete file {:?}: {}", file_path, e);
                } else {
                    tracing::info!("Deleted file: {:?}", file_path);
                }
            } else {
                tracing::debug!("File not found (already deleted or never existed): {:?}", file_path);
            }
            true
        } else {
            false
        }
    }
    
    /// Pause a task
    /// This stops the active download by cancelling the cancel_token.
    /// The download can be resumed later and will continue from where it left off.
    pub fn pause_task(&self, id: Uuid) -> Option<DownloadTask> {
        let mut tasks = self.tasks.write().unwrap();
        if let Some(task) = tasks.get_mut(&id) {
            if task.state.can_pause() {
                // Cancel the ongoing download
                task.cancel_token.cancel();
                task.state = DownloadState::Paused;
                tracing::info!("Paused task {} - download cancelled", id);
                
                let result = Some(task.clone());
                
                // Remove from processing set since it's no longer active
                drop(tasks); // Release tasks lock before acquiring processing lock
                let mut processing = self.processing_tasks.write().unwrap();
                if processing.remove(&id) {
                    tracing::debug!("Removed task {} from processing set (paused)", id);
                }
                
                return result;
            }
        }
        None
    }
    
    /// Resume a paused task
    /// Creates a new cancel_token and re-queues the task
    pub fn resume_task(&self, id: Uuid) -> Option<DownloadTask> {
        let mut tasks = self.tasks.write().unwrap();
        if let Some(task) = tasks.get_mut(&id) {
            if task.state.can_resume() {
                // Create a new cancel token since the old one was cancelled
                task.cancel_token = tokio_util::sync::CancellationToken::new();
                task.state = DownloadState::Queued;
                tracing::info!("Resumed task {} - re-queued with new cancel token", id);
                return Some(task.clone());
            }
        }
        None
    }
    
    /// Retry a failed task
    pub fn retry_task(&self, id: Uuid) -> Option<DownloadTask> {
        let mut tasks = self.tasks.write().unwrap();
        if let Some(task) = tasks.get_mut(&id) {
            if task.state.can_retry() {
                task.state = DownloadState::Queued;
                task.retry_count += 1;
                task.error_message = None;
                return Some(task.clone());
            }
        }
        None
    }
    
    /// Pause all active downloads
    /// Cancels all ongoing downloads
    pub fn pause_all(&self) -> usize {
        let mut tasks = self.tasks.write().unwrap();
        let mut paused_ids = Vec::new();
        let mut count = 0;
        for task in tasks.values_mut() {
            if task.state.can_pause() {
                task.cancel_token.cancel();
                task.state = DownloadState::Paused;
                paused_ids.push(task.id);
                count += 1;
            }
        }
        tracing::info!("Paused {} downloads", count);
        
        // Remove all paused tasks from processing set
        drop(tasks); // Release tasks lock before acquiring processing lock
        let mut processing = self.processing_tasks.write().unwrap();
        for id in paused_ids {
            if processing.remove(&id) {
                tracing::debug!("Removed task {} from processing set (pause_all)", id);
            }
        }
        
        count
    }
    
    /// Resume all paused downloads
    /// Creates new cancel tokens and re-queues tasks
    pub fn resume_all(&self) -> usize {
        let mut tasks = self.tasks.write().unwrap();
        let mut count = 0;
        for task in tasks.values_mut() {
            if task.state.can_resume() {
                task.cancel_token = tokio_util::sync::CancellationToken::new();
                task.state = DownloadState::Queued;
                count += 1;
            }
        }
        tracing::info!("Resumed {} downloads", count);
        count
    }
    
    /// Get task count
    pub fn count(&self) -> usize {
        let tasks = self.tasks.read().unwrap();
        tasks.len()
    }
    
    /// Get engine statistics
    pub fn get_stats(&self) -> EngineStats {
        let tasks = self.tasks.read().unwrap();
        
        let mut active = 0;
        let mut queued = 0;
        let mut completed = 0;
        let mut failed = 0;
        let mut paused = 0;
        let mut cancelled = 0;
        let mut total_speed = 0.0;
        
        for task in tasks.values() {
            if task.state == DownloadState::Downloading || task.state == DownloadState::Starting {
                active += 1;
                total_speed += task.speed;
            } else if task.state == DownloadState::Extracting {
                active += 1;
            } else if task.state == DownloadState::Queued || task.state == DownloadState::Waiting {
                queued += 1;
            } else if task.state == DownloadState::Completed {
                completed += 1;
            } else if task.state == DownloadState::Failed {
                failed += 1;
            } else if task.state == DownloadState::Paused {
                paused += 1;
            } else if task.state == DownloadState::Cancelled {
                cancelled += 1;
            }
        }
        
        let stats = EngineStats {
            active_downloads: active,
            queued,
            completed,
            failed,
            paused,
            cancelled,
            total_speed,
            db_counts: None, // Will be populated by orchestrator from database
        };

        stats
    }
    
    /// Pop next queued task (for worker processing)
    /// This atomically claims the task by changing its state to Starting
    /// Also checks for Waiting tasks whose wait_until time has passed
    pub fn pop_next_queued(&self) -> Option<DownloadTask> {
        let now = Utc::now();
        
        // Step 1: Collect candidates with READ lock (allows concurrent access)
        let (waiting_candidate, queued_data) = {
            let tasks = self.tasks.read().unwrap();
            let processing = self.processing_tasks.read().unwrap();
            // Note: No clone needed - use reference directly
        
            // Check for waiting tasks
            let waiting_id = tasks.values()
                .find(|t| {
                    t.state == DownloadState::Waiting && 
                    t.wait_until.map(|until| now >= until).unwrap_or(true) &&
                    !processing.contains(&t.id)
                })
                .map(|t| t.id);
            
            // Collect queued task data for sorting (ID, priority, remaining_bytes, progress, created_at)
            let queued: Vec<(Uuid, i32, u64, f32, chrono::DateTime<Utc>)> = tasks.values()
                .filter(|t| t.state == DownloadState::Queued && !processing.contains(&t.id))
                .map(|t| (
                    t.id,
                    t.priority,
                    t.size.saturating_sub(t.downloaded),
                    t.progress,
                    t.created_at
                ))
                .collect();
            
            (waiting_id, queued)
        };
        // ← READ LOCKS RELEASED HERE - other threads can now access tasks!
        
        // Step 2: Handle waiting task (needs write lock)
        if let Some(task_id) = waiting_candidate {
            let mut tasks = self.tasks.write().unwrap();
            let mut processing = self.processing_tasks.write().unwrap();
            
            if let Some(task) = tasks.get_mut(&task_id) {
                tracing::info!("Re-queuing waiting task {} after retry delay", task_id);
                task.state = DownloadState::Starting;
                task.wait_until = None;
                processing.insert(task_id);
                tracing::debug!("Added task {} to processing set (from Waiting)", task_id);
                return Some(task.clone());
            }
        }
        
        // Step 3: Sort queued candidates WITHOUT holding any lock
        let mut sorted_candidates = queued_data;
        sorted_candidates.sort_by(|a, b| {
            // 1. Priority (DESC) - higher priority first
            if a.1 != b.1 {
                return b.1.cmp(&a.1); // Reverse order for DESC
            }
            // 2. Remaining Size (ASC) - smaller files first
            if a.2 != b.2 {
                return a.2.cmp(&b.2);
            }
            // 3. Progress (DESC) - more complete first
            if a.3 > b.3 {
                return std::cmp::Ordering::Less;
            }
            if a.3 < b.3 {
                return std::cmp::Ordering::Greater;
            }
            // 4. Date Added (ASC) - older first
            a.4.cmp(&b.4)
        });
        
        // Step 4: Claim the first task (needs write lock, but very brief)
        if let Some((task_id, _, _, _, _)) = sorted_candidates.first() {
            let mut tasks = self.tasks.write().unwrap();
            let mut processing = self.processing_tasks.write().unwrap();
            
            if let Some(task) = tasks.get_mut(task_id) {
                // CRITICAL: Re-check state since another worker may have claimed it
                // between our read and this write
                if task.state != DownloadState::Queued {
                    tracing::debug!(
                        "Task {} already claimed by another worker (state: {:?})",
                        task_id,
                        task.state
                    );
                    return None;
                }
                
                if processing.contains(task_id) {
                    tracing::debug!("Task {} already in processing set", task_id);
                    return None;
                }
                
                tracing::debug!(
                    "Claiming task {} (current state: {:?}, changing to Starting)",
                    task_id,
                    task.state
                );
                task.state = DownloadState::Starting;
                processing.insert(*task_id);
                tracing::debug!("Added task {} to processing set (from Queued)", task_id);
                return Some(task.clone());
            }
        }
        
        None
    }
    
    /// Get tasks by state
    pub fn get_tasks_by_state(&self, state: DownloadState) -> Vec<DownloadTask> {
        let tasks = self.tasks.read().unwrap();
        tasks.values()
            .filter(|t| t.state == state)
            .cloned()
            .collect()
    }
    
    /// Update task progress
    pub fn update_progress(&self, id: Uuid, progress: f32, _speed: f64) {
        let mut tasks = self.tasks.write().unwrap();
        if let Some(task) = tasks.get_mut(&id) {
            task.progress = progress;
            // Note: speed is not stored in DownloadTask currently
            // It's broadcast via WebSocket instead
        }
    }
    
    /// Mark a task as failed
    pub fn mark_failed(&self, id: Uuid, error: String) {
        let mut tasks = self.tasks.write().unwrap();
        if let Some(task) = tasks.get_mut(&id) {
            task.state = DownloadState::Failed;
            task.error_message = Some(error);
            task.completed_at = Some(chrono::Utc::now());
        }
        
        // Remove from processing set
        drop(tasks); // Release tasks lock before acquiring processing lock
        let mut processing = self.processing_tasks.write().unwrap();
        if processing.remove(&id) {
            tracing::debug!("Removed task {} from processing set (failed)", id);
        }
    }
    
    /// Mark a task as completed
    pub fn mark_completed(&self, id: Uuid) {
        let mut tasks = self.tasks.write().unwrap();
        if let Some(task) = tasks.get_mut(&id) {
            task.state = DownloadState::Completed;
            task.completed_at = Some(Utc::now());
            task.progress = 100.0;
            task.error_message = None;
        }
        
        // Remove from processing set
        drop(tasks); // Release tasks lock before acquiring processing lock
        let mut processing = self.processing_tasks.write().unwrap();
        if processing.remove(&id) {
            tracing::debug!("Removed task {} from processing set (completed)", id);
        }
    }
}

impl Default for DownloadTaskManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Phase 4: Event-Driven Cache
// ============================================================================

impl DownloadTaskManager {
    /// Start event listener for auto-updating cache (Phase 4)
    /// TaskManager subscribes to EventBus and auto-updates on events
    pub fn start_event_listener(
        self: Arc<Self>,
        mut event_rx: tokio::sync::broadcast::Receiver<super::events::TaskEvent>,
    ) {
        tokio::spawn(async move {
            tracing::info!("TaskManager event listener started (Phase 4)");
            
            while let Ok(event) = event_rx.recv().await {
                match event {
                    super::events::TaskEvent::Created { task, .. } => {
                        tracing::debug!("Event: Task created {}", task.id);
                        self.add_task(task);
                    }
                    super::events::TaskEvent::StateChanged { task, .. } => {
                        tracing::debug!("Event: Task state changed {} -> {:?}", task.id, task.state);
                        self.update_task(task);
                    }
                    super::events::TaskEvent::Completed { task, .. } => {
                        tracing::info!("Event: Task completed {}", task.id);
                        self.update_task(task.clone());
                        
                        // Auto-evict completed tasks after 5 minutes
                        let manager = Arc::clone(&self);
                        let task_id = task.id;
                        tokio::spawn(async move {
                            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
                            if manager.remove_task(task_id).is_some() {
                                tracing::info!("Auto-evicted completed task {} from cache", task_id);
                            }
                        });
                    }
                    super::events::TaskEvent::Failed { task, .. } => {
                        tracing::warn!("Event: Task failed {}", task.id);
                        self.update_task(task.clone());
                        
                        // Auto-evict failed tasks after 5 minutes
                        let manager = Arc::clone(&self);
                        let task_id = task.id;
                        tokio::spawn(async move {
                            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
                            if manager.remove_task(task_id).is_some() {
                                tracing::info!("Auto-evicted failed task {} from cache", task_id);
                            }
                        });
                    }
                    super::events::TaskEvent::Removed { task_id, .. } => {
                        tracing::debug!("Event: Task removed {}", task_id);
                        self.remove_task(task_id);
                    }
                    _ => {}
                }
            }
            
            tracing::warn!("TaskManager event listener stopped");
        });
    }
    
    /// Start background cleanup loop (additional safety mechanism)
    /// Periodically evicts old completed/failed tasks that might have been missed by events
    pub fn start_cleanup_loop(self: Arc<Self>) {
        tokio::spawn(async move {
            tracing::info!("TaskManager background cleanup loop started");
            
            loop {
                // Run cleanup every 10 minutes
                tokio::time::sleep(tokio::time::Duration::from_secs(600)).await;
                
                let mut evicted_count = 0;
                let now = Utc::now();
                
                // Get all tasks
                let tasks_to_check: Vec<(Uuid, DownloadTask)> = {
                    let tasks = self.tasks.read().unwrap();
                    tasks.iter().map(|(id, task)| (*id, task.clone())).collect()
                };
                
                // Check each task
                for (task_id, task) in tasks_to_check {
                    let should_evict = match task.state {
                        DownloadState::Completed | DownloadState::Failed => {
                            // Evict if older than 5 minutes
                            if let Some(completed_at) = task.completed_at {
                                let age = now.signed_duration_since(completed_at);
                                age > chrono::Duration::minutes(5)
                            } else {
                                false
                            }
                        }
                        _ => false,
                    };
                    
                    if should_evict {
                        if self.remove_task(task_id).is_some() {
                            evicted_count += 1;
                            tracing::info!(
                                "Background cleanup: Evicted old {:?} task {} (age: {:?})",
                                task.state,
                                task_id,
                                task.completed_at.map(|c| now.signed_duration_since(c))
                            );
                        }
                    }
                }
                
                if evicted_count > 0 {
                    tracing::info!("Background cleanup: Evicted {} old tasks", evicted_count);
                }
            }
        });
    }
}
