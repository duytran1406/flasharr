pub mod aggregator;
pub mod orchestrator;
pub mod types;

use crate::services::folder_cache_service::expand_folder_cache_query_variants;
use crate::services::tmdb_service::MediaEnrichment;
use crate::services::tvdb_service::TvdbEnrichment;
use crate::utils::smart_tokenizer::smart_parse;
use crate::utils::title_matcher::{
    calculate_unified_similarity, detect_badges, extract_core_title, group_by_quality,
    normalize_vietnamese, SmartSearchResult,
};
use crate::utils::unified_scorer::{calculate_match_score, is_valid_match};
use crate::AppState;
use crate::TmdbEnrichmentCache;
use axum::{
    extract::{Json, State},
    response::IntoResponse,
    routing::post,
    Router,
};
use futures_util::{stream, StreamExt};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::OnceLock;
#[cfg(test)]
use tokio::time::Duration;
use tokio::time::Instant;
use tracing::info;

use types::parse_smart_search_request;
pub use types::{SmartSearchRequest, SmartSearchResponse};

pub(crate) const FOLDER_CACHE_SEARCH_LIMIT: u32 = 20;
pub(crate) const FOLDER_CACHE_SEASON_SEARCH_LIMIT: u32 = 200;

fn folder_cache_search_limit_for_request(req: &SmartSearchRequest) -> u32 {
    if req.r#type == "tv" && req.season.is_some() && req.episode.is_none() {
        FOLDER_CACHE_SEASON_SEARCH_LIMIT
    } else {
        FOLDER_CACHE_SEARCH_LIMIT
    }
}

fn folder_cache_search_limit_for_query(query: &str, limit: u32) -> u32 {
    if expand_folder_cache_query_variants(query)
        .iter()
        .any(|variant| !variant.eq_ignore_ascii_case(query))
    {
        limit.max(FOLDER_CACHE_SEASON_SEARCH_LIMIT)
    } else {
        limit
    }
}

#[derive(Clone, Copy)]
struct FolderSearchIdentity {
    tmdb_id: Option<i64>,
    media_type: Option<&'static str>,
    year: Option<u32>,
    season: Option<u32>,
    episode: Option<u32>,
}

fn mapped_folder_media_type(value: Option<&str>) -> Option<&'static str> {
    match value?.to_ascii_lowercase().as_str() {
        "movie" => Some("movie"),
        "tv" | "series" => Some("tv"),
        _ => None,
    }
}

fn folder_item_matches_identity(
    item: &crate::db::sqlite::CachedFolderItem,
    identity: FolderSearchIdentity,
) -> bool {
    if item
        .tmdb_id
        .zip(identity.tmdb_id)
        .is_some_and(|(item, target)| item != target)
    {
        return false;
    }
    if mapped_folder_media_type(item.media_type_hint.as_deref())
        .zip(identity.media_type)
        .is_some_and(|(item, target)| item != target)
    {
        return false;
    }
    if item
        .year
        .zip(identity.year)
        .is_some_and(|(item, target)| item.abs_diff(target) > 1)
    {
        return false;
    }
    if item
        .season
        .zip(identity.season)
        .is_some_and(|(item, target)| item != target)
    {
        return false;
    }
    !item
        .episode
        .zip(identity.episode)
        .is_some_and(|(item, target)| item != target)
}

fn folder_search_jobs(queries: &[String], limit: u32) -> Vec<(String, u32)> {
    let mut seen = std::collections::HashSet::new();
    queries
        .iter()
        .flat_map(|query| {
            let limit = folder_cache_search_limit_for_query(query, limit);
            expand_folder_cache_query_variants(query)
                .into_iter()
                .map(move |query| (query, limit))
        })
        .filter(|(query, _)| {
            let normalized = query.split_whitespace().collect::<Vec<_>>().join(" ");
            !normalized.is_empty() && seen.insert(normalized.to_lowercase())
        })
        .take(32)
        .collect()
}

struct FolderSearchRun<T> {
    completed: Vec<T>,
    truncated: bool,
}

async fn collect_folder_search_jobs_until<T, U, F, Fut>(
    jobs: Vec<T>,
    deadline: Instant,
    search: F,
) -> FolderSearchRun<U>
where
    T: Send + 'static,
    U: Send + 'static,
    F: Fn(T) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = U> + Send,
{
    let mut searches = stream::iter(jobs.into_iter().take(32).map(search)).buffer_unordered(4);
    let mut completed = Vec::new();
    loop {
        match tokio::time::timeout_at(deadline, searches.next()).await {
            Ok(Some(item)) => completed.push(item),
            Ok(None) => {
                return FolderSearchRun {
                    completed,
                    truncated: false,
                }
            }
            Err(_) => {
                return FolderSearchRun {
                    completed,
                    truncated: true,
                }
            }
        }
    }
}

async fn search_folder_cache_jobs_until(
    state: &Arc<AppState>,
    jobs: Vec<(String, u32)>,
    deadline: Instant,
) -> FolderSearchRun<(usize, Vec<crate::db::sqlite::CachedFolderItem>)> {
    collect_folder_search_jobs_until(jobs.into_iter().enumerate().collect(), deadline, |job| {
        let state = Arc::clone(state);
        async move {
            let (position, (query, limit)) = job;
            let items = state
                .folder_cache_service
                .search(&query, limit)
                .await
                .unwrap_or_default();
            (position, items)
        }
    })
    .await
}

fn stable_dedupe_folder_items(
    items: Vec<crate::db::sqlite::CachedFolderItem>,
) -> Vec<crate::db::sqlite::CachedFolderItem> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.linkcode.clone()))
        .collect()
}

struct FolderSearchOutput {
    results: Vec<ProviderSearchResult>,
    truncated: bool,
}

pub(crate) async fn search_folder_cache_with_limit(
    state: &Arc<AppState>,
    queries: &[String],
    limit: u32,
    identity: FolderSearchIdentity,
    deadline: Instant,
) -> FolderSearchOutput {
    let jobs = folder_search_jobs(queries, limit);
    let mut run = search_folder_cache_jobs_until(state, jobs, deadline).await;
    run.completed
        .sort_unstable_by_key(|(position, _)| *position);
    let items = stable_dedupe_folder_items(
        run.completed
            .into_iter()
            .flat_map(|(_, items)| items)
            .collect(),
    );

    FolderSearchOutput {
        results: items
            .into_iter()
            .filter(|item| {
                !item.is_directory
                    && crate::api::search_pipeline::is_media_file(&item.name)
                    && folder_item_matches_identity(item, identity)
            })
            .map(|item| ProviderSearchResult {
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
            })
            .collect(),
        truncated: run.truncated,
    }
}

pub(crate) fn response_from_smart_search(
    response: SmartSearchResponse,
) -> axum::response::Response {
    axum::Json(response).into_response()
}

pub(crate) fn legacy_request_from_intent(intent: &types::SmartSearchIntent) -> SmartSearchRequest {
    intent.legacy_request()
}

pub(crate) fn is_target_episode_request(intent: &types::SmartSearchIntent) -> bool {
    matches!(intent, types::SmartSearchIntent::TargetEpisode { .. })
}

pub(crate) fn mode_kind_for_intent(
    intent: &types::SmartSearchIntent,
) -> (&'static str, Option<&'static str>) {
    intent.mode_kind()
}

pub(crate) fn legacy_contract(contract: types::RequestContract) -> bool {
    matches!(contract, types::RequestContract::Legacy)
}

pub(crate) fn explicit_contract(contract: types::RequestContract) -> bool {
    matches!(contract, types::RequestContract::Explicit)
}

pub(crate) fn intent_is_movie(intent: &types::SmartSearchIntent) -> bool {
    matches!(intent, types::SmartSearchIntent::TargetMovie { .. })
}

pub(crate) fn request_contract(parsed: &types::ParsedSmartSearchRequest) -> types::RequestContract {
    parsed.contract
}

pub(crate) fn request_intent(
    parsed: &types::ParsedSmartSearchRequest,
) -> &types::SmartSearchIntent {
    &parsed.intent
}

pub(crate) fn validation_error_response(
    error: types::SmartSearchValidationError,
) -> axum::response::Response {
    error.into_response()
}

pub(crate) fn folder_cache_limit(req: &SmartSearchRequest) -> u32 {
    folder_cache_search_limit_for_request(req)
}

pub(crate) fn into_legacy_request(
    parsed: types::ParsedSmartSearchRequest,
) -> types::ParsedSmartSearchRequest {
    parsed
}

pub(crate) fn parsed_request_from_value(
    value: serde_json::Value,
) -> Result<types::ParsedSmartSearchRequest, types::SmartSearchValidationError> {
    parse_smart_search_request(value)
}

use serde_json::Value;

fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

use super::search_pipeline::ProviderSearchResult;

fn folder_cache_version(state: &Arc<AppState>) -> String {
    let count = state.db.get_folder_cache_count().unwrap_or(0);
    let last_sync = state
        .db
        .get_folder_cache_meta("last_sync")
        .ok()
        .flatten()
        .unwrap_or_default();
    let sync_status = state
        .db
        .get_folder_cache_meta("sync_status")
        .ok()
        .flatten()
        .unwrap_or_default();
    let generation = state
        .db
        .get_folder_cache_meta("sync_generation")
        .ok()
        .flatten()
        .unwrap_or_default();

    format!(
        "folders:{}:{}:{}:{}",
        count, last_sync, sync_status, generation
    )
}

fn prowlarr_search_version(state: &Arc<AppState>) -> String {
    let enabled = state
        .db
        .get_setting("prowlarr_enabled")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    let url = state
        .db
        .get_setting("prowlarr_url")
        .ok()
        .flatten()
        .unwrap_or_default();
    let has_key = state
        .db
        .get_setting("prowlarr_api_key")
        .ok()
        .flatten()
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let torbox = crate::api::settings::load_torbox_settings_public(state);
    use sha2::{Digest, Sha256};
    let torbox_fingerprint = format!("{:x}", Sha256::digest(torbox.api_key.as_bytes()));
    let addon_generation = state
        .db
        .list_stremio_addons()
        .map(|addons| {
            let config = addons
                .into_iter()
                .map(|addon| {
                    format!(
                        "{}:{}:{}:{}",
                        addon.public.id,
                        addon.public.enabled,
                        addon.public.sort_order,
                        addon.encrypted_config_url
                    )
                })
                .collect::<Vec<_>>()
                .join("|");
            format!("{:x}", Sha256::digest(config.as_bytes()))
        })
        .unwrap_or_default();
    let generation = state
        .db
        .get_setting("torrent_search_generation")
        .ok()
        .flatten()
        .unwrap_or_default();

    format!(
        "torrent:{}:{}:{}|torbox:{}:{}:{}:{}|stremio:{}|query-order:v3|generation:{}",
        enabled,
        url,
        has_key,
        torbox.enabled,
        torbox.base_url,
        torbox_fingerprint,
        torbox.show_cached_as_source,
        addon_generation,
        generation
    )
}

fn result_dedupe_key(result: &ProviderSearchResult) -> String {
    crate::sources::search::CanonicalSearchResult::from(result)
        .validate()
        .map(|canonical| {
            canonical.dedupe_key(
                result.result_source.as_deref().unwrap_or("fshare"),
                result.size,
            )
        })
        .unwrap_or_else(|_| format!("invalid:{}", result.fcode))
}

struct TorrentSearchResults {
    results: Vec<ProviderSearchResult>,
    sources: types::SmartSearchSourceStatus,
}

#[derive(Debug)]
struct StremioSearchOutput {
    rows: Vec<ProviderSearchResult>,
    state: &'static str,
    message: &'static str,
    addons: Vec<types::StremioAddonState>,
    telemetry: types::StremioTelemetry,
}

pub(super) fn sanitized_stremio_addons(
    addons: &[crate::stremio::service::AddonStatus],
) -> Vec<types::StremioAddonState> {
    addons
        .iter()
        .take(64)
        .map(|addon| types::StremioAddonState {
            id: addon.addon_id.chars().take(128).collect(),
            name: addon.addon_name.chars().take(128).collect(),
            health: serde_json::to_value(&addon.health)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_else(|| "degraded".into()),
            result_count: addon.result_count,
            elapsed_ms: addon.elapsed_ms,
            failure_class: (addon.health != crate::stremio::service::AddonHealth::Healthy)
                .then_some(addon.failure_class)
                .flatten(),
        })
        .collect()
}

