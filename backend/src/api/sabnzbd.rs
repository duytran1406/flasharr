//! SABnzbd Compatibility API
//!
//! Provides SABnzbd-compatible endpoints for integration with Sonarr/Radarr.
//! This allows *arr applications to use Flasharr as a download client.

use axum::{
    routing::{get, post},
    Router,
    Json,
    extract::{State, Query},
    http::StatusCode,
};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::AppState;
use crate::downloader::DownloadState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(handle_get))
        .route("/", post(handle_post))
        .route("/api", get(handle_get))
        .route("/api", post(handle_post))
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Deserialize)]
struct SabParams {
    mode: Option<String>,
    #[allow(dead_code)]
    output: Option<String>,
    #[allow(dead_code)]
    apikey: Option<String>,
    name: Option<String>,      // URL to download
    nzbname: Option<String>,   // Filename
    cat: Option<String>,       // Category
    #[allow(dead_code)]
    priority: Option<i32>,
    nzo_id: Option<String>,    // Task ID for operations
}

#[derive(Serialize)]
struct SabResponse {
    status: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    nzo_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    queue: Option<SabQueue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    history: Option<SabHistory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
}

#[derive(Serialize)]
struct SabQueue {
    slots: Vec<SabQueueSlot>,
    speed: String,
    size: String,
    sizeleft: String,
}

#[derive(Serialize)]
struct SabQueueSlot {
    nzo_id: String,
    filename: String,
    percentage: String,
    mb: String,
    mbleft: String,
    status: String,
    timeleft: String,
}

#[derive(Serialize)]
struct SabHistory {
    slots: Vec<SabHistorySlot>,
}

