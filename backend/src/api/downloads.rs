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
use crate::utils::status_utils::StatusCounts;

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
        // Batch actions
        .route("/batch/:batch_id/pause", post(pause_batch))
        .route("/batch/:batch_id/resume", post(resume_batch))
        .route("/batch/:batch_id", delete(delete_batch))
        .route("/batch/:batch_id/progress", get(get_batch_progress))
        .route("/batch/:batch_id/items", get(get_batch_items))
        // Batch summaries for pagination
        .route("/batches", get(list_batch_summaries))
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Serialize)]
struct DownloadsResponse {
    downloads: Vec<DownloadTask>,
    stats: EngineStats,
    /// Per-status counts from database (for filter dropdown)
    status_counts: StatusCounts,
    /// Pagination info
    total: u64,
    page: u32,
    limit: u32,
    total_pages: u32,
}

// StatusCounts imported from crate::utils::status_utils

/// Query parameters for list downloads
#[derive(Debug, Deserialize)]
struct ListDownloadsQuery {
    /// Page number (1-indexed, default 1)
    page: Option<u32>,
    /// Items per page (default 20, max 100)
    limit: Option<u32>,
    /// Sort column: "added" (default), "status", "filename", "size", "progress"
    sort_by: Option<String>,
    /// Sort direction: "asc" or "desc" (default)
    sort_dir: Option<String>,
    /// Filter by status (e.g., "DOWNLOADING", "QUEUED", "COMPLETED", "FAILED")
    status: Option<String>,
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

/// Batch summary for batch-first pagination
/// Represents a batch as a single row in the downloads table
#[derive(Serialize, Clone)]
pub struct BatchSummary {
    pub batch_id: String,
    pub batch_name: String,
    pub total_items: usize,
    pub completed_items: usize,
    pub failed_items: usize,
    pub downloading_items: usize,
    pub paused_items: usize,
    pub queued_items: usize,
    pub total_size: u64,
    pub downloaded_size: u64,
    pub progress: f32,
    pub speed: f64,
    pub created_at: String,
    pub state: String, // Aggregate state: DOWNLOADING, PAUSED, COMPLETED, FAILED, QUEUED
}

/// Response for /api/downloads/batches endpoint
#[derive(Serialize)]
struct BatchSummariesResponse {
    batches: Vec<BatchSummary>,
    standalone: Vec<DownloadTask>,
    stats: EngineStats,
    status_counts: StatusCounts,
    total: u64, // Total display units (batches + standalone)
    page: u32,
    limit: u32,
    total_pages: u32,
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
    #[serde(deserialize_with = "deserialize_year")]
    pub year: Option<i32>,
    /// Collection name (for movies in a collection)
    pub collection_name: Option<String>,
    /// Season number (for TV)
    pub season: Option<i32>,
    /// Episode number (for TV)
    pub episode: Option<i32>,
}

/// Custom deserializer for year field - accepts both string and integer
fn deserialize_year<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Deserialize};
    
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum YearValue {
        Int(i32),
        String(String),
    }
    
    match Option::<YearValue>::deserialize(deserializer)? {
        None => Ok(None),
        Some(YearValue::Int(i)) => Ok(Some(i)),
        Some(YearValue::String(s)) => {
            s.parse::<i32>()
                .map(Some)
                .map_err(|_| de::Error::custom(format!("invalid year string: {}", s)))
        }
    }
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
    /// Batch ID for grouping related downloads (e.g., TV season)
    #[serde(default)]
    pub batch_id: Option<String>,
    /// Batch display name (e.g., "Breaking Bad S01")
    #[serde(default)]
    pub batch_name: Option<String>,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/downloads - List downloads with pagination
