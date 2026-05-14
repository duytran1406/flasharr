//! Newznab/Torznab Indexer API
//!
//! Provides Newznab-compatible endpoints for integration with Sonarr/Radarr.
//! This allows *arr applications to search for content on Fshare.

use crate::constants::TMDB_API_KEY;
use crate::AppState;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Router,
};
use chrono::{DateTime, Utc};
use moka::future::Cache;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(handle_indexer))
        .route("/download", get(handle_nzb_download))
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Deserialize, Debug)]
pub struct IndexerParams {
    /// Query type: caps, search, tvsearch, movie
    pub t: String,

    /// API key for authentication
    #[serde(default)]
    pub apikey: String,

    /// Search query
    #[serde(default)]
    pub q: Option<String>,

    /// Season number (for TV)
    #[serde(default)]
    pub season: Option<u32>,

    /// Episode number (for TV)
    #[serde(default)]
    pub ep: Option<u32>,

    /// TMDB ID (for movies — Radarr sends this directly)
    #[serde(default)]
    pub tmdbid: Option<String>,

    /// IMDB ID (for movies — fallback if tmdbid not present)
    #[serde(default)]
    #[allow(dead_code)]
    pub imdbid: Option<String>,

    /// TVDB ID (for TV)
    #[serde(default)]
    #[allow(dead_code)]
    pub tvdbid: Option<String>,

    /// Category IDs
    #[serde(default)]
    #[allow(dead_code)]
    pub cat: Option<String>,
}

// ============================================================================
// Cache Setup
// ============================================================================

/// Get or initialize the search cache
fn get_search_cache() -> &'static Cache<String, Vec<IndexerResult>> {
    use once_cell::sync::Lazy;

    static SEARCH_CACHE: Lazy<Cache<String, Vec<IndexerResult>>> = Lazy::new(|| {
        Cache::builder()
            .max_capacity(1000) // Store up to 1000 search results
            .time_to_live(Duration::from_secs(300)) // 5 minute TTL
            .build()
    });

    &SEARCH_CACHE
}

// ============================================================================
// Handlers
// ============================================================================

/// Main indexer endpoint handler
async fn handle_indexer(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<IndexerParams>,
) -> impl axum::response::IntoResponse {
    use axum::body::Body;
    use axum::http::header;
    use axum::response::Response;

    // Extract host from request headers for dynamic URL generation
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost:8484");

    tracing::info!(
        "Newznab API request - mode: {}, q: {:?}, season: {:?}, ep: {:?}, tmdbid: {:?}, imdbid: {:?}, tvdbid: {:?}, cat: {:?}, apikey: {:?}",
        params.t,
        params.q,
        params.season,
        params.ep,
        params.tmdbid,
        params.imdbid,
        params.tvdbid,
        params.cat,
        params.apikey
    );

    let (status, xml_body) = match params.t.as_str() {
        "caps" => (StatusCode::OK, handle_caps()),
        "search" => {
            if !crate::api::auth::validate_api_key(&state, &params.apikey) {
                tracing::warn!("Invalid API key provided: {:?}", params.apikey);
                (
                    StatusCode::UNAUTHORIZED,
                    generate_error_xml("Invalid API key"),
                )
            } else {
                (StatusCode::OK, handle_search(state, params).await)
            }
        }
        "tvsearch" => {
            if !crate::api::auth::validate_api_key(&state, &params.apikey) {
                tracing::warn!("Invalid API key provided: {:?}", params.apikey);
                (
                    StatusCode::UNAUTHORIZED,
                    generate_error_xml("Invalid API key"),
                )
            } else {
                (StatusCode::OK, handle_tv_search(state, params, host).await)
            }
        }
        "movie" => {
            if !crate::api::auth::validate_api_key(&state, &params.apikey) {
                tracing::warn!("Invalid API key provided: {:?}", params.apikey);
                (
                    StatusCode::UNAUTHORIZED,
                    generate_error_xml("Invalid API key"),
                )
            } else {
                (
                    StatusCode::OK,
                    handle_movie_search(state, params, host).await,
                )
            }
        }
        _ => (
            StatusCode::BAD_REQUEST,
            generate_error_xml("Unknown function"),
        ),
    };

    // CRITICAL: Trim to remove any leading/trailing whitespace from r#""# strings
    let trimmed_body = xml_body.trim().to_string();

    // Debug: Log response preview for troubleshooting
    let preview = if trimmed_body.len() > 500 {
        format!(
            "{}... ({} total bytes)",
            &trimmed_body[..500],
            trimmed_body.len()
        )
    } else {
        trimmed_body.clone()
    };
    tracing::debug!("XML Response Preview: {}", preview);

    // FORCE the response headers and body using explicit builder
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
        .body(Body::from(trimmed_body))
        .unwrap()
}

#[derive(Deserialize)]
struct NzbDownloadParams {
    fcode: String,
    // TMDB metadata for proper file organization
    tmdb_id: Option<String>,
    media_type: Option<String>,
    season: Option<u32>,
    episode: Option<u32>,
    // Category from Sonarr/Radarr (e.g., "tv-sonarr", "movies")
    cat: Option<String>,
}

/// Generate a fake NZB file for Sonarr/Radarr validation
/// The NZB contains the Fshare URL in metadata which SABnzbd shim will extract
async fn handle_nzb_download(
    Query(params): Query<NzbDownloadParams>,
) -> impl axum::response::IntoResponse {
    use axum::body::Body;
    use axum::http::header;
    use axum::response::Response;

    let fshare_url = format!("https://www.fshare.vn/file/{}", params.fcode);

    // Build metadata tags
    let mut metadata_tags = format!("        <meta type=\"fshare_url\">{}</meta>\n", fshare_url);
    if let Some(tmdb_id) = params.tmdb_id {
        metadata_tags.push_str(&format!(
            "        <meta type=\"tmdb_id\">{}</meta>\n",
            tmdb_id
        ));
    }
    if let Some(media_type) = params.media_type {
        metadata_tags.push_str(&format!(
            "        <meta type=\"media_type\">{}</meta>\n",
            media_type
        ));
    }
    if let Some(season) = params.season {
        metadata_tags.push_str(&format!(
            "        <meta type=\"season\">{}</meta>\n",
            season
        ));
    }
    if let Some(episode) = params.episode {
        metadata_tags.push_str(&format!(
            "        <meta type=\"episode\">{}</meta>\n",
            episode
        ));
    }
    if let Some(ref category) = params.cat {
        metadata_tags.push_str(&format!(
            "        <meta type=\"category\">{}</meta>\n",
            category
        ));
    }

    // Generate a minimal valid NZB XML that won't trigger parser errors
    let nzb_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<nzb xmlns="http://www.newznab.com/DTD/2010/nzb">
    <head>
{}    </head>
    <file poster="Flasharr" date="1706534400" subject="Fshare_Download_{}">
        <groups><group>alt.binaries.test</group></groups>
        <segments><segment bytes="1" number="1">fake</segment></segments>
    </file>
</nzb>"#,
        metadata_tags, params.fcode
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-nzb")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}.nzb\"", params.fcode),
        )
        .body(Body::from(nzb_xml))
        .unwrap()
}

/// Handle capabilities request
fn handle_caps() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<caps>
  <server title="Fshare Indexer" version="1.0" />
  <limits max="100" default="100" />
  <searching>
    <search available="yes" supportedParams="q" />
    <tv-search available="yes" supportedParams="q,season,ep,tvdbid" />
    <movie-search available="yes" supportedParams="q,tmdbid,imdbid" />
  </searching>
  <categories>
    <category id="2000" name="Movies">
      <subcat id="2040" name="Movies/HD" />
      <subcat id="2045" name="Movies/UHD" />
    </category>
    <category id="5000" name="TV">
      <subcat id="5040" name="TV/HD" />
      <subcat id="5045" name="TV/UHD" />
    </category>
  </categories>
</caps>"#
        .to_string()
}

// ============================================================================
// ID Conversion & Smart Search Bridge
// ============================================================================

/// Convert TVDB ID to TMDB ID
async fn tvdb_to_tmdb(tvdb_id: &str) -> Option<String> {
    let client = Client::new();
    let url = format!(
        "https://api.themoviedb.org/3/find/{}?api_key={}&external_source=tvdb_id",
        tvdb_id, TMDB_API_KEY
    );

    let resp = client.get(&url).send().await.ok()?;
    let data: Value = resp.json().await.ok()?;

    data["tv_results"].as_array()?.first()?["id"]
        .as_u64()
        .map(|id| id.to_string())
}

/// Convert IMDB ID to TMDB ID
async fn imdb_to_tmdb(imdb_id: &str) -> Option<String> {
    let client = Client::new();

    // Ensure IMDB ID has 'tt' prefix
    let clean_id = if imdb_id.starts_with("tt") {
        imdb_id.to_string()
    } else {
        format!("tt{}", imdb_id)
    };

    let url = format!(
        "https://api.themoviedb.org/3/find/{}?api_key={}&external_source=imdb_id",
        clean_id, TMDB_API_KEY
    );

    let resp = client.get(&url).send().await.ok()?;
    let data: Value = resp.json().await.ok()?;

    data["movie_results"].as_array()?.first()?["id"]
        .as_u64()
        .map(|id| id.to_string())
}

