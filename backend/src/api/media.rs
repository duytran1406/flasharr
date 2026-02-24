use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use std::sync::Arc;
use std::collections::HashMap;

use crate::AppState;
use crate::db::media::{MediaItem, MediaEpisode};

#[derive(Serialize)]
pub struct MediaItemResponse {
    #[serde(flatten)]
    pub item: MediaItem,
    /// Number of downloads associated with this media item
    pub download_count: usize,
}

#[derive(Serialize)]
pub struct MediaDetailResponse {
    #[serde(flatten)]
    pub item: MediaItem,
    pub episodes: Vec<MediaEpisode>,
    pub downloads: Vec<crate::downloader::task::DownloadTask>,
}

/// A download instance with quality info for the grouped response
#[derive(Serialize)]
pub struct DownloadInstance {
    pub id: String,
    pub filename: String,
    pub quality: Option<String>,
    pub resolution: Option<String>,
    pub size: u64,
    pub state: String,
    pub progress: f32,
    pub created_at: String,
}

/// Downloads grouped by episode key (e.g., "S01E01") or "movie"
#[derive(Serialize)]
pub struct MediaDownloadsResponse {
    pub tmdb_id: i64,
    pub media_type: String,
    pub title: String,
    /// Map of episode key → list of download instances
    /// Keys: "S01E01", "S02E03", "movie" (for movies)
    pub episodes: HashMap<String, Vec<DownloadInstance>>,
    /// Total number of downloads across all episodes
    pub total_downloads: usize,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_media))
        .route("/:tmdb_id", get(get_media_detail))
        .route("/:tmdb_id/episodes", get(get_media_episodes))
        .route("/:tmdb_id/downloads", get(get_media_downloads))
}

/// GET /api/media — List all media items with download counts
async fn list_media(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<MediaItemResponse>>, (axum::http::StatusCode, String)> {
    let items = state.db.get_all_media_items_async().await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    
    let counts = state.db.get_media_download_counts()
        .unwrap_or_default();
    
    let response: Vec<MediaItemResponse> = items
        .into_iter()
        .map(|item| {
            let count = counts.get(&item.tmdb_id).copied().unwrap_or(0);
            MediaItemResponse {
                item,
                download_count: count,
            }
        })
        .collect();
    
    Ok(Json(response))
}

/// GET /api/media/:tmdb_id — Get a single media item with episodes and downloads
async fn get_media_detail(
    State(state): State<Arc<AppState>>,
    Path(tmdb_id): Path<i64>,
) -> Result<Json<MediaDetailResponse>, (axum::http::StatusCode, String)> {
    let result = state.db.get_media_with_downloads_async(tmdb_id).await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    
    match result {
        Some((item, downloads)) => {
            let episodes = state.db.get_episodes_for_series_async(tmdb_id).await
                .unwrap_or_default();
            
            Ok(Json(MediaDetailResponse {
                item,
                episodes,
                downloads,
            }))
        }
        None => Err((axum::http::StatusCode::NOT_FOUND, format!("Media item with TMDB ID {} not found", tmdb_id))),
    }
}

/// GET /api/media/:tmdb_id/episodes — Get episodes for a TV series
async fn get_media_episodes(
    State(state): State<Arc<AppState>>,
    Path(tmdb_id): Path<i64>,
) -> Result<Json<Vec<MediaEpisode>>, (axum::http::StatusCode, String)> {
    let episodes = state.db.get_episodes_for_series_async(tmdb_id).await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    
    Ok(Json(episodes))
}

/// GET /api/media/:tmdb_id/downloads — Get downloads grouped by episode with quality info
async fn get_media_downloads(
    State(state): State<Arc<AppState>>,
    Path(tmdb_id): Path<i64>,
) -> Result<Json<MediaDownloadsResponse>, (axum::http::StatusCode, String)> {
    let result = state.db.get_media_with_downloads_async(tmdb_id).await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    
    match result {
        Some((item, downloads)) => {
            let total = downloads.len();
            let mut episodes: HashMap<String, Vec<DownloadInstance>> = HashMap::new();
            
            for dl in downloads {
                let key = match (dl.tmdb_season, dl.tmdb_episode) {
                    (Some(s), Some(e)) => format!("S{:02}E{:02}", s, e),
                    _ => "movie".to_string(),
                };
                
                episodes.entry(key).or_default().push(DownloadInstance {
                    id: dl.id.to_string(),
                    filename: dl.filename.clone(),
                    quality: dl.quality.clone(),
                    resolution: dl.resolution.clone(),
                    size: dl.size,
                    state: format!("{:?}", dl.state).to_uppercase(),
                    progress: dl.progress,
                    created_at: dl.created_at.to_rfc3339(),
                });
            }
            
            Ok(Json(MediaDownloadsResponse {
                tmdb_id,
                media_type: item.media_type.clone(),
                title: item.title.clone(),
                episodes,
                total_downloads: total,
            }))
        }
        None => Err((axum::http::StatusCode::NOT_FOUND, format!("Media item {} not found", tmdb_id))),
    }
}
