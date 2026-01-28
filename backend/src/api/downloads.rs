//! Downloads API Routes
//!
//! REST endpoints for managing download tasks.

use axum::{
    routing::{get, post, delete},
    Router,
    Json,
    extract::{State, Path, Query},
    http::StatusCode,
};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::AppState;
use crate::downloader::{DownloadTask, DownloadState, EngineStats};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_downloads))
        .route("/", post(add_download))
        .route("/:id", get(get_download))
        .route("/:id", delete(delete_download))
        .route("/:id/pause", post(pause_download))
        .route("/:id/resume", post(resume_download))
        .route("/:id/retry", post(retry_download))
        .route("/pause-all", post(pause_all))
        .route("/resume-all", post(resume_all))
        .route("/stats", get(get_stats))
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Serialize)]
struct DownloadsResponse {
    downloads: Vec<DownloadTask>,
    stats: EngineStats,
    /// Pagination info
    total: u64,
    page: u32,
    limit: u32,
    total_pages: u32,
}

/// Query parameters for list downloads
#[derive(Debug, Deserialize)]
struct ListDownloadsQuery {
    /// Page number (1-indexed, default 1)
    page: Option<u32>,
    /// Items per page (default 20, max 100)
    limit: Option<u32>,
}

#[derive(Serialize)]
struct AddDownloadResponse {
    success: bool,
    task_id: String,
    filename: String,
    state: String,
}

#[derive(Serialize)]
struct ActionResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Serialize)]
struct BulkActionResponse {
    success: bool,
    affected: usize,
}

// ============================================================================
// Request Types
// ============================================================================

/// TMDB Metadata for organized folder structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TmdbMetadata {
    /// TMDB ID
    pub tmdb_id: Option<i64>,
    /// Media type: "movie" or "tv"
    pub media_type: Option<String>,
    /// Movie/Show title
    pub title: Option<String>,
    /// Release year
    pub year: Option<String>,
    /// Collection name (for movies in a collection)
    pub collection_name: Option<String>,
    /// Season number (for TV)
    pub season: Option<i32>,
    /// Episode number (for TV)
    pub episode: Option<i32>,
}

#[derive(Deserialize)]
pub struct AddDownloadRequest {
    pub url: String,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub priority: Option<String>,
    /// TMDB metadata for folder organization
    #[serde(default)]
    pub tmdb: Option<TmdbMetadata>,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/downloads - List downloads with pagination
/// 
/// Query params:
/// - page: Page number (1-indexed, default 1)
/// - limit: Items per page (default 20, max 100)
async fn list_downloads(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListDownloadsQuery>,
) -> Json<DownloadsResponse> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).min(100).max(1);
    
    // Query from database with pagination (async to prevent blocking)
    let (mut downloads, total) = state.db
        .get_tasks_paginated_async(page, limit).await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to get tasks from DB: {}", e);
            (Vec::new(), 0)
        });
    
    // Merge real-time progress from active downloads in memory
    let active_tasks = state.download_orchestrator.task_manager().get_active_tasks();
    for download in downloads.iter_mut() {
        if let Some(active) = active_tasks.iter().find(|t| t.id == download.id) {
            // Update with real-time data from memory
            download.progress = active.progress;
            download.speed = active.speed;
            download.eta = active.eta;
            download.downloaded = active.downloaded;
            download.state = active.state;
        }
    }
    
    let stats = state.download_orchestrator.task_manager().get_stats();
    let total_pages = ((total as f64) / (limit as f64)).ceil() as u32;
    
    Json(DownloadsResponse { 
        downloads, 
        stats,
        total,
        page,
        limit,
        total_pages,
    })
}

