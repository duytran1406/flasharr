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
    response::IntoResponse,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    config: Option<SabConfig>,
}

#[derive(Serialize)]
struct SabFullStatusResponse {
    #[serde(flatten)]
    queue: SabQueueData,
}

#[derive(Serialize)]
struct SabConfig {
    version: String,
    paused: bool,
    #[serde(rename = "pause_int")]
    pause_int: String,
    #[serde(rename = "download_dir")]
    download_dir: String,
    #[serde(rename = "complete_dir")]
    complete_dir: String,
    #[serde(rename = "nzb_backup_dir")]
    nzb_backup_dir: String,
    #[serde(rename = "script_dir")]
    script_dir: String,
    categories: Vec<SabCategory>,
    misc: SabMisc,
}

#[derive(Serialize)]
struct SabMisc {
    #[serde(rename = "queue_complete")]
    queue_complete: String,
    #[serde(rename = "refresh_rate")]
    refresh_rate: i32,
    #[serde(rename = "bandwidth_limit")]
    bandwidth_limit: String,
}

#[derive(Serialize)]
struct SabCategory {
    name: String,
    dir: String,
    newzbin: String,
    priority: i32,
}

#[derive(Serialize)]
struct SabQueue {
    paused: bool,
    status: String,
    noofslots: usize,
    slots: Vec<SabQueueSlot>,
    speed: String,
    size: String,
    sizeleft: String,
}

#[derive(Serialize)]
struct SabQueueData {
    paused: bool,
    noofslots: usize,
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
) -> axum::response::Response {
    let mode = params.mode.as_deref().unwrap_or("queue");
    
    tracing::info!("SABnzbd API request - mode: {}, apikey: {:?}", mode, params.apikey);

    let result = match mode {
        "addurl" => handle_add_url(state, params).await,
        "queue" => handle_queue(state).await,
        "fullstatus" => handle_fullstatus(state).await,
        "history" => handle_history(state).await,
        "version" => handle_version().await,
        "get_config" => handle_get_config().await,
        "pause" => handle_pause(state, params).await,
        "resume" => handle_resume(state, params).await,
        "delete" => handle_delete(state, params).await,
        _ => {
            tracing::warn!("Unknown SABnzbd mode: {}", mode);
            Ok(Json(serde_json::json!({ "status": false, "error": "Unknown mode" })))
        }
    };

    match result {
        Ok(json) => (StatusCode::OK, json).into_response(),
        Err(status) => status.into_response(),
    }
}

/// Handle POST requests (form data)
async fn handle_post(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SabParams>,
) -> axum::response::Response {
    handle_get(State(state), Query(params)).await
}

/// Add URL to download queue
async fn handle_add_url(
    state: Arc<AppState>,
    params: SabParams,
) -> Result<Json<serde_json::Value>, StatusCode> {
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
    
    Ok(Json(serde_json::json!({
        "status": true,
        "nzo_ids": [task.id.to_string()]
    })))
}

/// Get queue status
async fn handle_queue(state: Arc<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
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
            total_speed += t.speed as f64;
            total_size += t.size;
            let downloaded = ((t.progress as f64) / 100.0 * t.size as f64) as u64;
            let left = t.size.saturating_sub(downloaded);
            total_left += left;
            
            let speed = t.speed as f64;
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
    
    Ok(Json(serde_json::json!({
        "queue": {
            "paused": false,
            "status": if slots.is_empty() { "Idle" } else { "Downloading" },
            "noofslots": slots.len(),
            "slots": slots,
            "speed": format!("{:.2} MB/s", total_speed / 1_048_576.0),
            "size": format!("{:.2} MB", total_size as f64 / 1_048_576.0),
            "sizeleft": format!("{:.2} MB", total_left as f64 / 1_048_576.0),
        }
    })))
}

/// Get full status (flattened queue response for *arr compatibility)
async fn handle_fullstatus(state: Arc<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let tasks = state.download_orchestrator.task_manager().get_tasks();
    
    let mut total_speed = 0.0;
    let mut total_size = 0u64;
    let mut total_left = 0u64;
    
    let slots: Vec<SabQueueSlot> = tasks
        .iter()
        .filter(|t| matches!(t.state, crate::downloader::DownloadState::Downloading | crate::downloader::DownloadState::Queued))
        .map(|t| {
            total_speed += t.speed as f64;
            total_size += t.size;
            let downloaded = ((t.progress as f64) / 100.0 * t.size as f64) as u64;
            let left = t.size.saturating_sub(downloaded);
            total_left += left;
            
            let speed = t.speed as f64;
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
    
    Ok(Json(serde_json::json!({
        "status": {
            "state": if slots.is_empty() { "Idle" } else { "Downloading" },
            "paused": false,
            "noofslots": slots.len(),
            "slots": slots,
            "speed": format!("{:.2} MB/s", total_speed / 1_048_576.0),
            "size": format!("{:.2} MB", total_size as f64 / 1_048_576.0),
            "sizeleft": format!("{:.2} MB", total_left as f64 / 1_048_576.0),
        }
    })))
}


/// Get history
async fn handle_history(state: Arc<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
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
    
    Ok(Json(serde_json::json!({
        "history": {
            "slots": slots
        }
    })))
}

/// Get version
async fn handle_version() -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(serde_json::json!({
        "version": "3.5.0"
    })))
}

/// Get config (for *arr compatibility testing)
async fn handle_get_config() -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(serde_json::json!({
        "config": {
            "version": "3.5.0",
            "paused": false,
            "pause_int": "0",
            "download_dir": "/appData/downloads/incomplete",
            "complete_dir": "/appData/downloads",
            "nzb_backup_dir": "/appData/nzb_backup",
            "script_dir": "/appData/scripts",
            "categories": [
                {
                    "name": "tv",
                    "dir": "TV",
                    "newzbin": "",
                    "priority": 0
                },
                {
                    "name": "movies",
                    "dir": "Movies",
                    "newzbin": "",
                    "priority": 0
                },
                {
                    "name": "*",
                    "dir": "",
                    "newzbin": "",
                    "priority": 0
                }
            ],
            "misc": {
                "queue_complete": "",
                "refresh_rate": 2,
                "bandwidth_limit": ""
            }
        }
    })))
}

/// Pause a download
async fn handle_pause(
    state: Arc<AppState>,
    params: SabParams,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Some(nzo_id) = params.nzo_id {
        if let Ok(uuid) = uuid::Uuid::parse_str(&nzo_id) {
            state.download_orchestrator.task_manager().pause_task(uuid);
        }
    }
    
    Ok(Json(serde_json::json!({ "status": true })))
}

/// Resume a download
async fn handle_resume(
    state: Arc<AppState>,
    params: SabParams,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Some(nzo_id) = params.nzo_id {
        if let Ok(uuid) = uuid::Uuid::parse_str(&nzo_id) {
            state.download_orchestrator.task_manager().resume_task(uuid);
        }
    }
    
    Ok(Json(serde_json::json!({ "status": true })))
}

/// Delete a download
async fn handle_delete(
    state: Arc<AppState>,
    params: SabParams,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Some(nzo_id) = params.nzo_id {
        if let Ok(uuid) = uuid::Uuid::parse_str(&nzo_id) {
            state.download_orchestrator.task_manager().delete_task(uuid);
        }
    }
    
    Ok(Json(serde_json::json!({ "status": true })))
}
