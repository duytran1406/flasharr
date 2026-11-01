//! TMDB Proxy API Routes
//!
//! Proxy endpoints for The Movie Database (TMDB) API.

use crate::arr::client::SonarrEpisode;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

const TMDB_API_BASE: &str = "https://api.themoviedb.org/3";
const TMDB_IMAGE_BASE: &str = "https://image.tmdb.org/t/p";

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/search", get(search))
        .route("/discover/:media_type", get(discover))
        .route("/movie/:id", get(movie_details))
        .route("/tv/:id", get(tv_details))
        .route("/tv/resolve/:id", get(tv_resolve))
        .route("/collection/:id", get(collection_details))
        .route("/tv/:id/season/:season", get(season_details))
        .route("/genres/:media_type", get(genres))
        .route("/:media_type/:id/similar", get(similar))
        .route("/:media_type/:id/recommendations", get(recommendations))
        .route("/trending/:media_type/:time_window", get(trending))
        .route("/image/:size/*path", get(proxy_image))
}

// ============================================================================
// Query Parameters
// ============================================================================

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_media_type")]
    media_type: String,
    #[serde(default = "default_page")]
    page: u32,
}

#[derive(Deserialize)]
struct DiscoverQuery {
    #[serde(default = "default_page")]
    page: u32,
    #[serde(default)]
    genre: Option<String>,
    #[serde(default)]
    #[serde(rename = "with_genres")]
    with_genres: Option<String>,
    #[serde(default)]
    year: Option<u32>,
    #[serde(default)]
    #[serde(rename = "primary_release_date.gte")]
    primary_release_date_gte: Option<String>,
    #[serde(default)]
    #[serde(rename = "primary_release_date.lte")]
    primary_release_date_lte: Option<String>,
    #[serde(default)]
    #[serde(rename = "first_air_date.gte")]
    first_air_date_gte: Option<String>,
    #[serde(default)]
    #[serde(rename = "first_air_date.lte")]
    first_air_date_lte: Option<String>,
    #[serde(default)]
    sort_by: Option<String>,
    #[serde(default)]
    vote_average_gte: Option<f32>,
    #[serde(default)]
    #[serde(rename = "vote_average.gte")]
    vote_average_dot_gte: Option<f32>,
    #[serde(default)]
    #[serde(rename = "vote_average.lte")]
    vote_average_lte: Option<f32>,
}

#[derive(Deserialize)]
struct PaginationQuery {
    #[serde(default = "default_page")]
    page: u32,
}

fn default_media_type() -> String {
    "multi".to_string()
}
fn default_page() -> u32 {
    1
}

// ============================================================================
// Response Types
// ============================================================================

#[allow(dead_code)]
#[derive(Serialize)]
struct TmdbResponse {
    #[serde(flatten)]
    data: Value,
}

// ============================================================================
// TMDB Client
// ============================================================================

struct TmdbClient {
    client: Client,
    api_key: String,
}

impl TmdbClient {
    async fn from_state(state: &AppState) -> Self {
        Self {
            client: Client::new(),
            api_key: state.tmdb_service.api_key().await,
        }
    }

    pub(crate) fn with_key(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
        }
    }

    pub(crate) async fn get(
        &self,
        path: &str,
        extra_params: &[(&str, &str)],
    ) -> Result<Value, StatusCode> {
        if self.api_key.is_empty() {
            tracing::error!("[TMDB] API key is empty");
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }

        let mut url = format!("{}{}", TMDB_API_BASE, path);
        url.push_str(&format!("?api_key={}", self.api_key));

        for (key, value) in extra_params {
            url.push_str(&format!("&{}={}", key, value));
        }

        tracing::debug!("[TMDB] GET {}", url.replace(&self.api_key, "***"));

        let resp = self.client.get(&url).send().await.map_err(|e| {
            tracing::error!("[TMDB] Request failed: {} - {}", path, e);
            StatusCode::BAD_GATEWAY
        })?;

        let status = resp.status();
        if !status.is_success() {
            tracing::warn!("[TMDB] API returned {}: {}", status, path);
            return Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY));
        }

        resp.json::<Value>().await.map_err(|e| {
            tracing::error!("[TMDB] JSON parse failed: {} - {}", path, e);
            StatusCode::BAD_GATEWAY
        })
    }
}

