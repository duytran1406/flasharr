use serde::{Deserialize, Serialize};

use crate::api::search::MultiSearchResult;
use crate::providers::fshare::ProviderSearchResult;
use crate::torbox::client::TorBoxCacheHit;
use crate::utils::title_matcher::{SmartSearchResult, StreamBehaviorHints};
use crate::AppState;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheAvailability {
    Cached,
    PartiallyCached,
    NotCached,
    Unknown,
}

impl Default for CacheAvailability {
    fn default() -> Self {
        Self::Unknown
    }
}

impl CacheAvailability {
    pub fn from_legacy(cached: Option<bool>) -> Self {
        match cached {
            Some(true) => Self::Cached,
            Some(false) => Self::NotCached,
            None => Self::Unknown,
        }
    }

    pub const fn display_source<'a>(self, discovered_source: &'a str) -> &'a str {
        if matches!(self, Self::Cached) {
            "debrid"
        } else {
            discovered_source
        }
    }
}

/// Lookup-only enrichment: mutate discovered rows, never create a second projection.
pub trait CacheEnrichRow {
    fn info_hash(&self) -> Option<&str>;
    fn mark_cached(&mut self, hit: &TorBoxCacheHit);
}

fn apply_metadata(
    name: &mut String,
    size: &mut u64,
    behavior_hints: &mut Option<StreamBehaviorHints>,
    hit: &TorBoxCacheHit,
    file_idx: Option<u32>,
) {
    let (file, resolved_size) = crate::sources::torbox::resolved_metadata(hit, file_idx);
    if let Some(resolved_size) = resolved_size {
        *size = resolved_size;
    }
    if let Some(file) = file {
        *name = file.name.clone();
        let hints = behavior_hints.get_or_insert_with(|| StreamBehaviorHints {
            binge_group: None,
            filename: None,
            video_size: None,
        });
        hints.filename = Some(file.name.clone());
        hints.video_size = Some(file.size);
    }
}

impl CacheEnrichRow for MultiSearchResult {
    fn info_hash(&self) -> Option<&str> {
        self.info_hash.as_deref()
    }
    fn mark_cached(&mut self, hit: &TorBoxCacheHit) {
        apply_metadata(
            &mut self.name,
            &mut self.size,
            &mut self.behavior_hints,
            hit,
            self.file_idx,
        );
        self.origin_provider = Some(self.provider.clone());
        self.origin_indexer = self.indexer.clone();
        self.source = CacheAvailability::Cached
            .display_source(&self.source)
            .into();
        self.provider = "torbox".into();
        self.cached = Some(true);
    }
}

impl CacheEnrichRow for SmartSearchResult {
    fn info_hash(&self) -> Option<&str> {
        self.info_hash.as_deref()
    }
    fn mark_cached(&mut self, hit: &TorBoxCacheHit) {
        apply_metadata(
            &mut self.name,
            &mut self.size,
            &mut self.behavior_hints,
            hit,
            self.file_idx,
        );
        if self.origin_provider.is_none() {
            self.origin_provider = self.provider.clone();
        }
        if self.origin_indexer.is_none() {
            self.origin_indexer = self.indexer.clone();
        }
        self.result_source = Some(
            CacheAvailability::Cached
                .display_source(self.result_source.as_deref().unwrap_or("torrent"))
                .into(),
        );
        self.provider = Some("torbox".into());
        self.cached = Some(true);
    }
}

impl CacheEnrichRow for ProviderSearchResult {
    fn info_hash(&self) -> Option<&str> {
        self.info_hash.as_deref()
    }
    fn mark_cached(&mut self, hit: &TorBoxCacheHit) {
        apply_metadata(
            &mut self.name,
            &mut self.size,
            &mut self.behavior_hints,
            hit,
            self.file_idx,
        );
        self.origin_provider = self.provider.clone();
        self.origin_indexer = self.indexer.clone();
        self.result_source = Some(
            CacheAvailability::Cached
                .display_source(self.result_source.as_deref().unwrap_or("torrent"))
                .into(),
        );
        self.provider = Some("torbox".into());
        self.cached = Some(true);
    }
}

