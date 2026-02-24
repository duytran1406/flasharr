//! Download Service
//!
//! Business logic layer for download operations.
//! Abstracts database and TaskManager interactions from API handlers.

use std::sync::Arc;
use uuid::Uuid;
use crate::db::Db;
use crate::downloader::{DownloadOrchestrator, DownloadTask, DownloadState};
use crate::error::{FlasharrError, FlasharrResult};
use crate::utils::status_utils::StatusCounts;
use crate::utils::batch_utils::BatchStats;

/// Service for download operations
pub struct DownloadService {
    db: Arc<Db>,
    orchestrator: Arc<DownloadOrchestrator>,
}

impl DownloadService {
    /// Create a new DownloadService
    pub fn new(db: Arc<Db>, orchestrator: Arc<DownloadOrchestrator>) -> Self {
        Self { db, orchestrator }
    }
    
    // =========================================================================
    // Status Operations
    // =========================================================================
    
    /// Get status counts for UI filter dropdown
    pub async fn get_status_counts(&self) -> StatusCounts {
        let db_counts = self.db.get_status_counts_async().await.unwrap_or_default();
        StatusCounts::from_db_counts(db_counts)
    }
    
    // =========================================================================
    // Task Operations
    // =========================================================================
    
    /// Get a single task by ID, checking memory first then database
    pub async fn get_task(&self, id: Uuid) -> FlasharrResult<DownloadTask> {
        self.orchestrator
            .get_task_unified(id)
            .await
            .ok_or(FlasharrError::DownloadNotFound(id))
    }
    
    /// Pause a single task
    pub fn pause_task(&self, id: Uuid) -> FlasharrResult<DownloadTask> {
        self.orchestrator
            .task_manager()
            .pause_task(id)
            .ok_or(FlasharrError::DownloadInvalidState {
                id,
                expected: "DOWNLOADING/QUEUED".to_string(),
                actual: "unknown".to_string(),
            })
    }
    
    /// Resume a single task
    pub fn resume_task(&self, id: Uuid) -> FlasharrResult<DownloadTask> {
        self.orchestrator
            .task_manager()
            .resume_task(id)
            .ok_or(FlasharrError::DownloadInvalidState {
                id,
                expected: "PAUSED".to_string(),
                actual: "unknown".to_string(),
            })
    }
    
    /// Retry a failed task
    pub fn retry_task(&self, id: Uuid) -> FlasharrResult<DownloadTask> {
        self.orchestrator
            .task_manager()
            .retry_task(id)
            .ok_or(FlasharrError::DownloadInvalidState {
                id,
                expected: "FAILED/CANCELLED".to_string(),
                actual: "unknown".to_string(),
            })
    }
    
    /// Delete a task
    pub fn delete_task(&self, id: Uuid) -> FlasharrResult<()> {
        let deleted = self.orchestrator.task_manager().delete_task(id);
        if deleted {
            // Also delete from database
            if let Err(e) = self.db.delete_task(id) {
                tracing::warn!("Failed to delete task from database: {}", e);
            }
            self.orchestrator.broadcast_task_removed(&id.to_string());
            Ok(())
        } else {
            Err(FlasharrError::DownloadNotFound(id))
        }
    }
    
    // =========================================================================
    // Batch Operations
    // =========================================================================
    
    /// Get all tasks in a batch
    pub async fn get_batch_tasks(&self, batch_id: &str) -> FlasharrResult<Vec<DownloadTask>> {
        let tasks = self.db
            .get_tasks_by_batch_id_async(batch_id.to_string())
            .await
            .map_err(|e| FlasharrError::Database(e.to_string()))?;
        
        if tasks.is_empty() {
            return Err(FlasharrError::BatchNotFound(batch_id.to_string()));
        }
        
        Ok(self.merge_realtime_progress(tasks))
    }
    
    /// Get batch statistics
    pub async fn get_batch_stats(&self, batch_id: &str) -> FlasharrResult<BatchStats> {
        let tasks = self.get_batch_tasks(batch_id).await?;
        Ok(BatchStats::from_tasks(&tasks))
    }
    
    /// Pause all tasks in a batch
    pub async fn pause_batch(&self, batch_id: &str) -> FlasharrResult<usize> {
        let tasks = self.db
            .get_tasks_by_batch_id_async(batch_id.to_string())
            .await
            .map_err(|e| FlasharrError::Database(e.to_string()))?;
        
        if tasks.is_empty() {
            return Err(FlasharrError::BatchNotFound(batch_id.to_string()));
        }
        
        let pauseable_ids: Vec<Uuid> = tasks.iter()
            .filter(|t| matches!(t.state, 
                DownloadState::Downloading | 
                DownloadState::Queued | 
                DownloadState::Starting | 
                DownloadState::Waiting))
            .map(|t| t.id)
            .collect();
        
        if pauseable_ids.is_empty() {
            return Ok(0);
        }
        
        // Update database atomically
        self.db
            .batch_update_states_async(pauseable_ids.clone(), "PAUSED".to_string())
            .await
            .map_err(|e| FlasharrError::Database(e.to_string()))?;
        
        // Update TaskManager and broadcast
        let mut affected = 0;
        for task in tasks {
            if pauseable_ids.contains(&task.id) {
                if let Some(paused_task) = self.orchestrator.task_manager().pause_task(task.id) {
                    self.orchestrator.broadcast_task_update(&paused_task);
                    affected += 1;
                } else {
                    // Task not in memory, add it as paused
                    let mut paused = task.clone();
                    paused.state = DownloadState::Paused;
                    self.orchestrator.task_manager().add_task(paused.clone());
                    self.orchestrator.broadcast_task_update(&paused);
                    affected += 1;
                }
            }
        }
        
        Ok(affected)
    }
    
