//! Arr Suite Proxy API
//!
//! Proxies requests to Sonarr/Radarr APIs so Flasharr frontend can be the single UI.
//! This module replaces the need for users to visit Sonarr/Radarr/Seerr directly.

use crate::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// Router
// ============================================================================

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // Library
        .route("/library", get(library_overview))
        .route("/series", get(all_series))
        .route("/movies", get(all_movies))
        .route("/episodes", get(episodes))
        // Calendar & Wanted
        .route("/calendar", get(calendar))
        .route("/missing", get(missing))
        // System
        .route("/storage", get(storage))
        .route("/health", get(arr_health))
        .route("/status", get(arr_status))
        .route("/history", get(history))
        // Library management
        .route("/series/add", post(add_series))
        .route("/movies/add", post(add_movie))
        // Queue management
        .route("/queue", get(queue_overview))
        .route("/queue/sweep", post(queue_sweep))
}

// ============================================================================
// Types
// ============================================================================

#[derive(Serialize)]
struct LibraryOverview {
    sonarr_connected: bool,
    radarr_connected: bool,
    series_count: usize,
    movie_count: usize,
    total_episodes: i32,
    episodes_with_files: i32,
    episodes_missing: i32,
    movies_with_files: usize,
    movies_missing: usize,
    total_size_on_disk: i64,
}

#[derive(Deserialize)]
struct EpisodesQuery {
    series_id: i32,
}

#[derive(Deserialize)]
struct CalendarQuery {
    start: Option<String>,
    end: Option<String>,
}

#[derive(Deserialize)]
struct MissingQuery {
    page: Option<i32>,
    page_size: Option<i32>,
}

#[derive(Deserialize)]
struct HistoryQuery {
    page_size: Option<i32>,
}

#[derive(Deserialize)]
struct AddToLibraryRequest {
    tmdb_id: i64,
}

#[derive(Serialize)]
struct AddToLibraryResponse {
    success: bool,
    /// Sonarr series ID or Radarr movie ID
    arr_id: i32,
    message: String,
}

#[derive(Serialize)]
struct ArrStatusResponse {
    sonarr: Option<ArrServiceStatus>,
    radarr: Option<ArrServiceStatus>,
}

#[derive(Serialize)]
struct ArrServiceStatus {
    connected: bool,
    version: Option<String>,
    start_time: Option<String>,
    health_issues: Vec<crate::arr::HealthCheck>,
}