impl<T: CacheEnrichRow + ?Sized> CacheEnrichRow for &mut T {
    fn info_hash(&self) -> Option<&str> {
        (**self).info_hash()
    }

    fn mark_cached(&mut self, hit: &TorBoxCacheHit) {
        (**self).mark_cached(hit);
    }
}

pub fn apply_canonical_cache_hits<R: CacheEnrichRow>(
    results: &mut [R],
    cached: &std::collections::HashMap<String, TorBoxCacheHit>,
) -> usize {
    results
        .iter_mut()
        .filter_map(|item| {
            item.info_hash()
                .and_then(crate::torrent::normalize_info_hash)
                .and_then(|hash| cached.get(&hash))
                .map(|hit| (item, hit))
        })
        .map(|(item, hit)| item.mark_cached(hit))
        .count()
}

pub async fn enrich_canonical_search_results<R: CacheEnrichRow>(
    state: &Arc<AppState>,
    results: &mut [R],
) -> Result<usize, String> {
    let hashes = results
        .iter()
        .filter_map(|item| {
            item.info_hash()
                .and_then(crate::torrent::normalize_info_hash)
        })
        .collect::<Vec<_>>();
    let outcome = crate::sources::torbox::cached_hits(state, &hashes).await;
    let applied = apply_canonical_cache_hits(results, &outcome.hits);
    outcome
        .status
        .map(|error| Err(error.to_string()))
        .unwrap_or(Ok(applied))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalSearchResult {
    pub name: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_idx: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behavior_hints: Option<StreamBehaviorHints>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seeders: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leechers: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached: Option<bool>,
    #[serde(default)]
    pub availability: CacheAvailability,
}

fn playable_url(value: &str) -> Option<String> {
    value
        .starts_with("stremio://")
        .then(|| value.trim().to_string())
}

impl CanonicalSearchResult {
    pub fn validate(mut self) -> Result<Self, &'static str> {
        self.download_ref = self
            .download_ref
            .and_then(|v| (!v.trim().is_empty()).then(|| v.trim().to_string()));
        self.url = self.url.and_then(|v| playable_url(v.trim()));
        self.info_hash = self
            .info_hash
            .as_deref()
            .and_then(crate::torrent::normalize_info_hash);
        if self.info_hash.is_none() && self.download_ref.is_none() && self.url.is_none() {
            return Err("search result has no acquisition handle");
        }
        Ok(self)
    }

    pub fn dedupe_key(&self, source: &str, size: u64) -> String {
        let source = source.trim().to_ascii_lowercase();
        if let Some(hash) = &self.info_hash {
            // infoHash is the cross-provider torrent identity. Keep file identity when present.
            return format!("torrent:hash:{hash}:{:?}", self.file_idx);
        }
        if let Some(reference) = &self.download_ref {
            return format!("{source}:ref:{reference}");
        }
        format!(
            "{source}:meta:{}:{}:{size}",
            self.indexer.as_deref().unwrap_or("").to_lowercase(),
            self.title
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase()
        )
    }
}

impl From<&MultiSearchResult> for CanonicalSearchResult {
    fn from(item: &MultiSearchResult) -> Self {
        Self {
            name: item.name.clone(),
            title: item.parsed_name.clone(),
            info_hash: item.info_hash.as_ref().map(|v| v.to_ascii_lowercase()),
            file_idx: item.file_idx,
            sources: item.sources.clone(),
            behavior_hints: item.behavior_hints.clone(),
            download_ref: item.download_ref.clone(),
            url: playable_url(&item.url),
            provider: Some(item.provider.clone()),
            indexer: item.indexer.clone(),
            seeders: item.seeders,
            leechers: item.leechers,
            cached: item.cached,
            availability: CacheAvailability::from_legacy(item.cached),
        }
    }
}

impl From<&ProviderSearchResult> for CanonicalSearchResult {
    fn from(item: &ProviderSearchResult) -> Self {
        Self {
            name: item.name.clone(),
            title: item.name.clone(),
            info_hash: item.info_hash.as_ref().map(|v| v.to_ascii_lowercase()),
            file_idx: item.file_idx,
            sources: item.sources.clone(),
            behavior_hints: item.behavior_hints.clone(),
            download_ref: item.download_ref.clone(),
            url: playable_url(&item.url),
            provider: item.provider.clone(),
            indexer: item.indexer.clone(),
            seeders: item.seeders,
            leechers: item.leechers,
            cached: item.cached,
            availability: CacheAvailability::from_legacy(item.cached),
        }
    }
}