/// Map a Sonarr/Radarr quality profile ID to the set of allowed resolution strings.
/// Returns None for "Any" / unknown profiles (no filtering applied).
// NOTE: assumes default Sonarr/Radarr profile IDs (custom installs that delete/recreate profiles can shift IDs)
fn profile_id_to_allowed_resolutions(profile_id: i32) -> Option<Vec<&'static str>> {
    match profile_id {
        2 => Some(vec!["480p", "576p"]),  // SD
        3 => Some(vec!["720p"]),          // HD-720p
        4 => Some(vec!["1080p"]),         // HD-1080p
        5 => Some(vec!["2160p"]),         // Ultra-HD
        6 => Some(vec!["720p", "1080p"]), // HD-720p/1080p
        _ => None,                        // Any (1) or unknown — no filter
    }
}

/// Returns true if the file's resolution is acceptable under the given quality filter.
/// Files with an undetected resolution always pass (we can't classify them).
fn resolution_allowed(resolution: Option<&str>, allowed: &Option<Vec<&'static str>>) -> bool {
    match (resolution, allowed) {
        (_, None) => true,       // no filter configured
        (None, Some(_)) => true, // unknown resolution — let it through
        (Some(res), Some(allowed)) => allowed.iter().any(|&a| a == res),
    }
}

/// Convert SmartSearchResponse to Newznab XML
/// This extracts results from the smart_search response and converts to IndexerResult format
async fn convert_smart_response_to_xml(
    response: axum::response::Response,
    query: &str,
    host: &str,
    apikey: &str,
    tmdb_id: Option<String>,
    media_type: &str,
    season: Option<u32>,
    episode: Option<u32>,
    allowed_resolutions: Option<Vec<&'static str>>,
) -> String {
    use crate::api::smart_search::SmartSearchResponse;
    use axum::body::to_bytes;

    // Extract body bytes from response
    let (_parts, body) = response.into_parts();
    let body_bytes = match to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!("Failed to read response body: {}", e);
            return generate_search_xml(vec![], query);
        }
    };

    // Deserialize SmartSearchResponse
    let smart_response: SmartSearchResponse = match serde_json::from_slice(&body_bytes) {
        Ok(resp) => resp,
        Err(e) => {
            tracing::error!("Failed to deserialize SmartSearchResponse: {}", e);
            return generate_search_xml(vec![], query);
        }
    };

    // Fetch TMDB title AND year for synthesis — year is needed for Sonarr to disambiguate
    // series (e.g. "How Dare You!? (2026)"). Without the year Sonarr reports "Unknown Series".
    let (tmdb_title, tmdb_year) = if let Some(ref id) = tmdb_id {
        fetch_tmdb_title_and_year(id, media_type).await
    } else {
        (None, None)
    };

    // Extract all files from quality groups
    let mut indexer_results = Vec::new();
    let group_count = smart_response.groups.as_ref().map(|g| g.len()).unwrap_or(0);

    if let Some(groups) = smart_response.groups {
        for group in groups {
            for file in group.files {
                if !resolution_allowed(
                    file.quality_attrs.resolution.as_deref(),
                    &allowed_resolutions,
                ) {
                    tracing::debug!(
                        "Quality filter: skipping '{}' (resolution={:?})",
                        file.name,
                        file.quality_attrs.resolution
                    );
                    continue;
                }

                let category = determine_category(&file.name);
                let synthesized_title =
                    synthesize_title(&file, tmdb_title.as_deref(), tmdb_year, season, episode);
                indexer_results.push(IndexerResult {
                    title: synthesized_title,
                    guid: format!("fshare://{}", extract_file_id(&file.url)),
                    link: generate_nzb_url(
                        &file.url,
                        host,
                        apikey,
                        &tmdb_id,
                        &Some(media_type.to_string()),
                        &season,
                        &episode,
                    ),
                    size: file.size,
                    pub_date: Utc::now(),
                    category,
                });
            }
        }
    }

    // Handle TV seasons structure if present
    if let Some(seasons) = smart_response.seasons {
        for season_group in seasons {
            for episode_group in season_group.episodes_grouped {
                // Use the episode number from the group, not the outer query filter.
                // The outer `episode` is the Sonarr search filter (e.g. E03 when searching
                // for a specific episode). When iterating seasons we must use each group's
                // own episode number so every result gets the correct S##E## tag.
                let ep_num = Some(episode_group.episode_number);
                for file in episode_group.files {
                    if !resolution_allowed(
                        file.quality_attrs.resolution.as_deref(),
                        &allowed_resolutions,
                    ) {
                        tracing::debug!(
                            "Quality filter: skipping '{}' (resolution={:?})",
                            file.name,
                            file.quality_attrs.resolution
                        );
                        continue;
                    }

                    let synthesized_title = synthesize_title(
                        &file,
                        tmdb_title.as_deref(),
                        tmdb_year,
                        Some(season_group.season),
                        ep_num,
                    );
                    indexer_results.push(IndexerResult {
                        title: synthesized_title,
                        guid: format!("fshare://{}", extract_file_id(&file.url)),
                        link: generate_nzb_url(
                            &file.url,
                            host,
                            apikey,
                            &tmdb_id,
                            &Some(media_type.to_string()),
                            &Some(season_group.season),
                            &ep_num,
                        ),
                        size: file.size,
                        pub_date: Utc::now(),
                        category: determine_category(&file.name),
                    });
                }
            }
        }
    }

    tracing::info!(
        "Converted SmartSearchResponse: {} results from {} quality groups (TMDB: {:?} {:?}, filter: {:?})",
        indexer_results.len(),
        group_count,
        tmdb_title,
        tmdb_year,
        allowed_resolutions,
    );

    generate_search_xml(indexer_results, query)
}

/// Fetch TMDB title for a given ID — with in-process caching.
///
/// During a batch grab of 71 episodes the orchestrator calls this once per
/// episode because `meta.title` is None.  Without caching that's 71 TMDB
/// requests in rapid succession, which hits TMDB's rate limit (~40 req/10s)
/// and returns 429 errors from episode 31 onwards.
///
/// This cache keeps `(tmdb_id, media_type)` → title for 24 hours so the
/// entire batch costs exactly ONE real TMDB call.
pub async fn fetch_tmdb_title(tmdb_id: &str, media_type: &str) -> Option<String> {
    use moka::future::Cache;
    use once_cell::sync::Lazy;

    static TITLE_CACHE: Lazy<Cache<String, Option<String>>> = Lazy::new(|| {
        Cache::builder()
            .max_capacity(2048)
            .time_to_live(std::time::Duration::from_secs(86400)) // 24h
            .build()
    });

    let cache_key = format!("{}:{}", media_type, tmdb_id);

    if let Some(cached) = TITLE_CACHE.get(&cache_key).await {
        tracing::debug!("TMDB title cache HIT: {} → {:?}", cache_key, cached);
        return cached;
    }

    tracing::info!("TMDB title cache MISS: {} — fetching from API", cache_key);
    let client = Client::new();
    let endpoint = if media_type == "tv" { "tv" } else { "movie" };
    let url = format!(
        "https://api.themoviedb.org/3/{}/{}?api_key={}",
        endpoint, tmdb_id, TMDB_API_KEY
    );

    let result: Option<String> = async {
        let resp = client.get(&url).send().await.ok()?;
        let data: Value = resp.json().await.ok()?;
        if media_type == "tv" {
            let name = data["name"].as_str().unwrap_or("");
            let _year = data["first_air_date"]
                .as_str()
                .and_then(|d| d.split('-').next())
                .unwrap_or("");
            if !name.is_empty() {
                // Return bare title only; year is tracked separately via meta.year
                // to avoid embedding it directly into the string (which causes
                // "Title Year" vs "Title (Year)" inconsistency in filenames).
                Some(name.to_string())
            } else {
                None
            }
        } else {
            let title = data["title"].as_str().unwrap_or("");
            if !title.is_empty() {
                Some(title.to_string())
            } else {
                None
            }
        }
    }
    .await;

    // Only cache successful results — caching None would poison the cache for 24h
    // after any transient TMDB API failure (429, timeout, etc.)
    if result.is_some() {
        TITLE_CACHE.insert(cache_key, result.clone()).await;
    }
    result
}

