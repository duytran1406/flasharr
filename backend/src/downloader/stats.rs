//! Download Engine Statistics
//!
//! Provides statistics and metrics for the download engine.

use serde::{Deserialize, Serialize};

/// Engine statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStats {
    pub active_downloads: usize,
    pub queued: usize,
    pub completed: usize,
    pub failed: usize,
    pub total_speed: f64,
}

impl PartialEq for EngineStats {
    fn eq(&self, other: &Self) -> bool {
        self.active_downloads == other.active_downloads
            && self.queued == other.queued
            && self.completed == other.completed
            && self.failed == other.failed
            && (self.total_speed - other.total_speed).abs() < 0.01 // Consider speeds within 0.01 B/s as equal
    }
}

impl Default for EngineStats {
    fn default() -> Self {
        Self {
            active_downloads: 0,
            queued: 0,
            completed: 0,
            failed: 0,
            total_speed: 0.0,
        }
    }
}
