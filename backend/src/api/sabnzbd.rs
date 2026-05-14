//! SABnzbd Compatibility API
//!
//! Provides SABnzbd-compatible endpoints for integration with Sonarr/Radarr.
//! This allows *arr applications to use Flasharr as a download client.

use crate::downloader::DownloadState;
use crate::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(handle_get))
        .route("/", post(handle_post))
        .route("/api", get(handle_get))
        .route("/api", post(handle_post))
        .fallback(handle_fallback)
}

async fn handle_fallback(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SabParams>,
) -> axum::response::Response {
    tracing::info!(
        "SABnzbd API fallback handler - mode: {}",
        params.mode.as_deref().unwrap_or("queue")
    );
    handle_get(State(state), Query(params)).await
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Deserialize)]
pub struct SabParams {
    mode: Option<String>,
    #[allow(dead_code)]
    output: Option<String>,
    #[allow(dead_code)]
    apikey: Option<String>,
    name: Option<String>,    // URL to download
    nzbname: Option<String>, // Filename
    cat: Option<String>,     // Category
    #[allow(dead_code)]
    priority: Option<i32>,
    nzo_id: Option<String>, // Task ID for operations
}

#[derive(Serialize)]
struct SabQueueSlot {
    nzo_id: String,
    filename: String,
    percentage: u32,
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
pub async fn handle_get(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SabParams>,
) -> axum::response::Response {
    let mode = params.mode.as_deref().unwrap_or("queue");

    tracing::info!(
        "SABnzbd API request - mode: {}, apikey: {:?}",
        mode,
        params.apikey
    );

    let result = match mode {
        "addurl" => handle_add_url(state, params).await,
        "addfile" => {
            // addfile requires multipart data, return error for now
            // The actual implementation needs to be in handle_post with multipart
            Ok(Json(serde_json::json!({
                "status": false,
                "error": "addfile mode requires POST with multipart data"
            })))
        }
        "queue" => handle_queue(state).await,
        "fullstatus" => handle_fullstatus(state).await,
        "history" => handle_history(state).await,
        "version" => handle_version().await,
        "get_config" => handle_get_config().await,
        "qstatus" => handle_queue(state).await,
        "pause" => handle_pause(state, params).await,
        "resume" => handle_resume(state, params).await,
        "delete" => handle_delete(state, params).await,
        _ => {
            tracing::warn!("Unknown SABnzbd mode: {}", mode);
            Ok(Json(
                serde_json::json!({ "status": false, "error": "Unknown mode" }),
            ))
        }
    };

    match result {
        Ok(json) => (StatusCode::OK, json).into_response(),
        Err(status) => status.into_response(),
    }
}

/// Handle POST requests (form data or multipart)
pub async fn handle_post(
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
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "status": false,
                    "error": "addfile requires multipart data"
                })),
            )
                .into_response();
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
            tracing::debug!(
                "NZB content preview: {}",
                &content[..content.len().min(200)]
            );

            // Extract Fshare URL from <meta type="fshare_url"> tag
            let fshare_url = extract_nzb_metadata(&content, "fshare_url").ok_or_else(|| {
                tracing::error!("No Fshare URL found in NZB metadata");
                StatusCode::BAD_REQUEST
            })?;

            // Extract TMDB metadata if present
            let tmdb_id = extract_nzb_metadata(&content, "tmdb_id");
            let media_type_from_nzb = extract_nzb_metadata(&content, "media_type");
            let season =
                extract_nzb_metadata(&content, "season").and_then(|s| s.parse::<u32>().ok());
            let episode =
                extract_nzb_metadata(&content, "episode").and_then(|e| e.parse::<u32>().ok());

            // Extract category from NZB metadata (fallback to query param)
            let nzb_category =
                extract_nzb_metadata(&content, "category").unwrap_or(category.clone());

            tracing::info!(
                "SABnzbd: Extracted from NZB - URL: {}, category: {}, TMDB: {:?}, media_type: {:?}, S{:?}E{:?}",
                fshare_url,
                nzb_category,
                tmdb_id,
                media_type_from_nzb,
                season,
                episode
            );

            // Build TMDB metadata if available
            let tmdb_meta = if let Some(ref id) = tmdb_id {
                let tmdb_id_i64 = id.parse::<i64>().ok();

                // Prefer explicit media_type from NZB; fall back to inferring from season/episode
                let actual_media_type = media_type_from_nzb.as_deref().unwrap_or(
                    if season.is_some() && episode.is_some() {
                        "tv"
                    } else {
                        "movie"
                    },
                );

                let (title, year) =
                    crate::api::indexer::fetch_tmdb_title_and_year(id, actual_media_type).await;

                tracing::info!(
                    "SABnzbd: Fetched TMDB title: {:?}, year: {:?}, media_type: {}",
                    title,
                    year,
                    actual_media_type
                );

                if actual_media_type == "movie" {
                    Some(crate::downloader::TmdbDownloadMetadata {
                        tmdb_id: tmdb_id_i64,
                        media_type: Some("movie".to_string()),
                        title,
                        year,
                        collection_name: None,
                        season: None,
                        episode: None,
                    })
                } else if let (Some(s), Some(e)) = (season, episode) {
                    Some(crate::downloader::TmdbDownloadMetadata {
                        tmdb_id: tmdb_id_i64,
                        media_type: Some("tv".to_string()),
                        title,
                        year,
                        collection_name: None,
                        season: Some(s as i32),
                        episode: Some(e as i32),
                    })
                } else {
                    None
                }
            } else {
                None
            };

            // Auto-batch: TV episodes consolidate by TMDB ID then title — same logic as downloads API
            let (batch_id, batch_name) = if let Some(ref meta) = tmdb_meta {
                if meta.media_type.as_deref() == Some("tv") {
                    let auto_batch_name =
                        meta.title.as_deref().unwrap_or("Unknown Show").to_string();

                    let existing_id = if let Some(tid) = meta.tmdb_id {
                        state
                            .db
                            .get_batch_id_by_tmdb_id_async(tid)
                            .await
                            .ok()
                            .flatten()
                            .or(state
                                .db
                                .get_batch_id_by_name_async(&auto_batch_name)
                                .await
                                .ok()
                                .flatten())
                    } else {
                        state
                            .db
                            .get_batch_id_by_name_async(&auto_batch_name)
                            .await
                            .ok()
                            .flatten()
                    };

                    let auto_batch_id = if let Some(existing_id) = existing_id {
                        tracing::info!(
                            "SABnzbd: Reusing existing batch: {} ({})",
                            auto_batch_name,
                            existing_id
                        );
                        existing_id
                    } else {
                        let new_id = uuid::Uuid::new_v4().to_string();
                        tracing::info!(
                            "SABnzbd: Creating new batch: {} ({})",
                            auto_batch_name,
                            new_id
                        );
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
            let task = state
                .download_orchestrator
                .add_download_with_metadata(
                    fshare_url,
                    None, // Let orchestrator fetch real filename from Fshare API
                    "sabnzbd-fshare".to_string(),
                    nzb_category,
                    tmdb_meta,
                    batch_id,
                    batch_name,
                )
                .await
                .map_err(|e| {
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

    tracing::info!(
        "SABnzbd: Adding download - URL: {}, filename: {}",
        url,
        filename
    );

    let task = state
        .download_orchestrator
        .add_download(url, filename, "fshare".to_string(), category)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "status": true,
        "nzo_ids": [task.id.to_string()]
    })))
}

