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

    tracing::info!("SABnzbd API request - mode: {}", mode);

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
            let lookup_state = Arc::clone(&state);
            return match handle_add_file(
                params,
                mp,
                move |lookup| {
                    let state = Arc::clone(&lookup_state);
                    async move { lookup_download_title(state, lookup).await }
                },
                move |request| async move {
                    state
                        .download_orchestrator
                        .add_download_with_metadata(
                            request.url,
                            request.filename,
                            request.host,
                            request.category,
                            request.tmdb_metadata,
                            None,
                            None,
                            false,
                            request.download_source,
                            None,
                            None,
                            None,
                            None,
                            request.info_hash,
                            request.external_id,
                            None,
                            None,
                            None,
                            None,
                            None,
                        )
                        .await
                        .map(|task| task.id.to_string())
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
                },
            )
            .await
            {
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

struct QueueSource {
    url: String,
    filename: Option<String>,
    host: String,
    download_source: String,
    info_hash: Option<String>,
    external_id: Option<String>,
}

fn queue_source(
    source: crate::providers::fshare::AcquisitionSource,
    title: Option<String>,
) -> Option<QueueSource> {
    match source {
        crate::providers::fshare::AcquisitionSource::FshareUrl(url) => Some(QueueSource {
            url,
            filename: None,
            host: "sabnzbd-fshare".to_string(),
            download_source: "fshare".to_string(),
            info_hash: None,
            external_id: None,
        }),
        crate::providers::fshare::AcquisitionSource::TorrentInfoHash(hash) => Some(QueueSource {
            url: crate::sources::torrent::build_magnet_url(&hash, title.as_deref())?,
            filename: title,
            host: "qbittorrent".to_string(),
            download_source: "torrent".to_string(),
            info_hash: Some(hash.clone()),
            external_id: Some(hash),
        }),
    }
}

struct DownloadTitleLookup {
    tmdb_id: Option<i64>,
    raw_tmdb_id: Option<String>,
    media_type: String,
    title: Option<String>,
}

struct DownloadQueueRequest {
    url: String,
    filename: Option<String>,
    host: String,
    category: String,
    tmdb_metadata: Option<crate::downloader::TmdbDownloadMetadata>,
    download_source: Option<String>,
    info_hash: Option<String>,
    external_id: Option<String>,
}

async fn lookup_download_title(
    state: Arc<AppState>,
    lookup: DownloadTitleLookup,
) -> Result<(Option<String>, Option<i32>), StatusCode> {
    if let Some(item) = lookup
        .tmdb_id
        .and_then(|id| state.db.get_media_item(id).ok().flatten())
    {
        Ok((Some(item.title), item.year))
    } else if lookup.media_type == "tv" {
        Ok((lookup.title, None))
    } else if let Some(id) = lookup.raw_tmdb_id {
        Ok(crate::api::indexer::fetch_tmdb_title_and_year(
            &state.tmdb_service.api_key().await,
            &id,
            &lookup.media_type,
        )
        .await)
    } else {
        Ok((lookup.title, None))
    }
}

/// Parse an uploaded NZB and dispatch the exact orchestrator queue request.
async fn handle_add_file<Lookup, LookupFuture, Queue, QueueFuture>(
    params: SabParams,
    multipart: &mut axum::extract::Multipart,
    lookup_title: Lookup,
    queue_download: Queue,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    Lookup: FnOnce(DownloadTitleLookup) -> LookupFuture,
    LookupFuture: std::future::Future<Output = Result<(Option<String>, Option<i32>), StatusCode>>,
    Queue: FnOnce(DownloadQueueRequest) -> QueueFuture,
    QueueFuture: std::future::Future<Output = Result<String, StatusCode>>,
{
    tracing::info!("SABnzbd: Handling addfile request (NZB upload)");
    let category = params.cat.unwrap_or_else(|| "other".to_string());

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        tracing::error!("Failed to read multipart field: {}", error);
        StatusCode::BAD_REQUEST
    })? {
        let field_name = field.name().unwrap_or("");
        if field_name != "name" && field_name != "nzbfile" {
            continue;
        }

        let data = field.bytes().await.map_err(|error| {
            tracing::error!("Failed to read field bytes: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let metadata = crate::providers::fshare::decode_fake_nzb(&String::from_utf8_lossy(&data))
            .ok_or_else(|| {
            tracing::error!("Rejected invalid NZB acquisition metadata");
            StatusCode::BAD_REQUEST
        })?;
        let crate::providers::fshare::FakeNzbMetadata {
            source,
            tmdb_id,
            tvdb_id,
            title: title_from_nzb,
            media_type,
            season,
            episode,
            category: metadata_category,
        } = metadata;
        let actual_media_type = media_type
            .as_deref()
            .unwrap_or(if season.is_some() { "tv" } else { "movie" })
            .to_string();
        let tmdb_id_i64 = tmdb_id.as_deref().and_then(|id| id.parse::<i64>().ok());
        let tvdb_id_i64 = tvdb_id.as_deref().and_then(|id| id.parse::<i64>().ok());
        let (title, year) = lookup_title(DownloadTitleLookup {
            tmdb_id: tmdb_id_i64,
            raw_tmdb_id: tmdb_id,
            media_type: actual_media_type.clone(),
            title: title_from_nzb.clone(),
        })
        .await?;
        let tmdb_metadata = if actual_media_type == "tv" {
            let season = season.ok_or(StatusCode::BAD_REQUEST)?;
            if tvdb_id_i64.is_none() && tmdb_id_i64.is_none() {
                return Err(StatusCode::BAD_REQUEST);
            }
            Some(crate::downloader::TmdbDownloadMetadata {
                tmdb_id: tmdb_id_i64,
                tvdb_id: tvdb_id_i64,
                media_type: Some("tv".to_string()),
                title,
                year,
                collection_name: None,
                season: Some(season as i32),
                episode: episode.map(|value| value as i32),
            })
        } else {
            Some(crate::downloader::TmdbDownloadMetadata {
                tmdb_id: tmdb_id_i64,
                tvdb_id: None,
                media_type: Some("movie".to_string()),
                title,
                year,
                collection_name: None,
                season: None,
                episode: None,
            })
        };
        let source = queue_source(source, title_from_nzb).ok_or(StatusCode::BAD_REQUEST)?;
        let task_id = queue_download(DownloadQueueRequest {
            url: source.url,
            filename: source.filename,
            host: source.host,
            category: metadata_category.unwrap_or(category),
            tmdb_metadata,
            download_source: Some(source.download_source),
            info_hash: source.info_hash,
            external_id: source.external_id,
        })
        .await?;

        return Ok(Json(serde_json::json!({
            "status": true,
            "nzo_ids": [task_id]
        })));
    }

    tracing::error!("No NZB file found in multipart data");
    Err(StatusCode::BAD_REQUEST)
}

#[derive(Debug, PartialEq, Eq)]
struct AddUrlLogSummary {
    url_present: bool,
    filename_present: bool,
    source_class: &'static str,
}

fn addurl_log_summary(url: &str, filename: &str) -> AddUrlLogSummary {
    AddUrlLogSummary {
        url_present: !url.trim().is_empty(),
        filename_present: !filename.trim().is_empty(),
        source_class: if crate::providers::fshare::is_fshare_url(url) {
            "fshare"
        } else {
            "other"
        },
    }
}

/// Add URL to download queue
async fn handle_add_url(
    state: Arc<AppState>,
    params: SabParams,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let url = params.name.ok_or(StatusCode::BAD_REQUEST)?;
    let filename = params.nzbname.unwrap_or_else(|| "download".to_string());
    let category = params.cat.unwrap_or_else(|| "other".to_string());
    if category.to_ascii_lowercase().starts_with("tv") {
        tracing::warn!("SABnzbd addurl rejected: TV downloads require fake-NZB metadata");
        return Err(StatusCode::BAD_REQUEST);
    }

    let summary = addurl_log_summary(&url, &filename);
    tracing::info!(
        url_present = summary.url_present,
        filename_present = summary.filename_present,
        source_class = summary.source_class,
        "SABnzbd addurl accepted"
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
        match state
            .db
            .get_tasks_by_batch_id_async(batch_id.clone(), None)
            .await
        {
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

#[cfg(test)]
mod task33_tests {
    use super::*;
    use axum::{body::Body, extract::FromRequest, http::Request};
    use std::sync::Mutex;

    #[test]
    fn addurl_log_summary_never_contains_user_url_or_filename() {
        let sentinel = "https://fshare.vn/file/SECRET";
        let summary = addurl_log_summary(sentinel, "SECRET-file.mkv");

        assert_eq!(summary.source_class, "fshare");
        assert!(summary.url_present);
        assert!(summary.filename_present);
        assert!(!format!("{summary:?}").contains("SECRET"));
    }

    #[tokio::test]
    async fn multipart_torrent_nzb_reaches_add_download_with_metadata_unchanged() {
        let mut metadata = crate::providers::fshare::FakeNzbMetadata::torrent(
            "AABBCCDDEEFF00112233445566778899AABBCCDD".to_string(),
        );
        metadata.tvdb_id = Some("42".to_string());
        metadata.title = Some("Show.S01.mkv".to_string());
        metadata.media_type = Some("tv".to_string());
        metadata.season = Some(1);
        let nzb = crate::providers::fshare::encode_fake_nzb(&metadata).unwrap();
        let boundary = "task33-boundary";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"nzbfile\"; filename=\"Show.S01.nzb\"\r\nContent-Type: application/x-nzb\r\n\r\n{nzb}\r\n--{boundary}--\r\n"
        );
        let request = Request::builder()
            .header(
                axum::http::header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();
        let mut multipart = axum::extract::Multipart::from_request(request, &())
            .await
            .unwrap();
        let captured = Arc::new(Mutex::new(None));
        let queue_capture = Arc::clone(&captured);

        let response = handle_add_file(
            SabParams {
                mode: Some("addfile".to_string()),
                output: None,
                apikey: None,
                name: None,
                nzbname: None,
                cat: Some("tv".to_string()),
                priority: None,
                nzo_id: None,
            },
            &mut multipart,
            |lookup| async move { Ok((lookup.title, None)) },
            move |request| async move {
                *queue_capture.lock().unwrap() = Some(request);
                Ok("task-33-id".to_string())
            },
        )
        .await
        .unwrap();

        assert_eq!(response.0["nzo_ids"], serde_json::json!(["task-33-id"]));
        let request = captured.lock().unwrap().take().expect("queue call");
        assert_eq!(request.host, "qbittorrent");
        assert_eq!(request.download_source.as_deref(), Some("torrent"));
        assert_eq!(
            request.info_hash.as_deref(),
            Some("aabbccddeeff00112233445566778899aabbccdd")
        );
        assert_eq!(
            request.external_id.as_deref(),
            Some("aabbccddeeff00112233445566778899aabbccdd")
        );
        assert_eq!(request.tmdb_metadata.as_ref().unwrap().season, Some(1));
        assert_eq!(request.tmdb_metadata.as_ref().unwrap().episode, None);
        assert_eq!(
            request.url,
            "magnet:?xt=urn:btih:aabbccddeeff00112233445566778899aabbccdd&dn=Show.S01.mkv"
        );
    }
}
