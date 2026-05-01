//! Library Sync Service
//!
//! Synchronizes Sonarr and Radarr libraries into the local Flasharr database.
//! This ensures that the Flasharr UI correctly reflects the state of the Arr suite
//! even for items added or downloaded outside of Flasharr.

use std::sync::Arc;
use std::path::PathBuf;
use tokio::time::{Duration, interval};
use tracing::{info, error, warn};
use crate::arr::ArrClient;
use crate::db::Db;
use crate::db::media::{MediaItem, MediaEpisode};
use std::path::Path;
use crate::downloader::{MediaType, task::DownloadState};

pub struct LibrarySyncService {
    db: Arc<Db>,
    arr_client: Arc<ArrClient>,
    staging_dir: PathBuf,
}

impl LibrarySyncService {
    pub fn new(db: Arc<Db>, arr_client: Arc<ArrClient>, staging_dir: PathBuf) -> Self {
        Self { db, arr_client, staging_dir }
    }

    /// Run the sync loop periodically
    pub async fn start_background_sync(self: Arc<Self>, interval_hours: u64) {
        let mut interval = interval(Duration::from_secs(interval_hours * 3600));
        
        info!("Starting background library sync (every {} hours)", interval_hours);
        
        loop {
            interval.tick().await;
            info!("Starting scheduled library sync from Sonarr/Radarr");
            
            if let Err(e) = self.sync_all().await {
                error!("Library sync failed: {}", e);
            } else {
                info!("Library sync completed metadata update");
                if let Err(e) = self.reconcile_downloads().await {
                    error!("Library normalization failed: {}", e);
                } else {
                    info!("Library normalization completed");
                }
            }
        }
    }

    /// Full synchronization of all series and movies
    pub async fn sync_all(&self) -> anyhow::Result<()> {
        let mut total_updated = 0;

        // 1. Sync Sonarr Series & Episodes
        if self.arr_client.has_sonarr() {
            info!("[SYNC] Fetching all series from Sonarr");
            match self.arr_client.get_all_series().await {
                Ok(series_list) => {
                    for sonarr_series in series_list {
                        let tmdb_id = match sonarr_series.tmdb_id {
                            Some(id) => id as i64,
                            None => {
                                // Try to find by TVDB if TMDB is missing in Sonarr record
                                warn!("[SYNC] Series '{}' missing tmdb_id in Sonarr. Skipping.", sonarr_series.title);
                                continue;
                            }
                        };

                        // Sync MediaItem
                        let mut item = MediaItem::new(tmdb_id, "tv", &sonarr_series.title);
                        item.year = sonarr_series.year;
                        item.overview = sonarr_series.overview.clone();
                        item.tvdb_id = sonarr_series.tvdb_id;
                        item.arr_id = Some(sonarr_series.id);
                        item.arr_type = Some("sonarr".to_string());
                        item.arr_path = sonarr_series.path.clone();
                        item.arr_monitored = sonarr_series.monitored.unwrap_or(false);
                        item.arr_status = sonarr_series.status.clone();
                        item.arr_quality_profile_id = sonarr_series.quality_profile_id;
                        item.arr_synced_at = Some(chrono::Utc::now().to_rfc3339());
                        
                        if let Some(stats) = sonarr_series.statistics {
                            // Use episode counts when available — more reliable than the
                            // percent_of_episodes float (99.4% rounds down, 99.0 threshold missed).
                            item.arr_has_file = match (stats.episode_file_count, stats.episode_count) {
                                (Some(files), Some(total)) if total > 0 => files >= total,
                                _ => stats.percent_of_episodes.unwrap_or(0.0) >= 100.0,
                            };
                            item.arr_size_on_disk = stats.size_on_disk.unwrap_or(0);
                            item.total_seasons = stats.season_count;
                        }

                        // Get poster from images if possible
                        if let Some(images) = sonarr_series.images {
                            for img in images {
                                if img.cover_type == "poster" {
                                    item.poster_path = img.remote_url.or(img.url);
                                    break;
                                }
                            }
                        }

                        if let Err(e) = self.db.upsert_media_item(&item) {
                            error!("[SYNC] Failed to upsert series {}: {}", item.title, e);
                            continue;
                        }

                        // Sync Episodes
                        if let Err(e) = self.sync_series_episodes(tmdb_id, sonarr_series.id).await {
                            warn!("[SYNC] Failed to sync episodes for series {}: {}", item.title, e);
                        }

                        total_updated += 1;
                    }
                }
                Err(e) => error!("[SYNC] Failed to fetch Sonarr series: {}", e),
            }
        }

        // 2. Sync Radarr Movies
        if self.arr_client.has_radarr() {
            info!("[SYNC] Fetching all movies from Radarr");
            match self.arr_client.get_all_movies().await {
                Ok(movie_list) => {
                    for radarr_movie in movie_list {
                        // Handle cases where Radarr API returns null for tmdb_id
                        let tmdb_id = radarr_movie.tmdb_id.map(|id| id as i64).unwrap_or(0);

                        // Skip movies without TMDB ID as they can't be properly tracked
                        if tmdb_id == 0 {
                            tracing::warn!("Radarr movie '{}' has no TMDB ID, skipping sync", radarr_movie.title);
                            continue;
                        }

                        let mut item = MediaItem::new(tmdb_id, "movie", &radarr_movie.title);
                        item.year = radarr_movie.year;
                        item.overview = radarr_movie.overview.clone();
                        item.arr_id = Some(radarr_movie.id);
                        item.arr_type = Some("radarr".to_string());
                        item.arr_path = radarr_movie.path.clone();
                        item.arr_monitored = radarr_movie.monitored.unwrap_or(false);
                        item.arr_status = radarr_movie.status.clone();
                        item.arr_quality_profile_id = radarr_movie.quality_profile_id;
                        item.arr_has_file = radarr_movie.has_file.unwrap_or(false);
                        item.arr_synced_at = Some(chrono::Utc::now().to_rfc3339());
                        item.arr_size_on_disk = radarr_movie.size_on_disk.unwrap_or(0);
                        item.runtime = radarr_movie.runtime;

                        // Get poster
                        if let Some(images) = radarr_movie.images {
                            for img in images {
                                if img.cover_type == "poster" {
                                    item.poster_path = img.remote_url.or(img.url);
                                    break;
                                }
                            }
                        }

                        if let Err(e) = self.db.upsert_media_item(&item) {
                            error!("[SYNC] Failed to upsert movie {}: {}", item.title, e);
                            continue;
                        }
                        total_updated += 1;
                    }
                }
                Err(e) => error!("[SYNC] Failed to fetch Radarr movies: {}", e),
            }
        }

        info!("[SYNC] Completed. Processed {} items.", total_updated);
        Ok(())
    }

