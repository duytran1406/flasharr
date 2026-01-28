//! Newznab/Torznab Indexer API
//!
//! Provides Newznab-compatible endpoints for integration with Sonarr/Radarr.
//! This allows *arr applications to search for content on Fshare.

use axum::{
    routing::get,
    Router,
    extract::{State, Query},
    http::StatusCode,
};
use std::sync::Arc;
use serde::Deserialize;
use crate::AppState;
use chrono::{DateTime, Utc};
use moka::future::Cache;
use std::time::Duration;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(handle_indexer))
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
    
    /// IMDB ID (for movies)
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
    Query(params): Query<IndexerParams>,
) -> (StatusCode, String) {
    match params.t.as_str() {
        "caps" => (StatusCode::OK, handle_caps()),
        "search" => {
            if !validate_api_key(&state, &params.apikey) {
                return (StatusCode::UNAUTHORIZED, generate_error_xml("Invalid API key"));
            }
            (StatusCode::OK, handle_search(state, params).await)
        },
        "tvsearch" => {
            if !validate_api_key(&state, &params.apikey) {
                return (StatusCode::UNAUTHORIZED, generate_error_xml("Invalid API key"));
            }
            (StatusCode::OK, handle_tv_search(state, params).await)
        },
        "movie" => {
            if !validate_api_key(&state, &params.apikey) {
                return (StatusCode::UNAUTHORIZED, generate_error_xml("Invalid API key"));
            }
            (StatusCode::OK, handle_movie_search(state, params).await)
        },
        _ => (StatusCode::BAD_REQUEST, generate_error_xml("Unknown function")),
    }
}

/// Handle capabilities request
fn handle_caps() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<caps>
  <server title="Fshare Indexer" version="1.0" />
  <limits max="100" default="100" />
  <searching>
    <search available="yes" supportedParams="q" />
    <tv-search available="yes" supportedParams="q,season,ep" />
    <movie-search available="yes" supportedParams="q,imdbid" />
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
</caps>"#.to_string()
}

