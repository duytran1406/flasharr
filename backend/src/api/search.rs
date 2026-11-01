use crate::api::search_pipeline::{is_media_file, search_folder_cache};
use crate::utils::smart_tokenizer::smart_parse;
use crate::utils::title_matcher::{
    calculate_unified_similarity, extract_core_title, get_title_keywords,
};
use crate::AppState;
use anyhow::Context;
use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use futures_util::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tracing::warn;

fn search_admission() -> &'static Arc<tokio::sync::Semaphore> {
    use once_cell::sync::Lazy;
    static ADMISSION: Lazy<Arc<tokio::sync::Semaphore>> =
        Lazy::new(|| Arc::new(tokio::sync::Semaphore::new(4)));
    &ADMISSION
}

fn search_busy_response() -> axum::response::Response {
    use axum::response::IntoResponse;
    let mut response = (
        axum::http::StatusCode::TOO_MANY_REQUESTS,
        axum::Json(serde_json::json!({ "error": "search_busy" })),
    )
        .into_response();
    response.headers_mut().insert(
        axum::http::header::RETRY_AFTER,
        axum::http::HeaderValue::from_static("1"),
    );
    response
}

fn try_search_permit(
    semaphore: &Arc<tokio::sync::Semaphore>,
) -> Result<tokio::sync::OwnedSemaphorePermit, axum::response::Response> {
    Arc::clone(semaphore)
        .try_acquire_owned()
        .map_err(|_| search_busy_response())
}

async fn limit_search_concurrency(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let Ok(_permit) = try_search_permit(search_admission()) else {
        return search_busy_response();
    };
    next.run(request).await
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/enhanced", get(enhanced_search))
        .route("/multi", get(multi_search))
        .route("/", get(enhanced_search))
        .route("/smart", post(crate::api::smart_search::smart_search))
        .layer(axum::middleware::from_fn(limit_search_concurrency))
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

fn default_page() -> usize {
    1
}
fn default_limit() -> usize {
    20
}
fn default_enrich() -> bool {
    true
}

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

#[derive(Deserialize)]
pub struct MultiSearchQuery {
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Serialize, Clone, Debug)]
pub struct MultiSearchResult {
    pub id: String,
    pub name: String,
    pub parsed_name: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_ref: Option<String>,
    pub size: u64,
    pub source: String,
    pub provider: String,
    pub indexer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_indexer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached: Option<bool>,
    #[serde(rename = "fileIdx", skip_serializing_if = "Option::is_none")]
    pub file_idx: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<String>>,
    #[serde(rename = "behaviorHints", skip_serializing_if = "Option::is_none")]
    pub behavior_hints: Option<crate::utils::title_matcher::StreamBehaviorHints>,
    pub seeders: Option<i64>,
    pub leechers: Option<i64>,
    pub info_hash: Option<String>,
    pub quality: Option<String>,
    pub resolution: Option<String>,
    pub episode_tag: Option<String>,
    pub media_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inspection: Option<TorrentInspectionSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inspection_score: Option<i32>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TorrentContentKind {
    Movie,
    SingleEpisode,
    MultiEpisode,
    SeasonPack,
    MultiSeason,
    Unknown,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct TorrentInspectionSummary {
    pub status: &'static str,
    pub classification: TorrentContentKind,
    pub root_name: Option<String>,
    pub file_count: Option<usize>,
    pub media_file_count: Option<usize>,
    pub total_size: Option<u64>,
    #[serde(rename = "evidence_total")]
    pub evidence_total_count: usize,
    pub evidence_truncated: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub episode_evidence: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<crate::torrent::evidence::TorrentEvidence>,
}

fn episode_pairs(path: &str) -> Vec<(u32, u32)> {
    static RE: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| regex::Regex::new(r"(?i)S(\d{1,2})E(\d{1,3})").unwrap());
    RE.captures_iter(path)
        .filter_map(|c| Some((c[1].parse().ok()?, c[2].parse().ok()?)))
        .collect()
}

#[cfg(test)]
fn classify_torrent_paths(
    paths: &[&str],
    _intent: Option<(u32, Option<u32>)>,
) -> TorrentContentKind {
    let files = paths
        .iter()
        .enumerate()
        .map(|(index, path)| crate::torrent::metainfo::MetainfoFile {
            path_components: path.split('/').map(str::to_string).collect(),
            path: (*path).to_string(),
            length: 1,
            index,
        })
        .collect::<Vec<_>>();
    let meta = crate::torrent::metainfo::Metainfo {
        root_name: "fixture".into(),
        total_size: files.len() as u64,
        file_count: files.len(),
        media_file_count: files
            .iter()
            .filter(|f| crate::torrent::metainfo::is_media_path(&f.path))
            .count(),
        files,
    };
    match crate::torrent::evidence::analyze_metainfo(&meta, None).scope {
        crate::torrent::evidence::TorrentScope::Movie => TorrentContentKind::Movie,
        crate::torrent::evidence::TorrentScope::SingleEpisode => TorrentContentKind::SingleEpisode,
        crate::torrent::evidence::TorrentScope::SeasonPack => TorrentContentKind::SeasonPack,
        crate::torrent::evidence::TorrentScope::MultiEpisode => TorrentContentKind::MultiEpisode,
        crate::torrent::evidence::TorrentScope::MultiSeasonPack => TorrentContentKind::MultiSeason,
        crate::torrent::evidence::TorrentScope::Unknown => TorrentContentKind::Unknown,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InspectionFailureClass {
    DownloadRefUnavailable,
    LocatorInvalid,
    FetchTimeout,
    FetchNetwork,
    Http4xx,
    Http5xx,
    Redirect,
    RedirectCrossOriginNonHttps,
    RedirectTargetPrivateIp,
    RedirectTargetUserinfo,
    RedirectDnsResolutionFailed,
    RedirectLimit,
    RedirectStrictSameOrigin,
    RedirectLocationMissingOrInvalid,
    RedirectLocationJoinFailed,
    RedirectTargetMagnet,
    RedirectTargetSchemeOther,
    RedirectTargetHostMissing,
    ResponseType,
    TooLarge,
    MetainfoInvalid,
    Other,
}

fn classify_inspection_failure(error: &str) -> InspectionFailureClass {
    if error.starts_with("download ref unavailable") {
        InspectionFailureClass::DownloadRefUnavailable
    } else if error.starts_with("locator is not a torrent download") {
        InspectionFailureClass::LocatorInvalid
    } else if error.starts_with("torrent URL fetch failed:")
        || error.starts_with("torrent body read failed:")
    {
        if error.to_ascii_lowercase().contains("timeout")
            || error.to_ascii_lowercase().contains("timed out")
        {
            InspectionFailureClass::FetchTimeout
        } else {
            InspectionFailureClass::FetchNetwork
        }
    } else if error.starts_with("torrent URL returned HTTP 4") {
        InspectionFailureClass::Http4xx
    } else if error.starts_with("torrent URL returned HTTP 5") {
        InspectionFailureClass::Http5xx
    } else if error.starts_with("torrent redirect rejected: cross_origin_non_https") {
        InspectionFailureClass::RedirectCrossOriginNonHttps
    } else if error.starts_with("torrent redirect rejected: target_private_ip") {
        InspectionFailureClass::RedirectTargetPrivateIp
    } else if error.starts_with("torrent redirect rejected: target_userinfo") {
        InspectionFailureClass::RedirectTargetUserinfo
    } else if error.starts_with("torrent redirect rejected: dns_resolution_failed") {
        InspectionFailureClass::RedirectDnsResolutionFailed
    } else if error.starts_with("torrent redirect rejected: redirect_limit") {
        InspectionFailureClass::RedirectLimit
    } else if error.starts_with("torrent redirect rejected: strict_same_origin") {
        InspectionFailureClass::RedirectStrictSameOrigin
    } else if error.starts_with("torrent redirect rejected: location_missing_or_invalid") {
        InspectionFailureClass::RedirectLocationMissingOrInvalid
    } else if error.starts_with("torrent redirect rejected: location_join_failed") {
        InspectionFailureClass::RedirectLocationJoinFailed
    } else if error.starts_with("torrent redirect rejected: target_magnet") {
        InspectionFailureClass::RedirectTargetMagnet
    } else if error.starts_with("torrent redirect rejected: target_scheme_other") {
        InspectionFailureClass::RedirectTargetSchemeOther
    } else if error.starts_with("torrent redirect rejected: target_host_missing") {
        InspectionFailureClass::RedirectTargetHostMissing
    } else if error.starts_with("torrent redirect")
        || error.starts_with("cross-origin torrent redirect")
    {
        InspectionFailureClass::Redirect
    } else if error.starts_with("torrent response type is invalid") {
        InspectionFailureClass::ResponseType
    } else if error.starts_with("torrent body is too large") {
        InspectionFailureClass::TooLarge
    } else {
        InspectionFailureClass::Other
    }
}

async fn collect_until_deadline<Fut, T>(
    jobs: Vec<Fut>,
    deadline: tokio::time::Instant,
    concurrency: usize,
) -> (Vec<T>, usize)
where
    Fut: std::future::Future<Output = T>,
{
    let total = jobs.len();
    let work = stream::iter(jobs.into_iter().enumerate())
        .map(|(ordinal, job)| async move { (ordinal, job.await) })
        .buffer_unordered(concurrency.max(1));
    tokio::pin!(work);
    let mut completed = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, work.next()).await {
            Ok(Some(item)) => completed.push(item),
            Ok(None) | Err(_) => break,
        }
    }
    completed.sort_by_key(|(ordinal, _)| *ordinal);
    let completed: Vec<T> = completed.into_iter().map(|(_, item)| item).collect();
    let timed_out = total.saturating_sub(completed.len());
    (completed, timed_out)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct InspectionCounts {
    pub(crate) selected: usize,
    pub(crate) inspected: usize,
    pub(crate) failed: usize,
    pub(crate) timed_out: usize,
    pub(crate) cache_hits: usize,
    pub(crate) unavailable: usize,
    pub(crate) download_ref_unavailable: usize,
    pub(crate) locator_invalid: usize,
    pub(crate) fetch_timeout: usize,
    pub(crate) fetch_network: usize,
    pub(crate) status_4xx: usize,
    pub(crate) status_5xx: usize,
    pub(crate) redirect: usize,
    pub(crate) cross_origin_non_https: usize,
    pub(crate) target_private_ip: usize,
    pub(crate) target_userinfo: usize,
    pub(crate) dns_resolution_failed: usize,
    pub(crate) redirect_limit: usize,
    pub(crate) strict_same_origin: usize,
    pub(crate) location_missing_or_invalid: usize,
    pub(crate) location_join_failed: usize,
    pub(crate) target_magnet: usize,
    pub(crate) target_scheme_other: usize,
    pub(crate) target_host_missing: usize,
    pub(crate) response_type: usize,
    pub(crate) too_large: usize,
    pub(crate) metainfo_invalid: usize,
    pub(crate) other: usize,
}

impl InspectionCounts {
    fn record_failure(&mut self, class: InspectionFailureClass) {
        match class {
            InspectionFailureClass::DownloadRefUnavailable => self.download_ref_unavailable += 1,
            InspectionFailureClass::LocatorInvalid => self.locator_invalid += 1,
            InspectionFailureClass::FetchTimeout => self.fetch_timeout += 1,
            InspectionFailureClass::FetchNetwork => self.fetch_network += 1,
            InspectionFailureClass::Http4xx => self.status_4xx += 1,
            InspectionFailureClass::Http5xx => self.status_5xx += 1,
            InspectionFailureClass::Redirect => self.redirect += 1,
            InspectionFailureClass::RedirectCrossOriginNonHttps => {
                self.redirect += 1;
                self.cross_origin_non_https += 1;
            }
            InspectionFailureClass::RedirectTargetPrivateIp => {
                self.redirect += 1;
                self.target_private_ip += 1;
            }
            InspectionFailureClass::RedirectTargetUserinfo => {
                self.redirect += 1;
                self.target_userinfo += 1;
            }
            InspectionFailureClass::RedirectDnsResolutionFailed => {
                self.redirect += 1;
                self.dns_resolution_failed += 1;
            }
            InspectionFailureClass::RedirectLimit => {
                self.redirect += 1;
                self.redirect_limit += 1;
            }
            InspectionFailureClass::RedirectStrictSameOrigin => {
                self.redirect += 1;
                self.strict_same_origin += 1;
            }
            InspectionFailureClass::RedirectLocationMissingOrInvalid => {
                self.redirect += 1;
                self.location_missing_or_invalid += 1;
            }
            InspectionFailureClass::RedirectLocationJoinFailed => {
                self.redirect += 1;
                self.location_join_failed += 1;
            }
            InspectionFailureClass::RedirectTargetMagnet => {
                self.redirect += 1;
                self.target_magnet += 1;
            }
            InspectionFailureClass::RedirectTargetSchemeOther => {
                self.redirect += 1;
                self.target_scheme_other += 1;
            }
            InspectionFailureClass::RedirectTargetHostMissing => {
                self.redirect += 1;
                self.target_host_missing += 1;
            }
            InspectionFailureClass::ResponseType => self.response_type += 1,
            InspectionFailureClass::TooLarge => self.too_large += 1,
            InspectionFailureClass::MetainfoInvalid => self.metainfo_invalid += 1,
            InspectionFailureClass::Other => self.other += 1,
        }
    }
}

async fn enrich_prowlarr_candidates<F, Fut>(
    rows: &mut [MultiSearchResult],
    intent: Option<&TorrentSearchIntent>,
    cache: &moka::future::Cache<String, TorrentInspectionSummary>,
    deadline: tokio::time::Instant,
    fetch: F,
) -> InspectionCounts
where
    F: Fn(String) -> Fut + Clone,
    Fut: std::future::Future<Output = Result<Vec<u8>, String>>,
{
    if matches!(intent, Some(TorrentSearchIntent::FreeText)) {
        return InspectionCounts {
            unavailable: rows.len(),
            ..Default::default()
        };
    }
    let mut seen = std::collections::HashSet::new();
    let mut jobs = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let locator = row.download_ref.clone()?;
            let hash = normalized_info_hash(row.info_hash.as_deref()?)?;
            let key = format!(
                "torrent-evidence:v{}:{hash}:size:{}",
                crate::torrent::evidence::SCHEMA_VERSION,
                row.size
            );
            seen.insert(key.clone()).then_some((
                index,
                key,
                locator,
                row.seeders.unwrap_or(0),
                row.size,
            ))
        })
        .collect::<Vec<_>>();
    jobs.sort_by(|a, b| b.3.cmp(&a.3));
    jobs.truncate(10);
    let selected = jobs.len();
    let selected_indices = jobs
        .iter()
        .map(|(index, _, _, _, _)| *index)
        .collect::<std::collections::HashSet<_>>();
    let jobs = jobs
        .into_iter()
        .map(|(index, key, locator, _, declared_size)| {
            let cache = cache.clone();
            let fetch = fetch.clone();
            async move {
                if let Some(value) = cache.get(&key).await {
                    return (index, value, true, None);
                }
                let (summary, failure_class) = match fetch(locator).await {
                    Err(error) => (
                        TorrentInspectionSummary {
                            status: "failed",
                            classification: TorrentContentKind::Unknown,
                            root_name: None,
                            file_count: None,
                            media_file_count: None,
                            total_size: None,
                            evidence_total_count: 0,
                            evidence_truncated: false,
                            episode_evidence: vec![],
                            evidence: None,
                        },
                        Some(classify_inspection_failure(&error)),
                    ),
                    Ok(bytes) => match crate::torrent::metainfo::inspect_metainfo(&bytes) {
                        Ok(meta) => {
                            let evidence = crate::torrent::evidence::analyze_metainfo(
                                &meta,
                                Some(declared_size),
                            );
                            let classification = match evidence.scope {
                                crate::torrent::evidence::TorrentScope::Movie => {
                                    TorrentContentKind::Movie
                                }
                                crate::torrent::evidence::TorrentScope::SingleEpisode => {
                                    TorrentContentKind::SingleEpisode
                                }
                                crate::torrent::evidence::TorrentScope::MultiEpisode => {
                                    TorrentContentKind::MultiEpisode
                                }
                                crate::torrent::evidence::TorrentScope::MultiSeasonPack => {
                                    TorrentContentKind::MultiSeason
                                }
                                crate::torrent::evidence::TorrentScope::SeasonPack => {
                                    TorrentContentKind::SeasonPack
                                }
                                crate::torrent::evidence::TorrentScope::Unknown => {
                                    TorrentContentKind::Unknown
                                }
                            };
                            let evidence_total_count = evidence.files.len();
                            let mut wire_evidence = evidence.clone();
                            wire_evidence.files.truncate(100);
                            (
                                TorrentInspectionSummary {
                                    status: "inspected",
                                    classification,
                                    root_name: Some(meta.root_name.clone()),
                                    file_count: Some(meta.file_count),
                                    media_file_count: Some(meta.media_file_count),
                                    total_size: Some(meta.total_size),
                                    evidence_total_count,
                                    evidence_truncated: evidence_total_count > 100,
                                    episode_evidence: evidence
                                        .covered_episodes
                                        .iter()
                                        .map(|e| {
                                            e.season.map_or_else(
                                                || format!("E{:02}", e.episode),
                                                |s| format!("S{s:02}E{:02}", e.episode),
                                            )
                                        })
                                        .collect(),
                                    evidence: Some(wire_evidence),
                                },
                                None,
                            )
                        }
                        Err(_) => (
                            TorrentInspectionSummary {
                                status: "failed",
                                classification: TorrentContentKind::Unknown,
                                root_name: None,
                                file_count: None,
                                media_file_count: None,
                                total_size: None,
                                evidence_total_count: 0,
                                evidence_truncated: false,
                                episode_evidence: vec![],
                                evidence: None,
                            },
                            Some(InspectionFailureClass::MetainfoInvalid),
                        ),
                    },
                };
                if summary.status == "inspected" {
                    cache.insert(key, summary.clone()).await;
                }
                (index, summary, false, failure_class)
            }
        })
        .collect::<Vec<_>>();
    let (outcomes, timed_out) = collect_until_deadline(jobs, deadline, 3).await;
    let mut counts = InspectionCounts {
        selected,
        timed_out,
        unavailable: rows.len().saturating_sub(selected),
        ..Default::default()
    };
    for (index, summary, cache_hit, failure_class) in outcomes {
        counts.cache_hits += usize::from(cache_hit);
        counts.inspected += usize::from(summary.status == "inspected");
        counts.failed += usize::from(summary.status == "failed");
        if let Some(class) = failure_class {
            counts.record_failure(class);
        }
        rows[index].inspection = Some(summary);
    }
    if counts.timed_out > 0 {
        for index in selected_indices {
            let row = &mut rows[index];
            if row.inspection.is_some() {
                continue;
            }
            row.inspection = Some(TorrentInspectionSummary {
                status: "timeout",
                classification: TorrentContentKind::Unknown,
                root_name: None,
                file_count: None,
                media_file_count: None,
                total_size: None,
                evidence_total_count: 0,
                evidence_truncated: false,
                episode_evidence: vec![],
                evidence: None,
            });
        }
    }
    counts
}