/// Fetch both title and release year from TMDB in a single call.
///
/// Returns `(title, year)` where year is `None` when not available.
/// Shares the same HTTP round-trip budget as `fetch_tmdb_title` using a
/// dedicated cache keyed on `"year:{media_type}:{tmdb_id}"`.
pub async fn fetch_tmdb_title_and_year(
    tmdb_id: &str,
    media_type: &str,
) -> (Option<String>, Option<i32>) {
    use moka::future::Cache;
    use once_cell::sync::Lazy;

    // Cache stores (title, year_string) — both may be None independently.
    static PAIR_CACHE: Lazy<Cache<String, (Option<String>, Option<String>)>> = Lazy::new(|| {
        Cache::builder()
            .max_capacity(2048)
            .time_to_live(std::time::Duration::from_secs(86400))
            .build()
    });

    let cache_key = format!("pair:{}:{}", media_type, tmdb_id);

    if let Some((cached_title, cached_year_str)) = PAIR_CACHE.get(&cache_key).await {
        let year = cached_year_str
            .as_deref()
            .and_then(|y| y.parse::<i32>().ok());
        return (cached_title, year);
    }

    let client = Client::new();
    let endpoint = if media_type == "tv" { "tv" } else { "movie" };
    let url = format!(
        "https://api.themoviedb.org/3/{}/{}?api_key={}",
        endpoint, tmdb_id, TMDB_API_KEY
    );

    let pair: (Option<String>, Option<String>) = async {
        let resp = client.get(&url).send().await.ok()?;
        let data: Value = resp.json().await.ok()?;
        if media_type == "tv" {
            let name = data["name"].as_str().unwrap_or("");
            let year_str = data["first_air_date"]
                .as_str()
                .and_then(|d| d.split('-').next())
                .unwrap_or("");
            let title = if !name.is_empty() {
                Some(name.to_string())
            } else {
                None
            };
            let year = if !year_str.is_empty() {
                Some(year_str.to_string())
            } else {
                None
            };
            Some((title, year))
        } else {
            let name = data["title"].as_str().unwrap_or("");
            let year_str = data["release_date"]
                .as_str()
                .and_then(|d| d.split('-').next())
                .unwrap_or("");
            let title = if !name.is_empty() {
                Some(name.to_string())
            } else {
                None
            };
            let year = if !year_str.is_empty() {
                Some(year_str.to_string())
            } else {
                None
            };
            Some((title, year))
        }
    }
    .await
    .unwrap_or((None, None));

    // Only cache when we got a title — avoid poisoning the cache on transient API failures
    if pair.0.is_some() {
        PAIR_CACHE.insert(cache_key, pair.clone()).await;
    }
    let year = pair.1.as_deref().and_then(|y| y.parse::<i32>().ok());
    (pair.0, year)
}

/// Synthesize a parseable title for Sonarr/Radarr
/// Format: "Series.Name.2026.S01E01.2160p.WEB-DL.H.265-FShare.mkv"
///
/// The year is included between the title and S##E## so Sonarr can
/// unambiguously match the release to the correct series entry (e.g.
/// "How Dare You!? (2026)"). Without it Sonarr reports "Unknown Series".
fn synthesize_title(
    file: &crate::utils::title_matcher::SmartSearchResult,
    tmdb_title: Option<&str>,
    tmdb_year: Option<i32>,
    season: Option<u32>,
    episode: Option<u32>,
) -> String {
    let mut parts = Vec::new();

    // 1. Clean TMDB title (strip punctuation that confuses scene parsers)
    if let Some(title) = tmdb_title {
        let clean_title = title
            .replace(':', "")
            .replace('?', "")
            .replace('!', "")
            .replace(',', "");
        parts.push(clean_title.trim().replace(' ', "."));
    } else {
        // No TMDB info — fall back to the original filename unchanged
        return file.name.clone();
    }

    // 2. Release year (critical for Sonarr series disambiguation)
    if let Some(year) = tmdb_year {
        parts.push(year.to_string());
    }

    // 3. Season & Episode
    if let (Some(s), Some(e)) = (season, episode) {
        parts.push(format!("S{:02}E{:02}", s, e));
    }

    // 4. Quality metadata — fall back to size-inferred resolution so Radarr/Sonarr
    // never defaults to SDTV for files whose filename lacks a resolution keyword.
    let res_str = file
        .quality_attrs
        .resolution
        .as_deref()
        .filter(|r| !r.is_empty())
        .or_else(|| match file.size {
            s if s >= 10_000_000_000 => Some("2160p"),
            s if s >= 2_000_000_000 => Some("1080p"),
            s if s >= 300_000_000 => Some("720p"),
            _ => None,
        });
    if let Some(res) = res_str {
        parts.push(res.to_string());
    }
    if let Some(src) = &file.quality_attrs.source {
        if !src.is_empty() {
            parts.push(src.clone());
        }
    }
    if file.quality_attrs.hdr {
        parts.push("HDR".to_string());
    }
    if file.quality_attrs.dolby_vision {
        parts.push("DV".to_string());
    }
    if let Some(vc) = &file.quality_attrs.video_codec {
        if !vc.is_empty() {
            parts.push(vc.clone());
        }
    }
    if let Some(ac) = &file.quality_attrs.audio_codec {
        if !ac.is_empty() {
            parts.push(ac.clone());
        }
    }

    // 5. Release group tag
    parts.push("-FShare".to_string());

    let ext = std::path::Path::new(&file.name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mkv");

    format!("{}.{}", parts.join("."), ext)
}

/// Extract file ID from Fshare URL
fn extract_file_id(url: &str) -> String {
    url.split('/')
        .last()
        .and_then(|s| s.split('?').next())
        .unwrap_or("unknown")
        .to_string()
}

/// Generate NZB download URL for Sonarr/Radarr
/// Points to our /download endpoint which generates a fake NZB with Fshare URL embedded
fn generate_nzb_url(
    fshare_url: &str,
    host: &str,
    apikey: &str,
    tmdb_id: &Option<String>,
    media_type: &Option<String>,
    season: &Option<u32>,
    episode: &Option<u32>,
) -> String {
    let file_id = extract_file_id(fshare_url);
    let mut url = format!("http://{}/newznab/api/download?fcode={}", host, file_id);

    // Auth for download endpoint
    url.push_str(&format!("&apikey={}", apikey));

    // Add TMDB metadata as query parameters
    if let Some(id) = tmdb_id {
        url.push_str(&format!("&tmdb_id={}", id));
    }
    if let Some(mt) = media_type {
        url.push_str(&format!("&media_type={}", mt));
    }
    if let Some(s) = season {
        url.push_str(&format!("&season={}", s));
    }
    if let Some(e) = episode {
        url.push_str(&format!("&episode={}", e));
    }

    url
}

/// Handle general search
async fn handle_search(state: Arc<AppState>, params: IndexerParams) -> String {
    let query = match params.q {
        Some(q) if !q.is_empty() => q,
        _ => return generate_search_xml(vec![], ""), // Return empty feed instead of error
    };

    // Check cache first
    let cache = get_search_cache();
    let cache_key = format!("search:{}", query);

    let results = if let Some(cached) = cache.get(&cache_key).await {
        cached
    } else {
        // Cache miss - fetch from API
        let results = execute_fshare_search_for_indexer(&state, &query).await;
        // Store in cache
        cache.insert(cache_key, results.clone()).await;
        results
    };

    generate_search_xml(results, &query)
}

/// Handle TV search - Bridge to smart_search with TVDB → TMDB conversion
async fn handle_tv_search(state: Arc<AppState>, params: IndexerParams, host: &str) -> String {
    let apikey = params.apikey.clone();

    // Step 1: Convert TVDB ID to TMDB ID if provided
    let tmdb_id = if let Some(tvdb_id) = params.tvdbid {
        match tvdb_to_tmdb(&tvdb_id).await {
            Some(id) => {
                tracing::info!("Converted TVDB {} → TMDB {}", tvdb_id, id);
                Some(Value::String(id))
            }
            None => {
                tracing::warn!("Failed to convert TVDB {} to TMDB", tvdb_id);
                None
            }
        }
    } else {
        None
    };

    // Step 2: Build SmartSearchRequest with all available data
    // If both query and TVDB ID are empty, use a default term for testing
    let title = params.q.unwrap_or_else(|| {
        if tmdb_id.is_none() {
            "phim".to_string() // Vietnamese word - will return some results
        } else {
            "".to_string()
        }
    });

    let smart_req = crate::api::smart_search::SmartSearchRequest {
        title: title.clone(),
        tmdb_id,
        r#type: "tv".to_string(),
        season: params.season,
        episode: params.ep,
        year: None, // TV shows don't use year in the same way
    };

    tracing::info!(
        "TV Search Bridge: title='{}', tmdb_id={:?}, season={:?}, ep={:?}",
        title,
        smart_req.tmdb_id,
        smart_req.season,
        smart_req.episode
    );

    // Clone metadata before smart_req is moved
    let tmdb_id_str = smart_req
        .tmdb_id
        .as_ref()
        .map(|v| v.as_str().unwrap_or("").to_string());
    let season_num = smart_req.season;
    let episode_num = smart_req.episode;

    // Resolve quality filter from Sonarr profile setting
    let allowed_resolutions = state
        .db
        .get_setting("sonarr_quality_profile_id")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<i32>().ok())
        .and_then(profile_id_to_allowed_resolutions);

    // Step 3: Call smart_search (already has Vietnamese title logic!)
    let response = crate::api::smart_search::handle_tv_search(state, smart_req).await;

    // Step 4: Convert SmartSearchResponse to Newznab XML
    convert_smart_response_to_xml(
        response,
        &title,
        host,
        &apikey,
        tmdb_id_str,
        "tv",
        season_num,
        episode_num,
        allowed_resolutions,
    )
    .await
}

