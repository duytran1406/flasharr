//! Download Engine Statistics
//!
//! Provides statistics and metrics for the download engine.

use serde::{Deserialize, Serialize};

/// Database-sourced status counts (for filter dropdown)
/// These are total counts from the database, not just in-memory tasks
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DbStatusCounts {
    pub all: usize,
    pub downloading: usize,
    pub queued: usize,
    pub paused: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
}

/// Engine statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStats {
    /// In-memory active task counts (real-time)
    pub active_downloads: usize,
    pub queued: usize,
    pub completed: usize,
    pub failed: usize,
    pub paused: usize,
    pub cancelled: usize,
    pub total_speed: f64,
    
    /// Database-sourced status counts (for filter dropdown)
    /// Optional to allow backwards compatibility during rollout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_counts: Option<DbStatusCounts>,
}

impl PartialEq for EngineStats {
    fn eq(&self, other: &Self) -> bool {
        self.active_downloads == other.active_downloads
            && self.queued == other.queued
            && self.completed == other.completed
            && self.failed == other.failed
            && self.paused == other.paused
            && self.cancelled == other.cancelled
            && (self.total_speed - other.total_speed).abs() < 0.01 // Consider speeds within 0.01 B/s as equal
            && self.db_counts == other.db_counts
    }
}

impl Default for EngineStats {
    fn default() -> Self {
        Self {
            active_downloads: 0,
            queued: 0,
            completed: 0,
            failed: 0,
            paused: 0,
            cancelled: 0,
            total_speed: 0.0,
            db_counts: None,
        }
    }
}