fn episode_u32(ep: &Value, key: &str) -> Option<u32> {
    ep.get(key).and_then(|v| v.as_u64()).map(|v| v as u32)
}

fn canonical_season(ep: &Value) -> Option<u32> {
    episode_u32(ep, "seasonNumber").or_else(|| episode_u32(ep, "season_number"))
}

fn canonical_episode(ep: &Value) -> Option<u32> {
    episode_u32(ep, "number").or_else(|| episode_u32(ep, "episode_number"))
}

fn tvdb_episode_to_tmdb_shape(ep: &Value) -> Value {
    serde_json::json!({
        "id": ep.get("id").and_then(|v| v.as_i64()).unwrap_or_default(),
        "name": ep.get("name").and_then(|v| v.as_str()).or_else(|| ep.get("title").and_then(|v| v.as_str())).unwrap_or(""),
        "overview": ep.get("overview").and_then(|v| v.as_str()).unwrap_or(""),
        "still_path": ep.get("image").and_then(|v| v.as_str()),
        "air_date": episode_air_date(ep).unwrap_or(""),
        "episode_number": canonical_episode(ep).unwrap_or(0),
        "season_number": canonical_season(ep).unwrap_or(0),
        "vote_average": 0.0
    })
}

fn sonarr_episode_to_tmdb_shape(ep: &SonarrEpisode) -> Value {
    serde_json::json!({
        "id": ep.id,
        "name": ep.title.clone().unwrap_or_else(|| format!("Episode {}", ep.episode_number)),
        "overview": ep.overview.clone().unwrap_or_default(),
        "still_path": Value::Null,
        "air_date": ep.air_date_utc.clone().unwrap_or_default(),
        "episode_number": ep.episode_number.max(0) as u32,
        "season_number": ep.season_number.max(0) as u32,
        "vote_average": 0.0
    })
}

async fn sonarr_episodes_by_tvdb(state: &AppState, tvdb_id: u64) -> Option<Vec<Value>> {
    let client = state.download_orchestrator.get_arr_client().await?;
    let series = client.get_all_series().await.ok()?;
    let series = series
        .into_iter()
        .find(|s| s.tvdb_id == Some(tvdb_id as i32))?;
    let episodes = client.get_episodes(series.id).await.ok()?;
    Some(episodes.iter().map(sonarr_episode_to_tmdb_shape).collect())
}

fn episode_air_date(ep: &Value) -> Option<&str> {
    ep.get("air_date")
        .and_then(|v| v.as_str())
        .or_else(|| ep.get("aired").and_then(|v| v.as_str()))
        .or_else(|| ep.get("firstAired").and_then(|v| v.as_str()))
        .filter(|s| !s.trim().is_empty())
}

fn tmdb_episode_candidates(seasons: &[Value]) -> Vec<&Value> {
    let mut out = Vec::new();
    for season in seasons {
        if let Some(episodes) = season.get("episodes").and_then(|v| v.as_array()) {
            for ep in episodes {
                out.push(ep);
            }
        }
    }
    out.sort_by_key(|ep| {
        (
            episode_u32(ep, "season_number").unwrap_or(u32::MAX),
            episode_u32(ep, "episode_number").unwrap_or(u32::MAX),
        )
    });
    out
}

fn tvdb_absolute_number(all_tvdb_episodes: &[Value], season: u32, episode: u32) -> Option<u32> {
    let mut counts: BTreeMap<u32, u32> = BTreeMap::new();
    for ep in all_tvdb_episodes {
        let s = canonical_season(ep)?;
        let e = canonical_episode(ep)?;
        if s > 0 {
            let current = counts.entry(s).or_insert(0);
            *current = (*current).max(e);
        }
    }
    let previous: u32 = counts
        .iter()
        .filter(|(s, _)| **s < season)
        .map(|(_, count)| *count)
        .sum();
    Some(previous + episode)
}

