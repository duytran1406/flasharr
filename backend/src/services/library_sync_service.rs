//! Library Sync Service
//!
//! Synchronizes Sonarr and Radarr libraries into the local Flasharr database.
//! This ensures that the Flasharr UI correctly reflects the state of the Arr suite
//! even for items added or downloaded outside of Flasharr.

use std::sync::Arc;
use tokio::time::{Duration, interval};
use tracing::{info, error, warn};
use crate::arr::ArrClient;
use crate::db::Db;
use crate::db::media::{MediaItem, MediaEpisode};

pub struct LibrarySyncService {
    db: Arc<Db>,
    arr_client: Arc<ArrClient>,
}

impl LibrarySyncService {
    pub fn new(db: Arc<Db>, arr_client: Arc<ArrClient>) -> Self {
        Self { db, arr_client }
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

}