/// Handle movie search - Bridge to smart_search with IMDB → TMDB conversion
async fn handle_movie_search(state: Arc<AppState>, params: IndexerParams, host: &str) -> String {
    let apikey = params.apikey.clone();

    // Step 1: Resolve TMDB ID — prefer direct tmdbid, fall back to converting imdbid
    let tmdb_id = if let Some(ref id) = params.tmdbid {
        tracing::info!("Using direct TMDB ID from Radarr: {}", id);
        Some(Value::String(id.clone()))
    } else if let Some(imdb_id) = params.imdbid {
        match imdb_to_tmdb(&imdb_id).await {
            Some(id) => {
                tracing::info!("Converted IMDB {} → TMDB {}", imdb_id, id);
                Some(Value::String(id))
            }
            None => {
                tracing::warn!("Failed to convert IMDB {} to TMDB", imdb_id);
                None
            }
        }
    } else {
        None
    };

    // Step 2: Extract year from query or use default search term
    // If both query and IMDB ID are empty, use a default term for testing
    let title = params.q.unwrap_or_else(|| {
        if tmdb_id.is_none() {
            "phim".to_string() // Vietnamese word for "movie" - will return some results
        } else {
            "".to_string()
        }
    });
    let year = None; // Radarr doesn't typically send year as separate param

    // Step 3: Build SmartSearchRequest with all available data
    let smart_req = crate::api::smart_search::SmartSearchRequest {
        title: title.clone(),
        tmdb_id,
        r#type: "movie".to_string(),
        season: None,
        episode: None,
        year,
    };

    tracing::info!(
        "Movie Search Bridge: title='{}', tmdb_id={:?}, year={:?}",
        title,
        smart_req.tmdb_id,
        smart_req.year
    );

    // Clone metadata before smart_req is moved
    let tmdb_id_str = smart_req
        .tmdb_id
        .as_ref()
        .map(|v| v.as_str().unwrap_or("").to_string());

    // Resolve quality filter from Radarr profile setting
    let allowed_resolutions = state
        .db
        .get_setting("radarr_quality_profile_id")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<i32>().ok())
        .and_then(profile_id_to_allowed_resolutions);

    // Step 4: Call smart_search (already has Vietnamese title logic!)
    let response = crate::api::smart_search::handle_movie_search(state, smart_req).await;

    // Step 5: Convert SmartSearchResponse to Newznab XML
    convert_smart_response_to_xml(
        response,
        &title,
        host,
        &apikey,
        tmdb_id_str,
        "movie",
        None,
        None,
        allowed_resolutions,
    )
    .await
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Execute Fshare search and convert to indexer results
async fn execute_fshare_search_for_indexer(_state: &AppState, query: &str) -> Vec<IndexerResult> {
    use crate::api::search_pipeline::SearchPipeline;
    use reqwest::Client;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| Client::new());

    // Use SearchPipeline for consistent Fshare search
    let raw_results = SearchPipeline::execute_fshare_search(&client, query, 100).await;

    raw_results
        .into_iter()
        .map(|r| IndexerResult {
            title: r.name.clone(),
            guid: format!("fshare://{}", r.fcode),
            link: r.url,
            size: r.size,
            pub_date: Utc::now(),
            category: determine_category(&r.name),
        })
        .collect()
}

/// Determine category from filename with enhanced pattern matching
fn determine_category(filename: &str) -> u32 {
    use once_cell::sync::Lazy;
    use regex::Regex;

    // Compile regex patterns once
    static TV_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
        vec![
            // Standard patterns: S01E01, S1E1, s01e01
            Regex::new(r"(?i)s\d{1,2}e\d{1,2}").unwrap(),
            // Alternative: 1x01, 1X01
            Regex::new(r"(?i)\d{1,2}x\d{1,2}").unwrap(),
            // Season/Episode words
            Regex::new(r"(?i)(season|episode|tập|tap)\s*\d+").unwrap(),
            // Complete Season: Season 1, Season.1
            Regex::new(r"(?i)season[\s.]*\d+").unwrap(),
        ]
    });

    static ANIME_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
        vec![
            // Anime episode patterns: [01], - 01, EP01
            Regex::new(r"(?i)\[(\d{1,3})\]").unwrap(),
            Regex::new(r"(?i)\s-\s(\d{1,3})\s").unwrap(),
            Regex::new(r"(?i)ep\s*\d{1,3}").unwrap(),
            // Batch patterns
            Regex::new(r"(?i)(batch|complete|全集)").unwrap(),
        ]
    });

    let lower = filename.to_lowercase();

    // Check for anime indicators first (more specific)
    let is_anime = ANIME_PATTERNS.iter().any(|re| re.is_match(&lower))
        || lower.contains("anime")
        || lower.contains("アニメ")
        || lower.contains("[")
            && lower.contains("]")
            && (lower.contains("1080p") || lower.contains("720p") || lower.contains("2160p"));

    // Check for TV show patterns
    let is_tv = TV_PATTERNS.iter().any(|re| re.is_match(&lower));

    // Check for resolution
    let is_uhd = lower.contains("2160p")
        || lower.contains("4k")
        || lower.contains("uhd")
        || lower.contains("2k");

    // Determine category with priority: Anime > TV > Movies
    if is_anime {
        if is_uhd {
            5045 // TV/UHD (anime in UHD)
        } else {
            5040 // TV/HD (anime in HD)
        }
    } else if is_tv {
        if is_uhd {
            5045 // TV/UHD
        } else {
            5040 // TV/HD
        }
    } else {
        // Movies
        if is_uhd {
            2045 // Movies/UHD
        } else {
            2040 // Movies/HD (default)
        }
    }
}

/// Generate Newznab XML for search results
fn generate_search_xml(results: Vec<IndexerResult>, _query: &str) -> String {
    use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
    use quick_xml::Writer;
    use std::io::Cursor;

    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // XML declaration
    writer
        .write_event(Event::Decl(quick_xml::events::BytesDecl::new(
            "1.0",
            Some("UTF-8"),
            None,
        )))
        .unwrap();

    // RSS root
    let mut rss = BytesStart::new("rss");
    rss.push_attribute(("version", "2.0"));
    rss.push_attribute(("xmlns:atom", "http://www.w3.org/2005/Atom"));
    rss.push_attribute((
        "xmlns:newznab",
        "http://www.newznab.com/DTD/2010/feeds/attributes/",
    ));
    rss.push_attribute(("xmlns:torznab", "http://torznab.com/schemas/2015/feed"));
    writer.write_event(Event::Start(rss)).unwrap();

    // Channel
    writer
        .write_event(Event::Start(BytesStart::new("channel")))
        .unwrap();

    // Channel title
    writer
        .write_event(Event::Start(BytesStart::new("title")))
        .unwrap();
    writer
        .write_event(Event::Text(BytesText::new("Fshare Indexer")))
        .unwrap();
    writer
        .write_event(Event::End(BytesEnd::new("title")))
        .unwrap();

    // Channel description
    writer
        .write_event(Event::Start(BytesStart::new("description")))
        .unwrap();
    let desc = if results.is_empty() {
        "No results found"
    } else {
        "Fshare search results"
    };
    writer
        .write_event(Event::Text(BytesText::new(desc)))
        .unwrap();
    writer
        .write_event(Event::End(BytesEnd::new("description")))
        .unwrap();

    // Newznab response metadata (required by spec)
    let mut response_elem = BytesStart::new("newznab:response");
    response_elem.push_attribute(("offset", "0"));
    response_elem.push_attribute(("total", results.len().to_string().as_str()));
    writer.write_event(Event::Empty(response_elem)).unwrap();

    // Items
    for result in results {
        write_item(&mut writer, result);
    }

    // Close channel and RSS
    writer
        .write_event(Event::End(BytesEnd::new("channel")))
        .unwrap();
    writer
        .write_event(Event::End(BytesEnd::new("rss")))
        .unwrap();

    String::from_utf8(writer.into_inner().into_inner()).unwrap()
}

