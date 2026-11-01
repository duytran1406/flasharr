//! Duplicate Detector Module
//!
//! Handles detection and handling of duplicate downloads based on provider identity.
//! Extracted from orchestrator.rs for better modularity.

use crate::db::Db;
use crate::downloader::manager::DownloadTaskManager;
use crate::downloader::progress::{ProgressUpdate, TaskEvent};
use crate::downloader::task::{DownloadState, DownloadTask};
use crate::utils::parser::FilenameParser;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Duplicate detector for provider downloads
pub struct DuplicateDetector;

impl DuplicateDetector {
    /// Find task by canonical provider identity in task manager
    pub async fn find_task_by_provider_item(
        task_manager: &DownloadTaskManager,
        provider_id: &str,
        provider_item_id: &str,
    ) -> Option<DownloadTask> {
        task_manager.get_tasks().await.into_iter().find(|task| {
            task.provider_id.as_deref() == Some(provider_id)
                && task.provider_item_id.as_deref() == Some(provider_item_id)
        })
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
        provider_id: String,
        provider_item_id: String,
        download_dir: &std::path::Path,
        task_manager: &DownloadTaskManager,
        db: Option<&Arc<Db>>,
        progress_tx: &broadcast::Sender<ProgressUpdate>,
    ) -> Result<DownloadTask, anyhow::Error> {
        match existing.state {
            // Active states - skip and return existing
            DownloadState::Queued
            | DownloadState::Starting
            | DownloadState::Downloading
            | DownloadState::Paused
            | DownloadState::Completed => {
                tracing::info!(
                    "Duplicate detected [code: {}]: Task {} already exists in state {:?}, skipping",
                    provider_item_id,
                    existing.id,
                    existing.state
                );
                Ok(existing)
            }

            // Failed/Cancelled states - delete old and create new
            DownloadState::Failed | DownloadState::Cancelled => {
                tracing::info!(
                    "Duplicate detected [code: {}]: Task {} is {:?}, deleting and creating new",
                    provider_item_id,
                    existing.id,
                    existing.state
                );

                // Delete old task
                task_manager.delete_task(existing.id).await;

                // Delete from database
                if let Some(db) = db {
                    let _ = db.delete_task(existing.id);
                }

                // Create new task
                let mut task = DownloadTask::new(url, filename, host, category);
                task.provider_id = Some(provider_id);
                task.provider_item_id = Some(provider_item_id);
                task.destination = download_dir
                    .join(&task.filename)
                    .to_string_lossy()
                    .to_string();

                // Parse quality metadata from filename
                let quality_attrs = FilenameParser::extract_quality_attributes(&task.filename);
                task.quality = Some(quality_attrs.quality_name());
                task.resolution = quality_attrs.resolution.clone();

                // Add to manager
                task_manager.add_task(task.clone()).await;

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
                    provider_item_id,
                    existing.id,
                    existing.state
                );
                Ok(existing)
            }
        }
    }
}
