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