pub(crate) fn normalized_info_hash(hash: &str) -> Option<String> {
    crate::torrent::normalize_info_hash(hash)
}

fn inspection_cache() -> &'static moka::future::Cache<String, TorrentInspectionSummary> {
    static CACHE: once_cell::sync::Lazy<moka::future::Cache<String, TorrentInspectionSummary>> =
        once_cell::sync::Lazy::new(|| {
            moka::future::Cache::builder()
                .max_capacity(500)
                .time_to_live(std::time::Duration::from_secs(1800))
                .build()
        });
    &CACHE
}

pub(crate) async fn inspect_prowlarr_results_until(
    state: &Arc<AppState>,
    rows: &mut [MultiSearchResult],
    intent: Option<&TorrentSearchIntent>,
    deadline: tokio::time::Instant,
) -> InspectionCounts {
    enrich_prowlarr_candidates(rows, intent, inspection_cache(), deadline, { let state=state.clone(); move |download_ref| { let state=state.clone(); async move { crate::sources::torrent::fetch_download_ref_bytes(&state, &download_ref).await } } }).await
}

pub(crate) async fn inspect_prowlarr_results(
    state: &Arc<AppState>,
    rows: &mut [MultiSearchResult],
    intent: Option<&TorrentSearchIntent>,
) -> InspectionCounts {
    inspect_prowlarr_results_until(
        state,
        rows,
        intent,
        tokio::time::Instant::now() + std::time::Duration::from_secs(15),
    )
    .await
}

#[derive(Serialize)]
pub struct SourceStatus {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct MultiSearchResponse {
    pub results: Vec<MultiSearchResult>,
    pub sources: std::collections::HashMap<String, SourceStatus>,
    pub version: String,
}

async fn enhanced_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Json<SearchResponse> {
    let client = (*state.http_client).clone();
    let parsed_query = smart_parse(&params.q);
    let limit = params.limit.clamp(1, 100);
    let page = params.page.max(1);
    // Fetch a fixed, generous cap so `total`/`total_pages` reflect the real match
    // count regardless of which page is requested.
    let mut results = search_folder_cache(&state.folder_cache_service, &[params.q.clone()], 500)
        .await
        .into_iter()
        .map(|item| {
            let parsed = smart_parse(&item.name);
            let similarity = calculate_unified_similarity(&params.q, &item.name, &[]);
            let matched_count =
                (get_title_keywords(&params.q).len() as f32 * similarity.score).round() as i32;
            let mut score = matched_count * 10
                + (similarity.score * 50.0) as i32
                + parsed.total_score()
                + ((item.size as f64 / 1024.0_f64.powi(3)).min(10.0) * 5.0) as i32;
            if similarity.score < 0.3 {
                score -= 100;
            }
            if parsed_query.season.is_some() && parsed_query.season != parsed.season {
                score -= 100;
            }
            let quality = format!(
                "{} {}",
                parsed.resolution.as_deref().unwrap_or(""),
                parsed.source.as_deref().unwrap_or("")
            )
            .trim()
            .to_string();
            SearchResult {
                fcode: item.fcode,
                original_filename: item.name,
                name: parsed.title,
                url: item.url,
                size: item.size,
                score,
                quality: (!quality.is_empty()).then_some(quality),
                resolution: parsed.resolution,
                source: parsed.source,
                episode_tag: match (parsed.season, parsed.episode) {
                    (Some(season), Some(episode)) => Some(format!("S{season:02}E{episode:02}")),
                    (_, Some(episode)) => Some(format!("E{episode:02}")),
                    _ => None,
                },
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
                year: parsed.year.map(|year| year.to_string()),
                release_date: parsed.year.map(|year| format!("{year}-01-01")),
                overview: None,
                media_type: Some(
                    if parsed.media_type == crate::utils::smart_tokenizer::MediaType::TvShow {
                        "tv".to_string()
                    } else {
                        "movie".to_string()
                    },
                ),
            }
        })
        .collect::<Vec<_>>();
    let mut seen = std::collections::HashSet::new();
    results.retain(|result| {
        is_media_file(&result.original_filename)
            && crate::api::search_pipeline::is_safe_search_result(&result.original_filename)
            && calculate_unified_similarity(&params.q, &result.original_filename, &[]).is_valid
            && parsed_query
                .season
                .is_none_or(|season| result.season == Some(season))
            && seen.insert(result.fcode.clone())
    });
    results.sort_by(|left, right| right.score.cmp(&left.score));

    if params.enrich {
        enrich_search_results(
            &client,
            &state.tmdb_service.api_key().await,
            &params.q,
            &mut results,
        )
        .await;
    }

    let total = results.len();
    let total_pages = total.div_ceil(limit);
    let offset = page.saturating_sub(1).saturating_mul(limit);
    let results = results.into_iter().skip(offset).take(limit).collect();
    Json(SearchResponse {
        results,
        page,
        limit,
        total,
        total_pages,
        has_more: page < total_pages,
        version: "v3.3-folder-cache".to_string(),
    })
}

async fn enrich_search_results(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
    results: &mut [SearchResult],
) {
    let mut collection_lookup = std::collections::HashMap::new();
    let collection_url = format!(
        "https://api.themoviedb.org/3/search/collection?api_key={api_key}&query={}",
        urlencoding::encode(query)
    );
    if let Ok(response) = client.get(collection_url).send().await {
        if let Some(collection_id) = response
            .json::<Value>()
            .await
            .ok()
            .and_then(|data| {
                data["results"]
                    .as_array()
                    .and_then(|items| items.first())
                    .cloned()
            })
            .and_then(|item| item["id"].as_u64())
        {
            let details_url = format!(
                "https://api.themoviedb.org/3/collection/{collection_id}?api_key={api_key}"
            );
            if let Ok(response) = client.get(details_url).send().await {
                if let Ok(data) = response.json::<Value>().await {
                    for part in data["parts"].as_array().into_iter().flatten() {
                        if let Some(title) = part["title"].as_str() {
                            collection_lookup.insert(normalize_title(title), part.clone());
                        }
                    }
                }
            }
        }
    }

    let mut tasks = Vec::new();
    let api_key = api_key.to_string();
    for (index, result) in results.iter_mut().take(100).enumerate() {
        let title = extract_core_title(&result.name);
        if let Some(data) = collection_lookup.get(&normalize_title(&title)) {
            apply_tmdb_data(result, data);
            continue;
        }
        let client = client.clone();
        let api_key = api_key.clone();
        let media_type = if result.is_series { "tv" } else { "movie" };
        let year = result.year.clone();
        tasks.push(async move {
            let mut url = format!(
                "https://api.themoviedb.org/3/search/{media_type}?api_key={api_key}&query={}",
                urlencoding::encode(&title)
            );
            if let Some(year) = year {
                url.push_str(&format!(
                    "&{}={year}",
                    if media_type == "movie" {
                        "primary_release_year"
                    } else {
                        "first_air_date_year"
                    }
                ));
            }
            let data = match client.get(url).send().await {
                Ok(response) => response.json::<Value>().await.ok().and_then(|data| {
                    data["results"]
                        .as_array()
                        .and_then(|items| items.first())
                        .cloned()
                }),
                Err(_) => None,
            };
            (index, data)
        });
    }
    let mut tasks = stream::iter(tasks).buffer_unordered(4);
    while let Some((index, data)) = tasks.next().await {
        if let Some(data) = data {
            apply_tmdb_data(&mut results[index], &data);
        }
    }
}

async fn await_multi_torbox_until<F>(deadline: tokio::time::Instant, future: F) -> Option<F::Output>
where
    F: std::future::Future,
{
    tokio::time::timeout_at(deadline, future).await.ok()
}

async fn multi_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MultiSearchQuery>,
) -> Json<MultiSearchResponse> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut sources = std::collections::HashMap::new();
    let mut results = Vec::new();
    // Multi is free text. Stremio requires canonical IMDb identity; deterministic skip.
    sources.insert(
        "stremio".to_string(),
        SourceStatus {
            ok: false,
            error: Some("canonical IMDb ID unavailable".to_string()),
        },
    );

    let enabled = state
        .db
        .get_setting("prowlarr_enabled")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    let fshare_future = enhanced_search(
        State(Arc::clone(&state)),
        Query(SearchQuery {
            q: params.q.clone(),
            page: 1,
            limit: params.limit.min(100).max(1),
            enrich: true,
        }),
    );
    let fshare_response = tokio::time::timeout_at(deadline, fshare_future)
        .await
        .map(|response| response.0)
        .unwrap_or(SearchResponse {
            results: Vec::new(),
            page: 1,
            limit: params.limit.min(100).max(1),
            total: 0,
            total_pages: 0,
            has_more: false,
            version: "v3.3-folder-cache".into(),
        });
    sources.insert(
        "fshare".to_string(),
        SourceStatus {
            ok: true,
            error: None,
        },
    );
    results.extend(
        fshare_response
            .results
            .into_iter()
            .map(fshare_to_multi_result),
    );

    if enabled {
        let core = extract_core_title(&params.q);
        let run = search_prowlarr_intent_until(
            &state,
            TorrentSearchIntent::FreeText,
            params.q.clone(),
            vec![params.q.clone(), core],
            params.limit.min(200).max(1),
            deadline,
        )
        .await;
        let mut found: Vec<_> = run
            .outcomes
            .into_iter()
            .filter_map(|outcome| match outcome {
                ProwlarrQueryOutcome::Success(response) => Some(response.results),
                ProwlarrQueryOutcome::Failed | ProwlarrQueryOutcome::TimedOut => None,
            })
            .flatten()
            .collect();
        found.retain(|item| {
            is_media_file(&item.name)
                && crate::api::search_pipeline::is_safe_search_result(&item.name)
                && calculate_unified_similarity(&params.q, &item.name, &[]).is_valid
        });
        if !found.is_empty() {
            found.sort_by(|a, b| b.seeders.unwrap_or(0).cmp(&a.seeders.unwrap_or(0)));
            inspect_prowlarr_results_until(&state, &mut found, None, deadline).await;
            results.extend(found);
            sources.insert(
                "torrent".to_string(),
                SourceStatus {
                    ok: true,
                    error: None,
                },
            );
        } else {
            sources.insert(
                "torrent".to_string(),
                SourceStatus {
                    ok: false,
                    error: Some("Prowlarr returned no usable results".to_string()),
                },
            );
        }
    } else {
        sources.insert(
            "torrent".to_string(),
            SourceStatus {
                ok: false,
                error: Some("Prowlarr disabled".to_string()),
            },
        );
    }

    normalize_multi_results(&mut results);
    let mut enrich_rows = results.clone();
    let torbox_status = match await_multi_torbox_until(
        deadline,
        crate::sources::search::enrich_canonical_search_results(&state, &mut enrich_rows),
    )
    .await
    {
        Some(Ok(cached)) => {
            results = enrich_rows;
            SourceStatus {
                ok: true,
                error: Some(format!(
                    "{cached} cached result{}",
                    if cached == 1 { "" } else { "s" }
                )),
            }
        }
        Some(Err(_)) => SourceStatus {
            ok: false,
            error: Some("TorBox cache check failed".into()),
        },
        None => SourceStatus {
            ok: false,
            error: Some("TorBox cache check timed out".into()),
        },
    };
    sources.insert("torbox".into(), torbox_status);
    Json(MultiSearchResponse {
        results,
        sources,
        version: "v1-torrent".to_string(),
    })
}

