//! State Machine for Download Tasks
//!
//! Implements the State pattern for managing download task states.
//! Each state knows its valid transitions and available actions.

use super::task::DownloadState;
use std::fmt::Debug;

/// State behavior trait - defines what each state can do
pub trait TaskState: Send + Sync + Debug {
    /// Get the state enum value
    fn state_enum(&self) -> DownloadState;
    
    /// Check if can transition to target state
    fn can_transition_to(&self, target: DownloadState) -> bool;
    
    /// Get available actions in this state
    fn available_actions(&self) -> Vec<&'static str>;
    
    /// Can pause in this state?
    fn can_pause(&self) -> bool {
        false
    }
    
    /// Can resume from this state?
    fn can_resume(&self) -> bool {
        false
    }
    
    /// Can cancel in this state?
    fn can_cancel(&self) -> bool {
        false
    }
    
    /// Can retry from this state?
    fn can_retry(&self) -> bool {
        false
    }
    
    /// Can delete in this state?
    fn can_delete(&self) -> bool {
        false
    }
}

// ============================================================================
// CONCRETE STATE IMPLEMENTATIONS
// ============================================================================

/// Queued state - waiting to start
#[derive(Debug)]
pub struct QueuedState;

impl TaskState for QueuedState {
    fn state_enum(&self) -> DownloadState {
        DownloadState::Queued
    }
    
    fn can_transition_to(&self, target: DownloadState) -> bool {
        matches!(
            target,
            DownloadState::Starting
                | DownloadState::Paused
                | DownloadState::Cancelled
        )
    }
    
    fn available_actions(&self) -> Vec<&'static str> {
        vec!["pause", "cancel"]
    }
    
    fn can_pause(&self) -> bool {
        true
    }
    
    fn can_cancel(&self) -> bool {
        true
    }
    
    fn can_delete(&self) -> bool {
        true
    }
}

/// Starting state - resolving URL, preparing download
#[derive(Debug)]
pub struct StartingState;

impl TaskState for StartingState {
    fn state_enum(&self) -> DownloadState {
        DownloadState::Starting
    }
    
    fn can_transition_to(&self, target: DownloadState) -> bool {
        matches!(
            target,
            DownloadState::Downloading
                | DownloadState::Failed
                | DownloadState::Cancelled
                | DownloadState::Waiting
        )
    }
    
    fn available_actions(&self) -> Vec<&'static str> {
        vec!["cancel"]
    }
    
    fn can_cancel(&self) -> bool {
        true
    }
}

/// Downloading state - actively downloading
#[derive(Debug)]
pub struct DownloadingState;

impl TaskState for DownloadingState {
    fn state_enum(&self) -> DownloadState {
        DownloadState::Downloading
    }
    
    fn can_transition_to(&self, target: DownloadState) -> bool {
        matches!(
            target,
            DownloadState::Paused
                | DownloadState::Completed
                | DownloadState::Failed
                | DownloadState::Cancelled
                | DownloadState::Waiting
                | DownloadState::Extracting
        )
    }
    
    fn available_actions(&self) -> Vec<&'static str> {
        vec!["pause", "cancel"]
    }
    
    fn can_pause(&self) -> bool {
        true
    }
    
    fn can_cancel(&self) -> bool {
        true
    }
}

/// Paused state - paused by user
#[derive(Debug)]
pub struct PausedState;

impl TaskState for PausedState {
    fn state_enum(&self) -> DownloadState {
        DownloadState::Paused
    }
    
    fn can_transition_to(&self, target: DownloadState) -> bool {
        matches!(
            target,
            DownloadState::Queued | DownloadState::Cancelled
        )
    }
    
    fn available_actions(&self) -> Vec<&'static str> {
        vec!["resume", "cancel", "delete"]
    }
    
    fn can_resume(&self) -> bool {
        true
    }
    
    fn can_cancel(&self) -> bool {
        true
    }
    
    fn can_delete(&self) -> bool {
        true
    }
}

/// Waiting state - waiting to retry
#[derive(Debug)]
pub struct WaitingState;

impl TaskState for WaitingState {
    fn state_enum(&self) -> DownloadState {
        DownloadState::Waiting
    }
    
    fn can_transition_to(&self, target: DownloadState) -> bool {
        matches!(
            target,
            DownloadState::Queued
                | DownloadState::Starting
                | DownloadState::Paused
                | DownloadState::Failed
                | DownloadState::Cancelled
        )
    }
    
    fn available_actions(&self) -> Vec<&'static str> {
        vec!["pause", "cancel", "retry"]
    }
    
    fn can_pause(&self) -> bool {
        true
    }
    
    fn can_cancel(&self) -> bool {
        true
    }
    
    fn can_retry(&self) -> bool {
        true
    }
}

/// Completed state - download finished successfully
#[derive(Debug)]
pub struct CompletedState;

impl TaskState for CompletedState {
    fn state_enum(&self) -> DownloadState {
        DownloadState::Completed
    }
    
    fn can_transition_to(&self, target: DownloadState) -> bool {
        matches!(target, DownloadState::Queued) // Can re-download
    }
    
    fn available_actions(&self) -> Vec<&'static str> {
        vec!["delete", "retry"]
    }
    
    fn can_retry(&self) -> bool {
        true
    }
    
    fn can_delete(&self) -> bool {
        true
    }
}

/// Failed state - download failed
#[derive(Debug)]
pub struct FailedState;

impl TaskState for FailedState {
    fn state_enum(&self) -> DownloadState {
        DownloadState::Failed
    }
    
