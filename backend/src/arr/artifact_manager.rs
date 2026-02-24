//! Arr Artifact Manager
//!
//! Manages Series/Movie artifacts in Sonarr/Radarr.
//! Handles creation, updates, and status synchronization.
//! Decoupled from individual file downloads.

use std::sync::Arc;
use crate::arr::ArrClient;
use crate::db::Db;
use crate::downloader::{DownloadTask, MediaType};

/// Result of artifact management operation
#[derive(Debug, Clone)]
pub enum ArtifactStatus {
    /// New artifact created in Sonarr/Radarr
    Created { arr_id: i32 },
    /// Existing artifact found, already monitored
    AlreadyMonitored { arr_id: i32 },
    /// Skipped (no TMDB ID or not applicable)
    Skipped { reason: String },
    /// Failed to manage artifact
    Failed { error: String },
}



/// Manages Series/Movie artifacts in Sonarr/Radarr
pub struct ArrArtifactManager {
    arr_client: Arc<ArrClient>,
    db: Arc<Db>,
}

impl ArrArtifactManager {
    /// Create new artifact manager
    pub fn new(arr_client: Arc<ArrClient>, db: Arc<Db>) -> Self {
        Self { arr_client, db }
    }

    /// Manage artifact (series/movie) in Sonarr/Radarr
    /// Called when download is ADDED (not completed)
    /// 
    /// Strategy:
    /// 1. Detect media type
    /// 2. Query library status
    /// 3. Create new, update existing, or skip
    /// 4. Store arr_id in database
    pub async fn manage_artifact(&self, task: &DownloadTask) -> anyhow::Result<ArtifactStatus> {
        // Require TMDB ID for artifact management
        let tmdb_id = match task.tmdb_id {
            Some(id) => id,
            None => {
                return Ok(ArtifactStatus::Skipped {
                    reason: "No TMDB ID available".to_string(),
                });
            }
        };

        // Detect media type
        let media_type = task.detect_media_type();
        tracing::info!(
            "Managing artifact for {} (TMDB: {}, Type: {:?})",
            task.filename,
            tmdb_id,
            media_type
        );

        // Route to appropriate handler
        match media_type {
            MediaType::TvSeries | MediaType::TvEpisode => {
                self.manage_tv_series(task, tmdb_id).await
            }
            MediaType::Movie => {
                self.manage_movie(task, tmdb_id).await
            }
        }
    }

