use axum::{
    routing::post,
    extract::{State, Json},
    response::IntoResponse,
    Router,
};
use serde::{Deserialize, Serialize};
use crate::utils::title_matcher::{
    calculate_unified_similarity, group_by_quality, SmartSearchResult, QualityGroup, 
    extract_core_title, normalize_vietnamese, is_vietnamese_title, detect_badges
};
use crate::utils::unified_scorer::{calculate_match_score, is_valid_match};
use crate::utils::smart_tokenizer::smart_parse;
use tracing::{info, warn};
use regex::Regex;
use std::sync::Arc;
use crate::AppState;
use reqwest::Client;
use serde_json::Value;
use std::sync::OnceLock;

use crate::constants::TMDB_API_KEY;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartSearchRequest {
    pub title: String,
    pub year: Option<Value>,
    pub r#type: String,
    #[serde(alias = "tmdb_id")]
    pub tmdb_id: Option<Value>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Clone)]
pub struct SmartSearchResponse {
    pub query: String,
    pub total_found: usize,
    pub r#type: String,
    pub groups: Option<Vec<QualityGroup>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seasons: Option<Vec<SeasonGroup>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SeasonGroup {
    pub season: u32,
    pub episodes_grouped: Vec<EpisodeGroup>,
}

#[derive(Debug, Serialize, Clone)]
pub struct EpisodeGroup {
    pub episode_number: u32,
    pub name: String,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub still_path: Option<String>,
    pub files: Vec<SmartSearchResult>,
}

#[allow(dead_code)]
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(smart_search))
}

pub async fn smart_search(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SmartSearchRequest>,
) -> impl IntoResponse {
    let media_type = req.r#type.clone();
    
    if media_type == "tv" {
        handle_tv_search(state, req).await
    } else {
        handle_movie_search(state, req).await
    }
}