    fn can_transition_to(&self, target: DownloadState) -> bool {
        matches!(
            target,
            DownloadState::Queued | DownloadState::Cancelled
        )
    }
    
    fn available_actions(&self) -> Vec<&'static str> {
        vec!["retry", "delete", "cancel"]
    }
    
    fn can_retry(&self) -> bool {
        true
    }
    
    fn can_delete(&self) -> bool {
        true
    }
    
    fn can_cancel(&self) -> bool {
        true
    }
}

/// Cancelled state - cancelled by user
#[derive(Debug)]
pub struct CancelledState;

impl TaskState for CancelledState {
    fn state_enum(&self) -> DownloadState {
        DownloadState::Cancelled
    }
    
    fn can_transition_to(&self, target: DownloadState) -> bool {
        matches!(target, DownloadState::Queued) // Can restart
    }
    
    fn available_actions(&self) -> Vec<&'static str> {
        vec!["retry", "delete"]
    }
    
    fn can_retry(&self) -> bool {
        true
    }
    
    fn can_delete(&self) -> bool {
        true
    }
}

/// Extracting state - post-processing (extraction, etc.)
#[derive(Debug)]
pub struct ExtractingState;

impl TaskState for ExtractingState {
    fn state_enum(&self) -> DownloadState {
        DownloadState::Extracting
    }
    
    fn can_transition_to(&self, target: DownloadState) -> bool {
        matches!(
            target,
            DownloadState::Completed
                | DownloadState::Failed
                | DownloadState::Cancelled
        )
    }
    
    fn available_actions(&self) -> Vec<&'static str> {
        vec!["cancel"]
    }
    
    fn can_cancel(&self) -> bool {
        true
    }
}

/// Skipped state - skipped by user or logic
#[derive(Debug)]
pub struct SkippedState;

impl TaskState for SkippedState {
    fn state_enum(&self) -> DownloadState {
        DownloadState::Skipped
    }
    
    fn can_transition_to(&self, target: DownloadState) -> bool {
        matches!(target, DownloadState::Queued)
    }
    
    fn available_actions(&self) -> Vec<&'static str> {
        vec!["resume", "delete"]
    }
    
    fn can_resume(&self) -> bool {
        true
    }
    
    fn can_delete(&self) -> bool {
        true
    }
}

// ============================================================================
// STATE FACTORY
// ============================================================================

use once_cell::sync::Lazy;
use std::sync::Arc;

/// Factory for creating state objects (singleton pattern for memory efficiency)
pub struct TaskStateFactory;

// Pre-allocated state objects (created once, reused for all tasks)
static QUEUED: Lazy<Arc<dyn TaskState>> = Lazy::new(|| Arc::new(QueuedState));
static STARTING: Lazy<Arc<dyn TaskState>> = Lazy::new(|| Arc::new(StartingState));
static DOWNLOADING: Lazy<Arc<dyn TaskState>> = Lazy::new(|| Arc::new(DownloadingState));
static PAUSED: Lazy<Arc<dyn TaskState>> = Lazy::new(|| Arc::new(PausedState));
static WAITING: Lazy<Arc<dyn TaskState>> = Lazy::new(|| Arc::new(WaitingState));
static COMPLETED: Lazy<Arc<dyn TaskState>> = Lazy::new(|| Arc::new(CompletedState));
static FAILED: Lazy<Arc<dyn TaskState>> = Lazy::new(|| Arc::new(FailedState));
static CANCELLED: Lazy<Arc<dyn TaskState>> = Lazy::new(|| Arc::new(CancelledState));
static EXTRACTING: Lazy<Arc<dyn TaskState>> = Lazy::new(|| Arc::new(ExtractingState));
static SKIPPED: Lazy<Arc<dyn TaskState>> = Lazy::new(|| Arc::new(SkippedState));

impl TaskStateFactory {
    /// Get state object for given state enum (returns singleton)
    pub fn get_state(state: DownloadState) -> Arc<dyn TaskState> {
        match state {
            DownloadState::Queued => QUEUED.clone(),
            DownloadState::Starting => STARTING.clone(),
            DownloadState::Downloading => DOWNLOADING.clone(),
            DownloadState::Paused => PAUSED.clone(),
            DownloadState::Waiting => WAITING.clone(),
            DownloadState::Completed => COMPLETED.clone(),
            DownloadState::Failed => FAILED.clone(),
            DownloadState::Cancelled => CANCELLED.clone(),
            DownloadState::Extracting => EXTRACTING.clone(),
            DownloadState::Skipped => SKIPPED.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_queued_transitions() {
        let state = QueuedState;
        assert!(state.can_transition_to(DownloadState::Starting));
        assert!(state.can_transition_to(DownloadState::Paused));
        assert!(state.can_transition_to(DownloadState::Cancelled));
        assert!(!state.can_transition_to(DownloadState::Completed));
    }
    
    #[test]
    fn test_downloading_actions() {
        let state = DownloadingState;
        assert!(state.can_pause());
        assert!(state.can_cancel());
        assert!(!state.can_resume());
        assert!(!state.can_retry());
    }
    
    #[test]
    fn test_factory_singletons() {
        let state1 = TaskStateFactory::get_state(DownloadState::Queued);
        let state2 = TaskStateFactory::get_state(DownloadState::Queued);
        
        // Should be the same Arc (same pointer)
        assert!(Arc::ptr_eq(&state1, &state2));
    }
}