fn normalize_multi_results(results: &mut Vec<MultiSearchResult>) {
    let mut seen = std::collections::HashSet::new();
    results.retain_mut(|item| {
        if item.source.eq_ignore_ascii_case("fshare") {
            return seen.insert(format!("fshare:url:{}", item.url));
        }
        crate::sources::search::CanonicalSearchResult::from(&*item)
            .validate()
            .is_ok_and(|canonical| {
                item.url.clear();
                seen.insert(canonical.dedupe_key(&item.source, item.size))
            })
    });
}

fn fshare_to_multi_result(item: SearchResult) -> MultiSearchResult {
    MultiSearchResult {
        id: item.fcode.clone(),
        name: item.original_filename.clone(),
        parsed_name: item.name,
        url: item.url,
        download_ref: None,
        size: item.size,
        source: "fshare".to_string(),
        provider: "fshare".to_string(),
        indexer: None,
        seeders: None,
        leechers: None,
        info_hash: None,
        quality: item.quality,
        resolution: item.resolution,
        episode_tag: item.episode_tag,
        media_type: item.media_type,
        inspection: None,
        inspection_score: None,
        origin_provider: None,
        origin_indexer: None,
        cached: None,
        file_idx: None,
        sources: None,
        behavior_hints: None,
    }
}

pub(crate) struct ProwlarrSearchResponse {
    pub results: Vec<MultiSearchResult>,
    pub item_count: usize,
    pub discarded_count: usize,
    pub unexpected_shape: bool,
    pub diagnostic: &'static str,
}

pub(crate) enum ProwlarrQueryOutcome {
    Success(ProwlarrSearchResponse),
    Failed,
    TimedOut,
}

impl ProwlarrQueryOutcome {
    pub(crate) fn response(&self) -> Option<&ProwlarrSearchResponse> {
        match self {
            Self::Success(response) => Some(response),
            Self::Failed | Self::TimedOut => None,
        }
    }

