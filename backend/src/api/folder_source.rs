//! Folder Source API Routes
//!
//! Endpoints for managing Fshare folder sources used for searching.
//! Users configure a Gist URL pointing to a JSON file of curated folder links.
//! The scan endpoint fetches folders via the Fshare v3 API (returns XML) and
//! parses subfolder names to detect movies or TV series.

use axum::{
    routing::{get, put, post},
    Router,
    Json,
    extract::{State, Query},
};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use quick_xml::Reader;
use quick_xml::events::Event;
use crate::AppState;
use crate::utils::parser::FilenameParser;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/config", get(get_folder_source_config))
        .route("/config", put(update_folder_source_config))
        .route("/scan", post(scan_folder_sources))
        .route("/search", get(search_folder_cache))
        .route("/sync", post(trigger_sync))
        .route("/status", get(get_sync_status))
}

// ============================================================================
// Types
// ============================================================================

/// A single folder source entry from the user's JSON file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderSourceEntry {
    pub category: String,
    pub label: String,
    pub folder_url: String,
}

/// Config response/request for the Gist URL
#[derive(Debug, Serialize, Deserialize)]
struct FolderSourceConfig {
    pub gist_url: String,
}

/// A raw item parsed from the Fshare v3 XML response
#[derive(Debug, Clone, Default)]
struct FshareXmlItem {
    id: String,
    linkcode: String,
    name: String,
    r#type: String,    // "0" = folder, "1" = file
    size: String,
    mimetype: String,
    path: String,
}

/// A parsed item from a folder listing
#[derive(Debug, Serialize)]
struct ParsedFolderItem {
    /// Raw name from Fshare
    name: String,
    /// Linkcode for the item
    linkcode: String,
    /// Full Fshare URL to the file/subfolder
    fshare_url: String,
    /// Whether this is a directory (subfolder)
    is_directory: bool,
    /// Size in bytes (0 for directories)
    size: u64,
    /// Parsed title (English title extracted)
    title: String,
    /// Detected year
    year: Option<u32>,
    /// Whether it's a series
    is_series: bool,
    /// Season number
    season: Option<u32>,
    /// Episode number
    episode: Option<u32>,
    /// Quality description
    quality: String,
}

/// Result for a single folder source
#[derive(Debug, Serialize)]
struct FolderSourceResult {
    category: String,
    label: String,
    folder_url: String,
    items: Vec<ParsedFolderItem>,
    total_items: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Response from the scan endpoint
#[derive(Debug, Serialize)]
struct ScanResponse {
    sources: Vec<FolderSourceResult>,
    total_sources: usize,
}

/// Generic action response
#[derive(Debug, Serialize)]
struct ActionResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

/// Query parameters for search
#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_search_limit")]
    limit: u32,
}

fn default_search_limit() -> u32 {
    50
}

/// Search response
#[derive(Debug, Serialize)]
struct SearchResponse {
    results: Vec<crate::db::CachedFolderItem>,
    total: usize,
    query: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/folder-source/config — Get the configured Gist URL
async fn get_folder_source_config(
    State(state): State<Arc<AppState>>,
) -> Json<FolderSourceConfig> {
    let gist_url = state.db.get_setting("folder_sources_gist_url")
        .ok()
        .flatten()
        .unwrap_or_default();

    Json(FolderSourceConfig { gist_url })
}

/// PUT /api/folder-source/config — Save the Gist URL
async fn update_folder_source_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<FolderSourceConfig>,
) -> Json<ActionResponse> {
    match state.db.save_setting("folder_sources_gist_url", &payload.gist_url) {
        Ok(_) => {
            tracing::info!("[FOLDER-SOURCE] Saved gist URL: {}", payload.gist_url);
            Json(ActionResponse {
                success: true,
                message: Some("Folder source URL saved successfully".to_string()),
            })
        }
        Err(e) => {
            tracing::error!("[FOLDER-SOURCE] Failed to save gist URL: {}", e);
            Json(ActionResponse {
                success: false,
                message: Some(format!("Failed to save: {}", e)),
            })
        }
    }
}

