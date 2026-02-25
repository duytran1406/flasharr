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
use tracing::info;
use regex::Regex;
use std::sync::Arc;
use crate::AppState;
use crate::TmdbEnrichmentCache;
use crate::services::tmdb_service::MediaEnrichment;
use reqwest::Client;
use serde_json::Value;
use std::sync::OnceLock;


use super::search_pipeline::{SearchPipeline, RawFshareResult};

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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SmartSearchResponse {
    pub query: String,
    pub total_found: usize,
    pub r#type: String,
    pub groups: Option<Vec<QualityGroup>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seasons: Option<Vec<SeasonGroup>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SeasonGroup {
    pub season: u32,
    pub episode_count: u32,  // TMDB official episode count for uncut detection
    pub aired_episode_count: u32,  // Number of episodes that have actually aired
    pub episodes_grouped: Vec<EpisodeGroup>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

pub async fn handle_movie_search(
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

    // ── 1. TMDB ENRICHMENT ──────────────────────────────────────────────────
    let tmdb_id_str = match req.tmdb_id.clone() {
        Some(Value::String(s)) => Some(s),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    };

    let enrichment_cache: TmdbEnrichmentCache = if let Some(ref tmdb_id) = tmdb_id_str {
        // Check in-memory cache first
        if let Some(cached) = state.tmdb_cache.get(tmdb_id).await {
            cached
        } else {
            let tmdb_id_num: i64 = tmdb_id.parse().unwrap_or(0);
            let enrichment = state.tmdb_service.get_movie_enrichment(tmdb_id_num).await;
            let cache_entry = build_movie_enrichment_cache(enrichment, &title);
            state.tmdb_cache.insert(tmdb_id.clone(), cache_entry.clone()).await;
            cache_entry
        }
    } else {
        TmdbEnrichmentCache {
            official: Some(title.clone()),
            original_name: None,
            all_aliases: Vec::new(),
            vn_titles: Vec::new(),
            original_lang_titles: Vec::new(),
            us_titles: Vec::new(),
            poster: None,
            collections: Vec::new(),
        }
    };

    let official_name = enrichment_cache.official.clone().unwrap_or_else(|| title.clone());
    let aliases = enrichment_cache.all_aliases.clone();
    let base_poster = enrichment_cache.poster.clone();
    let collections = enrichment_cache.collections.clone();
    let base_tmdb_id = tmdb_id_str.as_ref().and_then(|s| s.parse::<u64>().ok());

    // ── 2. PARALLEL FSHARE SEARCHES ─────────────────────────────────────────
    // Build all unique query strings across every title category.
    let mut all_queries: Vec<String> = Vec::new();
    let mut seen_queries: std::collections::HashSet<String> = std::collections::HashSet::new();

    macro_rules! add_query {
        ($q:expr) => {{
            let q: String = $q;
            let key = q.trim().to_lowercase();
            if !key.is_empty() && seen_queries.insert(key) {
                all_queries.push(q);
            }
        }};
    }

    // Primary: official name ± year
    let core_title = extract_core_title(&title);
    if let Some(ref y) = year_str {
        add_query!(format!("{} {}", core_title, y));
    }
    add_query!(core_title.clone());

    // VN titles (+ diacritics-stripped variants)
    for vn in &enrichment_cache.vn_titles {
        add_query!(vn.clone());
        let norm = normalize_vietnamese(vn);
        if norm.to_lowercase() != vn.to_lowercase() {
            add_query!(norm);
        }
    }

    // Original-language titles (Chinese/Korean/Japanese etc.)
    for orig in &enrichment_cache.original_lang_titles {
        add_query!(orig.clone());
    }

    // Original name from TMDB (e.g. "牧神记" for CN shows)
    if let Some(ref orig_name) = enrichment_cache.original_name {
        add_query!(orig_name.clone());
    }

    // US/English alt titles that differ from official name
    for us in &enrichment_cache.us_titles {
        if us.to_lowercase() != official_name.to_lowercase() {
            add_query!(us.clone());
        }
    }

    info!("Movie search queries (parallel): {:?}", all_queries);

    let client_arc = Arc::new(client.clone());
    let fshare_futures: Vec<_> = all_queries.iter().map(|q| {
        let c = Arc::clone(&client_arc);
        let q = q.clone();
        async move { execute_fshare_search(&c, &q, 60).await }
    }).collect();

    let all_search_batches = futures_util::future::join_all(fshare_futures).await;

    // Flatten + deduplicate by pure fcode
    let mut results_pool: Vec<RawFshareResult> = Vec::new();
    let mut seen_fcodes: std::collections::HashSet<String> = std::collections::HashSet::new();
    for batch in all_search_batches {
        for r in batch {
            let pure_fcode = r.fcode.split('?').next().unwrap_or(&r.fcode).to_string();
            if seen_fcodes.insert(pure_fcode) {
                results_pool.push(r);
            }
        }
    }
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

pub async fn handle_tv_search(
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

    // Extract TMDB ID
    let tmdb_id_str = match req.tmdb_id {
        Some(Value::String(ref s)) => Some(s.to_string()),
        Some(Value::Number(ref n)) => Some(n.to_string()),
        _ => None,
    };

    info!("📊 [PERF] TV Smart Search START: title='{}', tmdbId={:?}, year={:?}", title, tmdb_id_str, year_str);

    // ── 1. TMDB ENRICHMENT ──────────────────────────────────────────────────
    let phase1_start = std::time::Instant::now();

    let enrichment_cache: TmdbEnrichmentCache = if let Some(ref tmdb_id) = tmdb_id_str {
        if let Some(cached) = state.tmdb_cache.get(tmdb_id).await {
            cached
        } else {
            let tmdb_id_num: i64 = tmdb_id.parse().unwrap_or(0);
            info!("Enriching TV metadata from TMDB: tmdb_id={}", tmdb_id_num);
            let enrichment = state.tmdb_service.get_tv_enrichment(tmdb_id_num).await;
            let cache_entry = build_tv_enrichment_cache(enrichment, &title);
            info!("TMDB enrichment built — official={:?}, vn={:?}, orig_lang={:?}, us={:?}",
                cache_entry.official, cache_entry.vn_titles,
                cache_entry.original_lang_titles, cache_entry.us_titles);
            state.tmdb_cache.insert(tmdb_id.clone(), cache_entry.clone()).await;
            cache_entry
        }
    } else {
        TmdbEnrichmentCache {
            official: Some(title.clone()),
            original_name: None,
            all_aliases: Vec::new(),
            vn_titles: Vec::new(),
            original_lang_titles: Vec::new(),
            us_titles: Vec::new(),
            poster: None,
            collections: Vec::new(),
        }
    };

    let official_name = enrichment_cache.official.clone().unwrap_or_else(|| title.clone());
    let aliases = enrichment_cache.all_aliases.clone();
    let base_poster = enrichment_cache.poster.clone();
    let base_tmdb_id = tmdb_id_str.as_ref().and_then(|s| s.parse::<u64>().ok());

    // ── 2. BUILD ALL FSHARE QUERIES ─────────────────────────────────────────
    // Collect unique queries across all title categories, then fire them all at once.
    let core_title = extract_core_title(&title);
    let query_keyword = if let Some(ref y) = year_str {
        format!("{} {}", core_title, y)
    } else {
        core_title.clone()
    };

    let mut all_queries: Vec<String> = Vec::new();
    let mut seen_queries: std::collections::HashSet<String> = std::collections::HashSet::new();

    macro_rules! add_query {
        ($q:expr) => {{
            let q: String = $q;
            let key = q.trim().to_lowercase();
            if !key.is_empty() && seen_queries.insert(key) {
                all_queries.push(q);
            }
        }};
    }

    // Primary queries: official name + S/E format
    let query_season = if let (Some(s), Some(e)) = (season, episode) {
        format!("{} S{:02}E{:02}", query_keyword, s, e)
    } else if let Some(s) = season {
        format!("{} Season {}", query_keyword, s)
    } else {
        query_keyword.clone()
    };
    add_query!(query_season.clone());
    if season.is_some() && episode.is_none() {
        if let Some(s) = season {
            add_query!(format!("{} S{:02}", query_keyword, s));
        }
    }

    // VN titles — search with and without diacritics
    for vn in &enrichment_cache.vn_titles {
        add_query!(vn.clone());
        let norm = normalize_vietnamese(vn);
        if norm.to_lowercase() != vn.to_lowercase() {
            add_query!(norm);
        }
    }

    // Original-language titles (e.g. "牧神记", "Mu Shen Ji" for CN shows)
    for orig in &enrichment_cache.original_lang_titles {
        add_query!(orig.clone());
    }

    // Raw original_name from TMDB (e.g. "牧神记")
    if let Some(ref orig_name) = enrichment_cache.original_name {
        add_query!(orig_name.clone());
    }

    // US/English alt titles that differ from official name
    for us in &enrichment_cache.us_titles {
        if us.to_lowercase() != official_name.to_lowercase() {
            add_query!(us.clone());
        }
    }

    info!("📊 [PERF] Phase 1 (TMDB enrichment) took: {:?}", phase1_start.elapsed());
    info!("TV search queries (parallel x{}): {:?}", all_queries.len(), all_queries);

    // ── 3. PARALLEL FSHARE SEARCHES ─────────────────────────────────────────
    let phase2_start = std::time::Instant::now();
    let client_arc = Arc::new(client.clone());
    let fshare_futures: Vec<_> = all_queries.iter().map(|q| {
        let c = Arc::clone(&client_arc);
        let q = q.clone();
        async move { execute_fshare_search(&c, &q, 100).await }
    }).collect();

    let all_search_batches = futures_util::future::join_all(fshare_futures).await;
    info!("📊 [PERF] Phase 2 (parallel Fshare searches) took: {:?}", phase2_start.elapsed());

    // Flatten + deduplicate by pure fcode
    let mut results_pool: Vec<RawFshareResult> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for batch in all_search_batches {
        for r in batch {
            let pure_fcode = r.fcode.split('?').next().unwrap_or(&r.fcode).to_string();
            if seen.insert(pure_fcode.clone()) {
                results_pool.push(r);
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
    let phase3_start = std::time::Instant::now();
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
                let check_limit = max_ep.max(50); // Increased from 35 to handle shows with 40+ episodes
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
    info!("📊 [PERF] Phase 3 (Snowball Logic) took: {:?}", phase3_start.elapsed());
    
    let target_results = results_pool; // Process ALL results, not just first 100


    // 4. PARSE AND EVALUATE
    let phase4_start = std::time::Instant::now();
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
    let mut season_episode_counts: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();  // TMDB official episode counts
    
    if let Some(tmdb_id) = base_tmdb_id {
        let seasons_to_fetch: Vec<u32> = seasons_map.keys().cloned().collect();
        let mut fetch_tasks = Vec::new();
        
        for s_num in seasons_to_fetch {
            let tmdb_svc = state.tmdb_service.clone();
            let tid = tmdb_id as i64;
            fetch_tasks.push(tokio::spawn(async move {
                if let Some(data) = tmdb_svc.get_season_details(tid, s_num as i32).await {
                    return Some((s_num, data));
                }
                None
            }));
        }
        
        let results = futures_util::future::join_all(fetch_tasks).await;
        for res in results.into_iter().flatten().flatten() {
            let (s_num, data) = res;
            if let Some(episodes) = data["episodes"].as_array() {
                // Store TMDB official episode count for this season
                season_episode_counts.insert(s_num, episodes.len() as u32);
                
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

    let today = chrono::Utc::now().date_naive();

    // Pre-calculate aired episode count per season from TMDB metadata
    let mut aired_counts: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    for (&(s, _e), (_name, _overview, air_date, _still)) in &episode_metadata {
        if !air_date.is_empty() {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(air_date, "%Y-%m-%d") {
                if date <= today {
                    *aired_counts.entry(s).or_insert(0) += 1;
                }
            }
        }
    }
    info!("TMDB aired episode counts: {:?}", aired_counts);

    let mut seasons = Vec::new();
    for (s_num, eps_map) in seasons_map {
        let aired_count = aired_counts.get(&s_num).copied().unwrap_or(0);
        let mut episodes_grouped = Vec::new();
        for (e_num, mut files) in eps_map {
            // Sort files by quality score descending
            files.sort_by(|a, b| b.total_score.cmp(&a.total_score));
            
            let meta = episode_metadata.get(&(s_num, e_num));

            // Filter out unreleased episodes.
            //
            // Rule: ONLY filter when TMDB explicitly says the air_date is in the future.
            //
            // We do NOT filter episodes that have no TMDB metadata at all — they may be:
            //  a) Shows with continuous flat numbering (S01E69-71) where TMDB splits into
            //     Season 2, so Season 1 TMDB data only has 68 entries but files go higher.
            //  b) TMDB data lag — episode has aired but TMDB hasn't added it yet.
            //
            // In both cases a real file exists and the user should see it.
            if let Some(m) = meta {
                if !m.2.is_empty() {
                    if let Ok(date) = chrono::NaiveDate::parse_from_str(&m.2, "%Y-%m-%d") {
                        if date > today {
                            info!("Filtering unreleased episode S{}E{} (air_date: {} is in future)", s_num, e_num, m.2);
                            continue;
                        }
                    }
                }
                // air_date empty but metadata exists → assume aired, show it
            }
            // No TMDB metadata at all → unknown, but file exists → show it

            
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
            episode_count: season_episode_counts.get(&s_num).copied().unwrap_or(episodes_grouped.len() as u32),
            aired_episode_count: aired_count,
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
    
    info!("📊 [PERF] Phase 4 (Parse & Evaluate) took: {:?}", phase4_start.elapsed());
    info!("📊 [PERF] ========================================");
    info!("📊 [PERF] TOTAL TV Smart Search took: {:?}", start_time.elapsed());
    info!("📊 [PERF] ========================================");
    
    state.search_cache.insert(cache_key, response.clone()).await;

    Json(response).into_response()
}

/// Delegate Fshare search to SearchPipeline module
async fn execute_fshare_search(client: &Client, query: &str, limit: usize) -> Vec<RawFshareResult> {
    SearchPipeline::execute_fshare_search(client, query, limit).await
}

// =============================================================================
// TMDB Enrichment Cache Builders
// =============================================================================

/// Map a TMDB language code (BCP-47) to the ISO-3166-1 country codes used in
/// alternative_titles and translations responses. We use this to bucket titles
/// from the content's origin language into `original_lang_titles`.
fn lang_to_countries(lang: &str) -> &'static [&'static str] {
    match lang {
        "zh" => &["CN", "TW", "HK", "SG"],
        "ko" => &["KR"],
        "ja" => &["JP"],
        "vi" => &["VN"],
        "th" => &["TH"],
        "en" => &["US", "GB", "AU", "CA"],
        "pt" => &["PT", "BR"],
        "es" => &["ES", "MX", "AR"],
        "fr" => &["FR", "BE", "CH"],
        "id" => &["ID"],
        "ms" => &["MY"],
        _ => &[],
    }
}

/// Build a `TmdbEnrichmentCache` from a TV `MediaEnrichment`.
/// Buckets titles by country code: VN → vn_titles, origin-lang → original_lang_titles,
/// US → us_titles, everything else → all_aliases (for scoring only).
fn build_tv_enrichment_cache(enrichment: MediaEnrichment, fallback_title: &str) -> TmdbEnrichmentCache {
    let official = enrichment.official_name.clone();
    let original_name = enrichment.original_name.clone();
    let original_language = enrichment.original_language.clone();
    let poster = enrichment.poster_path.clone();
    let orig_countries: &[&str] = original_language
        .as_deref()
        .map(lang_to_countries)
        .unwrap_or(&[]);

    // Merge alt_titles + translations into a single (title, iso) stream, deduped by title
    let mut iso_titles: Vec<(String, String)> = Vec::new();
    let mut seen_titles: std::collections::HashSet<String> = std::collections::HashSet::new();

    for t in enrichment.alternative_titles {
        if seen_titles.insert(t.title.to_lowercase()) {
            iso_titles.push((t.title, t.iso_3166_1));
        }
    }
    for t in enrichment.translations {
        if !t.name.is_empty() && seen_titles.insert(t.name.to_lowercase()) {
            iso_titles.push((t.name, t.iso_3166_1));
        }
    }
    // Also include original_name if not already present
    if let Some(ref on) = original_name {
        if !on.is_empty() && seen_titles.insert(on.to_lowercase()) {
            let on_country = orig_countries.first().copied().unwrap_or("");
            iso_titles.push((on.clone(), on_country.to_string()));
        }
    }

    let mut vn_titles = Vec::new();
    let mut original_lang_titles = Vec::new();
    let mut us_titles = Vec::new();
    let mut other_titles = Vec::new();

    let official_lower = official.as_deref().unwrap_or(fallback_title).to_lowercase();

    for (title, iso) in &iso_titles {
        if iso == "VN" {
            vn_titles.push(title.clone());
        } else if iso == "US" {
            if title.to_lowercase() != official_lower {
                us_titles.push(title.clone());
            }
        } else if orig_countries.contains(&iso.as_str()) {
            original_lang_titles.push(title.clone());
        } else {
            other_titles.push(title.clone());
        }
    }

    // all_aliases = VN first (priority for scoring), then orig-lang, US, others
    let mut all_aliases = vn_titles.clone();
    all_aliases.extend(original_lang_titles.iter().cloned());
    all_aliases.extend(us_titles.iter().cloned());
    all_aliases.extend(other_titles);

    TmdbEnrichmentCache {
        official,
        original_name,
        all_aliases,
        vn_titles,
        original_lang_titles,
        us_titles,
        poster,
        collections: Vec::new(), // TV has no collections
    }
}

/// Build a `TmdbEnrichmentCache` from a Movie `MediaEnrichment`.
fn build_movie_enrichment_cache(enrichment: MediaEnrichment, fallback_title: &str) -> TmdbEnrichmentCache {
    let official = enrichment.official_name.clone();
    let original_name = enrichment.original_name.clone();
    let original_language = enrichment.original_language.clone();
    let poster = enrichment.poster_path.clone();
    let orig_countries: &[&str] = original_language
        .as_deref()
        .map(lang_to_countries)
        .unwrap_or(&[]);

    let mut iso_titles: Vec<(String, String)> = Vec::new();
    let mut seen_titles: std::collections::HashSet<String> = std::collections::HashSet::new();

    for t in enrichment.alternative_titles {
        if seen_titles.insert(t.title.to_lowercase()) {
            iso_titles.push((t.title, t.iso_3166_1));
        }
    }
    if let Some(ref on) = original_name {
        if !on.is_empty() && seen_titles.insert(on.to_lowercase()) {
            let on_country = orig_countries.first().copied().unwrap_or("");
            iso_titles.push((on.clone(), on_country.to_string()));
        }
    }

    let mut vn_titles = Vec::new();
    let mut original_lang_titles = Vec::new();
    let mut us_titles = Vec::new();
    let mut other_titles = Vec::new();

    let official_lower = official.as_deref().unwrap_or(fallback_title).to_lowercase();

    for (title, iso) in &iso_titles {
        if iso == "VN" {
            vn_titles.push(title.clone());
        } else if iso == "US" {
            if title.to_lowercase() != official_lower {
                us_titles.push(title.clone());
            }
        } else if orig_countries.contains(&iso.as_str()) {
            original_lang_titles.push(title.clone());
        } else {
            other_titles.push(title.clone());
        }
    }

    // Extract collection parts
    let collections = enrichment.collection.map(|coll| {
        coll.parts.into_iter()
            .map(|p| (p.title, String::new(), p.id as u64, None::<String>))
            .collect()
    }).unwrap_or_default();

    let mut all_aliases = vn_titles.clone();
    all_aliases.extend(original_lang_titles.iter().cloned());
    all_aliases.extend(us_titles.iter().cloned());
    all_aliases.extend(other_titles);

    TmdbEnrichmentCache {
        official,
        original_name,
        all_aliases,
        vn_titles,
        original_lang_titles,
        us_titles,
        poster,
        collections,
    }
}
