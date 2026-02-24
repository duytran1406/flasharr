use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::downloader::task::{DownloadTask, DownloadState};

/// Events emitted by the download system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TaskEvent {
    /// Task was created and added to the system
    Created {
        task: DownloadTask,
    },
    
    /// Task state changed (e.g., QUEUED -> DOWNLOADING, DOWNLOADING -> PAUSED)
    StateChanged {
        task: DownloadTask,
        old_state: DownloadState,
        new_state: DownloadState,
        timestamp: DateTime<Utc>,
    },
    
    /// Task progress updated (download in progress)
    ProgressUpdated {
        task_id: Uuid,
        downloaded_bytes: u64,
        total_bytes: u64,
        speed_bytes_per_sec: f64,
        eta_seconds: f64,
        percentage: f64,
    },
    
    /// Task failed with error
    Failed {
        task: DownloadTask,
        error: String,
        retry_count: u32,
        timestamp: DateTime<Utc>,
    },
    
    /// Task completed successfully
    Completed {
        task: DownloadTask,
        timestamp: DateTime<Utc>,
    },
    
    /// Task was removed/deleted
    Removed {
        task_id: Uuid,
        timestamp: DateTime<Utc>,
    },
}

impl TaskEvent {
    /// Get the task ID from any event type
    pub fn task_id(&self) -> Uuid {
        match self {
            TaskEvent::Created { task } => task.id,
            TaskEvent::StateChanged { task, .. } => task.id,
            TaskEvent::ProgressUpdated { task_id, .. } => *task_id,
            TaskEvent::Failed { task, .. } => task.id,
            TaskEvent::Completed { task, .. } => task.id,
            TaskEvent::Removed { task_id, .. } => *task_id,
        }
    }
    
    /// Get the event type as a string
    pub fn event_type(&self) -> &'static str {
        match self {
            TaskEvent::Created { .. } => "CREATED",
            TaskEvent::StateChanged { .. } => "STATE_CHANGED",
            TaskEvent::ProgressUpdated { .. } => "PROGRESS_UPDATED",
            TaskEvent::Failed { .. } => "FAILED",
            TaskEvent::Completed { .. } => "COMPLETED",
            TaskEvent::Removed { .. } => "REMOVED",
        }
    }
}

/// Event bus for publishing and subscribing to task events
pub struct EventBus {
    sender: tokio::sync::broadcast::Sender<TaskEvent>,
}

impl EventBus {
    /// Create a new event bus with the specified capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(capacity);
        Self { sender }
    }
    
    /// Publish an event to all subscribers
    pub fn publish(&self, event: TaskEvent) {
        let event_type = event.event_type();
        let task_id = event.task_id();
        
        match self.sender.send(event) {
            Ok(subscriber_count) => {
                tracing::trace!(
                    "Published {} event for task {} to {} subscribers",
                    event_type,
                    task_id,
                    subscriber_count
                );
            }
            Err(_) => {
                tracing::warn!(
                    "No subscribers for {} event (task {})",
                    event_type,
                    task_id
                );
            }
        }
    }
    
    /// Subscribe to events
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<TaskEvent> {
        self.sender.subscribe()
    }
    
    /// Get the number of active subscribers
    #[allow(dead_code)] // Useful for monitoring and debugging
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let bus = EventBus::new(100);
        let mut rx = bus.subscribe();
        
        let task_id = Uuid::new_v4();
        bus.publish(TaskEvent::ProgressUpdated {
            task_id,
            downloaded_bytes: 1000,
            total_bytes: 10000,
            speed_bytes_per_sec: 100.0,
            eta_seconds: 90.0,
            percentage: 10.0,
        });
        
        let event = rx.recv().await.unwrap();
        assert_eq!(event.task_id(), task_id);
        assert_eq!(event.event_type(), "PROGRESS_UPDATED");
    }
}
