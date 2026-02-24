//! Media entities module
//!
//! Defines MediaItem and MediaEpisode types for the TMDB-centric data model.
//! TMDB ID is the primary key for media_items, establishing it as the 
//! universal join key across downloads, *arr integration, and the frontend.

use serde::{Deserialize, Serialize};

/// A media entity (movie or TV series) identified by TMDB ID.
/// This is the central record that links downloads to *arr library state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaItem {
    /// TMDB ID — the universal key
    pub tmdb_id: i64,
    /// "movie" or "tv"
    pub media_type: String,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    /// JSON array of genre names
    pub genres: Option<String>,
    /// Runtime in minutes (movies only)
    pub runtime: Option<i32>,
    /// Total seasons (TV only)
    pub total_seasons: Option<i32>,

    // ── Arr integration state ──────────────────────────────────────────
    /// Sonarr series_id or Radarr movie_id
    pub arr_id: Option<i32>,
    /// "sonarr" or "radarr"
    pub arr_type: Option<String>,
    /// File system path in *arr library
    pub arr_path: Option<String>,
    pub arr_monitored: bool,
    /// e.g. "continuing", "ended", "released"
    pub arr_status: Option<String>,
    pub arr_quality_profile_id: Option<i32>,
    /// Radarr: does file exist on disk?
    pub arr_has_file: bool,
    /// Bytes on disk (from *arr)
    pub arr_size_on_disk: i64,

    // ── Cross-reference IDs ────────────────────────────────────────────
    pub tvdb_id: Option<i32>,
    pub imdb_id: Option<String>,

    // ── Timestamps ─────────────────────────────────────────────────────
    pub created_at: String,
    pub updated_at: String,
    /// Last time *arr data was synced for this item
    pub arr_synced_at: Option<String>,
}

/// A TV episode within a series, linked to media_items by tmdb_id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaEpisode {
    pub id: i64,
    /// TMDB ID of the parent series
    pub tmdb_id: i64,
    pub season_number: i32,
    pub episode_number: i32,
    pub title: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<String>,

    // ── Arr integration ────────────────────────────────────────────────
    /// Sonarr episode ID
    pub arr_episode_id: Option<i32>,
    pub arr_has_file: bool,
    pub arr_monitored: bool,

    // ── Timestamps ─────────────────────────────────────────────────────
    pub created_at: String,
    pub updated_at: String,
}

impl MediaItem {
    /// Create a new MediaItem with minimal required fields.
    /// Timestamps are set to now.
    pub fn new(tmdb_id: i64, media_type: &str, title: &str) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            tmdb_id,
            media_type: media_type.to_string(),
            title: title.to_string(),
            original_title: None,
            year: None,
            overview: None,
            poster_path: None,
            backdrop_path: None,
            genres: None,
            runtime: None,
            total_seasons: None,
            arr_id: None,
            arr_type: None,
            arr_path: None,
            arr_monitored: false,
            arr_status: None,
            arr_quality_profile_id: None,
            arr_has_file: false,
            arr_size_on_disk: 0,
            tvdb_id: None,
            imdb_id: None,
            created_at: now.clone(),
            updated_at: now,
            arr_synced_at: None,
        }
    }

    pub fn is_movie(&self) -> bool {
        self.media_type == "movie"
    }

    pub fn is_tv(&self) -> bool {
        self.media_type == "tv"
    }

    /// Check if this media item is linked to an *arr instance
    pub fn is_in_arr(&self) -> bool {
        self.arr_id.is_some()
    }
}

impl MediaEpisode {
    /// Create a new MediaEpisode with minimal required fields.
    pub fn new(tmdb_id: i64, season_number: i32, episode_number: i32) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: 0, // Will be set by AUTOINCREMENT
            tmdb_id,
            season_number,
            episode_number,
            title: None,
            overview: None,
            air_date: None,
            arr_episode_id: None,
            arr_has_file: false,
            arr_monitored: true,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}