/// Handle general search
async fn handle_search(
    state: Arc<AppState>,
    params: IndexerParams,
) -> String {
    let query = match params.q {
        Some(q) if !q.is_empty() => q,
        _ => return generate_error_xml("Missing search query"),
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

/// Handle TV search
async fn handle_tv_search(
    state: Arc<AppState>,
    params: IndexerParams,
) -> String {
    let query = match params.q {
        Some(q) if !q.is_empty() => q,
        _ => return generate_error_xml("Missing search query"),
    };
    
    // Build TV-specific query
    let tv_query = if let (Some(season), Some(ep)) = (params.season, params.ep) {
        format!("{} S{:02}E{:02}", query, season, ep)
    } else if let Some(season) = params.season {
        format!("{} Season {}", query, season)
    } else {
        query.clone()
    };
    
    // Check cache
    let cache = get_search_cache();
    let cache_key = format!("tv:{}", tv_query);
    
    let results = if let Some(cached) = cache.get(&cache_key).await {
        cached
    } else {
        let results = execute_fshare_search_for_indexer(&state, &tv_query).await;
        cache.insert(cache_key, results.clone()).await;
        results
    };
    
    generate_search_xml(results, &query)
}

/// Handle movie search
async fn handle_movie_search(
    state: Arc<AppState>,
    params: IndexerParams,
) -> String {
    let query = match params.q {
        Some(q) if !q.is_empty() => q,
        _ => return generate_error_xml("Missing search query"),
    };
    
    // Check cache
    let cache = get_search_cache();
    let cache_key = format!("movie:{}", query);
    
    let results = if let Some(cached) = cache.get(&cache_key).await {
        cached
    } else {
        let results = execute_fshare_search_for_indexer(&state, &query).await;
        cache.insert(cache_key, results.clone()).await;
        results
    };
    
    generate_search_xml(results, &query)
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Validate API key
fn validate_api_key(state: &AppState, provided: &str) -> bool {
    // Get API key from config
    let config_key = state.config.indexer.as_ref()
        .map(|i| i.api_key.as_str())
        .unwrap_or("flasharr-default-key");
    
    !provided.is_empty() && provided == config_key
}

/// Execute Fshare search and convert to indexer results
async fn execute_fshare_search_for_indexer(
    _state: &AppState,
    query: &str,
) -> Vec<IndexerResult> {
    use reqwest::Client;
    
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| Client::new());
    
    // Use timfshare API directly (same as search.rs)
    let url = format!("https://timfshare.com/api/v1/string-query-search?query={}", urlencoding::encode(query.trim()));
    
    let resp = client.post(&url)
        .header("Content-Length", "0")
        .header("Origin", "https://timfshare.com")
        .header("Referer", format!("https://timfshare.com/search?key={}", urlencoding::encode(query)))
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await;
    
    match resp {
        Ok(r) => {
            if let Ok(data) = r.json::<serde_json::Value>().await {
                if let Some(arr) = data["data"].as_array() {
                    return arr.iter().take(100).filter_map(|item| {
                        let name = item["name"].as_str()?.to_string();
                        let url = item["url"].as_str()?.to_string();
                        let fcode = url.split("/file/").last()?.to_string();
                        let size = item["size"].as_u64().unwrap_or(0);
                        
                        Some(IndexerResult {
                            title: name.clone(),
                            guid: format!("fshare://{}", fcode),
                            link: url,
                            size,
                            pub_date: Utc::now(),
                            category: determine_category(&name),
                        })
                    }).collect();
                }
            }
        },
        Err(_) => {}
    }
    
    Vec::new()
}

/// Determine category from filename with enhanced pattern matching
fn determine_category(filename: &str) -> u32 {
    use regex::Regex;
    use once_cell::sync::Lazy;
    
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
    let is_anime = ANIME_PATTERNS.iter().any(|re| re.is_match(&lower)) ||
                   lower.contains("anime") ||
                   lower.contains("アニメ") ||
                   lower.contains("[") && lower.contains("]") && 
                   (lower.contains("1080p") || lower.contains("720p") || lower.contains("2160p"));
    
    // Check for TV show patterns
    let is_tv = TV_PATTERNS.iter().any(|re| re.is_match(&lower));
    
    // Check for resolution
    let is_uhd = lower.contains("2160p") || 
                 lower.contains("4k") || 
                 lower.contains("uhd") ||
                 lower.contains("2k");
    
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
    use quick_xml::events::{Event, BytesStart, BytesText, BytesEnd};
    use quick_xml::Writer;
    use std::io::Cursor;
    
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    
    // XML declaration
    writer.write_event(Event::Decl(quick_xml::events::BytesDecl::new("1.0", Some("UTF-8"), None))).unwrap();
    
    // RSS root
    let mut rss = BytesStart::new("rss");
    rss.push_attribute(("version", "2.0"));
    rss.push_attribute(("xmlns:atom", "http://www.w3.org/2005/Atom"));
    rss.push_attribute(("xmlns:newznab", "http://www.newznab.com/DTD/2010/feeds/attributes/"));
    writer.write_event(Event::Start(rss)).unwrap();
    
    // Channel
    writer.write_event(Event::Start(BytesStart::new("channel"))).unwrap();
    
    // Channel title
    writer.write_event(Event::Start(BytesStart::new("title"))).unwrap();
    writer.write_event(Event::Text(BytesText::new("Fshare Indexer"))).unwrap();
    writer.write_event(Event::End(BytesEnd::new("title"))).unwrap();
    
    // Items
    for result in results {
        write_item(&mut writer, result);
    }
    
    // Close channel and RSS
    writer.write_event(Event::End(BytesEnd::new("channel"))).unwrap();
    writer.write_event(Event::End(BytesEnd::new("rss"))).unwrap();
    
    String::from_utf8(writer.into_inner().into_inner()).unwrap()
}

/// Write a single item to XML
fn write_item<W: std::io::Write>(writer: &mut quick_xml::Writer<W>, result: IndexerResult) {
    use quick_xml::events::{Event, BytesStart, BytesText, BytesEnd};
    
    writer.write_event(Event::Start(BytesStart::new("item"))).unwrap();
    
    // Title
    writer.write_event(Event::Start(BytesStart::new("title"))).unwrap();
    writer.write_event(Event::Text(BytesText::new(&result.title))).unwrap();
    writer.write_event(Event::End(BytesEnd::new("title"))).unwrap();
    
    // GUID
    writer.write_event(Event::Start(BytesStart::new("guid"))).unwrap();
    writer.write_event(Event::Text(BytesText::new(&result.guid))).unwrap();
    writer.write_event(Event::End(BytesEnd::new("guid"))).unwrap();
    
    // Link
    writer.write_event(Event::Start(BytesStart::new("link"))).unwrap();
    writer.write_event(Event::Text(BytesText::new(&result.link))).unwrap();
    writer.write_event(Event::End(BytesEnd::new("link"))).unwrap();
    
    // Size
    writer.write_event(Event::Start(BytesStart::new("size"))).unwrap();
    writer.write_event(Event::Text(BytesText::new(&result.size.to_string()))).unwrap();
    writer.write_event(Event::End(BytesEnd::new("size"))).unwrap();
    
    // Pub date
    writer.write_event(Event::Start(BytesStart::new("pubDate"))).unwrap();
    writer.write_event(Event::Text(BytesText::new(&result.pub_date.to_rfc2822()))).unwrap();
    writer.write_event(Event::End(BytesEnd::new("pubDate"))).unwrap();
    
    // Newznab attributes
    let mut attr_cat = BytesStart::new("newznab:attr");
    attr_cat.push_attribute(("name", "category"));
    attr_cat.push_attribute(("value", result.category.to_string().as_str()));
    writer.write_event(Event::Empty(attr_cat)).unwrap();
    
    let mut attr_size = BytesStart::new("newznab:attr");
    attr_size.push_attribute(("name", "size"));
    attr_size.push_attribute(("value", result.size.to_string().as_str()));
    writer.write_event(Event::Empty(attr_size)).unwrap();
    
    writer.write_event(Event::End(BytesEnd::new("item"))).unwrap();
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
        assert_eq!(determine_category("Avengers.2012.1080p.BluRay.x264.mkv"), 2040);
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
        assert_eq!(determine_category("Series.Season.1.Episode.03.1080p.mkv"), 5040);
        
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
        assert!(xml.contains("<guid>fshare://TEST123</guid>"));
        assert!(xml.contains("value=\"2040\""));
        
        // Check second item
        assert!(xml.contains("<title>Test Show S01E01 2160p</title>"));
        assert!(xml.contains("<guid>fshare://TEST456</guid>"));
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
        assert_eq!(determine_category("The.Matrix.1999.1080p.BluRay.x264-GROUP.mkv"), 2040);
        assert_eq!(determine_category("Inception.2010.720p.WEB-DL.AAC2.0.H.264.mkv"), 2040);
        
        // Movies - UHD
        assert_eq!(determine_category("Avatar.2009.2160p.UHD.BluRay.x265.10bit.HDR.mkv"), 2045);
        assert_eq!(determine_category("Dune.2021.4K.HDR.DV.mkv"), 2045);
        
        // TV - HD
        assert_eq!(determine_category("Game.of.Thrones.S08E06.1080p.WEB.H264-MEMENTO.mkv"), 5040);
        assert_eq!(determine_category("The.Office.US.S05E14.720p.BluRay.x264.mkv"), 5040);
        
        // TV - UHD
        assert_eq!(determine_category("The.Mandalorian.S02E08.2160p.WEB.H265-GLHF.mkv"), 5045);
        assert_eq!(determine_category("Stranger.Things.S04E01.4K.NF.WEB-DL.mkv"), 5045);
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
        assert_eq!(determine_category("[SubGroup] Anime Name [01] [1080p].mkv"), 5040);
        assert_eq!(determine_category("[Group] Show - 12 [720p].mkv"), 5040);
        
        // Anime with EP notation
        assert_eq!(determine_category("Anime.Name.EP01.1080p.mkv"), 5040);
        
        // Anime batch
        assert_eq!(determine_category("[Group] Anime Complete Batch [1080p].mkv"), 5040);
        
        // Anime in UHD
        assert_eq!(determine_category("[SubGroup] Anime [01] [2160p].mkv"), 5045);
    }
    
    #[test]
    fn test_season_word_patterns() {
        // Season word patterns
        assert_eq!(determine_category("Show.Name.Season.1.1080p.mkv"), 5040);
        assert_eq!(determine_category("Series.Season 2.720p.mkv"), 5040);
    }
}