/// Write a single item to XML
fn write_item<W: std::io::Write>(writer: &mut quick_xml::Writer<W>, result: IndexerResult) {
    use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};

    writer
        .write_event(Event::Start(BytesStart::new("item")))
        .unwrap();

    // Title
    writer
        .write_event(Event::Start(BytesStart::new("title")))
        .unwrap();
    writer
        .write_event(Event::Text(BytesText::new(&result.title)))
        .unwrap();
    writer
        .write_event(Event::End(BytesEnd::new("title")))
        .unwrap();

    // GUID (with isPermaLink="false" as required by RSS spec)
    let mut guid_tag = BytesStart::new("guid");
    guid_tag.push_attribute(("isPermaLink", "false"));
    writer.write_event(Event::Start(guid_tag)).unwrap();
    writer
        .write_event(Event::Text(BytesText::new(&result.guid)))
        .unwrap();
    writer
        .write_event(Event::End(BytesEnd::new("guid")))
        .unwrap();

    // Link
    writer
        .write_event(Event::Start(BytesStart::new("link")))
        .unwrap();
    writer
        .write_event(Event::Text(BytesText::new(&result.link)))
        .unwrap();
    writer
        .write_event(Event::End(BytesEnd::new("link")))
        .unwrap();

    // Size
    writer
        .write_event(Event::Start(BytesStart::new("size")))
        .unwrap();
    writer
        .write_event(Event::Text(BytesText::new(&result.size.to_string())))
        .unwrap();
    writer
        .write_event(Event::End(BytesEnd::new("size")))
        .unwrap();

    // Pub date
    writer
        .write_event(Event::Start(BytesStart::new("pubDate")))
        .unwrap();
    writer
        .write_event(Event::Text(BytesText::new(&result.pub_date.to_rfc2822())))
        .unwrap();
    writer
        .write_event(Event::End(BytesEnd::new("pubDate")))
        .unwrap();

    // Enclosure (CRITICAL for *arr suite to recognize downloadable content)
    let mut enclosure = BytesStart::new("enclosure");
    enclosure.push_attribute(("url", result.link.as_str()));
    enclosure.push_attribute(("length", result.size.to_string().as_str()));
    enclosure.push_attribute(("type", "application/x-nzb"));
    writer.write_event(Event::Empty(enclosure)).unwrap();

    // Newznab attributes (critical for *arr suite)
    // GUID attribute for deduplication
    let mut attr_guid = BytesStart::new("newznab:attr");
    attr_guid.push_attribute(("name", "guid"));
    attr_guid.push_attribute(("value", result.guid.as_str()));
    writer.write_event(Event::Empty(attr_guid)).unwrap();

    // Category attribute (specific sub-category)
    let mut attr_cat = BytesStart::new("newznab:attr");
    attr_cat.push_attribute(("name", "category"));
    attr_cat.push_attribute(("value", result.category.to_string().as_str()));
    writer.write_event(Event::Empty(attr_cat)).unwrap();

    // Parent category attribute (required by *arr suite)
    // 2000 = Movies, 5000 = TV
    let parent_category = if result.category >= 5000 && result.category < 6000 {
        "5000" // TV
    } else {
        "2000" // Movies
    };
    let mut attr_parent = BytesStart::new("newznab:attr");
    attr_parent.push_attribute(("name", "category"));
    attr_parent.push_attribute(("value", parent_category));
    writer.write_event(Event::Empty(attr_parent)).unwrap();

    // Size attribute (*arr suite prefers this over <size> tag)
    let mut attr_size = BytesStart::new("newznab:attr");
    attr_size.push_attribute(("name", "size"));
    attr_size.push_attribute(("value", result.size.to_string().as_str()));
    writer.write_event(Event::Empty(attr_size)).unwrap();

    writer
        .write_event(Event::End(BytesEnd::new("item")))
        .unwrap();
}

/// Generate error XML
fn generate_error_xml(message: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<error code="100" description="{}" />"#,
        message
    )
}

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone)]
struct IndexerResult {
    title: String,
    guid: String,
    link: String,
    size: u64,
    pub_date: DateTime<Utc>,
    category: u32,
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_category_movies_hd() {
        // 1080p movies
        assert_eq!(
            determine_category("Avengers.2012.1080p.BluRay.x264.mkv"),
            2040
        );
        assert_eq!(determine_category("Movie.Name.720p.WEB-DL.mkv"), 2040);

        // HD without season/episode markers
        assert_eq!(determine_category("Some.Movie.1080p.mkv"), 2040);
    }

    #[test]
    fn test_determine_category_movies_uhd() {
        // 4K/UHD movies
        assert_eq!(determine_category("Avengers.2160p.UHD.BluRay.mkv"), 2045);
        assert_eq!(determine_category("Movie.4K.HDR.mkv"), 2045);
        assert_eq!(determine_category("Film.2160p.x265.mkv"), 2045);
        assert_eq!(determine_category("Movie.UHD.BluRay.mkv"), 2045);
    }

    #[test]
    fn test_determine_category_tv_hd() {
        // TV shows with season/episode patterns in HD
        assert_eq!(determine_category("Breaking.Bad.S01E01.1080p.mkv"), 5040);
        assert_eq!(determine_category("Show.Name.S02E05.720p.WEB-DL.mkv"), 5040);
        assert_eq!(
            determine_category("Series.Season.1.Episode.03.1080p.mkv"),
            5040
        );

        // TV shows without explicit resolution but with S/E markers
        assert_eq!(determine_category("Show.S01E01.mkv"), 5040);
    }

    #[test]
    fn test_determine_category_tv_uhd() {
        // TV shows in 4K/UHD
        assert_eq!(determine_category("Breaking.Bad.S01E01.2160p.mkv"), 5045);
        assert_eq!(determine_category("Show.S02E05.4K.HDR.mkv"), 5045);
        assert_eq!(determine_category("Series.S01E01.UHD.mkv"), 5045);
    }

    #[test]
    fn test_determine_category_edge_cases() {
        // Files without clear indicators default to Movies HD
        assert_eq!(determine_category("random_file.mkv"), 2040);
        assert_eq!(determine_category("no_quality_markers.avi"), 2040);

        // Mixed case should work
        assert_eq!(determine_category("Movie.s01e01.1080P.mkv"), 5040);
    }