pub(super) fn stremio_telemetry(
    addons: &[crate::stremio::service::AddonStatus],
) -> types::StremioTelemetry {
    let mut out = types::StremioTelemetry {
        total_calls: addons.len(),
        truncated: addons.len() > 64,
        ..Default::default()
    };
    for addon in addons {
        let health = serde_json::to_value(&addon.health)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        *out.by_health.entry(health).or_default() += 1;
        if let Some(class) = addon.failure_class {
            let class = serde_json::to_value(class)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned();
            *out.by_failure_class.entry(class).or_default() += 1;
        }
    }
    out
}

fn stremio_health_summary(
    result_count: usize,
    addons: &[crate::stremio::service::AddonStatus],
) -> (&'static str, &'static str) {
    use crate::stremio::service::AddonHealth;
    let healthy = addons
        .iter()
        .filter(|addon| addon.health == AddonHealth::Healthy)
        .count();
    let failed = addons.len().saturating_sub(healthy);
    match (healthy, failed, result_count) {
        (0, 0, _) => ("unavailable", "Stremio has no available addons"),
        (0, _, _) => ("unavailable", "Stremio addons unavailable"),
        (_, f, _) if f > 0 => ("partial", "Stremio search partially completed"),
        (_, 0, 0) => ("no_results", "Stremio returned no results"),
        _ => ("ok", "Stremio search completed"),
    }
}

struct TorrentProviderPlan {
    search_prowlarr: bool,
    search_stremio: bool,
    message: &'static str,
}

fn torrent_provider_plan(enabled: bool, has_key: bool, has_imdb: bool) -> TorrentProviderPlan {
    TorrentProviderPlan {
        search_prowlarr: enabled && has_key,
        search_stremio: has_imdb,
        message: match (enabled, has_key, has_imdb) {
            (false, _, true) => "Prowlarr disabled; Stremio searched",
            (true, false, true) => "Prowlarr API key missing; Stremio searched",
            (false, _, false) => "Prowlarr disabled; Stremio unavailable: IMDb ID missing",
            (true, false, false) => {
                "Prowlarr API key missing; Stremio unavailable: IMDb ID missing"
            }
            (true, true, false) => "Prowlarr searched; Stremio unavailable: IMDb ID missing",
            (true, true, true) => "Prowlarr and Stremio searched",
        },
    }
}

fn should_stop_prowlarr_fallback(outcome: &crate::api::search::ProwlarrQueryOutcome) -> bool {
    matches!(outcome, crate::api::search::ProwlarrQueryOutcome::Success(response) if !response.results.is_empty())
}

fn prioritized_prowlarr_queries(
    raw_title: &str,
    canonical_title: &str,
    candidates: &[String],
) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    [raw_title.to_string(), canonical_title.to_string()]
        .into_iter()
        .chain(candidates.iter().cloned())
        .filter_map(|query| {
            let key = query.trim().to_lowercase();
            (!key.is_empty() && seen.insert(key)).then_some(query)
        })
        .take(8)
        .collect()
}

fn source_state(state: &str, message: impl Into<Option<String>>) -> types::SmartSearchSourceState {
    types::SmartSearchSourceState {
        state: state.to_string(),
        message: message.into(),
        addons: None,
        telemetry: None,
    }
}

fn fshare_source_state(result_count: usize, truncated: bool) -> types::SmartSearchSourceState {
    if truncated {
        source_state("partial", Some("Fshare search timed out".to_string()))
    } else if result_count == 0 {
        source_state(
            "unavailable",
            Some("Fshare returned no results".to_string()),
        )
    } else {
        source_state("ok", Some("Fshare sources searched".to_string()))
    }
}

pub(crate) fn torbox_configuration_state(
    enabled: bool,
    show_cached_as_source: bool,
    api_key: &str,
    cached_count: usize,
) -> types::SmartSearchSourceState {
    if !enabled {
        source_state("disabled", Some("TorBox is disabled".to_string()))
    } else if !show_cached_as_source {
        source_state(
            "disabled",
            Some("Cached TorBox results are hidden".to_string()),
        )
    } else if crate::api::settings::validate_api_key(api_key).is_err() {
        source_state("unavailable", Some("TorBox is not configured".to_string()))
    } else {
        source_state(
            "ok",
            Some(format!(
                "{} cached result{}",
                cached_count,
                if cached_count == 1 { "" } else { "s" }
            )),
        )
    }
}

fn torbox_source_state(
    state: &Arc<AppState>,
    cached_count: usize,
) -> types::SmartSearchSourceState {
    let torbox = crate::api::settings::load_torbox_settings_public(state);
    torbox_configuration_state(
        torbox.enabled,
        torbox.show_cached_as_source,
        &torbox.api_key,
        cached_count,
    )
}

fn source_states(
    fshare: types::SmartSearchSourceState,
    torrent: Option<types::SmartSearchSourceState>,
    stremio: Option<types::SmartSearchSourceState>,
    torbox: types::SmartSearchSourceState,
) -> types::SmartSearchSourceStatus {
    let mut sources = types::SmartSearchSourceStatus::from([
        (crate::sources::model::SourceKind::Fshare, fshare),
        (crate::sources::model::SourceKind::Torbox, torbox),
    ]);
    if let Some(torrent) = torrent {
        sources.insert(crate::sources::model::SourceKind::Torrent, torrent);
    }
    if let Some(stremio) = stremio {
        sources.insert(crate::sources::model::SourceKind::Stremio, stremio);
    }
    sources
}

fn should_cache_smart_response(response: &SmartSearchResponse) -> bool {
    response.total_found > 0
        && response.sources.as_ref().is_none_or(|sources| {
            sources
                .values()
                .all(|source| !matches!(source.state.as_str(), "unavailable" | "partial"))
        })
}

async fn search_torrents_for_queries(
    state: &Arc<AppState>,
    intent: crate::api::search::TorrentSearchIntent,
    raw_title: &str,
    canonical_title: &str,
    candidates: &[String],
    limit: usize,
    stremio_query: Option<crate::stremio::service::StremioQuery>,
) -> TorrentSearchResults {
    search_torrents_for_queries_until(
        state,
        intent,
        raw_title,
        canonical_title,
        candidates,
        limit,
        stremio_query,
        tokio::time::Instant::now() + std::time::Duration::from_secs(30),
    )
    .await
}

async fn search_torrents_for_queries_until(
    state: &Arc<AppState>,
    intent: crate::api::search::TorrentSearchIntent,
    raw_title: &str,
    canonical_title: &str,
    candidates: &[String],
    limit: usize,
    stremio_query: Option<crate::stremio::service::StremioQuery>,
    deadline: tokio::time::Instant,
) -> TorrentSearchResults {
    let enabled = state
        .db
        .get_setting("prowlarr_enabled")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    let has_key = state
        .db
        .get_setting("prowlarr_api_key")
        .ok()
        .flatten()
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let plan = torrent_provider_plan(enabled, has_key, stremio_query.is_some());
    let bounded_queries = prioritized_prowlarr_queries(raw_title, canonical_title, candidates);
    let prowlarr_deadline =
        deadline.min(tokio::time::Instant::now() + std::time::Duration::from_secs(20));
    let stremio_started = std::time::Instant::now();
    let stremio_future = search_stremio(state, stremio_query, deadline);
    let prowlarr_future = async {
        if plan.search_prowlarr {
            Some(
                crate::api::search::search_prowlarr_intent_until(
                    state,
                    intent.clone(),
                    raw_title.to_string(),
                    bounded_queries,
                    limit,
                    prowlarr_deadline,
                )
                .await,
            )
        } else {
            None
        }
    };
    let (stremio, run) = tokio::join!(stremio_future, prowlarr_future);
    tracing::info!(
        phase = "stremio",
        elapsed_ms = stremio_started.elapsed().as_millis() as u64,
        "Smart Search phase timing"
    );
    let stremio_state = plan.search_stremio.then(|| types::SmartSearchSourceState {
        telemetry: Some(stremio.telemetry.clone().into()),
        addons: Some(stremio.addons.clone()),
        ..source_state(stremio.state, Some(stremio.message.to_string()))
    });
    let Some(run) = run else {
        info!("{}", plan.message);
        return TorrentSearchResults {
            results: stremio.rows,
            sources: source_states(
                fshare_source_state(0, false),
                None,
                stremio_state,
                torbox_source_state(state, 0),
            ),
        };
    };
    tracing::info!(
        phase = "prowlarr_discovery",
        elapsed_ms = run.discovery_elapsed.as_millis() as u64,
        "Smart Search phase timing"
    );
    tracing::info!(
        phase = "hash_resolution",
        elapsed_ms = run.hash_elapsed.as_millis() as u64,
        "Smart Search phase timing"
    );
    let attempted = run.attempted;
    let planned = run.planned;
    let truncated = run.truncated;
    let hash_counts = run.hashes;
    let batches = run.outcomes;
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut timed_out = 0usize;
    let mut parsed_count = 0usize;
    let mut discarded = 0usize;
    let mut unexpected_shape = 0usize;
    let mut parsed_results = Vec::new();
    for batch in batches {
        match batch {
            crate::api::search::ProwlarrQueryOutcome::Success(response) => {
                succeeded += 1;
                parsed_count += response.results.len();
                discarded += response.discarded_count;
                unexpected_shape += usize::from(response.unexpected_shape);
                parsed_results.extend(response.results);
            }
            crate::api::search::ProwlarrQueryOutcome::Failed => failed += 1,
            crate::api::search::ProwlarrQueryOutcome::TimedOut => timed_out += 1,
        }
    }

    let inspection_intent =
        (!matches!(intent, crate::api::search::TorrentSearchIntent::FreeText)).then_some(&intent);
    let inspection_started = std::time::Instant::now();
    let inspection = crate::api::search::inspect_prowlarr_results_until(
        state,
        &mut parsed_results,
        inspection_intent,
        deadline,
    )
    .await;
    tracing::info!(
        phase = "inspection",
        elapsed_ms = inspection_started.elapsed().as_millis() as u64,
        "Smart Search phase timing"
    );
    info!(
        selected = inspection.selected,
        inspected = inspection.inspected,
        failed = inspection.failed,
        cache_hits = inspection.cache_hits,
        unavailable = inspection.unavailable,
        download_ref_unavailable = inspection.download_ref_unavailable,
        locator_invalid = inspection.locator_invalid,
        fetch_timeout = inspection.fetch_timeout,
        fetch_network = inspection.fetch_network,
        status_4xx = inspection.status_4xx,
        status_5xx = inspection.status_5xx,
        redirect = inspection.redirect,
        cross_origin_non_https = inspection.cross_origin_non_https,
        target_private_ip = inspection.target_private_ip,
        target_userinfo = inspection.target_userinfo,
        dns_resolution_failed = inspection.dns_resolution_failed,
        redirect_limit = inspection.redirect_limit,
        strict_same_origin = inspection.strict_same_origin,
        location_missing_or_invalid = inspection.location_missing_or_invalid,
        location_join_failed = inspection.location_join_failed,
        target_magnet = inspection.target_magnet,
        target_scheme_other = inspection.target_scheme_other,
        target_host_missing = inspection.target_host_missing,
        response_type = inspection.response_type,
        too_large = inspection.too_large,
        metainfo_invalid = inspection.metainfo_invalid,
        other = inspection.other,
        "torrent inspection summary"
    );
    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();
    for item in parsed_results {
        let inspection_score =
            crate::api::search::inspection_score(&intent, item.inspection.as_ref());
        let mut result = ProviderSearchResult {
            name: item.name,
            url: item.url,
            fcode: item.info_hash.clone().unwrap_or(item.id),
            size: item.size,
            score: inspection_score,
            result_source: Some("torrent".to_string()),
            provider: Some(item.provider),
            indexer: item.indexer,
            seeders: item.seeders,
            leechers: item.leechers,
            info_hash: item.info_hash,
            download_ref: item.download_ref,
            origin_provider: None,
            origin_indexer: None,
            cached: None,
            file_idx: item.file_idx,
            sources: item.sources,
            behavior_hints: item.behavior_hints,
            inspection: item.inspection,
            inspection_score: Some(inspection_score),
        };
        if result.info_hash.is_none() && result.fcode.starts_with("magnet:") {
            result.info_hash = result
                .fcode
                .split("btih:")
                .nth(1)
                .and_then(|tail| tail.split('&').next())
                .map(|hash| hash.to_ascii_lowercase());
        }
        if seen.insert(result_dedupe_key(&result)) {
            results.push(result);
        }
    }
    // Prefer Prowlarr rows for the same torrent identity because they carry an
    // acquirable download_ref. Stremio remains an independent discovery source.
    for result in stremio.rows {
        if seen.insert(result_dedupe_key(&result)) {
            results.push(result);
        }
    }
    info!(
        "Prowlarr smart-search mapped {} unique torrent results",
        results.len()
    );

    let torbox_state = torbox_source_state(state, 0);

    let torrent_count = results.len();
    let torrent_state = if succeeded == 0 {
        "unavailable"
    } else if failed + timed_out > 0 {
        "partial"
    } else {
        "ok"
    };
    let prowlarr_telemetry = types::ProwlarrTelemetry {
        planned,
        attempted,
        succeeded,
        failed,
        timed_out,
        truncated,
        hash_resolved: hash_counts.resolved,
        hash_cache_hits: hash_counts.cache_hits,
        hash_omitted: hash_counts.omitted,
        inspections_selected: inspection.selected,
        inspections_completed: inspection.inspected,
        inspections_timed_out: inspection.timed_out,
    };
    let torrent_message = if succeeded == 0 {
        format!(
            "Prowlarr did not complete {} request{}{}",
            attempted,
            if attempted == 1 { "" } else { "s" },
            if timed_out > 0 { " (timeout)" } else { "" }
        )
    } else {
        format!(
            "{} result{} from {}/{} request{}; {} parsed{}{}",
            torrent_count,
            if torrent_count == 1 { "" } else { "s" },
            succeeded,
            attempted,
            if attempted == 1 { "" } else { "s" },
            parsed_count,
            if discarded > 0 {
                format!(", {} discarded without download links", discarded)
            } else {
                String::new()
            },
            if unexpected_shape > 0 {
                "; unexpected response shape"
            } else if failed + timed_out > 0 {
                "; some failed"
            } else {
                ""
            }
        )
    };
    TorrentSearchResults {
        results,
        sources: source_states(
            fshare_source_state(0, false),
            Some(types::SmartSearchSourceState {
                telemetry: Some(types::SourceTelemetry::Prowlarr(prowlarr_telemetry)),
                ..source_state(torrent_state, Some(torrent_message))
            }),
            stremio_state,
            torbox_state,
        ),
    }
}