#[derive(Serialize)]
struct HistoryResponse {
    sonarr: Option<serde_json::Value>,
    radarr: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct QueueOverviewResponse {
    sonarr_total: usize,
    sonarr_stuck: usize,
    radarr_total: usize,
    radarr_stuck: usize,
    sonarr_items: Vec<serde_json::Value>,
    radarr_items: Vec<serde_json::Value>,
}

#[derive(Serialize)]
struct QueueSweepResponse {
    sonarr_cleared: usize,
    radarr_cleared: usize,
    series_rescanned: usize,
    movies_rescanned: usize,
    errors: Vec<String>,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/arr/library — Combined library overview (series + movies stats)
async fn library_overview(
    State(state): State<Arc<AppState>>,
) -> Result<Json<LibraryOverview>, StatusCode> {
    let client = state
        .download_orchestrator
        .get_arr_client()
        .await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let sonarr_connected = client.has_sonarr();
    let radarr_connected = client.has_radarr();

    // Fetch series stats
    let (series_count, total_episodes, episodes_with_files, series_size) = if sonarr_connected {
        match client.get_all_series().await {
            Ok(series) => {
                let count = series.len();
                let total_eps: i32 = series
                    .iter()
                    .filter_map(|s| s.statistics.as_ref().and_then(|st| st.episode_count))
                    .sum();
                let file_eps: i32 = series
                    .iter()
                    .filter_map(|s| s.statistics.as_ref().and_then(|st| st.episode_file_count))
                    .sum();
                let size: i64 = series
                    .iter()
                    .filter_map(|s| s.statistics.as_ref().and_then(|st| st.size_on_disk))
                    .sum();
                (count, total_eps, file_eps, size)
            }
            Err(_) => (0, 0, 0, 0),
        }
    } else {
        (0, 0, 0, 0)
    };

    // Fetch movie stats
    let (movie_count, movies_with_files_count, movies_missing_count, movie_size) =
        if radarr_connected {
            match client.get_all_movies().await {
                Ok(movies) => {
                    let count = movies.len();
                    let with_files = movies.iter().filter(|m| m.has_file == Some(true)).count();
                    let missing = count - with_files;
                    let size: i64 = movies.iter().filter_map(|m| m.size_on_disk).sum();
                    (count, with_files, missing, size)
                }
                Err(_) => (0, 0, 0, 0),
            }
        } else {
            (0, 0, 0, 0)
        };

    Ok(Json(LibraryOverview {
        sonarr_connected,
        radarr_connected,
        series_count,
        movie_count,
        total_episodes,
        episodes_with_files,
        episodes_missing: total_episodes - episodes_with_files,
        movies_with_files: movies_with_files_count,
        movies_missing: movies_missing_count,
        total_size_on_disk: series_size + movie_size,
    }))
}

/// GET /api/arr/series — All series from Sonarr
async fn all_series(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::arr::SonarrSeries>>, StatusCode> {
    let client = state
        .download_orchestrator
        .get_arr_client()
        .await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    client.get_all_series().await.map(Json).map_err(|e| {
        tracing::error!("Failed to get series: {}", e);
        StatusCode::BAD_GATEWAY
    })
}

/// GET /api/arr/movies — All movies from Radarr
async fn all_movies(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::arr::RadarrMovie>>, StatusCode> {
    let client = state
        .download_orchestrator
        .get_arr_client()
        .await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    client.get_all_movies().await.map(Json).map_err(|e| {
        tracing::error!("Failed to get movies: {}", e);
        StatusCode::BAD_GATEWAY
    })
}

/// GET /api/arr/episodes?series_id=X — Episodes for a series
async fn episodes(
    State(state): State<Arc<AppState>>,
    Query(params): Query<EpisodesQuery>,
) -> Result<Json<Vec<crate::arr::SonarrEpisode>>, StatusCode> {
    let client = state
        .download_orchestrator
        .get_arr_client()
        .await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    client
        .get_episodes(params.series_id)
        .await
        .map(Json)
        .map_err(|e| {
            tracing::error!("Failed to get episodes: {}", e);
            StatusCode::BAD_GATEWAY
        })
}

/// GET /api/arr/calendar — Upcoming episodes from Sonarr
async fn calendar(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CalendarQuery>,
) -> Result<Json<Vec<crate::arr::SonarrCalendarEntry>>, StatusCode> {
    let client = state
        .download_orchestrator
        .get_arr_client()
        .await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    // Default: today to 14 days from now
    let start = params
        .start
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let end = params.end.unwrap_or_else(|| {
        (chrono::Utc::now() + chrono::Duration::days(14))
            .format("%Y-%m-%d")
            .to_string()
    });

    client
        .get_calendar(&start, &end)
        .await
        .map(Json)
        .map_err(|e| {
            tracing::error!("Failed to get calendar: {}", e);
            StatusCode::BAD_GATEWAY
        })
}

/// GET /api/arr/missing — Missing episodes and movies
async fn missing(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MissingQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = state
        .download_orchestrator
        .get_arr_client()
        .await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let page = params.page.unwrap_or(1);
    let page_size = params.page_size.unwrap_or(20);

    let mut result = serde_json::json!({});

    // Missing episodes from Sonarr - enrich with series info
    if client.has_sonarr() {
        match client.get_missing_episodes(page, page_size).await {
            Ok(mut data) => {
                // Fetch all series to enrich episode data
                if let Ok(all_series) = client.get_all_series().await {
                    // Create a map of seriesId -> series for quick lookup
                    let series_map: std::collections::HashMap<i32, &crate::arr::SonarrSeries> =
                        all_series.iter().map(|s| (s.id, s)).collect();

                    // Enrich each episode with series information
                    if let Some(records) = data.get_mut("records").and_then(|r| r.as_array_mut()) {
                        for episode in records.iter_mut() {
                            if let Some(series_id) =
                                episode.get("seriesId").and_then(|id| id.as_i64())
                            {
                                if let Some(series) = series_map.get(&(series_id as i32)) {
                                    episode["series"] = serde_json::json!({
                                        "title": series.title,
                                        "tvdbId": series.tvdb_id,
                                        "id": series.id
                                    });
                                }
                            }
                        }
                    }
                }
                result["episodes"] = data;
            }
            Err(e) => tracing::warn!("Failed to get missing episodes: {}", e),
        }
    }

    // Missing movies from Radarr
    if client.has_radarr() {
        match client.get_missing_movies(page, page_size).await {
            Ok(data) => {
                result["movies"] = data;
            }
            Err(e) => tracing::warn!("Failed to get missing movies: {}", e),
        }
    }

    Ok(Json(result))
}

/// GET /api/arr/storage — Disk space from Sonarr
async fn storage(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::arr::DiskSpace>>, StatusCode> {
    let client = state
        .download_orchestrator
        .get_arr_client()
        .await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    client.get_sonarr_disk_space().await.map(Json).map_err(|e| {
        tracing::error!("Failed to get disk space: {}", e);
        StatusCode::BAD_GATEWAY
    })
}

/// GET /api/arr/health — Health status of Sonarr and Radarr
async fn arr_health(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ArrStatusResponse>, StatusCode> {
    let client = state
        .download_orchestrator
        .get_arr_client()
        .await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let sonarr = if client.has_sonarr() {
        let status = client.get_sonarr_status().await.ok();
        let health = client.get_sonarr_health().await.unwrap_or_default();
        Some(ArrServiceStatus {
            connected: status.is_some(),
            version: status.as_ref().and_then(|s| s.version.clone()),
            start_time: status.and_then(|s| s.start_time),
            health_issues: health,
        })
    } else {
        None
    };

    let radarr = if client.has_radarr() {
        let status = client.get_radarr_status().await.ok();
        Some(ArrServiceStatus {
            connected: status.is_some(),
            version: status.as_ref().and_then(|s| s.version.clone()),
            start_time: status.and_then(|s| s.start_time),
            health_issues: vec![],
        })
    } else {
        None
    };

    Ok(Json(ArrStatusResponse { sonarr, radarr }))
}

/// GET /api/arr/status — System status of Sonarr and Radarr
async fn arr_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ArrStatusResponse>, StatusCode> {
    arr_health(State(state)).await
}

/// GET /api/arr/history — Recent import history from both services
async fn history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HistoryQuery>,
) -> Result<Json<HistoryResponse>, StatusCode> {
    let client = state
        .download_orchestrator
        .get_arr_client()
        .await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let page_size = params.page_size.unwrap_or(20);

    let sonarr = if client.has_sonarr() {
        client.get_sonarr_history(page_size).await.ok()
    } else {
        None
    };

    let radarr = if client.has_radarr() {
        client.get_radarr_history(page_size).await.ok()
    } else {
        None
    };

    Ok(Json(HistoryResponse { sonarr, radarr }))
}

// ============================================================================
// Library Management Handlers
// ============================================================================

/// GET /api/arr/queue — Returns current activity queues from Sonarr and Radarr
async fn queue_overview(
    State(state): State<Arc<AppState>>,
) -> Result<Json<QueueOverviewResponse>, StatusCode> {
    let client = state
        .download_orchestrator
        .get_arr_client()
        .await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let (sonarr_items, sonarr_total) = if client.has_sonarr() {
        match client.get_sonarr_queue().await {
            Ok(items) => {
                let total = items.len();
                (items, total)
            }
            Err(e) => {
                tracing::warn!("Failed to fetch Sonarr queue: {}", e);
                (vec![], 0)
            }
        }
    } else {
        (vec![], 0)
    };

    let (radarr_items, radarr_total) = if client.has_radarr() {
        match client.get_radarr_queue().await {
            Ok(items) => {
                let total = items.len();
                (items, total)
            }
            Err(e) => {
                tracing::warn!("Failed to fetch Radarr queue: {}", e);
                (vec![], 0)
            }
        }
    } else {
        (vec![], 0)
    };

    let sonarr_stuck = sonarr_items
        .iter()
        .filter(|i| is_stuck_sonarr_item(i))
        .count();
    let radarr_stuck = radarr_items
        .iter()
        .filter(|i| is_stuck_radarr_item(i))
        .count();

    Ok(Json(QueueOverviewResponse {
        sonarr_total,
        sonarr_stuck,
        radarr_total,
        radarr_stuck,
        sonarr_items,
        radarr_items,
    }))
}

/// Returns true for Sonarr queue items that are permanently stuck and safe to remove:
/// - `completed`: import is pending/blocked (file exists but arr can't auto-import it)
/// - `downloadClientUnavailable` with no downloadId: phantom entry — Sonarr grabbed the release
///   but never assigned it to a download client (no client was available at grab time)
/// - `failed`: download or grab failed; Sonarr won't auto-retry these
fn is_stuck_sonarr_item(item: &serde_json::Value) -> bool {
    match item["status"].as_str().unwrap_or("") {
        "completed" | "failed" => true,
        "downloadClientUnavailable" => {
            // Only phantom entries (never assigned): downloadId absent or empty
            item["downloadId"]
                .as_str()
                .map(|s| s.is_empty())
                .unwrap_or(true)
                && item["downloadClientId"].as_i64().is_none()
        }
        _ => false,
    }
}

/// Returns true for Radarr queue items that are permanently stuck and safe to remove.
/// Same categories as Sonarr — the API shape is identical.
fn is_stuck_radarr_item(item: &serde_json::Value) -> bool {
    is_stuck_sonarr_item(item)
}

/// POST /api/arr/queue/sweep — Clears all permanently stuck queue entries and triggers rescans.
///
/// Handles three categories:
/// - `completed`: file exists in library but arr's DownloadedEpisodesScan loop can't import it.
/// - `downloadClientUnavailable` (no downloadId): phantom entries that were never sent to a
///   download client and will never resolve on their own.
/// - `failed`: downloads that failed in the client or weren't grabbed; arr won't auto-retry.
///
/// Fix for each: remove the stale queue entry, then issue RescanSeries/RescanMovie so arr picks
/// up any file that is already on disk.
async fn queue_sweep(
    State(state): State<Arc<AppState>>,
) -> Result<Json<QueueSweepResponse>, StatusCode> {
    let client = state
        .download_orchestrator
        .get_arr_client()
        .await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let mut errors = Vec::<String>::new();
    let mut sonarr_cleared = 0usize;
    let mut radarr_cleared = 0usize;
    let mut series_rescanned = 0usize;
    let mut movies_rescanned = 0usize;

    // ---- Sonarr sweep ----
    if client.has_sonarr() {
        match client.get_sonarr_queue().await {
            Ok(items) => {
                let stuck: Vec<&serde_json::Value> = items
                    .iter()
                    .filter(|item| is_stuck_sonarr_item(item))
                    .collect();

                let ids: Vec<i64> = stuck
                    .iter()
                    .filter_map(|item| item["id"].as_i64())
                    .collect();

                // Collect unique series IDs to rescan after clearing
                let mut series_ids = std::collections::HashSet::<i32>::new();
                for item in &stuck {
                    if let Some(sid) = item["seriesId"].as_i64() {
                        series_ids.insert(sid as i32);
                    }
                }

                if !ids.is_empty() {
                    tracing::info!(
                        "[QUEUE-SWEEP] Bulk-deleting {} stuck Sonarr queue entries",
                        ids.len()
                    );
                    match client.bulk_delete_sonarr_queue(&ids, false).await {
                        Ok(()) => {
                            sonarr_cleared = ids.len();
                        }
                        Err(e) => {
                            errors.push(format!("Sonarr bulk delete failed: {}", e));
                        }
                    }
                }

                // Trigger rescan for every affected series so Sonarr picks up files
                for series_id in series_ids {
                    match client.trigger_series_rescan_by_id(series_id).await {
                        Ok(()) => {
                            tracing::info!(
                                "[QUEUE-SWEEP] Triggered RescanSeries for seriesId={}",
                                series_id
                            );
                            series_rescanned += 1;
                        }
                        Err(e) => {
                            errors
                                .push(format!("RescanSeries seriesId={} failed: {}", series_id, e));
                        }
                    }
                }
            }
            Err(e) => {
                errors.push(format!("Failed to fetch Sonarr queue: {}", e));
            }
        }
    }

    // ---- Radarr sweep ----
    if client.has_radarr() {
        match client.get_radarr_queue().await {
            Ok(items) => {
                let stuck: Vec<&serde_json::Value> = items
                    .iter()
                    .filter(|item| is_stuck_radarr_item(item))
                    .collect();

                // Items with a known movieId can be bulk-deleted safely with blocklist=false.
                // Items with no movieId ("unknown" items) trigger a NullReferenceException in
                // Radarr's IgnoredDownloadService when blocklist=false — use blocklist=true as
                // a workaround; the null downloadClientId prevents any actual blocklist entry.
                let known_ids: Vec<i64> = stuck
                    .iter()
                    .filter(|item| item["movieId"].is_number())
                    .filter_map(|item| item["id"].as_i64())
                    .collect();
                let unknown_ids: Vec<i64> = stuck
                    .iter()
                    .filter(|item| !item["movieId"].is_number())
                    .filter_map(|item| item["id"].as_i64())
                    .collect();

                let mut movie_ids = std::collections::HashSet::<i32>::new();
                for item in &stuck {
                    if let Some(mid) = item["movieId"].as_i64() {
                        movie_ids.insert(mid as i32);
                    }
                }

                if !known_ids.is_empty() {
                    tracing::info!(
                        "[QUEUE-SWEEP] Bulk-deleting {} known Radarr queue entries",
                        known_ids.len()
                    );
                    match client.bulk_delete_radarr_queue(&known_ids, false).await {
                        Ok(()) => radarr_cleared += known_ids.len(),
                        Err(e) => errors.push(format!("Radarr bulk delete (known) failed: {}", e)),
                    }
                }

                // Unknown items: delete individually with blocklist=true to avoid the NullRef
                for item_id in unknown_ids {
                    match client.delete_radarr_queue_item(item_id, true).await {
                        Ok(()) => radarr_cleared += 1,
                        Err(e) => {
                            errors.push(format!("Radarr delete item {} failed: {}", item_id, e))
                        }
                    }
                }

                for movie_id in movie_ids {
                    match client.trigger_movie_rescan_by_id(movie_id).await {
                        Ok(()) => {
                            tracing::info!(
                                "[QUEUE-SWEEP] Triggered RescanMovie for movieId={}",
                                movie_id
                            );
                            movies_rescanned += 1;
                        }
                        Err(e) => {
                            errors.push(format!("RescanMovie movieId={} failed: {}", movie_id, e));
                        }
                    }
                }
            }
            Err(e) => {
                errors.push(format!("Failed to fetch Radarr queue: {}", e));
            }
        }
    }

    tracing::info!(
        "[QUEUE-SWEEP] Done — sonarr_cleared={}, radarr_cleared={}, series_rescanned={}, movies_rescanned={}",
        sonarr_cleared,
        radarr_cleared,
        series_rescanned,
        movies_rescanned
    );

    Ok(Json(QueueSweepResponse {
        sonarr_cleared,
        radarr_cleared,
        series_rescanned,
        movies_rescanned,
        errors,
    }))
}

/// POST /api/arr/series/add — Add TV show to Sonarr by TMDB ID.
/// Auto-selects the first available root folder; quality profile defaults to 1 ("Any").
async fn add_series(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddToLibraryRequest>,
) -> Result<Json<AddToLibraryResponse>, (axum::http::StatusCode, String)> {
    let client = state
        .download_orchestrator
        .get_arr_client()
        .await
        .ok_or_else(|| {
            (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "Arr client not available".to_string(),
            )
        })?;

    let root_folders = client.get_sonarr_root_folders().await.map_err(|e| {
        (
            axum::http::StatusCode::BAD_GATEWAY,
            format!("Failed to get Sonarr root folders: {}", e),
        )
    })?;

    let root_folder = root_folders.first().ok_or_else(|| {
        (
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "No root folders configured in Sonarr".to_string(),
        )
    })?;

    match client
        .add_series_by_tmdb(body.tmdb_id, 1, &root_folder.path)
        .await
    {
        Ok(series_id) => Ok(Json(AddToLibraryResponse {
            success: true,
            arr_id: series_id,
            message: format!("Series added to Sonarr (ID: {})", series_id),
        })),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("already exists") || msg.contains("already been added") {
                Err((
                    axum::http::StatusCode::CONFLICT,
                    "Series is already in the Sonarr library".to_string(),
                ))
            } else {
                Err((
                    axum::http::StatusCode::BAD_GATEWAY,
                    format!("Sonarr error: {}", msg),
                ))
            }
        }
    }
}

/// POST /api/arr/movies/add — Add movie to Radarr by TMDB ID.
/// Auto-selects the first available root folder; quality profile defaults to 1 ("Any").
async fn add_movie(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddToLibraryRequest>,
) -> Result<Json<AddToLibraryResponse>, (axum::http::StatusCode, String)> {
    let client = state
        .download_orchestrator
        .get_arr_client()
        .await
        .ok_or_else(|| {
            (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "Arr client not available".to_string(),
            )
        })?;

    let root_folders = client.get_radarr_root_folders().await.map_err(|e| {
        (
            axum::http::StatusCode::BAD_GATEWAY,
            format!("Failed to get Radarr root folders: {}", e),
        )
    })?;

    let root_folder = root_folders.first().ok_or_else(|| {
        (
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "No root folders configured in Radarr".to_string(),
        )
    })?;

    match client
        .add_movie_by_tmdb(body.tmdb_id, 1, &root_folder.path)
        .await
    {
        Ok(movie_id) => Ok(Json(AddToLibraryResponse {
            success: true,
            arr_id: movie_id,
            message: format!("Movie added to Radarr (ID: {})", movie_id),
        })),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("already exists") || msg.contains("already been added") {
                Err((
                    axum::http::StatusCode::CONFLICT,
                    "Movie is already in the Radarr library".to_string(),
                ))
            } else {
                Err((
                    axum::http::StatusCode::BAD_GATEWAY,
                    format!("Radarr error: {}", msg),
                ))
            }
        }
    }
}