    /// Sync episodes for a specific series
    async fn sync_series_episodes(&self, tmdb_id: i64, sonarr_id: i32) -> anyhow::Result<()> {
        let episodes = self.arr_client.get_episodes(sonarr_id).await?;
        
        for sonarr_ep in episodes {
            let mut ep = MediaEpisode::new(tmdb_id, sonarr_ep.season_number, sonarr_ep.episode_number);
            ep.title = sonarr_ep.title;
            ep.overview = sonarr_ep.overview;
            ep.air_date = sonarr_ep.air_date_utc;
            ep.arr_episode_id = Some(sonarr_ep.id);
            ep.arr_has_file = sonarr_ep.has_file;
            ep.arr_monitored = sonarr_ep.monitored;

            if let Err(e) = self.db.upsert_media_episode(&ep) {
                error!("[SYNC] Failed to upsert episode S{:02}E{:02} for TMDB {}: {}", 
                    ep.season_number, ep.episode_number, tmdb_id, e);
            }
        }
        
        Ok(())
    }

    /// Reconciles the downloads table with the filesystem and Arr paths.
    /// Performs bi-directional check and moves files to their permanent homes.
    pub async fn reconcile_downloads(&self) -> anyhow::Result<()> {
        info!("[RECONCILE] Starting bi-directional library reconciliation");

        // Resolve media roots from arr once before iterating (avoids repeated API calls)
        let tv_root = if self.arr_client.has_sonarr() {
            self.arr_client.get_sonarr_root_folders().await
                .ok()
                .and_then(|f| f.first().map(|r| PathBuf::from(&r.path)))
                .unwrap_or_else(|| self.staging_dir.parent().unwrap_or(&self.staging_dir).join("media/tv"))
        } else {
            self.staging_dir.parent().unwrap_or(&self.staging_dir).join("media/tv")
        };
        let movie_root = if self.arr_client.has_radarr() {
            self.arr_client.get_radarr_root_folders().await
                .ok()
                .and_then(|f| f.first().map(|r| PathBuf::from(&r.path)))
                .unwrap_or_else(|| self.staging_dir.parent().unwrap_or(&self.staging_dir).join("media/movies"))
        } else {
            self.staging_dir.parent().unwrap_or(&self.staging_dir).join("media/movies")
        };

        // 1. Get all completed downloads
        let downloads = self.db.get_all_tasks().map_err(|e| anyhow::anyhow!("DB error: {}", e))?;
        let completed: Vec<_> = downloads.into_iter().filter(|d| d.state == DownloadState::Completed).collect();

        info!("[RECONCILE] Auditing {} completed downloads", completed.len());

        for mut download in completed {
            let mut changed = false;

            // 1. Normalize filename (strip leading slash)
            if download.filename.starts_with('/') {
                info!("[RECONCILE] Normalizing filename for {}: {} -> {}",
                    download.id, download.filename, download.filename.trim_start_matches('/'));
                download.filename = download.filename.trim_start_matches('/').to_string();
                changed = true;
            }

            // 2. Fix corrupted destination paths (e.g. movies with "Season X" or missing subfolders)
            // Re-detect media type to ensure we have the truth
            let media_type = download.detect_media_type();
            let expected_type_str = match media_type {
                MediaType::Movie => "movie",
                _ => "tv"
            };

            // If category is "movie" but path has "Season", or vice versa, it's corrupted
            let has_season_folder = download.destination.contains("/Season ") || download.destination.contains("/season ");
            let is_movie = expected_type_str == "movie";

            if (is_movie && has_season_folder) || (!is_movie && !has_season_folder && download.tmdb_season.is_some()) {
                warn!("[RECONCILE] Detected corrupted path for {} {}: {}",
                    expected_type_str, download.id, download.destination);

                let tmdb_meta = Some(crate::downloader::TmdbDownloadMetadata {
                    tmdb_id: download.tmdb_id,
                    media_type: Some(expected_type_str.to_string()),
                    title: download.tmdb_title.clone(),
                    year: None,
                    collection_name: None,
                    season: download.tmdb_season.map(|s| s as i32),
                    episode: download.tmdb_episode.map(|e| e as i32),
                });

                let root_dir = if is_movie { movie_root.clone() } else { tv_root.clone() };
                let new_dest = crate::downloader::PathBuilder::build_destination_path(
                    &download.filename,
                    expected_type_str,
                    &tmdb_meta,
                    &root_dir
                );

                if new_dest != download.destination {
                    info!("[RECONCILE] Reconstructed path for {}: {} -> {}", download.id, download.destination, new_dest);
                    download.destination = new_dest;
                    changed = true;
                }
            }

            // Save changes to DB if any
            if changed {
                if let Err(e) = self.db.save_task(&download) {
                    error!("[RECONCILE] Failed to update task {} in DB: {}", download.id, e);
                }
            }

            let destination = Path::new(&download.destination);

            // 3. Physical file reconciliation
            if destination.exists() {
                if let Ok(metadata) = tokio::fs::symlink_metadata(destination).await {
                    if metadata.file_type().is_symlink() {
                        info!("[RECONCILE] Found symlink at {:?}. Converting to real file.", destination);
                        if let Ok(source) = tokio::fs::read_link(destination).await {
                            if source.exists() {
                                let _ = tokio::fs::remove_file(destination).await;
                                if let Err(e) = tokio::fs::rename(&source, destination).await {
                                    warn!("[RECONCILE] Failed to convert symlink to move: {}. Falling back to copy.", e);
                                    if tokio::fs::copy(&source, destination).await.is_ok() {
                                        let _ = tokio::fs::remove_file(source).await;
                                    }
                                }
                                info!("[RECONCILE] Successfully converted symlink to REAL file for {}", download.id);
                            }
                        }
                    }
                }
                continue;
            }

            // If NOT at destination, check if it's still in the download staging folder
            let staging_path = self.staging_dir.join(download.filename.trim_start_matches('/'));
            if staging_path.exists() {
                info!("[RECONCILE] Found orphaned file at {:?}. Moving to library path: {:?}", staging_path, destination);

                if let Some(parent) = destination.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }

                if let Err(e) = tokio::fs::rename(&staging_path, destination).await {
                    warn!("[RECONCILE] Move failed: {}. Trying copy+remove.", e);
                    if tokio::fs::copy(&staging_path, destination).await.is_ok() {
                        let _ = tokio::fs::remove_file(&staging_path).await;
                        info!("[RECONCILE] Successfully moved orphaned file (via copy) for {}", download.id);
                    } else {
                        error!("[RECONCILE] Failed to recover orphaned file for {}: {:?}", download.id, e);
                    }
                } else {
                    info!("[RECONCILE] Successfully moved orphaned file to library for {}", download.id);
                }
            } else {
                warn!("[RECONCILE] File missing at path {:?}. Manual repair might be needed for {}", staging_path, download.id);
            }
        }

        Ok(())
    }
}