    fn response_mut(&mut self) -> Option<&mut ProwlarrSearchResponse> {
        match self {
            Self::Success(response) => Some(response),
            Self::Failed | Self::TimedOut => None,
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct HashResolutionCounts {
    pub(crate) resolved: usize,
    pub(crate) cache_hits: usize,
    pub(crate) omitted: usize,
}

fn apply_prowlarr_hash(row: &mut MultiSearchResult, hash: String) {
    row.id = hash.clone();
    row.sources = Some(vec![format!("infoHash:{hash}")]);
    row.info_hash = Some(hash);
}

async fn finalize_prowlarr_outcomes_until<F, Fut>(
    outcomes: &mut [ProwlarrQueryOutcome],
    cache: &moka::future::Cache<String, String>,
    deadline: tokio::time::Instant,
    resolve: F,
) -> HashResolutionCounts
where
    F: Fn(String) -> Fut + Clone,
    Fut: std::future::Future<Output = Result<Option<String>, String>>,
{
    let mut refs = std::collections::BTreeSet::new();
    for response in outcomes
        .iter_mut()
        .filter_map(ProwlarrQueryOutcome::response_mut)
    {
        for row in &mut response.results {
            if let Some(hash) = row
                .info_hash
                .as_deref()
                .and_then(crate::torrent::normalize_info_hash)
            {
                apply_prowlarr_hash(row, hash);
            } else {
                row.info_hash = None;
                if let Some(reference) = &row.download_ref {
                    refs.insert(reference.clone());
                }
            }
        }
    }

    let jobs = refs
        .into_iter()
        .map(|reference| {
            let cache = cache.clone();
            let resolve = resolve.clone();
            async move {
                if let Some(hash) = cache.get(&reference).await {
                    return (reference, Some(hash), true);
                }
                let hash = resolve(reference.clone()).await.ok().flatten();
                if let Some(hash) = &hash {
                    cache.insert(reference.clone(), hash.clone()).await;
                }
                (reference, hash, false)
            }
        })
        .collect::<Vec<_>>();
    let (completed, _) = collect_until_deadline(jobs, deadline, 3).await;
    let mut counts = HashResolutionCounts::default();
    let resolved = completed
        .into_iter()
        .map(|(reference, hash, cache_hit)| {
            counts.cache_hits += usize::from(cache_hit);
            (reference, hash)
        })
        .collect::<std::collections::HashMap<_, _>>();

    for response in outcomes
        .iter_mut()
        .filter_map(ProwlarrQueryOutcome::response_mut)
    {
        let mut omitted = 0;
        response.results.retain_mut(|row| {
            if row.info_hash.is_some() {
                return true;
            }
            let hash = row
                .download_ref
                .as_ref()
                .and_then(|reference| resolved.get(reference))
                .and_then(Clone::clone);
            if let Some(hash) = hash {
                apply_prowlarr_hash(row, hash);
                counts.resolved += 1;
                true
            } else if row.download_ref.is_some() {
                true
            } else {
                counts.omitted += 1;
                omitted += 1;
                false
            }
        });
        response.discarded_count += omitted;
    }
    counts
}

async fn finalize_prowlarr_outcomes(
    state: &Arc<AppState>,
    outcomes: &mut [anyhow::Result<ProwlarrSearchResponse>],
    deadline: tokio::time::Instant,
) {
    let mut typed = outcomes
        .iter_mut()
        .map(
            |outcome| match std::mem::replace(outcome, Err(anyhow::anyhow!("moved"))) {
                Ok(response) => ProwlarrQueryOutcome::Success(response),
                Err(_) => ProwlarrQueryOutcome::Failed,
            },
        )
        .collect::<Vec<_>>();
    let counts =
        finalize_prowlarr_outcomes_until(&mut typed, &state.torrent_info_hashes, deadline, {
            let state = Arc::clone(state);
            move |download_ref| {
                let state = Arc::clone(&state);
                async move {
                    crate::sources::torrent::resolve_download_ref_info_hash(&state, &download_ref)
                        .await
                }
            }
        })
        .await;
    for (target, source) in outcomes.iter_mut().zip(typed) {
        *target = match source {
            ProwlarrQueryOutcome::Success(response) => Ok(response),
            ProwlarrQueryOutcome::Failed | ProwlarrQueryOutcome::TimedOut => {
                Err(anyhow::anyhow!("Prowlarr search failed"))
            }
        };
    }
    tracing::info!(
        resolved = counts.resolved,
        cache_hits = counts.cache_hits,
        omitted = counts.omitted,
        "Prowlarr info-hash resolution complete"
    );
}

/// Structured intent for a Prowlarr torrent search, local to torrent search.
/// Drives capability-aware `type=` selection and identity params while the
/// canonical DTO (`MultiSearchResult`) and opaque `downloadRef` handling stay
/// unchanged. Text aliases are only used as a fallback when identity lanes miss.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TorrentSearchIntent {
    Movie {
        tmdb_id: Option<String>,
        imdb_id: Option<String>,
    },
    Season {
        tvdb_id: Option<String>,
        imdb_id: Option<String>,
        season: u32,
    },
    Episode {
        tvdb_id: Option<String>,
        imdb_id: Option<String>,
        season: u32,
        episode: u32,
    },
    FreeText,
}

pub(crate) fn inspection_score(
    intent: &TorrentSearchIntent,
    summary: Option<&TorrentInspectionSummary>,
) -> i32 {
    let Some(summary) = summary.filter(|summary| summary.status == "inspected") else {
        return 0;
    };
    match intent {
        TorrentSearchIntent::Episode {
            season, episode, ..
        } => {
            let target = format!("S{season:02}E{episode:02}");
            if summary
                .episode_evidence
                .iter()
                .any(|evidence| evidence.eq_ignore_ascii_case(&target))
            {
                1_000
            } else {
                -250
            }
        }
        TorrentSearchIntent::Season { season, .. } => {
            let episodes = summary
                .episode_evidence
                .iter()
                .flat_map(|evidence| episode_pairs(evidence))
                .collect::<std::collections::BTreeSet<_>>();
            if episodes.len() > 1
                && episodes
                    .iter()
                    .all(|(candidate_season, _)| candidate_season == season)
            {
                1_000
            } else {
                -250
            }
        }
        TorrentSearchIntent::Movie { .. } => {
            if summary.classification == TorrentContentKind::Movie {
                1_000
            } else {
                -250
            }
        }
        TorrentSearchIntent::FreeText => 0,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProwlarrLane {
    search_type: &'static str,
    tmdb_id: Option<String>,
    imdb_id: Option<String>,
    tvdb_id: Option<String>,
    season: Option<u32>,
    episode: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProwlarrIndexer {
    id: u64,
    name: String,
    enable: bool,
    protocol: String,
    capabilities: ProwlarrCapabilities,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProwlarrCapabilities {
    #[serde(default)]
    search_params: Vec<String>,
    #[serde(default)]
    tv_search_params: Vec<String>,
    #[serde(default)]
    movie_search_params: Vec<String>,
}

impl ProwlarrIndexer {
    fn params_for(&self, search_type: &str) -> &[String] {
        match search_type {
            "movie" => &self.capabilities.movie_search_params,
            "tvsearch" => &self.capabilities.tv_search_params,
            _ => &self.capabilities.search_params,
        }
    }

    fn supports(&self, search_type: &str, param: &str) -> bool {
        self.params_for(search_type)
            .iter()
            .any(|value| value.eq_ignore_ascii_case(param))
    }
}

/// Plan the required Prowlarr lanes for an intent. Episode runs two required
/// lanes (episode-specific + season-level); Season runs the season-level lane;
/// Movie is identity-first; FreeText is a single opaque text lane.
fn prowlarr_lanes(intent: &TorrentSearchIntent) -> Vec<ProwlarrLane> {
    let lane = |search_type, tmdb_id, imdb_id, tvdb_id, season, episode| ProwlarrLane {
        search_type,
        tmdb_id,
        imdb_id,
        tvdb_id,
        season,
        episode,
    };
    match intent {
        TorrentSearchIntent::Movie { tmdb_id, imdb_id } => {
            vec![lane(
                "movie",
                tmdb_id.clone(),
                imdb_id.clone(),
                None,
                None,
                None,
            )]
        }
        TorrentSearchIntent::Season {
            tvdb_id,
            imdb_id,
            season,
        } => {
            vec![lane(
                "tvsearch",
                None,
                imdb_id.clone(),
                tvdb_id.clone(),
                Some(*season),
                None,
            )]
        }
        TorrentSearchIntent::Episode {
            tvdb_id,
            imdb_id,
            season,
            episode,
        } => vec![
            lane(
                "tvsearch",
                None,
                imdb_id.clone(),
                tvdb_id.clone(),
                Some(*season),
                Some(*episode),
            ),
            lane(
                "tvsearch",
                None,
                imdb_id.clone(),
                tvdb_id.clone(),
                Some(*season),
                None,
            ),
        ],
        TorrentSearchIntent::FreeText => vec![lane("search", None, None, None, None, None)],
    }
}

fn newznab_url(
    base: &str,
    indexer_id: u64,
    query: &str,
    limit: usize,
    categories: Option<&str>,
    lane: &ProwlarrLane,
) -> String {
    let mut url = reqwest::Url::parse(&format!(
        "{}/api/v1/indexer/{indexer_id}/newznab",
        base.trim().trim_end_matches('/')
    ))
    .expect("validated Prowlarr URL");
    {
        let mut pairs = url.query_pairs_mut();
        pairs
            .append_pair("t", lane.search_type)
            .append_pair("q", query)
            .append_pair("limit", &limit.to_string());
        if let Some(value) = lane.tmdb_id.as_deref() {
            pairs.append_pair("tmdbid", value);
        }
        if let Some(value) = lane.imdb_id.as_deref() {
            pairs.append_pair("imdbid", value);
        }
        if let Some(value) = lane.tvdb_id.as_deref() {
            pairs.append_pair("tvdbid", value);
        }
        if let Some(value) = lane.season {
            pairs.append_pair("season", &value.to_string());
        }
        if let Some(value) = lane.episode {
            pairs.append_pair("ep", &value.to_string());
        }
        if let Some(value) = categories.filter(|value| !value.trim().is_empty()) {
            pairs.append_pair("cat", value.trim());
        }
    }
    url.into()
}

/// Dedupe results across concurrent lanes without dropping the opaque
/// `downloadRef`. Keyed by info_hash when present, else name+size.
fn dedupe_prowlarr_responses<'a>(
    responses: impl IntoIterator<Item = &'a mut ProwlarrSearchResponse>,
) {
    let mut seen = std::collections::HashSet::new();
    for response in responses {
        response.results.retain(|item| {
            seen.insert(
                item.info_hash
                    .clone()
                    .unwrap_or_else(|| format!("{}:{}", item.name.to_lowercase(), item.size)),
            )
        });
    }
}

fn dedupe_prowlarr_outcomes(outcomes: &mut [anyhow::Result<ProwlarrSearchResponse>]) {
    dedupe_prowlarr_responses(
        outcomes
            .iter_mut()
            .filter_map(|outcome| outcome.as_mut().ok()),
    );
}

fn dedupe_typed_prowlarr_outcomes(outcomes: &mut [ProwlarrQueryOutcome]) {
    dedupe_prowlarr_responses(
        outcomes
            .iter_mut()
            .filter_map(ProwlarrQueryOutcome::response_mut),
    );
}

pub(crate) struct ProwlarrSearchRun {
    pub(crate) outcomes: Vec<ProwlarrQueryOutcome>,
    pub(crate) planned: usize,
    pub(crate) attempted: usize,
    pub(crate) truncated: usize,
    pub(crate) hashes: HashResolutionCounts,
    pub(crate) discovery_elapsed: std::time::Duration,
    pub(crate) hash_elapsed: std::time::Duration,
}

struct ProwlarrJobRun {
    outcomes: Vec<ProwlarrQueryOutcome>,
    planned: usize,
    started: usize,
    unstarted: usize,
}

async fn collect_prowlarr_jobs_until_deadline<Fut>(
    jobs: Vec<Fut>,
    deadline: tokio::time::Instant,
    concurrency: usize,
) -> ProwlarrJobRun
where
    Fut: std::future::Future<Output = ProwlarrQueryOutcome>,
{
    use std::sync::atomic::{AtomicUsize, Ordering};

    let planned = jobs.len();
    let started = Arc::new(AtomicUsize::new(0));
    let work = stream::iter(jobs.into_iter().enumerate())
        .map({
            let started = Arc::clone(&started);
            move |(ordinal, job)| {
                let started = Arc::clone(&started);
                async move {
                    started.fetch_add(1, Ordering::SeqCst);
                    (ordinal, job.await)
                }
            }
        })
        .buffer_unordered(concurrency.max(1));
    tokio::pin!(work);
    let mut completed = std::collections::BTreeMap::new();
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, work.next()).await {
            Ok(Some((ordinal, outcome))) => {
                completed.insert(ordinal, outcome);
            }
            Ok(None) | Err(_) => break,
        }
    }
    let started = started.load(Ordering::SeqCst).min(planned);
    let outcomes = (0..started)
        .map(|ordinal| {
            completed
                .remove(&ordinal)
                .unwrap_or(ProwlarrQueryOutcome::TimedOut)
        })
        .collect();
    ProwlarrJobRun {
        outcomes,
        planned,
        started,
        unstarted: planned.saturating_sub(started),
    }
}

pub(crate) async fn search_prowlarr(
    state: &Arc<AppState>,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<MultiSearchResult>> {
    let mut outcomes = vec![search_prowlarr_with_diagnostics(state, query, limit).await];
    finalize_prowlarr_outcomes(
        state,
        &mut outcomes,
        tokio::time::Instant::now() + std::time::Duration::from_secs(20),
    )
    .await;
    Ok(outcomes.remove(0)?.results)
}

pub(crate) async fn search_prowlarr_queries(
    state: &Arc<AppState>,
    queries: impl IntoIterator<Item = String>,
    limit: usize,
    stop_on_success: bool,
) -> Vec<anyhow::Result<ProwlarrSearchResponse>> {
    search_prowlarr_queries_until(
        state,
        queries,
        limit,
        stop_on_success,
        tokio::time::Instant::now() + std::time::Duration::from_secs(18),
    )
    .await
}

pub(crate) async fn search_prowlarr_queries_until(
    state: &Arc<AppState>,
    queries: impl IntoIterator<Item = String>,
    limit: usize,
    stop_on_success: bool,
    deadline: tokio::time::Instant,
) -> Vec<anyhow::Result<ProwlarrSearchResponse>> {
    let mut seen = std::collections::HashSet::new();
    let mut outcomes = Vec::new();
    for query in queries
        .into_iter()
        .filter(|q| !q.trim().is_empty() && seen.insert(q.trim().to_lowercase()))
        .take(8)
    {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        let outcome = match tokio::time::timeout(
            remaining.min(std::time::Duration::from_secs(10)),
            search_prowlarr_with_diagnostics(state, &query, limit),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!("Prowlarr search attempt timed out")),
        };
        let found = outcome
            .as_ref()
            .is_ok_and(|response| !response.results.is_empty());
        outcomes.push(outcome);
        if stop_on_success && found {
            break;
        }
    }
    finalize_prowlarr_outcomes(state, &mut outcomes, deadline).await;
    outcomes
}

async fn search_prowlarr_query_outcomes_until(
    state: &Arc<AppState>,
    queries: impl IntoIterator<Item = String>,
    limit: usize,
    stop_on_success: bool,
    deadline: tokio::time::Instant,
) -> ProwlarrJobRun {
    let mut seen = std::collections::HashSet::new();
    let mut outcomes = Vec::new();
    for query in queries
        .into_iter()
        .filter(|q| !q.trim().is_empty() && seen.insert(q.trim().to_lowercase()))
        .take(8)
    {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        let outcome = match tokio::time::timeout(
            remaining.min(std::time::Duration::from_secs(10)),
            search_prowlarr_with_diagnostics(state, &query, limit),
        )
        .await
        {
            Ok(Ok(response)) => ProwlarrQueryOutcome::Success(response),
            Ok(Err(_)) => ProwlarrQueryOutcome::Failed,
            Err(_) => ProwlarrQueryOutcome::TimedOut,
        };
        let found = matches!(outcome, ProwlarrQueryOutcome::Success(ref response) if !response.results.is_empty());
        outcomes.push(outcome);
        if stop_on_success && found {
            break;
        }
    }
    let started = outcomes.len();
    ProwlarrJobRun {
        outcomes,
        planned: started,
        started,
        unstarted: 0,
    }
}

/// Capability-aware Prowlarr search driven by a structured intent. Runs the
/// intent's required lanes concurrently (Episode: episode + season; Season:
/// season; Movie: identity-first). All required lanes always execute — no
/// early stop after the first hit. Text aliases run only as a fallback when
/// every identity lane comes back empty. Results are deduped across lanes and
/// opaque `downloadRef` handling is preserved by the underlying request path.
pub(crate) async fn search_prowlarr_intent(
    state: &Arc<AppState>,
    intent: TorrentSearchIntent,
    query: String,
    aliases: Vec<String>,
    limit: usize,
) -> Vec<anyhow::Result<ProwlarrSearchResponse>> {
    search_prowlarr_intent_until(
        state,
        intent,
        query,
        aliases,
        limit,
        tokio::time::Instant::now() + std::time::Duration::from_secs(20),
    )
    .await
    .outcomes
    .into_iter()
    .map(|outcome| match outcome {
        ProwlarrQueryOutcome::Success(response) => Ok(response),
        ProwlarrQueryOutcome::Failed => Err(anyhow::anyhow!("Prowlarr search failed")),
        ProwlarrQueryOutcome::TimedOut => Err(anyhow::anyhow!("Prowlarr search timed out")),
    })
    .collect()
}

pub(crate) async fn search_prowlarr_intent_until(
    state: &Arc<AppState>,
    intent: TorrentSearchIntent,
    query: String,
    aliases: Vec<String>,
    limit: usize,
    deadline: tokio::time::Instant,
) -> ProwlarrSearchRun {
    let discovery_started = std::time::Instant::now();
    if intent == TorrentSearchIntent::FreeText {
        let mut jobs =
            search_prowlarr_query_outcomes_until(state, aliases, limit, true, deadline).await;
        let discovery_elapsed = discovery_started.elapsed();
        let hash_started = std::time::Instant::now();
        let hashes = finalize_prowlarr_outcomes_until(
            &mut jobs.outcomes,
            &state.torrent_info_hashes,
            deadline,
            {
                let state = Arc::clone(state);
                move |reference| {
                    let state = Arc::clone(&state);
                    async move {
                        crate::sources::torrent::resolve_download_ref_info_hash(&state, &reference)
                            .await
                    }
                }
            },
        )
        .await;
        return ProwlarrSearchRun {
            outcomes: jobs.outcomes,
            planned: jobs.planned,
            attempted: jobs.started,
            truncated: jobs.unstarted,
            hashes,
            discovery_elapsed,
            hash_elapsed: hash_started.elapsed(),
        };
    }
    let indexers = match tokio::time::timeout(
        deadline.saturating_duration_since(tokio::time::Instant::now()),
        prowlarr_indexers(state),
    )
    .await
    {
        Ok(Ok(indexers)) => indexers,
        Ok(Err(_)) => {
            return ProwlarrSearchRun {
                outcomes: vec![ProwlarrQueryOutcome::Failed],
                planned: 1,
                attempted: 1,
                truncated: 0,
                hashes: Default::default(),
                discovery_elapsed: discovery_started.elapsed(),
                hash_elapsed: Default::default(),
            }
        }
        Err(_) => {
            return ProwlarrSearchRun {
                outcomes: vec![ProwlarrQueryOutcome::TimedOut],
                planned: 1,
                attempted: 1,
                truncated: 0,
                hashes: Default::default(),
                discovery_elapsed: discovery_started.elapsed(),
                hash_elapsed: Default::default(),
            }
        }
    };
    let lanes = prowlarr_lanes(&intent);
    let mut searches: Vec<_> = lanes
        .into_iter()
        .flat_map(|lane| {
            indexers
                .iter()
                .filter(|indexer| indexer_supports_lane(indexer, &lane))
                .cloned()
                .map(|indexer| (lane_for_indexer(&lane, &indexer), indexer))
                .collect::<Vec<_>>()
        })
        .collect();
    let unsupported = searches.is_empty();
    let lane_count = searches.len();
    searches.truncate(15);
    let truncated = lane_count.saturating_sub(searches.len());
    let mut jobs = searches
        .into_iter()
        .map(|(lane, indexer)| {
            let query = query.clone();
            Box::pin(async move { search_prowlarr_lane(state, query, limit, lane, indexer).await })
                as futures_util::future::BoxFuture<'_, ProwlarrQueryOutcome>
        })
        .collect::<Vec<_>>();
    jobs.push(Box::pin(async {
        match search_prowlarr_aggregate(state, &query, limit).await {
            Ok(response) => ProwlarrQueryOutcome::Success(response),
            Err(_) => ProwlarrQueryOutcome::Failed,
        }
    }));
    let mut run = collect_prowlarr_jobs_until_deadline(jobs, deadline, 4).await;
    let no_results = run.outcomes.iter().all(|outcome| {
        outcome
            .response()
            .is_none_or(|response| response.results.is_empty())
    });
    if unsupported || no_results {
        for alias in aliases.into_iter().take(8) {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() || run.planned >= 16 {
                break;
            }
            run.planned += 1;
            run.started += 1;
            let outcome = tokio::time::timeout(
                remaining.min(std::time::Duration::from_secs(10)),
                search_prowlarr_with_diagnostics(state, &alias, limit),
            )
            .await;
            match outcome {
                Ok(Ok(response)) => {
                    let found = !response.results.is_empty();
                    run.outcomes.push(ProwlarrQueryOutcome::Success(response));
                    if found {
                        break;
                    }
                }
                Ok(Err(_)) => run.outcomes.push(ProwlarrQueryOutcome::Failed),
                Err(_) => run.outcomes.push(ProwlarrQueryOutcome::TimedOut),
            }
        }
    }
    dedupe_typed_prowlarr_outcomes(&mut run.outcomes);
    let discovery_elapsed = discovery_started.elapsed();
    let hash_started = std::time::Instant::now();
    let hashes = finalize_prowlarr_outcomes_until(
        &mut run.outcomes,
        &state.torrent_info_hashes,
        deadline,
        {
            let state = Arc::clone(state);
            move |reference| {
                let state = Arc::clone(&state);
                async move {
                    crate::sources::torrent::resolve_download_ref_info_hash(&state, &reference)
                        .await
                }
            }
        },
    )
    .await;
    ProwlarrSearchRun {
        outcomes: run.outcomes,
        planned: run.planned,
        attempted: run.started,
        truncated: truncated + run.unstarted,
        hashes,
        discovery_elapsed,
        hash_elapsed: hash_started.elapsed(),
    }
}

fn indexer_supports_lane(indexer: &ProwlarrIndexer, lane: &ProwlarrLane) -> bool {
    !indexer.params_for(lane.search_type).is_empty()
        && lane
            .season
            .is_none_or(|_| indexer.supports(lane.search_type, "season"))
        && lane
            .episode
            .is_none_or(|_| indexer.supports(lane.search_type, "ep"))
}

fn lane_for_indexer(lane: &ProwlarrLane, indexer: &ProwlarrIndexer) -> ProwlarrLane {
    let mut projected = lane.clone();
    if !indexer.supports(lane.search_type, "tmdbid") {
        projected.tmdb_id = None;
    }
    if !indexer.supports(lane.search_type, "imdbid") {
        projected.imdb_id = None;
    }
    if !indexer.supports(lane.search_type, "tvdbid") {
        projected.tvdb_id = None;
    }
    projected
}

async fn typed_lane_outcome<Fut>(timeout: std::time::Duration, job: Fut) -> ProwlarrQueryOutcome
where
    Fut: std::future::Future<Output = anyhow::Result<ProwlarrSearchResponse>>,
{
    match tokio::time::timeout(timeout, job).await {
        Ok(Ok(response)) => ProwlarrQueryOutcome::Success(response),
        Ok(Err(_)) => ProwlarrQueryOutcome::Failed,
        Err(_) => ProwlarrQueryOutcome::TimedOut,
    }
}

async fn search_prowlarr_lane(
    state: &Arc<AppState>,
    query: String,
    limit: usize,
    lane: ProwlarrLane,
    _indexer: ProwlarrIndexer,
) -> ProwlarrQueryOutcome {
    typed_lane_outcome(
        std::time::Duration::from_secs(10),
        search_prowlarr_native(state, &query, limit, &lane),
    )
    .await
}

pub(crate) async fn search_prowlarr_with_diagnostics(
    state: &Arc<AppState>,
    query: &str,
    limit: usize,
) -> anyhow::Result<ProwlarrSearchResponse> {
    search_prowlarr_aggregate(state, query, limit).await
}

fn prowlarr_config(
    state: &Arc<AppState>,
    limit: usize,
) -> anyhow::Result<(String, String, usize, Option<String>)> {
    let base = state
        .db
        .get_setting("prowlarr_url")
        .ok()
        .flatten()
        .unwrap_or_else(|| "http://localhost:9696".to_string());
    let api_key = state
        .db
        .get_setting("prowlarr_api_key")
        .ok()
        .flatten()
        .unwrap_or_default();
    crate::api::settings::validate_api_key(&api_key)
        .map_err(anyhow::Error::msg)
        .context("Prowlarr API key")?;
    let configured_limit = state
        .db
        .get_setting("prowlarr_limit")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(limit)
        .max(1);
    Ok((
        base,
        api_key,
        limit.min(configured_limit).max(1),
        state.db.get_setting("prowlarr_categories").ok().flatten(),
    ))
}

async fn prowlarr_indexers(state: &Arc<AppState>) -> anyhow::Result<Vec<ProwlarrIndexer>> {
    let (base, api_key, _, _) = prowlarr_config(state, 1)?;
    let response = state
        .http_client
        .get(format!(
            "{}/api/v1/indexer",
            base.trim().trim_end_matches('/')
        ))
        .header("X-Api-Key", api_key)
        .send()
        .await?
        .error_for_status()?;
    let indexers: Vec<ProwlarrIndexer> = parse_bounded_prowlarr_json(response).await?;
    Ok(indexers
        .into_iter()
        .filter(|indexer| indexer.enable && indexer.protocol.eq_ignore_ascii_case("torrent"))
        .collect())
}

async fn search_prowlarr_aggregate(
    state: &Arc<AppState>,
    query: &str,
    limit: usize,
) -> anyhow::Result<ProwlarrSearchResponse> {
    let (base, api_key, effective_limit, categories) = prowlarr_config(state, limit)?;
    let mut url = reqwest::Url::parse(&format!(
        "{}/api/v1/search",
        base.trim().trim_end_matches('/')
    ))?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs
            .append_pair("query", query)
            .append_pair("type", "search")
            .append_pair("limit", &effective_limit.to_string());
        if let Some(value) = categories
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            pairs.append_pair("categories", value.trim());
        }
    }
    let body = prowlarr_get(state, url.into(), api_key).await?;
    parse_and_secure_prowlarr(state, &body, query, effective_limit, None).await
}

async fn search_prowlarr_native(
    state: &Arc<AppState>,
    query: &str,
    limit: usize,
    lane: &ProwlarrLane,
) -> anyhow::Result<ProwlarrSearchResponse> {
    let (base, api_key, effective_limit, categories) = prowlarr_config(state, limit)?;
    let mut url = reqwest::Url::parse(&format!(
        "{}/api/v1/search",
        base.trim().trim_end_matches('/')
    ))?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs
            .append_pair("query", query)
            .append_pair("type", lane.search_type)
            .append_pair("limit", &effective_limit.to_string());
        if let Some(value) = lane.tmdb_id.as_deref() { pairs.append_pair("tmdbId", value); }
        if let Some(value) = lane.imdb_id.as_deref() { pairs.append_pair("imdbId", value); }
        if let Some(value) = lane.tvdb_id.as_deref() { pairs.append_pair("tvdbId", value); }
        if let Some(value) = lane.season { pairs.append_pair("season", &value.to_string()); }
        if let Some(value) = lane.episode { pairs.append_pair("episode", &value.to_string()); }
        if let Some(value) = categories.as_deref().filter(|value| !value.trim().is_empty()) {
            pairs.append_pair("categories", value.trim());
        }
    }
    let body = prowlarr_get(state, url.into(), api_key).await?;
    parse_and_secure_prowlarr(state, &body, query, effective_limit, None).await
}