/// POST /api/folder-source/scan — Fetch and parse all folder sources
async fn scan_folder_sources(
    State(state): State<Arc<AppState>>,
) -> Json<ScanResponse> {
    let gist_url = state.db.get_setting("folder_sources_gist_url")
        .ok()
        .flatten()
        .unwrap_or_default();

    if gist_url.is_empty() {
        return Json(ScanResponse {
            sources: vec![],
            total_sources: 0,
        });
    }

    tracing::info!("[FOLDER-SOURCE] Scanning from gist: {}", gist_url);

    // Step 1: Fetch the Gist JSON
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    let entries: Vec<FolderSourceEntry> = match client.get(&gist_url).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                tracing::error!("[FOLDER-SOURCE] Gist fetch failed with status: {}", resp.status());
                return Json(ScanResponse {
                    sources: vec![],
                    total_sources: 0,
                });
            }
            match resp.json().await {
                Ok(e) => e,
                Err(e) => {
                    tracing::error!("[FOLDER-SOURCE] Failed to parse gist JSON: {}", e);
                    return Json(ScanResponse {
                        sources: vec![],
                        total_sources: 0,
                    });
                }
            }
        }
        Err(e) => {
            tracing::error!("[FOLDER-SOURCE] Failed to fetch gist: {}", e);
            return Json(ScanResponse {
                sources: vec![],
                total_sources: 0,
            });
        }
    };

    tracing::info!("[FOLDER-SOURCE] Found {} folder entries in gist", entries.len());

    // Step 2: For each folder, call v3 API and parse contents
    let mut sources = Vec::new();

    for entry in &entries {
        let result = scan_single_folder(&client, entry).await;
        sources.push(result);
    }

    let total_sources = sources.len();
    Json(ScanResponse {
        sources,
        total_sources,
    })
}

/// GET /api/folder-source/search?q=...&limit=50 — Search the FTS5 cache
async fn search_folder_cache(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Json<SearchResponse> {
    if params.q.trim().is_empty() {
        return Json(SearchResponse {
            results: vec![],
            total: 0,
            query: params.q,
        });
    }

    let limit = params.limit.min(200); // Cap at 200

    match state.folder_cache_service.search(&params.q, limit).await {
        Ok(results) => {
            let total: usize = results.len();
            Json(SearchResponse {
                results,
                total,
                query: params.q,
            })
        }
        Err(e) => {
            tracing::error!("[FOLDER-SOURCE] Search error: {}", e);
            Json(SearchResponse {
                results: vec![],
                total: 0,
                query: params.q,
            })
        }
    }
}

/// POST /api/folder-source/sync — Manually trigger a full cache sync
async fn trigger_sync(
    State(state): State<Arc<AppState>>,
) -> Json<ActionResponse> {
    let service = Arc::clone(&state.folder_cache_service);

    // Spawn the sync in background so the endpoint returns immediately
    tokio::spawn(async move {
        tracing::info!("[FOLDER-SOURCE] Manual sync triggered");
        match service.sync_all_sources().await {
            Ok(report) => {
                tracing::info!(
                    "[FOLDER-SOURCE] Manual sync complete: {} items from {} sources ({} folders) in {:.1}s",
                    report.total_items, report.total_sources, report.total_folders_scanned, report.duration_secs
                );
            }
            Err(e) => {
                tracing::error!("[FOLDER-SOURCE] Manual sync failed: {}", e);
            }
        }
    });

    Json(ActionResponse {
        success: true,
        message: Some("Sync started in background".to_string()),
    })
}

/// GET /api/folder-source/status — Get sync status
async fn get_sync_status(
    State(state): State<Arc<AppState>>,
) -> Json<crate::services::folder_cache_service::SyncStatus> {
    let status = state.folder_cache_service.get_sync_status();
    Json(status)
}

// ============================================================================
// Helpers
// ============================================================================

/// Extract the folder linkcode from a Fshare folder URL
/// e.g. "https://www.fshare.vn/folder/F8INH63OUZ6L817?token=..." → "F8INH63OUZ6L817"
fn extract_folder_code(folder_url: &str) -> Option<String> {
    if !folder_url.contains("/folder/") {
        return None;
    }
    let after = folder_url.split("/folder/").last()?;
    let code = after.split('?').next().unwrap_or("");
    if code.is_empty() {
        None
    } else {
        Some(code.to_string())
    }
}

/// Parse the Fshare v3 XML response into a list of items.
///
/// The XML structure is:
/// ```xml
/// <response>
///   <items>
///     <item>
///       <id>...</id>
///       <linkcode>VE41H65WVF9M</linkcode>
///       <name>Reality Z - Chương Trình Thực Tế Z Season 1_Việt Sub</name>
///       <type>0</type>
///       <size>0</size>
///       <mimetype></mimetype>
///       ...
///     </item>
///   </items>
///   <_links>
///     <next>/v3/files/folder?linkcode=...&amp;page=2&amp;per-page=50</next>
///   </_links>
/// </response>
/// ```
/// Auto-detect JSON or XML and parse accordingly
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
            tracing::error!("[FOLDER-SOURCE] JSON parse error: {}", e);
            return (vec![], false);
        }
    };

    let mut items = Vec::new();
    if let Some(arr) = parsed.get("items").and_then(|v| v.as_array()) {
        for item in arr {
            let linkcode = item.get("linkcode").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if linkcode.is_empty() || name.is_empty() { continue; }

            let id = item.get("id").map(|v| match v {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                _ => String::new(),
            }).unwrap_or_default();

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

            let mimetype = item.get("mimetype").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let path = item.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();

            items.push(FshareXmlItem {
                id,
                linkcode,
                name,
                r#type: item_type,
                size,
                mimetype,
                path,
            });
        }
    }

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
                            "id" => item.id = text,
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
                tracing::error!("[FOLDER-SOURCE] XML parse error: {}", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    (items, has_next_page)
}

