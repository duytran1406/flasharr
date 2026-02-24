//! Arr Suite Proxy API
//!
//! Proxies requests to Sonarr/Radarr APIs so Flasharr frontend can be the single UI.
//! This module replaces the need for users to visit Sonarr/Radarr/Seerr directly.

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::AppState;

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

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/arr/library — Combined library overview (series + movies stats)
async fn library_overview(
    State(state): State<Arc<AppState>>,
) -> Result<Json<LibraryOverview>, StatusCode> {
    let client = state.download_orchestrator.get_arr_client().await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let sonarr_connected = client.has_sonarr();
    let radarr_connected = client.has_radarr();

    // Fetch series stats
    let (series_count, total_episodes, episodes_with_files, series_size) = if sonarr_connected {
        match client.get_all_series().await {
            Ok(series) => {
                let count = series.len();
                let total_eps: i32 = series.iter()
                    .filter_map(|s| s.statistics.as_ref().and_then(|st| st.episode_count))
                    .sum();
                let file_eps: i32 = series.iter()
                    .filter_map(|s| s.statistics.as_ref().and_then(|st| st.episode_file_count))
                    .sum();
                let size: i64 = series.iter()
                    .filter_map(|s| s.statistics.as_ref().and_then(|st| st.size_on_disk))
                    .sum();
                (count, total_eps, file_eps, size)
            }
            Err(_) => (0, 0, 0, 0)
        }
    } else {
        (0, 0, 0, 0)
    };

    // Fetch movie stats
    let (movie_count, movies_with_files_count, movies_missing_count, movie_size) = if radarr_connected {
        match client.get_all_movies().await {
            Ok(movies) => {
                let count = movies.len();
                let with_files = movies.iter()
                    .filter(|m| m.has_file == Some(true))
                    .count();
                let missing = count - with_files;
                let size: i64 = movies.iter()
                    .filter_map(|m| m.size_on_disk)
                    .sum();
                (count, with_files, missing, size)
            }
            Err(_) => (0, 0, 0, 0)
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
    let client = state.download_orchestrator.get_arr_client().await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    client.get_all_series().await
        .map(Json)
        .map_err(|e| {
            tracing::error!("Failed to get series: {}", e);
            StatusCode::BAD_GATEWAY
        })
}

/// GET /api/arr/movies — All movies from Radarr
async fn all_movies(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::arr::RadarrMovie>>, StatusCode> {
    let client = state.download_orchestrator.get_arr_client().await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    client.get_all_movies().await
        .map(Json)
        .map_err(|e| {
            tracing::error!("Failed to get movies: {}", e);
            StatusCode::BAD_GATEWAY
        })
}

/// GET /api/arr/episodes?series_id=X — Episodes for a series
async fn episodes(
    State(state): State<Arc<AppState>>,
    Query(params): Query<EpisodesQuery>,
) -> Result<Json<Vec<crate::arr::SonarrEpisode>>, StatusCode> {
    let client = state.download_orchestrator.get_arr_client().await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    client.get_episodes(params.series_id).await
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
    let client = state.download_orchestrator.get_arr_client().await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    // Default: today to 14 days from now
    let start = params.start.unwrap_or_else(|| {
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    });
    let end = params.end.unwrap_or_else(|| {
        (chrono::Utc::now() + chrono::Duration::days(14)).format("%Y-%m-%d").to_string()
    });

    client.get_calendar(&start, &end).await
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
    let client = state.download_orchestrator.get_arr_client().await
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
                            if let Some(series_id) = episode.get("seriesId").and_then(|id| id.as_i64()) {
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
            Ok(data) => { result["movies"] = data; }
            Err(e) => tracing::warn!("Failed to get missing movies: {}", e),
        }
    }

    Ok(Json(result))
}

/// GET /api/arr/storage — Disk space from Sonarr
async fn storage(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::arr::DiskSpace>>, StatusCode> {
    let client = state.download_orchestrator.get_arr_client().await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    client.get_sonarr_disk_space().await
        .map(Json)
        .map_err(|e| {
            tracing::error!("Failed to get disk space: {}", e);
            StatusCode::BAD_GATEWAY
        })
}

/// GET /api/arr/health — Health status of Sonarr and Radarr
async fn arr_health(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ArrStatusResponse>, StatusCode> {
    let client = state.download_orchestrator.get_arr_client().await
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
    let client = state.download_orchestrator.get_arr_client().await
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