    /// Resume all tasks in a batch
    pub async fn resume_batch(&self, batch_id: &str) -> FlasharrResult<usize> {
        let tasks = self.db
            .get_tasks_by_batch_id_async(batch_id.to_string())
            .await
            .map_err(|e| FlasharrError::Database(e.to_string()))?;
        
        if tasks.is_empty() {
            return Err(FlasharrError::BatchNotFound(batch_id.to_string()));
        }
        
        let resumable_ids: Vec<Uuid> = tasks.iter()
            .filter(|t| t.state == DownloadState::Paused || 
                       (t.state.can_retry() && t.state != DownloadState::Completed))
            .map(|t| t.id)
            .collect();
        
        if resumable_ids.is_empty() {
            return Ok(0);
        }
        
        // Update database atomically
        self.db
            .batch_update_states_async(resumable_ids.clone(), "QUEUED".to_string())
            .await
            .map_err(|e| FlasharrError::Database(e.to_string()))?;
        
        // Update TaskManager and broadcast
        let mut affected = 0;
        for task in tasks {
            if resumable_ids.contains(&task.id) {
                affected += self.resume_single_task_for_batch(&task);
            }
        }
        
        // Wake workers to process resumed tasks
        self.orchestrator.wake_workers();
        
        Ok(affected)
    }
    
    /// Delete all tasks in a batch
    pub async fn delete_batch(&self, batch_id: &str) -> FlasharrResult<usize> {
        let tasks = self.db
            .get_tasks_by_batch_id_async(batch_id.to_string())
            .await
            .map_err(|e| FlasharrError::Database(e.to_string()))?;
        
        // Delete from TaskManager
        let mut affected = 0;
        for task in &tasks {
            if self.orchestrator.task_manager().delete_task(task.id) {
                affected += 1;
            }
        }
        
        // Delete from database
        self.db
            .delete_tasks_by_batch_id_async(batch_id.to_string())
            .await
            .map_err(|e| FlasharrError::Database(e.to_string()))?;
        
        Ok(affected)
    }
    
    /// Get or create a batch ID by name
    pub async fn get_or_create_batch(&self, name: &str, provided_id: Option<&str>) -> String {
        if let Ok(Some(existing_id)) = self.db.get_batch_id_by_name_async(name).await {
            tracing::info!("Reusing existing batch: {} ({})", name, existing_id);
            existing_id
        } else {
            let new_id = provided_id
                .map(|s| s.to_string())
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            tracing::info!("Creating new batch: {} ({})", name, new_id);
            new_id
        }
    }
    
    // =========================================================================
    // Helper Methods
    // =========================================================================
    
    /// Merge real-time progress from active tasks into a list of tasks
    pub fn merge_realtime_progress(&self, mut tasks: Vec<DownloadTask>) -> Vec<DownloadTask> {
        let active_tasks = self.orchestrator.task_manager().get_active_tasks();
        
        for task in tasks.iter_mut() {
            if let Some(active) = active_tasks.iter().find(|t| t.id == task.id) {
                task.progress = active.progress;
                task.speed = active.speed;
                task.eta = active.eta;
                task.downloaded = active.downloaded;
                task.state = active.state;
            }
        }
        
        tasks
    }
    
    /// Helper for resuming a single task during batch resume
    fn resume_single_task_for_batch(&self, task: &DownloadTask) -> usize {
        if task.state == DownloadState::Paused {
            if let Some(resumed_task) = self.orchestrator.task_manager().resume_task(task.id) {
                self.orchestrator.broadcast_task_update(&resumed_task);
                return 1;
            } else {
                // Task not in memory, add it as queued
                let mut resumed = task.clone();
                resumed.state = DownloadState::Queued;
                resumed.cancel_token = tokio_util::sync::CancellationToken::new();
                self.orchestrator.task_manager().add_task(resumed.clone());
                self.orchestrator.broadcast_task_update(&resumed);
                return 1;
            }
        } else if task.state.can_retry() && task.state != DownloadState::Completed {
            if let Some(retried_task) = self.orchestrator.task_manager().retry_task(task.id) {
                self.orchestrator.broadcast_task_update(&retried_task);
                return 1;
            } else {
                // Task not in memory, add it as queued
                let mut retried = task.clone();
                retried.state = DownloadState::Queued;
                retried.retry_count += 1;
                retried.error_message = None;
                self.orchestrator.task_manager().add_task(retried.clone());
                self.orchestrator.broadcast_task_update(&retried);
                return 1;
            }
        }
        0
    }
}
