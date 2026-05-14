//! Discovery Service
//!
//! Orchestrates media acquisition: Search -> Decision -> Library -> Download.
//! This service centralizes the "Smart Grab" logic previously split between frontend and backend.

use crate::db::Db;
use crate::downloader::{DownloadOrchestrator, TmdbDownloadMetadata};
use crate::services::TmdbService;
use crate::utils::smart_tokenizer::smart_parse;
use crate::utils::title_matcher::{calculate_unified_similarity, is_different_franchise_entry};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

pub struct DiscoveryService {
    db: Arc<Db>,
    orchestrator: Arc<DownloadOrchestrator>,
    tmdb_service: Arc<TmdbService>,
    http_client: Arc<reqwest::Client>,
}

impl DiscoveryService {
    pub fn new(
        db: Arc<Db>,
        orchestrator: Arc<DownloadOrchestrator>,
        tmdb_service: Arc<TmdbService>,
        http_client: Arc<reqwest::Client>,
    ) -> Self {
        Self {
            db,
            orchestrator,
            tmdb_service,
            http_client,
        }
    }

    /// Atomic Smart Grab
    /// 1. Adds series/movie to Sonarr/Radarr (if not already present).
    /// 2. Performs a smart search across indexers.
    /// 3. Selects the best candidate based on quality/score.
    /// 4. Initiates download.
    pub async fn smart_grab(
        &self,
        tmdb_id: i64,
        media_type: &str,
        title: &str,
        year: Option<String>,
    ) -> Result<Value, String> {
        info!(
            "Starting Atomic Smart Grab for {} (TMDB: {})",
            title, tmdb_id
        );

        // 1. Ensure in Library (Arr Suite) — best-effort, non-fatal
        let arr_id = if let Some(arr_client) = self.orchestrator.get_arr_client().await {
            let result: anyhow::Result<i32> = if media_type == "tv" {
                let quality_profile_id = self
                    .db
                    .get_setting("sonarr_quality_profile_id")
                    .ok()
                    .flatten()
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(1);
                let root_folder = arr_client
                    .get_sonarr_root_folders()
                    .await
                    .ok()
                    .and_then(|folders| folders.into_iter().next())
                    .map(|folder| folder.path)
                    .unwrap_or_else(|| "/data/media/tv".to_string());

                arr_client
                    .add_series_by_tmdb(tmdb_id, quality_profile_id, &root_folder)
                    .await
            } else {
                let quality_profile_id = self
                    .db
                    .get_setting("radarr_quality_profile_id")
                    .ok()
                    .flatten()
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(1);
                let root_folder = arr_client
                    .get_radarr_root_folders()
                    .await
                    .ok()
                    .and_then(|folders| folders.into_iter().next())
                    .map(|folder| folder.path)
                    .unwrap_or_else(|| "/data/media/movies".to_string());

                arr_client
                    .add_movie_by_tmdb(tmdb_id, quality_profile_id, &root_folder)
                    .await
            };
            match result {
                Ok(id) => Some(id),
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("exists") || msg.contains("already") {
                        info!("Item already in arr library");
                    } else {
                        warn!("Failed to add item to arr: {}", msg);
                    }
                    None
                }
            }
        } else {
            warn!("Arr client not configured, skipping library add");
            None
        };

        // 2. Perform Smart Search
        let search_results = self
            .perform_internal_search(title, year.clone(), media_type, Some(tmdb_id))
            .await?;

        if search_results.is_empty() {
            return Err("No results found for smart grab".to_string());
        }

        if media_type == "tv" {
            return self
                .queue_tv_smart_grab(tmdb_id, title, year, arr_id, search_results)
                .await;
        }