    /// Manage TV series artifact in Sonarr
    async fn manage_tv_series(&self, _task: &DownloadTask, tmdb_id: i64) -> anyhow::Result<ArtifactStatus> {
        // Check if series already exists
        match self.arr_client.series_exists(tmdb_id).await {
            Ok(Some(series_id)) => {
                tracing::info!(
                    "Series already exists in Sonarr (ID: {}), updating database",
                    series_id
                );

                // Update ALL downloads with this TMDB ID
                if let Err(e) = self.db.update_arr_series_id_by_tmdb(tmdb_id, series_id as i64) {
                    tracing::warn!("Failed to update arr_series_id for TMDB {}: {}", tmdb_id, e);
                }

                Ok(ArtifactStatus::AlreadyMonitored { arr_id: series_id })
            }
            Ok(None) => {
                // Series doesn't exist, create it
                tracing::info!("Creating new series in Sonarr (TMDB: {})", tmdb_id);

                // Get configuration
                let quality_profile_id = self.db.get_setting("sonarr_quality_profile_id")
                    .ok()
                    .flatten()
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(1);

                // Fetch root folders from Sonarr
                let root_folder = match self.arr_client.get_sonarr_root_folders().await {
                    Ok(folders) if !folders.is_empty() => {
                        tracing::info!("Using Sonarr root folder: {}", folders[0].path);
                        folders[0].path.clone()
                    }
                    Ok(_) => {
                        tracing::warn!("No root folders found in Sonarr, using default /tv");
                        "/tv".to_string()
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch Sonarr root folders: {}, using default /tv", e);
                        "/tv".to_string()
                    }
                };

                // Add series to Sonarr
                match self.arr_client.add_series_by_tmdb(tmdb_id, quality_profile_id, &root_folder).await {
                    Ok(series_id) => {
                        tracing::info!("Successfully created series in Sonarr (ID: {})", series_id);

                        // Update database
                        if let Err(e) = self.db.update_arr_series_id_by_tmdb(tmdb_id, series_id as i64) {
                            tracing::warn!("Failed to update arr_series_id for TMDB {}: {}", tmdb_id, e);
                        }

                        Ok(ArtifactStatus::Created { arr_id: series_id })
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to create series in Sonarr: {}", e);
                        tracing::error!("{}", error_msg);
                        Ok(ArtifactStatus::Failed { error: error_msg })
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("Failed to check series existence: {}", e);
                tracing::error!("{}", error_msg);
                Ok(ArtifactStatus::Failed { error: error_msg })
            }
        }
    }

    /// Manage movie artifact in Radarr
    async fn manage_movie(&self, task: &DownloadTask, tmdb_id: i64) -> anyhow::Result<ArtifactStatus> {
        // Check if movie already exists
        match self.arr_client.movie_exists(tmdb_id).await {
            Ok(Some(movie_id)) => {
                tracing::info!(
                    "Movie already exists in Radarr (ID: {}), updating database",
                    movie_id
                );

                // Update database - both per-task and by TMDB
                if let Err(e) = self.db.update_download_arr_status(
                    &task.id.to_string(),
                    true,
                    None,
                    Some(movie_id),
                ) {
                    tracing::warn!("Failed to update arr_movie_id for task {}: {}", task.id, e);
                }
                if let Err(e) = self.db.update_arr_movie_id_by_tmdb(tmdb_id, movie_id as i64) {
                    tracing::warn!("Failed to update arr_movie_id for TMDB {}: {}", tmdb_id, e);
                }

                Ok(ArtifactStatus::AlreadyMonitored { arr_id: movie_id })
            }
            Ok(None) => {
                // Movie doesn't exist, create it
                tracing::info!("Creating new movie in Radarr (TMDB: {})", tmdb_id);

                // Get configuration
                let quality_profile_id = self.db.get_setting("radarr_quality_profile_id")
                    .ok()
                    .flatten()
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(1);

                // Fetch root folder from Radarr API (more reliable than DB setting)
                let root_folder = match self.arr_client.get_radarr_root_folders().await {
                    Ok(folders) if !folders.is_empty() => {
                        tracing::info!("Using Radarr root folder: {}", folders[0].path);
                        folders[0].path.clone()
                    }
                    Ok(_) => {
                        tracing::warn!("No root folders found in Radarr, using default /movies");
                        "/movies".to_string()
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch Radarr root folders: {}, using default /movies", e);
                        "/movies".to_string()
                    }
                };

                // Add movie to Radarr
                match self.arr_client.add_movie_by_tmdb(tmdb_id, quality_profile_id, &root_folder).await {
                    Ok(movie_id) => {
                        tracing::info!("Successfully created movie in Radarr (ID: {})", movie_id);

                        // Update database
                        if let Err(e) = self.db.update_download_arr_status(
                            &task.id.to_string(),
                            true,
                            None,
                            Some(movie_id),
                        ) {
                            tracing::warn!("Failed to update database arr status: {}", e);
                        }

                        Ok(ArtifactStatus::Created { arr_id: movie_id })
                    }
                    Err(e) => {
                        // If add failed (e.g., 400 = already exists), try to find existing movie
                        let error_str = format!("{}", e);
                        if error_str.contains("400") {
                            tracing::warn!(
                                "Radarr returned 400 when adding movie (TMDB: {}), likely already exists. Re-querying...",
                                tmdb_id
                            );
                            // Re-query — the movie may have been added concurrently
                            if let Ok(Some(movie_id)) = self.arr_client.movie_exists(tmdb_id).await {
                                tracing::info!(
                                    "Found existing movie in Radarr after 400 (ID: {}), updating database",
                                    movie_id
                                );
                                if let Err(e) = self.db.update_download_arr_status(
                                    &task.id.to_string(),
                                    true,
                                    None,
                                    Some(movie_id),
                                ) {
                                    tracing::warn!("Failed to update arr_movie_id: {}", e);
                                }
                                if let Err(e) = self.db.update_arr_movie_id_by_tmdb(tmdb_id, movie_id as i64) {
                                    tracing::warn!("Failed to update arr_movie_id by TMDB: {}", e);
                                }
                                return Ok(ArtifactStatus::AlreadyMonitored { arr_id: movie_id });
                            }
                        }
                        
                        let error_msg = format!("Failed to create movie in Radarr: {}", e);
                        tracing::error!("{}", error_msg);
                        Ok(ArtifactStatus::Failed { error: error_msg })
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("Failed to check movie existence: {}", e);
                tracing::error!("{}", error_msg);
                Ok(ArtifactStatus::Failed { error: error_msg })
            }
        }
    }
}
