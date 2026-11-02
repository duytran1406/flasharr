//! Discovery API Routes
//!
//! Smart search and discovery features.

use crate::api::search_pipeline::search_folder_cache;
use crate::utils::smart_tokenizer::smart_parse;
use crate::utils::title_matcher::{get_title_keywords, is_different_franchise_entry};
use crate::AppState;
use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/smart-search", post(smart_search))
        .route("/popular-today", get(popular_today))
        .route("/available-on-fshare", get(available_on_fshare))
        .route("/trending", get(trending))
}

// ============================================================================
// Request Types
// ============================================================================

#[derive(Deserialize)]
pub struct SmartSearchRequest {
    pub title: String,
    #[serde(default)]
    pub year: Option<String>,
    #[serde(default)]
    pub season: Option<u32>,
    #[serde(default)]
    pub episode: Option<u32>,
    #[serde(default = "default_media_type")]
    pub media_type: String,
    #[serde(default)]
    pub tmdb_id: Option<u32>,
}

#[derive(Deserialize)]
struct PopularQuery {
    #[serde(default = "default_media_type", alias = "type")]
    media_type: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Deserialize)]
struct AvailabilityQuery {
    title: String,
    #[serde(default)]
    year: Option<String>,
}

fn default_media_type() -> String {
    "movie".to_string()
}
fn default_limit() -> usize {
    20
}

// ============================================================================
// Response Types
// ============================================================================

#[allow(dead_code)]
#[derive(Serialize)]
pub struct SmartSearchResponse {
    pub queries_used: Vec<String>,
    pub results: Vec<SearchResult>,
    pub groups: Option<Vec<QualityGroup>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seasons: Option<Vec<SeasonGroup>>,
    pub media_type: String,
    pub version: String,
}

#[derive(Serialize, Clone)]
pub struct SearchResult {
    pub name: String,
    pub original_name: String,
    pub url: String,
    pub size: u64,
    pub score: i32,
    pub fcode: String,
    pub quality: Option<String>,
    pub resolution: Option<String>,
    pub source: Option<String>,
    pub viet_sub: bool,
    pub viet_dub: bool,
}

#[derive(Serialize)]
pub struct QualityGroup {
    pub quality: String,
    pub score: i32,
    pub count: usize,
    pub files: Vec<SearchResult>,
}

#[derive(Serialize)]
pub struct SeasonGroup {
    pub season: u32,
    pub episodes_grouped: Vec<EpisodeGroup>,
}

#[derive(Serialize)]
pub struct EpisodeGroup {
    pub episode_number: u32,
    pub name: String,
    pub files: Vec<SearchResult>,
}

#[derive(Serialize)]
struct PopularItem {
    id: u32,
    title: String,
    media_type: String,
    poster_url: Option<String>,
    backdrop_path: Option<String>,
    overview: Option<String>,
    vote_average: f32,
    score: f32,
    fshare_available: bool,
    fshare_count: usize,
}

#[derive(Serialize)]
struct PopularResponse {
    results: Vec<PopularItem>,
}

#[derive(Serialize)]
struct AvailabilityResponse {
    available: bool,
    count: usize,
    results: Vec<SearchResult>,
}

// ============================================================================
// Handlers
// ============================================================================

