use std::sync::Arc;

use axum::response::IntoResponse;

use crate::AppState;

use super::aggregator::{aggregate_legacy_response, serialize_result};
use super::types::{
    ParsedSmartSearchRequest, RequestContract, SeasonPlanOutcome, SeasonPlanState,
    SmartSearchIntent, SmartSearchResponse,
};

fn season_stream_result(
    item: crate::stremio::service::AggregatedStream,
) -> super::SmartSearchResult {
    let filename = item
        .stream
        .behavior_hints
        .filename
        .clone()
        .unwrap_or_else(|| "Stremio stream".into());
    let parsed = crate::utils::smart_tokenizer::smart_parse(&filename);
    let (vietdub, vietsub, hdr, dolby_vision) =
        crate::utils::title_matcher::detect_badges(&filename);
    super::SmartSearchResult {
        name: filename,
        url: String::new(),
        size: item.stream.behavior_hints.video_size.unwrap_or(0),
        score: 0,
        quality_name: parsed.quality_name(),
        quality_score: parsed.quality_score(),
        custom_format_score: parsed.custom_format_score(),
        total_score: parsed.total_score(),
        normalized_score: parsed.normalized_score(),
        match_type: "tv_match".into(),
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
            is_tv: true,
            is_movie: false,
            is_hd: parsed
                .resolution
                .as_ref()
                .is_some_and(|v| ["720", "1080", "2160", "4K"].iter().any(|n| v.contains(n))),
        },
        tmdb_id: None,
        poster_path: None,
        vietdub,
        vietsub,
        hdr,
        dolby_vision,
        result_source: Some("torrent".into()),
        provider: Some("stremio".into()),
        indexer: Some(item.addon_name.clone()),
        seeders: None,
        leechers: None,
        info_hash: Some(item.stream.info_hash.clone()),
        download_ref: None,
        origin_provider: Some("stremio".into()),
        origin_indexer: Some(item.addon_id),
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
}

fn observed_episode(season: u32, filename: Option<&str>) -> Option<u32> {
    let parsed = crate::utils::smart_tokenizer::smart_parse(filename?);
    (parsed.season == Some(season))
        .then_some(parsed.episode)
        .flatten()
}

fn season_result_state(
    has_results: bool,
    healthy: usize,
    unhealthy: usize,
    complete: bool,
) -> SeasonPlanState {
    if !has_results {
        if healthy == 0 && unhealthy > 0 {
            SeasonPlanState::AddonUnavailable
        } else {
            SeasonPlanState::NoResults
        }
    } else if complete && unhealthy == 0 {
        SeasonPlanState::Ok
    } else {
        SeasonPlanState::Partial
    }
}

fn season_status(state: SeasonPlanState) -> (&'static str, &'static str) {
    match state {
        SeasonPlanState::Ok => ("ok", "Stremio season plan ready"),
        SeasonPlanState::Partial => ("partial", "Stremio season search partially completed"),
        SeasonPlanState::IdentityUnavailable => {
            ("identity_unavailable", "Stremio identity unavailable")
        }
        SeasonPlanState::RegistryUnavailable => {
            ("registry_unavailable", "Stremio registry unavailable")
        }
        SeasonPlanState::AddonUnavailable => ("unavailable", "Stremio addons unavailable"),
        SeasonPlanState::NoTargets => ("no_targets", "No aired missing episodes to search"),
        SeasonPlanState::NoResults => ("no_results", "Stremio returned no season results"),
    }
}

fn merge_season_groups(target: &mut Vec<super::SeasonGroup>, incoming: Vec<super::SeasonGroup>) {
    for season in incoming {
        let Some(target_season) = target.iter_mut().find(|item| item.season == season.season)
        else {
            target.push(season);
            continue;
        };
        for episode in season.episodes_grouped {
            let Some(target_episode) = target_season
                .episodes_grouped
                .iter_mut()
                .find(|item| item.episode_number == episode.episode_number)
            else {
                target_season.episodes_grouped.push(episode);
                continue;
            };
            let file_key = |file: &super::SmartSearchResult| {
                (
                    file.origin_indexer.clone(),
                    file.info_hash
                        .as_ref()
                        .map(|hash| hash.to_ascii_lowercase()),
                    file.behavior_hints
                        .as_ref()
                        .and_then(|hints| hints.binge_group.clone()),
                    episode.episode_number,
                    file.file_idx,
                )
            };
            let mut seen = target_episode
                .files
                .iter()
                .map(file_key)
                .collect::<std::collections::HashSet<_>>();
            for file in episode.files {
                if seen.insert(file_key(&file)) {
                    target_episode.files.push(file);
                }
            }
        }
    }
}

