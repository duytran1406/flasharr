//! Download Engine Module
//!
//! Core download functionality with concurrent downloads and progress tracking.

pub mod config;
pub mod duplicate_detector;
pub mod engine_simple;
pub mod error_classifier;
pub mod events;
// pub mod import_lifecycle_tests;
pub mod manager;
pub mod orchestrator;
// pub mod orchestrator_error_tests;
pub mod path_builder;
pub mod progress;
// pub mod restart_recovery_tests;
pub mod state_machine;
pub mod stats;
pub mod task;

// Re-export commonly used types
pub use orchestrator::DownloadOrchestrator;
pub use path_builder::{PathBuilder, TmdbDownloadMetadata};
pub use stats::EngineStats;
pub use task::{DownloadState, DownloadTask, MediaType};