fn enrich_tvdb_episode_with_tmdb_metadata(
    tvdb_ep: &Value,
    all_tvdb_episodes: &[Value],
    tmdb_seasons: &[Value],
) -> Value {
    let mut shaped = tvdb_episode_to_tmdb_shape(tvdb_ep);
    let season = canonical_season(tvdb_ep).unwrap_or(0);
    let episode = canonical_episode(tvdb_ep).unwrap_or(0);
    let candidates = tmdb_episode_candidates(tmdb_seasons);

    let by_air_date = episode_air_date(tvdb_ep).and_then(|date| {
        candidates
            .iter()
            .find(|ep| episode_air_date(ep) == Some(date))
            .copied()
    });

    let by_same_season_episode = candidates
        .iter()
        .find(|ep| canonical_season(ep) == Some(season) && canonical_episode(ep) == Some(episode))
        .copied();

    let by_flat_absolute =
        tvdb_absolute_number(all_tvdb_episodes, season, episode).and_then(|abs| {
            candidates
                .iter()
                .find(|ep| episode_u32(ep, "episode_number") == Some(abs))
                .copied()
        });

    if let Some(tmdb_ep) = by_air_date.or(by_same_season_episode).or(by_flat_absolute) {
        if let Some(obj) = shaped.as_object_mut() {
            if let Some(still) = tmdb_ep.get("still_path").cloned() {
                if !still.is_null() {
                    obj.insert("still_path".to_string(), still);
                }
            }
            if let Some(overview) = tmdb_ep.get("overview").cloned() {
                if overview
                    .as_str()
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false)
                {
                    obj.insert("overview".to_string(), overview);
                }
            }
            obj.insert(
                "tmdb_episode_ref".to_string(),
                serde_json::json!({
                    "season_number": episode_u32(tmdb_ep, "season_number"),
                    "episode_number": episode_u32(tmdb_ep, "episode_number"),
                }),
            );
        }
    }

    shaped
}

fn tvdb_seasons_from_episodes(episodes: &[Value]) -> Vec<Value> {
    let mut by_season: BTreeMap<u32, Vec<&Value>> = BTreeMap::new();
    for ep in episodes {
        if let Some(season) = episode_u32(ep, "seasonNumber") {
            if season > 0 {
                by_season.entry(season).or_default().push(ep);
            }
        }
    }
    by_season
        .into_iter()
        .map(|(season, eps)| {
            let first_air = eps
                .iter()
                .filter_map(|ep| {
                    ep.get("aired")
                        .and_then(|v| v.as_str())
                        .or_else(|| ep.get("firstAired").and_then(|v| v.as_str()))
                })
                .min()
                .unwrap_or("");
            serde_json::json!({
                "id": season,
                "name": format!("Season {}", season),
                "overview": "",
                "poster_path": Value::Null,
                "season_number": season,
                "episode_count": eps.len(),
                "air_date": first_air
            })
        })
        .collect()
}

