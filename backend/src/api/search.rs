use axum::{
    routing::{get, post},
    Router,
    Json,
    extract::{State, Query},
};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use reqwest::Client;
use crate::AppState;
use crate::utils::smart_tokenizer::smart_parse;
use crate::utils::title_matcher::{calculate_unified_similarity, extract_core_title, get_title_keywords};
use crate::api::search_pipeline::is_media_file;
use futures_util::future::join_all;
use std::time::Duration;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/enhanced", get(enhanced_search))
        .route("/", get(enhanced_search))
        .route("/smart", post(crate::api::smart_search::smart_search))
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_enrich")]
    pub enrich: bool,
}

fn default_page() -> usize { 1 }
fn default_limit() -> usize { 20 }
fn default_enrich() -> bool { true }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchResult {
    #[serde(rename = "id")]
    pub fcode: String,
    
    #[serde(rename = "name")]
    pub original_filename: String,
    
    #[serde(rename = "parsed_name")]
    pub name: String,
    
    pub url: String,
    pub size: u64,
    pub score: i32,
    
    pub quality: Option<String>,
    pub resolution: Option<String>,
    pub source: Option<String>,
    
    pub episode_tag: Option<String>,
    pub is_series: bool,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    
    #[serde(rename = "vietsub")]
    pub has_vietsub: bool,
    
    #[serde(rename = "vietdub")]
    pub has_vietdub: bool,
    
    // TMDB Enrichment
    pub tmdb_id: Option<u32>,
    pub tmdb_title: Option<String>,
    pub poster_path: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_path: Option<String>,
    pub backdrop_url: Option<String>,
    pub vote_average: Option<f32>,
    pub year: Option<String>,
    pub release_date: Option<String>,
    pub overview: Option<String>,
    pub media_type: Option<String>,
}

#[derive(Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub page: usize,
    pub limit: usize,
    pub total: usize,
    pub total_pages: usize,
    pub has_more: bool,
    pub version: String,
}

const TMDB_KEY: &str = "8d95150f3391194ca66fef44df497ad6";