fn lift_torrent_packages(seasons: &mut [super::SeasonGroup]) {
    for season in seasons {
        let mut packages = std::collections::BTreeMap::<
            (String, Option<String>, Option<String>),
            super::TorrentPackage,
        >::new();
        for episode in &mut season.episodes_grouped {
            episode.files.retain(|file| {
                let Some(hash) = file.info_hash.as_deref().map(str::to_ascii_lowercase) else {
                    return true;
                };
                let package = packages
                    .entry((
                        hash.clone(),
                        file.origin_indexer.clone(),
                        file.behavior_hints
                            .as_ref()
                            .and_then(|hints| hints.binge_group.clone()),
                    ))
                    .or_insert_with(|| super::TorrentPackage {
                        info_hash: hash,
                        result: file.clone(),
                        covered_episodes: Vec::new(),
                        file_indices: Default::default(),
                        episode_files: Default::default(),
                    });
                if !package.covered_episodes.contains(&episode.episode_number) {
                    package.covered_episodes.push(episode.episode_number);
                }
                if let Some(file_idx) = file.file_idx {
                    package
                        .file_indices
                        .insert(episode.episode_number, file_idx);
                }
                package
                    .episode_files
                    .insert(episode.episode_number, file.clone());
                false
            });
        }
        season.packages = packages.into_values().collect();
    }
}

