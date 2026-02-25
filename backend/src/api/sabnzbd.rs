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
struct SabHistorySlot {
    nzo_id: String,
    name: String,
    category: String,
    path: String,
    storage: String,
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
        "addfile" => {
            // addfile requires multipart data, return error for now
            // The actual implementation needs to be in handle_post with multipart
            Ok(Json(serde_json::json!({ 
                "status": false, 
                "error": "addfile mode requires POST with multipart data" 
            })))
        },
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

/// Handle POST requests (form data or multipart)
async fn handle_post(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SabParams>,
    mut multipart: Option<axum::extract::Multipart>,
) -> axum::response::Response {
    let mode = params.mode.as_deref().unwrap_or("queue");
    
    // Handle addfile mode with multipart data
    if mode == "addfile" {
        if let Some(ref mut mp) = multipart {
            return match handle_add_file(state, params, mp).await {
                Ok(json) => (StatusCode::OK, json).into_response(),
                Err(status) => status.into_response(),
            };
        } else {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "status": false,
                "error": "addfile requires multipart data"
            }))).into_response();
        }
    }
    
    // Otherwise, handle as regular GET request
    handle_get(State(state), Query(params)).await
}

/// Handle NZB file upload (addfile mode)
async fn handle_add_file(
    state: Arc<AppState>,
    params: SabParams,
    multipart: &mut axum::extract::Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    
    tracing::info!("SABnzbd: Handling addfile request (NZB upload)");
    
    // Extract category from params (Sonarr sends it as query param)
    let category = params.cat.unwrap_or_else(|| "other".to_string());
    
    // Parse multipart form data
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::error!("Failed to read multipart field: {}", e);
        StatusCode::BAD_REQUEST
    })? {
        let field_name = field.name().unwrap_or("");
        
        // Look for the NZB file field (usually named "name" or "nzbfile")
        if field_name == "name" || field_name == "nzbfile" {
            let data = field.bytes().await.map_err(|e| {
                tracing::error!("Failed to read field bytes: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            
            let content = String::from_utf8_lossy(&data);
            tracing::debug!("NZB content preview: {}", &content[..content.len().min(200)]);
            
            // Extract Fshare URL from <meta type="fshare_url"> tag
            let fshare_url = extract_nzb_metadata(&content, "fshare_url")
                .ok_or_else(|| {
                    tracing::error!("No Fshare URL found in NZB metadata");
                    StatusCode::BAD_REQUEST
                })?;
            
            // Extract TMDB metadata if present
            let tmdb_id = extract_nzb_metadata(&content, "tmdb_id");
            let season = extract_nzb_metadata(&content, "season")
                .and_then(|s| s.parse::<u32>().ok());
            let episode = extract_nzb_metadata(&content, "episode")
                .and_then(|e| e.parse::<u32>().ok());
            
            // Extract category from NZB metadata (fallback to query param)
            let nzb_category = extract_nzb_metadata(&content, "category")
                .unwrap_or(category.clone());
            
            tracing::info!(
                "SABnzbd: Extracted from NZB - URL: {}, category: {}, TMDB: {:?}, S{:?}E{:?}",
                fshare_url, nzb_category, tmdb_id, season, episode
            );
            
            // Build TMDB metadata if available
            let tmdb_meta = if let (Some(id), Some(s), Some(e)) = (tmdb_id.clone(), season, episode) {
                // Parse tmdb_id as i64
                let tmdb_id_i64 = id.parse::<i64>().ok();
                
                // Fetch series title from TMDB for proper folder organization
                let title = if let Some(tmdb_id_val) = &tmdb_id {
                    crate::api::indexer::fetch_tmdb_title(tmdb_id_val, "tv").await
                } else {
                    None
                };
                
                tracing::info!("SABnzbd: Fetched TMDB title: {:?}", title);
                
                Some(crate::downloader::TmdbDownloadMetadata {
                    tmdb_id: tmdb_id_i64,
                    media_type: Some("tv".to_string()),
                    title, // Use fetched title for proper folder structure
                    year: None,
                    collection_name: None,
                    season: Some(s as i32),
                    episode: Some(e as i32),
                })
            } else {
                None
            };
            
            // Auto-batch: TV episodes should always be in a batch (same logic as downloads API)
            let (batch_id, batch_name) = if let Some(ref meta) = tmdb_meta {
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
                        tracing::info!("SABnzbd: Reusing existing batch: {} ({})", auto_batch_name, existing_id);
                        existing_id
                    } else {
                        let new_id = uuid::Uuid::new_v4().to_string();
                        tracing::info!("SABnzbd: Creating new batch: {} ({})", auto_batch_name, new_id);
                        new_id
                    };
                    
                    (Some(auto_batch_id), Some(auto_batch_name))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };
            
            // Add to download queue
            // Pass None for filename to let orchestrator fetch real filename from Fshare API
            let task = state.download_orchestrator.add_download_with_metadata(
                fshare_url,
                None, // Let orchestrator fetch real filename from Fshare API
                "fshare".to_string(),
                nzb_category,
                tmdb_meta,
                batch_id,
                batch_name,
            ).await.map_err(|e| {
                tracing::error!("Failed to add download: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            
            return Ok(Json(serde_json::json!({
                "status": true,
                "nzo_ids": [task.id.to_string()]
            })));
        }
    }
    
    tracing::error!("No NZB file found in multipart data");
    Err(StatusCode::BAD_REQUEST)
}

/// Extract metadata value from NZB XML
fn extract_nzb_metadata(content: &str, meta_type: &str) -> Option<String> {
    let tag_start = format!(r#"<meta type="{}">"#, meta_type);
    if let Some(start) = content.find(&tag_start) {
        let after_tag = &content[start + tag_start.len()..];
        if let Some(end) = after_tag.find("</meta>") {
            return Some(after_tag[..end].to_string());
        }
    }
    None
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
/// 
/// Returns completed/failed downloads for Sonarr/Radarr to detect and import.
/// Uses TMDB title for the `name` field so *arr can match to series/movies in its library.
/// Only returns items where the file actually exists on disk.
async fn handle_history(state: Arc<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let tasks = state.download_orchestrator.task_manager().get_tasks();
    
    let slots: Vec<SabHistorySlot> = tasks.iter()
        .filter(|t| matches!(t.state, 
            DownloadState::Completed | 
            DownloadState::Failed |
            DownloadState::Cancelled
        ))
        .filter_map(|t| {
            let status = match t.state {
                DownloadState::Completed => "Completed",
                DownloadState::Failed => "Failed",
                DownloadState::Cancelled => "Failed",
                _ => "Unknown",
            };
            
            // Skip items with no destination — they'd produce an empty path
            // which Sonarr rejects as "not a valid path".
            if t.destination.is_empty() {
                tracing::debug!("SABnzbd history: skipping {} - destination is empty", t.filename);
                return None;
            }

            // Map container path to Sonarr path.
            // Flasharr container:  /downloads/Show/ep.mkv
            // Sonarr (LXC 110):   /data/downloads/Show/ep.mkv  (has /data → /data)
            let sonarr_path = if t.destination.starts_with("/downloads/") {
                // Container-internal path → prefix with /data
                format!("/data{}", t.destination)
            } else if t.destination.starts_with("/data/downloads/") {
                // Already the correct Sonarr-visible path (tasks added post-fix)
                t.destination.clone()
            } else {
                // Unknown path — log and skip so Sonarr doesn't get a garbage path
                tracing::warn!(
                    "SABnzbd history: unexpected destination path for {}: {}",
                    t.filename, t.destination
                );
                return None;
            };

            // Only return completed items where the file actually exists on disk
            // This prevents "No files found are eligible for import" errors
            if status == "Completed" {
                let check_path = std::path::Path::new(&t.destination);
                if !check_path.exists() {
                    tracing::debug!("SABnzbd history: skipping {} - file not found at {}", t.filename, t.destination);
                    return None;
                }
            }

            
            // Extract storage directory from destination path
            let storage = if let Some(parent) = std::path::Path::new(&sonarr_path).parent() {
                parent.to_string_lossy().to_string()
            } else {
                sonarr_path.clone()
            };
            
            // Build a name that Sonarr/Radarr can reliably parse for series/movie matching.
            // Use scene-release-like format: "Series.Name.S01E07.WEB-DL.Flasharr"
            // This strips special chars, replaces spaces with dots, matching scene naming
            // conventions that Sonarr's parser is optimized for.
            let name = if let Some(ref tmdb_title) = t.tmdb_title {
                // Normalize title: strip special chars, replace spaces with dots
                let clean_title: String = tmdb_title.chars()
                    .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<&str>>()
                    .join(".");
                
                if let (Some(season), Some(episode)) = (t.tmdb_season, t.tmdb_episode) {
                    // TV: "Series.Name.S01E07.WEB-DL.Flasharr"
                    format!("{}.S{:02}E{:02}.WEB-DL.Flasharr", clean_title, season, episode)
                } else {
                    // Movie: "Movie.Name.WEB-DL.Flasharr"
                    format!("{}.WEB-DL.Flasharr", clean_title)
                }
            } else {
                // No TMDB metadata: use filename without extension
                std::path::Path::new(&t.filename)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| t.filename.clone())
            };
            
            Some(SabHistorySlot {
                nzo_id: t.id.to_string(),
                name,
                category: t.category.clone(),
                path: sonarr_path,
                storage,
                status: status.to_string(),
                fail_message: t.error_message.clone().unwrap_or_default(),
            })
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
/// 
/// Sonarr/Radarr use complete_dir + category dir to determine where downloads land.
/// Our downloads go directly into series/movie-named folders under /data/downloads/,
/// so category dirs must be empty to avoid Sonarr looking in /data/downloads/TV/ etc.
async fn handle_get_config() -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(serde_json::json!({
        "config": {
            "version": "3.5.0",
            "paused": false,
            "pause_int": "0",
            "download_dir": "/data/downloads/incomplete",
            "complete_dir": "/data/downloads",
            "nzb_backup_dir": "/appData/nzb_backup",
            "script_dir": "/appData/scripts",
            "categories": [
                {
                    "name": "tv",
                    "dir": "",
                    "newzbin": "",
                    "priority": 0
                },
                {
                    "name": "movies",
                    "dir": "",
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
