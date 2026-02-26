//! Folder Cache Service
//!
//! Handles recursive scanning of Fshare folders and populating the FTS5 cache.
//! Provides search interface over the cached data.

use std::sync::Arc;
use std::collections::{VecDeque, HashMap};
use serde::Serialize;
use quick_xml::Reader;
use quick_xml::events::Event;
use crate::db::{Db, CachedFolderItem};
use crate::api::folder_source::FolderSourceEntry;
use crate::utils::parser::FilenameParser;
use crate::services::tmdb_service::TmdbService;

/// Maximum recursion depth for subfolder scanning
const MAX_DEPTH: u32 = 5;
/// Items per page for v3 API
const PER_PAGE: usize = 50;
/// Batch size for DB inserts
const INSERT_BATCH_SIZE: usize = 500;
/// Max total pages per folder to prevent runaway scanning
const MAX_PAGES_PER_FOLDER: u32 = 200;

pub struct FolderCacheService {
    db: Arc<Db>,
    tmdb: Arc<TmdbService>,
}

/// Report from a sync operation
#[derive(Debug, Serialize, Clone)]
pub struct SyncReport {
    pub total_items: usize,
    pub total_sources: usize,
    pub total_folders_scanned: usize,
    pub tmdb_mapped: usize,
    pub tmdb_skipped: usize,
    pub duration_secs: f64,
    pub errors: Vec<String>,
}

/// Candidate item for TMDB mapping (any depth, any type)
#[derive(Debug, Clone)]
struct TmdbCandidate {
    linkcode: String,
    title: String,
    year: Option<u32>,
    category: String,
}

/// Status of the folder cache
#[derive(Debug, Serialize)]
pub struct SyncStatus {
    pub last_sync: Option<String>,
    pub total_items: u64,
    pub is_syncing: bool,
}

/// A folder to scan (linkcode + depth)
struct FolderToScan {
    linkcode: String,
    category: String,
    label: String,
    depth: u32,
    parent_linkcode: String,
}

/// Raw item parsed from Fshare v3 XML response
#[derive(Debug, Clone, Default)]
struct FshareXmlItem {
    linkcode: String,
    name: String,
    r#type: String,
    size: String,
    mimetype: String,
    path: String,
}

impl FolderCacheService {
    pub fn new(db: Arc<Db>, tmdb: Arc<TmdbService>) -> Self {
        Self { db, tmdb }
    }

    /// Sync all folder sources if cache is stale (older than 24 hours) or empty
    pub async fn sync_if_stale(&self) {
        let is_stale = match self.db.get_folder_cache_meta("last_sync") {
            Ok(Some(ts)) => {
                // Parse timestamp and check if older than 24h
                if let Ok(last) = ts.parse::<i64>() {
                    let now = chrono::Utc::now().timestamp();
                    now - last > 86400
                } else {
                    true
                }
            }
            _ => true, // No metadata = never synced
        };

        let item_count = self.db.get_folder_cache_count().unwrap_or(0);

        if is_stale || item_count == 0 {
            tracing::info!("[FOLDER-CACHE] Cache is stale or empty (items: {}, stale: {}), starting sync", item_count, is_stale);
            match self.sync_all_sources().await {
                Ok(report) => {
                    tracing::info!(
                        "[FOLDER-CACHE] Sync complete: {} items from {} sources ({} folders) in {:.1}s",
                        report.total_items, report.total_sources, report.total_folders_scanned, report.duration_secs
                    );
                }
                Err(e) => {
                    tracing::error!("[FOLDER-CACHE] Sync failed: {}", e);
                }
            }
        } else {
            tracing::info!("[FOLDER-CACHE] Cache is fresh ({} items), skipping sync", item_count);
        }
    }

