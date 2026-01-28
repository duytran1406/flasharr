//! Download Task
//!
//! Represents a single download task with state management.

use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use super::state_machine::{TaskState, TaskStateFactory};
use super::error_classifier::{ErrorClassifier, ErrorCategory};

/// Download task state
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DownloadState {
    /// Waiting in queue
    Queued,
    /// Starting download (resolving URL, etc.)
    Starting,
    /// Actively downloading
    Downloading,
    /// Paused by user
    Paused,
    /// Waiting for retry (rate limit, server busy)
    Waiting,
    /// Successfully completed
    Completed,
    /// Failed with error
    Failed,
    /// Cancelled by user
    Cancelled,
    /// Post-processing (extraction, etc.)
    Extracting,
    /// Skipped by user or logic
    Skipped,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self::Queued
    }
}

/// Capability matrix for task actions
impl DownloadState {
    /// Check if pause action is available
    pub fn can_pause(&self) -> bool {
        matches!(self, Self::Queued | Self::Downloading | Self::Waiting)
    }
    
    /// Check if resume action is available
    pub fn can_resume(&self) -> bool {
        matches!(self, Self::Paused | Self::Waiting | Self::Skipped)
    }
    
    /// Check if cancel action is available
    pub fn can_cancel(&self) -> bool {
        matches!(self, 
            Self::Queued | Self::Starting | Self::Downloading | 
            Self::Waiting | Self::Paused | Self::Extracting
        )
    }
    
    /// Check if retry action is available
    pub fn can_retry(&self) -> bool {
        matches!(self, 
            Self::Waiting | Self::Completed | Self::Failed | 
            Self::Cancelled | Self::Skipped
        )
    }
    
    /// Check if delete action is available
    pub fn can_delete(&self) -> bool {
        matches!(self, 
            Self::Queued | Self::Paused | Self::Completed | 
            Self::Failed | Self::Cancelled | Self::Skipped
        )
    }
    
    /// Get list of available actions
    pub fn available_actions(&self) -> Vec<&'static str> {
        let mut actions = Vec::new();
        if self.can_pause() { actions.push("pause"); }
        if self.can_resume() { actions.push("resume"); }
        if self.can_cancel() { actions.push("cancel"); }
        if self.can_retry() { actions.push("retry"); }
        if self.can_delete() { actions.push("delete"); }
        actions
    }
}

/// Download task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    /// Unique task ID
    pub id: Uuid,
    
    /// Download URL (may be direct or needs resolution)
    pub url: String,
    
    /// Original URL (e.g., Fshare page URL)
    pub original_url: String,
    
    /// Target filename
    pub filename: String,
    
    /// Destination path
    pub destination: String,
    
    /// Current state
    pub state: DownloadState,
    
    /// Progress percentage (0.0 to 100.0)
    pub progress: f32,
    
    /// Total file size in bytes
    pub size: u64,
    
    /// Bytes downloaded so far
    #[serde(default)]
    pub downloaded: u64,
    
    /// Current download speed in bytes/sec
    #[serde(default)]
    pub speed: f64,
    
    /// Estimated time remaining in seconds
    #[serde(default)]
    pub eta: f64,
    
    /// Host identifier (e.g., "fshare", "gdrive")
    pub host: String,
    
    /// Category (e.g., "tv", "movies", "other")
    pub category: String,
    
    /// Priority (0 = normal, higher = more priority)
    pub priority: i32,
    
    /// Number of segments for this download
    pub segments: u32,
    
    /// Retry count
    pub retry_count: u32,
    
    /// Time when task was created
    pub created_at: DateTime<Utc>,
    
    /// Time when download started
    pub started_at: Option<DateTime<Utc>>,
    
    /// Time when download completed/failed
    pub completed_at: Option<DateTime<Utc>>,
    
    /// Wait until this time before retrying
    pub wait_until: Option<DateTime<Utc>>,
    
    /// Error message if failed
    pub error_message: Option<String>,
    
    /// URL metadata for expiration tracking
    pub url_metadata: Option<UrlMetadata>,
    
    /// Error history for debugging
    #[serde(default)]
    pub error_history: Vec<ErrorRecord>,
    
    /// Fshare file code for duplicate detection (e.g., "8DW6WQOV5R551DL")
    pub fshare_code: Option<String>,
    
    /// State machine object (not serialized)
    #[serde(skip, default = "default_state_obj")]
    pub state_obj: Arc<dyn TaskState>,
    
    /// Cancellation token (not serialized)
    #[serde(skip)]
    pub cancel_token: CancellationToken,
    
    /// Pause notification (not serialized)
    #[serde(skip)]
    pub pause_notify: Arc<Notify>,
}

/// Error record for tracking error history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    pub timestamp: DateTime<Utc>,
    pub error_message: String,
    pub error_category: String,
    pub retry_attempt: u32,
}

/// URL metadata for tracking expiration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlMetadata {
    /// When the URL was resolved
    pub resolved_at: DateTime<Utc>,
    
    /// When the URL expires
    pub expires_at: DateTime<Utc>,
}

/// Default state object for deserialization
fn default_state_obj() -> Arc<dyn TaskState> {
    TaskStateFactory::get_state(DownloadState::Queued)
}

impl DownloadTask {
    /// Create a new download task
    pub fn new(url: String, filename: String, host: String, category: String) -> Self {
        let state = DownloadState::Queued;
        Self {
            id: Uuid::new_v4(),
            original_url: url.clone(),
            url,
            filename,
            destination: "/downloads".to_string(),
            state,
            progress: 0.0,
            size: 0,
            downloaded: 0,
            speed: 0.0,
            eta: 0.0,
            host,
            category,
            priority: 0,
            segments: 4,
            retry_count: 0,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            wait_until: None,
            error_message: None,
            url_metadata: None,
            error_history: Vec::new(),
            fshare_code: None,
            state_obj: TaskStateFactory::get_state(state),
            cancel_token: CancellationToken::new(),
            pause_notify: Arc::new(Notify::new()),
        }
    }
    
    /// Transition to new state with validation
    pub fn transition_to(&mut self, new_state: DownloadState) -> Result<(), String> {
        if !self.state_obj.can_transition_to(new_state) {
            return Err(format!(
                "Invalid transition from {:?} to {:?}",
                self.state, new_state
            ));
        }
        
        self.state = new_state;
        self.state_obj = TaskStateFactory::get_state(new_state);
        
        Ok(())
    }
    
    /// Handle error and return recovery action
    pub fn on_error(&mut self, error: &anyhow::Error) -> ErrorCategory {
        let category = ErrorClassifier::classify(error);
        
        // Log error to history
        self.error_history.push(ErrorRecord {
            timestamp: Utc::now(),
            error_message: error.to_string(),
            error_category: format!("{:?}", category),
            retry_attempt: self.retry_count,
        });
        
        category
    }
    
    /// Check if task is cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }
    
    /// Cancel the task
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }
    
    /// Get available actions for current state
    pub fn get_available_actions(&self) -> Vec<&'static str> {
        self.state_obj.available_actions()
    }
}

impl Default for DownloadTask {
    fn default() -> Self {
        Self::new(
            String::new(),
            String::from("unknown"),
            String::from("unknown"),
            String::from("other"),
        )
    }
}