/// POST /api/discovery/smart-search - Perform smart search with v2 tactics
async fn smart_search(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SmartSearchRequest>,
) -> Json<Value> {
    let client = Client::builder()
        .cookie_store(true)
        .build()
        .unwrap_or_default();
    let mut queries = vec![payload.title.clone()];

    // 1. Resolve Aliases from TMDB
    let mut aliases = Vec::new();
    if let Some(tmdb_id) = payload.tmdb_id {
        let url = format!(
            "https://api.themoviedb.org/3/{}/{}/alternative_titles?api_key={}",
            if payload.media_type == "tv" {
                "tv"
            } else {
                "movie"
            },
            tmdb_id,
            state.tmdb_service.api_key().await
        );

        if let Ok(resp) = client.get(&url).send().await {
            if let Ok(data) = resp.json::<Value>().await {
                if let Some(titles) = data["titles"]
                    .as_array()
                    .or_else(|| data["results"].as_array())
                {
                    for t in titles {
                        if let Some(title) = t["title"].as_str().or_else(|| t["name"].as_str()) {
                            aliases.push(title.to_string());
                            if aliases.len() < 3 {
                                queries.push(title.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Add Year to queries
    if let Some(ref year) = payload.year {
        let base_queries = queries.clone();
        for q in base_queries {
            queries.push(format!("{} {}", q, year));
        }
    }

    // Add S/E markers
    if let (Some(s), Some(e)) = (payload.season, payload.episode) {
        queries.push(format!("{} S{:02}E{:02}", payload.title, s, e));
        queries.push(format!("{} S{:02} E{:02}", payload.title, s, e));
        queries.push(format!("{} {}x{:02}", payload.title, s, e));
    } else if let Some(s) = payload.season {
        queries.push(format!("{} Season {}", payload.title, s));
        queries.push(format!("{} S{:02}", payload.title, s));
    }

    // 2. Search only the synchronized folder cache.
    let all_raw_results = search_folder_cache(&state.folder_cache_service, &queries, 100)
        .await
        .into_iter()
        .map(|item| json!({"name": item.name, "url": item.url, "size": item.size}))
        .collect::<Vec<_>>();

    // 3. Parse and Filter
    let mut filtered_results = Vec::new();
    let mut seen_urls = HashMap::new();
    let search_keywords = get_title_keywords(&payload.title);

    for item in all_raw_results {
        let name = item["name"].as_str().unwrap_or("").to_string();
        let url = item["url"].as_str().unwrap_or("").to_string();

        if seen_urls.contains_key(&url) {
            continue;
        }
        seen_urls.insert(url.clone(), true);

        // Franchise Conflict Check
        if is_different_franchise_entry(&payload.title, &name) {
            continue;
        }

        // Unified Similarity Check from v2
        let sim_res =
            crate::utils::title_matcher::calculate_unified_similarity(&payload.title, &name, &[]);
        if !sim_res.is_valid && search_keywords.len() > 1 {
            // Allow if it's a very high similarity match of an alias
            let mut alias_match = false;
            for alias in &aliases {
                let alias_sim =
                    crate::utils::title_matcher::calculate_unified_similarity(alias, &name, &[]);
                if alias_sim.is_valid {
                    alias_match = true;
                    break;
                }
            }
            if !alias_match {
                continue;
            }
        }

        let parsed = smart_parse(&name);

        // Season validation for series (Strict Mode)
        if let Some(req_s) = payload.season {
            if let Some(file_s) = parsed.season {
                if file_s != req_s {
                    continue;
                }
            }
        }
        // Episode validation REMOVED to match V2 permissive behavior (V2 regex fails on 'Chapter', allowing all files)

        let fcode = url.split("/file/").last().unwrap_or("").to_string();

        // Score = relevance (title similarity) + quality (source/resolution/HDR/audio/etc.)
        // Relevance acts as a minor tiebreaker; total_score() from the smart tokenizer
        // dominates ranking so the best-quality file wins among equally-relevant results.
        let matched_count = (search_keywords.len() as f32 * sim_res.score).round() as i32;
        let relevance = matched_count * 10 + (sim_res.score * 50.0) as i32;
        let quality = parsed.total_score(); // 10-355: source×resolution + HDR/DV/codec/audio/vietsub
        let size_gb = item["size"].as_u64().unwrap_or(0) as f64 / (1024.0 * 1024.0 * 1024.0);
        let size_bonus = (size_gb.min(10.0) * 5.0) as i32;
        let score = relevance + quality + size_bonus;

        filtered_results.push(SearchResult {
            name: parsed.title,
            original_name: name,
            url,
            size: item["size"].as_u64().unwrap_or(0),
            score,
            fcode,
            quality: format!(
                "{} {}",
                parsed.resolution.as_deref().unwrap_or(""),
                parsed.source.as_deref().unwrap_or("")
            )
            .trim()
            .to_string()
            .into(),
            resolution: parsed.resolution.clone(),
            source: parsed.source.clone(),
            viet_sub: parsed.viet_sub,
            viet_dub: parsed.viet_dub,
        });
    }

    // Sort by score desc
    filtered_results.sort_by(|a, b| b.score.cmp(&a.score));

    // 4. Grouping
    if payload.media_type == "tv" {
        // Group by season/episode
        let mut seasons_map: HashMap<u32, HashMap<u32, Vec<SearchResult>>> = HashMap::new();

        for res in filtered_results {
            let s = payload.season.unwrap_or(1); // Default to search season or 1
            let e = smart_parse(&res.original_name).episode.unwrap_or(0);

            seasons_map
                .entry(s)
                .or_default()
                .entry(e)
                .or_default()
                .push(res);
        }

        let mut seasons = Vec::new();
        for (s_num, eps_map) in seasons_map {
            let mut episodes_grouped = Vec::new();
            for (e_num, files) in eps_map {
                episodes_grouped.push(EpisodeGroup {
                    episode_number: e_num,
                    name: format!("Episode {}", e_num),
                    files,
                });
            }
            episodes_grouped.sort_by(|a, b| a.episode_number.cmp(&b.episode_number));
            seasons.push(SeasonGroup {
                season: s_num,
                episodes_grouped,
            });
        }

        Json(json!({
            "queries_used": queries,
            "seasons": seasons,
            "media_type": "tv",
            "version": "v3.2-reflection-fix"
        }))
    } else {
        // Group by Quality for Movies
        let mut quality_map: HashMap<String, Vec<SearchResult>> = HashMap::new();
        for res in filtered_results {
            let q = res.resolution.clone().unwrap_or("SD".to_string());
            quality_map.entry(q).or_default().push(res);
        }

        let mut groups = Vec::new();
        for (q_name, files) in quality_map {
            let avg_score = files.iter().map(|f| f.score).sum::<i32>() / files.len() as i32;
            groups.push(QualityGroup {
                quality: q_name,
                score: avg_score,
                count: files.len(),
                files,
            });
        }
        groups.sort_by(|a, b| b.score.cmp(&a.score));

        Json(json!({
            "queries_used": queries,
            "groups": groups,
            "media_type": "movie",
            "version": "v3.2-reflection-fix"
        }))
    }
}

/// GET /api/discovery/popular-today - Get popular items with Fshare availability
async fn popular_today(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PopularQuery>,
) -> Json<PopularResponse> {
    let client = Client::new();
    let url = format!(
        "https://api.themoviedb.org/3/trending/{}/day?api_key={}",
        params.media_type,
        state.tmdb_service.api_key().await
    );

    let mut results: Vec<PopularItem> = Vec::new();

    if let Ok(resp) = client.get(&url).send().await {
        if let Ok(data) = resp.json::<Value>().await {
            if let Some(items) = data["results"].as_array() {
                for item in items.iter().take(params.limit) {
                    let id = item["id"].as_u64().unwrap_or(0) as u32;
                    let title = item["title"]
                        .as_str()
                        .or_else(|| item["name"].as_str())
                        .unwrap_or("Unknown")
                        .to_string();
                    let poster_path = item["poster_path"].as_str();
                    let backdrop_path = item["backdrop_path"].as_str().map(|s| s.to_string());
                    let overview = item["overview"].as_str().map(|s| s.to_string());
                    let vote_average = item["vote_average"].as_f64().unwrap_or(0.0) as f32;

                    results.push(PopularItem {
                        id,
                        title,
                        media_type: params.media_type.clone(),
                        poster_url: poster_path
                            .map(|p| format!("https://image.tmdb.org/t/p/w500{}", p)),
                        backdrop_path,
                        overview,
                        vote_average,
                        score: vote_average,
                        fshare_available: true, // Mocked for UI
                        fshare_count: 5,
                    });
                }
            }
        }
    }

    Json(PopularResponse { results })
}

/// GET /api/discovery/available-on-fshare - Check Fshare availability
async fn available_on_fshare(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AvailabilityQuery>,
) -> Json<AvailabilityResponse> {
    let query = if let Some(ref year) = params.year {
        format!("{} {}", params.title, year)
    } else {
        params.title.clone()
    };

    let raw_results = search_folder_cache(&state.folder_cache_service, &[query], 5).await;

    let mut results: Vec<SearchResult> = Vec::new();
    for item in raw_results {
        let parsed = smart_parse(&item.name);

        results.push(SearchResult {
            name: parsed.title,
            original_name: item.name,
            url: item.url,
            size: item.size,
            score: 0,
            fcode: item.fcode,
            quality: format!(
                "{} {}",
                parsed.resolution.as_deref().unwrap_or(""),
                parsed.source.as_deref().unwrap_or("")
            )
            .trim()
            .to_string()
            .into(),
            resolution: parsed.resolution.clone(),
            source: parsed.source.clone(),
            viet_sub: parsed.viet_sub,
            viet_dub: parsed.viet_dub,
        });
    }

    let count = results.len();
    Json(AvailabilityResponse {
        available: count > 0,
        count,
        results,
    })
}

/// GET /api/discovery/trending
async fn trending() -> Json<TrendingResponse> {
    // ponytail: add a local popularity metric before reintroducing trending results.
    Json(TrendingResponse {
        results: Vec::new(),
    })
}

#[derive(Serialize)]
pub struct TrendingResponse {
    pub results: Vec<TrendingItem>,
}

#[derive(Serialize)]
pub struct TrendingItem {
    #[serde(rename = "id")]
    pub fcode: String,

    #[serde(rename = "name")]
    pub original_filename: String,

    #[serde(rename = "parsed_name")]
    pub name: String,

    pub url: String,
    pub size: u64,
    pub quality: Option<String>,

    #[serde(rename = "vietsub")]
    pub has_vietsub: bool,

    #[serde(rename = "vietdub")]
    pub has_vietdub: bool,

    pub tmdb_id: Option<u32>,
    pub tmdb_title: Option<String>,
    pub poster_url: Option<String>,
    pub vote_average: Option<f32>,
    pub year: Option<String>,
    pub media_type: Option<String>,
}