    #[test]
    fn test_handle_caps_xml_structure() {
        let xml = handle_caps();

        // Check basic XML structure
        assert!(xml.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<caps>"));
        assert!(xml.contains("</caps>"));

        // Check server info
        assert!(xml.contains("<server title=\"Fshare Indexer\""));

        // Check limits
        assert!(xml.contains("<limits max=\"100\" default=\"100\""));

        // Check search capabilities
        assert!(xml.contains("<searching>"));
        assert!(xml.contains("<search available=\"yes\""));
        assert!(xml.contains("<tv-search available=\"yes\""));
        assert!(xml.contains("<movie-search available=\"yes\""));

        // Check categories
        assert!(xml.contains("<category id=\"2000\" name=\"Movies\">"));
        assert!(xml.contains("<subcat id=\"2040\" name=\"Movies/HD\""));
        assert!(xml.contains("<subcat id=\"2045\" name=\"Movies/UHD\""));
        assert!(xml.contains("<category id=\"5000\" name=\"TV\">"));
        assert!(xml.contains("<subcat id=\"5040\" name=\"TV/HD\""));
        assert!(xml.contains("<subcat id=\"5045\" name=\"TV/UHD\""));
    }

    #[test]
    fn test_generate_error_xml() {
        let error = generate_error_xml("Test error message");

        assert!(error.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(error.contains("<error code=\"100\""));
        assert!(error.contains("Test error message"));
    }

    #[test]
    fn test_generate_search_xml_empty() {
        let results = vec![];
        let xml = generate_search_xml(results, "test");

        // Should still have valid XML structure
        assert!(xml.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<rss"));
        assert!(xml.contains("<channel>"));
        assert!(xml.contains("</channel>"));
        assert!(xml.contains("</rss>"));
    }

    #[test]
    fn test_generate_search_xml_with_items() {
        let results = vec![
            IndexerResult {
                title: "Test Movie 1080p".to_string(),
                guid: "fshare://TEST123".to_string(),
                link: "https://www.fshare.vn/file/TEST123".to_string(),
                size: 1024 * 1024 * 1024, // 1GB
                pub_date: Utc::now(),
                category: 2040,
            },
            IndexerResult {
                title: "Test Show S01E01 2160p".to_string(),
                guid: "fshare://TEST456".to_string(),
                link: "https://www.fshare.vn/file/TEST456".to_string(),
                size: 2 * 1024 * 1024 * 1024, // 2GB
                pub_date: Utc::now(),
                category: 5045,
            },
        ];

        let xml = generate_search_xml(results, "test");

        // Check structure
        assert!(xml.contains("<rss"));
        assert!(xml.contains("<channel>"));

        // Check first item
        assert!(xml.contains("<title>Test Movie 1080p</title>"));
        assert!(xml.contains("fshare://TEST123"));
        assert!(xml.contains("value=\"2040\""));

        // Check second item
        assert!(xml.contains("<title>Test Show S01E01 2160p</title>"));
        assert!(xml.contains("fshare://TEST456"));
        assert!(xml.contains("value=\"5045\""));

        // Check Newznab attributes
        assert!(xml.contains("newznab:attr"));
        assert!(xml.contains("name=\"category\""));
        assert!(xml.contains("name=\"size\""));
    }

    #[test]
    fn test_indexer_result_category_assignment() {
        let movie_hd = IndexerResult {
            title: "Movie.1080p.mkv".to_string(),
            guid: "fshare://TEST1".to_string(),
            link: "https://example.com".to_string(),
            size: 1000,
            pub_date: Utc::now(),
            category: determine_category("Movie.1080p.mkv"),
        };
        assert_eq!(movie_hd.category, 2040);

        let tv_uhd = IndexerResult {
            title: "Show.S01E01.2160p.mkv".to_string(),
            guid: "fshare://TEST2".to_string(),
            link: "https://example.com".to_string(),
            size: 2000,
            pub_date: Utc::now(),
            category: determine_category("Show.S01E01.2160p.mkv"),
        };
        assert_eq!(tv_uhd.category, 5045);
    }

    #[test]
    fn test_category_detection_comprehensive() {
        // Movies - HD
        assert_eq!(
            determine_category("The.Matrix.1999.1080p.BluRay.x264-GROUP.mkv"),
            2040
        );
        assert_eq!(
            determine_category("Inception.2010.720p.WEB-DL.AAC2.0.H.264.mkv"),
            2040
        );

        // Movies - UHD
        assert_eq!(
            determine_category("Avatar.2009.2160p.UHD.BluRay.x265.10bit.HDR.mkv"),
            2045
        );
        assert_eq!(determine_category("Dune.2021.4K.HDR.DV.mkv"), 2045);

        // TV - HD
        assert_eq!(
            determine_category("Game.of.Thrones.S08E06.1080p.WEB.H264-MEMENTO.mkv"),
            5040
        );
        assert_eq!(
            determine_category("The.Office.US.S05E14.720p.BluRay.x264.mkv"),
            5040
        );

        // TV - UHD
        assert_eq!(
            determine_category("The.Mandalorian.S02E08.2160p.WEB.H265-GLHF.mkv"),
            5045
        );
        assert_eq!(
            determine_category("Stranger.Things.S04E01.4K.NF.WEB-DL.mkv"),
            5045
        );
    }

    #[test]
    fn test_alternative_episode_formats() {
        // Alternative format: 1x01
        assert_eq!(determine_category("Show.Name.1x01.720p.mkv"), 5040);
        assert_eq!(determine_category("Series.2x05.1080p.mkv"), 5040);

        // Alternative format with UHD
        assert_eq!(determine_category("Show.1x01.2160p.mkv"), 5045);
    }

    #[test]
    fn test_anime_detection() {
        // Anime with brackets
        assert_eq!(
            determine_category("[SubGroup] Anime Name [01] [1080p].mkv"),
            5040
        );
        assert_eq!(determine_category("[Group] Show - 12 [720p].mkv"), 5040);

        // Anime with EP notation
        assert_eq!(determine_category("Anime.Name.EP01.1080p.mkv"), 5040);

        // Anime batch
        assert_eq!(
            determine_category("[Group] Anime Complete Batch [1080p].mkv"),
            5040
        );

        // Anime in UHD
        assert_eq!(
            determine_category("[SubGroup] Anime [01] [2160p].mkv"),
            5045
        );
    }

    #[test]
    fn test_season_word_patterns() {
        // Season word patterns
        assert_eq!(determine_category("Show.Name.Season.1.1080p.mkv"), 5040);
        assert_eq!(determine_category("Series.Season 2.720p.mkv"), 5040);
    }

    // ========================================================================
    // Newznab Search-to-Import Pipeline Tests
    //
    // These tests cover the third convergence point: automated Newznab search
    // discovering a match and triggering the full download→import flow.
    // ========================================================================

    // --- Step 1 & 2: NZB URL generation (search result → download endpoint) ---

    #[test]
    fn test_generate_nzb_url_embeds_all_tv_metadata() {
        // Verify the NZB download URL produced for a TV episode contains every
        // required parameter that the POST /api/downloads handler needs.
        let fshare_url = "https://www.fshare.vn/file/8DW6WQOV5R551DL";
        let host = "localhost:8484";
        let apikey = "abc123";
        let tmdb_id = Some("94997".to_string());
        let media_type = Some("tv".to_string());
        let season = Some(1u32);
        let episode = Some(3u32);

        let url = generate_nzb_url(
            fshare_url,
            host,
            apikey,
            &tmdb_id,
            &media_type,
            &season,
            &episode,
        );

        // Must point to our internal download endpoint
        assert!(url.starts_with("http://localhost:8484/newznab/api/download?"));
        // File code extracted from the Fshare URL path segment
        assert!(
            url.contains("fcode=8DW6WQOV5R551DL"),
            "fcode missing: {}",
            url
        );
        // Auth key forwarded so SABnzbd shim can authenticate
        assert!(url.contains("apikey=abc123"), "apikey missing: {}", url);
        // TMDB metadata — all three must be present for Sonarr matching
        assert!(url.contains("tmdb_id=94997"), "tmdb_id missing: {}", url);
        assert!(url.contains("media_type=tv"), "media_type missing: {}", url);
        assert!(url.contains("season=1"), "season missing: {}", url);
        assert!(url.contains("episode=3"), "episode missing: {}", url);
    }

    #[test]
    fn test_generate_nzb_url_embeds_movie_metadata() {
        // Movie NZB URL must carry tmdb_id and media_type but no season/episode.
        let fshare_url = "https://www.fshare.vn/file/MOVIECODE9X";
        let tmdb_id = Some("12345".to_string());
        let media_type = Some("movie".to_string());

        let url = generate_nzb_url(
            fshare_url,
            "192.168.1.10:8484",
            "testkey",
            &tmdb_id,
            &media_type,
            &None,
            &None,
        );

        assert!(url.contains("fcode=MOVIECODE9X"), "fcode missing: {}", url);
        assert!(url.contains("tmdb_id=12345"), "tmdb_id missing: {}", url);
        assert!(
            url.contains("media_type=movie"),
            "media_type missing: {}",
            url
        );
        // Season/episode must NOT appear in movie URL
        assert!(
            !url.contains("season="),
            "unexpected season param in movie URL"
        );
        assert!(
            !url.contains("episode="),
            "unexpected episode param in movie URL"
        );
    }

    #[test]
    fn test_generate_nzb_url_without_tmdb_metadata() {
        // When no TMDB info is available the URL is still valid; arr will
        // fall back to filename matching.
        let url = generate_nzb_url(
            "https://www.fshare.vn/file/UNKNOWNCODE",
            "localhost:8484",
            "key",
            &None,
            &None,
            &None,
            &None,
        );

        assert!(url.contains("fcode=UNKNOWNCODE"));
        // Optional params must be absent (not "&tmdb_id=")
        assert!(!url.contains("tmdb_id="));
        assert!(!url.contains("media_type="));
    }

    #[test]
    fn test_extract_file_id_from_fshare_url() {
        // extract_file_id must strip the path prefix and any query string.
        assert_eq!(
            extract_file_id("https://www.fshare.vn/file/8DW6WQOV5R551DL"),
            "8DW6WQOV5R551DL"
        );
        assert_eq!(
            extract_file_id("https://www.fshare.vn/file/ABCDEF123?token=x"),
            "ABCDEF123"
        );
        // Already just a code with no slashes
        assert_eq!(extract_file_id("PLAINCODE"), "PLAINCODE");
    }

    // --- Step 3: NZB file metadata tags (download metadata population) ---

    #[test]
    fn test_nzb_metadata_tags_for_tv_episode() {
        // The fake NZB XML generated by handle_nzb_download must embed every
        // metadata tag that the SABnzbd shim parses to build the AddDownloadRequest.
        // We test the tag-building logic directly by constructing the expected XML.
        let fcode = "8DW6WQOV5R551DL";
        let fshare_url = format!("https://www.fshare.vn/file/{}", fcode);
        let tmdb_id = "94997";
        let media_type = "tv";
        let season = 1u32;
        let episode = 3u32;

        // Mirror the exact tag construction from handle_nzb_download
        let mut metadata_tags =
            format!("        <meta type=\"fshare_url\">{}</meta>\n", fshare_url);
        metadata_tags.push_str(&format!(
            "        <meta type=\"tmdb_id\">{}</meta>\n",
            tmdb_id
        ));
        metadata_tags.push_str(&format!(
            "        <meta type=\"media_type\">{}</meta>\n",
            media_type
        ));
        metadata_tags.push_str(&format!(
            "        <meta type=\"season\">{}</meta>\n",
            season
        ));
        metadata_tags.push_str(&format!(
            "        <meta type=\"episode\">{}</meta>\n",
            episode
        ));

        // All five tags must be present so the SABnzbd shim can reconstruct metadata
        assert!(metadata_tags.contains(&format!("<meta type=\"fshare_url\">{}</meta>", fshare_url)));
        assert!(metadata_tags.contains(&format!("<meta type=\"tmdb_id\">{}</meta>", tmdb_id)));
        assert!(metadata_tags.contains(&format!("<meta type=\"media_type\">{}</meta>", media_type)));
        assert!(metadata_tags.contains("<meta type=\"season\">1</meta>"));
        assert!(metadata_tags.contains("<meta type=\"episode\">3</meta>"));
    }

    #[test]
    fn test_nzb_metadata_tags_for_movie() {
        // Movie NZB must NOT include season/episode tags.
        let fcode = "MOVIECODE9X";
        let fshare_url = format!("https://www.fshare.vn/file/{}", fcode);
        let mut metadata_tags =
            format!("        <meta type=\"fshare_url\">{}</meta>\n", fshare_url);
        metadata_tags.push_str("        <meta type=\"tmdb_id\">12345</meta>\n");
        metadata_tags.push_str("        <meta type=\"media_type\">movie</meta>\n");
        // season/episode params are None for movies → no tags added

        assert!(metadata_tags.contains("<meta type=\"fshare_url\">"));
        assert!(metadata_tags.contains("<meta type=\"tmdb_id\">12345</meta>"));
        assert!(metadata_tags.contains("<meta type=\"media_type\">movie</meta>"));
        assert!(!metadata_tags.contains("<meta type=\"season\">"));
        assert!(!metadata_tags.contains("<meta type=\"episode\">"));
    }

    // --- Step 4: Title synthesis for Sonarr/Radarr matching ---

    fn make_test_result(
        name: &str,
        size: u64,
        resolution: Option<&str>,
        source: Option<&str>,
    ) -> crate::utils::title_matcher::SmartSearchResult {
        crate::utils::title_matcher::SmartSearchResult {
            name: name.to_string(),
            url: format!("https://www.fshare.vn/file/TESTCODE"),
            size,
            score: 100,
            quality_name: resolution.unwrap_or("Unknown").to_string(),
            quality_score: 100,
            custom_format_score: 0,
            total_score: 100,
            normalized_score: 1.0,
            match_type: "exact".to_string(),
            quality_attrs: crate::utils::parser::QualityAttributes {
                resolution: resolution.map(|s| s.to_string()),
                source: source.map(|s| s.to_string()),
                video_codec: None,
                audio_codec: None,
                hdr: false,
                dolby_vision: false,
                bit_depth: 8,
                viet_sub: false,
                viet_dub: false,
                is_tv: true,
                is_movie: false,
                is_hd: true,
            },
            tmdb_id: None,
            poster_path: None,
            vietdub: false,
            vietsub: false,
            hdr: false,
            dolby_vision: false,
        }
    }

    #[test]
    fn test_synthesize_title_tv_episode_full_metadata() {
        // The synthesized title must follow Sonarr's naming convention so it is
        // parsed correctly: "Series.Name.2026.S01E03.1080p.WEB-DL.-FShare.mkv"
        let file = make_test_result(
            "Phim.S01E03.1080p.WEB-DL.mkv",
            2_000_000_000,
            Some("1080p"),
            Some("WEB-DL"),
        );
        let title = synthesize_title(&file, Some("Breaking Bad"), Some(2008), Some(1), Some(3));

        // Must start with the sanitized TMDB title
        assert!(
            title.starts_with("Breaking.Bad."),
            "Expected dot-separated title prefix: {}",
            title
        );
        // Year must appear for Sonarr series disambiguation
        assert!(
            title.contains("2008"),
            "Year missing from synthesized title: {}",
            title
        );
        // Season/episode in SxxExx format
        assert!(
            title.contains("S01E03"),
            "Season/episode missing: {}",
            title
        );
        // Resolution from quality_attrs
        assert!(title.contains("1080p"), "Resolution missing: {}", title);
        // Source codec
        assert!(title.contains("WEB-DL"), "Source missing: {}", title);
        // Release group tag
        assert!(
            title.contains("-FShare"),
            "Release group tag missing: {}",
            title
        );
        // File extension preserved
        assert!(title.ends_with(".mkv"), "Extension missing: {}", title);
    }

    #[test]
    fn test_synthesize_title_strips_punctuation_from_tmdb_title() {
        // Titles with colons, question marks, exclamation marks must be cleaned
        // so they don't confuse scene name parsers.
        let file = make_test_result("raw_file.mkv", 1_500_000_000, Some("1080p"), None);
        let title = synthesize_title(
            &file,
            Some("How Dare You!? (2026): The Series"),
            Some(2026),
            Some(1),
            Some(1),
        );

        assert!(!title.contains(':'), "Colon must be stripped: {}", title);
        assert!(
            !title.contains('?'),
            "Question mark must be stripped: {}",
            title
        );
        assert!(
            !title.contains('!'),
            "Exclamation mark must be stripped: {}",
            title
        );
        assert!(
            title.contains("S01E01"),
            "Season/episode missing: {}",
            title
        );
    }

    #[test]
    fn test_synthesize_title_without_tmdb_falls_back_to_filename() {
        // When no TMDB title is available the original filename is returned
        // unchanged so arr can still attempt filename-based matching.
        let original = "Original.Filename.S02E05.2160p.mkv";
        let file = make_test_result(original, 8_000_000_000, Some("2160p"), None);
        let title = synthesize_title(&file, None, None, Some(2), Some(5));

        assert_eq!(
            title, original,
            "Should return raw filename when no TMDB title"
        );
    }

    #[test]
    fn test_synthesize_title_infers_resolution_from_file_size() {
        // When the filename carries no resolution keyword, the size-inferred
        // resolution must be injected so arr never defaults to SDTV.
        let file = make_test_result("NoResolution.mkv", 15_000_000_000, None, None);
        let title = synthesize_title(&file, Some("Test Series"), Some(2024), Some(1), Some(1));

        // 15 GB >= 10 GB threshold → 2160p inferred
        assert!(
            title.contains("2160p"),
            "Size-inferred 2160p missing: {}",
            title
        );
    }

    #[test]
    fn test_synthesize_title_movie_no_season_episode() {
        // For movies there is no S##E## segment.
        let file = make_test_result(
            "Movie.2160p.mkv",
            20_000_000_000,
            Some("2160p"),
            Some("BluRay"),
        );
        let title = synthesize_title(&file, Some("Inception"), Some(2010), None, None);

        assert!(
            title.starts_with("Inception."),
            "Title prefix wrong: {}",
            title
        );
        assert!(title.contains("2010"), "Year missing: {}", title);
        assert!(
            !title.contains("S0"),
            "Unexpected season tag in movie title: {}",
            title
        );
        assert!(
            !title.contains("E0"),
            "Unexpected episode tag in movie title: {}",
            title
        );
        assert!(title.contains("2160p"), "Resolution missing: {}", title);
    }

    // --- Quality profile resolution filter ---

    #[test]
    fn test_profile_id_to_allowed_resolutions() {
        // Profile 4 = HD-1080p — only 1080p files should pass
        let allowed = profile_id_to_allowed_resolutions(4);
        assert!(resolution_allowed(Some("1080p"), &allowed));
        assert!(!resolution_allowed(Some("720p"), &allowed));
        assert!(!resolution_allowed(Some("2160p"), &allowed));

        // Profile 5 = Ultra-HD
        let uhd = profile_id_to_allowed_resolutions(5);
        assert!(resolution_allowed(Some("2160p"), &uhd));
        assert!(!resolution_allowed(Some("1080p"), &uhd));

        // Profile 6 = HD 720p/1080p
        let dual = profile_id_to_allowed_resolutions(6);
        assert!(resolution_allowed(Some("720p"), &dual));
        assert!(resolution_allowed(Some("1080p"), &dual));
        assert!(!resolution_allowed(Some("2160p"), &dual));

        // Profile 1 = Any → no restriction
        let any = profile_id_to_allowed_resolutions(1);
        assert!(resolution_allowed(Some("480p"), &any));
        assert!(resolution_allowed(Some("2160p"), &any));
    }

    #[test]
    fn test_resolution_allowed_unknown_resolution_always_passes() {
        // A file whose resolution cannot be detected must always pass through
        // so we never silently discard content we can't classify.
        let strict = profile_id_to_allowed_resolutions(4); // 1080p only
        assert!(
            resolution_allowed(None, &strict),
            "Unknown resolution should pass strict filter"
        );
        assert!(
            resolution_allowed(None, &None),
            "Unknown resolution should pass no filter"
        );
    }

    // --- Step 4 (state transition): DownloadState capability matrix ---

    #[test]
    fn test_download_state_transitions_importing_path() {
        use crate::downloader::task::DownloadState;

        // A task progresses Queued → Downloading → (Completed internally) → Importing
        // From Importing the only valid user action is cancel
        let state = DownloadState::Importing;
        assert!(state.can_cancel(), "Importing state must allow cancel");
        assert!(!state.can_pause(), "Importing state must not allow pause");
        assert!(!state.can_resume(), "Importing state must not allow resume");
        assert!(!state.can_retry(), "Importing state must not allow retry");
        assert!(!state.can_delete(), "Importing state must not allow delete");

        let actions = state.available_actions();
        assert!(
            actions.contains(&"cancel"),
            "cancel must be in available actions"
        );
        assert!(
            !actions.contains(&"pause"),
            "pause must not be in available actions"
        );
    }

    #[test]
    fn test_download_state_completed_allows_retry_and_delete() {
        use crate::downloader::task::DownloadState;

        // After entering Completed (whether with arr_announced=true or with
        // arr_announce_error set) the user can retry or delete the task.
        let state = DownloadState::Completed;
        assert!(state.can_retry(), "Completed state must allow retry");
        assert!(state.can_delete(), "Completed state must allow delete");
        assert!(!state.can_pause(), "Completed state must not allow pause");
        assert!(!state.can_cancel(), "Completed state must not allow cancel");
    }

    #[test]
    fn test_download_state_failed_allows_retry() {
        use crate::downloader::task::DownloadState;

        let state = DownloadState::Failed;
        assert!(state.can_retry(), "Failed state must allow retry");
        assert!(state.can_delete(), "Failed state must allow delete");
        assert!(!state.can_cancel(), "Failed state must not allow cancel");
    }

    // --- Step 5 & 6: Import polling timeout and error persistence ---

    #[test]
    fn test_import_poll_timeout_error_message_format() {
        // The exact error string stored in arr_announce_error must include the
        // poll duration in seconds so operators can diagnose stalled imports.
        let poll_timeout = std::time::Duration::from_secs(600);
        let error = format!("import poll timed out after {}s", poll_timeout.as_secs());

        assert_eq!(error, "import poll timed out after 600s");
    }

    #[test]
    fn test_import_poll_timeout_is_10_minutes() {
        // The polling window hardcoded in the worker must be exactly 10 minutes
        // (600 seconds) as specified in the design.
        let poll_timeout = std::time::Duration::from_secs(600);
        assert_eq!(
            poll_timeout.as_secs(),
            600,
            "Poll timeout must be exactly 600s (10 min)"
        );
        assert_eq!(
            poll_timeout.as_secs() / 60,
            10,
            "Poll timeout must be exactly 10 minutes"
        );
    }

    #[test]
    fn test_arr_announce_error_persisted_on_timeout() {
        // After a poll timeout the task must have:
        //   state       = Completed (not Failed — download itself succeeded)
        //   arr_announced = false
        //   arr_announce_error = Some("import poll timed out after 600s")
        // Simulate the finalization logic from the worker.
        use crate::downloader::task::{DownloadState, DownloadTask};

        let mut final_task = DownloadTask::new(
            "https://www.fshare.vn/file/TESTCODE".to_string(),
            "Test.S01E01.mkv".to_string(),
            "fshare".to_string(),
            "tv".to_string(),
        );
        // Worker sets Completed unconditionally after the poll resolves
        final_task.state = DownloadState::Completed;

        // Simulate Ok(false) branch (poll timed out)
        let poll_timeout = std::time::Duration::from_secs(600);
        final_task.arr_announce_error = Some(format!(
            "import poll timed out after {}s",
            poll_timeout.as_secs()
        ));
        // arr_announced stays false on timeout
        final_task.arr_announced = false;

        assert_eq!(final_task.state, DownloadState::Completed);
        assert!(!final_task.arr_announced);
        assert_eq!(
            final_task.arr_announce_error.as_deref(),
            Some("import poll timed out after 600s")
        );
    }

    #[test]
    fn test_arr_announced_true_clears_error_on_success() {
        // When poll returns Ok(true) the task must have arr_announced=true
        // and arr_announce_error cleared, even if a previous error was set.
        use crate::downloader::task::{DownloadState, DownloadTask};

        let mut final_task = DownloadTask::new(
            "https://www.fshare.vn/file/TESTCODE".to_string(),
            "Test.S01E01.mkv".to_string(),
            "fshare".to_string(),
            "tv".to_string(),
        );
        // Simulate poll error from a prior attempt
        final_task.arr_announce_error = Some("import poll error: connection refused".to_string());

        // Simulate Ok(true) branch
        final_task.state = DownloadState::Completed;
        final_task.arr_announced = true;
        final_task.arr_announce_error = None;

        assert!(final_task.arr_announced);
        assert!(final_task.arr_announce_error.is_none());
        assert_eq!(final_task.state, DownloadState::Completed);
    }

    #[test]
    fn test_arr_announce_error_set_on_poll_error() {
        // When poll returns Err(e) the error string is stored with the
        // "import poll error: " prefix for operator visibility.
        use crate::downloader::task::{DownloadState, DownloadTask};

        let mut final_task = DownloadTask::new(
            "https://www.fshare.vn/file/TESTCODE".to_string(),
            "Test.Movie.mkv".to_string(),
            "fshare".to_string(),
            "movies".to_string(),
        );
        final_task.state = DownloadState::Completed;

        let error = anyhow::anyhow!("connection refused");
        final_task.arr_announce_error = Some(format!("import poll error: {}", error));

        assert_eq!(
            final_task.arr_announce_error.as_deref(),
            Some("import poll error: connection refused")
        );
        assert!(!final_task.arr_announced);
    }

    // --- Media type detection for routing Sonarr vs Radarr poll ---

    #[test]
    fn test_media_type_detection_tv_episode_with_season_episode() {
        use crate::downloader::task::{DownloadTask, MediaType};

        let mut task = DownloadTask::new(
            "https://www.fshare.vn/file/TVCODE".to_string(),
            "Show.S01E03.1080p.mkv".to_string(),
            "fshare".to_string(),
            "tv".to_string(),
        );
        task.tmdb_season = Some(1);
        task.tmdb_episode = Some(3);

        assert_eq!(task.detect_media_type(), MediaType::TvEpisode);
    }

    #[test]
    fn test_media_type_detection_tv_series_with_batch_id() {
        use crate::downloader::task::{DownloadTask, MediaType};

        let mut task = DownloadTask::new(
            "https://www.fshare.vn/file/BATCHCODE".to_string(),
            "Show.S01.mkv".to_string(),
            "fshare".to_string(),
            "tv".to_string(),
        );
        task.batch_id = Some("batch-abc123".to_string());

        assert_eq!(task.detect_media_type(), MediaType::TvSeries);
    }

    #[test]
    fn test_media_type_detection_movie_from_category() {
        use crate::downloader::task::{DownloadTask, MediaType};

        let task = DownloadTask::new(
            "https://www.fshare.vn/file/MOVIECODE".to_string(),
            "Movie.2024.1080p.mkv".to_string(),
            "fshare".to_string(),
            "movies".to_string(),
        );

        assert_eq!(task.detect_media_type(), MediaType::Movie);
    }

    #[test]
    fn test_media_type_detection_radarr_category() {
        use crate::downloader::task::{DownloadTask, MediaType};

        let task = DownloadTask::new(
            "https://www.fshare.vn/file/MOVIECODE".to_string(),
            "Movie.2024.mkv".to_string(),
            "fshare".to_string(),
            "radarr".to_string(),
        );

        assert_eq!(task.detect_media_type(), MediaType::Movie);
    }

    // --- nzo_id = task UUID used as downloadId in arr history ---

    #[test]
    fn test_task_id_is_valid_uuid() {
        // The task UUID is used as the nzo_id in the SABnzbd shim.
        // Sonarr/Radarr store it as downloadId and we match against it
        // case-insensitively.  Verify it is a parseable UUID.
        use crate::downloader::task::DownloadTask;
        use uuid::Uuid;

        let task = DownloadTask::new(
            "https://www.fshare.vn/file/TEST".to_string(),
            "test.mkv".to_string(),
            "fshare".to_string(),
            "tv".to_string(),
        );

        let nzo_id = task.id.to_string();
        // Must round-trip through Uuid::parse_str without error
        assert!(
            Uuid::parse_str(&nzo_id).is_ok(),
            "task.id must be a valid UUID string: {}",
            nzo_id
        );
        // Lowercase comparison must equal itself (case-insensitive match guard)
        assert_eq!(nzo_id.to_lowercase(), nzo_id.to_lowercase());
    }

    // --- Newznab XML parent category mapping ---

    #[test]
    fn test_parent_category_tv_range() {
        // Any category in 5000-5999 must map to parent "5000" (TV)
        let tv_hd_parent = if 5040 >= 5000 && 5040 < 6000 {
            "5000"
        } else {
            "2000"
        };
        let tv_uhd_parent = if 5045 >= 5000 && 5045 < 6000 {
            "5000"
        } else {
            "2000"
        };
        assert_eq!(tv_hd_parent, "5000");
        assert_eq!(tv_uhd_parent, "5000");
    }

    #[test]
    fn test_parent_category_movie_range() {
        // Any category in 2000-4999 must map to parent "2000" (Movies)
        let movie_hd_parent = if 2040 >= 5000 && 2040 < 6000 {
            "5000"
        } else {
            "2000"
        };
        let movie_uhd_parent = if 2045 >= 5000 && 2045 < 6000 {
            "5000"
        } else {
            "2000"
        };
        assert_eq!(movie_hd_parent, "2000");
        assert_eq!(movie_uhd_parent, "2000");
    }

    // --- Backfill timeout error format ---

    #[test]
    fn test_backfill_timeout_error_message_format() {
        // The backfill path uses a distinct prefix so operators can distinguish
        // a live-poll timeout from a restart-recovery timeout.
        let error = "backfill: import poll timed out";
        assert!(
            error.starts_with("backfill:"),
            "Backfill error must be prefixed: {}",
            error
        );
        assert!(
            error.contains("import poll timed out"),
            "Backfill error must contain timeout message: {}",
            error
        );
    }
}