/// 
/// Query params:
/// - page: Page number (1-indexed, default 1)
/// - limit: Items per page (default 20, max 100)
/// - sort_by: Column to sort by ("added", "status", "filename", "size", "progress")
/// - sort_dir: Sort direction ("asc" or "desc")
/// - status: Filter by status (e.g., "DOWNLOADING", "QUEUED", "COMPLETED", "FAILED")
async fn list_downloads(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListDownloadsQuery>,
) -> Json<DownloadsResponse> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).min(100).max(1);
    let sort_by = params.sort_by.unwrap_or_else(|| "added".to_string());
    let sort_dir = params.sort_dir.unwrap_or_else(|| "desc".to_string());
    let status_filter = params.status; // None means "all"
    
    // Query from database with pagination, sorting, and optional status filter (async to prevent blocking)
    let (mut downloads, total) = state.db
        .get_tasks_paginated_sorted_filtered_async(page, limit, &sort_by, &sort_dir, status_filter.as_deref()).await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to get tasks from DB: {}", e);
            (Vec::new(), 0)
        });
    
    // Get status counts from database for filter dropdown
    let db_status_counts = state.db.get_status_counts_async().await.unwrap_or_default();
    let status_counts = StatusCounts::from_db_counts(db_status_counts);
    
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
        status_counts,
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
) -> Result<Json<DownloadTask>, (StatusCode, String)> {
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
            match tmdb.media_type.as_deref() {
                Some("movie") => "movies".to_string(),
                Some("tv") => "tv".to_string(),
                Some(other) => other.to_string(),
                None => "other".to_string(),
            }
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
    
    // Smart Grab Batch Consolidation: Always check for existing batch by name
    let (batch_id, batch_name) = if let (Some(provided_batch_id), Some(provided_batch_name)) = 
        (&payload.batch_id, &payload.batch_name) {
        
        // Frontend provided batch info - check if we should consolidate with existing batch
        let existing_batch_id = state.db
            .get_batch_id_by_name_async(provided_batch_name)
            .await
            .ok()
            .flatten();
        
        if let Some(existing_id) = existing_batch_id {
            tracing::info!("Consolidating into existing batch: {} ({})", provided_batch_name, existing_id);
            (Some(existing_id), Some(provided_batch_name.clone()))
        } else {
            tracing::info!("Creating new batch: {} ({})", provided_batch_name, provided_batch_id);
            (Some(provided_batch_id.clone()), Some(provided_batch_name.clone()))
        }
    } else if let Some(ref meta) = tmdb_metadata {
        // No batch info provided - auto-generate for TV shows
        if meta.media_type.as_deref() == Some("tv") && meta.season.is_some() {
            let auto_batch_name = format!(
                "{} S{:02}",
                meta.title.as_deref().unwrap_or("Unknown Show"),
                meta.season.unwrap()
            );
            
            let existing_batch_id = state.db
                .get_batch_id_by_name_async(&auto_batch_name)
                .await
                .ok()
                .flatten();
            
            let auto_batch_id = if let Some(existing_id) = existing_batch_id {
                tracing::info!("Reusing existing batch: {} ({})", auto_batch_name, existing_id);
                existing_id
            } else {
                let new_id = uuid::Uuid::new_v4().to_string();
                tracing::info!("Creating new batch: {} ({})", auto_batch_name, new_id);
                new_id
            };
            
            (Some(auto_batch_id), Some(auto_batch_name))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };
    
    let task = match state.download_orchestrator.add_download_with_metadata(
        payload.url,
        payload.filename,
        "fshare".to_string(),
        category,
        tmdb_metadata,
        batch_id,
        batch_name,
    ).await {
        Ok(task) => task,
        Err(e) => {
            let error_msg = format!("{}", e);
            if error_msg.contains("already exists") {
                tracing::info!("Duplicate download skipped: {}", error_msg);
                return Err((StatusCode::CONFLICT, format!("Skipped: {}", error_msg)));
            }
            tracing::error!("Failed to add download: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed: {}", e)));
        }
    };
    
    // Return full task object so frontend gets batch_id, batch_name, etc.
    Ok(Json(task))
}