async fn enhanced_search(
    State(_state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Json<SearchResponse> {
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .cookie_store(true)
        .build()
        .unwrap_or_default();
        
    let parsed_query = smart_parse(&params.q);
    
    // Increased Search Depth (Default to 100 to match V2 expectation)
    let url = format!("https://timfshare.com/api/v1/string-query-search?query={}", urlencoding::encode(&params.q.trim()));
    
    let resp = client.post(&url)
        .header("Content-Length", "0")
        .header("Origin", "https://timfshare.com")
        .header("Referer", format!("https://timfshare.com/search?key={}", urlencoding::encode(&params.q)))
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await;
        
    let backend_version = "v3.3-deep-enrich".to_string();

    match resp {
        Ok(r) => {
            if let Ok(data) = r.json::<serde_json::Value>().await {
                let mut results: Vec<SearchResult> = data["data"].as_array().map(|arr| {
                    arr.iter().map(|item| {
                        let original_filename = item["name"].as_str().unwrap_or("Unknown").to_string();
                        let result_url = item["url"].as_str().unwrap_or("").to_string();
                        let fcode = result_url.split("/file/").last().unwrap_or("").to_string();
                        let size = item["size"].as_u64().unwrap_or(0);
                        
                        let parsed = smart_parse(&original_filename);
                        let sim_res = calculate_unified_similarity(&params.q, &original_filename, &[]);
                        
                        let keywords = get_title_keywords(&params.q);
                        let matched_count = (keywords.len() as f32 * sim_res.score).round() as i32;
                        
                        let mut score = matched_count * 10;
                        score += (sim_res.score * 50.0) as i32;
                        
                        if parsed.year.is_some() { score += 20; }
                        if parsed.resolution.is_some() { score += 10; }
                        if parsed.viet_dub || parsed.viet_sub { score += 15; }
                        
                        let size_gb = size as f64 / (1024.0 * 1024.0 * 1024.0);
                        score += (size_gb.min(10.0) * 5.0) as i32;
                        
                        // Relaxed Similarity Penalty (V2 parity: only punish extremely low matches)
                        if sim_res.score < 0.3 { score -= 100; }

                        if let Some(req_s) = parsed_query.season {
                            if let Some(res_s) = parsed.season {
                                if req_s != res_s { score -= 100; }
                            }
                        }
                        
                        let qual_str = format!("{} {}", 
                            parsed.resolution.as_deref().unwrap_or(""), 
                            parsed.source.as_deref().unwrap_or("")
                        ).trim().to_string();
                        let quality = if qual_str.is_empty() { None } else { Some(qual_str) };
                        
                        let episode_tag = if let (Some(s), Some(e)) = (parsed.season, parsed.episode) {
                            Some(format!("S{:02}E{:02}", s, e))
                        } else if let Some(e) = parsed.episode {
                            Some(format!("E{:02}", e))
                        } else {
                            None
                        };

                        SearchResult {
                            fcode,
                            original_filename: original_filename.clone(),
                            name: parsed.title.clone(),
                            url: result_url,
                            size,
                            score,
                            quality,
                            resolution: parsed.resolution.clone(),
                            source: parsed.source.clone(),
                            episode_tag,
                            is_series: parsed.media_type == crate::utils::smart_tokenizer::MediaType::TvShow,
                            season: parsed.season,
                            episode: parsed.episode,
                            has_vietsub: parsed.viet_sub,
                            has_vietdub: parsed.viet_dub,
                            tmdb_id: None,
                            tmdb_title: None,
                            poster_path: None,
                            poster_url: None,
                            backdrop_path: None,
                            backdrop_url: None,
                            vote_average: None,
                            year: parsed.year.map(|y| y.to_string()),
                            release_date: parsed.year.map(|y| format!("{}-01-01", y)),
                            overview: None,
                            media_type: if parsed.media_type == crate::utils::smart_tokenizer::MediaType::TvShow { Some("tv".to_string()) } else { Some("movie".to_string()) },
                        }
                    }).collect::<Vec<SearchResult>>()
                }).unwrap_or_default();

                // Filter out non-media files (.ts, .iso, .nfo, .srt, .txt, etc.)
                results.retain(|r| is_media_file(&r.original_filename));
                
                // Deep Sort
                results.sort_by(|a, b| b.score.cmp(&a.score));
                
                // GLOBAL ENRICHMENT: Enrich top 100 results before returning, not just the page slice
                let enrichment_pool_size = results.len().min(100);
                if params.enrich && enrichment_pool_size > 0 {
                    // Optimized Step 1: Collection Lookup for entire franchise (e.g. "Doraemon")
                    let mut collection_lookup: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
                    let collection_url = format!(
                        "https://api.themoviedb.org/3/search/collection?api_key={}&query={}",
                        TMDB_KEY, urlencoding::encode(&params.q)
                    );
                    
                    if let Ok(coll_resp) = client.get(&collection_url).send().await {
                        if let Ok(coll_data) = coll_resp.json::<Value>().await {
                            if let Some(coll_results) = coll_data["results"].as_array() {
                                if let Some(first_collection) = coll_results.first() {
                                    if let Some(collection_id) = first_collection["id"].as_u64() {
                                        let details_url = format!(
                                            "https://api.themoviedb.org/3/collection/{}?api_key={}",
                                            collection_id, TMDB_KEY
                                        );
                                        if let Ok(details_resp) = client.get(&details_url).send().await {
                                            if let Ok(details_data) = details_resp.json::<Value>().await {
                                                if let Some(parts) = details_data["parts"].as_array() {
                                                    for part in parts {
                                                        if let Some(title) = part["title"].as_str() {
                                                            let normalized = normalize_title(title);
                                                            collection_lookup.insert(normalized.clone(), part.clone());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    let mut enrichment_tasks = Vec::new();
                    for idx in 0..enrichment_pool_size {
                        let res = &mut results[idx];
                        let clean_title = extract_core_title(&res.name);
                        let normalized = normalize_title(&clean_title);
                        
                        if let Some(tmdb_match) = collection_lookup.get(&normalized) {
                            apply_tmdb_data(res, tmdb_match);
                        } else {
                            // Parallel individual enrichment for non-collection items
                            let is_series = res.is_series;
                            let year = res.year.clone();
                            let c = client.clone();
                            let pool_idx = idx;
                            let ct = clean_title.clone();
                            
                            enrichment_tasks.push(tokio::spawn(async move {
                                if ct.trim().is_empty() { return (pool_idx, None); }
                                let media_type = if is_series { "tv" } else { "movie" };
                                let mut url = format!(
                                    "https://api.themoviedb.org/3/search/{}?api_key={}&query={}",
                                    media_type, TMDB_KEY, urlencoding::encode(&ct)
                                );
                                if let Some(y) = &year {
                                    url.push_str(&format!("&{}={}", if media_type == "movie" { "primary_release_year" } else { "first_air_date_year" }, y));
                                }
                                
                                if let Ok(resp) = c.get(&url).send().await {
                                    if let Ok(data) = resp.json::<Value>().await {
                                        if let Some(hit) = data["results"].as_array().and_then(|a| a.first()) {
                                            return (pool_idx, Some(hit.clone()));
                                        }
                                    }
                                }
                                
                                // Retry without year
                                if year.is_some() {
                                    let url_no_year = format!("https://api.themoviedb.org/3/search/{}?api_key={}&query={}", media_type, TMDB_KEY, urlencoding::encode(&ct));
                                    if let Ok(resp) = c.get(&url_no_year).send().await {
                                        if let Ok(data) = resp.json::<Value>().await {
                                            if let Some(hit) = data["results"].as_array().and_then(|a| a.first()) {
                                                return (pool_idx, Some(hit.clone()));
                                            }
                                        }
                                    }
                                }
                                (pool_idx, None)
                            }) as tokio::task::JoinHandle<(usize, Option<Value>)>);
                        }
                    }
                    
                    let enriched_data = join_all(enrichment_tasks).await;
                    for task_res in enriched_data {
                        if let Ok((idx, Some(data))) = task_res {
                            apply_tmdb_data(&mut results[idx], &data);
                        }
                    }
                }
                
                // Final Pagination response behavior (V3 parity: return up to 100 for client-side handle)
                let total = results.len();
                let limit = params.limit.max(1).min(100);
                let page = params.page.max(1);
                let total_pages = (total + limit - 1) / limit;
                
                // Note: The frontend Search/+page.svelte currently ignores backend pagination and uses all results 
                // for its own itemsPerPage logic. We return the full enriched set for parity with expectation.
                Json(SearchResponse {
                    results,
                    page,
                    limit,
                    total,
                    total_pages,
                    has_more: page < total_pages,
                    version: backend_version,
                })
            } else {
                 Json(SearchResponse { results: vec![], page: 1, limit: 20, total: 0, total_pages: 0, has_more: false, version: backend_version })
            }
        },
        Err(_) => Json(SearchResponse { results: vec![], page: 1, limit: 20, total: 0, total_pages: 0, has_more: false, version: "v3.3-error".to_string() }),
    }
}

fn normalize_title(s: &str) -> String {
    s.to_lowercase()
     .chars()
     .filter(|c| c.is_alphanumeric() || c.is_whitespace())
     .collect::<String>()
     .split_whitespace()
     .collect::<Vec<_>>()
     .join(" ")
}

fn apply_tmdb_data(res: &mut SearchResult, data: &Value) {
    res.tmdb_id = data["id"].as_u64().map(|id| id as u32);
    res.tmdb_title = data["title"].as_str().or_else(|| data["name"].as_str()).map(|s| s.to_string());
    res.poster_path = data["poster_path"].as_str().map(|s| s.to_string());
    if let Some(path) = &res.poster_path {
        res.poster_url = Some(format!("https://image.tmdb.org/t/p/w500{}", path));
    }
    res.backdrop_path = data["backdrop_path"].as_str().map(|s| s.to_string());
    if let Some(path) = &res.backdrop_path {
        res.backdrop_url = Some(format!("https://image.tmdb.org/t/p/original{}", path));
    }
    res.vote_average = data["vote_average"].as_f64().map(|v| v as f32);
    res.overview = data["overview"].as_str().map(|s| s.to_string());
    
    // Media Type
    if let Some(media_type) = data["media_type"].as_str() {
        res.media_type = Some(media_type.to_string());
    } else {
        // Fallback for items that don't have media_type (e.g. from type specific search)
        // If it looks like a movie has release_date
        if data.get("release_date").is_some() {
             res.media_type = Some("movie".to_string());
        } else if data.get("first_air_date").is_some() {
             res.media_type = Some("tv".to_string());
        }
    }
    
    // Release Date
    if let Some(date) = data.get("release_date").and_then(|v| v.as_str()) {
        res.release_date = Some(date.to_string());
        res.year = date.split('-').next().map(|s| s.to_string());
    } else if let Some(date) = data.get("first_air_date").and_then(|v| v.as_str()) {
        res.release_date = Some(date.to_string());
        res.year = date.split('-').next().map(|s| s.to_string());
    }
}
