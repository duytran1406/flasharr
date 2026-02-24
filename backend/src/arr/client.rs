//! *arr API Client
//!
//! HTTP client for communicating with Sonarr and Radarr APIs.
//! Triggers automatic imports when downloads complete.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use crate::config::ArrConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootFolder {
    pub id: i32,
    pub path: String,
    #[serde(rename = "freeSpace")]
    pub free_space: Option<i64>,
}

/// *arr API client for triggering imports
pub struct ArrClient {
    http_client: Client,
    sonarr_config: Option<ArrConfig>,
    radarr_config: Option<ArrConfig>,
}



// ============================================================================
// Sonarr Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonarrStatistics {
    #[serde(rename = "seasonCount", default)]
    pub season_count: Option<i32>,
    #[serde(rename = "episodeFileCount", default)]
    pub episode_file_count: Option<i32>,
    #[serde(rename = "episodeCount", default)]
    pub episode_count: Option<i32>,
    #[serde(rename = "totalEpisodeCount", default)]
    pub total_episode_count: Option<i32>,
    #[serde(rename = "sizeOnDisk", default)]
    pub size_on_disk: Option<i64>,
    #[serde(rename = "percentOfEpisodes", default)]
    pub percent_of_episodes: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonarrSeries {
    pub id: i32,
    pub title: String,
    #[serde(rename = "tvdbId")]
    pub tvdb_id: Option<i32>,
    #[serde(rename = "tmdbId")]
    pub tmdb_id: Option<i32>,
    pub path: Option<String>,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub status: Option<String>,
    pub monitored: Option<bool>,
    pub images: Option<Vec<MediaImage>>,
    #[serde(rename = "qualityProfileId")]
    pub quality_profile_id: Option<i32>,
    pub statistics: Option<SonarrStatistics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonarrEpisode {
    pub id: i32,
    #[serde(rename = "seriesId")]
    pub series_id: i32,
    pub title: Option<String>,
    #[serde(rename = "seasonNumber")]
    pub season_number: i32,
    #[serde(rename = "episodeNumber")]
    pub episode_number: i32,
    pub overview: Option<String>,
    #[serde(rename = "hasFile")]
    pub has_file: bool,
    pub monitored: bool,
    #[serde(rename = "airDateUtc")]
    pub air_date_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonarrCalendarEntry {
    pub id: i32,
    #[serde(rename = "seriesId")]
    pub series_id: i32,
    pub title: Option<String>,
    #[serde(rename = "seasonNumber")]
    pub season_number: i32,
    #[serde(rename = "episodeNumber")]
    pub episode_number: i32,
    #[serde(rename = "airDateUtc")]
    pub air_date_utc: Option<String>,
    #[serde(rename = "hasFile")]
    pub has_file: bool,
    pub series: Option<SonarrCalendarSeries>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonarrCalendarSeries {
    pub id: i32,
    pub title: String,
    pub images: Option<Vec<MediaImage>>,
}

// ============================================================================
// Radarr Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarrMovie {
    pub id: i32,
    pub title: String,
    #[serde(rename = "tmdbId")]
    pub tmdb_id: i32,
    pub path: Option<String>,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub status: Option<String>,
    pub monitored: Option<bool>,
    #[serde(rename = "hasFile")]
    pub has_file: Option<bool>,
    #[serde(rename = "sizeOnDisk")]
    pub size_on_disk: Option<i64>,
    pub images: Option<Vec<MediaImage>>,
    #[serde(rename = "qualityProfileId")]
    pub quality_profile_id: Option<i32>,
    pub runtime: Option<i32>,
}

// ============================================================================
// Shared Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaImage {
    #[serde(rename = "coverType")]
    pub cover_type: String,
    pub url: Option<String>,
    #[serde(rename = "remoteUrl")]
    pub remote_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskSpace {
    pub path: String,
    pub label: Option<String>,
    #[serde(rename = "freeSpace")]
    pub free_space: i64,
    #[serde(rename = "totalSpace")]
    pub total_space: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrHistoryRecord {
    pub id: i32,
    #[serde(rename = "sourceTitle")]
    pub source_title: Option<String>,
    pub quality: Option<serde_json::Value>,
    #[serde(rename = "eventType")]
    pub event_type: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub version: Option<String>,
    #[serde(rename = "startTime")]
    pub start_time: Option<String>,
    #[serde(rename = "appName")]
    pub app_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub source: Option<String>,
    #[serde(rename = "type")]
    pub check_type: Option<String>,
    pub message: Option<String>,
    #[serde(rename = "wikiUrl")]
    pub wiki_url: Option<String>,
}


impl ArrClient {
    /// Create a new *arr client with configuration
    pub fn new(sonarr_config: Option<ArrConfig>, radarr_config: Option<ArrConfig>) -> Self {
        Self {
            http_client: Client::new(),
            sonarr_config,
            radarr_config,
        }
    }



    /// Test connection to an *arr service (Sonarr or Radarr)
    pub async fn test_connection(url: &str, api_key: &str, service_name: &str) -> anyhow::Result<String> {
        let client = Client::new();
        let test_url = format!("{}/api/v3/system/status", url.trim_end_matches('/'));
        
        let response = client
            .get(&test_url)
            .header("X-Api-Key", api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Connection failed: HTTP {}", response.status());
        }

        #[derive(Deserialize)]
        struct SystemStatus {
            version: String,
        }

        let status: SystemStatus = response.json().await?;
        Ok(format!("Connected to {} v{}", service_name, status.version))
    }

    /// Test connection to Sonarr (convenience wrapper)
    pub async fn test_sonarr_connection(url: &str, api_key: &str) -> anyhow::Result<String> {
        Self::test_connection(url, api_key, "Sonarr").await
    }

    /// Test connection to Radarr (convenience wrapper)
    pub async fn test_radarr_connection(url: &str, api_key: &str) -> anyhow::Result<String> {
        Self::test_connection(url, api_key, "Radarr").await
    }


    // ============================================================================
    // Series/Movie Management Methods
    // ============================================================================

    /// Add series to Sonarr by TMDB ID
    /// Returns Sonarr series ID on success
    pub async fn add_series_by_tmdb(
        &self,
        tmdb_id: i64,
        quality_profile_id: i32,
        root_folder_path: &str,
    ) -> anyhow::Result<i32> {
        let config = self.sonarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sonarr not configured"))?;

        if !config.enabled {
            anyhow::bail!("Sonarr is disabled");
        }

        // Step 1: Convert TMDB ID to TVDB ID using Sonarr's lookup
        let tvdb_id = self.tmdb_to_tvdb_via_sonarr(tmdb_id, config).await?;

        // Step 2: Get series details from Sonarr lookup
        let lookup_url = format!(
            "{}/api/v3/series/lookup?term=tvdb:{}",
            config.url.trim_end_matches('/'),
            tvdb_id
        );

        let lookup_response = self.http_client
            .get(&lookup_url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !lookup_response.status().is_success() {
            anyhow::bail!("Failed to lookup series: HTTP {}", lookup_response.status());
        }

        let mut lookup_results: Vec<serde_json::Value> = lookup_response.json().await?;
        let series_data = lookup_results.first_mut()
            .ok_or_else(|| anyhow::anyhow!("No series found for TVDB ID {}", tvdb_id))?;

        // Step 3: Modify series data for adding
        series_data["qualityProfileId"] = serde_json::json!(quality_profile_id);
        series_data["rootFolderPath"] = serde_json::json!(root_folder_path);
        series_data["monitored"] = serde_json::json!(true);
        series_data["seasonFolder"] = serde_json::json!(true);
        series_data["addOptions"] = serde_json::json!({
            "searchForMissingEpisodes": false
        });

        // Step 4: Add series to Sonarr
        let add_url = format!("{}/api/v3/series", config.url.trim_end_matches('/'));
        let add_response = self.http_client
            .post(&add_url)
            .header("X-Api-Key", &config.api_key)
            .json(&series_data)
            .send()
            .await?;

        if !add_response.status().is_success() {
            let status = add_response.status();
            let body = add_response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to add series: HTTP {} - {}", status, body);
        }

        let added_series: SonarrSeries = add_response.json().await?;
        tracing::info!("Added series '{}' to Sonarr (ID: {})", added_series.title, added_series.id);
        
        Ok(added_series.id)
    }

    /// Add movie to Radarr by TMDB ID
    /// Returns Radarr movie ID on success
    pub async fn add_movie_by_tmdb(
        &self,
        tmdb_id: i64,
        quality_profile_id: i32,
        root_folder_path: &str,
    ) -> anyhow::Result<i32> {
        let config = self.radarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Radarr not configured"))?;

        if !config.enabled {
            anyhow::bail!("Radarr is disabled");
        }

        // Step 1: Get movie details from Radarr lookup
        let lookup_url = format!(
            "{}/api/v3/movie/lookup/tmdb?tmdbId={}",
            config.url.trim_end_matches('/'),
            tmdb_id
        );

        let lookup_response = self.http_client
            .get(&lookup_url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !lookup_response.status().is_success() {
            anyhow::bail!("Failed to lookup movie: HTTP {}", lookup_response.status());
        }

        let mut movie_data: serde_json::Value = lookup_response.json().await?;

        // Step 2: Modify movie data for adding
        movie_data["qualityProfileId"] = serde_json::json!(quality_profile_id);
        movie_data["rootFolderPath"] = serde_json::json!(root_folder_path);
        movie_data["monitored"] = serde_json::json!(true);
        movie_data["addOptions"] = serde_json::json!({
            "searchForMovie": false
        });

        // Step 3: Add movie to Radarr
        let add_url = format!("{}/api/v3/movie", config.url.trim_end_matches('/'));
        let add_response = self.http_client
            .post(&add_url)
            .header("X-Api-Key", &config.api_key)
            .json(&movie_data)
            .send()
            .await?;

        if !add_response.status().is_success() {
            let status = add_response.status();
            let body = add_response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to add movie: HTTP {} - {}", status, body);
        }

        let added_movie: RadarrMovie = add_response.json().await?;
        tracing::info!("Added movie '{}' to Radarr (ID: {})", added_movie.title, added_movie.id);
        
        Ok(added_movie.id)
    }

    /// Check if series exists in Sonarr by TMDB ID
    /// Returns Sonarr series ID if exists, None otherwise
    pub async fn series_exists(&self, tmdb_id: i64) -> anyhow::Result<Option<i32>> {
        let config = self.sonarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sonarr not configured"))?;

        if !config.enabled {
            return Ok(None);
        }

        // Get all series from Sonarr
        let url = format!("{}/api/v3/series", config.url.trim_end_matches('/'));
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let all_series: Vec<SonarrSeries> = response.json().await?;
        
        // Primary: Match directly by TMDB ID (most reliable, no external API needed)
        for series in &all_series {
            if series.tmdb_id == Some(tmdb_id as i32) {
                tracing::info!("Found series '{}' in Sonarr by tmdb_id={} (ID: {})", series.title, tmdb_id, series.id);
                return Ok(Some(series.id));
            }
        }

        // Fallback: Convert TMDB to TVDB and match (for older series that might not have tmdb_id set)
        match self.tmdb_to_tvdb_via_sonarr(tmdb_id, config).await {
            Ok(tvdb_id) => {
                for series in &all_series {
                    if series.tvdb_id == Some(tvdb_id) {
                        tracing::info!("Found series '{}' in Sonarr by tvdb_id={} (from tmdb_id={}, ID: {})", series.title, tvdb_id, tmdb_id, series.id);
                        return Ok(Some(series.id));
                    }
                }
            }
            Err(e) => {
                tracing::debug!("TMDB→TVDB conversion failed for tmdb_id={}: {} (non-fatal, tmdb_id match already tried)", tmdb_id, e);
            }
        }

        Ok(None)
    }


    /// Check if movie exists in Radarr by TMDB ID
    /// Returns Radarr movie ID if exists, None otherwise
    pub async fn movie_exists(&self, tmdb_id: i64) -> anyhow::Result<Option<i32>> {
        let config = self.radarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Radarr not configured"))?;

        if !config.enabled {
            return Ok(None);
        }

        // Get all movies from Radarr
        let url = format!("{}/api/v3/movie", config.url.trim_end_matches('/'));
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let all_movies: Vec<RadarrMovie> = response.json().await?;
        
        // Find movie with matching TMDB ID
        for movie in all_movies {
            if movie.tmdb_id == tmdb_id as i32 {
                return Ok(Some(movie.id));
            }
        }

        Ok(None)
    }



    /// Get root folders from Sonarr
    pub async fn get_sonarr_root_folders(&self) -> anyhow::Result<Vec<RootFolder>> {
        let config = self.sonarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sonarr not configured"))?;

        let url = format!("{}/api/v3/rootfolder", config.url.trim_end_matches('/'));
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get root folders: HTTP {}", response.status());
        }

        let folders: Vec<RootFolder> = response.json().await?;
        Ok(folders)
    }



    /// Get Radarr root folders
    pub async fn get_radarr_root_folders(&self) -> anyhow::Result<Vec<RootFolder>> {
        let config = self.radarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Radarr not configured"))?;

        let url = format!("{}/api/v3/rootfolder", config.url.trim_end_matches('/'));
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get Radarr root folders: HTTP {}", response.status());
        }

        let folders: Vec<RootFolder> = response.json().await?;
        Ok(folders)
    }


    // ============================================================================
    // Path Lookup Methods (for post-completion file placement)
    // ============================================================================

    /// Get series folder path from Sonarr by series ID
    /// Returns the path Sonarr uses for this series (e.g., "/data/media/tv/How Dare You!! (2026)")
    pub async fn get_series_path(&self, series_id: i64) -> anyhow::Result<String> {
        let config = self.sonarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sonarr not configured"))?;

        let url = format!("{}/api/v3/series/{}", config.url.trim_end_matches('/'), series_id);
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get series {}: HTTP {}", series_id, response.status());
        }

        let series: SonarrSeries = response.json().await?;
        series.path.ok_or_else(|| anyhow::anyhow!("Series {} has no path", series_id))
    }

    /// Get movie folder path from Radarr by movie ID
    /// Returns the path Radarr uses for this movie (e.g., "/data/media/movies/Snow White (2025)")
    pub async fn get_movie_path(&self, movie_id: i64) -> anyhow::Result<String> {
        let config = self.radarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Radarr not configured"))?;

        let url = format!("{}/api/v3/movie/{}", config.url.trim_end_matches('/'), movie_id);
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get movie {}: HTTP {}", movie_id, response.status());
        }

        let movie: RadarrMovie = response.json().await?;
        movie.path.ok_or_else(|| anyhow::anyhow!("Movie {} has no path", movie_id))
    }

    // ============================================================================
    // Command Methods (trigger Sonarr/Radarr actions)
    // ============================================================================

    /// Trigger Sonarr to rescan a series folder for new/changed files
    /// POST /api/v3/command { "name": "RescanSeries", "seriesId": X }
    pub async fn trigger_series_rescan(&self, series_id: i64) -> anyhow::Result<()> {
        let config = self.sonarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sonarr not configured"))?;

        let url = format!("{}/api/v3/command", config.url.trim_end_matches('/'));
        let body = serde_json::json!({
            "name": "RescanSeries",
            "seriesId": series_id
        });

        let response = self.http_client
            .post(&url)
            .header("X-Api-Key", &config.api_key)
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("RescanSeries failed for series {}: HTTP {} - {}", series_id, status, text);
        }

        tracing::info!("Triggered RescanSeries for series ID {}", series_id);
        Ok(())
    }

    /// Trigger Radarr to refresh a movie (rescan folder for new/changed files)
    /// POST /api/v3/command { "name": "RefreshMovie", "movieId": X }
    pub async fn trigger_movie_refresh(&self, movie_id: i64) -> anyhow::Result<()> {
        let config = self.radarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Radarr not configured"))?;

        let url = format!("{}/api/v3/command", config.url.trim_end_matches('/'));
        let body = serde_json::json!({
            "name": "RefreshMovie",
            "movieId": movie_id
        });

        let response = self.http_client
            .post(&url)
            .header("X-Api-Key", &config.api_key)
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("RefreshMovie failed for movie {}: HTTP {} - {}", movie_id, status, text);
        }

        tracing::info!("Triggered RefreshMovie for movie ID {}", movie_id);
        Ok(())
    }

    // ============================================================================
    // Library Methods (for Flasharr library view)
    // ============================================================================

    /// Get all series from Sonarr
    pub async fn get_all_series(&self) -> anyhow::Result<Vec<SonarrSeries>> {
        let config = self.sonarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sonarr not configured"))?;

        let url = format!("{}/api/v3/series", config.url.trim_end_matches('/'));
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get series: HTTP {}", response.status());
        }

        Ok(response.json().await?)
    }

    /// Get all movies from Radarr
    pub async fn get_all_movies(&self) -> anyhow::Result<Vec<RadarrMovie>> {
        let config = self.radarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Radarr not configured"))?;

        let url = format!("{}/api/v3/movie", config.url.trim_end_matches('/'));
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get movies: HTTP {}", response.status());
        }

        Ok(response.json().await?)
    }

    /// Get episodes for a series from Sonarr
    pub async fn get_episodes(&self, series_id: i32) -> anyhow::Result<Vec<SonarrEpisode>> {
        let config = self.sonarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sonarr not configured"))?;

        let url = format!("{}/api/v3/episode?seriesId={}", config.url.trim_end_matches('/'), series_id);
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get episodes: HTTP {}", response.status());
        }

        Ok(response.json().await?)
    }

    /// Get specific episode from Sonarr by checking all episodes for series
    /// Returns the episode if found (matching season/episode numbers)
    pub async fn get_episode_by_details(&self, series_id: i32, season_number: i32, episode_number: i32) -> anyhow::Result<Option<SonarrEpisode>> {
        let episodes = self.get_episodes(series_id).await?;
        
        // Find matching episode
        Ok(episodes.into_iter().find(|e| 
            e.season_number == season_number && 
            e.episode_number == episode_number
        ))
    }

    /// Get movie by TMDB ID from Radarr
    /// Returns full movie object if found
    pub async fn get_movie_by_tmdb(&self, tmdb_id: i64) -> anyhow::Result<Option<RadarrMovie>> {
        let config = self.radarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Radarr not configured"))?;

        if !config.enabled {
            return Ok(None);
        }

        // Get all movies from Radarr
        // optimization: In V3 API we might be able to filter, but standard pattern is fetch all
        // For large libraries this might be heavy, but it's safe.
        // Option B: lookup/tmdb?tmdbId=X returns the movie *metadata*, 
        // but we want the *library status*. `movie/lookup/tmdb` returns "added": "0001-01-01..." if not added.
        // Let's use `movie` endpoint which returns library movies.
        
        let url = format!("{}/api/v3/movie", config.url.trim_end_matches('/'));
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let all_movies: Vec<RadarrMovie> = response.json().await?;
        
        // Find movie with matching TMDB ID
        for movie in all_movies {
            if movie.tmdb_id == tmdb_id as i32 {
                return Ok(Some(movie));
            }
        }

        Ok(None)
    }

    // ============================================================================
    // Calendar & Missing Methods 
    // ============================================================================

    /// Get upcoming episodes from Sonarr calendar
    pub async fn get_calendar(&self, start: &str, end: &str) -> anyhow::Result<Vec<SonarrCalendarEntry>> {
        let config = self.sonarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sonarr not configured"))?;

        let url = format!(
            "{}/api/v3/calendar?start={}&end={}&includeSeries=true",
            config.url.trim_end_matches('/'), start, end
        );
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get calendar: HTTP {}", response.status());
        }

        Ok(response.json().await?)
    }

    /// Get missing episodes from Sonarr (wanted/missing)
    pub async fn get_missing_episodes(&self, page: i32, page_size: i32) -> anyhow::Result<serde_json::Value> {
        let config = self.sonarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sonarr not configured"))?;

        let url = format!(
            "{}/api/v3/wanted/missing?page={}&pageSize={}&sortKey=airDateUtc&sortDirection=descending",
            config.url.trim_end_matches('/'), page, page_size
        );
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get missing episodes: HTTP {}", response.status());
        }

        Ok(response.json().await?)
    }

    /// Get missing movies from Radarr (wanted/missing)
    pub async fn get_missing_movies(&self, page: i32, page_size: i32) -> anyhow::Result<serde_json::Value> {
        let config = self.radarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Radarr not configured"))?;

        let url = format!(
            "{}/api/v3/wanted/missing?page={}&pageSize={}&sortKey=date&sortDirection=descending",
            config.url.trim_end_matches('/'), page, page_size
        );
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get missing movies: HTTP {}", response.status());
        }

        Ok(response.json().await?)
    }

    // ============================================================================
    // System / Health / Storage Methods
    // ============================================================================

    /// Get disk space from Sonarr
    pub async fn get_sonarr_disk_space(&self) -> anyhow::Result<Vec<DiskSpace>> {
        let config = self.sonarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sonarr not configured"))?;

        let url = format!("{}/api/v3/diskspace", config.url.trim_end_matches('/'));
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get disk space: HTTP {}", response.status());
        }

        Ok(response.json().await?)
    }

    /// Get health checks from Sonarr
    pub async fn get_sonarr_health(&self) -> anyhow::Result<Vec<HealthCheck>> {
        let config = self.sonarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sonarr not configured"))?;

        let url = format!("{}/api/v3/health", config.url.trim_end_matches('/'));
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get health: HTTP {}", response.status());
        }

        Ok(response.json().await?)
    }

    /// Get system status from Sonarr
    pub async fn get_sonarr_status(&self) -> anyhow::Result<SystemStatus> {
        let config = self.sonarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sonarr not configured"))?;

        let url = format!("{}/api/v3/system/status", config.url.trim_end_matches('/'));
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get status: HTTP {}", response.status());
        }

        Ok(response.json().await?)
    }

    /// Get system status from Radarr
    pub async fn get_radarr_status(&self) -> anyhow::Result<SystemStatus> {
        let config = self.radarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Radarr not configured"))?;

        let url = format!("{}/api/v3/system/status", config.url.trim_end_matches('/'));
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get status: HTTP {}", response.status());
        }

        Ok(response.json().await?)
    }

    /// Get recent history from Sonarr
    pub async fn get_sonarr_history(&self, page_size: i32) -> anyhow::Result<serde_json::Value> {
        let config = self.sonarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sonarr not configured"))?;

        let url = format!(
            "{}/api/v3/history?page=1&pageSize={}&sortKey=date&sortDirection=descending",
            config.url.trim_end_matches('/'), page_size
        );
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get history: HTTP {}", response.status());
        }

        Ok(response.json().await?)
    }

    /// Get recent history from Radarr
    pub async fn get_radarr_history(&self, page_size: i32) -> anyhow::Result<serde_json::Value> {
        let config = self.radarr_config.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Radarr not configured"))?;

        let url = format!(
            "{}/api/v3/history?page=1&pageSize={}&sortKey=date&sortDirection=descending",
            config.url.trim_end_matches('/'), page_size
        );
        let response = self.http_client
            .get(&url)
            .header("X-Api-Key", &config.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get history: HTTP {}", response.status());
        }

        Ok(response.json().await?)
    }

    /// Check if Sonarr is configured
    pub fn has_sonarr(&self) -> bool {
        self.sonarr_config.is_some()
    }

    /// Check if Radarr is configured
    pub fn has_radarr(&self) -> bool {
        self.radarr_config.is_some()
    }

    // ============================================================================
    // Helper Methods
    // ============================================================================

    /// Convert TMDB ID to TVDB ID using Sonarr's lookup API
    async fn tmdb_to_tvdb_via_sonarr(&self, tmdb_id: i64, _config: &ArrConfig) -> anyhow::Result<i32> {
        // Use TMDB API to get TVDB ID
        use crate::constants::TMDB_API_KEY;
        
        let url = format!(
            "https://api.themoviedb.org/3/tv/{}?api_key={}",
            tmdb_id, TMDB_API_KEY
        );

        let response = self.http_client.get(&url).send().await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to lookup TMDB ID: HTTP {}", response.status());
        }

        let _data: serde_json::Value = response.json().await?;
        
        // Extract external_ids
        let external_ids_url = format!(
            "https://api.themoviedb.org/3/tv/{}/external_ids?api_key={}",
            tmdb_id, TMDB_API_KEY
        );

        let ext_response = self.http_client.get(&external_ids_url).send().await?;
        
        if !ext_response.status().is_success() {
            anyhow::bail!("Failed to get external IDs: HTTP {}", ext_response.status());
        }

        let ext_data: serde_json::Value = ext_response.json().await?;
        
        let tvdb_id = ext_data["tvdb_id"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("No TVDB ID found for TMDB ID {}", tmdb_id))?;

        Ok(tvdb_id as i32)
    }

}