async fn search_stremio(
    state: &Arc<AppState>,
    query: Option<crate::stremio::service::StremioQuery>,
    deadline: tokio::time::Instant,
) -> StremioSearchOutput {
    let Some(query) = query else {
        return StremioSearchOutput {
            rows: Vec::new(),
            state: "unavailable",
            message: "Stremio identity unavailable",
            addons: Vec::new(),
            telemetry: Default::default(),
        };
    };
    let Ok(registry) = crate::stremio::AddonRegistry::load_or_create() else {
        return StremioSearchOutput {
            rows: Vec::new(),
            state: "unavailable",
            message: "Stremio registry unavailable",
            addons: Vec::new(),
            telemetry: Default::default(),
        };
    };
    let Ok(service) = crate::stremio::service::StremioService::new(&state.db, &registry) else {
        return StremioSearchOutput {
            rows: Vec::new(),
            state: "unavailable",
            message: "Stremio service unavailable",
            addons: Vec::new(),
            telemetry: Default::default(),
        };
    };
    let Ok(output) = service.search_until(query, deadline).await else {
        return StremioSearchOutput {
            rows: Vec::new(),
            state: "unavailable",
            message: "Stremio search unavailable",
            addons: Vec::new(),
            telemetry: Default::default(),
        };
    };
    let summary = stremio_health_summary(output.results.len(), &output.addons);
    let telemetry = stremio_telemetry(&output.addons);
    let addons = sanitized_stremio_addons(&output.addons);
    let rows = output
        .results
        .into_iter()
        .map(|item| {
            let filename = item
                .stream
                .behavior_hints
                .filename
                .clone()
                .unwrap_or_else(|| "Stremio stream".into());
            ProviderSearchResult {
                name: filename,
                url: String::new(),
                fcode: item.stream.info_hash.clone(),
                size: item.stream.behavior_hints.video_size.unwrap_or(0),
                score: 0,
                result_source: Some("torrent".into()),
                provider: Some("stremio".into()),
                indexer: Some(item.addon_name.clone()),
                seeders: None,
                leechers: None,
                info_hash: Some(item.stream.info_hash),
                download_ref: None,
                origin_provider: Some("stremio".into()),
                origin_indexer: Some(item.addon_name),
                cached: None,
                file_idx: item.stream.file_idx,
                sources: None,
                behavior_hints: Some(crate::utils::title_matcher::StreamBehaviorHints {
                    filename: item.stream.behavior_hints.filename,
                    binge_group: item.stream.behavior_hints.binge_group,
                    video_size: item.stream.behavior_hints.video_size,
                }),
                inspection: None,
                inspection_score: None,
            }
        })
        .collect();
    StremioSearchOutput {
        rows,
        state: summary.0,
        message: summary.1,
        addons,
        telemetry,
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SeasonGroup {
    pub season: u32,
    pub episode_count: u32, // TMDB official episode count for uncut detection
    pub aired_episode_count: u32, // Number of episodes that have actually aired
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<TorrentPackage>,
    pub episodes_grouped: Vec<EpisodeGroup>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TorrentPackage {
    pub info_hash: String,
    pub result: SmartSearchResult,
    pub covered_episodes: Vec<u32>,
    pub file_indices: std::collections::BTreeMap<u32, u32>,
    pub episode_files: std::collections::BTreeMap<u32, SmartSearchResult>,
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
    Router::new().route("/", post(smart_search))
}

fn smart_result_order(a: &SmartSearchResult, b: &SmartSearchResult) -> std::cmp::Ordering {
    let priority = |match_type: &str| match match_type {
        "alias" => 0,
        "exact" => 1,
        "fuzzy" | "keyword_overlap" | "tv_match" => 2,
        _ => 3,
    };
    b.inspection_score
        .unwrap_or(0)
        .cmp(&a.inspection_score.unwrap_or(0))
        .then_with(|| priority(&a.match_type).cmp(&priority(&b.match_type)))
        .then_with(|| b.seeders.unwrap_or(0).cmp(&a.seeders.unwrap_or(0)))
        .then_with(|| b.total_score.cmp(&a.total_score))
        .then_with(|| a.name.cmp(&b.name))
        .then_with(|| a.info_hash.cmp(&b.info_hash))
}

pub async fn smart_search(
    State(state): State<Arc<AppState>>,
    Json(value): Json<Value>,
) -> impl IntoResponse {
    match parse_smart_search_request(value) {
        Ok(parsed) => orchestrator::execute(state, parsed).await,
        Err(error) => error.into_response(),
    }
}

pub async fn handle_movie_search(
    state: Arc<AppState>,
    req: SmartSearchRequest,
) -> axum::response::Response {
    Json(
        handle_movie_search_until(
            state,
            req,
            tokio::time::Instant::now() + std::time::Duration::from_secs(30),
            true,
        )
        .await,
    )
    .into_response()
}

pub(crate) async fn handle_movie_search_until(
    state: Arc<AppState>,
    req: SmartSearchRequest,
    deadline: tokio::time::Instant,
    cache_eligible: bool,
) -> SmartSearchResponse {
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
    let cache_key = format!(
        "movie|{}|{:?}|{:?}|{:?}|{:?}|{}|{}",
        req.title,
        req.year,
        req.tmdb_id,
        req.season,
        req.episode,
        folder_cache_version(&state),
        prowlarr_search_version(&state)
    );
    if let Some(cached) = state.search_cache.get(&cache_key).await {
        info!("Search Cache HIT: {}", cache_key);
        return cached;
    }

    let start_time = std::time::Instant::now();

    // ── 1. TMDB ENRICHMENT ──────────────────────────────────────────────────
    let tmdb_id_str = match req.tmdb_id.clone() {
        Some(Value::String(s)) => Some(s),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    };

    let fallback_enrichment = || TmdbEnrichmentCache {
        official: Some(title.clone()),
        original_name: None,
        all_aliases: Vec::new(),
        vn_titles: Vec::new(),
        original_lang_titles: Vec::new(),
        us_titles: Vec::new(),
        poster: None,
        year: year_str.as_ref().and_then(|y| y.parse::<u32>().ok()),
        collections: Vec::new(),
    };
    let enrichment_cache: TmdbEnrichmentCache = if let Some(ref tmdb_id) = tmdb_id_str {
        if let Some(cached) = state.tmdb_cache.get(tmdb_id).await {
            cached
        } else {
            let tmdb_id_num: i64 = tmdb_id.parse().unwrap_or(0);
            match tokio::time::timeout_at(
                deadline,
                state.tmdb_service.get_movie_enrichment(tmdb_id_num),
            )
            .await
            {
                Ok(enrichment) => {
                    let cache_entry = build_movie_enrichment_cache(enrichment, &title);
                    state
                        .tmdb_cache
                        .insert(tmdb_id.clone(), cache_entry.clone())
                        .await;
                    cache_entry
                }
                Err(_) => fallback_enrichment(),
            }
        }
    } else {
        fallback_enrichment()
    };

    let official_name = enrichment_cache
        .official
        .clone()
        .unwrap_or_else(|| title.clone());
    let aliases = enrichment_cache.all_aliases.clone();
    let base_poster = enrichment_cache.poster.clone();
    let _collections = enrichment_cache.collections.clone();
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

    let limit = adaptive_limit(all_queries.len(), SearchPhase::PrimaryMovie);
    let folder_future = search_folder_cache_with_limit(
        &state,
        &all_queries,
        folder_cache_limit(&req),
        FolderSearchIdentity {
            tmdb_id: tmdb_id_str.as_deref().and_then(|id| id.parse().ok()),
            media_type: Some("movie"),
            year: year_str.as_deref().and_then(|year| year.parse().ok()),
            season: req.season,
            episode: req.episode,
        },
        deadline,
    );
    let imdb_id = match tmdb_id_str.as_deref().and_then(|id| id.parse::<i64>().ok()) {
        Some(id) => {
            tokio::time::timeout_at(deadline, state.tmdb_service.get_movie_external_ids(id))
                .await
                .ok()
                .and_then(|ids| ids.imdb_id)
        }
        None => None,
    };
    let stremio_query = imdb_id
        .clone()
        .map(|imdb| crate::stremio::service::StremioQuery::Movie { imdb });
    let torrent_future = search_torrents_for_queries_until(
        &state,
        crate::api::search::TorrentSearchIntent::Movie {
            tmdb_id: tmdb_id_str.clone(),
            imdb_id,
        },
        &title,
        &official_name,
        &all_queries,
        limit,
        stremio_query,
        deadline,
    );
    let (folder_results, mut torrent_search) = tokio::join!(folder_future, torrent_future);

    info!(
        "Folder cache returned {} matches",
        folder_results.results.len()
    );
    torrent_search.sources.insert(
        crate::sources::model::SourceKind::Fshare,
        fshare_source_state(folder_results.results.len(), folder_results.truncated),
    );

    // Folder-cache Fshare results followed by unchanged torrent/TorBox results.
    let mut results_pool: Vec<ProviderSearchResult> = Vec::new();
    let mut seen_results: std::collections::HashSet<String> = std::collections::HashSet::new();
    for r in folder_results.results {
        if seen_results.insert(result_dedupe_key(&r)) {
            results_pool.push(r);
        }
    }
    for r in torrent_search.results {
        if seen_results.insert(result_dedupe_key(&r)) {
            results_pool.push(r);
        }
    }
    results_pool.retain(|result| {
        crate::api::search_pipeline::is_safe_search_result(&result.name)
            && (result
                .result_source
                .as_deref()
                .is_some_and(|source| source != "fshare")
                || crate::api::search_pipeline::is_media_file(&result.name))
    });
    results_pool.sort_by(|a, b| {
        let a_parsed = smart_parse(&a.name);
        let b_parsed = smart_parse(&b.name);
        let target_year = year_str.as_ref().and_then(|y| y.parse::<u32>().ok());
        let a_score = calculate_match_score(
            &official_name,
            &a.name,
            a_parsed.year,
            target_year,
            &aliases,
            false,
        );
        let b_score = calculate_match_score(
            &official_name,
            &b.name,
            b_parsed.year,
            target_year,
            &aliases,
            false,
        );

        b_score
            .partial_cmp(&a_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b_parsed.total_score().cmp(&a_parsed.total_score()))
    });
    let target_results = results_pool.into_iter().take(100).collect::<Vec<_>>();

    // 4. PARSE AND MAP TOP 100 AFTER MATCH SCORING
    let mut valid_results = Vec::new();
    static RE_YEAR: OnceLock<Regex> = OnceLock::new();
    let _year_re = RE_YEAR.get_or_init(|| Regex::new(r"\b(19|20)\d{2}\b").unwrap());

    for r in target_results {
        let parsed = smart_parse(&r.name);

        let final_id = base_tmdb_id;
        let final_poster = base_poster.clone();

        // Use unified scorer for primary match (movies: 70% title + 20% year)
        let primary_score = calculate_match_score(
            &official_name,
            &r.name,
            parsed.year,
            year_str.as_ref().and_then(|y| y.parse::<u32>().ok()),
            &aliases,
            false, // is_tv_series = false for movies
        );

        let best_score = primary_score;
        let title_similarity = calculate_unified_similarity(&official_name, &r.name, &aliases);

        // FILTER: keep strong matches; for weaker matches, drop clear episodic TV names.
        let lower_name = r.name.to_lowercase();
        let looks_tv_episode = lower_name.contains("s01e")
            || lower_name.contains("s02e")
            || lower_name.contains("s03e")
            || lower_name.contains(" season ")
            || lower_name.contains(" episode ");

        if !title_similarity.is_valid
            || !is_valid_match(best_score, false)
            || !is_movie_year_match(
                parsed.year,
                year_str.as_ref().and_then(|year| year.parse().ok()),
            )
            || (looks_tv_episode && best_score < 0.80)
        {
            continue;
        }

        // PHASE 3: Detect badges
        let (vietdub, vietsub, hdr, dolby_vision) = detect_badges(&r.name);

        valid_results.push(SmartSearchResult {
            name: r.name.clone(),
            url: r.url,
            size: r.size,
            score: (best_score * 100.0).round() as i32,
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
                is_hd: parsed
                    .resolution
                    .as_ref()
                    .map(|r| {
                        r.contains("720")
                            || r.contains("1080")
                            || r.contains("2160")
                            || r.contains("4K")
                    })
                    .unwrap_or(false),
            },
            tmdb_id: final_id,
            poster_path: final_poster,
            vietdub,
            vietsub,
            hdr,
            dolby_vision,
            result_source: r.result_source,
            provider: r.provider,
            indexer: r.indexer,
            seeders: r.seeders,
            leechers: r.leechers,
            info_hash: r.info_hash,
            download_ref: r.download_ref,
            origin_provider: r.origin_provider,
            origin_indexer: r.origin_indexer,
            cached: r.cached,
            file_idx: r.file_idx,
            sources: r.sources,
            behavior_hints: r.behavior_hints,
            inspection: r.inspection,
            inspection_score: r.inspection_score,
        });
    }

    // PHASE 2.1: Sort by match_type first (alias > exact > fuzzy), then by quality
    valid_results.sort_by(smart_result_order);

    let groups = group_by_quality(valid_results);
    info!(
        "Total Optimized Smart Search took: {:?}",
        start_time.elapsed()
    );

    let response = SmartSearchResponse {
        query: title,
        total_found: groups.iter().map(|g| g.count).sum(),
        r#type: "movie".to_string(),
        mode: None,
        kind: None,
        groups: Some(groups),
        seasons: None,
        sources: Some(torrent_search.sources),
        season_plan: None,
    };

    if cache_eligible && should_cache_smart_response(&response) {
        state.search_cache.insert(cache_key, response.clone()).await;
    }

    info!(
        "Total Optimized Movie Smart Search took: {:?}",
        start_time.elapsed()
    );
    response
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EpisodePosition {
    season: u32,
    episode: u32,
    absolute: u32,
}

#[derive(Debug, Clone)]
struct TvdbEpisodeIndex {
    positions: Vec<EpisodePosition>,
}

impl TvdbEpisodeIndex {
    fn from_pairs<I>(pairs: I) -> Option<Self>
    where
        I: IntoIterator<Item = (u32, u32)>,
    {
        let mut pairs: Vec<(u32, u32)> = pairs
            .into_iter()
            .filter(|(season, episode)| *season > 0 && *episode > 0)
            .collect();
        pairs.sort_unstable();
        pairs.dedup();
        if pairs.is_empty() {
            return None;
        }
        let positions = pairs
            .into_iter()
            .enumerate()
            .map(|(idx, (season, episode))| EpisodePosition {
                season,
                episode,
                absolute: idx as u32 + 1,
            })
            .collect();
        Some(Self { positions })
    }

    fn from_tvdb_episodes(episodes: &[Value]) -> Option<Self> {
        Self::from_pairs(episodes.iter().filter_map(|ep| {
            Some((
                episode_u32(ep, &["seasonNumber", "season_number"]),
                episode_u32(ep, &["number", "episodeNumber", "episode_number"]),
            ))
            .and_then(|(season, episode)| Some((season?, episode?)))
        }))
    }

    fn absolute_to_episode(&self, absolute: u32) -> Option<EpisodePosition> {
        self.positions
            .iter()
            .copied()
            .find(|pos| pos.absolute == absolute)
    }

    fn episode_to_absolute(&self, season: u32, episode: u32) -> Option<u32> {
        self.positions
            .iter()
            .find(|pos| pos.season == season && pos.episode == episode)
            .map(|pos| pos.absolute)
    }

    fn absolutes_for_season(&self, season: u32) -> Vec<u32> {
        self.positions
            .iter()
            .filter(|pos| pos.season == season)
            .map(|pos| pos.absolute)
            .collect()
    }

    fn episode_counts(&self) -> std::collections::HashMap<u32, u32> {
        let mut counts = std::collections::HashMap::new();
        for pos in &self.positions {
            *counts.entry(pos.season).or_insert(0) += 1;
        }
        counts
    }
}

fn episode_u32(ep: &Value, keys: &[&str]) -> Option<u32> {
    keys.iter()
        .find_map(|key| ep.get(*key).and_then(|v| v.as_u64()).map(|v| v as u32))
}

async fn tvdb_episode_index(
    state: &Arc<AppState>,
    tvdb_id: Option<&str>,
    deadline: tokio::time::Instant,
) -> Option<TvdbEpisodeIndex> {
    let tvdb_id = tvdb_id.and_then(|id| id.parse::<i64>().ok())?;
    let episodes = tokio::time::timeout_at(
        deadline,
        state.tvdb_service.get_series_episodes_default(tvdb_id),
    )
    .await
    .ok()??;
    TvdbEpisodeIndex::from_tvdb_episodes(&episodes)
}

fn map_absolute_episode(
    tmdb_id: Option<u64>,
    tvdb_index: Option<&TvdbEpisodeIndex>,
    parsed_season: Option<u32>,
    parsed_episode: Option<u32>,
) -> (Option<u32>, Option<u32>) {
    if let Some(abs) = parsed_episode {
        if parsed_season.is_none() || parsed_season == Some(1) {
            if let Some(pos) = tvdb_index.and_then(|index| index.absolute_to_episode(abs)) {
                return (Some(pos.season), Some(pos.episode));
            }
            let _ = tmdb_id;
        }
    }
    (parsed_season, parsed_episode)
}

fn is_junk_tv_file(name: &str, episode: Option<u32>) -> bool {
    let lower = name.to_lowercase();
    episode == Some(0)
        || lower.ends_with(".rar")
        || lower.ends_with(".zip")
        || lower.contains("one-handed")
}

fn is_movie_year_match(file_year: Option<u32>, target_year: Option<u32>) -> bool {
    file_year
        .zip(target_year)
        .is_none_or(|(file, target)| file.abs_diff(target) <= 1)
}

async fn sonarr_tv_episode_index(
    state: &Arc<AppState>,
    tvdb_id: Option<&str>,
    deadline: tokio::time::Instant,
) -> Option<TvdbEpisodeIndex> {
    let tvdb_id = tvdb_id.and_then(|id| id.parse::<i32>().ok())?;
    tokio::time::timeout_at(deadline, async {
        let client = state.download_orchestrator.get_arr_client().await?;
        let series = client.get_all_series().await.ok()?;
        let series = series.into_iter().find(|s| s.tvdb_id == Some(tvdb_id))?;
        let episodes = client.get_episodes(series.id).await.ok()?;
        TvdbEpisodeIndex::from_pairs(episodes.into_iter().map(|ep| {
            (
                ep.season_number.max(0) as u32,
                ep.episode_number.max(0) as u32,
            )
        }))
    })
    .await
    .ok()
    .flatten()
}

pub async fn handle_tv_search(
    state: Arc<AppState>,
    req: SmartSearchRequest,
) -> axum::response::Response {
    Json(
        handle_tv_search_until(
            state,
            req,
            tokio::time::Instant::now() + std::time::Duration::from_secs(30),
            true,
        )
        .await,
    )
    .into_response()
}

pub(crate) async fn handle_tv_search_until(
    state: Arc<AppState>,
    req: SmartSearchRequest,
    deadline: tokio::time::Instant,
    cache_eligible: bool,
) -> SmartSearchResponse {
    let title = req.title.clone();
    let season = req.season;
    let episode = req.episode;
    let year_str = match req.year {
        Some(Value::String(ref s)) => Some(s.clone()),
        Some(Value::Number(ref n)) => Some(n.to_string()),
        _ => None,
    };

    let start_time = std::time::Instant::now();

    // Resolve canonical TV identity before cache keys. TVDB is canonical; TMDB is
    // a legacy bridge only. The raw request title remains a query/scoring alias.
    let mut tmdb_id_str = match req.tmdb_id {
        Some(Value::String(ref s)) => Some(s.to_string()),
        Some(Value::Number(ref n)) => Some(n.to_string()),
        _ => None,
    };
    if tmdb_id_str.is_none() {
        tmdb_id_str = tokio::time::timeout_at(deadline, state.tmdb_service.resolve_tv_title_id(&title))
            .await
            .ok()
            .flatten()
            .map(|id| id.to_string());
    }
    let mut tvdb_id_str = match req.tvdb_id {
        Some(Value::String(ref s)) => Some(s.to_string()),
        Some(Value::Number(ref n)) => Some(n.to_string()),
        _ => None,
    };
    if tvdb_id_str.is_none() {
        if let Some(ref tmdb_id) = tmdb_id_str {
            if let Ok(tmdb_id_num) = tmdb_id.parse::<i64>() {
                tvdb_id_str = tokio::time::timeout_at(
                    deadline,
                    state.tmdb_service.get_tv_external_ids(tmdb_id_num),
                )
                .await
                .ok()
                .and_then(|ids| ids.tvdb_id)
                .map(|id| id.to_string());
            }
        }
    }

    let canonical_key = tvdb_id_str
        .as_ref()
        .map(|id| format!("tvdb:{}", id))
        .or_else(|| tmdb_id_str.as_ref().map(|id| format!("legacy-tmdb:{}", id)))
        .unwrap_or_else(|| format!("title:{}", title.to_lowercase()));
    let cache_key = format!(
        "tv|{}|title:{}|year:{:?}|season:{:?}|episode:{:?}|episode-map:v2|{}|{}",
        canonical_key,
        req.title.to_lowercase(),
        req.year,
        req.season,
        req.episode,
        folder_cache_version(&state),
        prowlarr_search_version(&state)
    );
    if let Some(cached) = state.search_cache.get(&cache_key).await {
        info!("Search Cache HIT: {}", cache_key);
        return cached;
    }

    let tvdb_episode_index =
        match tvdb_episode_index(&state, tvdb_id_str.as_deref(), deadline).await {
            Some(index) => Some(index),
            None => sonarr_tv_episode_index(&state, tvdb_id_str.as_deref(), deadline).await,
        };
    let canonical_episode_counts = tvdb_episode_index
        .as_ref()
        .map(TvdbEpisodeIndex::episode_counts)
        .unwrap_or_default();

    info!(
        "📊 [PERF] TV Smart Search START: title='{}', tmdbId={:?}, tvdbId={:?}, year={:?}",
        title, tmdb_id_str, tvdb_id_str, year_str
    );

    // ── 1. TVDB PRIMARY ENRICHMENT; TMDB FALLBACK ─────────────────────────────
    let phase1_start = std::time::Instant::now();

    let cache_id = tvdb_id_str
        .as_ref()
        .map(|id| format!("tvdb:{}", id))
        .or_else(|| tmdb_id_str.as_ref().map(|id| format!("tmdb:{}", id)));

    let enrichment_cache: TmdbEnrichmentCache = if let Some(ref cache_id) = cache_id {
        if let Some(cached) = state.tmdb_cache.get(cache_id).await {
            cached
        } else {
            let mut cache_entry = None;
            if let Some(ref tvdb_id) = tvdb_id_str {
                if let Ok(tvdb_id_num) = tvdb_id.parse::<i64>() {
                    info!("Enriching TV metadata from TVDB: tvdb_id={}", tvdb_id_num);
                    if let Ok(enrichment) = tokio::time::timeout_at(
                        deadline,
                        state.tvdb_service.get_tv_enrichment(tvdb_id_num),
                    )
                    .await
                    {
                        let built = build_tvdb_enrichment_cache(enrichment, &title);
                        if built.official.is_some() || !built.all_aliases.is_empty() {
                            cache_entry = Some(built);
                        }
                    }
                }
            }
            if let Some(ref tmdb_id) = tmdb_id_str {
                let tmdb_id_num: i64 = tmdb_id.parse().unwrap_or(0);
                if let Ok(enrichment) = tokio::time::timeout_at(
                    deadline,
                    state.tmdb_service.get_tv_enrichment(tmdb_id_num),
                )
                .await
                {
                    let tmdb_entry = build_tv_enrichment_cache(enrichment, &title);
                    cache_entry = Some(match cache_entry {
                        Some(tvdb_entry) => {
                            merge_tvdb_primary_with_tmdb_fallback(tvdb_entry, tmdb_entry)
                        }
                        None => {
                            info!(
                                "Enriching TV metadata from TMDB fallback: tmdb_id={}",
                                tmdb_id_num
                            );
                            tmdb_entry
                        }
                    });
                }
            }
            let cache_entry = cache_entry.unwrap_or_else(|| TmdbEnrichmentCache {
                official: Some(title.clone()),
                original_name: None,
                all_aliases: Vec::new(),
                vn_titles: Vec::new(),
                original_lang_titles: Vec::new(),
                us_titles: Vec::new(),
                poster: None,
                year: year_str.as_ref().and_then(|y| y.parse::<u32>().ok()),
                collections: Vec::new(),
            });
            info!(
                "TV enrichment built — official={:?}, vn={:?}, orig_lang={:?}, us={:?}",
                cache_entry.official,
                cache_entry.vn_titles,
                cache_entry.original_lang_titles,
                cache_entry.us_titles
            );
            state
                .tmdb_cache
                .insert(cache_id.clone(), cache_entry.clone())
                .await;
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
            year: year_str.as_ref().and_then(|y| y.parse::<u32>().ok()),
            collections: Vec::new(),
        }
    };

    let official_name = enrichment_cache
        .official
        .clone()
        .unwrap_or_else(|| title.clone());
    let aliases = enrichment_cache.all_aliases.clone();
    let base_poster = enrichment_cache.poster.clone();
    let base_tmdb_id = tmdb_id_str.as_ref().and_then(|s| s.parse::<u64>().ok());
    let effective_year_str = year_str
        .clone()
        .or_else(|| enrichment_cache.year.map(|y| y.to_string()));

    // ── 2. BUILD ALL FSHARE QUERIES ─────────────────────────────────────────
    // Collect unique queries across all title categories, then fire them all at once.
    let core_title = extract_core_title(&title);
    let query_keyword = if let Some(ref y) = effective_year_str {
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

    // Always include no-year title variants. Provider years/official titles can be
    // wrong for Fshare naming, while UI/request title or aliases often match files.
    let mut add_tv_title_queries = |raw_title: &str| {
        let core = extract_core_title(raw_title);
        if core.is_empty() {
            return;
        }
        if let (Some(s), Some(e)) = (season, episode) {
            add_query!(format!("{} S{:02}E{:02}", core, s, e));
            add_query!(format!("{} E{:02}", core, e));
            if let Some(abs) = tvdb_episode_index
                .as_ref()
                .and_then(|index| index.episode_to_absolute(s, e))
            {
                add_query!(format!("{} E{:02}", core, abs));
            }
        } else if let Some(s) = season {
            add_query!(format!("{} Season {}", core, s));
            add_query!(format!("{} S{:02}", core, s));
        } else {
            add_query!(core);
        }
    };
    add_tv_title_queries(&title);
    add_tv_title_queries(&official_name);
    for alias in &aliases {
        add_tv_title_queries(alias);
    }

    if season.is_some() && episode.is_none() {
        if let Some(s) = season {
            add_query!(format!("{} S{:02}", query_keyword, s));
            if let Some(index) = tvdb_episode_index.as_ref() {
                for abs in index.absolutes_for_season(s) {
                    add_query!(format!("{} E{:02}", query_keyword, abs));
                    add_query!(format!("{} E{:02}", extract_core_title(&title), abs));
                }
            }
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

    info!(
        "📊 [PERF] Phase 1 (TMDB enrichment) took: {:?}",
        phase1_start.elapsed()
    );
    info!(
        "TV search queries (parallel x{}): {:?}",
        all_queries.len(),
        all_queries
    );

    // ── 3. FOLDER-CACHE FSHARE SEARCH ───────────────────────────────────────
    let limit = adaptive_limit(all_queries.len(), SearchPhase::PrimaryTv);
    let folder_future = search_folder_cache_with_limit(
        &state,
        &all_queries,
        folder_cache_limit(&req),
        FolderSearchIdentity {
            tmdb_id: tmdb_id_str.as_deref().and_then(|id| id.parse().ok()),
            media_type: Some("tv"),
            year: effective_year_str
                .as_deref()
                .and_then(|year| year.parse().ok()),
            season,
            episode,
        },
        deadline,
    );
    let tv_imdb_id = match tmdb_id_str.as_deref().and_then(|id| id.parse::<i64>().ok()) {
        Some(id) => tokio::time::timeout_at(deadline, state.tmdb_service.get_tv_external_ids(id))
            .await
            .ok()
            .and_then(|ids| ids.imdb_id),
        None => None,
    };
    let stremio_query = match (tv_imdb_id.clone(), season, episode) {
        (Some(imdb), Some(season), Some(episode)) => {
            Some(crate::stremio::service::StremioQuery::Series {
                imdb,
                season,
                episode,
            })
        }
        _ => None,
    };
    let torrent_intent = match (season, episode) {
        (Some(season), Some(episode)) => crate::api::search::TorrentSearchIntent::Episode {
            tvdb_id: tvdb_id_str.clone(),
            imdb_id: tv_imdb_id.clone(),
            season,
            episode,
        },
        (Some(season), None) => crate::api::search::TorrentSearchIntent::Season {
            tvdb_id: tvdb_id_str.clone(),
            imdb_id: tv_imdb_id,
            season,
        },
        _ => crate::api::search::TorrentSearchIntent::FreeText,
    };
    let torrent_future = search_torrents_for_queries_until(
        &state,
        torrent_intent,
        &title,
        &official_name,
        &all_queries,
        limit,
        stremio_query,
        deadline,
    );
    let (folder_results, mut torrent_search) = tokio::join!(folder_future, torrent_future);

    info!(
        "Folder cache returned {} matches for TV",
        folder_results.results.len()
    );
    torrent_search.sources.insert(
        crate::sources::model::SourceKind::Fshare,
        fshare_source_state(folder_results.results.len(), folder_results.truncated),
    );

    // Folder-cache Fshare results followed by unchanged torrent/TorBox results.
    let mut results_pool: Vec<ProviderSearchResult> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for r in folder_results.results {
        if seen.insert(result_dedupe_key(&r)) {
            results_pool.push(r);
        }
    }
    for r in torrent_search.results {
        if seen.insert(result_dedupe_key(&r)) {
            results_pool.push(r);
        }
    }
    results_pool.retain(|result| {
        crate::api::search_pipeline::is_safe_search_result(&result.name)
            && (result
                .result_source
                .as_deref()
                .is_some_and(|source| source != "fshare")
                || crate::api::search_pipeline::is_media_file(&result.name))
    });

    // 3. FINAL DEDUPLICATE (matching V2)
    let mut final_seen = std::collections::HashSet::new();
    results_pool.retain(|f| final_seen.insert(result_dedupe_key(f)));

    // ponytail: restore targeted Fshare discovery only with a verified provider contract.

    let target_results = results_pool; // Process ALL results, not just first 100

    // 4. PARSE AND EVALUATE
    let phase4_start = std::time::Instant::now();
    let mut valid_results = Vec::new();
    let mut reject_counts: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();
    let mut reject_samples: std::collections::HashMap<&'static str, Vec<String>> =
        std::collections::HashMap::new();
    let mut record_reject = |reason: &'static str, name: &str| {
        *reject_counts.entry(reason).or_insert(0) += 1;
        let samples = reject_samples.entry(reason).or_default();
        if samples.len() < 5 {
            samples.push(truncate_chars(name, 140));
        }
    };

    info!("Total files to evaluate: {}", target_results.len());
    let vn_files: Vec<_> = target_results
        .iter()
        .filter(|r| r.name.contains("Bộ Bộ"))
        .map(|r| &r.name)
        .collect();
    info!(
        "Vietnamese files in results: {} - {:?}",
        vn_files.len(),
        vn_files
    );

    for r in target_results {
        let parsed = smart_parse(&r.name);

        // Strict S/E validation for TV
        if let (Some(s_req), Some(e_req)) = (season, episode) {
            // If parser found season/episode, check strict match
            let (mapped_s, mapped_e) = map_absolute_episode(
                base_tmdb_id,
                tvdb_episode_index.as_ref(),
                parsed.season,
                parsed.episode,
            );
            if let (Some(s_file), Some(e_file)) = (mapped_s, mapped_e) {
                if s_file != s_req || e_file != e_req {
                    info!(
                        "FILTERED: '{}' - S/E mismatch (file: S{:02}E{:02}, req: S{:02}E{:02})",
                        r.name, s_file, e_file, s_req, e_req
                    );
                    record_reject("se_mismatch", &r.name);
                    continue;
                }
            } else {
                // Without parser evidence, an exact targeted provider query is not enough
                // to establish episode identity across aliases or season packs.
                record_reject("no_se_found", &r.name);
                continue;
            }
        } else if let Some(s_req) = season {
            // Season pack / Season search
            let (mapped_s, mapped_e) = map_absolute_episode(
                base_tmdb_id,
                tvdb_episode_index.as_ref(),
                parsed.season,
                parsed.episode,
            );
            if is_junk_tv_file(&r.name, mapped_e) {
                record_reject("junk_file", &r.name);
                continue;
            }
            if let Some(s_file) = mapped_s {
                if s_file != s_req {
                    record_reject("season_mismatch", &r.name);
                    continue;
                }
            } else {
                record_reject("no_season_found", &r.name);
                continue;
            }
        }

        let sim = calculate_unified_similarity(&official_name, &r.name, &aliases);
        let best_score = calculate_match_score(
            &official_name,
            &r.name,
            parsed.year,
            effective_year_str
                .as_ref()
                .and_then(|year| year.parse().ok()),
            &aliases,
            true,
        );

        // Debug logging for similarity check
        let _truncated_name: String = r.name.chars().take(80).collect();
        // info!("SIM: '{}' vs '{}...' => score:{:.2} type:{} valid:{}",
        //       official_name, truncated_name, sim.score, sim.match_type, sim.is_valid);

        // Filter out invalid matches (like V2 does - Filter 1)
        if !sim.is_valid || !is_valid_match(best_score, true) {
            record_reject("title_sim_invalid", &r.name);
            continue;
        }

        // PHASE 3: Detect badges
        let (vietdub, vietsub, hdr, dolby_vision) = detect_badges(&r.name);

        valid_results.push(SmartSearchResult {
            name: r.name.clone(),
            url: r.url,
            size: r.size,
            score: (best_score * 100.0).round() as i32,
            quality_name: parsed.quality_name(),
            quality_score: parsed.quality_score(),
            custom_format_score: parsed.custom_format_score(),
            total_score: parsed.total_score(),
            normalized_score: parsed.normalized_score(),
            match_type: if sim.is_valid {
                sim.match_type
            } else {
                "tv_match".to_string()
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
                is_hd: parsed
                    .resolution
                    .as_ref()
                    .map(|r| {
                        r.contains("720")
                            || r.contains("1080")
                            || r.contains("2160")
                            || r.contains("4K")
                    })
                    .unwrap_or(false),
            },
            tmdb_id: base_tmdb_id,
            poster_path: base_poster.clone(),
            vietdub,
            vietsub,
            hdr,
            dolby_vision,
            result_source: r.result_source,
            provider: r.provider,
            indexer: r.indexer,
            seeders: r.seeders,
            leechers: r.leechers,
            info_hash: r.info_hash,
            download_ref: r.download_ref,
            origin_provider: r.origin_provider,
            origin_indexer: r.origin_indexer,
            cached: r.cached,
            file_idx: r.file_idx,
            sources: r.sources,
            behavior_hints: r.behavior_hints,
            inspection: r.inspection,
            inspection_score: r.inspection_score,
        });
    }

    info!("Valid results count: {}", valid_results.len());
    {
        let mut summary: Vec<(&'static str, usize)> =
            reject_counts.iter().map(|(k, v)| (*k, *v)).collect();
        summary.sort_by(|a, b| b.1.cmp(&a.1));
        info!("TV reject summary (title='{}'): {:?}", title, summary);
        for (reason, samples) in &reject_samples {
            info!("TV reject sample [{}]: {:?}", reason, samples);
        }
    }
    let vn_valid: Vec<_> = valid_results
        .iter()
        .filter(|r| r.name.contains("Bộ Bộ"))
        .map(|r| &r.name)
        .collect();
    info!(
        "Vietnamese files in valid_results: {} - {:?}",
        vn_valid.len(),
        vn_valid
    );

    // PHASE 2.1: Sort by match_type first (alias > exact > fuzzy), then by quality
    // This ensures Vietnamese files (alias matches) appear first
    valid_results.sort_by(smart_result_order);

    // Grouping for TV: Seasons -> Episodes -> Files
    let mut seasons_map: std::collections::HashMap<
        u32,
        std::collections::HashMap<u32, Vec<SmartSearchResult>>,
    > = std::collections::HashMap::new();

    for res in valid_results {
        let parsed = smart_parse(&res.name);
        let (mapped_s, mapped_e) = map_absolute_episode(
            base_tmdb_id,
            tvdb_episode_index.as_ref(),
            parsed.season,
            parsed.episode,
        );
        if is_junk_tv_file(&res.name, mapped_e) {
            continue;
        }
        let s = mapped_s.or(season).unwrap_or(1);
        let e = mapped_e.or(episode).unwrap_or(0);
        if e == 0 {
            continue;
        }

        seasons_map
            .entry(s)
            .or_default()
            .entry(e)
            .or_default()
            .push(res);
    }

    // PHASE 4: Fetch Episode Metadata from TMDB
    let mut episode_metadata: std::collections::HashMap<
        (u32, u32),
        (String, String, String, String),
    > = std::collections::HashMap::new();
    let mut season_episode_counts: std::collections::HashMap<u32, u32> =
        std::collections::HashMap::new(); // TMDB official episode counts

    if let Some(tmdb_id) = base_tmdb_id {
        let fetches = seasons_map.keys().copied().map(|s_num| {
            let tmdb_svc = state.tmdb_service.clone();
            let tid = tmdb_id as i64;
            async move {
                tmdb_svc
                    .get_season_details(tid, s_num as i32)
                    .await
                    .map(|data| (s_num, data))
            }
        });

        let results = tokio::time::timeout_at(deadline, futures_util::future::join_all(fetches))
            .await
            .unwrap_or_default();
        for res in results.into_iter().flatten() {
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

                        episode_metadata.insert(
                            (s_num, e_num as u32),
                            (name, overview, air_date, still_path),
                        );
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
        let canonical_episode_count = canonical_episode_counts.get(&s_num).copied();
        for (e_num, mut files) in eps_map {
            if let Some(max_episode) = canonical_episode_count {
                if e_num > max_episode {
                    info!(
                        "Filtering episode outside canonical TVDB/Sonarr season range S{}E{} > {}",
                        s_num, e_num, max_episode
                    );
                    continue;
                }
            }
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
                            info!(
                                "Filtering unreleased episode S{}E{} (air_date: {} is in future)",
                                s_num, e_num, m.2
                            );
                            continue;
                        }
                    }
                }
                // air_date empty but metadata exists → assume aired, show it
            }
            // No TMDB metadata at all → unknown, but file exists → show it

            episodes_grouped.push(EpisodeGroup {
                episode_number: e_num,
                name: meta
                    .map(|m| m.0.clone())
                    .unwrap_or_else(|| format!("Episode {}", e_num)),
                overview: meta
                    .map(|m| {
                        if m.1.is_empty() {
                            None
                        } else {
                            Some(m.1.clone())
                        }
                    })
                    .flatten(),
                air_date: meta
                    .map(|m| {
                        if m.2.is_empty() {
                            None
                        } else {
                            Some(m.2.clone())
                        }
                    })
                    .flatten(),
                still_path: meta
                    .map(|m| {
                        if m.3.is_empty() {
                            None
                        } else {
                            Some(m.3.clone())
                        }
                    })
                    .flatten(),
                files,
            });
        }
        episodes_grouped.sort_by(|a, b| a.episode_number.cmp(&b.episode_number));

        seasons.push(SeasonGroup {
            season: s_num,
            episode_count: canonical_episode_count
                .or_else(|| season_episode_counts.get(&s_num).copied())
                .unwrap_or(episodes_grouped.len() as u32),
            aired_episode_count: aired_count,
            packages: Vec::new(),
            episodes_grouped,
        });
    }
    seasons.sort_by(|a, b| a.season.cmp(&b.season));

    info!(
        "Total Optimized TV Smart Search took: {:?}",
        start_time.elapsed()
    );

    let response = SmartSearchResponse {
        query: query_season,
        total_found: seasons
            .iter()
            .map(|s| {
                s.episodes_grouped
                    .iter()
                    .map(|e| e.files.len())
                    .sum::<usize>()
            })
            .sum(),
        r#type: if episode.is_some() {
            "episode".to_string()
        } else {
            "tv".to_string()
        },
        mode: None,
        kind: None,
        groups: None,
        seasons: Some(seasons),
        sources: Some(torrent_search.sources),
        season_plan: None,
    };

    info!(
        "📊 [PERF] Phase 4 (Parse & Evaluate) took: {:?}",
        phase4_start.elapsed()
    );
    info!("📊 [PERF] ========================================");
    info!(
        "📊 [PERF] TOTAL TV Smart Search took: {:?}",
        start_time.elapsed()
    );
    info!("📊 [PERF] ========================================");

    if cache_eligible && should_cache_smart_response(&response) {
        state.search_cache.insert(cache_key, response.clone()).await;
    } else {
        info!("TV Smart Search returned 0 results; skipping cache to avoid sticky metadata/bridge misses");
    }

    response
}

#[derive(Clone, Copy)]
enum SearchPhase {
    PrimaryMovie,
    PrimaryTv,
}

fn adaptive_limit(_query_count: usize, phase: SearchPhase) -> usize {
    match phase {
        SearchPhase::PrimaryMovie => 100,
        SearchPhase::PrimaryTv => 100,
    }
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
fn build_tv_enrichment_cache(
    enrichment: MediaEnrichment,
    fallback_title: &str,
) -> TmdbEnrichmentCache {
    let official = enrichment.official_name.clone();
    let original_name = enrichment.original_name.clone();
    let original_language = enrichment.original_language.clone();
    let poster = enrichment.poster_path.clone();
    let year = enrichment.year;
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
        year,
        collections: Vec::new(), // TV has no collections
    }
}

fn push_unique_title(
    target: &mut Vec<String>,
    seen: &mut std::collections::HashSet<String>,
    value: String,
) {
    if !value.trim().is_empty() && seen.insert(value.to_lowercase()) {
        target.push(value);
    }
}

fn merge_tvdb_primary_with_tmdb_fallback(
    mut primary: TmdbEnrichmentCache,
    fallback: TmdbEnrichmentCache,
) -> TmdbEnrichmentCache {
    let mut seen: std::collections::HashSet<String> = primary
        .all_aliases
        .iter()
        .chain(primary.vn_titles.iter())
        .chain(primary.original_lang_titles.iter())
        .chain(primary.us_titles.iter())
        .map(|s| s.to_lowercase())
        .collect();

    for title in fallback.vn_titles {
        push_unique_title(&mut primary.vn_titles, &mut seen, title.clone());
        push_unique_title(&mut primary.all_aliases, &mut seen, title);
    }
    for title in fallback.original_lang_titles {
        push_unique_title(&mut primary.original_lang_titles, &mut seen, title.clone());
        push_unique_title(&mut primary.all_aliases, &mut seen, title);
    }
    for title in fallback.us_titles {
        push_unique_title(&mut primary.us_titles, &mut seen, title.clone());
        push_unique_title(&mut primary.all_aliases, &mut seen, title);
    }
    for title in fallback.all_aliases {
        push_unique_title(&mut primary.all_aliases, &mut seen, title);
    }

    if primary.year.is_none() {
        primary.year = fallback.year;
    }
    if primary.poster.is_none() {
        primary.poster = fallback.poster;
    }
    if primary.original_name.is_none() {
        primary.original_name = fallback.original_name;
    }
    primary
}

fn build_tvdb_enrichment_cache(
    enrichment: TvdbEnrichment,
    fallback_title: &str,
) -> TmdbEnrichmentCache {
    let official = enrichment.official_name.clone();
    let original_name = enrichment.original_name.clone();
    let poster = enrichment.poster_path.clone();
    let year = enrichment.year;
    let mut seen = std::collections::HashSet::new();
    let mut all_aliases = Vec::new();
    let mut vn_titles = Vec::new();
    let mut us_titles = Vec::new();
    let original_lang_titles = Vec::new();

    for alias in enrichment.aliases {
        if alias.title.trim().is_empty() || !seen.insert(alias.title.to_lowercase()) {
            continue;
        }
        if alias.country.eq_ignore_ascii_case("VN") || alias.country.eq_ignore_ascii_case("vie") {
            vn_titles.push(alias.title.clone());
        } else if alias.country.eq_ignore_ascii_case("US")
            || alias.country.eq_ignore_ascii_case("eng")
        {
            us_titles.push(alias.title.clone());
        }
        all_aliases.push(alias.title);
    }
    for t in enrichment.translations {
        if t.name.trim().is_empty() || !seen.insert(t.name.to_lowercase()) {
            continue;
        }
        if t.country.eq_ignore_ascii_case("VN") || t.country.eq_ignore_ascii_case("vie") {
            vn_titles.push(t.name.clone());
        } else if t.country.eq_ignore_ascii_case("US") || t.country.eq_ignore_ascii_case("eng") {
            us_titles.push(t.name.clone());
        }
        all_aliases.push(t.name);
    }

    // Preserve the user/UI title as a primary search/scoring alias. TVDB may use
    // native names (e.g. Korean) while Fshare/TMDB/UI use English release titles.
    if !fallback_title.trim().is_empty() && seen.insert(fallback_title.to_lowercase()) {
        us_titles.push(fallback_title.to_string());
        all_aliases.push(fallback_title.to_string());
    }

    TmdbEnrichmentCache {
        official: official.or_else(|| Some(fallback_title.to_string())),
        original_name,
        all_aliases,
        vn_titles,
        original_lang_titles,
        us_titles,
        poster,
        year,
        collections: Vec::new(),
    }
}

/// Build a `TmdbEnrichmentCache` from a Movie `MediaEnrichment`.
fn build_movie_enrichment_cache(
    enrichment: MediaEnrichment,
    fallback_title: &str,
) -> TmdbEnrichmentCache {
    let official = enrichment.official_name.clone();
    let original_name = enrichment.original_name.clone();
    let original_language = enrichment.original_language.clone();
    let poster = enrichment.poster_path.clone();
    let year = enrichment.year;
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
    let collections = enrichment
        .collection
        .map(|coll| {
            coll.parts
                .into_iter()
                .map(|p| (p.title, String::new(), p.id as u64, None::<String>))
                .collect()
        })
        .unwrap_or_default();

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
        year,
        collections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    fn cached_folder_item(linkcode: &str) -> crate::db::sqlite::CachedFolderItem {
        crate::db::sqlite::CachedFolderItem {
            linkcode: linkcode.into(),
            name: "Target.2024.S01E02.mkv".into(),
            title: "Target".into(),
            category: String::new(),
            label: String::new(),
            parent_linkcode: String::new(),
            fshare_url: format!("https://fshare.vn/file/{linkcode}"),
            year: Some(2024),
            season: Some(1),
            episode: Some(2),
            is_series: true,
            is_directory: false,
            size: 1,
            quality: String::new(),
            path: String::new(),
            tmdb_id: Some(7),
            media_type_hint: Some("tv".into()),
            poster_path: None,
        }
    }

    fn folder_identity() -> FolderSearchIdentity {
        FolderSearchIdentity {
            tmdb_id: Some(7),
            media_type: Some("tv"),
            year: Some(2024),
            season: Some(1),
            episode: Some(2),
        }
    }

    #[test]
    fn folder_identity_rejects_each_known_conflict() {
        let identity = folder_identity();
        let mut item = cached_folder_item("base");
        item.tmdb_id = Some(8);
        assert!(!folder_item_matches_identity(&item, identity));

        let mut item = cached_folder_item("type");
        item.media_type_hint = Some("movie".into());
        assert!(!folder_item_matches_identity(&item, identity));

        let mut item = cached_folder_item("year");
        item.year = Some(2022);
        assert!(!folder_item_matches_identity(&item, identity));

        let mut item = cached_folder_item("season");
        item.season = Some(2);
        assert!(!folder_item_matches_identity(&item, identity));

        let mut item = cached_folder_item("episode");
        item.episode = Some(3);
        assert!(!folder_item_matches_identity(&item, identity));
    }

    #[test]
    fn folder_identity_keeps_unknown_evidence_and_near_years() {
        let identity = folder_identity();
        let mut item = cached_folder_item("unknown");
        item.tmdb_id = None;
        item.media_type_hint = Some("unmapped".into());
        item.year = None;
        item.season = None;
        item.episode = None;
        assert!(folder_item_matches_identity(&item, identity));

        for year in [2023, 2024, 2025] {
            let mut item = cached_folder_item("year");
            item.year = Some(year);
            assert!(folder_item_matches_identity(&item, identity));
        }
    }

    #[tokio::test]
    async fn folder_jobs_normalize_unicode_dedupe_before_cap_and_execute_once() {
        let queries = ["ÄRGER", "ärger", " Duplicate ", "duplicate"]
            .into_iter()
            .map(str::to_owned)
            .chain((0..40).map(|index| format!("Title {index}")))
            .collect::<Vec<_>>();
        let jobs = folder_search_jobs(&queries, FOLDER_CACHE_SEARCH_LIMIT);
        let invoked = Arc::new(AtomicUsize::new(0));
        let run = collect_folder_search_jobs_until(
            jobs.clone(),
            Instant::now() + Duration::from_secs(1),
            |job| {
                let invoked = Arc::clone(&invoked);
                async move {
                    invoked.fetch_add(1, Ordering::SeqCst);
                    job
                }
            },
        )
        .await;

        assert_eq!(jobs.len(), 32);
        assert_eq!(invoked.load(Ordering::SeqCst), 32);
        assert_eq!(run.completed.len(), 32);
        assert!(!run.truncated);
        assert_eq!(
            jobs.iter()
                .filter(|(query, _)| query.to_lowercase() == "ärger")
                .count(),
            1
        );
        assert_eq!(
            jobs.iter()
                .filter(|(query, _)| query.to_lowercase() == "duplicate")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn folder_fanout_caps_peak_concurrency_at_four() {
        let started = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let run = collect_folder_search_jobs_until(
            (0..40).collect(),
            Instant::now() + Duration::from_secs(1),
            |job| {
                let started = Arc::clone(&started);
                let active = Arc::clone(&active);
                let peak = Arc::clone(&peak);
                async move {
                    started.fetch_add(1, Ordering::SeqCst);
                    let active_now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(active_now, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    job
                }
            },
        )
        .await;

        assert_eq!(run.completed.len(), 32);
        assert!(!run.truncated);
        assert_eq!(started.load(Ordering::SeqCst), 32);
        assert!(peak.load(Ordering::SeqCst) <= 4);
    }

    #[tokio::test]
    async fn folder_fanout_keeps_fast_rows_before_shared_deadline() {
        let run = collect_folder_search_jobs_until(
            vec!["fast", "slow"],
            Instant::now() + Duration::from_millis(20),
            |name| async move {
                if name == "slow" {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                name
            },
        )
        .await;

        assert_eq!(run.completed, vec!["fast"]);
        assert!(run.truncated);
    }

    #[tokio::test]
    async fn folder_fanout_restores_planned_order_before_linkcode_dedupe() {
        let mut run = collect_folder_search_jobs_until(
            vec![(0, "first"), (1, "other"), (2, "duplicate")],
            Instant::now() + Duration::from_secs(1),
            |(position, name)| async move {
                tokio::time::sleep(Duration::from_millis((2 - position) as u64 * 5)).await;
                (
                    position,
                    cached_folder_item(if name == "duplicate" { "first" } else { name }),
                )
            },
        )
        .await;
        run.completed
            .sort_unstable_by_key(|(position, _)| *position);
        let rows =
            stable_dedupe_folder_items(run.completed.into_iter().map(|(_, item)| item).collect());

        assert_eq!(
            rows.iter()
                .map(|item| item.linkcode.as_str())
                .collect::<Vec<_>>(),
            ["first", "other"]
        );
    }

    #[test]
    fn fshare_source_state_is_partial_after_deadline() {
        assert_eq!(fshare_source_state(1, true).state, "partial");
        assert_eq!(fshare_source_state(1, false).state, "ok");
        assert_eq!(fshare_source_state(0, false).state, "unavailable");
    }

    #[tokio::test]
    async fn dropped_folder_run_cancels_in_flight_work() {
        let active = Arc::new(AtomicUsize::new(0));
        let mut run = Box::pin(collect_folder_search_jobs_until(
            (0..32).collect(),
            Instant::now() + Duration::from_secs(1),
            |_| {
                let active = Arc::clone(&active);
                async move {
                    struct ActiveGuard(Arc<AtomicUsize>);
                    impl Drop for ActiveGuard {
                        fn drop(&mut self) {
                            self.0.fetch_sub(1, Ordering::SeqCst);
                        }
                    }
                    active.fetch_add(1, Ordering::SeqCst);
                    let _guard = ActiveGuard(active);
                    std::future::pending::<()>().await;
                }
            },
        ));
        tokio::select! {
            _ = &mut run => panic!("pending folder run completed"),
            _ = async {
                while active.load(Ordering::SeqCst) < 4 {
                    tokio::task::yield_now().await;
                }
            } => {}
        }
        drop(run);
        tokio::task::yield_now().await;

        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn orchestrated_legacy_handlers_do_not_write_search_cache() {
        let source = include_str!("smart_search.rs");
        assert!(source.contains("cache_eligible && should_cache_smart_response"));
        let orchestrator = include_str!("smart_search/orchestrator.rs");
        assert_eq!(orchestrator.matches("deadline, false").count(), 1);
        assert_eq!(orchestrator.matches("false,\n                )").count(), 1);
    }

    #[test]
    fn folder_linkcode_dedupe_is_stable() {
        let rows = stable_dedupe_folder_items(vec![
            cached_folder_item("first"),
            cached_folder_item("other"),
            cached_folder_item("first"),
        ]);
        assert_eq!(
            rows.iter()
                .map(|item| item.linkcode.as_str())
                .collect::<Vec<_>>(),
            ["first", "other"]
        );
    }

    fn dragon_ball_tvdb_index() -> TvdbEpisodeIndex {
        let pairs = (1..=13).map(|ep| (1, ep)).chain((1..=20).map(|ep| (2, ep)));
        TvdbEpisodeIndex::from_pairs(pairs).expect("test index should build")
    }

    #[test]
    fn tvdb_episode_index_maps_absolute_to_canonical_episode() {
        let index = dragon_ball_tvdb_index();

        assert_eq!(index.absolute_to_episode(13).unwrap().season, 1);
        assert_eq!(index.absolute_to_episode(13).unwrap().episode, 13);
        assert_eq!(index.absolute_to_episode(14).unwrap().season, 2);
        assert_eq!(index.absolute_to_episode(14).unwrap().episode, 1);
        assert_eq!(index.absolute_to_episode(29).unwrap().season, 2);
        assert_eq!(index.absolute_to_episode(29).unwrap().episode, 16);
    }

    #[test]
    fn tvdb_episode_index_maps_canonical_episode_to_absolute() {
        let index = dragon_ball_tvdb_index();

        assert_eq!(index.episode_to_absolute(2, 1), Some(14));
        assert_eq!(index.episode_to_absolute(2, 16), Some(29));
        assert_eq!(index.episode_to_absolute(3, 1), None);
    }

    #[test]
    fn tvdb_absolute_mapping_uses_inferred_flat_season_only() {
        let index = dragon_ball_tvdb_index();

        assert_eq!(
            map_absolute_episode(None, Some(&index), Some(1), Some(29)),
            (Some(2), Some(16))
        );
        assert_eq!(
            map_absolute_episode(None, Some(&index), None, Some(14)),
            (Some(2), Some(1))
        );
        assert_eq!(
            map_absolute_episode(None, Some(&index), Some(2), Some(1)),
            (Some(2), Some(1))
        );
    }

    fn cache_response(total_found: usize, states: &[&str]) -> SmartSearchResponse {
        SmartSearchResponse {
            query: "test".to_string(),
            total_found,
            r#type: "movie".to_string(),
            mode: None,
            kind: None,
            groups: None,
            seasons: None,
            sources: Some(
                states
                    .iter()
                    .enumerate()
                    .map(|(index, state)| {
                        let source = match index {
                            0 => crate::sources::model::SourceKind::Fshare,
                            1 => crate::sources::model::SourceKind::Torrent,
                            _ => crate::sources::model::SourceKind::Torbox,
                        };
                        (source, source_state(state, None))
                    })
                    .collect(),
            ),
            season_plan: None,
        }
    }

    #[test]
    fn torbox_source_state_rejects_malformed_enabled_key_without_exposure() {
        let state = torbox_configuration_state(true, true, " bad-key", 0);
        assert_eq!(state.state, "unavailable");
        assert_eq!(state.message.as_deref(), Some("TorBox is not configured"));
        assert!(!serde_json::to_string(&state).unwrap().contains("bad-key"));
    }

    #[test]
    fn smart_response_cache_rejects_partial_or_unavailable_sources() {
        assert!(!should_cache_smart_response(&cache_response(
            13,
            &["ok", "unavailable", "disabled"]
        )));
        assert!(!should_cache_smart_response(&cache_response(
            13,
            &["ok", "partial", "disabled"]
        )));
    }

    #[test]
    fn smart_response_cache_allows_complete_results_and_disabled_sources() {
        assert!(should_cache_smart_response(&cache_response(
            13,
            &["ok", "ok", "disabled"]
        )));
        assert!(!should_cache_smart_response(&cache_response(
            0,
            &["ok", "ok", "disabled"]
        )));
    }

    #[test]
    fn prowlarr_queries_prioritize_raw_title() {
        let queries = prioritized_prowlarr_queries(
            "Moments We Shared",
            "Moments We Shared Again",
            &[
                "moments we shared".to_string(),
                "Moments We Shared Again S01E01".to_string(),
                "Moments We Shared Season 1".to_string(),
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
                "f".to_string(),
            ],
        );

        assert_eq!(queries[0], "Moments We Shared");
        assert_eq!(queries[1], "Moments We Shared Again");
        assert_eq!(queries.len(), 8);
    }

    fn test_source(state: &str, message: &str, addon: bool) -> types::SmartSearchSourceState {
        types::SmartSearchSourceState {
            state: state.into(),
            message: Some(message.into()),
            addons: addon.then(|| {
                vec![types::StremioAddonState {
                    id: "addon-id".into(),
                    name: "Torrentio".into(),
                    health: "healthy".into(),
                    result_count: 1,
                    elapsed_ms: 7,
                    failure_class: None,
                }]
            }),
            telemetry: addon.then(|| types::StremioTelemetry::default().into()),
        }
    }

    #[test]
    fn prowlarr_telemetry_serializes_counts_without_secrets() {
        let telemetry = types::ProwlarrTelemetry {
            planned: 9,
            attempted: 8,
            succeeded: 6,
            failed: 1,
            timed_out: 1,
            truncated: 2,
            hash_resolved: 4,
            hash_cache_hits: 3,
            hash_omitted: 2,
            inspections_selected: 10,
            inspections_completed: 7,
            inspections_timed_out: 3,
        };
        let source = types::SmartSearchSourceState {
            state: "partial".into(),
            message: Some("timeout".into()),
            addons: None,
            telemetry: Some(types::SourceTelemetry::Prowlarr(telemetry)),
        };
        let wire = serde_json::to_string(&source).unwrap();
        assert!(wire.contains("\"timed_out\":1"));
        assert!(wire.contains("\"hash_resolved\":4"));
        assert!(!wire.contains("SUPER-SECRET"));
        assert!(!wire.contains("url"));
        assert!(!wire.contains("query"));
    }

    #[test]
    fn target_sources_keep_torrent_and_stremio_attribution_independent() {
        let torrent = test_source("ok", "Prowlarr searched", false);
        let stremio = test_source("partial", "Stremio addons searched", true);
        let sources = source_states(
            fshare_source_state(1, false),
            Some(torrent),
            Some(stremio),
            source_state("disabled", None),
        );
        let wire = serde_json::to_string(&sources).unwrap();
        let value: serde_json::Value = serde_json::from_str(&wire).unwrap();

        assert_eq!(value["torrent"]["message"], "Prowlarr searched");
        assert!(value["torrent"].get("addons").is_none());
        assert!(value["torrent"].get("telemetry").is_none());
        assert_eq!(value["stremio"]["message"], "Stremio addons searched");
        assert_eq!(value["stremio"]["addons"][0]["name"], "Torrentio");
        assert!(value["stremio"].get("telemetry").is_some());
    }

    #[test]
    fn target_sources_omit_provider_buckets_that_were_not_searched() {
        let stremio_only = source_states(
            fshare_source_state(1, false),
            None,
            Some(test_source("ok", "Stremio searched", true)),
            source_state("disabled", None),
        );
        assert!(stremio_only.contains_key(&crate::sources::model::SourceKind::Stremio));
        assert!(!stremio_only.contains_key(&crate::sources::model::SourceKind::Torrent));

        let torrent_only = source_states(
            fshare_source_state(1, false),
            Some(test_source("ok", "Prowlarr searched", false)),
            None,
            source_state("disabled", None),
        );
        assert!(torrent_only.contains_key(&crate::sources::model::SourceKind::Torrent));
        assert!(!torrent_only.contains_key(&crate::sources::model::SourceKind::Stremio));
    }

    #[test]
    fn target_movie_and_episode_source_wire_is_sanitized() {
        for intent in [
            types::SmartSearchIntent::TargetMovie {
                title: "Movie".into(),
                year: None,
                tmdb_id: None,
            },
            types::SmartSearchIntent::TargetEpisode {
                title: "Show".into(),
                year: None,
                tmdb_id: None,
                tvdb_id: None,
                season: 1,
                episode: 3,
            },
        ] {
            let sources = source_states(
                fshare_source_state(1, false),
                None,
                Some(test_source("ok", "Stremio searched", true)),
                source_state("disabled", None),
            );
            let result = match intent {
                types::SmartSearchIntent::TargetMovie { .. } => {
                    aggregator::AggregatedSmartSearchResult::MovieGroups {
                        query: "canonical".into(),
                        groups: vec![],
                        sources: Some(sources),
                    }
                }
                _ => aggregator::AggregatedSmartSearchResult::Seasons {
                    query: "canonical".into(),
                    seasons: vec![],
                    is_target_episode: true,
                    sources: Some(sources),
                },
            };
            let response =
                aggregator::serialize_result(result, &intent, types::RequestContract::Explicit);
            let wire = serde_json::to_string(&response).unwrap();
            let value: serde_json::Value = serde_json::from_str(&wire).unwrap();
            assert_eq!(value["mode"], "target");
            assert!(matches!(value["kind"].as_str(), Some("movie" | "episode")));
            assert!(value["sources"].get("stremio").is_some());
            assert!(value["sources"].get("torrent").is_none());
            assert_eq!(value["total_found"], 0);
            for secret in [
                "sentinel-token",
                "sentinel-ciphertext",
                "https://sentinel.invalid/config",
            ] {
                assert!(!wire.contains(secret));
            }
        }
    }

    #[test]
    fn torrent_provider_plan_keeps_stremio_independent_and_reports_missing_imdb() {
        let disabled = torrent_provider_plan(false, true, true);
        assert!(!disabled.search_prowlarr);
        assert!(disabled.search_stremio);
        assert_eq!(disabled.message, "Prowlarr disabled; Stremio searched");

        let missing_key = torrent_provider_plan(true, false, true);
        assert!(!missing_key.search_prowlarr);
        assert!(missing_key.search_stremio);
        assert_eq!(
            missing_key.message,
            "Prowlarr API key missing; Stremio searched"
        );

        let missing_imdb = torrent_provider_plan(false, false, false);
        assert!(!missing_imdb.search_stremio);
        assert_eq!(
            missing_imdb.message,
            "Prowlarr disabled; Stremio unavailable: IMDb ID missing"
        );
    }

    #[test]
    fn stremio_health_maps_success_zero_mixed_and_unavailable() {
        use crate::stremio::service::{AddonHealth, AddonStatus};
        let addon = |health| AddonStatus {
            addon_id: "id".into(),
            addon_name: "name".into(),
            addon_order: 0,
            health,
            result_count: 0,
            elapsed_ms: 1,
            failure_class: None,
        };
        assert_eq!(
            stremio_health_summary(1, &[addon(AddonHealth::Healthy)]).0,
            "ok"
        );
        assert_eq!(
            stremio_health_summary(0, &[addon(AddonHealth::Healthy)]).0,
            "no_results"
        );
        assert_eq!(
            stremio_health_summary(
                1,
                &[addon(AddonHealth::Healthy), addon(AddonHealth::Timeout)]
            )
            .0,
            "partial"
        );
        assert_eq!(
            stremio_health_summary(0, &[addon(AddonHealth::Degraded)]).0,
            "unavailable"
        );
    }

    #[test]
    fn stremio_public_health_messages_never_include_secret() {
        const SENTINEL: &str = "secret-sentinel";
        for (_, message) in [
            stremio_health_summary(0, &[]),
            stremio_health_summary(
                0,
                &[crate::stremio::service::AddonStatus {
                    addon_id: SENTINEL.into(),
                    addon_name: SENTINEL.into(),
                    addon_order: 0,
                    health: crate::stremio::service::AddonHealth::Degraded,
                    result_count: 0,
                    elapsed_ms: 0,
                    failure_class: Some(crate::stremio::client::StremioFailureClass::Other),
                }],
            ),
        ] {
            assert!(!message.contains(SENTINEL));
        }
    }

    #[test]
    fn stremio_telemetry_counts_full_run_beyond_projection_cap() {
        use crate::stremio::{
            client::StremioFailureClass,
            service::{AddonHealth, AddonStatus},
        };
        let statuses = (0..70)
            .map(|i| AddonStatus {
                addon_id: format!("id-{i}"),
                addon_name: "name".into(),
                addon_order: i,
                health: if i % 2 == 0 {
                    AddonHealth::Healthy
                } else {
                    AddonHealth::Degraded
                },
                result_count: 0,
                elapsed_ms: 1,
                failure_class: (i % 2 == 1).then_some(StremioFailureClass::Status429),
            })
            .collect::<Vec<_>>();
        let summary = stremio_telemetry(&statuses);
        assert_eq!(sanitized_stremio_addons(&statuses).len(), 64);
        assert_eq!(summary.total_calls, 70);
        assert!(summary.truncated);
        assert_eq!(summary.by_health["healthy"], 35);
        assert_eq!(summary.by_failure_class["status_429"], 35);
    }

    #[test]
    fn prowlarr_fallback_stops_only_on_usable_results() {
        let empty = crate::api::search::ProwlarrQueryOutcome::Success(
            crate::api::search::ProwlarrSearchResponse {
                results: Vec::new(),
                item_count: 0,
                discarded_count: 0,
                unexpected_shape: false,
                diagnostic: "JSON array",
            },
        );
        let usable = crate::api::search::ProwlarrQueryOutcome::Success(
            crate::api::search::ProwlarrSearchResponse {
                results: vec![crate::api::search::MultiSearchResult {
                    id: "id".to_string(),
                    name: "name".to_string(),
                    parsed_name: "name".to_string(),
                    url: "magnet:?xt=urn:btih:abc".to_string(),
                    download_ref: None,
                    size: 0,
                    source: "torrent".to_string(),
                    provider: "prowlarr".to_string(),
                    indexer: None,
                    origin_provider: None,
                    origin_indexer: None,
                    cached: None,
                    file_idx: None,
                    sources: None,
                    behavior_hints: None,
                    seeders: None,
                    leechers: None,
                    info_hash: Some("abc".to_string()),
                    quality: None,
                    resolution: None,
                    episode_tag: None,
                    media_type: None,
                    inspection: None,
                    inspection_score: None,
                }],
                item_count: 1,
                discarded_count: 0,
                unexpected_shape: false,
                diagnostic: "JSON array",
            },
        );

        assert!(!should_stop_prowlarr_fallback(&empty));
        assert!(!should_stop_prowlarr_fallback(
            &crate::api::search::ProwlarrQueryOutcome::Failed
        ));
        assert!(!should_stop_prowlarr_fallback(
            &crate::api::search::ProwlarrQueryOutcome::TimedOut
        ));
        assert!(should_stop_prowlarr_fallback(&usable));
    }

    #[test]
    fn keyword_overlap_is_a_valid_tv_match() {
        let similarity = calculate_unified_similarity(
            "Moments We Shared Again",
            "Moments We Shared S01E01 1080p WEB-DL.mkv",
            &[],
        );

        assert!(similarity.is_valid);
        assert_eq!(similarity.match_type, "keyword_overlap");
    }

    #[test]
    fn torrent_info_hash_dedupe_key_is_cross_source() {
        let hash = "0123456789abcdef0123456789abcdef01234567";
        let torrent = ProviderSearchResult {
            name: "Moments We Shared 1080p".to_string(),
            url: format!("magnet:?xt=urn:btih:{hash}"),
            fcode: hash.to_string(),
            size: 1,
            score: 0,
            result_source: Some("torrent".to_string()),
            provider: Some("prowlarr".to_string()),
            indexer: None,
            seeders: None,
            leechers: None,
            info_hash: Some(hash.to_string()),
            download_ref: None,
            origin_provider: None,
            origin_indexer: None,
            cached: None,
            file_idx: None,
            sources: None,
            behavior_hints: None,
            inspection: None,
            inspection_score: None,
        };
        let mut torbox = torrent.clone();
        torbox.result_source = Some("torbox".to_string());

        assert_eq!(
            result_dedupe_key(&torrent),
            format!("torrent:hash:{hash}:None")
        );
        assert_eq!(
            result_dedupe_key(&torbox),
            format!("torrent:hash:{hash}:None")
        );
    }

    fn ranked_fixture(id: &str, inspection_score: i32, seeders: i64) -> SmartSearchResult {
        serde_json::from_value(serde_json::json!({
            "name": id, "url": "", "size": 1, "score": 0,
            "quality_name": "1080p", "quality_score": 0, "custom_format_score": 0,
            "total_score": 0, "normalized_score": 0.0, "match_type": "exact",
            "quality_attrs": crate::utils::parser::QualityAttributes::default(),
            "tmdb_id": null, "poster_path": null, "vietdub": false, "vietsub": false,
            "hdr": false, "dolby_vision": false, "seeders": seeders,
            "info_hash": id, "inspection_score": inspection_score
        }))
        .unwrap()
    }

    #[test]
    fn verified_mismatch_cannot_outseed_verified_target() {
        let mut results = vec![
            ranked_fixture("mismatch", -250, i64::MAX),
            ranked_fixture("cover-target", 1_000, 1),
            ranked_fixture("exact-target", 1_000, 2),
        ];
        results.sort_by(smart_result_order);
        assert_eq!(
            results
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            ["exact-target", "cover-target", "mismatch"]
        );
    }

    #[test]
    fn ranking_is_permutation_invariant_and_ties_are_stable() {
        let base = [
            ranked_fixture("charlie", 1_000, 4),
            ranked_fixture("alpha", 1_000, 4),
            ranked_fixture("bravo", 1_000, 4),
            ranked_fixture("mismatch", -250, i64::MAX),
        ];
        let expected = ["alpha", "bravo", "charlie", "mismatch"];
        for permutation in 0..24 {
            let mut results = (0..4)
                .map(|offset| base[(offset + permutation) % 4].clone())
                .collect::<Vec<_>>();
            if permutation % 2 == 1 {
                results.reverse();
            }
            results.sort_by(smart_result_order);
            assert_eq!(
                results
                    .iter()
                    .map(|item| item.name.as_str())
                    .collect::<Vec<_>>(),
                expected,
                "permutation {permutation}"
            );
        }
        let a = ranked_fixture("same", 1_000, 4);
        let b = a.clone();
        assert_eq!(smart_result_order(&a, &b), std::cmp::Ordering::Equal);
    }

    #[test]
    fn magnet_fcode_fallback_extracts_btih_only() {
        let mut result = ProviderSearchResult {
            name: "x".to_string(),
            url: "magnet:?xt=urn:btih:22A2E1DF69B9016D5130D3E20DEEC478EEDADF8B&dn=lion".to_string(),
            fcode: "magnet:?xt=urn:btih:22A2E1DF69B9016D5130D3E20DEEC478EEDADF8B&dn=lion"
                .to_string(),
            size: 1,
            score: 0,
            result_source: Some("torrent".to_string()),
            provider: Some("prowlarr".to_string()),
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
        };

        if result.info_hash.is_none() && result.fcode.starts_with("magnet:") {
            result.info_hash = result
                .fcode
                .split("btih:")
                .nth(1)
                .and_then(|tail| tail.split('&').next())
                .map(|hash| hash.to_ascii_lowercase());
        }

        assert_eq!(
            result.info_hash.as_deref(),
            Some("22a2e1df69b9016d5130d3e20deec478eedadf8b")
        );
    }
}