pub(crate) async fn build_season_plan(
    state: &Arc<AppState>,
    intent: &SmartSearchIntent,
) -> SeasonPlanOutcome {
    use crate::stremio::season_pack::{plan_season, EpisodeCandidate, EpisodeTarget, PlanMode};
    let SmartSearchIntent::Season {
        tmdb_id, season, ..
    } = intent
    else {
        return SeasonPlanOutcome::new(SeasonPlanState::Ok, None);
    };
    let number = |v: &serde_json::Value| v.as_i64().or_else(|| v.as_str()?.parse().ok());
    let Some(tmdb) = tmdb_id.as_ref().and_then(number) else {
        return SeasonPlanOutcome::new(SeasonPlanState::IdentityUnavailable, None);
    };
    let external = state.tmdb_service.get_tv_external_ids(tmdb).await;
    let Some(imdb) = external.imdb_id else {
        return SeasonPlanOutcome::new(SeasonPlanState::IdentityUnavailable, None);
    };
    let Some(details) = state
        .tmdb_service
        .get_season_details(tmdb, *season as i32)
        .await
    else {
        return SeasonPlanOutcome::new(SeasonPlanState::IdentityUnavailable, None);
    };
    let Some(episode_rows) = details["episodes"].as_array() else {
        return SeasonPlanOutcome::new(SeasonPlanState::IdentityUnavailable, None);
    };
    let today = chrono::Utc::now().date_naive();
    let targets: Vec<_> = episode_rows
        .iter()
        .filter_map(|episode| {
            let number = episode["episode_number"].as_u64()? as u32;
            let aired = episode["air_date"]
                .as_str()
                .and_then(|date| chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
                .is_some_and(|date| date <= today);
            Some(EpisodeTarget {
                episode: number,
                aired,
                monitored: true,
                missing: true,
            })
        })
        .collect();
    let episodes: Vec<_> = targets
        .iter()
        .filter(|e| e.aired && e.monitored && e.missing)
        .map(|e| e.episode)
        .collect();
    if episodes.is_empty() {
        return SeasonPlanOutcome::new(SeasonPlanState::NoTargets, None);
    }
    let Ok(registry) = crate::stremio::AddonRegistry::load_or_create() else {
        return SeasonPlanOutcome::new(SeasonPlanState::RegistryUnavailable, None);
    };
    let Ok(service) = crate::stremio::service::StremioService::new(&state.db, &registry) else {
        return SeasonPlanOutcome::new(SeasonPlanState::RegistryUnavailable, None);
    };
    let discovery = service
        .search_season_exhaustive(&imdb, *season, &episodes)
        .await;
    let season_counts = (
        discovery.target_episodes.len(),
        discovery.queried_episodes.len(),
        discovery.covered_episodes.len(),
        discovery.waves,
    );
    let mut candidates = Vec::new();
    let mut display_files = std::collections::BTreeMap::<u32, Vec<super::SmartSearchResult>>::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut healthy = 0usize;
    let mut unhealthy = 0usize;
    let mut addons = Vec::new();
    for (_queried_episode, output) in discovery.results {
        for addon in &output.addons {
            match addon.health {
                crate::stremio::service::AddonHealth::Healthy => healthy += 1,
                _ => unhealthy += 1,
            }
        }
        addons.extend(output.addons.iter().cloned());
        for (position, item) in output.results.into_iter().enumerate() {
            let hints = &item.stream.behavior_hints;
            let Some(episode) = observed_episode(*season, hints.filename.as_deref()) else {
                continue;
            };
            let binge_group = hints.binge_group.clone();
            if !seen.insert((
                item.addon_id.clone(),
                item.stream.info_hash.to_ascii_lowercase(),
                binge_group.clone(),
                episode,
                item.stream.file_idx,
            )) {
                continue;
            }
            display_files
                .entry(episode)
                .or_default()
                .push(season_stream_result(item.clone()));
            candidates.push(EpisodeCandidate {
                episode,
                addon_id: item.addon_id,
                provider: item.addon_name,
                info_hash: item.stream.info_hash,
                binge_group,
                manifest_position: position,
                provider_file_id: None,
                file_idx: item.stream.file_idx,
                identity_match: true,
                mapping_match: true,
                provider_allowed: true,
                quality_language_match: true,
                cached: false,
                quality_score: 0,
                size_bytes: hints.video_size.unwrap_or(0),
            });
        }
    }
    if candidates.is_empty() {
        let mut outcome =
            SeasonPlanOutcome::new(season_result_state(false, healthy, unhealthy, false), None);
        let mut telemetry = super::stremio_telemetry(&addons);
        (
            telemetry.target_episodes,
            telemetry.queried_episodes,
            telemetry.covered_episodes,
            telemetry.waves,
        ) = season_counts;
        outcome.telemetry = Some(telemetry);
        outcome.addons = super::sanitized_stremio_addons(&addons);
        return outcome;
    }
    let mut plan = plan_season(
        &targets,
        &candidates,
        PlanMode::Consistent,
        &Default::default(),
    );
    let complete = discovery.terminal
        && discovery.covered_episodes == discovery.target_episodes
        && unhealthy == 0;
    plan.discovery_status = if complete { "complete" } else { "degraded" }.into();
    plan.queried_episodes = discovery.queried_episodes;
    plan.request_bound = episodes.len();
    let mut outcome = SeasonPlanOutcome::new(
        season_result_state(true, healthy, unhealthy, complete),
        Some(plan),
    );
    outcome.seasons = vec![super::SeasonGroup {
        season: *season,
        episode_count: targets.len() as u32,
        aired_episode_count: episodes.len() as u32,
        packages: Vec::new(),
        episodes_grouped: episode_rows
            .iter()
            .filter_map(|row| {
                let episode = row["episode_number"].as_u64()? as u32;
                let files = display_files.remove(&episode).unwrap_or_default();
                (!files.is_empty()).then(|| super::EpisodeGroup {
                    episode_number: episode,
                    name: row["name"].as_str().unwrap_or("Episode").to_string(),
                    overview: row["overview"].as_str().map(str::to_string),
                    air_date: row["air_date"].as_str().map(str::to_string),
                    still_path: row["still_path"].as_str().map(str::to_string),
                    files,
                })
            })
            .collect(),
    }];
    let mut telemetry = super::stremio_telemetry(&addons);
    (
        telemetry.target_episodes,
        telemetry.queried_episodes,
        telemetry.covered_episodes,
        telemetry.waves,
    ) = season_counts;
    outcome.telemetry = Some(telemetry);
    outcome.addons = super::sanitized_stremio_addons(&addons);
    outcome
}

struct BranchOutcomes<S, L> {
    season: Option<S>,
    legacy: Option<L>,
}

async fn run_branches_until<A, B>(
    season: A,
    legacy: B,
    deadline: tokio::time::Instant,
) -> BranchOutcomes<A::Output, B::Output>
where
    A: std::future::Future,
    B: std::future::Future,
{
    tokio::pin!(season);
    tokio::pin!(legacy);
    let mut season_outcome = None;
    let mut legacy_outcome = None;
    loop {
        tokio::select! {
            result = &mut season, if season_outcome.is_none() => season_outcome = Some(result),
            result = &mut legacy, if legacy_outcome.is_none() => legacy_outcome = Some(result),
            _ = tokio::time::sleep_until(deadline) => break,
        }
        if season_outcome.is_some() && legacy_outcome.is_some() {
            break;
        }
    }
    BranchOutcomes {
        season: season_outcome,
        legacy: legacy_outcome,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TorboxEnrichment {
    cached_count: usize,
    status: Option<crate::sources::torbox::CacheLookupError>,
}

impl TorboxEnrichment {
    pub(crate) fn complete(cached_count: usize) -> Self {
        Self {
            cached_count,
            status: None,
        }
    }

    pub(crate) fn degraded(
        cached_count: usize,
        status: crate::sources::torbox::CacheLookupError,
    ) -> Self {
        Self {
            cached_count,
            status: Some(status),
        }
    }

    pub(crate) fn cached_count(&self) -> usize {
        self.cached_count
    }

    pub(crate) fn status(&self) -> Option<crate::sources::torbox::CacheLookupError> {
        self.status
    }
}

fn final_torbox_hashes(response: &super::types::SmartSearchResponse) -> Vec<String> {
    response
        .groups
        .iter()
        .flatten()
        .flat_map(|group| &group.files)
        .chain(
            response
                .seasons
                .iter()
                .flatten()
                .flat_map(|season| &season.episodes_grouped)
                .flat_map(|episode| &episode.files),
        )
        .filter_map(|row| {
            row.info_hash
                .as_deref()
                .and_then(crate::torrent::normalize_info_hash)
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn apply_final_torbox_hits(
    response: &mut super::types::SmartSearchResponse,
    hits: &std::collections::HashMap<String, crate::torbox::client::TorBoxCacheHit>,
) -> usize {
    let mut rows = response
        .groups
        .iter_mut()
        .flatten()
        .flat_map(|group| &mut group.files)
        .chain(
            response
                .seasons
                .iter_mut()
                .flatten()
                .flat_map(|season| &mut season.episodes_grouped)
                .flat_map(|episode| &mut episode.files),
        )
        .collect::<Vec<_>>();
    crate::sources::search::apply_canonical_cache_hits(&mut rows, hits)
}

pub(crate) async fn enrich_final_torbox_response_with<F, Fut>(
    response: &mut super::types::SmartSearchResponse,
    _deadline: tokio::time::Instant,
    fetch: F,
) -> TorboxEnrichment
where
    F: FnOnce(Vec<String>) -> Fut,
    Fut: std::future::Future<Output = crate::sources::torbox::CacheLookupOutcome>,
{
    let hashes = final_torbox_hashes(response);
    if hashes.is_empty() {
        return TorboxEnrichment::complete(0);
    }
    let outcome = fetch(hashes).await;
    let cached_count = apply_final_torbox_hits(response, &outcome.hits);
    match outcome.status {
        Some(status) => TorboxEnrichment::degraded(cached_count, status),
        None => TorboxEnrichment::complete(cached_count),
    }
}

async fn enrich_final_torbox_response(
    state: &Arc<AppState>,
    response: &mut super::types::SmartSearchResponse,
    deadline: tokio::time::Instant,
) -> TorboxEnrichment {
    enrich_final_torbox_response_with(response, deadline, |hashes| async move {
        crate::sources::torbox::cached_hits_until(state, &hashes, deadline).await
    })
    .await
}

pub(crate) fn final_torbox_source_state_from_settings(
    enabled: bool,
    show_cached_as_source: bool,
    api_key: &str,
    enrichment: TorboxEnrichment,
) -> super::types::SmartSearchSourceState {
    match enrichment.status {
        None => super::torbox_configuration_state(
            enabled,
            show_cached_as_source,
            api_key,
            enrichment.cached_count,
        ),
        Some(crate::sources::torbox::CacheLookupError::Partial) => super::source_state(
            "partial",
            Some(format!(
                "TorBox cache check timed out; {} cached result{} retained",
                enrichment.cached_count,
                if enrichment.cached_count == 1 {
                    ""
                } else {
                    "s"
                }
            )),
        ),
        Some(crate::sources::torbox::CacheLookupError::Unavailable) => super::source_state(
            "unavailable",
            Some(format!(
                "TorBox cache check failed; {} cached result{} retained",
                enrichment.cached_count,
                if enrichment.cached_count == 1 {
                    ""
                } else {
                    "s"
                }
            )),
        ),
    }
}

fn final_torbox_source_state(
    state: &Arc<AppState>,
    enrichment: TorboxEnrichment,
) -> super::types::SmartSearchSourceState {
    let settings = crate::api::settings::load_torbox_settings_public(state);
    final_torbox_source_state_from_settings(
        settings.enabled,
        settings.show_cached_as_source,
        &settings.api_key,
        enrichment,
    )
}

fn deadline_smart_response(
    intent: &SmartSearchIntent,
    contract: RequestContract,
) -> SmartSearchResponse {
    let (mode, kind) = intent.mode_kind();
    let explicit = matches!(contract, RequestContract::Explicit);
    let mut sources = super::types::SmartSearchSourceStatus::new();
    sources.insert(
        crate::sources::model::SourceKind::Fshare,
        super::types::SmartSearchSourceState {
            state: "partial".into(),
            message: Some("Smart Search request timed out".into()),
            addons: None,
            telemetry: None,
        },
    );
    let response = SmartSearchResponse {
        query: intent.title().to_string(),
        total_found: 0,
        r#type: if matches!(intent, SmartSearchIntent::TargetMovie { .. }) {
            "movie"
        } else if matches!(intent, SmartSearchIntent::TargetEpisode { .. }) {
            "episode"
        } else {
            "tv"
        }
        .into(),
        mode: explicit.then(|| mode.to_string()),
        kind: explicit.then_some(kind).flatten().map(str::to_string),
        groups: matches!(intent, SmartSearchIntent::TargetMovie { .. }).then(Vec::new),
        seasons: (!matches!(intent, SmartSearchIntent::TargetMovie { .. })).then(Vec::new),
        sources: Some(sources),
        season_plan: None,
    };
    response
}

async fn resolve_title_only_tv_intent(
    state: &Arc<AppState>,
    intent: SmartSearchIntent,
    deadline: tokio::time::Instant,
) -> SmartSearchIntent {
    let title = intent.title().to_string();
    let tmdb_id = match &intent {
        SmartSearchIntent::Season { tmdb_id, .. } | SmartSearchIntent::TargetEpisode { tmdb_id, .. } => tmdb_id.clone(),
        SmartSearchIntent::TargetMovie { .. } => return intent,
    };
    if tmdb_id.is_some() {
        return intent;
    }
    let Some(tmdb_id) = tokio::time::timeout_at(deadline, state.tmdb_service.resolve_tv_title_id(&title))
        .await
        .ok()
        .flatten()
        .map(serde_json::Value::from)
    else {
        return intent;
    };
    match intent {
        SmartSearchIntent::Season { title, year, tvdb_id, season, .. } => SmartSearchIntent::Season { title, year, tmdb_id: Some(tmdb_id), tvdb_id, season },
        SmartSearchIntent::TargetEpisode { title, year, tvdb_id, season, episode, .. } => SmartSearchIntent::TargetEpisode { title, year, tmdb_id: Some(tmdb_id), tvdb_id, season, episode },
        SmartSearchIntent::TargetMovie { .. } => unreachable!(),
    }
}

pub(crate) async fn execute_typed(
    state: Arc<AppState>,
    mut parsed: ParsedSmartSearchRequest,
) -> SmartSearchResponse {
    let budget = if matches!(parsed.intent, SmartSearchIntent::Season { .. }) {
        std::time::Duration::from_secs(45)
    } else {
        std::time::Duration::from_secs(30)
    };
    let deadline = tokio::time::Instant::now() + budget;
    parsed.intent = resolve_title_only_tv_intent(&state, parsed.intent, deadline).await;
    let legacy_request = parsed.intent.legacy_request();
    let season_future = build_season_plan(&state, &parsed.intent);
    let legacy_future = async {
        match &parsed.intent {
            SmartSearchIntent::TargetMovie { .. } => {
                super::handle_movie_search_until(
                    Arc::clone(&state),
                    legacy_request,
                    deadline,
                    false,
                )
                .await
            }
            SmartSearchIntent::Season { .. } | SmartSearchIntent::TargetEpisode { .. } => {
                super::handle_tv_search_until(Arc::clone(&state), legacy_request, deadline, false)
                    .await
            }
        }
    };
    let branches = run_branches_until(season_future, legacy_future, deadline).await;
    let season_outcome = branches
        .season
        .unwrap_or_else(|| SeasonPlanOutcome::new(SeasonPlanState::AddonUnavailable, None));
    let legacy_timed_out = branches.legacy.is_none();
    let legacy_response = branches
        .legacy
        .unwrap_or_else(|| deadline_smart_response(&parsed.intent, parsed.contract));

    let aggregated = aggregate_legacy_response(legacy_response, &parsed.intent);
    let mut response = serialize_result(aggregated, &parsed.intent, parsed.contract);
    response.season_plan = season_outcome.plan;
    if matches!(parsed.intent, SmartSearchIntent::Season { .. })
        && !season_outcome.seasons.is_empty()
    {
        merge_season_groups(
            response.seasons.get_or_insert_with(Vec::new),
            season_outcome.seasons.clone(),
        );
        response.total_found = response
            .seasons
            .iter()
            .flatten()
            .flat_map(|season| &season.episodes_grouped)
            .map(|episode| episode.files.len())
            .sum();
    }
    let torbox_started = std::time::Instant::now();
    let torbox = enrich_final_torbox_response(&state, &mut response, deadline).await;
    tracing::info!(
        phase = "torbox",
        elapsed_ms = torbox_started.elapsed().as_millis() as u64,
        "Smart Search phase timing"
    );
    response
        .sources
        .get_or_insert_with(Default::default)
        .insert(
            crate::sources::model::SourceKind::Torbox,
            final_torbox_source_state(&state, torbox),
        );
    if let Some(seasons) = response.seasons.as_mut() {
        lift_torrent_packages(seasons);
        response.total_found = seasons
            .iter()
            .map(|season| {
                season.packages.len()
                    + season
                        .episodes_grouped
                        .iter()
                        .map(|episode| episode.files.len())
                        .sum::<usize>()
            })
            .sum();
    }
    if matches!(parsed.intent, SmartSearchIntent::Season { .. }) {
        let (state, message) = season_status(season_outcome.state);
        let source = response
            .sources
            .get_or_insert_with(Default::default)
            .entry(crate::sources::model::SourceKind::Stremio)
            .or_insert_with(|| super::types::SmartSearchSourceState {
                state: state.into(),
                message: Some(message.into()),
                addons: Some(season_outcome.addons.clone()),
                telemetry: None,
            });
        source.state = state.into();
        source.message = Some(message.into());
        source.addons = Some(season_outcome.addons);
        source.telemetry = season_outcome.telemetry.map(Into::into);
    }

    tracing::info!(
        phase = "total",
        elapsed_ms = budget
            .saturating_sub(deadline.saturating_duration_since(tokio::time::Instant::now()))
            .as_millis() as u64,
        timed_out = legacy_timed_out,
        "Smart Search phase timing"
    );
    response
}

pub async fn execute(
    state: Arc<AppState>,
    parsed: ParsedSmartSearchRequest,
) -> axum::response::Response {
    let legacy = matches!(parsed.contract, RequestContract::Legacy);
    let response = execute_typed(state, parsed).await;
    let mut response = axum::Json(response).into_response();
    if legacy {
        let headers = response.headers_mut();
        headers.insert("Deprecation", axum::http::HeaderValue::from_static("true"));
        headers.insert(
            "X-Flasharr-SmartSearch-Compat",
            axum::http::HeaderValue::from_static("legacy"),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::smart_search::types::SmartSearchResponse;
    use crate::sources::model::SourceKind;
    use serde_json::json;

    #[tokio::test]
    async fn completed_legacy_branch_survives_season_deadline() {
        let branches = run_branches_until(
            async { std::future::pending::<()>().await },
            async { "legacy" },
            tokio::time::Instant::now() + std::time::Duration::from_millis(10),
        )
        .await;

        assert_eq!(branches.season, None);
        assert_eq!(branches.legacy, Some("legacy"));
    }

    #[tokio::test]
    async fn completed_season_branch_survives_legacy_deadline() {
        let branches = run_branches_until(
            async { "season" },
            async { std::future::pending::<()>().await },
            tokio::time::Instant::now() + std::time::Duration::from_millis(10),
        )
        .await;

        assert_eq!(branches.season, Some("season"));
        assert_eq!(branches.legacy, None);
    }

    #[test]
    fn deadline_response_is_atomic_and_partial() {
        let intent = SmartSearchIntent::TargetMovie {
            title: "Movie".into(),
            year: None,
            tmdb_id: None,
        };
        let response = deadline_smart_response(&intent, RequestContract::Explicit);
        assert_eq!(response.total_found, 0);
        assert_eq!(
            response.sources.unwrap()[&SourceKind::Fshare].state,
            "partial"
        );
    }

    #[tokio::test]
    async fn season_and_legacy_branches_overlap_before_either_completes() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let active = Arc::new(AtomicUsize::new(0));
        let both_active = Arc::new(tokio::sync::Barrier::new(3));
        let release = Arc::new(tokio::sync::Notify::new());
        let branch = || {
            let active = Arc::clone(&active);
            let both_active = Arc::clone(&both_active);
            let release = Arc::clone(&release);
            async move {
                active.fetch_add(1, Ordering::SeqCst);
                both_active.wait().await;
                release.notified().await;
                active.fetch_sub(1, Ordering::SeqCst);
            }
        };

        let execution = tokio::spawn(run_branches_until(
            branch(),
            branch(),
            tokio::time::Instant::now() + std::time::Duration::from_secs(1),
        ));
        both_active.wait().await;
        assert_eq!(active.load(Ordering::SeqCst), 2);
        release.notify_waiters();
        execution.await.unwrap();
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn final_torbox_enrichment_once_updates_movie_and_stremio_episode_rows() {
        let mut movie = stream_result("movie-indexer", "Movie", 1);
        movie.provider = Some("Movie".into());
        movie.origin_provider = None;
        movie.info_hash = Some("A".repeat(40));
        let mut episode = stream_result("stremio-addon", "Stremio", 1);
        episode.info_hash = Some("a".repeat(40));
        let mut response = SmartSearchResponse {
            query: "Show".into(),
            total_found: 2,
            r#type: "tv".into(),
            mode: None,
            kind: None,
            groups: Some(vec![crate::utils::title_matcher::QualityGroup {
                quality: "1080p".into(),
                score: 0,
                count: 1,
                files: vec![movie],
            }]),
            seasons: Some(vec![super::super::SeasonGroup {
                season: 1,
                episode_count: 1,
                aired_episode_count: 1,
                packages: vec![],
                episodes_grouped: vec![super::super::EpisodeGroup {
                    episode_number: 2,
                    name: "Episode 2".into(),
                    overview: None,
                    air_date: None,
                    still_path: None,
                    files: vec![episode],
                }],
            }]),
            sources: None,
            season_plan: None,
        };
        let lookups = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = enrich_final_torbox_response_with(
            &mut response,
            tokio::time::Instant::now() + std::time::Duration::from_secs(1),
            {
                let lookups = Arc::clone(&lookups);
                move |hashes| {
                    lookups.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    assert_eq!(hashes, vec!["a".repeat(40)]);
                    async move {
                        crate::sources::torbox::CacheLookupOutcome {
                            hits: std::collections::HashMap::from([(
                                "a".repeat(40),
                                crate::torbox::client::TorBoxCacheHit {
                                    hash: "a".repeat(40),
                                    name: None,
                                    size: 0,
                                    files: vec![
                                        crate::torbox::client::TorBoxFile {
                                            id: 1,
                                            name: "wrong.mkv".into(),
                                            size: 1,
                                            mimetype: None,
                                        },
                                        crate::torbox::client::TorBoxFile {
                                            id: 2,
                                            name: "exact.mkv".into(),
                                            size: 99,
                                            mimetype: None,
                                        },
                                    ],
                                },
                            )]),
                            status: None,
                        }
                    }
                }
            },
        )
        .await;

        assert_eq!(count.cached_count(), 2);
        assert_eq!(count.status(), None);
        assert_eq!(lookups.load(std::sync::atomic::Ordering::SeqCst), 1);
        for row in response
            .groups
            .iter()
            .flatten()
            .flat_map(|group| &group.files)
            .chain(
                response
                    .seasons
                    .iter()
                    .flatten()
                    .flat_map(|season| &season.episodes_grouped)
                    .flat_map(|episode| &episode.files),
            )
        {
            assert_eq!(row.name, "exact.mkv");
            assert_eq!(row.size, 99);
            assert_eq!(row.cached, Some(true));
            assert_eq!(row.provider.as_deref(), Some("torbox"));
            assert_eq!(row.result_source.as_deref(), Some("debrid"));
            assert_eq!(
                row.behavior_hints
                    .as_ref()
                    .and_then(|hints| hints.filename.as_deref()),
                Some("exact.mkv")
            );
        }
        assert_eq!(
            response.groups.as_ref().unwrap()[0].files[0]
                .origin_provider
                .as_deref(),
            Some("Movie")
        );
        assert_eq!(
            response.seasons.as_ref().unwrap()[0].episodes_grouped[0].files[0]
                .origin_provider
                .as_deref(),
            Some("stremio")
        );
        let seasons = response.seasons.as_mut().unwrap();
        lift_torrent_packages(seasons);
        assert_eq!(seasons[0].packages[0].result.name, "exact.mkv");
        assert_eq!(seasons[0].packages[0].file_indices[&2], 1);
    }

    #[tokio::test]
    async fn final_torbox_timeout_or_error_preserves_response_rows() {
        let response = || SmartSearchResponse {
            query: "Show".into(),
            total_found: 1,
            r#type: "tv".into(),
            mode: None,
            kind: None,
            groups: None,
            seasons: Some(vec![super::super::SeasonGroup {
                season: 1,
                episode_count: 1,
                aired_episode_count: 1,
                packages: vec![],
                episodes_grouped: vec![super::super::EpisodeGroup {
                    episode_number: 2,
                    name: "Episode 2".into(),
                    overview: None,
                    air_date: None,
                    still_path: None,
                    files: vec![stream_result("addon", "Stremio", 1)],
                }],
            }]),
            sources: None,
            season_plan: None,
        };
        let mut error = response();
        assert_eq!(
            enrich_final_torbox_response_with(
                &mut error,
                tokio::time::Instant::now() + std::time::Duration::from_secs(1),
                |_| async {
                    crate::sources::torbox::CacheLookupOutcome {
                        hits: Default::default(),
                        status: Some(crate::sources::torbox::CacheLookupError::Unavailable),
                    }
                }
            )
            .await
            .status(),
            Some(crate::sources::torbox::CacheLookupError::Unavailable)
        );
        let mut timeout = response();
        assert_eq!(
            enrich_final_torbox_response_with(
                &mut timeout,
                tokio::time::Instant::now() + std::time::Duration::from_millis(1),
                |_| async {
                    crate::sources::torbox::CacheLookupOutcome {
                        hits: Default::default(),
                        status: Some(crate::sources::torbox::CacheLookupError::Partial),
                    }
                }
            )
            .await
            .status(),
            Some(crate::sources::torbox::CacheLookupError::Partial)
        );
        for response in [error, timeout] {
            let row = &response.seasons.unwrap()[0].episodes_grouped[0].files[0];
            assert_eq!(row.provider.as_deref(), Some("stremio"));
            assert_eq!(row.cached, None);
        }
    }

    fn season_wire(state: SeasonPlanState) -> String {
        let (state, message) = season_status(state);
        let mut sources = super::super::types::SmartSearchSourceStatus::new();
        sources.insert(
            SourceKind::Stremio,
            super::super::types::SmartSearchSourceState {
                state: state.into(),
                message: Some(message.into()),
                addons: None,
                telemetry: None,
            },
        );
        serde_json::to_string(&SmartSearchResponse {
            query: "sentinel show".into(),
            total_found: 0,
            r#type: "tv".into(),
            mode: Some("season".into()),
            kind: None,
            groups: None,
            seasons: None,
            sources: Some(sources),
            season_plan: None,
        })
        .unwrap()
    }

    #[test]
    fn season_terminal_states_are_fixed_sanitized_and_empty() {
        for (outcome, expected) in [
            (SeasonPlanState::IdentityUnavailable, "identity_unavailable"),
            (SeasonPlanState::RegistryUnavailable, "registry_unavailable"),
            (SeasonPlanState::NoTargets, "no_targets"),
            (SeasonPlanState::NoResults, "no_results"),
        ] {
            let wire = season_wire(outcome);
            let value: serde_json::Value = serde_json::from_str(&wire).unwrap();
            assert_eq!(value["sources"]["stremio"]["state"], expected);
            assert!(value["sources"].get("torrent").is_none());
            assert_eq!(value["total_found"], 0);
            assert!(value.get("season_plan").is_none());
            for sentinel in [
                "https://sentinel.invalid/config",
                "sentinel-token",
                "sentinel-ciphertext",
                "magnet:?xt=urn:btih:sentinel",
            ] {
                assert!(!wire.contains(sentinel));
            }
        }
    }

    #[test]
    fn season_result_classification_covers_zero_mixed_and_healthy() {
        assert_eq!(
            season_result_state(false, 1, 0, true),
            SeasonPlanState::NoResults
        );
        assert_eq!(
            season_result_state(false, 0, 1, false),
            SeasonPlanState::AddonUnavailable
        );
        assert_eq!(
            season_result_state(true, 1, 1, false),
            SeasonPlanState::Partial
        );
        assert_eq!(season_result_state(true, 2, 0, true), SeasonPlanState::Ok);
        assert_eq!(season_status(SeasonPlanState::Partial).0, "partial");
        assert_eq!(season_status(SeasonPlanState::Ok).0, "ok");
    }

    #[test]
    fn observed_episode_comes_from_matching_season_filename() {
        assert_eq!(observed_episode(1, Some("Show.S01E02.mkv")), Some(2));
        assert_eq!(observed_episode(1, Some("Show.S02E02.mkv")), None);
        assert_eq!(observed_episode(1, None), None);
    }

    fn stream_result(
        addon_id: &str,
        addon_name: &str,
        file_idx: u32,
    ) -> crate::utils::title_matcher::SmartSearchResult {
        season_stream_result(crate::stremio::service::AggregatedStream {
            stream: crate::stremio::parser::StreamCandidate {
                info_hash: "a".repeat(40),
                file_idx: Some(file_idx),
                sources: Vec::new(),
                behavior_hints: crate::stremio::parser::StreamBehaviorHints {
                    filename: Some("Show.S01E02.1080p.mkv".into()),
                    binge_group: Some("season-pack".into()),
                    video_size: Some(42),
                },
            },
            addon_id: addon_id.into(),
            addon_name: addon_name.into(),
            addon_order: 0,
            addon_health: crate::stremio::service::AddonHealth::Healthy,
        })
    }

    #[test]
    fn season_stream_preserves_observed_filename_file_index_and_addon_id() {
        let file = stream_result("addon-id", "Same addon name", 7);

        assert_eq!(file.name, "Show.S01E02.1080p.mkv");
        assert_eq!(
            file.behavior_hints.unwrap().filename.as_deref(),
            Some(file.name.as_str())
        );
        assert_eq!(file.file_idx, Some(7));
        assert_eq!(file.origin_indexer.as_deref(), Some("addon-id"));
    }

    #[test]
    fn merge_keeps_same_hash_from_distinct_addons() {
        let mut target = vec![super::super::SeasonGroup {
            season: 1,
            episode_count: 2,
            aired_episode_count: 2,
            packages: Vec::new(),
            episodes_grouped: vec![super::super::EpisodeGroup {
                episode_number: 2,
                name: "Episode 2".into(),
                overview: None,
                air_date: None,
                still_path: None,
                files: vec![stream_result("addon-a", "Same addon name", 7)],
            }],
        }];
        merge_season_groups(
            &mut target,
            vec![super::super::SeasonGroup {
                season: 1,
                episode_count: 2,
                aired_episode_count: 2,
                packages: Vec::new(),
                episodes_grouped: vec![super::super::EpisodeGroup {
                    episode_number: 2,
                    name: "Episode 2".into(),
                    overview: None,
                    air_date: None,
                    still_path: None,
                    files: vec![stream_result("addon-b", "Same addon name", 7)],
                }],
            }],
        );

        assert_eq!(target[0].episodes_grouped[0].files.len(), 2);
        lift_torrent_packages(&mut target);
        assert_eq!(target[0].packages.len(), 2);
    }

    #[test]
    fn torrent_packages_keep_same_hash_from_distinct_addons() {
        let mut seasons = vec![super::super::SeasonGroup {
            season: 1,
            episode_count: 2,
            aired_episode_count: 2,
            packages: Vec::new(),
            episodes_grouped: vec![super::super::EpisodeGroup {
                episode_number: 2,
                name: "Episode 2".into(),
                overview: None,
                air_date: None,
                still_path: None,
                files: vec![
                    stream_result("addon-a", "Same addon name", 7),
                    stream_result("addon-b", "Same addon name", 7),
                ],
            }],
        }];

        lift_torrent_packages(&mut seasons);

        assert_eq!(seasons[0].packages.len(), 2);
        assert_eq!(seasons[0].packages[0].file_indices[&2], 7);
        assert_eq!(seasons[0].packages[1].file_indices[&2], 7);
    }

    #[test]
    fn torrent_packages_keep_same_hash_from_distinct_binge_groups() {
        let mut first = stream_result("addon-a", "Addon", 7);
        first.behavior_hints.as_mut().unwrap().binge_group = Some("group-a".into());
        let mut second = stream_result("addon-a", "Addon", 7);
        second.behavior_hints.as_mut().unwrap().binge_group = Some("group-b".into());
        let mut seasons = vec![super::super::SeasonGroup {
            season: 1,
            episode_count: 2,
            aired_episode_count: 2,
            packages: Vec::new(),
            episodes_grouped: vec![super::super::EpisodeGroup {
                episode_number: 2,
                name: "Episode 2".into(),
                overview: None,
                air_date: None,
                still_path: None,
                files: vec![first, second],
            }],
        }];

        lift_torrent_packages(&mut seasons);

        assert_eq!(seasons[0].packages.len(), 2);
    }

    #[test]
    fn movie_and_episode_intents_remain_non_season() {
        for body in [
            json!({"mode":"target","kind":"movie","title":"Movie"}),
            json!({"mode":"target","kind":"episode","title":"Show","season":1,"episode":2}),
        ] {
            let parsed = super::super::types::parse_smart_search_request(body).unwrap();
            assert!(!matches!(parsed.intent, SmartSearchIntent::Season { .. }));
        }
    }
}