/// Scan a single Fshare folder using the v3 API (XML response)
async fn scan_single_folder(
    client: &reqwest::Client,
    entry: &FolderSourceEntry,
) -> FolderSourceResult {
    let folder_code = match extract_folder_code(&entry.folder_url) {
        Some(code) => code,
        None => {
            return FolderSourceResult {
                category: entry.category.clone(),
                label: entry.label.clone(),
                folder_url: entry.folder_url.clone(),
                items: vec![],
                total_items: 0,
                error: Some("Invalid folder URL — could not extract linkcode".to_string()),
            };
        }
    };

    tracing::info!("[FOLDER-SOURCE] Scanning folder '{}' (code: {})", entry.label, folder_code);

    let mut all_items = Vec::new();
    let mut page = 1; // Fshare v3 API pages start from 1
    let per_page = 50;

    loop {
        // Build query params
        let page_str = page.to_string();
        let per_page_str = per_page.to_string();
        let mut query_params: Vec<(&str, &str)> = vec![
            ("linkcode", &folder_code),
            ("sort", "type"),
            ("page", &page_str),
            ("per-page", &per_page_str),
        ];

        // Extract token from URL if present
        let token_value = entry.folder_url
            .split('?')
            .nth(1)
            .and_then(|qs| qs.split('&').find(|p| p.starts_with("token=")))
            .and_then(|p| p.strip_prefix("token="))
            .unwrap_or("")
            .to_string();

        if !token_value.is_empty() {
            query_params.push(("token", &token_value));
        }

        let resp = match client.get("https://www.fshare.vn/api/v3/files/folder")
            .query(&query_params)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("[FOLDER-SOURCE] V3 API request failed for {}: {}", folder_code, e);
                return FolderSourceResult {
                    category: entry.category.clone(),
                    label: entry.label.clone(),
                    folder_url: entry.folder_url.clone(),
                    items: all_items,
                    total_items: 0,
                    error: Some(format!("API request failed: {}", e)),
                };
            }
        };

        if !resp.status().is_success() {
            tracing::error!("[FOLDER-SOURCE] V3 API returned status {} for {}", resp.status(), folder_code);
            break;
        }

        let body = match resp.text().await {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("[FOLDER-SOURCE] Failed to read response body for {}: {}", folder_code, e);
                break;
            }
        };

        // Parse the response (auto-detects JSON or XML)
        let (xml_items, has_next) = parse_folder_response(&body);

        if xml_items.is_empty() {
            tracing::info!("[FOLDER-SOURCE] Page {}: no items, stopping", page);
            break;
        }

        tracing::info!("[FOLDER-SOURCE] Page {}: {} items from folder '{}'", page, xml_items.len(), entry.label);

        for xml_item in &xml_items {
            // type "0" = folder, "1" = file in Fshare v3
            let is_dir = xml_item.r#type == "0" || xml_item.mimetype.is_empty();
            let size: u64 = xml_item.size.parse().unwrap_or(0);

            // Build the Fshare URL
            let fshare_url = if is_dir {
                format!("https://www.fshare.vn/folder/{}", xml_item.linkcode)
            } else {
                format!("https://www.fshare.vn/file/{}", xml_item.linkcode)
            };

            // Parse the name to detect movies/series
            let parsed = FilenameParser::parse(&xml_item.name);

            all_items.push(ParsedFolderItem {
                name: xml_item.name.clone(),
                linkcode: xml_item.linkcode.clone(),
                fshare_url,
                is_directory: is_dir,
                size,
                title: parsed.title,
                year: parsed.year,
                is_series: parsed.is_series,
                season: parsed.season,
                episode: parsed.episode,
                quality: parsed.quality_attrs.quality_name(),
            });
        }

        // Check if there are more pages
        if !has_next || xml_items.len() < per_page {
            break;
        }

        page += 1;

        // Safety limit
        if page > 100 {
            tracing::warn!("[FOLDER-SOURCE] Hit pagination safety limit for folder '{}'", entry.label);
            break;
        }
    }

    let total_items = all_items.len();
    tracing::info!("[FOLDER-SOURCE] Folder '{}' scanned: {} total items", entry.label, total_items);

    FolderSourceResult {
        category: entry.category.clone(),
        label: entry.label.clone(),
        folder_url: entry.folder_url.clone(),
        items: all_items,
        total_items,
        error: None,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_folder_code() {
        assert_eq!(
            extract_folder_code("https://www.fshare.vn/folder/F8INH63OUZ6L817?token=1772074221"),
            Some("F8INH63OUZ6L817".to_string())
        );
        assert_eq!(
            extract_folder_code("https://www.fshare.vn/folder/XEVZ47FBZSR4"),
            Some("XEVZ47FBZSR4".to_string())
        );
        assert_eq!(extract_folder_code("https://fshare.vn/file/ABC123"), None);
        assert_eq!(extract_folder_code("garbage"), None);
    }

    #[test]
    fn test_parse_folder_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<response>
    <items>
        <item>
            <id>74053746</id>
            <linkcode>VE41H65WVF9M</linkcode>
            <name>Reality Z - Chương Trình Thực Tế Z Season 1_Việt Sub</name>
            <type>0</type>
            <size>0</size>
            <mimetype></mimetype>
            <path>/01_Folder Chính/Phim Bộ Mỹ</path>
        </item>
        <item>
            <id>74185488</id>
            <linkcode>R53RB1UZ5HMH</linkcode>
            <name>Traces.S01.1080p.AMZN.WEBRip.DDP2.0.x264-Cinefeel</name>
            <type>0</type>
            <size>0</size>
            <mimetype></mimetype>
            <path>/01_Folder Chính/Phim Bộ Mỹ</path>
        </item>
        <item>
            <id>74501068</id>
            <linkcode>BN243UOLOA7U</linkcode>
            <name>Mrs.America.S01.1080p.AMZN.WEBRip.DDP5.1.x264-NTb</name>
            <type>0</type>
            <size>0</size>
            <mimetype></mimetype>
            <path>/01_Folder Chính/Phim Bộ Mỹ</path>
        </item>
        <item>
            <id>74516126</id>
            <linkcode>454TL817M8Z47GC</linkcode>
            <name>Peaky Blinders - Bóng Ma Anh Quốc Season 6</name>
            <type>0</type>
            <size>0</size>
            <mimetype></mimetype>
            <path>/01_Folder Chính/Phim Bộ Mỹ</path>
        </item>
    </items>
    <_links>
        <self>/v3/files/folder?linkcode=XEVZ47FBZSR4&amp;page=1&amp;per-page=50</self>
        <first>/v3/files/folder?linkcode=XEVZ47FBZSR4&amp;page=1&amp;per-page=50</first>
        <last>/v3/files/folder?linkcode=XEVZ47FBZSR4&amp;page=30&amp;per-page=50</last>
        <next>/v3/files/folder?linkcode=XEVZ47FBZSR4&amp;page=2&amp;per-page=50</next>
    </_links>
</response>"#;

        let (items, has_next) = parse_folder_xml(xml);

        // Should find 4 items
        assert_eq!(items.len(), 4);
        assert!(has_next, "Should detect next page");

        // Verify first item
        assert_eq!(items[0].linkcode, "VE41H65WVF9M");
        assert_eq!(items[0].name, "Reality Z - Chương Trình Thực Tế Z Season 1_Việt Sub");
        assert_eq!(items[0].r#type, "0");

        // Verify parsing of standard release format
        assert_eq!(items[1].name, "Traces.S01.1080p.AMZN.WEBRip.DDP2.0.x264-Cinefeel");
        assert_eq!(items[2].name, "Mrs.America.S01.1080p.AMZN.WEBRip.DDP5.1.x264-NTb");
    }

    #[test]
    fn test_parse_folder_names_with_filename_parser() {
        // These are REAL folder names from the Fshare API.
        // Names are highly varied — this test is DIAGNOSTIC only.
        // It prints what FilenameParser extracts; no strict assertions
        // since naming patterns from Fshare folders are unpredictable.
        let names = vec![
            // Standard scene format
            "Traces.S01.1080p.AMZN.WEBRip.DDP2.0.x264-Cinefeel",
            "Mrs.America.S01.1080p.AMZN.WEBRip.DDP5.1.x264-NTb",
            "4400.S01.1080p.AMZN.WEBRip.DDP5.1.x264-NTb",
            "The.Beatles.Get.Back.S01.2021",
            // Vietnamese naming: "English Title - Vietnamese Title Year_SubFormat"
            "Inventing Anna - Tiểu Thư Dựng Chuyện 2022_Việt Sub",
            "Vikings Valhalla - Huyền Thoại Vikings Valhalla 2022_Việt Sub",
            "Peaky Blinders - Bóng Ma Anh Quốc Season 6",
            "Reality Z - Chương Trình Thực Tế Z Season 1_Việt Sub",
            "Cupids Kitchen - Vị Giác Tình Yêu 2022_Thuyết Minh",
            "Oggy and the Cockroaches Collection (1998 – 2018)",
            "Thái Tử Phi Giá Đáo 2020 27 tập HD1080p LT - Fake Princess",
            "From - Bẫy_Việt Sub",
        ];

        println!("\n📊 Folder Name Parsing Results (diagnostic)\n");
        for name in &names {
            let parsed = FilenameParser::parse(name);
            println!("Input:    {}", name);
            println!("  title:     '{}'", parsed.title);
            println!("  is_series: {}", parsed.is_series);
            println!("  season:    {:?}", parsed.season);
            println!("  episode:   {:?}", parsed.episode);
            println!("  year:      {:?}", parsed.year);
            println!("  quality:   {}", parsed.quality_attrs.quality_name());
            println!();
        }

        // Only assert that parsing doesn't panic — all names are parseable
        assert!(names.len() > 0);
    }

    #[test]
    fn test_parse_folder_xml_no_next_page() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<response>
    <items>
        <item>
            <id>1</id>
            <linkcode>ABC123</linkcode>
            <name>Test Movie 2024</name>
            <type>0</type>
            <size>0</size>
            <mimetype></mimetype>
            <path>/test</path>
        </item>
    </items>
    <_links>
        <self>/v3/files/folder?linkcode=TEST&amp;page=1&amp;per-page=50</self>
    </_links>
</response>"#;

        let (items, has_next) = parse_folder_xml(xml);
        assert_eq!(items.len(), 1);
        assert!(!has_next, "Should not detect next page");
    }
}