/// GET /api/downloads/:id - Get single download
async fn get_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<DownloadTask>, StatusCode> {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    state.download_orchestrator.task_manager()
        .get_task(uuid)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// POST /api/downloads - Add new download
async fn add_download(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AddDownloadRequest>,
) -> Json<AddDownloadResponse> {
    // === DEBUG: Log incoming request ===
    tracing::info!("=== [API] add_download called ===");
    tracing::info!("[API] Incoming URL: {}", payload.url);
    tracing::info!("[API] Incoming filename: {:?}", payload.filename);
    tracing::info!("[API] Incoming category: {:?}", payload.category);
    if let Some(ref tmdb) = payload.tmdb {
        tracing::info!("[API] TMDB metadata: tmdb_id={:?}, media_type={:?}, title={:?}, year={:?}, season={:?}, episode={:?}",
            tmdb.tmdb_id, tmdb.media_type, tmdb.title, tmdb.year, tmdb.season, tmdb.episode);
    } else {
        tracing::info!("[API] No TMDB metadata provided");
    }
    
    // Category from payload or derive from TMDB metadata
    let category = payload.category.unwrap_or_else(|| {
        if let Some(ref tmdb) = payload.tmdb {
            tmdb.media_type.clone().unwrap_or_else(|| "other".to_string())
        } else {
            "other".to_string()
        }
    });
    
    // Convert TMDB metadata to orchestrator format
    let tmdb_metadata = payload.tmdb.map(|t| crate::downloader::TmdbDownloadMetadata {
        tmdb_id: t.tmdb_id,
        media_type: t.media_type,
        title: t.title,
        year: t.year,
        collection_name: t.collection_name,
        season: t.season,
        episode: t.episode,
    });
    
    let task = match state.download_orchestrator.add_download_with_metadata(
        payload.url,
        payload.filename,
        "fshare".to_string(),
        category,
        tmdb_metadata,
    ).await {
        Ok(task) => task,
        Err(e) => {
            tracing::error!("Failed to add download: {}", e);
            return Json(AddDownloadResponse {
                success: false,
                task_id: String::new(),
                filename: String::new(),
                state: format!("Failed: {}", e),
            });
        }
    };
    
    Json(AddDownloadResponse {
        success: true,
        task_id: task.id.to_string(),
        filename: task.filename.clone(),
        state: "Queued".to_string(),
    })
}

/// DELETE /api/downloads/:id - Delete download
async fn delete_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse>, StatusCode> {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Delete from in-memory task manager
    let success = state.download_orchestrator.task_manager().delete_task(uuid);
    
    // Also delete from database
    if success {
        if let Err(e) = state.db.delete_task(uuid) {
            tracing::warn!("Failed to delete task from database: {}", e);
        }
    }
    
    Ok(Json(ActionResponse {
        success,
        message: if success { None } else { Some("Task not found".to_string()) },
    }))
}

/// POST /api/downloads/:id/pause - Pause download
async fn pause_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse>, StatusCode> {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let task_result = state.download_orchestrator.task_manager().pause_task(uuid);
    let success = task_result.is_some();
    
    // Broadcast state change if successful
    if let Some(task) = task_result {
        state.download_orchestrator.broadcast_task_update(&task);
    }
    
    Ok(Json(ActionResponse {
        success,
        message: if !success { 
            Some("Task not found or cannot be paused".to_string()) 
        } else { 
            None 
        },
    }))
}

/// POST /api/downloads/:id/resume - Resume paused download
async fn resume_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse>, StatusCode> {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let task_result = state.download_orchestrator.task_manager().resume_task(uuid);
    let success = task_result.is_some();
    
    // Broadcast state change if successful
    if let Some(task) = task_result {
        state.download_orchestrator.broadcast_task_update(&task);
    }
    
    Ok(Json(ActionResponse {
        success,
        message: if !success { 
            Some("Task not found or not paused".to_string()) 
        } else { 
            None 
        },
    }))
}

/// POST /api/downloads/:id/retry - Retry failed download
async fn retry_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse>, StatusCode> {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // For retry, we handle failed/cancelled tasks
    let success = state.download_orchestrator.task_manager().retry_task(uuid).is_some();
    
    Ok(Json(ActionResponse {
        success,
        message: if !success { 
            Some("Task not found or cannot be retried".to_string()) 
        } else { 
            None 
        },
    }))
}

/// POST /api/downloads/pause-all - Pause all active downloads
async fn pause_all(
    State(state): State<Arc<AppState>>,
) -> Json<BulkActionResponse> {
    let tasks = state.download_orchestrator.task_manager().get_tasks();
    let mut affected = 0;
    
    for task in tasks {
        // Only pause tasks that are downloading or queued
        if matches!(task.state, DownloadState::Downloading | DownloadState::Queued | DownloadState::Starting) {
            if let Some(paused_task) = state.download_orchestrator.task_manager().pause_task(task.id) {
                // Broadcast the state change via WebSocket
                state.download_orchestrator.broadcast_task_update(&paused_task);
                affected += 1;
            }
        }
    }
    
    Json(BulkActionResponse {
        success: true,
        affected,
    })
}

/// POST /api/downloads/resume-all - Resume all paused downloads
async fn resume_all(
    State(state): State<Arc<AppState>>,
) -> Json<BulkActionResponse> {
    let tasks = state.download_orchestrator.task_manager().get_tasks();
    let mut affected = 0;
    
    for task in tasks {
        // Resume paused tasks OR retry failed/cancelled tasks
        if matches!(task.state, DownloadState::Paused) || task.state.can_retry() {
            // For Paused, use resume; for others, use retry
            if task.state == DownloadState::Paused {
                if let Some(resumed_task) = state.download_orchestrator.task_manager().resume_task(task.id) {
                    state.download_orchestrator.broadcast_task_update(&resumed_task);
                    affected += 1;
                }
            } else {
                if let Some(retried_task) = state.download_orchestrator.task_manager().retry_task(task.id) {
                    state.download_orchestrator.broadcast_task_update(&retried_task);
                    affected += 1;
                }
            }
        }
    }
    
    Json(BulkActionResponse {
        success: true,
        affected,
    })
}

/// GET /api/downloads/stats - Get engine statistics
async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Json<EngineStats> {
    Json(state.download_orchestrator.task_manager().get_stats())
}
