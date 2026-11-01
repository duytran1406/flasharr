//! Folder-cache-backed Fshare search primitives.

use serde::{Deserialize, Serialize};

/// Allowed media file extensions (lowercase, without dot).
pub const MEDIA_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "m2ts", "vob", "3gp",
    "ogv", "divx", "rmvb", "rar", "zip", "7z",
];

/// Check if a filename has a media file extension.
pub fn is_media_file(filename: &str) -> bool {
    if let Some(dot_pos) = filename.rfind('.') {
        let ext = &filename[dot_pos + 1..].to_lowercase();
        MEDIA_EXTENSIONS.contains(&ext.as_str())
    } else {
        true
    }
}

/// Reject executable payloads while allowing extensionless torrent release names.
pub fn is_safe_search_result(filename: &str) -> bool {
    let extension = filename
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    !matches!(
        extension.as_deref(),
        Some(
            "apk"
                | "bat"
                | "cmd"
                | "com"
                | "dmg"
                | "exe"
                | "jar"
                | "js"
                | "msi"
                | "pkg"
                | "ps1"
                | "scr"
                | "vbs"
        )
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSearchResult {
    pub name: String,
    pub url: String,
    pub fcode: String,
    pub size: u64,
    pub score: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seeders: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leechers: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_indexer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached: Option<bool>,
    #[serde(default, alias = "fileIdx", skip_serializing_if = "Option::is_none")]
    pub file_idx: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<String>>,
    #[serde(
        default,
        alias = "behaviorHints",
        skip_serializing_if = "Option::is_none"
    )]
    pub behavior_hints: Option<crate::utils::title_matcher::StreamBehaviorHints>,
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub inspection: Option<crate::api::search::TorrentInspectionSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspection_score: Option<i32>,
}

pub async fn search_folder_cache(
    service: &super::folder_cache::FolderCacheService,
    queries: &[String],
    limit: u32,
) -> Vec<ProviderSearchResult> {
    let mut seen = std::collections::HashSet::new();
    let mut matches = Vec::new();

    for query in queries {
        if matches.len() >= limit as usize {
            break;
        }
        for query in super::folder_cache::expand_folder_cache_query_variants(query) {
            if matches.len() >= limit as usize {
                break;
            }
            let remaining = limit.saturating_sub(matches.len() as u32);
            let items = match service.search(&query, remaining).await {
                Ok(items) => items,
                Err(error) => {
                    tracing::warn!("Fshare folder-cache search '{query}' failed: {error}");
                    continue;
                }
            };
            for item in items {
                if !item.is_directory
                    && is_media_file(&item.name)
                    && seen.insert(item.linkcode.clone())
                {
                    matches.push(ProviderSearchResult {
                        name: item.name,
                        url: item.fshare_url,
                        fcode: item.linkcode,
                        size: item.size,
                        score: 0,
                        result_source: None,
                        provider: Some("fshare".to_string()),
                        indexer: None,
                        seeders: None,
                        leechers: None,
                        info_hash: None,
                        download_ref: None,
                        origin_provider: None,
                        origin_indexer: None,
                        cached: None,
                        file_idx: None,
                        sources: None,
                        behavior_hints: None,
                        inspection: None,
                        inspection_score: None,
                    });
                }
            }
        }
    }

    matches.truncate(limit as usize);
    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::Db, services::TmdbService};
    use std::sync::Arc;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn production_search_reads_seeded_folder_cache() {
        let tmp = NamedTempFile::new().expect("temp database");
        let db = Arc::new(Db::new(tmp.path()).expect("test database"));
        db.insert_folder_cache_batch(&[crate::db::sqlite::CachedFolderItem {
            linkcode: "CACHE001".to_string(),
            name: "Cache.Only.Movie.2026.mkv".to_string(),
            title: "Cache Only Movie".to_string(),
            category: "movie".to_string(),
            label: "test".to_string(),
            parent_linkcode: "ROOT".to_string(),
            fshare_url: "https://www.fshare.vn/file/CACHE001".to_string(),
            year: Some(2026),
            season: None,
            episode: None,
            is_series: false,
            is_directory: false,
            size: 123,
            quality: "1080p".to_string(),
            path: "/movies".to_string(),
            tmdb_id: None,
            media_type_hint: None,
            poster_path: None,
        }])
        .expect("seed folder cache");
        let service = super::super::folder_cache::FolderCacheService::new(
            db,
            Arc::new(TmdbService::new_with_api_key(String::new())),
        );

        let results = search_folder_cache(&service, &["Cache Only Movie".to_string()], 10).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fcode, "CACHE001");
        assert_eq!(results[0].provider.as_deref(), Some("fshare"));
    }

    #[test]
    fn media_file_filter_rejects_non_media_extensions() {
        for name in ["Movie.2025.mkv", "Season.1.rar", "Untitled release"] {
            assert!(is_media_file(name), "{name} should be accepted");
        }
        for name in ["Movie.exe", "Movie.nfo", "Movie.srt", "Movie.iso"] {
            assert!(!is_media_file(name), "{name} should be rejected");
        }
        for name in ["Movie.exe", "Movie.msi", "Movie.scr"] {
            assert!(!is_safe_search_result(name), "{name} should be rejected");
        }
    }
}