async fn search_prowlarr_newznab(
    state: &Arc<AppState>,
    query: &str,
    limit: usize,
    lane: &ProwlarrLane,
    indexer: &ProwlarrIndexer,
) -> anyhow::Result<ProwlarrSearchResponse> {
    let (base, api_key, effective_limit, categories) = prowlarr_config(state, limit)?;
    let url = newznab_url(
        &base,
        indexer.id,
        query,
        effective_limit,
        categories.as_deref(),
        lane,
    );
    let body = prowlarr_get(state, url, api_key).await?;
    parse_and_secure_prowlarr(state, &body, query, effective_limit, Some(&indexer.name)).await
}

const PROWLARR_RESPONSE_LIMIT_BYTES: usize = 4 * 1024 * 1024;

async fn read_bounded_response(
    response: reqwest::Response,
    limit: usize,
) -> anyhow::Result<String> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        anyhow::bail!("Prowlarr response body exceeded limit");
    }
    let mut body = Vec::new();
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk?;
        if body.len().saturating_add(chunk.len()) > limit {
            anyhow::bail!("Prowlarr response body exceeded limit");
        }
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body).context("Prowlarr response body was not UTF-8")
}

async fn parse_bounded_prowlarr_json<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> anyhow::Result<T> {
    let body = read_bounded_response(response, PROWLARR_RESPONSE_LIMIT_BYTES).await?;
    serde_json::from_str(&body).context("Prowlarr response body was not valid JSON")
}

async fn prowlarr_get(
    state: &Arc<AppState>,
    url: String,
    api_key: String,
) -> anyhow::Result<String> {
    let response = state
        .http_client
        .get(url)
        .header("X-Api-Key", api_key)
        .send()
        .await?
        .error_for_status()?;
    read_bounded_response(response, PROWLARR_RESPONSE_LIMIT_BYTES).await
}

async fn parse_and_secure_prowlarr(
    state: &Arc<AppState>,
    body: &str,
    query: &str,
    limit: usize,
    indexer: Option<&str>,
) -> anyhow::Result<ProwlarrSearchResponse> {
    let (mut results, item_count, discarded_count, unexpected_shape, diagnostic) =
        if let Ok(json) = serde_json::from_str::<Value>(body) {
            let parsed = parse_prowlarr_json_items_with_diagnostics(&json, query, limit);
            (
                parsed.results,
                parsed.item_count,
                parsed.discarded_count,
                parsed.unexpected_shape,
                parsed.diagnostic,
            )
        } else {
            let results = parse_torznab_items(body, query, limit);
            let item_count = results.len();
            (results, item_count, 0, false, "torznab")
        };
    for result in &mut results {
        if result.indexer.is_none() {
            result.indexer = indexer.map(str::to_string);
        }
        if !result.url.trim().is_empty() {
            let raw_url = result.url.clone();
            let download_ref = uuid::Uuid::new_v4().to_string();
            state
                .torrent_download_refs
                .insert(download_ref.clone(), raw_url)
                .await;
            result.url.clear();
            result.download_ref = Some(download_ref);
        }
    }
    Ok(ProwlarrSearchResponse {
        results,
        item_count,
        discarded_count,
        unexpected_shape,
        diagnostic,
    })
}

struct ProwlarrParseResult {
    results: Vec<MultiSearchResult>,
    item_count: usize,
    discarded_count: usize,
    unexpected_shape: bool,
    diagnostic: &'static str,
}

fn parse_prowlarr_json_items_with_diagnostics(
    data: &Value,
    query: &str,
    limit: usize,
) -> ProwlarrParseResult {
    let Some(items) = data.as_array() else {
        return ProwlarrParseResult {
            results: Vec::new(),
            item_count: 0,
            discarded_count: 0,
            unexpected_shape: true,
            diagnostic: "unexpected JSON shape",
        };
    };
    let item_count = items.len().min(limit);
    let mut discarded_count = 0;
    let results = items
        .iter()
        .take(limit)
        .filter_map(|item| {
            let title = string_val(item, "title").unwrap_or_else(|| "Unknown".to_string());
            let magnet_url = string_val_any(item, &["magnetUrl", "magneturl", "magnet"]);
            let download_url = string_val_any(item, &["downloadUrl", "downloadurl", "link"]);
            let guid = string_val(item, "guid");
            let url = magnet_url.clone().or(download_url.clone()).or_else(|| {
                guid.clone()
                    .filter(|v| looks_like_direct_torrent_download(v))
            });
            let Some(url) = url else {
                discarded_count += 1;
                return None;
            };
            let size = u64_val(item, "size").unwrap_or(0);
            let seeders = i64_val(item, "seeders");
            let leechers = i64_val(item, "leechers").or_else(|| i64_val(item, "peers"));
            let info_hash = string_val_any(
                item,
                &[
                    "infoHash",
                    "infohash",
                    "info_hash",
                    "torrentInfoHash",
                    "hash",
                ],
            )
            .as_deref()
            .and_then(crate::torrent::normalize_info_hash)
            .or_else(|| magnet_url.as_deref().and_then(crate::torrent::extract_btih))
            .or_else(|| {
                download_url
                    .as_deref()
                    .and_then(crate::torrent::extract_btih)
            })
            .or_else(|| guid.as_deref().and_then(crate::torrent::extract_btih))
            .or_else(|| crate::torrent::extract_btih(&url));
            let parsed = smart_parse(&title);
            let quality = parsed.quality_name();
            let episode_tag = if let (Some(s), Some(e)) = (parsed.season, parsed.episode) {
                Some(format!("S{:02}E{:02}", s, e))
            } else {
                parsed.episode.map(|e| format!("E{:02}", e))
            };
            let id = info_hash
                .clone()
                .or_else(|| string_val(item, "guid").filter(|v| !should_hide_provider_url(v)))
                .unwrap_or_else(|| format!("{}:{}", title, size));

            Some(MultiSearchResult {
                id,
                name: title.clone(),
                parsed_name: parsed.title.clone(),
                url,
                download_ref: None,
                size,
                source: "torrent".to_string(),
                provider: "prowlarr".to_string(),
                indexer: string_val(item, "indexer"),
                origin_provider: None,
                origin_indexer: None,
                cached: None,
                file_idx: None,
                sources: info_hash
                    .as_ref()
                    .map(|hash| vec![format!("infoHash:{hash}")]),
                behavior_hints: Some(crate::utils::title_matcher::StreamBehaviorHints {
                    binge_group: None,
                    filename: Some(title.clone()),
                    video_size: Some(size),
                }),
                seeders,
                leechers,
                info_hash,
                quality: Some(quality),
                resolution: parsed.resolution,
                episode_tag,
                media_type: Some(
                    if parsed.media_type == crate::utils::smart_tokenizer::MediaType::TvShow
                        || query.to_ascii_lowercase().contains('s')
                            && query.to_ascii_lowercase().contains('e')
                    {
                        "tv"
                    } else {
                        "movie"
                    }
                    .to_string(),
                ),
                inspection: None,
                inspection_score: None,
            })
        })
        .collect();
    ProwlarrParseResult {
        discarded_count,
        results,
        item_count,
        unexpected_shape: false,
        diagnostic: "JSON array",
    }
}

