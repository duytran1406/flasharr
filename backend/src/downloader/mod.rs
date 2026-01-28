//! Download Engine Module
//!
//! Core download functionality with concurrent downloads and progress tracking.

pub mod config;
pub mod engine_simple;
pub mod error_classifier;
pub mod manager;
pub mod orchestrator;
pub mod progress;
pub mod state_machine;
pub mod stats;
pub mod task;

// Re-export commonly used types
pub use orchestrator::{DownloadOrchestrator, TmdbDownloadMetadata};
pub use stats::EngineStats;
pub use task::{DownloadTask, DownloadState};
