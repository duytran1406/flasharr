//! Batch Utilities
//!
//! Reusable functions for batch progress calculation and aggregation.

use crate::downloader::{DownloadTask, DownloadState};

/// Aggregate batch state from individual task states
/// Returns the most representative state for display
#[allow(dead_code)]
pub fn aggregate_batch_state(tasks: &[DownloadTask]) -> DownloadState {
    if tasks.is_empty() {
        return DownloadState::Queued;
    }
    
    let downloading = tasks.iter().any(|t| matches!(t.state, DownloadState::Downloading | DownloadState::Starting));
    let failed = tasks.iter().any(|t| matches!(t.state, DownloadState::Failed));
    let paused = tasks.iter().any(|t| matches!(t.state, DownloadState::Paused));
    let queued = tasks.iter().any(|t| matches!(t.state, DownloadState::Queued | DownloadState::Waiting));
    let all_completed = tasks.iter().all(|t| matches!(t.state, DownloadState::Completed));
    
    if downloading {
        DownloadState::Downloading
    } else if all_completed {
        DownloadState::Completed
    } else if failed && !downloading && !queued {
        DownloadState::Failed
    } else if paused && !downloading && !queued {
        DownloadState::Paused
    } else {
        DownloadState::Queued
    }
}

/// Calculate batch progress statistics
#[derive(Debug, Clone, Default)]
pub struct BatchStats {
    pub total_items: usize,
    pub completed_items: usize,
    pub failed_items: usize,
    pub downloading_items: usize,
    pub paused_items: usize,
    pub queued_items: usize,
    pub total_size: u64,
    pub downloaded_size: u64,
    pub progress: f32,
    pub speed: f64,
}

impl BatchStats {
    /// Calculate stats from a slice of tasks
    pub fn from_tasks(tasks: &[DownloadTask]) -> Self {
        let total_items = tasks.len();
        let completed_items = tasks.iter().filter(|t| matches!(t.state, DownloadState::Completed)).count();
        let failed_items = tasks.iter().filter(|t| matches!(t.state, DownloadState::Failed)).count();
        let downloading_items = tasks.iter()
            .filter(|t| matches!(t.state, DownloadState::Downloading | DownloadState::Starting))
            .count();
        let paused_items = tasks.iter().filter(|t| matches!(t.state, DownloadState::Paused)).count();
        let queued_items = tasks.iter()
            .filter(|t| matches!(t.state, DownloadState::Queued | DownloadState::Waiting))
            .count();
        
        let total_size: u64 = tasks.iter().map(|t| t.size).sum();
        let downloaded_size: u64 = tasks.iter()
            .map(|t| ((t.progress as f64 / 100.0) * t.size as f64) as u64)
            .sum();
        
        let progress = if total_size > 0 {
            (downloaded_size as f64 / total_size as f64 * 100.0) as f32
        } else {
            0.0
        };
        
        let speed: f64 = tasks.iter()
            .filter(|t| matches!(t.state, DownloadState::Downloading))
            .map(|t| t.speed)
            .sum();
        
        Self {
            total_items,
            completed_items,
            failed_items,
            downloading_items,
            paused_items,
            queued_items,
            total_size,
            downloaded_size,
            progress,
            speed,
        }
    }
    
    /// Calculate estimated time remaining in seconds
    pub fn eta(&self) -> f64 {
        let remaining = self.total_size.saturating_sub(self.downloaded_size);
        if self.speed > 0.0 {
            remaining as f64 / self.speed
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn make_task(state: DownloadState, size: u64, progress: f32) -> DownloadTask {
        DownloadTask {
            id: uuid::Uuid::new_v4(),
            state,
            size,
            progress,
            speed: if matches!(state, DownloadState::Downloading) { 1000.0 } else { 0.0 },
            ..Default::default()
        }
    }
    
    #[test]
    fn test_aggregate_empty() {
        assert!(matches!(aggregate_batch_state(&[]), DownloadState::Queued));
    }
    
    #[test]
    fn test_aggregate_all_completed() {
        let tasks = vec![
            make_task(DownloadState::Completed, 100, 100.0),
            make_task(DownloadState::Completed, 100, 100.0),
        ];
        assert!(matches!(aggregate_batch_state(&tasks), DownloadState::Completed));
    }
    
    #[test]
    fn test_aggregate_downloading_priority() {
        let tasks = vec![
            make_task(DownloadState::Completed, 100, 100.0),
            make_task(DownloadState::Downloading, 100, 50.0),
            make_task(DownloadState::Failed, 100, 0.0),
        ];
        assert!(matches!(aggregate_batch_state(&tasks), DownloadState::Downloading));
    }
    
    #[test]
    fn test_batch_stats() {
        let tasks = vec![
            make_task(DownloadState::Completed, 1000, 100.0),
            make_task(DownloadState::Downloading, 1000, 50.0),
        ];
        let stats = BatchStats::from_tasks(&tasks);
        
        assert_eq!(stats.total_items, 2);
        assert_eq!(stats.completed_items, 1);
        assert_eq!(stats.downloading_items, 1);
        assert_eq!(stats.total_size, 2000);
        assert_eq!(stats.downloaded_size, 1500); // 1000 + 500
    }
}