fn parse_prowlarr_json_items(data: &Value, query: &str, limit: usize) -> Vec<MultiSearchResult> {
    parse_prowlarr_json_items_with_diagnostics(data, query, limit).results
}

fn string_val(item: &Value, key: &str) -> Option<String> {
    item.get(key)?.as_str().map(|s| s.to_string())
}

fn string_val_any(item: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| string_val(item, key))
}

fn u64_val(item: &Value, key: &str) -> Option<u64> {
    item.get(key)?
        .as_u64()
        .or_else(|| item.get(key)?.as_str()?.parse::<u64>().ok())
}

fn i64_val(item: &Value, key: &str) -> Option<i64> {
    item.get(key)?
        .as_i64()
        .or_else(|| item.get(key)?.as_str()?.parse::<i64>().ok())
}

fn parse_torznab_items(xml: &str, query: &str, limit: usize) -> Vec<MultiSearchResult> {
    let item_re = regex::Regex::new(r"(?s)<item\b.*?</item>").unwrap();
    item_re
        .find_iter(xml)
        .take(limit)
        .filter_map(|m| parse_torznab_item(m.as_str(), query))
        .collect()
}

fn parse_torznab_item(item: &str, query: &str) -> Option<MultiSearchResult> {
    let title = xml_text(item, "title").unwrap_or_else(|| "Unknown".to_string());
    let link = xml_text(item, "link");
    let enclosure = attr_first(item, "enclosure", "url");
    let url = enclosure.or(link)?;
    let size = torznab_attr(item, "size")
        .and_then(|v| v.parse::<u64>().ok())
        .or_else(|| attr_first(item, "enclosure", "length").and_then(|v| v.parse::<u64>().ok()))
        .unwrap_or(0);
    let seeders = torznab_attr(item, "seeders").and_then(|v| v.parse::<i64>().ok());
    let leechers = torznab_attr(item, "peers").and_then(|v| v.parse::<i64>().ok());
    let info_hash = torznab_attr(item, "infohash")
        .as_deref()
        .and_then(crate::torrent::normalize_info_hash)
        .or_else(|| crate::torrent::extract_btih(&url));
    let parsed = smart_parse(&title);
    let quality = parsed.quality_name();
    let episode_tag = if let (Some(s), Some(e)) = (parsed.season, parsed.episode) {
        Some(format!("S{:02}E{:02}", s, e))
    } else {
        parsed.episode.map(|e| format!("E{:02}", e))
    };
    let id = info_hash
        .clone()
        .unwrap_or_else(|| format!("{}:{}", title, size));

    Some(MultiSearchResult {
        id,
        name: title.clone(),
        parsed_name: parsed.title.clone(),
        url,
        download_ref: None,
        size,
        source: "torrent".to_string(),
        provider: "prowlarr".to_string(),
        indexer: xml_text(item, "prowlarrindexer").or_else(|| xml_text(item, "jackettindexer")),
        origin_provider: None,
        origin_indexer: None,
        cached: None,
        file_idx: None,
        sources: info_hash
            .as_ref()
            .map(|hash| vec![format!("infoHash:{hash}")]),
        behavior_hints: Some(crate::utils::title_matcher::StreamBehaviorHints {
            binge_group: None,
            filename: Some(title.clone()),
            video_size: Some(size),
        }),
        seeders,
        leechers,
        info_hash,
        quality: Some(quality),
        resolution: parsed.resolution,
        episode_tag,
        media_type: Some(
            if parsed.media_type == crate::utils::smart_tokenizer::MediaType::TvShow
                || query.to_ascii_lowercase().contains('s')
                    && query.to_ascii_lowercase().contains('e')
            {
                "tv"
            } else {
                "movie"
            }
            .to_string(),
        ),
        inspection: None,
        inspection_score: None,
    })
}

fn xml_text(xml: &str, tag: &str) -> Option<String> {
    let re = regex::Regex::new(&format!(
        r"(?s)<{}[^>]*>(.*?)</{}>",
        regex::escape(tag),
        regex::escape(tag)
    ))
    .ok()?;
    let raw = re.captures(xml)?.get(1)?.as_str();
    Some(html_unescape(raw.trim()))
}

fn attr_first(xml: &str, tag: &str, attr: &str) -> Option<String> {
    let re = regex::Regex::new(&format!(
        r#"<{}\b[^>]*\s{}=[\"']([^\"']+)[\"'][^>]*"#,
        regex::escape(tag),
        regex::escape(attr)
    ))
    .ok()?;
    Some(html_unescape(re.captures(xml)?.get(1)?.as_str()))
}

fn torznab_attr(xml: &str, name: &str) -> Option<String> {
    let re = regex::Regex::new(&format!(
        r#"<torznab:attr\b[^>]*\bname=[\"']{}[\"'][^>]*\bvalue=[\"']([^\"']*)[\"'][^>]*/?>"#,
        regex::escape(name)
    ))
    .ok()?;
    Some(html_unescape(re.captures(xml)?.get(1)?.as_str()))
}

fn html_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn looks_like_direct_torrent_download(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("download.php?id=") || lower.ends_with(".torrent")
}