impl From<&SmartSearchResult> for CanonicalSearchResult {
    fn from(item: &SmartSearchResult) -> Self {
        Self {
            name: item.name.clone(),
            title: item.name.clone(),
            info_hash: item.info_hash.as_ref().map(|v| v.to_ascii_lowercase()),
            file_idx: item.file_idx,
            sources: item.sources.clone(),
            behavior_hints: item.behavior_hints.clone(),
            download_ref: item.download_ref.clone(),
            url: playable_url(&item.url),
            provider: item.provider.clone(),
            indexer: item.indexer.clone(),
            seeders: item.seeders,
            leechers: item.leechers,
            cached: item.cached,
            availability: CacheAvailability::from_legacy(item.cached),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_search_cache_enrichment_preserves_provenance_and_exact_file() {
        let mut result: SmartSearchResult = serde_json::from_value(serde_json::json!({
            "name": "original.mkv", "url": "", "size": 1, "score": 0,
            "quality_name": "", "quality_score": 0, "custom_format_score": 0,
            "total_score": 0, "normalized_score": 0.0, "match_type": "exact",
            "quality_attrs": crate::utils::parser::QualityAttributes::default(),
            "tmdb_id": null, "poster_path": null, "vietdub": false, "vietsub": false,
            "hdr": false, "dolby_vision": false, "result_source": "torrent",
            "provider": "stremio", "indexer": "addon", "info_hash": "abc", "file_idx": 1
        }))
        .unwrap();
        result.mark_cached(&TorBoxCacheHit {
            hash: "abc".into(),
            name: None,
            size: 0,
            files: vec![
                crate::torbox::client::TorBoxFile {
                    id: 1,
                    name: "wrong.mkv".into(),
                    size: 10,
                    mimetype: None,
                },
                crate::torbox::client::TorBoxFile {
                    id: 2,
                    name: "episode.mkv".into(),
                    size: 20,
                    mimetype: None,
                },
            ],
        });

        assert_eq!(result.name, "episode.mkv");
        assert_eq!(result.size, 20);
        assert_eq!(result.result_source.as_deref(), Some("debrid"));
        assert_eq!(result.provider.as_deref(), Some("torbox"));
        assert_eq!(result.origin_provider.as_deref(), Some("stremio"));
        assert_eq!(result.origin_indexer.as_deref(), Some("addon"));
        assert_eq!(
            result.behavior_hints.unwrap().filename.as_deref(),
            Some("episode.mkv")
        );
    }

    #[test]
    fn cache_metadata_applies_exact_stremio_manifest_file() {
        let mut name = "stream".to_string();
        let mut size = 0;
        let mut hints = Some(StreamBehaviorHints {
            binge_group: None,
            filename: None,
            video_size: Some(0),
        });
        apply_metadata(
            &mut name,
            &mut size,
            &mut hints,
            &TorBoxCacheHit {
                hash: "abc".into(),
                name: None,
                size: 0,
                files: vec![
                    crate::torbox::client::TorBoxFile {
                        id: 1,
                        name: "first.mkv".into(),
                        size: 10,
                        mimetype: None,
                    },
                    crate::torbox::client::TorBoxFile {
                        id: 2,
                        name: "episode.mkv".into(),
                        size: 2_000_000_000,
                        mimetype: None,
                    },
                ],
            },
            Some(1),
        );
        assert_eq!(name, "episode.mkv");
        assert_eq!(size, 2_000_000_000);
        assert_eq!(hints.unwrap().video_size, Some(2_000_000_000));
    }

    #[test]
    fn cache_metadata_preserves_known_size_when_hit_has_no_usable_size() {
        let mut name = "release.mkv".to_string();
        let mut size = 42;
        let mut hints = None;
        apply_metadata(
            &mut name,
            &mut size,
            &mut hints,
            &TorBoxCacheHit {
                hash: "abc".into(),
                name: None,
                size: 0,
                files: vec![],
            },
            None,
        );
        assert_eq!(name, "release.mkv");
        assert_eq!(size, 42);
        assert!(hints.is_none());
    }

    #[test]
    fn stremio_fixture_normalizes_hash_and_uses_camel_case() {
        let fixture = r#"{"name":"Pack","title":"4K","infoHash":"0123456789ABCDEF0123456789ABCDEF01234567","fileIdx":2,"downloadRef":"opaque","behaviorHints":{"filename":"pack.mkv","videoSize":42}}"#;
        let canonical: CanonicalSearchResult = serde_json::from_str(fixture).unwrap();
        let canonical = canonical.validate().unwrap();
        assert_eq!(
            canonical.info_hash.as_deref(),
            Some("0123456789abcdef0123456789abcdef01234567")
        );
        let wire = serde_json::to_value(canonical).unwrap();
        assert_eq!(wire["downloadRef"], "opaque");
        assert!(wire.get("download_ref").is_none());
    }

    #[test]
    fn raw_prowlarr_locator_is_not_a_playable_url() {
        assert_eq!(playable_url("prowlarr-ref://opaque"), None);
    }

    #[test]
    fn acquisition_handle_is_required() {
        let result: CanonicalSearchResult =
            serde_json::from_str(r#"{"name":"x","title":"x"}"#).unwrap();
        assert!(result.validate().is_err());
    }

    #[test]
    fn info_hash_dedupes_across_discovery_sources() {
        let mut a: CanonicalSearchResult = serde_json::from_str(
            r#"{"name":"x","title":"x","infoHash":"0123456789abcdef0123456789abcdef01234567","fileIdx":2}"#,
        ).unwrap();
        let mut b = a.clone();
        a.download_ref = Some("prowlarr-ref".into());
        b.url = Some("stremio://stream".into());
        let a = a.validate().unwrap();
        let b = b.validate().unwrap();
        assert_eq!(a.dedupe_key("torrent", 42), b.dedupe_key("stremio", 42));
    }

    #[test]
    fn validation_rejects_empty_handles_and_invalid_hashes() {
        let mut result: CanonicalSearchResult =
            serde_json::from_str(r#"{"name":"x","title":"x","downloadRef":"   "}"#).unwrap();
        assert!(result.clone().validate().is_err());
        result.download_ref = None;
        result.info_hash = Some("ABCDEF".into());
        assert!(result.validate().is_err());
    }

    #[test]
    fn prowlarr_multi_adapter_hides_locator_and_normalizes_hash() {
        let item = MultiSearchResult {
            id: "id".into(),
            name: "Release Title".into(),
            parsed_name: "Release".into(),
            url: "magnet:?xt=urn:btih:0123456789ABCDEF0123456789ABCDEF01234567".into(),
            download_ref: Some(" opaque ".into()),
            size: 42,
            source: "torrent".into(),
            provider: "prowlarr".into(),
            indexer: Some("NetHD".into()),
            origin_provider: None,
            origin_indexer: None,
            cached: None,
            file_idx: Some(2),
            sources: Some(vec!["tracker:x".into()]),
            behavior_hints: Some(StreamBehaviorHints {
                binge_group: None,
                filename: Some("release.mkv".into()),
                video_size: Some(42),
            }),
            seeders: None,
            leechers: None,
            info_hash: Some("0123456789ABCDEF0123456789ABCDEF01234567".into()),
            quality: None,
            resolution: None,
            episode_tag: None,
            media_type: None,
            inspection: None,
            inspection_score: None,
        };
        let canonical = CanonicalSearchResult::from(&item).validate().unwrap();
        assert_eq!(canonical.url, None);
        assert_eq!(canonical.download_ref.as_deref(), Some("opaque"));
        assert_eq!(
            canonical.info_hash.as_deref(),
            Some("0123456789abcdef0123456789abcdef01234567")
        );
        assert_eq!(canonical.file_idx, Some(2));
        assert!(!serde_json::to_string(&canonical)
            .unwrap()
            .contains("magnet:"));
    }
}