#[derive(Serialize)]
struct SabHistorySlot {
    nzo_id: String,
    name: String,
    status: String,
    fail_message: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// Handle GET requests (query params)
async fn handle_get(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SabParams>,
) -> Result<Json<SabResponse>, StatusCode> {
    handle_request(state, params).await
}

/// Handle POST requests (form data)
async fn handle_post(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SabParams>,
) -> Result<Json<SabResponse>, StatusCode> {
    handle_request(state, params).await
}

async fn handle_request(
    state: Arc<AppState>,
    params: SabParams,
) -> Result<Json<SabResponse>, StatusCode> {
    let mode = params.mode.as_deref().unwrap_or("queue");
    
    match mode {
        "addurl" => handle_add_url(state, params).await,
        "queue" => handle_queue(state).await,
        "history" => handle_history(state).await,
        "version" => handle_version().await,
        "pause" => handle_pause(state, params).await,
        "resume" => handle_resume(state, params).await,
        "delete" => handle_delete(state, params).await,
        _ => {
            tracing::warn!("Unknown SABnzbd mode: {}", mode);
            Ok(Json(SabResponse {
                status: false,
                nzo_ids: None,
                queue: None,
                history: None,
                version: None,
            }))
        }
    }
}

/// Add URL to download queue
async fn handle_add_url(
    state: Arc<AppState>,
    params: SabParams,
) -> Result<Json<SabResponse>, StatusCode> {
    let url = params.name.ok_or(StatusCode::BAD_REQUEST)?;
    let filename = params.nzbname.unwrap_or_else(|| "download".to_string());
    let category = params.cat.unwrap_or_else(|| "other".to_string());
    
    tracing::info!("SABnzbd: Adding download - URL: {}, filename: {}", url, filename);
    
    let task = state.download_orchestrator.add_download(
        url,
        filename,
        "fshare".to_string(),
        category,
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(SabResponse {
        status: true,
        nzo_ids: Some(vec![task.id.to_string()]),
        queue: None,
        history: None,
        version: None,
    }))
}

/// Get queue status
async fn handle_queue(state: Arc<AppState>) -> Result<Json<SabResponse>, StatusCode> {
    let tasks = state.download_orchestrator.task_manager().get_tasks();
    
    let mut total_speed = 0.0;
    let mut total_size = 0u64;
    let mut total_left = 0u64;
    
    let slots: Vec<SabQueueSlot> = tasks.iter()
        .filter(|t| matches!(t.state, 
            DownloadState::Queued | 
            DownloadState::Starting | 
            DownloadState::Downloading |
            DownloadState::Waiting
        ))
        .map(|t| {
            let downloaded = ((t.progress as f64 / 100.0) * t.size as f64) as u64;
            let left = t.size.saturating_sub(downloaded);
            
            total_size += t.size;
            total_left += left;
            
            // Get speed from progress (if available)
            // For now, we'll use a simple calculation
            let speed = if t.state == DownloadState::Downloading {
                // This would come from progress tracking in real implementation
                0.0
            } else {
                0.0
            };
            total_speed += speed;
            
            let eta = if speed > 0.0 {
                format!("{}s", (left as f64 / speed) as u64)
            } else {
                "Unknown".to_string()
            };
            
            SabQueueSlot {
                nzo_id: t.id.to_string(),
                filename: t.filename.clone(),
                percentage: format!("{:.1}", t.progress),
                mb: format!("{:.2}", t.size as f64 / 1_048_576.0),
                mbleft: format!("{:.2}", left as f64 / 1_048_576.0),
                status: format!("{:?}", t.state),
                timeleft: eta,
            }
        })
        .collect();
    
    Ok(Json(SabResponse {
        status: true,
        nzo_ids: None,
        queue: Some(SabQueue {
            slots,
            speed: format!("{:.2} MB/s", total_speed / 1_048_576.0),
            size: format!("{:.2} MB", total_size as f64 / 1_048_576.0),
            sizeleft: format!("{:.2} MB", total_left as f64 / 1_048_576.0),
        }),
        history: None,
        version: None,
    }))
}

/// Get history
async fn handle_history(state: Arc<AppState>) -> Result<Json<SabResponse>, StatusCode> {
    let tasks = state.download_orchestrator.task_manager().get_tasks();
    
    let slots: Vec<SabHistorySlot> = tasks.iter()
        .filter(|t| matches!(t.state, 
            DownloadState::Completed | 
            DownloadState::Failed |
            DownloadState::Cancelled
        ))
        .map(|t| {
            let status = match t.state {
                DownloadState::Completed => "Completed",
                DownloadState::Failed => "Failed",
                DownloadState::Cancelled => "Failed",
                _ => "Unknown",
            };
            
            SabHistorySlot {
                nzo_id: t.id.to_string(),
                name: t.filename.clone(),
                status: status.to_string(),
                fail_message: t.error_message.clone().unwrap_or_default(),
            }
        })
        .collect();
    
    Ok(Json(SabResponse {
        status: true,
        nzo_ids: None,
        queue: None,
        history: Some(SabHistory { slots }),
        version: None,
    }))
}

/// Get version
async fn handle_version() -> Result<Json<SabResponse>, StatusCode> {
    Ok(Json(SabResponse {
        status: true,
        nzo_ids: None,
        queue: None,
        history: None,
        version: Some("3.0.0".to_string()),
    }))
}

/// Pause a download
async fn handle_pause(
    state: Arc<AppState>,
    params: SabParams,
) -> Result<Json<SabResponse>, StatusCode> {
    if let Some(nzo_id) = params.nzo_id {
        if let Ok(uuid) = uuid::Uuid::parse_str(&nzo_id) {
            state.download_orchestrator.task_manager().pause_task(uuid);
        }
    }
    
    Ok(Json(SabResponse {
        status: true,
        nzo_ids: None,
        queue: None,
        history: None,
        version: None,
    }))
}

/// Resume a download
async fn handle_resume(
    state: Arc<AppState>,
    params: SabParams,
) -> Result<Json<SabResponse>, StatusCode> {
    if let Some(nzo_id) = params.nzo_id {
        if let Ok(uuid) = uuid::Uuid::parse_str(&nzo_id) {
            state.download_orchestrator.task_manager().resume_task(uuid);
        }
    }
    
    Ok(Json(SabResponse {
        status: true,
        nzo_ids: None,
        queue: None,
        history: None,
        version: None,
    }))
}

/// Delete a download
async fn handle_delete(
    state: Arc<AppState>,
    params: SabParams,
) -> Result<Json<SabResponse>, StatusCode> {
    if let Some(nzo_id) = params.nzo_id {
        if let Ok(uuid) = uuid::Uuid::parse_str(&nzo_id) {
            state.download_orchestrator.task_manager().delete_task(uuid);
        }
    }
    
    Ok(Json(SabResponse {
        status: true,
        nzo_ids: None,
        queue: None,
        history: None,
        version: None,
    }))
}
