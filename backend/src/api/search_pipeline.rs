//! Search Pipeline Module
//!
//! Contains reusable search components extracted from smart_search.rs.
//! Provides building blocks for TV and Movie search handlers.

use moka::future::Cache;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, warn};

/// Allowed media file extensions (lowercase, without dot)
pub const MEDIA_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "mov", "wmv", "flv", "webm", "m4v",
    "mpg", "mpeg", "m2ts", "vob", "3gp", "ogv", "divx", "rmvb",
    "rar", "zip", "7z",  // archives that often contain media
];

/// Check if a filename has a media file extension.
/// Returns true for video files and common archives, false for .ts, .iso, .nfo, .srt, .txt, etc.
pub fn is_media_file(filename: &str) -> bool {
    if let Some(dot_pos) = filename.rfind('.') {
        let ext = &filename[dot_pos + 1..].to_lowercase();
        MEDIA_EXTENSIONS.contains(&ext.as_str())
    } else {
        // No extension — assume it could be media (folder links, etc.)
        true
    }
}

/// Raw search result from TimFshare API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFshareResult {
    pub name: String,
    pub url: String,
    pub fcode: String,
    pub size: u64,
    pub score: i32,
}

/// Search pipeline utilities
pub struct SearchPipeline;

impl SearchPipeline {
    /// Execute search via timfshare.com API with moka cache layer.
    /// Cache key is the normalized query string. Hits skip the network entirely.
    pub async fn execute_fshare_search_cached(
        client: &Client,
        query: &str,
        limit: usize,
        cache: &Cache<String, Vec<RawFshareResult>>,
    ) -> Vec<RawFshareResult> {
        let cache_key = query.trim().to_lowercase();

        // Check cache first
        if let Some(cached) = cache.get(&cache_key).await {
            info!("TimFshare CACHE HIT: '{}' ({} results)", query, cached.len());
            // Apply limit on cached results
            return cached.into_iter().take(limit).collect();
        }

        // Cache miss — fetch from network
        let results = Self::execute_fshare_search(client, query, limit).await;

        // Store in cache (full results, limit applied on read)
        if !results.is_empty() {
            cache.insert(cache_key, results.clone()).await;
        }

        results
    }

    /// Execute search via timfshare.com API (uncached, for snowball/targeted queries)
    pub async fn execute_fshare_search(client: &Client, query: &str, limit: usize) -> Vec<RawFshareResult> {
        let url = format!("https://timfshare.com/api/v1/string-query-search?query={}", urlencoding::encode(query));
        let mut results = Vec::new();

        info!("Executing TimFshare search: '{}'", query);
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
                        info!("TimFshare search '{}' returned {} results", query, arr.len());
                        for item in arr.iter().take(limit) {
                            let name = item["name"].as_str().unwrap_or("Unknown").to_string();
                            let f_url = item["url"].as_str().unwrap_or("").to_string();
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
                        warn!("TimFshare search '{}' returned no data array", query);
                    }
                } else {
                    warn!("TimFshare search '{}' returned invalid JSON", query);
                }
            },
            Err(e) => {
                warn!("TimFshare search '{}' request failed: {}", query, e);
            }
        }
        // Filter out non-media files (.ts, .iso, .nfo, .srt, .txt, etc.)
        results.retain(|r| is_media_file(&r.name));
        results
    }

    /// Deduplicate results by fcode
    #[allow(dead_code)]
    pub fn deduplicate_by_fcode(results: Vec<RawFshareResult>) -> Vec<RawFshareResult> {
        let mut seen = std::collections::HashSet::new();
        results.into_iter()
            .filter(|r| {
                let pure_fcode = r.fcode.split('?').next().unwrap_or(&r.fcode);
                seen.insert(pure_fcode.to_string())
            })
            .collect()
    }
}