/// Get queue status
async fn handle_queue(state: Arc<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let tasks = state.download_orchestrator.task_manager().get_tasks().await;

    let mut total_speed = 0.0;
    let mut total_size = 0u64;
    let mut total_left = 0u64;

    let slots: Vec<SabQueueSlot> = tasks
        .iter()
        .filter(|t| {
            matches!(
                t.state,
                DownloadState::Queued
                    | DownloadState::Starting
                    | DownloadState::Downloading
                    | DownloadState::Waiting
                    | DownloadState::Extracting
            )
        })
        .map(|t| {
            total_speed += t.speed as f64;
            total_size += t.size;
            let downloaded = ((t.progress as f64) / 100.0 * t.size as f64) as u64;
            let left = t.size.saturating_sub(downloaded);
            total_left += left;

            let speed = t.speed as f64;
            let eta = if speed > 0.0 {
                let secs = (left as f64 / speed) as u64;
                format!("{}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
            } else {
                "0:00:00".to_string()
            };

            let sab_status = match t.state {
                DownloadState::Downloading | DownloadState::Starting => "Downloading",
                DownloadState::Extracting => "Extracting",
                DownloadState::Paused => "Paused",
                _ => "Queued",
            };

            SabQueueSlot {
                nzo_id: t.id.to_string(),
                filename: t.filename.clone(),
                percentage: t.progress as u32,
                mb: format!("{:.2}", t.size as f64 / 1_048_576.0),
                mbleft: format!("{:.2}", left as f64 / 1_048_576.0),
                status: sab_status.to_string(),
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
    let tasks = state.download_orchestrator.task_manager().get_tasks().await;

    let mut total_speed = 0.0;
    let mut total_size = 0u64;
    let mut total_left = 0u64;

    let slots: Vec<SabQueueSlot> = tasks
        .iter()
        .filter(|t| {
            matches!(
                t.state,
                DownloadState::Downloading
                    | DownloadState::Queued
                    | DownloadState::Starting
                    | DownloadState::Waiting
                    | DownloadState::Extracting
            )
        })
        .map(|t| {
            total_speed += t.speed as f64;
            total_size += t.size;
            let downloaded = ((t.progress as f64) / 100.0 * t.size as f64) as u64;
            let left = t.size.saturating_sub(downloaded);
            total_left += left;

            let speed = t.speed as f64;
            let eta = if speed > 0.0 {
                let secs = (left as f64 / speed) as u64;
                format!("{}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
            } else {
                "0:00:00".to_string()
            };

            let sab_status = match t.state {
                DownloadState::Downloading | DownloadState::Starting => "Downloading",
                DownloadState::Extracting => "Extracting",
                DownloadState::Paused => "Paused",
                _ => "Queued",
            };

            SabQueueSlot {
                nzo_id: t.id.to_string(),
                filename: t.filename.clone(),
                percentage: t.progress as u32,
                mb: format!("{:.2}", t.size as f64 / 1_048_576.0),
                mbleft: format!("{:.2}", left as f64 / 1_048_576.0),
                status: sab_status.to_string(),
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
    // Read directly from DB — in-memory tasks are evicted after 5 min (too short for Radarr polls)
    let tasks = state
        .db
        .get_tasks_by_states_async(vec![
            "COMPLETED".to_string(),
            "FAILED".to_string(),
            "CANCELLED".to_string(),
            "IMPORTING".to_string(),
        ])
        .await
        .unwrap_or_default();

    let mut ready_batches = std::collections::HashSet::new();
    let mut held_batches = std::collections::HashSet::new();
    let batch_ids: std::collections::HashSet<String> =
        tasks.iter().filter_map(|t| t.batch_id.clone()).collect();
    for batch_id in batch_ids {
        match state.db.get_tasks_by_batch_id_async(batch_id.clone()).await {
            Ok(batch_tasks) => {
                let all_complete = !batch_tasks.is_empty()
                    && batch_tasks.iter().all(|task| {
                        matches!(
                            task.state,
                            DownloadState::Completed | DownloadState::Importing
                        ) && std::path::Path::new(&task.destination).exists()
                    });
                if all_complete {
                    ready_batches.insert(batch_id);
                } else {
                    held_batches.insert(batch_id);
                }
            }
            Err(e) => {
                tracing::warn!(
                    "SABnzbd history: holding batch {} because readiness check failed: {}",
                    batch_id,
                    e
                );
                held_batches.insert(batch_id);
            }
        }
    }

    let slots: Vec<SabHistorySlot> = tasks
        .iter()
        .filter_map(|t| {
            let status = match t.state {
                DownloadState::Completed | DownloadState::Importing => "Completed",
                DownloadState::Failed => "Failed",
                DownloadState::Cancelled => "Failed",
                _ => return None,
            };

            if let Some(batch_id) = &t.batch_id {
                if held_batches.contains(batch_id) || !ready_batches.contains(batch_id) {
                    tracing::debug!(
                        "SABnzbd history: holding {} until batch {} is complete",
                        t.filename,
                        batch_id
                    );
                    return None;
                }
            }

            // Skip items with no destination — they'd produce an empty path
            // which Sonarr rejects as "not a valid path".
            if t.destination.is_empty() {
                tracing::debug!(
                    "SABnzbd history: skipping {} - destination is empty",
                    t.filename
                );
                return None;
            }

            // Map container path to Sonarr/Radarr-visible path.
            //
            // Three cases:
            // 1. /appData/downloads/... → /data/flasharr-download/...
            //    (file is still in staging, not yet moved to library)
            // 2. /data/flasharr-download/... → unchanged
            //    (already the host-side staging path)
            // 3. /data/media/... → unchanged
            //    (file was moved to library by move_to_arr_path; Sonarr/Radarr can see it directly)
            // 4. /downloads/... → /data/flasharr-download/...
            //    (legacy compat for old tasks)
            let sonarr_path = if t.destination.starts_with("/appData/downloads/") {
                t.destination
                    .replace("/appData/downloads/", "/data/flasharr-download/")
            } else if t.destination.starts_with("/data/flasharr-download/") {
                t.destination.clone()
            } else if t.destination.starts_with("/data/media/") {
                // File already imported into library — pass through as-is
                t.destination.clone()
            } else if t.destination.starts_with("/downloads/") {
                t.destination
                    .replace("/downloads/", "/data/flasharr-download/")
            } else {
                tracing::warn!(
                    "SABnzbd history: unexpected destination path for {}: {}",
                    t.filename,
                    t.destination
                );
                return None;
            };

            // Only return completed items where the file actually exists on disk
            // This prevents "No files found are eligible for import" errors
            if status == "Completed" {
                let check_path = std::path::Path::new(&t.destination);
                if !check_path.exists() {
                    tracing::debug!(
                        "SABnzbd history: skipping {} - file not found at {}",
                        t.filename,
                        t.destination
                    );
                    return None;
                }

                // Only skip items in the media library if they are OLD (completed > 15 mins ago).
                // This gives Sonarr/Radarr a window to see the "Completed" status and clear their queue.
                if t.destination.contains("/data/media/") {
                    if let Some(completed_at) = t.completed_at {
                        let age = chrono::Utc::now().signed_duration_since(completed_at);
                        if age > chrono::Duration::minutes(15) {
                            tracing::debug!(
                                "SABnzbd history: skipping old library item {} (age: {:?})",
                                t.filename,
                                age
                            );
                            return None;
                        }
                    } else {
                        // If no completion time, assume it's old if it's already in library
                        return None;
                    }
                }
            }

            // Extract storage directory from destination path
            let storage = if t.host == "sabnzbd-fshare" {
                sonarr_path.clone()
            } else if let Some(parent) = std::path::Path::new(&sonarr_path).parent() {
                parent.to_string_lossy().to_string()
            } else {
                sonarr_path.clone()
            };

            // Build a scene-release-style name Sonarr/Radarr can parse for matching.
            // Format: "Movie.Title.Year.WEB-DL" (movies) or "Series.Title.S01E07.WEB-DL" (TV)
            // - Replace `&` with `and` before stripping non-alphanumeric chars
            // - No "Flasharr" suffix — it confuses Radarr's title parser (mistaken for group tag)
            let name = if let Some(ref tmdb_title) = t.tmdb_title {
                // Normalize `&` → `and` first, then strip remaining special chars
                let normalized = tmdb_title.replace('&', "and");
                let clean_title: String = normalized
                    .chars()
                    .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-')
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<&str>>()
                    .join(".");

                if let (Some(season), Some(episode)) = (t.tmdb_season, t.tmdb_episode) {
                    // TV: "Series.Title.S01E07.WEB-DL"
                    format!("{}.S{:02}E{:02}.WEB-DL", clean_title, season, episode)
                } else {
                    // Movie: "Movie.Title.Year.WEB-DL" — year required for Radarr title matching
                    let year = t
                        .tmdb_id
                        .and_then(|id| state.db.get_media_item(id).ok().flatten())
                        .and_then(|item| item.year);
                    if let Some(y) = year {
                        format!("{}.{}.WEB-DL", clean_title, y)
                    } else {
                        format!("{}.WEB-DL", clean_title)
                    }
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
                "complete_dir": "/data/flasharr-download",
                "download_dir": "/data/flasharr-download/incomplete",
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
            let _ = state
                .download_orchestrator
                .task_manager()
                .pause_task(uuid)
                .await;
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
            let _ = state
                .download_orchestrator
                .task_manager()
                .resume_task(uuid)
                .await;
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
            let _ = state
                .download_orchestrator
                .task_manager()
                .delete_task(uuid)
                .await;
        }
    }

    Ok(Json(serde_json::json!({ "status": true })))
}