async fn handle_movie_search(
    state: Arc<AppState>,
    req: SmartSearchRequest,
) -> axum::response::Response {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_else(|_| Client::new());

    let title = req.title.clone();
    let year_str = match req.year {
        Some(Value::String(ref s)) => Some(s.clone()),
        Some(Value::Number(ref n)) => Some(n.to_string()),
        _ => None,
    };
    
    let core_title = extract_core_title(&title);
    let _query_keyword = if let Some(ref y) = year_str {
        format!("{} {}", core_title, y)
    } else {
        core_title.clone()
    };
    let cache_key = format!("movie|{}|{:?}|{:?}|{:?}|{:?}", req.title, req.year, req.tmdb_id, req.season, req.episode);
    if let Some(cached) = state.search_cache.get(&cache_key).await {
        info!("Search Cache HIT: {}", cache_key);
        return Json::<SmartSearchResponse>(cached).into_response();
    }

    let start_time = std::time::Instant::now();
    
    // 1. EXECUTE TMDB ENRICHMENT AND CONCURRENT FSHARE KEYWORD SEARCH
    let tmdb_id_v = req.tmdb_id.clone();
    let tmdb_id_str = match tmdb_id_v {
        Some(Value::String(s)) => Some(s),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    };
    
    let c1 = client.clone();
    let id1 = tmdb_id_str.clone();
    let state_clone = state.clone();
    let tmdb_handle = tokio::spawn(async move {
        let mut iso_titles = Vec::new();
        let mut poster = None;
        let mut official = None;
        let mut collections = Vec::new(); // (title, year, id, poster)

        if let Some(tmdb_id) = id1.clone() {
            if let Some(cached) = state_clone.tmdb_cache.get(&tmdb_id).await {
                return Some((cached.0, cached.1, cached.3, cached.2));
            }
            // V2 HARD MODE: Movie alternative_titles endpoint.
            let url = format!("https://api.themoviedb.org/3/movie/{}/alternative_titles?api_key={}", tmdb_id, TMDB_API_KEY);
            if let Ok(resp) = c1.get(&url).send().await {
                if let Ok(data) = resp.json::<Value>().await {
                    if let Some(titles) = data["titles"].as_array() {
                        for a in titles {
                            if let Some(at) = a["title"].as_str() {
                                let iso = a["iso_3166_1"].as_str().unwrap_or("");
                                iso_titles.push((at.to_string(), iso.to_string()));
                            }
                        }
                    }
                }
            }

            // Official details
            let details_url = format!("https://api.themoviedb.org/3/movie/{}?api_key={}&append_to_response=belongs_to_collection", tmdb_id, TMDB_API_KEY);
            if let Ok(resp) = c1.get(&details_url).send().await {
                if let Ok(data) = resp.json::<Value>().await {
                    official = data["title"].as_str().map(|s| s.to_string());
                    poster = data["poster_path"].as_str().map(|s| s.to_string());
                    
                    if let Some(coll_data) = data["belongs_to_collection"].as_object() {
                        if let Some(coll_id) = coll_data["id"].as_u64() {
                            let coll_url = format!("https://api.themoviedb.org/3/collection/{}?api_key={}", coll_id, TMDB_API_KEY);
                            if let Ok(c_resp) = c1.get(&coll_url).send().await {
                                if let Ok(c_data) = c_resp.json::<Value>().await {
                                    if let Some(parts) = c_data["parts"].as_array() {
                                        for p in parts {
                                            let t = p["title"].as_str().unwrap_or("").to_string();
                                            let y = p["release_date"].as_str().unwrap_or("").split('-').next().unwrap_or("").to_string();
                                            let pid = p["id"].as_u64().unwrap_or(0);
                                            let post = p["poster_path"].as_str().map(|s| s.to_string());
                                            collections.push((t, y, pid, post));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if official.is_some() {
            let mut vn_titles = Vec::new();
            let mut other_titles = Vec::new();
            let mut seen_t = std::collections::HashSet::new();

            for (title, iso) in iso_titles {
                if seen_t.insert(title.clone()) {
                    if iso == "VN" {
                        vn_titles.push(title);
                    } else {
                        other_titles.push(title);
                    }
                }
            }

            let mut all_aliases = vn_titles;
            all_aliases.extend(other_titles);

            // Cache metadata
            if let Some(tmdb_id_val) = id1 {
                state_clone.tmdb_cache.insert(tmdb_id_val, (official.clone(), all_aliases.clone(), poster.clone(), collections.clone())).await;
            }
            return Some((official, all_aliases, collections, poster));
        }
        None
    });

    let tmdb_res = tmdb_handle.await.unwrap();
    type Enrichment = (Option<String>, Vec<String>, Vec<(String, String, u64, Option<String>)>, Option<String>);
    let enrichment: Enrichment = tmdb_res.unwrap_or((None, Vec::new(), Vec::new(), None));
    let (tmdb_official, aliases, collections, base_poster) = enrichment;
    let official_name = tmdb_official.clone().unwrap_or_else(|| title.clone());
    let base_tmdb_id = tmdb_id_str.and_then(|s| s.parse::<u64>().ok());

    // 2. PRIMARY SEARCH EXECUTION (V3: Multiple variations for maximum coverage)
    let c_primary = client.clone();
    let title_clean = core_title.clone();
    let year_val = year_str.clone();
    let vn_alias_opt = aliases.iter().find(|a| is_vietnamese_title(a)).cloned();
    
    let fshare_handle = tokio::spawn(async move {
        let mut all_res = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut queries = Vec::new();
        
        // Variation 1: Title + Year (Precise)
        if let Some(ref y) = year_val {
            queries.push(format!("{} {}", title_clean, y));
        }
        
        // Variation 2: Title Only (Loose - handles year mismatches like 2024/2025)
        queries.push(title_clean.clone());
        
        // Variation 3: Vietnamese Alias (if any)
        if let Some(ref vn) = vn_alias_opt {
            queries.push(vn.clone());
            let norm = normalize_vietnamese(vn);
            if norm != vn.to_lowercase() {
                queries.push(norm);
            }
        }
        
        // Variation 4: Non-accented Primary Title (if Vietnamese)
        if is_vietnamese_title(&title_clean) {
            let norm = normalize_vietnamese(&title_clean);
            if norm != title_clean.to_lowercase() {
                queries.push(norm.clone());
                if let Some(ref y) = year_val {
                    queries.push(format!("{} {}", norm, y));
                }
            }
        }

        for q in queries {
            let res = execute_fshare_search(&c_primary, &q, 60).await;
            for r in res {
                let pure_fcode = r.fcode.split('?').next().unwrap_or(&r.fcode).to_string();
                if seen.insert(pure_fcode) {
                    all_res.push(r);
                }
            }
            if all_res.len() >= 120 { break; } // Cap at reasonable limit
        }
        all_res
    });

    let fshare_res = fshare_handle.await.unwrap();
    let mut results_pool = fshare_res;
    
    // 3. FINAL DEDUPLICATE AND LIMIT TO 100 (matching V2)
    let mut final_seen = std::collections::HashSet::new();
    results_pool.retain(|f| {
        let pure_fcode = f.fcode.split('?').next().unwrap_or(&f.fcode);
        final_seen.insert(pure_fcode.to_string())
    });
    let target_results = results_pool.into_iter().take(100).collect::<Vec<_>>();

    // 4. PARSE AND MAP TOP 100
    let mut valid_results = Vec::new();
    static RE_YEAR: OnceLock<Regex> = OnceLock::new();
    let _year_re = RE_YEAR.get_or_init(|| Regex::new(r"\b(19|20)\d{2}\b").unwrap());

    for r in target_results {
        let parsed = smart_parse(&r.name);

        let mut final_id = base_tmdb_id;
        let mut final_poster = base_poster.clone();
        
        // Use unified scorer for primary match (movies: 70% title + 20% year)
        let primary_score = calculate_match_score(
            &official_name,
            &r.name,
            parsed.year,
            year_str.as_ref().and_then(|y| y.parse::<u32>().ok()),
            &aliases,
            false, // is_tv_series = false for movies
        );
        
        let mut best_score = primary_score;
        
        // Map collection items with unified scorer
        for (ct, cy, cid, cp) in &collections {
            let collection_year = cy.parse::<u32>().ok();
            let collection_score = calculate_match_score(
                ct,
                &r.name,
                parsed.year,
                collection_year,
                &[], // No aliases for collection items
                false,
            );
            
            // Collection item wins if it scores higher
            if collection_score > best_score {
                best_score = collection_score;
                final_id = Some(*cid);
                final_poster = cp.clone();
            }
        }

        // FILTER: Use unified scorer's validation
        if !is_valid_match(best_score, false) {
            continue;
        }

        // PHASE 3: Detect badges
        let (vietdub, vietsub, hdr, dolby_vision) = detect_badges(&r.name);
        
        valid_results.push(SmartSearchResult {
            name: r.name.clone(),
            url: r.url,
            size: r.size,
            score: r.score,
            quality_name: parsed.quality_name(),
            quality_score: parsed.quality_score(),
            custom_format_score: parsed.custom_format_score(),
            total_score: parsed.total_score(),
            normalized_score: parsed.normalized_score(),
            match_type: if best_score >= 0.90 { 
                "exact".to_string() 
            } else if best_score >= 0.75 { 
                "high_confidence".to_string() 
            } else { 
                "valid".to_string() 
            },
            quality_attrs: crate::utils::parser::QualityAttributes {
                resolution: parsed.resolution.clone(),
                source: parsed.source.clone(),
                video_codec: parsed.video_codec.clone(),
                audio_codec: parsed.audio_codec.clone(),
                hdr: parsed.hdr,
                dolby_vision: parsed.dolby_vision,
                bit_depth: 8,
                viet_sub: parsed.viet_sub,
                viet_dub: parsed.viet_dub,
                is_tv: parsed.media_type == crate::utils::smart_tokenizer::MediaType::TvShow,
                is_movie: parsed.media_type == crate::utils::smart_tokenizer::MediaType::Movie,
                is_hd: parsed.resolution.as_ref().map(|r| r.contains("720") || r.contains("1080") || r.contains("2160") || r.contains("4K")).unwrap_or(false),
            },
            tmdb_id: final_id,
            poster_path: final_poster,
            vietdub,
            vietsub,
            hdr,
            dolby_vision,
        });
    }

    // PHASE 2.1: Sort by match_type first (alias > exact > fuzzy), then by quality
    valid_results.sort_by(|a, b| {
        let a_priority = match a.match_type.as_str() {
            "alias" => 0,
            "exact" => 1,
            "fuzzy" | "keyword_overlap" => 2,
            _ => 3,
        };
        let b_priority = match b.match_type.as_str() {
            "alias" => 0,
            "exact" => 1,
            "fuzzy" | "keyword_overlap" => 2,
            _ => 3,
        };
        
        match a_priority.cmp(&b_priority) {
            std::cmp::Ordering::Equal => b.total_score.cmp(&a.total_score),
            other => other,
        }
    });

    let groups = group_by_quality(valid_results);
    info!("Total Optimized Smart Search took: {:?}", start_time.elapsed());

    let response = SmartSearchResponse {
        query: title,
        total_found: groups.iter().map(|g| g.count).sum(),
        r#type: "movie".to_string(),
        groups: Some(groups),
        seasons: None,
    };
    
    state.search_cache.insert(cache_key, response.clone()).await;
    
    info!("Total Optimized Movie Smart Search took: {:?}", start_time.elapsed());
    Json(response).into_response()
}

async fn handle_tv_search(
    state: Arc<AppState>,
    req: SmartSearchRequest,
) -> axum::response::Response {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .unwrap_or_else(|_| Client::new());

    let title = req.title.clone();
    let season = req.season;
    let episode = req.episode;
    let year_str = match req.year {
        Some(Value::String(ref s)) => Some(s.clone()),
        Some(Value::Number(ref n)) => Some(n.to_string()),
        _ => None,
    };
    
    let core_title = extract_core_title(&title);
    let query_keyword = if let Some(ref y) = year_str {
        format!("{} {}", core_title, y)
    } else {
        core_title
    };
    let cache_key = format!("tv|{}|{:?}|{:?}|{:?}|{:?}", req.title, req.year, req.tmdb_id, req.season, req.episode);
    if let Some(cached) = state.search_cache.get(&cache_key).await {
        info!("Search Cache HIT: {}", cache_key);
        return Json::<SmartSearchResponse>(cached).into_response();
    }

    let start_time = std::time::Instant::now();
    
    // 1. EXECUTE TMDB ENRICHMENT AND KEYWORD SEARCH AT ONCE
    let tmdb_id_str = match req.tmdb_id {
        Some(Value::String(ref s)) => Some(s.to_string()),
        Some(Value::Number(ref n)) => Some(n.to_string()),
        _ => None,
    };
    
    info!("TV Smart Search Request: title='{}', tmdbId={:?}, year={:?}", title, tmdb_id_str, year_str);
    
    let c1 = client.clone();
    let id1 = tmdb_id_str.clone();
    let state_clone = state.clone();
    let tmdb_handle = tokio::spawn(async move {
        let mut iso_titles = Vec::new();
        let mut poster = None;
        let mut official = None;

        if let Some(tmdb_id) = id1.clone() {
            if let Some(cached) = state_clone.tmdb_cache.get(&tmdb_id).await {
                return Some((cached.0, cached.1, cached.2));
            }
            // V2 HARD MODE: TV alternative_titles endpoint.
            let url = format!("https://api.themoviedb.org/3/tv/{}/alternative_titles?api_key={}", tmdb_id, TMDB_API_KEY);
            info!("Enriching TV metadata from TMDB: {}", url);
            if let Ok(resp) = c1.get(&url).send().await {
                if let Ok(data) = resp.json::<Value>().await {
                    if let Some(results) = data["results"].as_array() {
                        for a in results {
                            if let Some(at) = a["title"].as_str().or(a["name"].as_str()) {
                                let iso = a["iso_3166_1"].as_str().unwrap_or("");
                                iso_titles.push((at.to_string(), iso.to_string()));
                            }
                        }
                    }
                }
            }
            
            // Details for name/poster
            let details_url = format!("https://api.themoviedb.org/3/tv/{}?api_key={}", tmdb_id, TMDB_API_KEY);
            if let Ok(resp) = c1.get(&details_url).send().await {
                if let Ok(data) = resp.json::<Value>().await {
                    official = data["name"].as_str().map(|s| s.to_string());
                    poster = data["poster_path"].as_str().map(|s| s.to_string());
                }
            }

            // V3 EXTENSION: Also fetch translations for better VN title coverage (e.g. Bộ Bộ Kinh Tâm)
            let trans_url = format!("https://api.themoviedb.org/3/tv/{}/translations?api_key={}", tmdb_id, TMDB_API_KEY);
            if let Ok(resp) = c1.get(&trans_url).send().await {
                if let Ok(data) = resp.json::<Value>().await {
                    if let Some(translations) = data["translations"].as_array() {
                        for t in translations {
                            let iso = t["iso_3166_1"].as_str().unwrap_or("");
                            if let Some(data) = t["data"].as_object() {
                                if let Some(t_name) = data.get("name").and_then(|v| v.as_str()) {
                                    if !t_name.is_empty() {
                                        iso_titles.push((t_name.to_string(), iso.to_string()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if official.is_some() {
            let mut vn_titles = Vec::new();
            let mut other_titles = Vec::new();
            let mut seen_t = std::collections::HashSet::new();

            for (title, iso) in iso_titles {
                if seen_t.insert(title.clone()) {
                    if iso == "VN" {
                        vn_titles.push(title);
                    } else {
                        other_titles.push(title);
                    }
                }
            }

            let mut all_aliases = vn_titles;
            all_aliases.extend(other_titles);
            info!("TMDB Enrichment: Official='{}', Aliases={:?}", official.as_deref().unwrap_or("None"), all_aliases);

            // Cache metadata (TV has empty collections)
            if let Some(tmdb_id_val) = id1 {
                state_clone.tmdb_cache.insert(tmdb_id_val, (official.clone(), all_aliases.clone(), poster.clone(), Vec::new())).await;
            }
            return Some((official, all_aliases, poster));
        }
        None
    });

    // 2. PRIMARY SEARCH EXECUTION (V2 Hard Mode: Single query - matching V2 generic search)
    let query_season = if let (Some(s), Some(e)) = (season, episode) {
        format!("{} S{:02}E{:02}", query_keyword, s, e)
    } else if let Some(s) = season {
        format!("{} Season {}", query_keyword, s)
    } else {
        query_keyword.clone()
    };
    
    // V3 EXTRA: Add Sxx variant for better coverage (prevents buried Sxx in loose search)
    let mut primary_queries = vec![query_season.clone()];
    if season.is_some() && episode.is_none() {
        if let Some(s) = season {
            primary_queries.push(format!("{} S{:02}", query_keyword, s));
        }
    }

    let c_primary = client.clone();
    let fshare_handle = tokio::spawn(async move {
        let mut all_res = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for q in primary_queries {
            let res = execute_fshare_search(&c_primary, &q, 100).await;
            for r in res {
                // Deduplicate by normalized fcode (strip des=)
                let pure_fcode = r.fcode.split('?').next().unwrap_or("").to_string();
                if seen.insert(pure_fcode) {
                    all_res.push(r);
                }
            }
        }
        all_res
    });

    let (tmdb_res, fshare_res) = tokio::join!(tmdb_handle, fshare_handle);
    type Enrichment = (Option<String>, Vec<String>, Option<String>);
    let enrichment: Enrichment = tmdb_res.unwrap().unwrap_or((None, Vec::new(), None));
    let (tmdb_official, aliases, base_poster) = enrichment;
    let official_name = tmdb_official.clone().unwrap_or_else(|| title.clone());
    let base_tmdb_id = tmdb_id_str.and_then(|s| s.parse::<u64>().ok());

    let mut results_pool: Vec<RawFshareResult> = fshare_res.unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    for r in &results_pool {
        let pure_fcode = r.fcode.split('?').next().unwrap_or(&r.fcode);
        seen.insert(pure_fcode.to_string());
    }

    // 2.5. SECONDARY ALIAS SEARCH (V2: First VN alias only)
    if let Some(vn_alias) = aliases.iter().find(|a| is_vietnamese_title(a)) {
        if vn_alias.to_lowercase() != official_name.to_lowercase() {
            info!("Performing secondary TV search with Vietnamese alias: '{}'", vn_alias);
            
            // V2 HARD MODE: Generic alias search (no S/E)
            let mut vn_results: Vec<RawFshareResult> = execute_fshare_search(&client, &vn_alias, 100).await;
            
            let vn_normalized = normalize_vietnamese(vn_alias);
            if vn_normalized != vn_alias.to_lowercase() {
                let vn_norm_results: Vec<RawFshareResult> = execute_fshare_search(&client, &vn_normalized, 100).await;
                vn_results.extend(vn_norm_results);
            }
            
            for vr in vn_results {
                let pure_fcode = vr.fcode.split('?').next().unwrap_or(&vr.fcode);
                if seen.insert(pure_fcode.to_string()) {
                    results_pool.push(vr);
                }
            }
        }
    }

    // 3. FINAL DEDUPLICATE AND LIMIT TO 100 (matching V2)
    let mut final_seen = std::collections::HashSet::new();
    results_pool.retain(|f| {
        let pure_fcode = f.fcode.split('?').next().unwrap_or(&f.fcode);
        final_seen.insert(pure_fcode.to_string())
    });
    
    // 3.5 SNOWBALL LOGIC (TV Only) - matching V2's deep-dive search
    info!("Snowball Check: results_pool={}, aliases={}", results_pool.len(), aliases.len());
    if !results_pool.is_empty() && !aliases.is_empty() {
        // Reuse the final_seen set for snowball results
        let mut seen = final_seen; 
        // Step 1: Group files by pattern
            let mut pattern_groups: std::collections::HashMap<String, (std::collections::HashSet<u32>, String, String)> = std::collections::HashMap::new();
            
            for r in &results_pool {
                let name = &r.name;
                
                // Try different patterns to extract episode number
                static RE_S_E: OnceLock<Regex> = OnceLock::new();
                static RE_TAP: OnceLock<Regex> = OnceLock::new();
                static RE_LEADING: OnceLock<Regex> = OnceLock::new();
                static RE_TRAILING: OnceLock<Regex> = OnceLock::new();

                let re_s_e = RE_S_E.get_or_init(|| Regex::new(r"^(.+?)[._\s]S(\d{1,2})[Ee](\d{1,3})(.*)$").unwrap());
                let re_tap = RE_TAP.get_or_init(|| Regex::new(r"^(.+?)(?:[\s_.-]?(?:Tập|[Tt]ap|[Ee]p?))[\s_.-]*(\d{1,4})(.*)$").unwrap());
                let re_leading = RE_LEADING.get_or_init(|| Regex::new(r"^(\d{1,3})([_\s.].+)$").unwrap());
                let re_trailing = RE_TRAILING.get_or_init(|| Regex::new(r"^(.+?)[_\s.-](\d{1,3})(\.(?:mkv|mp4))$").unwrap());
                
                let (ep, template, base_search) = if let Some(caps) = re_s_e.captures(name) {
                    let ep = caps.get(3).and_then(|m| m.as_str().parse::<u32>().ok()).unwrap_or(0);
                    let season = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                    let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    (ep, format!("{} S{}E{{ep}}", prefix, season), format!("{} S{}", prefix, season))
                } else if let Some(caps) = re_tap.captures(name) {
                    let ep = caps.get(2).and_then(|m| m.as_str().parse::<u32>().ok()).unwrap_or(0);
                    let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    (ep, format!("{} Tap {{ep}}", prefix), prefix.to_string())
                } else if let Some(caps) = re_leading.captures(name) {
                    let ep = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok()).unwrap_or(0);
                    let suffix = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                    let base = suffix.trim_start_matches(|c: char| c == '_' || c == '.' || c == ' ').chars().take(30).collect::<String>();
                    (ep, format!("{{ep}}{}", suffix), base)
                } else if let Some(caps) = re_trailing.captures(name) {
                    let ep_str = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                    // Skip if it looks like a resolution (e.g., "1080")
                    if ep_str.len() >= 3 && ep_str.len() <= 4 {
                        continue;
                    }
                    let ep = ep_str.parse::<u32>().ok().unwrap_or(0);
                    let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    let ext = caps.get(3).map(|m| m.as_str()).unwrap_or("");
                    let base = prefix.trim().chars().take(40).collect::<String>();
                    (ep, format!("{} {{ep}}{}", prefix, ext), base)
                } else {
                    continue;
                };
                
                if ep >= 1 && ep <= 1000 {
                    pattern_groups.entry(template.clone())
                        .or_insert_with(|| (std::collections::HashSet::new(), name.clone(), base_search))
                        .0.insert(ep);
                }
            }
            
            // Step 2: Deep Dive - find missing episodes
            let mut sorted_patterns: Vec<_> = pattern_groups.iter().collect();
            sorted_patterns.sort_by(|a, b| b.1.0.len().cmp(&a.1.0.len()));
            
            // INCREASED: Take top 5 patterns to ensure we don't miss our show due to noise
            for (template, (found_eps, sample_name, base_pattern)) in sorted_patterns.iter().take(5) {
                info!("Snowball: Evaluating pattern group '{}' (count: {})", template, found_eps.len());
                
                // FILTER: Only deep-dive if the pattern likely belongs to the show we're searching for
                let sim = calculate_unified_similarity(&official_name, sample_name, &aliases);
                if !sim.is_valid {
                    info!("Snowball: Skipping group '{}' - matched_title={}, match_type={}", template, sim.match_type, sim.match_type);
                    continue;
                }

                if found_eps.len() < 5 {
                    info!("Snowball: Group '{}' too small (count: {})", template, found_eps.len());
                    continue;
                }
                
                let max_ep = *found_eps.iter().max().unwrap_or(&0);
                let check_limit = max_ep.max(35);
                let missing_eps: Vec<u32> = (1..=check_limit).filter(|ep| !found_eps.contains(ep)).collect();
                
                if missing_eps.is_empty() {
                    info!("Snowball: Group '{}' has no missing episodes", template);
                    continue;
                }
                
                info!("Deep-dive (Snowball): '{}' missing {} episodes (max detected: {})", &template[..template.len().min(50)], missing_eps.len(), max_ep);
                
                let is_large_series = missing_eps.len() > 50 || max_ep > 100;
                let mut snowball_queries = Vec::new();
                
                if is_large_series {
                    for i in 0..10 {
                        snowball_queries.push(format!("{} {}", base_pattern, i));
                    }
                } else {
                    snowball_queries.push(base_pattern.clone());
                    // Try to extract the title part by splitting at " S" or " Season"
                    let lower_base = base_pattern.to_lowercase();
                    if let Some(idx) = lower_base.find(" s") {
                        let title_part = &base_pattern[..idx];
                        if !title_part.trim().is_empty() {
                            snowball_queries.push(title_part.trim().to_string());
                        }
                    } else if let Some(idx) = lower_base.find(" season") {
                        let title_part = &base_pattern[..idx];
                        if !title_part.trim().is_empty() {
                            snowball_queries.push(title_part.trim().to_string());
                        }
                    }
                }
                
                for q in snowball_queries {
                    info!("Snowball Search Execute: '{}'", q);
                    let s_results: Vec<RawFshareResult> = execute_fshare_search(&client, &q, 100).await;
                    info!("Snowball Result from '{}': {} files", q, s_results.len());
                    for sr in s_results {
                        let pure_fcode = sr.fcode.split('?').next().unwrap_or("").to_string();
                        if seen.insert(pure_fcode) {
                            results_pool.push(sr);
                        }
                    }
                }
            }
    }
    
    let target_results = results_pool; // Process ALL results, not just first 100


    // 4. PARSE AND EVALUATE
    let mut valid_results = Vec::new();
    
    info!("Total files to evaluate: {}", target_results.len());
    let vn_files: Vec<_> = target_results.iter().filter(|r| r.name.contains("Bộ Bộ")).map(|r| &r.name).collect();
    info!("Vietnamese files in results: {} - {:?}", vn_files.len(), vn_files);

    for r in target_results {
        let parsed = smart_parse(&r.name);

        // Version Filtering: Strict Year Match for TV originals vs remakes
        if let Some(ref y_req) = year_str {
            if let Some(file_year) = parsed.year {
                if let Ok(y_val) = y_req.parse::<u32>() {
                    if (file_year as i32 - y_val as i32).abs() > 1 {
                        info!("REJECTED Version: '{}' (Year mismatch: {} != {})", r.name, file_year, y_req);
                        continue;
                    }
                }
            }
        }

        // Strict S/E validation for TV
        if let (Some(s_req), Some(e_req)) = (season, episode) {
            // If parser found season/episode, check strict match
            if let (Some(s_file), Some(e_file)) = (parsed.season, parsed.episode) {
                if s_file != s_req || e_file != e_req {
                    info!("FILTERED: '{}' - S/E mismatch (file: S{:02}E{:02}, req: S{:02}E{:02})", r.name, s_file, e_file, s_req, e_req);
                    continue;
                }
            } else {
                // If parser failed to find S/E, skip (strict mode)
                info!("FILTERED: '{}' - No S/E found by parser", r.name);
                continue;
            }
        } else if let Some(s_req) = season {
            // Season pack / Season search
             if let Some(s_file) = parsed.season {
                 if s_file != s_req { continue; }
             }
        }
        
        
        let sim = calculate_unified_similarity(&official_name, &r.name, &aliases);
        
        // Debug logging for similarity check
        let _truncated_name: String = r.name.chars().take(80).collect();
        // info!("SIM: '{}' vs '{}...' => score:{:.2} type:{} valid:{}", 
        //       official_name, truncated_name, sim.score, sim.match_type, sim.is_valid);
        
        // Filter out invalid matches (like V2 does - Filter 1)
        if !sim.is_valid {
            continue;
        }
        
        // V2's Filter 2: When TMDB ID is available, only accept high-quality matches
        // Valid types: 'alias', 'exact', 'all_keywords'
        // Reject: 'missing_keywords', 'fuzzy', 'franchise_conflict', 'keyword_overlap'
        if base_tmdb_id.is_some() {
            let valid_match_types = ["alias", "exact", "all_keywords"];
            if !valid_match_types.contains(&sim.match_type.as_str()) {
                continue;
            }
        }
        
        // PHASE 3: Detect badges
        let (vietdub, vietsub, hdr, dolby_vision) = detect_badges(&r.name);
        
        valid_results.push(SmartSearchResult {
            name: r.name.clone(),
            url: r.url,
            size: r.size,
            score: r.score,
            quality_name: parsed.quality_name(),
            quality_score: parsed.quality_score(),
            custom_format_score: parsed.custom_format_score(),
            total_score: parsed.total_score(),
            normalized_score: parsed.normalized_score(),
            match_type: if sim.is_valid { sim.match_type } else { "tv_match".to_string() },
            quality_attrs: crate::utils::parser::QualityAttributes {
                resolution: parsed.resolution.clone(),
                source: parsed.source.clone(),
                video_codec: parsed.video_codec.clone(),
                audio_codec: parsed.audio_codec.clone(),
                hdr: parsed.hdr,
                dolby_vision: parsed.dolby_vision,
                bit_depth: 8,
                viet_sub: parsed.viet_sub,
                viet_dub: parsed.viet_dub,
                is_tv: parsed.media_type == crate::utils::smart_tokenizer::MediaType::TvShow,
                is_movie: parsed.media_type == crate::utils::smart_tokenizer::MediaType::Movie,
                is_hd: parsed.resolution.as_ref().map(|r| r.contains("720") || r.contains("1080") || r.contains("2160") || r.contains("4K")).unwrap_or(false),
            },
            tmdb_id: base_tmdb_id,
            poster_path: base_poster.clone(),
            vietdub,
            vietsub,
            hdr,
            dolby_vision,
        });
    }

    info!("Valid results count: {}", valid_results.len());
    let vn_valid: Vec<_> = valid_results.iter().filter(|r| r.name.contains("Bộ Bộ")).map(|r| &r.name).collect();
    info!("Vietnamese files in valid_results: {} - {:?}", vn_valid.len(), vn_valid);

    // PHASE 2.1: Sort by match_type first (alias > exact > fuzzy), then by quality
    // This ensures Vietnamese files (alias matches) appear first
    valid_results.sort_by(|a, b| {
        // Prioritize alias matches (Vietnamese files)
        let a_priority = match a.match_type.as_str() {
            "alias" => 0,      // Highest priority
            "exact" => 1,
            "fuzzy" | "keyword_overlap" | "tv_match" => 2,
            _ => 3,
        };
        let b_priority = match b.match_type.as_str() {
            "alias" => 0,
            "exact" => 1,
            "fuzzy" | "keyword_overlap" | "tv_match" => 2,
            _ => 3,
        };
        
        // First compare by match_type priority
        match a_priority.cmp(&b_priority) {
            std::cmp::Ordering::Equal => {
                // If same match_type, sort by quality score (higher first)
                b.total_score.cmp(&a.total_score)
            },
            other => other,
        }
    });

    // Grouping for TV: Seasons -> Episodes -> Files
    let mut seasons_map: std::collections::HashMap<u32, std::collections::HashMap<u32, Vec<SmartSearchResult>>> = std::collections::HashMap::new();
    
    for res in valid_results {
        let parsed = smart_parse(&res.name);
        let s = parsed.season.or(season).unwrap_or(1); 
        let e = parsed.episode.or(episode).unwrap_or(0);
        
        seasons_map.entry(s).or_default()
            .entry(e).or_default()
            .push(res);
    }


    // PHASE 4: Fetch Episode Metadata from TMDB
    let mut episode_metadata: std::collections::HashMap<(u32, u32), (String, String, String, String)> = std::collections::HashMap::new();
    
    if let Some(tmdb_id) = base_tmdb_id {
        let seasons_to_fetch: Vec<u32> = seasons_map.keys().cloned().collect();
        let mut fetch_tasks = Vec::new();
        
        for s_num in seasons_to_fetch {
            let c = client.clone();
            let tid = tmdb_id;
            fetch_tasks.push(tokio::spawn(async move {
                let url = format!("https://api.themoviedb.org/3/tv/{}/season/{}?api_key={}", tid, s_num, TMDB_API_KEY);
                if let Ok(resp) = c.get(&url).send().await {
                    if let Ok(data) = resp.json::<Value>().await {
                        return Some((s_num, data));
                    }
                }
                None
            }));
        }
        
        let results = futures_util::future::join_all(fetch_tasks).await;
        for res in results.into_iter().flatten().flatten() {
            let (s_num, data) = res;
            if let Some(episodes) = data["episodes"].as_array() {
                for ep in episodes {
                    if let Some(e_num) = ep["episode_number"].as_u64() {
                        let name = ep["name"].as_str().unwrap_or("").to_string();
                        let overview = ep["overview"].as_str().unwrap_or("").to_string();
                        let air_date = ep["air_date"].as_str().unwrap_or("").to_string();
                        let still_path = ep["still_path"].as_str().unwrap_or("").to_string();
                        
                        episode_metadata.insert((s_num, e_num as u32), (name, overview, air_date, still_path));
                    }
                }
            }
        }
    }

    let mut seasons = Vec::new();
    for (s_num, eps_map) in seasons_map {
        let mut episodes_grouped = Vec::new();
        for (e_num, mut files) in eps_map {
            // Sort files by quality score descending
            files.sort_by(|a, b| b.total_score.cmp(&a.total_score));
            
            let meta = episode_metadata.get(&(s_num, e_num));
            
            episodes_grouped.push(EpisodeGroup {
                episode_number: e_num,
                name: meta.map(|m| m.0.clone()).unwrap_or_else(|| format!("Episode {}", e_num)),
                overview: meta.map(|m| if m.1.is_empty() { None } else { Some(m.1.clone()) }).flatten(),
                air_date: meta.map(|m| if m.2.is_empty() { None } else { Some(m.2.clone()) }).flatten(),
                still_path: meta.map(|m| if m.3.is_empty() { None } else { Some(m.3.clone()) }).flatten(),
                files,
            });
        }
        episodes_grouped.sort_by(|a, b| a.episode_number.cmp(&b.episode_number));
        
        seasons.push(SeasonGroup {
            season: s_num,
            episodes_grouped,
        });
    }
    seasons.sort_by(|a, b| a.season.cmp(&b.season));

    info!("Total Optimized TV Smart Search took: {:?}", start_time.elapsed());

    let response = SmartSearchResponse {
        query: query_season,
        total_found: seasons.iter().map(|s| s.episodes_grouped.iter().map(|e| e.files.len()).sum::<usize>()).sum(),
        r#type: if episode.is_some() { "episode".to_string() } else { "tv".to_string() },
        groups: None, // TV uses seasons structure
        seasons: Some(seasons),
    };
    
    state.search_cache.insert(cache_key, response.clone()).await;

    Json(response).into_response()
}


struct RawFshareResult {
    name: String,
    url: String,
    fcode: String,
    size: u64,
    score: i32,
}

async fn execute_fshare_search(client: &Client, query: &str, limit: usize) -> Vec<RawFshareResult> {
    let url = format!("https://timfshare.com/api/v1/string-query-search?query={}", urlencoding::encode(query));
    let mut results = Vec::new();

    info!("Executing FShare search: '{}'", query);
    let resp = client.post(&url)
        .header("Referer", format!("https://timfshare.com/search?key={}", urlencoding::encode(query)))
        .header("Origin", "https://timfshare.com")
        .header("Content-Length", "0")
        .send()
        .await;

    match resp {
        Ok(r) => {
            if let Ok(data) = r.json::<Value>().await {
                if let Some(arr) = data["data"].as_array() {
                    info!("FShare search '{}' returned {} results", query, arr.len());
                    for item in arr.iter().take(limit) {
                        let name = item["name"].as_str().unwrap_or("Unknown").to_string();
                        let f_url = item["url"].as_str().unwrap_or("").to_string();
                        // Match V2 exactly: take everything after /file/ including parameters
                        let fcode = f_url.split("/file/").last().unwrap_or("").to_string();
                        let size = item["size"].as_u64().unwrap_or(0);
                        
                        results.push(RawFshareResult {
                            name,
                            url: f_url,
                            fcode,
                            size,
                            score: 0,
                        });
                    }
                } else {
                    warn!("FShare search '{}' returned no data array", query);
                }
            } else {
                warn!("FShare search '{}' returned invalid JSON", query);
            }
        },
        Err(e) => {
            warn!("FShare search '{}' request failed: {}", query, e);
        }
    }
    results
}