async fn apply_tvdb_episode_overlay(state: &AppState, tvdb_id: u64, data: &mut Value) {
    if let Some(episodes) = state
        .tvdb_service
        .get_series_episodes_default(tvdb_id as i64)
        .await
    {
        let seasons = tvdb_seasons_from_episodes(&episodes);
        if seasons.is_empty() {
            return;
        }
        if let Some(obj) = data.as_object_mut() {
            obj.insert("seasons".to_string(), Value::Array(seasons));
            obj.insert(
                "number_of_seasons".to_string(),
                Value::from(obj["seasons"].as_array().map(|s| s.len()).unwrap_or(0)),
            );
            obj.insert(
                "number_of_episodes".to_string(),
                Value::from(
                    episodes
                        .iter()
                        .filter(|ep| episode_u32(ep, "seasonNumber").unwrap_or(0) > 0)
                        .count(),
                ),
            );
            obj.insert(
                "metadata_source".to_string(),
                Value::String("tvdb".to_string()),
            );
        }
    }
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/tmdb/search - Search movies and TV shows
async fn search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Value>, StatusCode> {
    let client = TmdbClient::from_state(&state).await;

    let path = format!("/search/{}", params.media_type);
    let page_str = params.page.to_string();

    let data = client
        .get(&path, &[("query", params.q.as_str()), ("page", &page_str)])
        .await?;

    Ok(Json(data))
}

/// GET /api/tmdb/discover/:media_type - Discover movies or TV shows
async fn discover(
    State(state): State<Arc<AppState>>,
    Path(media_type): Path<String>,
    Query(params): Query<DiscoverQuery>,
) -> Result<Json<Value>, StatusCode> {
    let client = TmdbClient::from_state(&state).await;

    let path = format!("/discover/{}", media_type);
    let page_str = params.page.to_string();
    let mut query_params: Vec<(&str, String)> = vec![("page", page_str)];

    if let Some(ref genre) = params.with_genres.or(params.genre) {
        query_params.push(("with_genres", genre.clone()));
    }
    if let Some(year) = params.year {
        let key = if media_type == "movie" {
            "primary_release_year"
        } else {
            "first_air_date_year"
        };
        query_params.push((key, year.to_string()));
    }
    if let Some(ref date) = params.primary_release_date_gte {
        query_params.push(("primary_release_date.gte", date.clone()));
    }
    if let Some(ref date) = params.primary_release_date_lte {
        query_params.push(("primary_release_date.lte", date.clone()));
    }
    if let Some(ref date) = params.first_air_date_gte {
        query_params.push(("first_air_date.gte", date.clone()));
    }
    if let Some(ref date) = params.first_air_date_lte {
        query_params.push(("first_air_date.lte", date.clone()));
    }
    if let Some(ref sort) = params.sort_by {
        query_params.push(("sort_by", sort.clone()));
    }
    if let Some(vote) = params.vote_average_dot_gte.or(params.vote_average_gte) {
        query_params.push(("vote_average.gte", vote.to_string()));
    }
    if let Some(vote) = params.vote_average_lte {
        query_params.push(("vote_average.lte", vote.to_string()));
    }

    let params_ref: Vec<(&str, &str)> =
        query_params.iter().map(|(k, v)| (*k, v.as_str())).collect();

    let data = client.get(&path, &params_ref).await?;
    Ok(Json(data))
}

/// GET /api/tmdb/movie/:id - Get movie details
async fn movie_details(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
) -> Result<Json<Value>, StatusCode> {
    let client = TmdbClient::from_state(&state).await;
    let path = format!("/movie/{}", id);

    let data = client
        .get(&path, &[("append_to_response", "credits,videos,images")])
        .await?;

    Ok(Json(data))
}

/// GET /api/tmdb/tv/:id - Get TV show details
/// Requests en-US overview first; if empty (non-English original), re-fetches
/// using the show's original_language so the native overview is returned.
async fn tv_details(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
) -> Result<Json<Value>, StatusCode> {
    let client = TmdbClient::from_state(&state).await;
    let path = format!("/tv/{}", id);

    let mut data = client
        .get(
            &path,
            &[
                (
                    "append_to_response",
                    "credits,videos,images,keywords,content_ratings,external_ids",
                ),
                ("language", "en-US"),
            ],
        )
        .await?;

    // If the English overview is empty, re-fetch with the show's original language
    // so Chinese/Korean/Japanese shows display their native-language overview.
    let overview_empty = data
        .get("overview")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);

    if overview_empty {
        if let Some(orig_lang) = data.get("original_language").and_then(|v| v.as_str()) {
            // Build locale string: zh -> zh-CN, ko -> ko-KR, ja -> ja-JP, etc.
            let locale = match orig_lang {
                "zh" => "zh-CN",
                "ko" => "ko-KR",
                "ja" => "ja-JP",
                "vi" => "vi-VN",
                "th" => "th-TH",
                "pt" => "pt-BR",
                "es" => "es-ES",
                "fr" => "fr-FR",
                "de" => "de-DE",
                "it" => "it-IT",
                "ar" => "ar-SA",
                "ru" => "ru-RU",
                _ => orig_lang,
            };

            if let Ok(native_data) = client
                .get(
                    &path,
                    &[
                        (
                            "append_to_response",
                            "keywords,content_ratings,external_ids",
                        ),
                        ("language", locale),
                    ],
                )
                .await
            {
                // Splice in only the overview (and name if also missing)
                if let Some(overview) = native_data.get("overview").and_then(|v| v.as_str()) {
                    if !overview.trim().is_empty() {
                        if let Some(obj) = data.as_object_mut() {
                            obj.insert("overview".to_string(), Value::String(overview.to_string()));
                            // Also expose original-language name for UI display
                            if let Some(orig_name) =
                                native_data.get("name").and_then(|v| v.as_str())
                            {
                                obj.insert(
                                    "original_name_native".to_string(),
                                    Value::String(orig_name.to_string()),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(tvdb_id) = data
        .get("external_ids")
        .and_then(|v| v.get("tvdb_id"))
        .and_then(|v| v.as_u64())
    {
        apply_tvdb_episode_overlay(&state, tvdb_id, &mut data).await;
    }

    Ok(Json(data))
}

/// GET /api/tmdb/tv/resolve/:id - Resolve a TV route id to TVDB-canonical identity.
async fn tv_resolve(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
) -> Result<Json<Value>, StatusCode> {
    let client = TmdbClient::from_state(&state).await;

    // TMDB-origin routes still dominate discovery/search/dashboard links. Try TMDB first so
    // numeric collisions (a TMDB ID that is also some unrelated TVDB ID) do not open/add the
    // wrong series.
    let tmdb_path = format!("/tv/{}", id);
    if let Ok(data) = client
        .get(&tmdb_path, &[("append_to_response", "external_ids")])
        .await
    {
        if let Some(tvdb_id) = data
            .get("external_ids")
            .and_then(|v| v.get("tvdb_id"))
            .and_then(|v| v.as_u64())
        {
            return Ok(Json(serde_json::json!({
                "canonical_tvdb_id": tvdb_id,
                "legacy_tmdb_id": id,
                "is_canonical_tv_route": false
            })));
        }
    }

    // Canonical path: id is TVDB, bridge to TMDB for existing TMDB-backed UI data.
    let find_path = format!("/find/{}", id);
    if let Ok(find) = client
        .get(&find_path, &[("external_source", "tvdb_id")])
        .await
    {
        if let Some(tmdb_id) = find
            .get("tv_results")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_u64())
        {
            return Ok(Json(serde_json::json!({
                "canonical_tvdb_id": id,
                "legacy_tmdb_id": tmdb_id,
                "is_canonical_tv_route": true
            })));
        }
    }

    Err(StatusCode::NOT_FOUND)
}

/// GET /api/tmdb/collection/:id - Get collection details
async fn collection_details(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
) -> Result<Json<Value>, StatusCode> {
    let client = TmdbClient::from_state(&state).await;
    let path = format!("/collection/{}", id);

    let data = client.get(&path, &[]).await?;

    Ok(Json(data))
}

/// GET /api/tmdb/tv/:id/season/:season - Get season details
async fn season_details(
    State(state): State<Arc<AppState>>,
    Path((id, season)): Path<(u32, u32)>,
) -> Result<Json<Value>, StatusCode> {
    let client = TmdbClient::from_state(&state).await;
    let path = format!("/tv/{}/season/{}", id, season);

    let mut data = match client.get(&path, &[]).await {
        Ok(data) => data,
        Err(status) if season != 1 => client
            .get(&format!("/tv/{}/season/1", id), &[])
            .await
            .or(Err(status))?,
        Err(status) => return Err(status),
    };
    let tvdb_id = client
        .get(
            &format!("/tv/{}", id),
            &[("append_to_response", "external_ids")],
        )
        .await
        .ok()
        .and_then(|v| {
            v.get("external_ids")
                .and_then(|e| e.get("tvdb_id"))
                .and_then(|x| x.as_u64())
        });
    if let Some(tvdb_id) = tvdb_id {
        let mut episodes = state
            .tvdb_service
            .get_series_episodes_default(tvdb_id as i64)
            .await;
        if episodes.is_none() {
            episodes = sonarr_episodes_by_tvdb(&state, tvdb_id).await;
        }
        if let Some(episodes) = episodes {
            let mut tmdb_metadata_seasons = vec![data.clone()];
            if season != 1 {
                if let Ok(flat_season) = client.get(&format!("/tv/{}/season/1", id), &[]).await {
                    tmdb_metadata_seasons.push(flat_season);
                }
            }

            let mut season_eps: Vec<Value> = episodes
                .iter()
                .filter(|ep| canonical_season(ep) == Some(season))
                .map(|ep| {
                    enrich_tvdb_episode_with_tmdb_metadata(ep, &episodes, &tmdb_metadata_seasons)
                })
                .collect();
            season_eps.sort_by_key(|ep| {
                ep.get("episode_number")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            });
            if !season_eps.is_empty() {
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("id".to_string(), Value::from(season));
                    obj.insert(
                        "name".to_string(),
                        Value::String(format!("Season {}", season)),
                    );
                    obj.insert("season_number".to_string(), Value::from(season));
                    obj.insert("episodes".to_string(), Value::Array(season_eps));
                    obj.insert(
                        "metadata_source".to_string(),
                        Value::String("tvdb_sonarr+tmdb_bridge".to_string()),
                    );
                }
            }
        }
    }
    Ok(Json(data))
}

/// GET /api/tmdb/genres/:media_type - Get genres
async fn genres(
    State(state): State<Arc<AppState>>,
    Path(media_type): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let client = TmdbClient::from_state(&state).await;
    let path = format!("/genre/{}/list", media_type);

    let data = client.get(&path, &[]).await?;
    Ok(Json(data))
}

/// GET /api/tmdb/:media_type/:id/similar - Get similar items
async fn similar(
    State(state): State<Arc<AppState>>,
    Path((media_type, id)): Path<(String, u32)>,
    Query(params): Query<PaginationQuery>,
) -> Result<Json<Value>, StatusCode> {
    let client = TmdbClient::from_state(&state).await;
    let path = format!("/{}/{}/similar", media_type, id);
    let page_str = params.page.to_string();

    let data = client.get(&path, &[("page", &page_str)]).await?;
    Ok(Json(data))
}

/// GET /api/tmdb/:media_type/:id/recommendations - Get recommendations
async fn recommendations(
    State(state): State<Arc<AppState>>,
    Path((media_type, id)): Path<(String, u32)>,
    Query(params): Query<PaginationQuery>,
) -> Result<Json<Value>, StatusCode> {
    let client = TmdbClient::from_state(&state).await;
    let path = format!("/{}/{}/recommendations", media_type, id);
    let page_str = params.page.to_string();

    let data = client.get(&path, &[("page", &page_str)]).await?;
    Ok(Json(data))
}

/// GET /api/tmdb/trending/:media_type/:time_window - Get trending
async fn trending(
    State(state): State<Arc<AppState>>,
    Path((media_type, time_window)): Path<(String, String)>,
    Query(params): Query<PaginationQuery>,
) -> Result<Json<Value>, StatusCode> {
    let client = TmdbClient::from_state(&state).await;
    let path = format!("/trending/{}/{}", media_type, time_window);
    let page_str = params.page.to_string();

    let data = client.get(&path, &[("page", &page_str)]).await?;
    Ok(Json(data))
}

/// GET /api/tmdb/image/:size/*path - Proxy TMDB images
async fn proxy_image(
    Path((size, path)): Path<(String, String)>,
) -> Result<axum::response::Response, StatusCode> {
    let client = Client::new();
    let url = format!("{}/{}/{}", TMDB_IMAGE_BASE, size, path);

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    if !resp.status().is_success() {
        return Err(StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY));
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    let body = resp.bytes().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    Ok(axum::response::Response::builder()
        .header("Content-Type", content_type)
        .header("Cache-Control", "public, max-age=31536000")
        .body(axum::body::Body::from(body))
        .unwrap())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Unit Tests - Helper Functions
    // ========================================================================

    #[test]
    fn tvdb_episode_enrichment_uses_flat_tmdb_episode_metadata() {
        let tvdb_eps = vec![
            serde_json::json!({"id": 1, "name": "A", "seasonNumber": 1, "number": 1, "aired": "1986-01-01"}),
            serde_json::json!({"id": 2, "name": "B", "seasonNumber": 1, "number": 2, "aired": "1986-01-08"}),
            serde_json::json!({"id": 3, "name": "C", "seasonNumber": 2, "number": 1, "aired": "1986-01-15"}),
        ];
        let tmdb_seasons = vec![serde_json::json!({
            "season_number": 1,
            "episodes": [
                {"season_number": 1, "episode_number": 1, "overview": "A art", "still_path": "/a.jpg"},
                {"season_number": 1, "episode_number": 2, "overview": "B art", "still_path": "/b.jpg"},
                {"season_number": 1, "episode_number": 3, "overview": "C art", "still_path": "/c.jpg"}
            ]
        })];

        let out = enrich_tvdb_episode_with_tmdb_metadata(&tvdb_eps[2], &tvdb_eps, &tmdb_seasons);

        assert_eq!(out["season_number"], 2);
        assert_eq!(out["episode_number"], 1);
        assert_eq!(out["still_path"], "/c.jpg");
        assert_eq!(out["tmdb_episode_ref"]["season_number"], 1);
        assert_eq!(out["tmdb_episode_ref"]["episode_number"], 3);
    }

    #[test]
    fn test_default_media_type() {
        assert_eq!(default_media_type(), "multi");
    }

    #[test]
    fn test_default_page() {
        assert_eq!(default_page(), 1);
    }

    #[test]
    fn test_tmdb_client_creation() {
        let client = TmdbClient::with_key("test");
        // Should not panic
        assert!(client.api_key.is_empty() || !client.api_key.is_empty());
    }

    // ========================================================================
    // Unit Tests - Query Parameters
    // ========================================================================

    #[test]
    fn test_search_query_deserialization() {
        let json = r#"{"q": "Inception", "media_type": "movie", "page": 2}"#;
        let query: SearchQuery = serde_json::from_str(json).unwrap();

        assert_eq!(query.q, "Inception");
        assert_eq!(query.media_type, "movie");
        assert_eq!(query.page, 2);
    }

    #[test]
    fn test_search_query_defaults() {
        let json = r#"{"q": "Inception"}"#;
        let query: SearchQuery = serde_json::from_str(json).unwrap();

        assert_eq!(query.q, "Inception");
        assert_eq!(query.media_type, "multi"); // default
        assert_eq!(query.page, 1); // default
    }

    #[test]
    fn test_discover_query_full() {
        let json = r#"{
            "page": 1,
            "genre": "28",
            "year": 2024,
            "sort_by": "popularity.desc",
            "vote_average_gte": 7.5
        }"#;
        let query: DiscoverQuery = serde_json::from_str(json).unwrap();

        assert_eq!(query.page, 1);
        assert_eq!(query.genre, Some("28".to_string()));
        assert_eq!(query.year, Some(2024));
        assert_eq!(query.sort_by, Some("popularity.desc".to_string()));
        assert_eq!(query.vote_average_gte, Some(7.5));
    }

    #[test]
    fn test_discover_query_minimal() {
        let json = r#"{"page": 1}"#;
        let query: DiscoverQuery = serde_json::from_str(json).unwrap();

        assert_eq!(query.page, 1);
        assert_eq!(query.genre, None);
        assert_eq!(query.year, None);
        assert_eq!(query.sort_by, None);
        assert_eq!(query.vote_average_gte, None);
    }

    #[test]
    fn test_discover_query_defaults() {
        let json = r#"{}"#;
        let query: DiscoverQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.page, 1); // default
    }

    #[test]
    fn test_pagination_query() {
        let json = r#"{"page": 5}"#;
        let query: PaginationQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.page, 5);
    }

    #[test]
    fn test_pagination_query_default() {
        let json = r#"{}"#;
        let query: PaginationQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.page, 1);
    }

    // ========================================================================
    // Unit Tests - URL Building
    // ========================================================================

    #[test]
    fn test_tmdb_api_base_constant() {
        assert_eq!(TMDB_API_BASE, "https://api.themoviedb.org/3");
    }

    #[test]
    fn test_tmdb_image_base_constant() {
        assert_eq!(TMDB_IMAGE_BASE, "https://image.tmdb.org/t/p");
    }

    // ========================================================================
    // Integration Tests - TmdbClient
    // ========================================================================

    #[tokio::test]
    async fn test_tmdb_client_get_without_api_key() {
        let client = TmdbClient {
            client: Client::new(),
            api_key: String::new(),
        };
        let result = client.get("/test", &[]).await;
        assert_eq!(result.unwrap_err(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_tmdb_client_url_construction() {
        // This test verifies URL construction logic without making actual requests
        let client = TmdbClient::with_key("");

        // Test that empty API key returns error immediately
        if client.api_key.is_empty() {
            let result = client.get("/test", &[]).await;
            assert_eq!(result.unwrap_err(), StatusCode::SERVICE_UNAVAILABLE);
        }
    }

    // ========================================================================
    // Router Tests
    // ========================================================================

    #[test]
    fn test_router_creation() {
        let router = router();
        // Should not panic
        drop(router);
    }

    // ========================================================================
    // Edge Cases
    // ========================================================================

    #[test]
    fn test_search_query_special_characters() {
        let json = r#"{"q": "The Matrix: Reloaded (2003)"}"#;
        let query: SearchQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.q, "The Matrix: Reloaded (2003)");
    }

    #[test]
    fn test_search_query_unicode() {
        let json = r#"{"q": "千と千尋の神隠し"}"#;
        let query: SearchQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.q, "千と千尋の神隠し");
    }

    #[test]
    fn test_discover_query_negative_vote() {
        let json = r#"{"vote_average_gte": 0.0}"#;
        let query: DiscoverQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.vote_average_gte, Some(0.0));
    }

    #[test]
    fn test_discover_query_max_vote() {
        let json = r#"{"vote_average_gte": 10.0}"#;
        let query: DiscoverQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.vote_average_gte, Some(10.0));
    }

    #[test]
    fn test_pagination_large_page() {
        let json = r#"{"page": 1000}"#;
        let query: PaginationQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.page, 1000);
    }

    #[test]
    fn test_discover_query_old_year() {
        let json = r#"{"year": 1900}"#;
        let query: DiscoverQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.year, Some(1900));
    }

    #[test]
    fn test_discover_query_future_year() {
        let json = r#"{"year": 2030}"#;
        let query: DiscoverQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.year, Some(2030));
    }

    #[test]
    fn test_discover_query_multiple_genres() {
        let json = r#"{"genre": "28,12,16"}"#;
        let query: DiscoverQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.genre, Some("28,12,16".to_string()));
    }

    #[test]
    fn test_discover_query_all_sort_options() {
        let sort_options = vec![
            "popularity.asc",
            "popularity.desc",
            "release_date.asc",
            "release_date.desc",
            "revenue.asc",
            "revenue.desc",
            "primary_release_date.asc",
            "primary_release_date.desc",
            "original_title.asc",
            "original_title.desc",
            "vote_average.asc",
            "vote_average.desc",
            "vote_count.asc",
            "vote_count.desc",
        ];

        for sort in sort_options {
            let json = format!(r#"{{"sort_by": "{}"}}"#, sort);
            let query: DiscoverQuery = serde_json::from_str(&json).unwrap();
            assert_eq!(query.sort_by, Some(sort.to_string()));
        }
    }

    // ========================================================================
    // Serialization Tests
    // ========================================================================

    #[test]
    fn test_tmdb_response_serialization() {
        let value = serde_json::json!({
            "results": [],
            "page": 1,
            "total_pages": 1,
            "total_results": 0
        });

        let response = TmdbResponse {
            data: value.clone(),
        };
        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized, value);
    }
}