/// DELETE /api/downloads/:id - Delete download
async fn delete_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse>, StatusCode> {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Delete from in-memory task manager (may not exist if task is queued/failed)
    let in_memory = state.download_orchestrator.task_manager().delete_task(uuid);
    
    // Always try to delete from database (task might exist in DB but not in memory)
    let from_db = match state.db.delete_task(uuid) {
        Ok(_) => {
            tracing::info!("Deleted task {} from database", uuid);
            true
        }
        Err(e) => {
            tracing::warn!("Failed to delete task {} from database: {}", uuid, e);
            false
        }
    };
    
    let success = in_memory || from_db;
    
    // Broadcast TASK_REMOVED to frontend if deleted from either location
    if success {
        state.download_orchestrator.broadcast_task_removed(&id);
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
        // Wake idle workers to process the resumed task
        state.download_orchestrator.wake_workers();
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
    let task_result = state.download_orchestrator.task_manager().retry_task(uuid);
    let success = task_result.is_some();
    
    // Wake idle workers to process the retried task
    if success {
        state.download_orchestrator.wake_workers();
    }
    
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
/// Delegates to Orchestrator which handles: DB (atomic) → TaskManager → Broadcast
async fn pause_all(
    State(state): State<Arc<AppState>>,
) -> Json<BulkActionResponse> {
    let affected = state.download_orchestrator.pause_all_async().await;
    
    Json(BulkActionResponse {
        success: true,
        affected,
    })
}

/// POST /api/downloads/resume-all - Resume all paused downloads
/// Delegates to Orchestrator which handles: DB (atomic) → TaskManager → Broadcast → Wake
async fn resume_all(
    State(state): State<Arc<AppState>>,
) -> Json<BulkActionResponse> {
    let affected = state.download_orchestrator.resume_all_async().await;
    
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

// ============================================================================
// Batch Action Handlers
// ============================================================================

/// POST /api/downloads/batch/:batch_id/pause - Pause all downloads in a batch
async fn pause_batch(
    State(state): State<Arc<AppState>>,
    Path(batch_id): Path<String>,
) -> Json<BulkActionResponse> {
    match state.download_service.pause_batch(&batch_id).await {
        Ok(affected) => Json(BulkActionResponse { success: true, affected }),
        Err(e) => {
            tracing::error!("Failed to pause batch {}: {}", batch_id, e);
            Json(BulkActionResponse { success: false, affected: 0 })
        }
    }
}

/// POST /api/downloads/batch/:batch_id/resume - Resume all downloads in a batch
async fn resume_batch(
    State(state): State<Arc<AppState>>,
    Path(batch_id): Path<String>,
) -> Json<BulkActionResponse> {
    match state.download_service.resume_batch(&batch_id).await {
        Ok(affected) => Json(BulkActionResponse { success: true, affected }),
        Err(e) => {
            tracing::error!("Failed to resume batch {}: {}", batch_id, e);
            Json(BulkActionResponse { success: false, affected: 0 })
        }
    }
}

/// DELETE /api/downloads/batch/:batch_id - Delete all downloads in a batch
async fn delete_batch(
    State(state): State<Arc<AppState>>,
    Path(batch_id): Path<String>,
) -> Json<BulkActionResponse> {
    match state.download_service.delete_batch(&batch_id).await {
        Ok(affected) => {
            tracing::info!("Deleted batch {}: {} tasks", batch_id, affected);
            Json(BulkActionResponse { success: true, affected })
        }
        Err(e) => {
            tracing::error!("Failed to delete batch {}: {}", batch_id, e);
            Json(BulkActionResponse { success: false, affected: 0 })
        }
    }
}

// ============================================================================
// Batch Progress
// ============================================================================

#[derive(Serialize)]
pub struct BatchProgress {
    pub batch_id: String,
    pub batch_name: String,
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub downloading_tasks: usize,
    pub paused_tasks: usize,
    pub queued_tasks: usize,
    pub overall_progress: f32,
    pub total_size: u64,
    pub downloaded_size: u64,
    pub combined_speed: f64,
    pub estimated_time_remaining: f64,
}

/// GET /api/downloads/batch/:batch_id/progress - Get batch progress aggregation
async fn get_batch_progress(
    State(state): State<Arc<AppState>>,
    Path(batch_id): Path<String>,
) -> Result<Json<BatchProgress>, StatusCode> {
    let all_tasks = state.download_orchestrator.task_manager().get_tasks();
    
    // Filter tasks by batch_id
    let batch_tasks: Vec<_> = all_tasks.iter()
        .filter(|t| t.batch_id.as_deref() == Some(&batch_id))
        .collect();
    
    if batch_tasks.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }
    
    let batch_name = batch_tasks[0].batch_name.clone().unwrap_or_default();
    let total_tasks = batch_tasks.len();
    
    // Count by state
    let completed_tasks = batch_tasks.iter().filter(|t| matches!(t.state, DownloadState::Completed)).count();
    let failed_tasks = batch_tasks.iter().filter(|t| matches!(t.state, DownloadState::Failed)).count();
    let downloading_tasks = batch_tasks.iter().filter(|t| matches!(t.state, DownloadState::Downloading)).count();
    let paused_tasks = batch_tasks.iter().filter(|t| matches!(t.state, DownloadState::Paused)).count();
    let queued_tasks = batch_tasks.iter().filter(|t| matches!(t.state, DownloadState::Queued)).count();
    
    // Calculate totals
    let total_size: u64 = batch_tasks.iter().map(|t| t.size).sum();
    let downloaded_size: u64 = batch_tasks.iter()
        .map(|t| ((t.progress as f64 / 100.0) * t.size as f64) as u64)
        .sum();
    
    // Calculate overall progress
    let overall_progress = if total_size > 0 {
        (downloaded_size as f64 / total_size as f64 * 100.0) as f32
    } else {
        0.0
    };
    
    // Calculate combined speed (only downloading tasks)
    let combined_speed: f64 = batch_tasks.iter()
        .filter(|t| matches!(t.state, DownloadState::Downloading))
        .map(|t| t.speed)
        .sum();
    
    // Calculate ETA
    let remaining_bytes = total_size.saturating_sub(downloaded_size);
    let estimated_time_remaining = if combined_speed > 0.0 {
        remaining_bytes as f64 / combined_speed
    } else {
        0.0
    };
    
    Ok(Json(BatchProgress {
        batch_id,
        batch_name,
        total_tasks,
        completed_tasks,
        failed_tasks,
        downloading_tasks,
        paused_tasks,
        queued_tasks,
        overall_progress,
        total_size,
        downloaded_size,
        combined_speed,
        estimated_time_remaining,
    }))
}

// ============================================================================
// Batch Summaries for Pagination
// ============================================================================

/// GET /api/downloads/batches - List batch summaries and standalone downloads
/// This endpoint supports batch-first pagination where batches are treated as single rows
async fn list_batch_summaries(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListDownloadsQuery>,
) -> Json<BatchSummariesResponse> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).min(100).max(1);
    let status_filter = params.status;
    
    // Get batch summaries from database
    let (batches, standalone, total_batches, total_standalone) = state.db
        .get_batch_summaries_paginated_async(page, limit, status_filter.as_deref()).await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to get batch summaries from DB: {}", e);
            (Vec::new(), Vec::new(), 0, 0)
        });
    
    // Merge real-time progress from active downloads for standalone items
    let active_tasks = state.download_orchestrator.task_manager().get_active_tasks();
    let mut standalone_with_realtime = standalone;
    for download in standalone_with_realtime.iter_mut() {
        if let Some(active) = active_tasks.iter().find(|t| t.id == download.id) {
            download.progress = active.progress;
            download.speed = active.speed;
            download.eta = active.eta;
            download.downloaded = active.downloaded;
            download.state = active.state;
        }
    }
    
    // Update batch speeds and real-time progress from active tasks
    let mut batches_with_realtime = batches;
    for batch in batches_with_realtime.iter_mut() {
        // Find all active tasks belonging to this batch
        let batch_active: Vec<_> = active_tasks.iter()
            .filter(|t| t.batch_id.as_ref() == Some(&batch.batch_id))
            .collect();
            
        if !batch_active.is_empty() {
            let mut live_speed: f64 = 0.0;
            let mut live_downloaded: u64 = 0;
            
            for task in &batch_active {
                live_speed += task.speed;
                live_downloaded += task.downloaded;
            }
            
            batch.speed = live_speed;
            
            // Fix double-counting: The DB query summed 'downloaded' for ALL tasks including active ones.
            // But for active tasks, the DB value is stale. We need to:
            // 1. Subtract the number of active tasks * their stale DB contribution (usually 0)
            // 2. Add the live values from memory
            // 
            // Since active tasks typically have 0 in DB until paused/completed, the DB sum
            // represents completed tasks. Adding live_downloaded gives us the accurate total.
            // However, if tasks were resumed with partial progress saved, we might double-count.
            //
            // Safe approach: Replace progress calculation entirely with:
            // (DB downloaded for non-active tasks) + (memory downloaded for active tasks)
            //
            // Since we know downloading_items count, we can approximate:
            // DB already has 0 for active tasks' downloaded (they haven't been persisted since starting),
            // so batch.downloaded_size is already "completed + paused" progress.
            // We just add live progress for currently active tasks.
            batch.downloaded_size += live_downloaded;
            
            // Recalculate progress
            if batch.total_size > 0 {
                batch.progress = (batch.downloaded_size as f64 / batch.total_size as f64 * 100.0) as f32;
            }
        }
    }
    
    // Get status counts from database
    let db_status_counts = state.db.get_status_counts_async().await.unwrap_or_default();
    let status_counts = StatusCounts::from_db_counts(db_status_counts);
    
    let stats = state.download_orchestrator.task_manager().get_stats();
    let total = total_batches + total_standalone;
    let total_pages = ((total as f64) / (limit as f64)).ceil() as u32;
    
    Json(BatchSummariesResponse {
        batches: batches_with_realtime,
        standalone: standalone_with_realtime,
        stats,
        status_counts,
        total,
        page,
        limit,
        total_pages,
    })
}

/// GET /api/downloads/batch/:batch_id/items - Get all items in a batch
/// Used for lazy loading when user expands a batch
async fn get_batch_items(
    State(state): State<Arc<AppState>>,
    Path(batch_id): Path<String>,
) -> Result<Json<Vec<DownloadTask>>, StatusCode> {
    // Get all tasks with this batch_id from database
    let tasks = match state.db.get_tasks_by_batch_id_async(batch_id.clone()).await {
        Ok(tasks) => tasks,
        Err(e) => {
            tracing::error!("Failed to get tasks for batch {}: {}", batch_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    if tasks.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }
    
    // Merge real-time progress from active downloads
    let active_tasks = state.download_orchestrator.task_manager().get_active_tasks();
    let mut tasks_with_realtime = tasks;
    for task in tasks_with_realtime.iter_mut() {
        if let Some(active) = active_tasks.iter().find(|t| t.id == task.id) {
            task.progress = active.progress;
            task.speed = active.speed;
            task.eta = active.eta;
            task.downloaded = active.downloaded;
            task.state = active.state;
        }
    }
    
    Ok(Json(tasks_with_realtime))
}