fn should_hide_provider_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("magnet:") || lower.starts_with("http://") || lower.starts_with("https://")
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
    res.tmdb_title = data["title"]
        .as_str()
        .or_else(|| data["name"].as_str())
        .map(|s| s.to_string());
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    async fn serve_raw_http(response: Vec<u8>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 2048];
            let _ = stream.read(&mut request).await;
            stream.write_all(&response).await.unwrap();
        });
        format!("http://{addr}/")
    }

    fn multi_fixture(
        source: &str,
        provider: &str,
        url: &str,
        download_ref: Option<&str>,
    ) -> MultiSearchResult {
        MultiSearchResult {
            id: format!("{source}-id"),
            name: "Same Release".into(),
            parsed_name: "Same Release".into(),
            url: url.into(),
            download_ref: download_ref.map(str::to_string),
            size: 42,
            source: source.into(),
            provider: provider.into(),
            indexer: Some("NetHD".into()),
            origin_provider: None,
            origin_indexer: None,
            cached: None,
            file_idx: None,
            sources: None,
            behavior_hints: None,
            seeders: None,
            leechers: None,
            info_hash: None,
            quality: None,
            resolution: None,
            episode_tag: None,
            media_type: None,
            inspection: None,
            inspection_score: None,
        }
    }

    #[tokio::test]
    async fn multi_torbox_deadline_preserves_existing_rows() {
        let mut rows = vec![multi_fixture(
            "fshare",
            "fshare",
            "https://fshare.vn/file/A",
            None,
        )];
        let outcome = await_multi_torbox_until(
            tokio::time::Instant::now() + std::time::Duration::from_millis(10),
            async {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                rows.clear();
                Ok::<usize, ()>(0)
            },
        )
        .await;

        assert!(outcome.is_none());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "fshare-id");
    }

    #[tokio::test]
    async fn search_admission_allows_four_and_rejects_fifth_until_release() {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(4));
        let mut permits = Vec::new();
        for _ in 0..4 {
            permits.push(try_search_permit(&semaphore).unwrap());
        }
        let rejected = try_search_permit(&semaphore).unwrap_err();
        assert_eq!(rejected.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(rejected.headers()[axum::http::header::RETRY_AFTER], "1");

        permits.pop();
        assert!(try_search_permit(&semaphore).is_ok());
    }

    #[tokio::test]
    async fn prowlarr_indexer_body_uses_the_shared_bound() {
        let body = vec![b'x'; PROWLARR_RESPONSE_LIMIT_BYTES + 1];
        let response = [
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .as_bytes(),
            body.as_slice(),
        ]
        .concat();
        let url = serve_raw_http(response).await;
        let response = reqwest::Client::new().get(url).send().await.unwrap();

        let error = parse_bounded_prowlarr_json::<Vec<ProwlarrIndexer>>(response)
            .await
            .unwrap_err();

        assert_eq!(error.to_string(), "Prowlarr response body exceeded limit");
    }

    #[tokio::test]
    async fn bounded_response_rejects_declared_oversize() {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            PROWLARR_RESPONSE_LIMIT_BYTES + 1
        );
        let url = serve_raw_http(response.into_bytes()).await;
        let response = reqwest::Client::new().get(url).send().await.unwrap();

        let error = read_bounded_response(response, PROWLARR_RESPONSE_LIMIT_BYTES)
            .await
            .unwrap_err();

        assert_eq!(error.to_string(), "Prowlarr response body exceeded limit");
    }

    #[tokio::test]
    async fn bounded_response_rejects_chunked_oversize_and_accepts_valid_body() {
        let oversized = vec![b'x'; PROWLARR_RESPONSE_LIMIT_BYTES + 1];
        let response = [
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
                .as_slice(),
            format!("{:x}\r\n", oversized.len()).as_bytes(),
            oversized.as_slice(),
            b"\r\n0\r\n\r\n",
        ]
        .concat();
        let url = serve_raw_http(response).await;
        let response = reqwest::Client::new().get(url).send().await.unwrap();
        assert_eq!(
            read_bounded_response(response, PROWLARR_RESPONSE_LIMIT_BYTES)
                .await
                .unwrap_err()
                .to_string(),
            "Prowlarr response body exceeded limit"
        );

        let body = br#"[{"title":"ok"}]"#;
        let response = [
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .as_bytes(),
            body,
        ]
        .concat();
        let url = serve_raw_http(response).await;
        let response = reqwest::Client::new().get(url).send().await.unwrap();
        assert_eq!(
            read_bounded_response(response, PROWLARR_RESPONSE_LIMIT_BYTES)
                .await
                .unwrap(),
            String::from_utf8(body.to_vec()).unwrap()
        );
    }

    #[test]
    fn free_text_uses_typed_deadline_finalization_and_total_job_cap() {
        let source = include_str!("search.rs");
        assert!(source.contains("search_prowlarr_query_outcomes_until"));
        assert!(source.contains("searches.truncate(15)"));
        assert!(source.contains("finalize_prowlarr_outcomes_until"));
        assert!(source.contains("tokio::time::timeout_at(deadline, fshare_future)"));
    }

    #[test]
    fn public_multi_uses_one_absolute_deadline_for_free_text_work() {
        let source = include_str!("search.rs");
        assert!(source.contains(
            "search_prowlarr_intent_until(
                &state,
                TorrentSearchIntent::FreeText"
        ));
        assert!(
            source.contains("inspect_prowlarr_results_until(&state, &mut found, None, deadline)")
        );
    }

    #[test]
    fn multi_canonical_normalization_is_source_aware() {
        let fshare_url = "https://www.fshare.vn/file/ABC";
        let signed = "https://nethd.example/download.php?id=1&apikey=secret";
        let opaque = "prowlarr-ref://opaque";
        let mut results = vec![
            multi_fixture("fshare", "fshare", fshare_url, None),
            multi_fixture("fshare", "fshare", fshare_url, None),
            multi_fixture("torrent", "prowlarr", signed, Some(opaque)),
            multi_fixture(
                "torrent",
                "prowlarr",
                "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567",
                Some(opaque),
            ),
        ];

        normalize_multi_results(&mut results);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].url, fshare_url);
        assert_eq!(results[0].download_ref, None);
        assert_eq!(results[1].source, "torrent");
        assert_eq!(results[1].download_ref.as_deref(), Some(opaque));
        let wire = serde_json::to_string(&results).unwrap();
        assert!(!wire.contains("magnet:"));
        assert!(!wire.contains("apikey=secret"));
    }

    #[test]
    fn parse_prowlarr_json_items_prefers_alt_hash_fields() {
        let data = json!([
            {
                "title": "The Lion King 1 1/2 2004 1080p BluRay x265",
                "downloadUrl": "https://indexer.example/download?id=123",
                "guid": "https://guid.example/item/123",
                "infohash": "22A2E1DF69B9016D5130D3E20DEEC478EEDADF8B",
                "seeders": 3,
                "size": 1449551488
            }
        ]);

        let results = parse_prowlarr_json_items(&data, "lion king", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].info_hash.as_deref(),
            Some("22a2e1df69b9016d5130d3e20deec478eedadf8b")
        );
    }

    #[test]
    fn parse_prowlarr_json_items_extracts_hash_from_magnet_when_present() {
        let data = json!([
            {
                "title": "The Lion King 1 1/2 2004 1080p BluRay x265",
                "magnet": "magnet:?xt=urn:btih:22A2E1DF69B9016D5130D3E20DEEC478EEDADF8B&dn=lion",
                "seeders": 3,
                "size": 1449551488
            }
        ]);

        let results = parse_prowlarr_json_items(&data, "lion king", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].info_hash.as_deref(),
            Some("22a2e1df69b9016d5130d3e20deec478eedadf8b")
        );
    }

    #[test]
    fn parse_prowlarr_json_items_prefers_prowlarr_download_url_over_private_guid() {
        let data = json!([
            {
                "title": "The Lion King 1994 BluRay REMUX AVC DTS-HD MA 7 1-EPiK",
                "downloadUrl": "http://192.168.1.112:9696/1/download?apikey=secret&link=abc",
                "guid": "https://example.test/download.php?id=204885",
                "seeders": 12,
                "size": 28711856128_u64
            }
        ]);

        let results = parse_prowlarr_json_items(&data, "lion king", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].url,
            "http://192.168.1.112:9696/1/download?apikey=secret&link=abc"
        );
    }

    #[test]
    fn prowlarr_json_array_tracks_discarded_items() {
        let parsed = parse_prowlarr_json_items_with_diagnostics(
            &json!([
                {"title": "Valid", "magnet": "magnet:?xt=urn:btih:abc"},
                {"title": "No locator"}
            ]),
            "valid",
            10,
        );

        assert_eq!(parsed.item_count, 2);
        assert_eq!(parsed.results.len(), 1);
        assert_eq!(parsed.discarded_count, 1);
        assert!(!parsed.unexpected_shape);
    }

    #[test]
    fn prowlarr_non_array_json_is_diagnostic_not_empty_array() {
        let parsed =
            parse_prowlarr_json_items_with_diagnostics(&json!({"message": "busy"}), "x", 10);

        assert!(parsed.results.is_empty());
        assert!(parsed.unexpected_shape);
        assert_eq!(parsed.diagnostic, "unexpected JSON shape");
    }

    #[test]
    fn detects_direct_torrent_download_links() {
        assert!(looks_like_direct_torrent_download(
            "https://nethd.org/download.php?id=204885"
        ));
        assert!(looks_like_direct_torrent_download(
            "https://indexer.example/file.torrent"
        ));
        assert!(!looks_like_direct_torrent_download(
            "http://192.168.1.112:9696/1/download?apikey=secret&link=abc"
        ));
    }

    #[tokio::test]
    async fn per_lane_elapsed_timeout_maps_to_typed_timeout() {
        let outcome = typed_lane_outcome(std::time::Duration::from_millis(10), async {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            Ok(ProwlarrSearchResponse {
                results: Vec::new(),
                item_count: 0,
                discarded_count: 0,
                unexpected_shape: false,
                diagnostic: "test",
            })
        })
        .await;

        assert!(matches!(outcome, ProwlarrQueryOutcome::TimedOut));
    }

    #[test]
    fn episode_lanes_include_episode_and_required_season_searches() {
        let lanes = prowlarr_lanes(&TorrentSearchIntent::Episode {
            tvdb_id: Some("42".into()),
            imdb_id: None,
            season: 2,
            episode: 3,
        });
        assert_eq!(lanes.len(), 2);
        assert_eq!(lanes[0].episode, Some(3));
        assert_eq!(lanes[1].episode, None);
        assert_eq!(lanes[1].season, Some(2));
    }

    #[test]
    fn movie_newznab_url_uses_exact_endpoint_and_lowercase_params() {
        let lane = prowlarr_lanes(&TorrentSearchIntent::Movie {
            tmdb_id: Some("550".into()),
            imdb_id: Some("tt0137523".into()),
        })
        .remove(0);
        let url = reqwest::Url::parse(&newznab_url(
            "http://localhost:9696/",
            39,
            "Fight Club",
            25,
            Some("2000,2010"),
            &lane,
        ))
        .unwrap();
        assert_eq!(url.path(), "/api/v1/indexer/39/newznab");
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(params.get("t").map(String::as_str), Some("movie"));
        assert_eq!(params.get("q").map(String::as_str), Some("Fight Club"));
        assert_eq!(params.get("limit").map(String::as_str), Some("25"));
        assert_eq!(params.get("cat").map(String::as_str), Some("2000,2010"));
        assert_eq!(params.get("tmdbid").map(String::as_str), Some("550"));
        assert_eq!(params.get("imdbid").map(String::as_str), Some("tt0137523"));
        for invalid in ["query", "type", "categories", "tmdbId", "imdbId"] {
            assert!(!params.contains_key(invalid));
        }
    }

    #[test]
    fn episode_newznab_url_uses_tvdbid_season_ep() {
        let lane = prowlarr_lanes(&TorrentSearchIntent::Episode {
            tvdb_id: Some("42".into()),
            imdb_id: None,
            season: 2,
            episode: 3,
        })
        .remove(0);
        let url = reqwest::Url::parse(&newznab_url(
            "http://localhost:9696",
            7,
            "Show",
            10,
            None,
            &lane,
        ))
        .unwrap();
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(params.get("t").map(String::as_str), Some("tvsearch"));
        assert_eq!(params.get("tvdbid").map(String::as_str), Some("42"));
        assert_eq!(params.get("season").map(String::as_str), Some("2"));
        assert_eq!(params.get("ep").map(String::as_str), Some("3"));
        assert!(!params.contains_key("episode"));
    }

    #[test]
    fn prowlarr_lane_dedupe_is_stable_across_required_lanes() {
        let duplicate = multi_fixture("torrent", "prowlarr", "", Some("opaque-a"));
        let mut second = duplicate.clone();
        second.download_ref = Some("opaque-b".into());
        let response = |results| {
            Ok(ProwlarrSearchResponse {
                results,
                item_count: 1,
                discarded_count: 0,
                unexpected_shape: false,
                diagnostic: "test",
            })
        };
        let mut outcomes = vec![response(vec![duplicate]), response(vec![second])];
        dedupe_prowlarr_outcomes(&mut outcomes);
        assert_eq!(
            outcomes
                .iter()
                .filter_map(|outcome| outcome.as_ref().ok())
                .map(|response| response.results.len())
                .sum::<usize>(),
            1
        );
    }

    #[tokio::test]
    async fn deadline_scheduler_counts_only_started_jobs_as_typed_timeouts() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let began = Arc::new(AtomicUsize::new(0));
        let jobs = (0..8)
            .map(|_| {
                let began = Arc::clone(&began);
                async move {
                    began.fetch_add(1, Ordering::SeqCst);
                    std::future::pending::<()>().await;
                    ProwlarrQueryOutcome::Failed
                }
            })
            .collect();

        let run = collect_prowlarr_jobs_until_deadline(
            jobs,
            tokio::time::Instant::now() + std::time::Duration::from_millis(10),
            2,
        )
        .await;
        let typed_timeouts = run
            .outcomes
            .iter()
            .filter(|outcome| matches!(outcome, ProwlarrQueryOutcome::TimedOut))
            .count();

        assert_eq!(run.planned, 8);
        assert!(run.started < run.planned);
        assert_eq!(run.started, began.load(Ordering::SeqCst));
        assert_eq!(run.outcomes.len(), run.started);
        assert_eq!(typed_timeouts, run.started);
        assert_eq!(run.unstarted, run.planned - run.started);
    }

    #[tokio::test]
    async fn deadline_scheduler_caps_concurrency_and_reports_pending() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let jobs = (0..20)
            .map(|index| {
                let active = active.clone();
                let peak = peak.clone();
                async move {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    if index > 1 {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                    active.fetch_sub(1, Ordering::SeqCst);
                    index
                }
            })
            .collect();

        let (completed, timed_out) = collect_until_deadline(
            jobs,
            tokio::time::Instant::now() + std::time::Duration::from_millis(30),
            4,
        )
        .await;

        assert_eq!(peak.load(Ordering::SeqCst), 4);
        assert!(completed.contains(&0) && completed.contains(&1));
        assert_eq!(completed.len() + timed_out, 20);
        assert!(timed_out > 0);
    }

    #[tokio::test]
    async fn deadline_scheduler_restores_planned_order_after_fast_completion() {
        let jobs: Vec<futures_util::future::BoxFuture<'static, usize>> = vec![
            Box::pin(async {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                0
            }),
            Box::pin(async { 1 }),
        ];
        let (completed, timed_out) = collect_until_deadline(
            jobs,
            tokio::time::Instant::now() + std::time::Duration::from_secs(1),
            2,
        )
        .await;

        assert_eq!(timed_out, 0);
        assert_eq!(completed, vec![0, 1]);
    }

    #[test]
    fn season_pack_classification_requires_file_evidence() {
        let paths = ["Show.S02E01.mkv", "Show.S02E02.mkv"];
        assert_eq!(
            classify_torrent_paths(&paths, Some((2, None))),
            TorrentContentKind::SeasonPack
        );
    }

    #[test]
    fn classification_uses_media_paths_only() {
        for path in ["readme.txt", "Show.S02E03.srt", "movie.nfo"] {
            assert_eq!(
                classify_torrent_paths(&[path], Some((2, Some(3)))),
                TorrentContentKind::Unknown
            );
        }
        assert_eq!(
            classify_torrent_paths(&["Movie.2026.mkv"], None),
            TorrentContentKind::Movie
        );
    }

    #[test]
    fn inspection_failure_classification_is_bounded_and_secret_free() {
        let sentinel = "?apikey=SECRET&tracker=SECRET";
        let cases = [
            (
                "download ref unavailable",
                InspectionFailureClass::DownloadRefUnavailable,
            ),
            (
                "locator is not a torrent download",
                InspectionFailureClass::LocatorInvalid,
            ),
            (
                "torrent URL fetch failed: operation timed out",
                InspectionFailureClass::FetchTimeout,
            ),
            (
                "torrent URL fetch failed: connection reset",
                InspectionFailureClass::FetchNetwork,
            ),
            (
                "torrent URL returned HTTP 404 Not Found",
                InspectionFailureClass::Http4xx,
            ),
            (
                "torrent URL returned HTTP 503 Service Unavailable",
                InspectionFailureClass::Http5xx,
            ),
            (
                "torrent redirect rejected: cross_origin_non_https",
                InspectionFailureClass::RedirectCrossOriginNonHttps,
            ),
            (
                "torrent redirect rejected: target_private_ip",
                InspectionFailureClass::RedirectTargetPrivateIp,
            ),
            (
                "torrent redirect rejected: target_userinfo",
                InspectionFailureClass::RedirectTargetUserinfo,
            ),
            (
                "torrent redirect rejected: dns_resolution_failed",
                InspectionFailureClass::RedirectDnsResolutionFailed,
            ),
            (
                "torrent redirect rejected: redirect_limit",
                InspectionFailureClass::RedirectLimit,
            ),
            (
                "torrent redirect rejected: strict_same_origin",
                InspectionFailureClass::RedirectStrictSameOrigin,
            ),
            (
                "torrent redirect rejected: location_missing_or_invalid",
                InspectionFailureClass::RedirectLocationMissingOrInvalid,
            ),
            (
                "torrent redirect rejected: location_join_failed",
                InspectionFailureClass::RedirectLocationJoinFailed,
            ),
            (
                "torrent redirect rejected: target_magnet",
                InspectionFailureClass::RedirectTargetMagnet,
            ),
            (
                "torrent redirect rejected: target_scheme_other",
                InspectionFailureClass::RedirectTargetSchemeOther,
            ),
            (
                "torrent redirect rejected: target_host_missing",
                InspectionFailureClass::RedirectTargetHostMissing,
            ),
            (
                "torrent response type is invalid",
                InspectionFailureClass::ResponseType,
            ),
            (
                "torrent body is too large",
                InspectionFailureClass::TooLarge,
            ),
            ("unexpected failure", InspectionFailureClass::Other),
        ];
        for (prefix, expected) in cases {
            let class = classify_inspection_failure(&format!("{prefix}{sentinel}"));
            assert_eq!(class, expected);
            assert!(!format!("{class:?}").contains("SECRET"));
        }
    }

    #[test]
    fn redirect_target_scheme_buckets_are_counted_separately() {
        let mut counts = InspectionCounts::default();
        counts.record_failure(classify_inspection_failure(
            "torrent redirect rejected: target_magnet",
        ));
        counts.record_failure(classify_inspection_failure(
            "torrent redirect rejected: target_scheme_other",
        ));

        assert_eq!(counts.redirect, 2);
        assert_eq!(counts.target_magnet, 1);
        assert_eq!(counts.target_scheme_other, 1);
    }

    #[tokio::test]
    async fn metainfo_failure_is_counted_separately() {
        let cache = moka::future::Cache::builder().max_capacity(1).build();
        let mut rows = vec![multi_fixture("torrent", "prowlarr", "", Some("ref"))];
        rows[0].info_hash = Some("0000000000000000000000000000000000000001".into());
        let counts = enrich_prowlarr_candidates(
            &mut rows,
            None,
            &cache,
            tokio::time::Instant::now() + std::time::Duration::from_secs(15),
            |_| async { Ok(b"not bencode SECRET".to_vec()) },
        )
        .await;
        assert_eq!(counts.metainfo_invalid, 1);
        assert_eq!(counts.other, 0);
        assert!(!format!("{counts:?}").contains("SECRET"));
    }

    #[tokio::test]
    async fn subtitle_episode_tag_is_not_episode_evidence() {
        let cache = moka::future::Cache::builder().max_capacity(1).build();
        let mut rows = vec![multi_fixture("torrent", "prowlarr", "", Some("ref"))];
        rows[0].info_hash = Some("0000000000000000000000000000000000000001".into());
        enrich_prowlarr_candidates(
            &mut rows,
            None,
            &cache,
            tokio::time::Instant::now() + std::time::Duration::from_secs(15),
            |_| async { Ok(b"d4:infod6:lengthi1e4:name15:Show.S02E03.srtee".to_vec()) },
        )
        .await;
        let summary = rows[0].inspection.as_ref().unwrap();
        assert_eq!(summary.classification, TorrentContentKind::Unknown);
        assert!(summary.episode_evidence.is_empty());
    }

    #[tokio::test]
    async fn inspection_keeps_fast_evidence_and_marks_pending_at_deadline() {
        let cache = moka::future::Cache::builder().max_capacity(2).build();
        let mut rows = vec![
            multi_fixture("torrent", "prowlarr", "", Some("fast-ref")),
            multi_fixture("torrent", "prowlarr", "", Some("slow-ref")),
        ];
        rows[0].info_hash = Some("0000000000000000000000000000000000000001".into());
        rows[1].info_hash = Some("0000000000000000000000000000000000000002".into());

        let counts = enrich_prowlarr_candidates(
            &mut rows,
            None,
            &cache,
            tokio::time::Instant::now() + std::time::Duration::from_millis(30),
            |locator| async move {
                if locator == "slow-ref" {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                Ok(b"d4:infod6:lengthi1e4:name9:movie.mkvee".to_vec())
            },
        )
        .await;

        assert_eq!(counts.inspected, 1);
        assert_eq!(counts.timed_out, 1);
        assert_eq!(rows[0].inspection.as_ref().unwrap().status, "inspected");
        assert_eq!(rows[1].inspection.as_ref().unwrap().status, "timeout");
    }

    #[tokio::test]
    async fn inspection_deadline_leaves_non_selected_rows_untouched() {
        let cache = moka::future::Cache::builder().max_capacity(20).build();
        let mut rows = (0..11)
            .map(|index| {
                let mut row =
                    multi_fixture("torrent", "prowlarr", "", Some(&format!("ref-{index}")));
                row.info_hash = Some(format!("{index:040x}"));
                row.seeders = Some((11 - index) as i64);
                row
            })
            .collect::<Vec<_>>();

        let counts = enrich_prowlarr_candidates(
            &mut rows,
            None,
            &cache,
            tokio::time::Instant::now() + std::time::Duration::from_millis(30),
            |locator| async move {
                if locator == "ref-0" {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                Ok(b"d4:infod6:lengthi1e4:name9:movie.mkvee".to_vec())
            },
        )
        .await;

        assert_eq!(counts.selected, 10);
        assert_eq!(counts.timed_out, 1);
        assert_eq!(rows[0].inspection.as_ref().unwrap().status, "timeout");
        assert!(rows[10].inspection.is_none());
    }

    #[tokio::test]
    async fn inspection_is_top_ten_max_three_cached_failure_retained_and_magnet_skipped() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let cache = moka::future::Cache::builder().max_capacity(20).build();
        let mut rows = (0..12)
            .map(|n| {
                let mut row = multi_fixture("torrent", "prowlarr", "", Some(&format!("ref-{n}")));
                row.info_hash = Some(format!("{n:040x}"));
                row
            })
            .collect::<Vec<_>>();
        rows.push(multi_fixture("torrent", "prowlarr", "", None));
        enrich_prowlarr_candidates(
            &mut rows,
            None,
            &cache,
            tokio::time::Instant::now() + std::time::Duration::from_secs(15),
            {
                let calls = calls.clone();
                let active = active.clone();
                let peak = peak.clone();
                move |_| {
                    let calls = calls.clone();
                    let active = active.clone();
                    let peak = peak.clone();
                    async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                        active.fetch_sub(1, Ordering::SeqCst);
                        Err("bad torrent".to_string())
                    }
                }
            },
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 10);
        assert_eq!(peak.load(Ordering::SeqCst), 3);
        assert_eq!(rows.len(), 13);
        assert_eq!(rows[0].inspection.as_ref().unwrap().status, "failed");
        assert!(rows[12].inspection.is_none());
        enrich_prowlarr_candidates(
            &mut rows,
            None,
            &cache,
            tokio::time::Instant::now() + std::time::Duration::from_secs(15),
            {
                let calls = calls.clone();
                move |_| {
                    let calls = calls.clone();
                    async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Err("bad torrent".to_string())
                    }
                }
            },
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 20);
    }

    #[tokio::test]
    async fn successful_inspection_is_cached() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = Arc::new(AtomicUsize::new(0));
        let cache = moka::future::Cache::builder().max_capacity(1).build();
        let mut rows = vec![multi_fixture("torrent", "prowlarr", "", Some("ref"))];
        rows[0].info_hash = Some("0000000000000000000000000000000000000001".into());
        for _ in 0..2 {
            enrich_prowlarr_candidates(
                &mut rows,
                None,
                &cache,
                tokio::time::Instant::now() + std::time::Duration::from_secs(15),
                {
                    let calls = calls.clone();
                    move |_| {
                        let calls = calls.clone();
                        async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            Ok(b"d4:infod6:lengthi1e4:name9:movie.mkvee".to_vec())
                        }
                    }
                },
            )
            .await;
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            rows[0].inspection.as_ref().unwrap().classification,
            TorrentContentKind::Movie
        );
    }

    #[tokio::test]
    async fn structured_movie_inspection_uses_opaque_ref_and_reports_sanitized_counts() {
        let cache = moka::future::Cache::builder().max_capacity(1).build();
        let mut rows = vec![multi_fixture(
            "torrent",
            "prowlarr",
            "",
            Some("opaque-download-ref"),
        )];
        rows[0].info_hash = Some("0000000000000000000000000000000000000042".into());
        let intent = TorrentSearchIntent::Movie {
            tmdb_id: Some("42".into()),
            imdb_id: None,
        };

        let counts = enrich_prowlarr_candidates(
            &mut rows,
            Some(&intent),
            &cache,
            tokio::time::Instant::now() + std::time::Duration::from_secs(15),
            |locator| async move {
                assert_eq!(locator, "opaque-download-ref");
                Ok(b"d4:infod6:lengthi1e4:name9:movie.mkvee".to_vec())
            },
        )
        .await;

        let summary = rows[0].inspection.as_ref().unwrap();
        assert_eq!(summary.classification, TorrentContentKind::Movie);
        assert_eq!(inspection_score(&intent, Some(summary)), 1_000);
        assert_eq!(
            counts,
            InspectionCounts {
                selected: 1,
                inspected: 1,
                failed: 0,
                cache_hits: 0,
                unavailable: 0,
                ..Default::default()
            }
        );
        let debug = format!("{counts:?}");
        for secret in [
            "opaque-download-ref",
            "tracker.invalid",
            "SECRET",
            "apikey=",
        ] {
            assert!(!debug.contains(secret));
        }
    }

    #[tokio::test]
    async fn production_inspection_caps_wire_evidence_and_excludes_locator_secrets() {
        let sentinel = "downloadRef=https://tracker.invalid/announce?apikey=SECRET&magnet=SECRET";
        let mut torrent = b"d4:infod5:filesl".to_vec();
        for index in 0..10_000 {
            let path = format!("file-{index:05}.mkv");
            torrent.extend_from_slice(
                format!("d6:lengthi1e4:pathl{}:{}ee", path.len(), path).as_bytes(),
            );
        }
        torrent.extend_from_slice(b"e4:name4:rootee");
        let cache = moka::future::Cache::builder().max_capacity(1).build();
        let mut rows = vec![multi_fixture(
            "torrent",
            "prowlarr",
            sentinel,
            Some(sentinel),
        )];
        rows[0].info_hash = Some("0000000000000000000000000000000000000001".into());
        enrich_prowlarr_candidates(
            &mut rows,
            None,
            &cache,
            tokio::time::Instant::now() + std::time::Duration::from_secs(15),
            move |_| {
                let torrent = torrent.clone();
                async move { Ok(torrent) }
            },
        )
        .await;

        let summary = rows[0].inspection.as_ref().unwrap();
        assert_eq!(summary.file_count, Some(10_000));
        assert_eq!(summary.evidence_total_count, 10_000);
        assert!(summary.evidence_truncated);
        assert_eq!(summary.evidence.as_ref().unwrap().files.len(), 100);
        let wire = serde_json::to_string(summary).unwrap();
        assert!(wire.contains("\"evidence_total\":10000"));
        assert!(!wire.contains("evidence_total_count"));
        for secret in [
            "downloadRef",
            "https://",
            "magnet",
            "tracker",
            "apikey",
            "SECRET",
        ] {
            assert!(!wire.contains(secret), "summary leaked {secret}: {wire}");
        }
    }

    #[tokio::test]
    async fn inspection_dedupes_then_ranks_before_top_ten() {
        let cache = moka::future::Cache::builder().max_capacity(20).build();
        let mut rows = (0..11)
            .map(|n| {
                let mut row = multi_fixture("torrent", "prowlarr", "", Some(&format!("ref-{n}")));
                row.info_hash = Some(if n < 2 {
                    [
                        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    ][n]
                        .into()
                } else {
                    format!("{n:040x}")
                });
                row.seeders = Some(n as i64);
                row
            })
            .collect::<Vec<_>>();
        let fetched = Arc::new(std::sync::Mutex::new(Vec::new()));
        enrich_prowlarr_candidates(
            &mut rows,
            None,
            &cache,
            tokio::time::Instant::now() + std::time::Duration::from_secs(15),
            {
                let fetched = fetched.clone();
                move |locator| {
                    let fetched = fetched.clone();
                    async move {
                        fetched.lock().unwrap().push(locator);
                        Err("bad torrent".to_string())
                    }
                }
            },
        )
        .await;
        let fetched = fetched.lock().unwrap();
        assert_eq!(fetched.len(), 10);
        assert_eq!(
            fetched
                .iter()
                .filter(|locator| locator.as_str() == "ref-0" || locator.as_str() == "ref-1")
                .count(),
            1
        );
        assert!(fetched.contains(&"ref-10".to_string()));
    }

    #[test]
    fn verified_episode_evidence_scores_exact_target_even_for_pack() {
        let summary = TorrentInspectionSummary {
            status: "inspected",
            classification: TorrentContentKind::SeasonPack,
            root_name: None,
            file_count: Some(2),
            media_file_count: Some(2),
            total_size: None,
            evidence_total_count: 0,
            evidence_truncated: false,
            episode_evidence: vec!["S02E02".into(), "S02E03".into()],
            evidence: None,
        };
        assert_eq!(
            inspection_score(
                &TorrentSearchIntent::Episode {
                    tvdb_id: None,
                    imdb_id: None,
                    season: 2,
                    episode: 3
                },
                Some(&summary)
            ),
            1_000
        );
        assert_eq!(
            inspection_score(
                &TorrentSearchIntent::Episode {
                    tvdb_id: None,
                    imdb_id: None,
                    season: 2,
                    episode: 4
                },
                Some(&summary)
            ),
            -250
        );
    }

    #[test]
    fn verified_season_evidence_scores_multiple_target_season_episodes() {
        let summary = TorrentInspectionSummary {
            status: "inspected",
            classification: TorrentContentKind::MultiEpisode,
            root_name: None,
            file_count: Some(2),
            media_file_count: Some(2),
            total_size: None,
            evidence_total_count: 0,
            evidence_truncated: false,
            episode_evidence: vec!["S02E01".into(), "S02E02".into()],
            evidence: None,
        };
        assert_eq!(
            inspection_score(
                &TorrentSearchIntent::Season {
                    tvdb_id: None,
                    imdb_id: None,
                    season: 2
                },
                Some(&summary)
            ),
            1_000
        );
    }

    #[test]
    fn movie_scoring_is_not_free_text_scoring() {
        let summary = TorrentInspectionSummary {
            status: "inspected",
            classification: TorrentContentKind::Movie,
            root_name: None,
            file_count: Some(1),
            media_file_count: Some(1),
            total_size: None,
            evidence_total_count: 0,
            evidence_truncated: false,
            episode_evidence: vec![],
            evidence: None,
        };
        assert_eq!(
            inspection_score(
                &TorrentSearchIntent::Movie {
                    tmdb_id: None,
                    imdb_id: None
                },
                Some(&summary)
            ),
            1_000
        );
        assert_eq!(
            inspection_score(&TorrentSearchIntent::FreeText, Some(&summary)),
            0
        );
    }
}
