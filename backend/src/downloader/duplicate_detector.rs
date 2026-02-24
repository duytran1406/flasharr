//! Duplicate Detector Module
//!
//! Handles detection and handling of duplicate downloads based on Fshare file codes.
//! Extracted from orchestrator.rs for better modularity.

use crate::downloader::task::{DownloadTask, DownloadState};
use crate::downloader::manager::DownloadTaskManager;
use crate::downloader::progress::{ProgressUpdate, TaskEvent};
use crate::db::Db;
use crate::utils::parser::FilenameParser;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Duplicate detector for Fshare downloads
pub struct DuplicateDetector;

impl DuplicateDetector {
    /// Extract Fshare file code from URL
    /// Example: https://www.fshare.vn/file/8DW6WQOV5R551DL -> Some("8DW6WQOV5R551DL")
    pub fn extract_fshare_code(url: &str) -> Option<String> {
        if url.contains("fshare.vn/file/") {
            url.split("/file/")
                .nth(1)
                .and_then(|s| s.split('?').next())
                .map(|s| s.to_string())
        } else {
            None
        }
    }
    
    /// Find task by Fshare code in task manager
    pub fn find_task_by_fshare_code(
        task_manager: &DownloadTaskManager, 
        code: &str
    ) -> Option<DownloadTask> {
        task_manager.get_tasks()
            .into_iter()
            .find(|t| t.fshare_code.as_deref() == Some(code))
    }
    
    /// Handle duplicate download based on existing task state
    /// Returns the existing task if it should be kept, or creates a new one if old should be replaced
    /// Note: Not yet wired into orchestrator due to async complexity - will be done in future refactor
    #[allow(dead_code)]
    pub async fn handle_duplicate(
        existing: DownloadTask,
        url: String,
        filename: String,
        host: String,
        category: String,
        code: String,
        download_dir: &std::path::Path,
        task_manager: &DownloadTaskManager,
        db: Option<&Arc<Db>>,
        progress_tx: &broadcast::Sender<ProgressUpdate>,
    ) -> Result<DownloadTask, anyhow::Error> {
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
                
                // Delete old task
                task_manager.delete_task(existing.id);
                
                // Delete from database
                if let Some(db) = db {
                    let _ = db.delete_task(existing.id);
                }
                
                // Create new task
                let mut task = DownloadTask::new(url, filename, host, category);
                task.fshare_code = Some(code);
                task.destination = download_dir.join(&task.filename).to_string_lossy().to_string();
                
                // Parse quality metadata from filename
                let quality_attrs = FilenameParser::extract_quality_attributes(&task.filename);
                task.quality = Some(quality_attrs.quality_name());
                task.resolution = quality_attrs.resolution.clone();
                
                // Add to manager
                task_manager.add_task(task.clone());
                
                // Persist to database
                if let Some(db) = db {
                    db.save_task(&task)?;
                }
                
                // Broadcast task added event
                let _ = progress_tx.send(ProgressUpdate {
                    event: TaskEvent::Added,
                    task_id: task.id.to_string(),
                    downloaded_bytes: 0,
                    total_bytes: task.size,
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
}