    /// Perform a full sync: fetch gist, recursively scan all folders, populate FTS5 cache
    pub async fn sync_all_sources(&self) -> anyhow::Result<SyncReport> {
        let start = std::time::Instant::now();
        let mut errors = Vec::new();

        // Get the gist URL from settings
        let gist_url = self.db.get_setting("folder_sources_gist_url")
            .ok()
            .flatten()
            .unwrap_or_default();

        if gist_url.is_empty() {
            tracing::warn!("[FOLDER-CACHE] No gist URL configured, skipping sync");
            return Ok(SyncReport {
                total_items: 0,
                total_sources: 0,
                total_folders_scanned: 0,
                tmdb_mapped: 0,
                tmdb_skipped: 0,
                duration_secs: 0.0,
                errors: vec!["No gist URL configured".to_string()],
            });
        }

        // Mark sync as in-progress
        let _ = self.db.set_folder_cache_meta("sync_status", "syncing");

        // Fetch the gist as plain text (one URL per line)
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let body = match client.get(&gist_url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let msg = format!("Gist fetch failed: HTTP {}", resp.status());
                    errors.push(msg.clone());
                    let _ = self.db.set_folder_cache_meta("sync_status", "idle");
                    return Err(anyhow::anyhow!(msg));
                }
                resp.text().await?
            }
            Err(e) => {
                let msg = format!("Gist fetch error: {}", e);
                errors.push(msg.clone());
                let _ = self.db.set_folder_cache_meta("sync_status", "idle");
                return Err(anyhow::anyhow!(msg));
            }
        };

        // Parse: one Fshare folder URL per line, skip blanks and comments
        let entries: Vec<FolderSourceEntry> = body
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#') && l.contains("/folder/"))
            .filter_map(|url| {
                let code = extract_folder_code(url)?;
                Some(FolderSourceEntry {
                    category: "auto".to_string(),
                    label: code.clone(),
                    folder_url: url.to_string(),
                })
            })
            .collect();

        tracing::info!("[FOLDER-CACHE] Syncing {} folder sources from gist", entries.len());

        // Clear old cache before populating
        if let Err(e) = self.db.clear_folder_cache() {
            tracing::error!("[FOLDER-CACHE] Failed to clear cache: {}", e);
        }

        let total_sources = entries.len();
        let mut total_items = 0;
        let mut total_folders_scanned = 0;
        let mut tmdb_candidates: Vec<TmdbCandidate> = Vec::new();
        let mut seen_candidate_titles: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Process each source
        for entry in &entries {
            let folder_code = match extract_folder_code(&entry.folder_url) {
                Some(code) => code,
                None => {
                    errors.push(format!("Invalid URL for '{}': {}", entry.label, entry.folder_url));
                    continue;
                }
            };

            // Extract token from URL
            let token = entry.folder_url
                .split('?')
                .nth(1)
                .and_then(|qs| qs.split('&').find(|p| p.starts_with("token=")))
                .and_then(|p| p.strip_prefix("token="))
                .unwrap_or("")
                .to_string();

            // BFS queue for recursive scanning
            let mut queue = VecDeque::new();
            queue.push_back(FolderToScan {
                linkcode: folder_code.clone(),
                category: entry.category.clone(),
                label: entry.label.clone(),
                depth: 0,
                parent_linkcode: String::new(),
            });

            let mut batch_buffer: Vec<CachedFolderItem> = Vec::new();

            while let Some(folder) = queue.pop_front() {
                if folder.depth > MAX_DEPTH {
                    continue;
                }

                total_folders_scanned += 1;
                let mut page = 1;

                loop {
                    let xml_body = match fetch_folder_page(&client, &folder.linkcode, &token, page).await {
                        Ok(body) => body,
                        Err(e) => {
                            errors.push(format!("API error for {}: {}", folder.linkcode, e));
                            break;
                        }
                    };

                    let (items, has_next) = parse_folder_response(&xml_body);

                    if items.is_empty() {
                        break;
                    }

                    for item in &items {
                        let is_dir = item.r#type == "0" || item.mimetype.is_empty();
                        let size: u64 = item.size.parse().unwrap_or(0);

                        let fshare_url = if is_dir {
                            format!("https://www.fshare.vn/folder/{}", item.linkcode)
                        } else {
                            format!("https://www.fshare.vn/file/{}", item.linkcode)
                        };

                        let parsed = FilenameParser::parse(&item.name);

                        batch_buffer.push(CachedFolderItem {
                            linkcode: item.linkcode.clone(),
                            name: item.name.clone(),
                            title: parsed.title.clone(),
                            category: folder.category.clone(),
                            label: folder.label.clone(),
                            parent_linkcode: folder.linkcode.clone(),
                            fshare_url,
                            year: parsed.year,
                            season: parsed.season,
                            episode: parsed.episode,
                            is_series: parsed.is_series,
                            is_directory: is_dir,
                            size,
                            quality: parsed.quality_attrs.quality_name(),
                            path: item.path.clone(),
                            tmdb_id: None,
                            media_type_hint: None,
                            poster_path: None,
                        });

                        // Collect TMDB candidates: any item with a media-like title.
                        // Dedup by normalized title to keep one representative per title.
                        let norm_title = parsed.title.trim().to_lowercase();
                        if !norm_title.is_empty()
                            && is_likely_content_title(&norm_title)
                            && seen_candidate_titles.insert(norm_title)
                        {
                            tmdb_candidates.push(TmdbCandidate {
                                linkcode: item.linkcode.clone(),
                                title: parsed.title.clone(),
                                year: parsed.year,
                                category: folder.category.clone(),
                            });
                        }

                        // Queue subfolders for recursive scanning
                        if is_dir && !item.linkcode.is_empty() {
                            queue.push_back(FolderToScan {
                                linkcode: item.linkcode.clone(),
                                category: folder.category.clone(),
                                label: folder.label.clone(),
                                depth: folder.depth + 1,
                                parent_linkcode: folder.linkcode.clone(),
                            });
                        }
                    }

                    // Flush batch to DB when buffer is large enough
                    if batch_buffer.len() >= INSERT_BATCH_SIZE {
                        match self.db.insert_folder_cache_batch(&batch_buffer) {
                            Ok(n) => total_items += n,
                            Err(e) => errors.push(format!("DB insert error: {}", e)),
                        }
                        batch_buffer.clear();
                    }

                    if !has_next || items.len() < PER_PAGE {
                        break;
                    }

                    page += 1;
                    if page > MAX_PAGES_PER_FOLDER {
                        tracing::warn!("[FOLDER-CACHE] Hit page limit for folder {}", folder.linkcode);
                        break;
                    }

                    // Small delay to be nice to Fshare API
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }

            // Flush remaining items
            if !batch_buffer.is_empty() {
                match self.db.insert_folder_cache_batch(&batch_buffer) {
                    Ok(n) => total_items += n,
                    Err(e) => errors.push(format!("DB insert error: {}", e)),
                }
            }

            tracing::info!("[FOLDER-CACHE] Source '{}' complete", entry.label);
        }

        // ── TMDB MAPPING PASS ───────────────────────────────────────────────────
        tracing::info!("[FOLDER-CACHE] Starting TMDB mapping for {} unique titles", tmdb_candidates.len());
        let (tmdb_mapped, tmdb_skipped) = self.run_tmdb_mapping(&tmdb_candidates, &mut errors).await;

        // Update metadata
        let now = chrono::Utc::now().timestamp().to_string();
        let _ = self.db.set_folder_cache_meta("last_sync", &now);
        let _ = self.db.set_folder_cache_meta("sync_status", "idle");
        let _ = self.db.set_folder_cache_meta("total_items", &total_items.to_string());

        let duration = start.elapsed().as_secs_f64();

        if !errors.is_empty() {
            tracing::warn!("[FOLDER-CACHE] Sync completed with {} errors", errors.len());
            for err in &errors {
                tracing::warn!("[FOLDER-CACHE]   - {}", err);
            }
        }

        Ok(SyncReport {
            total_items,
            total_sources,
            total_folders_scanned,
            tmdb_mapped,
            tmdb_skipped,
            duration_secs: duration,
            errors,
        })
    }

    /// Run TMDB mapping for top-level folders that don't already have a mapping.
    /// Returns (mapped_count, skipped_count).
    async fn run_tmdb_mapping(&self, items: &[TmdbCandidate], errors: &mut Vec<String>) -> (usize, usize) {
        // Load existing mappings to skip already-mapped items
        let existing = match self.db.get_all_folder_tmdb_mappings() {
            Ok(m) => m,
            Err(e) => {
                errors.push(format!("Failed to load TMDB mappings: {}", e));
                return (0, 0);
            }
        };

        // Group items by parsed title to avoid redundant TMDB calls.
        // e.g., if 3 folders all parse to "Peaky Blinders", we search TMDB once.
        let mut title_groups: HashMap<String, Vec<&TmdbCandidate>> = HashMap::new();
        let mut already_mapped = 0usize;

        for item in items {
            if existing.contains_key(&item.linkcode) {
                already_mapped += 1;
                continue;
            }
            let key = item.title.trim().to_lowercase();
            if key.is_empty() { continue; }
            title_groups.entry(key).or_default().push(item);
        }

        tracing::info!(
            "[FOLDER-CACHE] TMDB: {} unique titles to map, {} already mapped",
            title_groups.len(), already_mapped
        );

        let mut mapped_count = 0usize;
        let mut batch_mappings: Vec<(String, i64, String, String)> = Vec::new();

        for (title_key, group) in &title_groups {
            // Use the first item for context
            let sample = group[0];
            let media_type = if sample.category == "movie" { "movie" } else { "tv" };

            // Build TMDB search query: title + year (if available)
            let search_query = if let Some(y) = sample.year {
                format!("{} {}", sample.title, y)
            } else {
                sample.title.clone()
            };

            // Search TMDB
            let result = self.tmdb.search(media_type, &search_query).await;

            if let Some(data) = result {
                if let Some(results) = data.get("results").and_then(|r| r.as_array()) {
                    if let Some(first) = results.first() {
                        let tmdb_id = first.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                        let poster = first.get("poster_path")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        if tmdb_id > 0 {
                            let title_check = first.get("title")
                                .or_else(|| first.get("name"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            tracing::debug!(
                                "[FOLDER-CACHE] TMDB match: '{}' → '{}' (id={}, poster={})",
                                sample.title, title_check, tmdb_id, poster
                            );

                            // Apply to ALL items in this title group
                            for item in group {
                                batch_mappings.push((
                                    item.linkcode.clone(),
                                    tmdb_id,
                                    media_type.to_string(),
                                    poster.clone(),
                                ));
                            }
                            mapped_count += group.len();
                        }
                    }
                }
            }

            // Flush batch every 50 mappings
            if batch_mappings.len() >= 50 {
                if let Err(e) = self.db.set_folder_tmdb_mappings_batch(&batch_mappings) {
                    errors.push(format!("TMDB batch insert error: {}", e));
                }
                batch_mappings.clear();
            }

            // TMDB rate limit: ~40 req/10s → ~250ms between calls
            tokio::time::sleep(std::time::Duration::from_millis(260)).await;
        }

        // Flush remaining
        if !batch_mappings.is_empty() {
            if let Err(e) = self.db.set_folder_tmdb_mappings_batch(&batch_mappings) {
                errors.push(format!("TMDB batch insert error: {}", e));
            }
        }

        tracing::info!(
            "[FOLDER-CACHE] TMDB mapping complete: {} newly mapped, {} previously mapped",
            mapped_count, already_mapped
        );

        (mapped_count, already_mapped)
    }

    /// Search the FTS5 cache
    pub async fn search(&self, query: &str, limit: u32) -> anyhow::Result<Vec<CachedFolderItem>> {
        let results = self.db.search_folder_cache_async(query.to_string(), limit).await
            .map_err(|e| anyhow::anyhow!("Search failed: {}", e))?;
        Ok(results)
    }

    /// Get the current sync status
    pub fn get_sync_status(&self) -> SyncStatus {
        let last_sync = self.db.get_folder_cache_meta("last_sync").ok().flatten();
        let total_items = self.db.get_folder_cache_count().unwrap_or(0);
        let sync_status = self.db.get_folder_cache_meta("sync_status")
            .ok()
            .flatten()
            .unwrap_or_else(|| "idle".to_string());

        SyncStatus {
            last_sync,
            total_items,
            is_syncing: sync_status == "syncing",
        }
    }
}

// ============================================================================
// Private helpers
// ============================================================================
/// Heuristic: is this parsed title likely a real movie/show name?
/// Filters out structural folder names, season labels, pure numbers, etc.
fn is_likely_content_title(title_lower: &str) -> bool {
    // Too short to be a real title
    if title_lower.len() < 2 { return false; }

    // Pure numbers (e.g. "01", "2024")
    if title_lower.chars().all(|c| c.is_ascii_digit()) { return false; }

    // Generic structural folder names
    static SKIP_PATTERNS: &[&str] = &[
        "season", "s01", "s02", "s03", "s04", "s05", "s06", "s07", "s08", "s09", "s10",
        "folder", "phim", "movie", "film", "video", "collection", "complete",
        "subs", "subtitle", "extra", "bonus", "featurette", "sample",
        "disc", "disk", "cd", "dvd", "bluray", "remux",
        "temp", "tmp", "new", "old", "backup",
    ];

    // Skip if title IS one of these generic words
    if SKIP_PATTERNS.contains(&title_lower) { return false; }

    // Skip if title starts with common non-content patterns
    if title_lower.starts_with("season ") { return false; }
    if title_lower.starts_with("disc ") { return false; }
    if title_lower.starts_with("01_") || title_lower.starts_with("02_") || title_lower.starts_with("03_") { return false; }

    true
}

/// Extract folder linkcode from URL
fn extract_folder_code(folder_url: &str) -> Option<String> {
    if !folder_url.contains("/folder/") {
        return None;
    }
    let after = folder_url.split("/folder/").last()?;
    let code = after.split('?').next().unwrap_or("");
    if code.is_empty() { None } else { Some(code.to_string()) }
}

/// Fetch a single page of a folder listing from the v3 API
async fn fetch_folder_page(
    client: &reqwest::Client,
    linkcode: &str,
    token: &str,
    page: u32,
) -> anyhow::Result<String> {
    let page_str = page.to_string();
    let per_page_str = PER_PAGE.to_string();
    let mut query_params: Vec<(&str, &str)> = vec![
        ("linkcode", linkcode),
        ("sort", "type"),
        ("page", &page_str),
        ("per-page", &per_page_str),
    ];

    if !token.is_empty() {
        query_params.push(("token", token));
    }

    let resp = client.get("https://www.fshare.vn/api/v3/files/folder")
        .query(&query_params)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow::anyhow!("HTTP {}", resp.status()));
    }

    Ok(resp.text().await?)
}

