//! WebSocket handler for real-time updates
//!
//! Provides WebSocket endpoint for broadcasting download progress and task updates.

use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
};
use std::sync::Arc;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use crate::AppState;
use crate::downloader::{DownloadTask, EngineStats};

/// WebSocket message types sent to clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    /// Initial sync of all tasks on connection
    #[serde(rename = "SYNC_ALL")]
    SyncAll { tasks: Vec<DownloadTask> },
    
    /// Task was added
    #[serde(rename = "TASK_ADDED")]
    TaskAdded { task: DownloadTask },
    
    /// Task was updated (progress, state change)
    #[serde(rename = "TASK_UPDATED")]
    TaskUpdated { task: DownloadTask },
    
    /// Task was removed
    #[serde(rename = "TASK_REMOVED")]
    TaskRemoved { task_id: String },
    
    /// Engine statistics update
    #[serde(rename = "ENGINE_STATS")]
    EngineStats { stats: EngineStats },
}

pub async fn handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    
    tracing::debug!("WebSocket client connected");
    
    // Send initial SYNC_ALL message with only active tasks (DOWNLOADING/STARTING)
    // Historical tasks should be loaded via API with pagination
    let tasks = state.download_orchestrator.task_manager().get_active_tasks();
    let sync_msg = WsMessage::SyncAll { tasks };
    
    if let Ok(json) = serde_json::to_string(&sync_msg) {
        if sender.send(Message::Text(json)).await.is_err() {
            tracing::warn!("Failed to send SYNC_ALL to new client");
            return;
        }
        tracing::debug!("Sent SYNC_ALL to client");
    }
    
    // Subscribe to progress updates from the download engine
    let mut progress_rx = state.download_orchestrator.subscribe_progress();
    
    // Clone state for the send task
    let orchestrator = state.download_orchestrator.clone();
    
    // Spawn a task to handle incoming messages (ping/pong, client commands)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(_text) => {
                    // Future: Handle client commands if needed
                    tracing::trace!("Received text message from client");
                }
                Message::Binary(_) => {
                    tracing::trace!("Received binary message from client");
                }
                Message::Ping(_) => {
                    // Axum handles pong automatically
                }
                Message::Close(_) => {
                    tracing::debug!("Client sent close frame");
                    break;
                }
                _ => {}
            }
        }
    });

    // Handle outgoing messages (progress broadcasts)
    let mut send_task = tokio::spawn(async move {
        // Send periodic stats updates (check every 2 seconds)
        let mut stats_interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
        
        // Track last sent stats to avoid sending duplicates
        let mut last_stats: Option<EngineStats> = None;
        
        loop {
            tokio::select! {
                result = progress_rx.recv() => {
                    match result {
                        Ok(progress) => {
                            // Task was already updated by the orchestrator's progress callback
                            // Just get the task and broadcast it (no duplicate update needed)
                            let task_id = uuid::Uuid::parse_str(&progress.task_id).unwrap_or_default();
                            
                            // Get the updated task and broadcast
                            if let Some(task) = orchestrator.task_manager().get_task(task_id) {
                                let msg = WsMessage::TaskUpdated { task };
                                
                                if let Ok(json) = serde_json::to_string(&msg) {
                                    if sender.send(Message::Text(json)).await.is_err() {
                                        tracing::debug!("WebSocket client disconnected during progress update");
                                        break;
                                    }
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                            // Client is slow, some messages were dropped - this is OK
                            tracing::debug!("WebSocket client lagged, skipped {} messages", count);
                            // Continue processing - next message will be up-to-date
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            tracing::debug!("Broadcast channel closed");
                            break;
                        }
                    }
                }
                
                // Check stats periodically, but only send if changed
                _ = stats_interval.tick() => {
                    let stats = orchestrator.get_stats().await;
                    
                    // Only send if stats have changed
                    let should_send = match &last_stats {
                        None => true, // First time, always send
                        Some(prev) => prev != &stats, // Only send if different
                    };
                    
                    if should_send {
                        let msg = WsMessage::EngineStats { stats: stats.clone() };
                        
                        if let Ok(json) = serde_json::to_string(&msg) {
                            if sender.send(Message::Text(json)).await.is_err() {
                                tracing::debug!("WebSocket client disconnected during stats update");
                                break;
                            }
                        }
                        
                        // Update last sent stats
                        last_stats = Some(stats);
                    }
                }
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut recv_task) => send_task.abort(),
        _ = (&mut send_task) => recv_task.abort(),
    }
    
    tracing::debug!("WebSocket connection closed");
}
