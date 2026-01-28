//! *arr API Client
//!
//! HTTP client for communicating with Sonarr and Radarr APIs.
//! Triggers automatic imports when downloads complete.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use crate::config::ArrConfig;
use once_cell::sync::Lazy;
use regex::Regex;

/// Media type detection patterns (reused from indexer)
static TV_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(s\d{1,2}e\d{1,2}|\d{1,2}x\d{1,2}|season[\s.]+\d+|episode[\s.]+\d+|tập\s+\d+|tap\s+\d+)").unwrap()
});

static ANIME_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(\[\d{1,3}\]|ep\d{1,3}|batch|anime|アニメ)").unwrap()
});

#[derive(Debug, Clone, PartialEq)]
pub enum MediaType {
    TvShow,
    Movie,
    #[allow(dead_code)]
    Unknown,
}

/// *arr API client for triggering imports
pub struct ArrClient {
    http_client: Client,
    sonarr_config: Option<ArrConfig>,
    radarr_config: Option<ArrConfig>,
}

#[derive(Serialize)]
struct CommandRequest {
    name: String,
    path: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct CommandResponse {
    #[allow(dead_code)]
    id: i32,
    name: String,
    status: String,
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

    /// Notify *arr applications when a download completes
    pub async fn notify_completion(&self, filename: &str, file_path: &Path) {
        let media_type = Self::detect_media_type(filename);
        
        match media_type {
            MediaType::TvShow => {
                if let Some(ref config) = self.sonarr_config {
                    if config.enabled && config.auto_import {
                        if let Err(e) = self.trigger_sonarr_import(file_path, config).await {
                            tracing::error!("Failed to trigger Sonarr import: {}", e);
                        } else {
                            tracing::info!("Triggered Sonarr import for: {}", filename);
                        }
                    }
                }
            }
            MediaType::Movie => {
                if let Some(ref config) = self.radarr_config {
                    if config.enabled && config.auto_import {
                        if let Err(e) = self.trigger_radarr_import(file_path, config).await {
                            tracing::error!("Failed to trigger Radarr import: {}", e);
                        } else {
                            tracing::info!("Triggered Radarr import for: {}", filename);
                        }
                    }
                }
            }
            MediaType::Unknown => {
                tracing::debug!("Unknown media type for: {}, skipping *arr sync", filename);
            }
        }
    }

    /// Trigger Sonarr to scan for new episodes
    async fn trigger_sonarr_import(&self, file_path: &Path, config: &ArrConfig) -> anyhow::Result<()> {
        let url = format!("{}/api/v3/command", config.url.trim_end_matches('/'));
        
        let command = CommandRequest {
            name: "DownloadedEpisodesScan".to_string(),
            path: file_path.parent()
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string(),
        };

        let response = self.http_client
            .post(&url)
            .header("X-Api-Key", &config.api_key)
            .json(&command)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Sonarr API error {}: {}", status, body);
        }

        let result: CommandResponse = response.json().await?;
        tracing::debug!("Sonarr command queued: {} (status: {})", result.name, result.status);
        
        Ok(())
    }

    /// Trigger Radarr to scan for new movies
    async fn trigger_radarr_import(&self, file_path: &Path, config: &ArrConfig) -> anyhow::Result<()> {
        let url = format!("{}/api/v3/command", config.url.trim_end_matches('/'));
        
        let command = CommandRequest {
            name: "DownloadedMoviesScan".to_string(),
            path: file_path.parent()
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string(),
        };

        let response = self.http_client
            .post(&url)
            .header("X-Api-Key", &config.api_key)
            .json(&command)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Radarr API error {}: {}", status, body);
        }

        let result: CommandResponse = response.json().await?;
        tracing::debug!("Radarr command queued: {} (status: {})", result.name, result.status);
        
        Ok(())
    }

    /// Test connection to Sonarr
    pub async fn test_sonarr_connection(url: &str, api_key: &str) -> anyhow::Result<String> {
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
        Ok(format!("Connected to Sonarr v{}", status.version))
    }

    /// Test connection to Radarr
    pub async fn test_radarr_connection(url: &str, api_key: &str) -> anyhow::Result<String> {
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
        Ok(format!("Connected to Radarr v{}", status.version))
    }

    /// Detect media type from filename
    fn detect_media_type(filename: &str) -> MediaType {
        let filename_lower = filename.to_lowercase();
        
        // Check for anime patterns first (higher priority)
        if ANIME_PATTERN.is_match(&filename_lower) {
            return MediaType::TvShow;
        }
        
        // Check for TV show patterns
        if TV_PATTERN.is_match(&filename_lower) {
            return MediaType::TvShow;
        }
        
        // Default to movie
        MediaType::Movie
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_tv_show_standard() {
        assert_eq!(ArrClient::detect_media_type("Breaking.Bad.S01E01.mkv"), MediaType::TvShow);
        assert_eq!(ArrClient::detect_media_type("game.of.thrones.s05e10.1080p.mkv"), MediaType::TvShow);
    }

    #[test]
    fn test_detect_tv_show_alternative() {
        assert_eq!(ArrClient::detect_media_type("The.Office.1x01.mkv"), MediaType::TvShow);
        assert_eq!(ArrClient::detect_media_type("Friends.2x05.720p.mkv"), MediaType::TvShow);
    }

    #[test]
    fn test_detect_tv_show_word_based() {
        assert_eq!(ArrClient::detect_media_type("Show.Season.1.Episode.5.mkv"), MediaType::TvShow);
        assert_eq!(ArrClient::detect_media_type("Series.season.2.episode.3.mkv"), MediaType::TvShow);
    }

    #[test]
    fn test_detect_anime() {
        assert_eq!(ArrClient::detect_media_type("Naruto.[01].mkv"), MediaType::TvShow);
        assert_eq!(ArrClient::detect_media_type("One.Piece.EP001.mkv"), MediaType::TvShow);
        assert_eq!(ArrClient::detect_media_type("Attack.on.Titan.Batch.mkv"), MediaType::TvShow);
    }

    #[test]
    fn test_detect_movie() {
        assert_eq!(ArrClient::detect_media_type("Avengers.Endgame.2019.1080p.mkv"), MediaType::Movie);
        assert_eq!(ArrClient::detect_media_type("The.Matrix.1999.2160p.mkv"), MediaType::Movie);
        assert_eq!(ArrClient::detect_media_type("Inception.2010.BluRay.mkv"), MediaType::Movie);
    }

    #[test]
    fn test_detect_edge_cases() {
        // Files with "season" in title but not TV pattern
        assert_eq!(ArrClient::detect_media_type("The.Seasonal.Movie.2020.mkv"), MediaType::Movie);
        
        // Mixed case
        assert_eq!(ArrClient::detect_media_type("SHOW.S01E01.MKV"), MediaType::TvShow);
    }
}