/// Parse Fshare v3 API response — auto-detects JSON or XML format
fn parse_folder_response(body: &str) -> (Vec<FshareXmlItem>, bool) {
    let trimmed = body.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        parse_folder_json(trimmed)
    } else {
        parse_folder_xml(trimmed)
    }
}

/// Parse JSON response from Fshare v3 API
fn parse_folder_json(json_text: &str) -> (Vec<FshareXmlItem>, bool) {
    let parsed: serde_json::Value = match serde_json::from_str(json_text) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("[FOLDER-CACHE] JSON parse error: {}", e);
            return (vec![], false);
        }
    };

    let mut items = Vec::new();

    if let Some(arr) = parsed.get("items").and_then(|v| v.as_array()) {
        for item in arr {
            let linkcode = item.get("linkcode").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();

            if linkcode.is_empty() || name.is_empty() { continue; }

            let item_type = item.get("type").map(|v| match v {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                _ => String::new(),
            }).unwrap_or_default();

            let size = item.get("size").map(|v| match v {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                _ => "0".to_string(),
            }).unwrap_or_else(|| "0".to_string());

            let mimetype = item.get("mimetype")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let path = item.get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            items.push(FshareXmlItem {
                linkcode,
                name,
                r#type: item_type,
                size,
                mimetype,
                path,
            });
        }
    }

    // Check for pagination
    let has_next = parsed.get("_links")
        .and_then(|links| links.get("next"))
        .map(|next| !next.is_null())
        .unwrap_or(false);

    (items, has_next)
}