        // 3. Decision Logic (Quick Grab - Highest Score)
        let best_candidate = search_results.first().ok_or("No valid candidates found")?;
        match self
            .orchestrator
            .add_download_with_metadata(
                best_candidate.url.clone(),
                Some(best_candidate.original_name.clone()),
                "fshare".to_string(),
                "media".to_string(),
                Some(TmdbDownloadMetadata {
                    tmdb_id: Some(tmdb_id),
                    media_type: Some("movie".to_string()),
                    title: Some(title.to_string()),
                    year: year.as_deref().and_then(|y| y.parse::<i32>().ok()),
                    collection_name: None,
                    season: None,
                    episode: None,
                }),
                None,
                None,
            )
            .await
        {
            Ok(_task) => {
                info!("Smart Grab successful for {}", title);
                Ok(json!({
                    "success": true,
                    "message": format!("Grab queued: {}", best_candidate.original_name),
                    "grabbed": {
                        "name": best_candidate.original_name,
                        "score": best_candidate.score,
                        "size": best_candidate.size,
                    },
                    "arr_id": arr_id
                }))
            }
            Err(e) => Err(format!("Failed to initiate download: {}", e)),
        }
    }

    async fn queue_tv_smart_grab(
        &self,
        tmdb_id: i64,
        title: &str,
        year: Option<String>,
        arr_id: Option<i32>,
        search_results: Vec<InternalSearchResult>,
    ) -> Result<Value, String> {
        let mut best_by_episode: HashMap<(u32, u32), (InternalSearchResult, u32, u32)> =
            HashMap::new();
        let valid_episodes = self.get_valid_tv_episodes(tmdb_id).await;

        for result in search_results {
            let parsed = smart_parse(&result.original_name);
            let Some(episode) = parsed.episode else {
                continue;
            };
            let season = parsed.season.unwrap_or(1);
            if !valid_episodes.is_empty() && !valid_episodes.contains(&(season, episode)) {
                continue;
            }
            let key = (season, episode);

            let replace = best_by_episode
                .get(&key)
                .map(|(existing, _, _)| result.score > existing.score)
                .unwrap_or(true);
            if replace {
                best_by_episode.insert(key, (result, season, episode));
            }
        }

        if best_by_episode.is_empty() {
            return Err("No episode results found for TV smart grab".to_string());
        }

        let mut selected: Vec<_> = best_by_episode.into_values().collect();
        selected.sort_by_key(|(_, season, episode)| (*season, *episode));

        let batch_id = Uuid::new_v4().to_string();
        let batch_name = format!(
            "{}{}",
            title,
            year.as_deref()
                .map_or(String::new(), |y| format!(" ({})", y))
        );
        let parsed_year = year.as_deref().and_then(|y| y.parse::<i32>().ok());
        let mut grabbed = Vec::new();

        for (candidate, season, episode) in selected {
            let task = self
                .orchestrator
                .add_download_with_metadata(
                    candidate.url.clone(),
                    Some(candidate.original_name.clone()),
                    "fshare".to_string(),
                    "tv".to_string(),
                    Some(TmdbDownloadMetadata {
                        tmdb_id: Some(tmdb_id),
                        media_type: Some("tv".to_string()),
                        title: Some(title.to_string()),
                        year: parsed_year,
                        collection_name: None,
                        season: Some(season as i32),
                        episode: Some(episode as i32),
                    }),
                    Some(batch_id.clone()),
                    Some(batch_name.clone()),
                )
                .await
                .map_err(|e| format!("Failed to initiate download: {}", e))?;

            grabbed.push(json!({
                "task_id": task.id,
                "name": candidate.original_name,
                "score": candidate.score,
                "size": candidate.size,
                "season": season,
                "episode": episode,
            }));
        }

        Ok(json!({
            "success": true,
            "message": format!("Queued {} episode(s) for {}", grabbed.len(), title),
            "grabbed": grabbed,
            "expected_episodes": valid_episodes.len(),
            "batch_id": batch_id,
            "batch_name": batch_name,
            "arr_id": arr_id
        }))
    }

    async fn get_valid_tv_episodes(&self, tmdb_id: i64) -> HashSet<(u32, u32)> {
        let mut valid = HashSet::new();
        let Some(details) = self.tmdb_service.get_tv_details(tmdb_id).await else {
            return valid;
        };

        let Some(seasons) = details["seasons"].as_array() else {
            return valid;
        };

        for season in seasons {
            let Some(season_number) = season["season_number"].as_i64() else {
                continue;
            };
            if season_number <= 0 {
                continue;
            }

            let Some(season_details) = self
                .tmdb_service
                .get_season_details(tmdb_id, season_number as i32)
                .await
            else {
                continue;
            };

            if let Some(episodes) = season_details["episodes"].as_array() {
                for episode in episodes {
                    if let Some(episode_number) = episode["episode_number"].as_i64() {
                        if episode_number > 0 {
                            valid.insert((season_number as u32, episode_number as u32));
                        }
                    }
                }
            }
        }

        valid
    }

    /// Internal helper to perform search logic similar to api/discovery.rs
    async fn perform_internal_search(
        &self,
        title: &str,
        year: Option<String>,
        media_type: &str,
        tmdb_id: Option<i64>,
    ) -> Result<Vec<InternalSearchResult>, String> {
        let mut queries = vec![title.to_string()];
        let mut aliases_for_match = Vec::new();

        // Resolve Aliases
        if let Some(tid) = tmdb_id {
            let aliases = if media_type == "tv" {
                self.tmdb_service.get_tv_alternative_titles(tid).await
            } else {
                self.tmdb_service.get_movie_alternative_titles(tid).await
            };
            for alias in aliases.iter().take(2) {
                queries.push(alias.title.clone());
                aliases_for_match.push(alias.title.clone());
            }
        }

        if let Some(ref y) = year {
            let base = queries.clone();
            for q in base {
                queries.push(format!("{} {}", q, y));
            }
        }

        let mut all_results = Vec::new();

        for query in queries {
            let url = format!(
                "https://timfshare.com/api/v1/string-query-search?query={}",
                urlencoding::encode(&query)
            );

            if let Ok(resp) = self
                .http_client
                .post(&url)
                .header("Content-Length", "0")
                .send()
                .await
            {
                if let Ok(data) = resp.json::<Value>().await {
                    if let Some(items) = data["data"].as_array() {
                        for item in items {
                            let name = item["name"].as_str().unwrap_or("").to_string();
                            let url = item["url"].as_str().unwrap_or("").to_string();

                            // Scoring & Filtering
                            let sim_res =
                                calculate_unified_similarity(title, &name, &aliases_for_match);
                            if !sim_res.is_valid {
                                continue;
                            }
                            if is_different_franchise_entry(title, &name) {
                                continue;
                            }

                            let parsed = smart_parse(&name);

                            // Score = relevance (title similarity) + quality (source/resolution/HDR/audio/etc.)
                            // similarity acts as a gate (filtered above) and minor tiebreaker;
                            // total_score() from the smart tokenizer dominates ranking so the
                            // best-quality file wins among equally-relevant results.
                            let relevance = (sim_res.score * 50.0) as i32; // 0-50 range
                            let quality = parsed.total_score(); // 10-355 range
                            let score = relevance + quality;

                            all_results.push(InternalSearchResult {
                                original_name: name,
                                url,
                                size: item["size"].as_u64().unwrap_or(0),
                                score,
                            });
                        }
                    }
                }
            }

            match self.db.search_folder_cache_async(query.clone(), 100).await {
                Ok(items) => {
                    for item in items {
                        if item.is_directory {
                            continue;
                        }
                        if let (Some(expected_tmdb), Some(item_tmdb)) = (tmdb_id, item.tmdb_id) {
                            if expected_tmdb != item_tmdb {
                                continue;
                            }
                        }

                        let sim_res =
                            calculate_unified_similarity(title, &item.name, &aliases_for_match);
                        if !sim_res.is_valid {
                            continue;
                        }
                        if is_different_franchise_entry(title, &item.name) {
                            continue;
                        }

                        let parsed = smart_parse(&item.name);
                        let relevance = (sim_res.score * 50.0) as i32;
                        let quality = parsed.total_score();

                        all_results.push(InternalSearchResult {
                            original_name: item.name,
                            url: item.fshare_url,
                            size: item.size,
                            score: relevance + quality,
                        });
                    }
                }
                Err(e) => {
                    warn!(
                        "Folder cache search failed for smart grab query '{}': {}",
                        query, e
                    );
                }
            }
        }

        all_results.sort_by(|a, b| b.score.cmp(&a.score));
        // Remove duplicates by URL (dedup_by only works on consecutive elements;
        // use a HashSet over the sorted list to correctly remove all URL dupes)
        let mut seen_urls = HashSet::new();
        all_results.retain(|r| seen_urls.insert(r.url.clone()));

        Ok(all_results)
    }
}

#[derive(Debug, Clone)]
struct InternalSearchResult {
    pub original_name: String,
    pub url: String,
    pub size: u64,
    pub score: i32,
}