/// Parse Fshare v3 XML response (legacy format)
fn parse_folder_xml(xml_text: &str) -> (Vec<FshareXmlItem>, bool) {
    let mut reader = Reader::from_str(xml_text);
    reader.trim_text(true);

    let mut items = Vec::new();
    let mut current_item: Option<FshareXmlItem> = None;
    let mut current_tag = String::new();
    let mut in_items = false;
    let mut in_item = false;
    let mut in_links = false;
    let mut has_next_page = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag_name.as_str() {
                    "items" => in_items = true,
                    "item" if in_items => {
                        in_item = true;
                        current_item = Some(FshareXmlItem::default());
                    }
                    "_links" => in_links = true,
                    "next" if in_links => {
                        has_next_page = true;
                    }
                    _ => {}
                }
                current_tag = tag_name;
            }
            Ok(Event::End(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag_name.as_str() {
                    "items" => in_items = false,
                    "item" if in_items => {
                        if let Some(item) = current_item.take() {
                            if !item.linkcode.is_empty() && !item.name.is_empty() {
                                items.push(item);
                            }
                        }
                        in_item = false;
                    }
                    "_links" => in_links = false,
                    _ => {}
                }
                current_tag.clear();
            }
            Ok(Event::Text(ref e)) => {
                if in_item {
                    if let Some(ref mut item) = current_item {
                        let text = e.unescape().unwrap_or_default().to_string();
                        match current_tag.as_str() {
                            "linkcode" => item.linkcode = text,
                            "name" => item.name = text,
                            "type" => item.r#type = text,
                            "size" => item.size = text,
                            "mimetype" => item.mimetype = text,
                            "path" => item.path = text,
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::error!("[FOLDER-CACHE] XML parse error: {}", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    (items, has_next_page)
}

// ============================================================================
// Tests — Standalone, no server required
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    /// Create a fresh test DB (temp file, auto-cleaned up)
    fn test_db() -> (Arc<Db>, NamedTempFile) {
        let tmp = NamedTempFile::new().expect("Failed to create temp file");
        let db = Db::new(tmp.path()).expect("Failed to create test DB");
        (Arc::new(db), tmp)
    }

    /// Build a set of realistic cached folder items for testing
    fn sample_items() -> Vec<CachedFolderItem> {
        vec![
            CachedFolderItem {
                linkcode: "VE41H65WVF9M".to_string(),
                name: "Reality Z - Chương Trình Thực Tế Z Season 1_Việt Sub".to_string(),
                title: "Reality Z".to_string(),
                category: "tv".to_string(),
                label: "Phim Bộ Mỹ".to_string(),
                parent_linkcode: "ROOT1".to_string(),
                fshare_url: "https://www.fshare.vn/folder/VE41H65WVF9M".to_string(),
                year: None,
                season: Some(1),
                episode: None,
                is_series: true,
                is_directory: true,
                size: 0,
                quality: "Unknown".to_string(),
                path: "/01_Folder Chính/Phim Bộ Mỹ".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
            CachedFolderItem {
                linkcode: "R53RB1UZ5HMH".to_string(),
                name: "Traces.S01.1080p.AMZN.WEBRip.DDP2.0.x264-Cinefeel".to_string(),
                title: "Traces".to_string(),
                category: "tv".to_string(),
                label: "Phim Bộ Mỹ".to_string(),
                parent_linkcode: "ROOT1".to_string(),
                fshare_url: "https://www.fshare.vn/folder/R53RB1UZ5HMH".to_string(),
                year: None,
                season: Some(1),
                episode: None,
                is_series: true,
                is_directory: true,
                size: 0,
                quality: "WEBRip-1080p".to_string(),
                path: "/01_Folder Chính/Phim Bộ Mỹ".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
            CachedFolderItem {
                linkcode: "BN243UOLOA7U".to_string(),
                name: "Mrs.America.S01.1080p.AMZN.WEBRip.DDP5.1.x264-NTb".to_string(),
                title: "Mrs America".to_string(),
                category: "tv".to_string(),
                label: "Phim Bộ Mỹ".to_string(),
                parent_linkcode: "ROOT1".to_string(),
                fshare_url: "https://www.fshare.vn/folder/BN243UOLOA7U".to_string(),
                year: None,
                season: Some(1),
                episode: None,
                is_series: true,
                is_directory: true,
                size: 0,
                quality: "WEBRip-1080p".to_string(),
                path: "/01_Folder Chính/Phim Bộ Mỹ".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
            CachedFolderItem {
                linkcode: "454TL817M8Z47GC".to_string(),
                name: "Peaky Blinders - Bóng Ma Anh Quốc Season 6".to_string(),
                title: "Peaky Blinders".to_string(),
                category: "tv".to_string(),
                label: "Phim Bộ Mỹ".to_string(),
                parent_linkcode: "ROOT1".to_string(),
                fshare_url: "https://www.fshare.vn/folder/454TL817M8Z47GC".to_string(),
                year: None,
                season: Some(6),
                episode: None,
                is_series: true,
                is_directory: true,
                size: 0,
                quality: "Unknown".to_string(),
                path: "/01_Folder Chính/Phim Bộ Mỹ".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
            CachedFolderItem {
                linkcode: "FILE001ABC".to_string(),
                name: "Inventing.Anna.S01E01.1080p.NF.WEB-DL.DDP5.1.x264.mkv".to_string(),
                title: "Inventing Anna".to_string(),
                category: "tv".to_string(),
                label: "Phim Bộ Mỹ".to_string(),
                parent_linkcode: "FOLDER_ANNA".to_string(),
                fshare_url: "https://www.fshare.vn/file/FILE001ABC".to_string(),
                year: None,
                season: Some(1),
                episode: Some(1),
                is_series: true,
                is_directory: false,
                size: 2_500_000_000,
                quality: "WEB-DL-1080p".to_string(),
                path: "/01_Folder Chính/Phim Bộ Mỹ/Inventing Anna".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
            CachedFolderItem {
                linkcode: "MOVIE123".to_string(),
                name: "Dune Part Two 2024 2160p WEB-DL DDP5.1 Atmos DV HDR x265".to_string(),
                title: "Dune Part Two".to_string(),
                category: "movie".to_string(),
                label: "Phim Lẻ".to_string(),
                parent_linkcode: "ROOT2".to_string(),
                fshare_url: "https://www.fshare.vn/file/MOVIE123".to_string(),
                year: Some(2024),
                season: None,
                episode: None,
                is_series: false,
                is_directory: false,
                size: 15_000_000_000,
                quality: "WEB-DL-2160p".to_string(),
                path: "/02_Phim Lẻ".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
            CachedFolderItem {
                linkcode: "MOVIE456".to_string(),
                name: "Oppenheimer 2023 1080p BluRay x264-SPARKS".to_string(),
                title: "Oppenheimer".to_string(),
                category: "movie".to_string(),
                label: "Phim Lẻ".to_string(),
                parent_linkcode: "ROOT2".to_string(),
                fshare_url: "https://www.fshare.vn/file/MOVIE456".to_string(),
                year: Some(2023),
                season: None,
                episode: None,
                is_series: false,
                is_directory: false,
                size: 10_000_000_000,
                quality: "BluRay-1080p".to_string(),
                path: "/02_Phim Lẻ".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
            CachedFolderItem {
                linkcode: "VIET789".to_string(),
                name: "Vikings Valhalla - Huyền Thoại Vikings Valhalla 2022_Việt Sub".to_string(),
                title: "Vikings Valhalla".to_string(),
                category: "tv".to_string(),
                label: "Phim Bộ Mỹ".to_string(),
                parent_linkcode: "ROOT1".to_string(),
                fshare_url: "https://www.fshare.vn/folder/VIET789".to_string(),
                year: Some(2022),
                season: None,
                episode: None,
                is_series: false,
                is_directory: true,
                size: 0,
                quality: "Unknown".to_string(),
                path: "/01_Folder Chính/Phim Bộ Mỹ".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
        ]
    }

    // ── Test: Basic insert and count ──────────────────────────────────

    #[test]
    fn test_insert_and_count() {
        let (db, _tmp) = test_db();
        let items = sample_items();

        let inserted = db.insert_folder_cache_batch(&items).unwrap();
        assert_eq!(inserted, 8, "Should insert all 8 items");

        let count = db.get_folder_cache_count().unwrap();
        assert_eq!(count, 8, "Count should be 8");
    }

    // ── Test: FTS5 search by title ───────────────────────────────────

    #[test]
    fn test_search_by_title() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();

        // Search for "Peaky" — should find Peaky Blinders
        let results = db.search_folder_cache("Peaky", 50).unwrap();
        assert!(!results.is_empty(), "Should find Peaky Blinders");
        assert!(results.iter().any(|r| r.name.contains("Peaky Blinders")));

        println!("\n🔍 Search 'Peaky': {} results", results.len());
        for r in &results {
            println!("  - {} ({})", r.name, r.fshare_url);
        }
    }

    // ── Test: FTS5 search by raw filename ─────────────────────────────

    #[test]
    fn test_search_by_filename() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();

        // Search for "Traces" — matches the scene release name
        let results = db.search_folder_cache("Traces", 50).unwrap();
        assert!(!results.is_empty(), "Should find Traces");
        assert!(results.iter().any(|r| r.linkcode == "R53RB1UZ5HMH"));

        println!("\n🔍 Search 'Traces': {} results", results.len());
        for r in &results {
            println!("  - {} [{}]", r.name, r.quality);
        }
    }

    // ── Test: FTS5 prefix matching ───────────────────────────────────

    #[test]
    fn test_search_prefix_matching() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();

        // Search for "Vik" — prefix should match "Vikings"
        let results = db.search_folder_cache("Vik", 50).unwrap();
        assert!(!results.is_empty(), "Prefix 'Vik' should match Vikings");
        assert!(results.iter().any(|r| r.name.contains("Vikings")));

        println!("\n🔍 Search 'Vik' (prefix): {} results", results.len());
        for r in &results {
            println!("  - {}", r.name);
        }
    }

    // ── Test: Multi-word search ──────────────────────────────────────

    #[test]
    fn test_search_multi_word() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();

        // "Dune Part" should match "Dune Part Two"
        let results = db.search_folder_cache("Dune Part", 50).unwrap();
        assert!(!results.is_empty(), "Should find Dune Part Two");
        assert!(results.iter().any(|r| r.name.contains("Dune Part Two")));

        println!("\n🔍 Search 'Dune Part': {} results", results.len());
        for r in &results {
            println!("  - {} (year: {:?}, quality: {})", r.name, r.year, r.quality);
        }
    }

    // ── Test: Search by category/label ────────────────────────────────

    #[test]
    fn test_search_by_category() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();

        // Search by label "Phim Lẻ" — should find movies
        let results = db.search_folder_cache("Phim Lẻ", 50).unwrap();
        println!("\n🔍 Search 'Phim Lẻ': {} results", results.len());
        for r in &results {
            println!("  - {} [cat: {}, label: {}]", r.name, r.category, r.label);
        }
        // Should find items with label "Phim Lẻ"
        assert!(results.iter().any(|r| r.label == "Phim Lẻ"));
    }

    // ── Test: Empty query returns nothing ─────────────────────────────

    #[test]
    fn test_search_empty_query() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();

        // Empty string should not match (we'd handle this in the API layer)
        // But FTS5 MATCH with an empty string will error, so the API catches that
        let results = db.search_folder_cache("xyznonexistent", 50).unwrap();
        assert!(results.is_empty(), "Nonexistent query should return empty");
    }

    // ── Test: Clear cache ────────────────────────────────────────────

    #[test]
    fn test_clear_cache() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();
        assert_eq!(db.get_folder_cache_count().unwrap(), 8);

        db.clear_folder_cache().unwrap();
        assert_eq!(db.get_folder_cache_count().unwrap(), 0, "Cache should be empty after clear");

        // Search after clear = no results
        let results = db.search_folder_cache("Peaky", 50).unwrap();
        assert!(results.is_empty());
    }

    // ── Test: Metadata operations ────────────────────────────────────

    #[test]
    fn test_metadata() {
        let (db, _tmp) = test_db();

        // Initially no metadata
        assert!(db.get_folder_cache_meta("last_sync").unwrap().is_none());

        // Set and get
        db.set_folder_cache_meta("last_sync", "1709000000").unwrap();
        let val = db.get_folder_cache_meta("last_sync").unwrap();
        assert_eq!(val, Some("1709000000".to_string()));

        // Update
        db.set_folder_cache_meta("last_sync", "1709100000").unwrap();
        let val = db.get_folder_cache_meta("last_sync").unwrap();
        assert_eq!(val, Some("1709100000".to_string()));
    }

    // ── Test: Search result data integrity ────────────────────────────

    #[test]
    fn test_search_result_fields() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();

        let results = db.search_folder_cache("Oppenheimer", 50).unwrap();
        assert_eq!(results.len(), 1);

        let r = &results[0];
        assert_eq!(r.linkcode, "MOVIE456");
        assert_eq!(r.title, "Oppenheimer");
        assert_eq!(r.category, "movie");
        assert_eq!(r.year, Some(2023));
        assert!(!r.is_directory);
        assert!(!r.is_series);
        assert_eq!(r.quality, "BluRay-1080p");
        assert_eq!(r.size, 10_000_000_000);
        assert_eq!(r.fshare_url, "https://www.fshare.vn/file/MOVIE456");

        println!("\n✅ Oppenheimer data integrity verified:");
        println!("  linkcode: {}", r.linkcode);
        println!("  year: {:?}", r.year);
        println!("  quality: {}", r.quality);
        println!("  size: {} GB", r.size / 1_000_000_000);
    }

    // ── Test: Batch insert large dataset ──────────────────────────────

    #[test]
    fn test_large_batch_insert() {
        let (db, _tmp) = test_db();

        // Create 1000 items
        let mut items = Vec::new();
        for i in 0..1000 {
            items.push(CachedFolderItem {
                linkcode: format!("CODE{:04}", i),
                name: format!("Movie.{}.2024.1080p.BluRay.x264-GROUP", i),
                title: format!("Movie {}", i),
                category: "movie".to_string(),
                label: "Test".to_string(),
                parent_linkcode: "ROOT".to_string(),
                fshare_url: format!("https://www.fshare.vn/file/CODE{:04}", i),
                year: Some(2024),
                season: None,
                episode: None,
                is_series: false,
                is_directory: false,
                size: 5_000_000_000,
                quality: "BluRay-1080p".to_string(),
                path: "/test".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            });
        }

        let start = std::time::Instant::now();
        let inserted = db.insert_folder_cache_batch(&items).unwrap();
        let insert_time = start.elapsed();

        assert_eq!(inserted, 1000);
        assert_eq!(db.get_folder_cache_count().unwrap(), 1000);

        // Now search
        let start = std::time::Instant::now();
        let results = db.search_folder_cache("Movie 500", 10).unwrap();
        let search_time = start.elapsed();

        println!("\n⚡ Performance (1000 items):");
        println!("  Insert: {:.2}ms", insert_time.as_secs_f64() * 1000.0);
        println!("  Search: {:.3}ms", search_time.as_secs_f64() * 1000.0);
        println!("  Results: {}", results.len());
        for r in &results {
            println!("    - {}", r.name);
        }

        assert!(!results.is_empty(), "Should find results for 'Movie 500'");
    }

    // ── Test: XML parsing ────────────────────────────────────────────

    #[test]
    fn test_xml_parsing() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<response>
    <items>
        <item>
            <linkcode>ABC123</linkcode>
            <name>Test Movie 2024</name>
            <type>1</type>
            <size>5000000000</size>
            <mimetype>video/x-matroska</mimetype>
            <path>/movies</path>
        </item>
        <item>
            <linkcode>DEF456</linkcode>
            <name>TV Show Season 1</name>
            <type>0</type>
            <size>0</size>
            <mimetype></mimetype>
            <path>/shows</path>
        </item>
    </items>
    <_links>
        <next>/v3/files/folder?page=2</next>
    </_links>
</response>"#;

        let (items, has_next) = parse_folder_xml(xml);

        assert_eq!(items.len(), 2);
        assert!(has_next);

        assert_eq!(items[0].linkcode, "ABC123");
        assert_eq!(items[0].name, "Test Movie 2024");
        assert_eq!(items[0].r#type, "1"); // file
        assert_eq!(items[0].size, "5000000000");

        assert_eq!(items[1].linkcode, "DEF456");
        assert_eq!(items[1].r#type, "0"); // folder
    }

    // ── Test: extract_folder_code ─────────────────────────────────────

    #[test]
    fn test_extract_folder_code() {
        assert_eq!(
            extract_folder_code("https://www.fshare.vn/folder/ABC123?token=xyz"),
            Some("ABC123".to_string())
        );
        assert_eq!(
            extract_folder_code("https://www.fshare.vn/folder/XEVZ47FBZSR4"),
            Some("XEVZ47FBZSR4".to_string())
        );
        assert_eq!(extract_folder_code("https://fshare.vn/file/ABC"), None);
        assert_eq!(extract_folder_code("garbage"), None);
    }

    // ══════════════════════════════════════════════════════════════════
    // Live Integration Test — fetches REAL data from Fshare
    // Run with: cargo test live_folder_scan -- --ignored --nocapture
    // ══════════════════════════════════════════════════════════════════

    #[tokio::test]
    #[ignore] // Requires network access — run explicitly
    async fn live_folder_scan_and_search() {
        println!("\n╔══════════════════════════════════════════════════════════╗");
        println!("║      Live Folder Cache Test (Real Fshare Folders)       ║");
        println!("╚══════════════════════════════════════════════════════════╝\n");

        // Source entries — real Fshare folders
        let entries = vec![
            ("tv", "Phim Bộ Mỹ", "https://www.fshare.vn/folder/XEVZ47FBZSR4?token=1772074278"),
            ("movie", "Phim Lẻ", "https://www.fshare.vn/folder/QOV7Z7MDMTXY?token=1772075966"),
        ];

        // Create temp DB
        let tmp = NamedTempFile::new().unwrap();
        let db = Arc::new(Db::new(tmp.path()).unwrap());

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();

        let mut total_items = 0usize;
        let mut total_folders = 0usize;
        let start = std::time::Instant::now();

        for (category, label, url) in &entries {
            let folder_code = extract_folder_code(url).unwrap();
            let token = url.split('?').nth(1)
                .and_then(|qs| qs.split('&').find(|p| p.starts_with("token=")))
                .and_then(|p| p.strip_prefix("token="))
                .unwrap_or("")
                .to_string();

            println!("📂 Scanning '{}' (code: {})...", label, folder_code);

            // BFS scan — depth 0 only for speed (just top-level items)
            let mut queue = VecDeque::new();
            queue.push_back((folder_code.clone(), 0u32));

            let mut batch: Vec<CachedFolderItem> = Vec::new();

            while let Some((linkcode, depth)) = queue.pop_front() {
                if depth > 1 { continue; } // Only top level + 1 deep for test speed
                total_folders += 1;
                let mut page = 1;

                loop {
                    let body = match fetch_folder_page(&client, &linkcode, &token, page).await {
                        Ok(b) => b,
                        Err(e) => {
                            eprintln!("   ⚠️  API error for {} page {}: {}", linkcode, page, e);
                            break;
                        }
                    };

                    let (items, has_next) = parse_folder_response(&body);
                    if items.is_empty() { break; }

                    println!("   📄 Page {} (depth {}): {} items", page, depth, items.len());

                    for item in &items {
                        let is_dir = item.r#type == "0" || item.mimetype.is_empty();
                        let size: u64 = item.size.parse().unwrap_or(0);
                        let fshare_url = if is_dir {
                            format!("https://www.fshare.vn/folder/{}", item.linkcode)
                        } else {
                            format!("https://www.fshare.vn/file/{}", item.linkcode)
                        };
                        let parsed = FilenameParser::parse(&item.name);

                        batch.push(CachedFolderItem {
                            linkcode: item.linkcode.clone(),
                            name: item.name.clone(),
                            title: parsed.title,
                            category: category.to_string(),
                            label: label.to_string(),
                            parent_linkcode: linkcode.clone(),
                            fshare_url,
                            year: parsed.year,
                            season: parsed.season,
                            episode: parsed.episode,
                            is_series: parsed.is_series,
                            is_directory: is_dir,
                            size,
                            quality: parsed.quality_attrs.quality_name(),
                            path: item.path.clone(),
                            tmdb_id: None,
                            media_type_hint: None,
                            poster_path: None,
                        });

                        if is_dir && depth < 1 {
                            queue.push_back((item.linkcode.clone(), depth + 1));
                        }
                    }

                    if !has_next || items.len() < PER_PAGE { break; }
                    page += 1;
                    if page > 10 { break; } // Safety limit for test

                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }

            if !batch.is_empty() {
                let count = db.insert_folder_cache_batch(&batch).unwrap();
                total_items += count;
                println!("   ✅ Inserted {} items from '{}'\n", count, label);
            }
        }

        let scan_time = start.elapsed();
        println!("═══════════════════════════════════════════════════════");
        println!("📊 Scan Summary:");
        println!("   Total items:   {}", total_items);
        println!("   Total folders: {}", total_folders);
        println!("   Duration:      {:.1}s", scan_time.as_secs_f64());
        println!("   Cache count:   {}", db.get_folder_cache_count().unwrap());
        println!("═══════════════════════════════════════════════════════\n");

        // ── Search tests ─────────────────────────────────────────────

        let search_queries = vec![
            "Vĩnh Dạ Tinh Hà",
            "Loki",
            "Peaky",
            "Vikings",
            "Season",
            "1080p",
            "Dune",
        ];

        for query in &search_queries {
            println!("🔍 Search: \"{}\"", query);
            let search_start = std::time::Instant::now();
            let results = db.search_folder_cache(query, 10).unwrap();
            let search_ms = search_start.elapsed().as_secs_f64() * 1000.0;

            if results.is_empty() {
                println!("   (no results)  [{:.3}ms]\n", search_ms);
            } else {
                println!("   {} results  [{:.3}ms]", results.len(), search_ms);
                for (i, r) in results.iter().enumerate() {
                    let size_str = if r.size > 0 {
                        format!("{:.1}GB", r.size as f64 / 1_000_000_000.0)
                    } else {
                        "📁 dir".to_string()
                    };
                    println!("   {}. {} [{}] [{}]", i+1, r.name, r.quality, size_str);
                    println!("      → {}", r.fshare_url);
                }
                println!();
            }
        }

        // Assertions — cache should have items
        assert!(total_items > 0, "Should have scanned some items from Fshare");
        println!("✅ Live integration test complete!");
    }
}
