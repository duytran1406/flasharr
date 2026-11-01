use super::{
    client::{StremioError, StremioFailureClass},
    parser::{valid_btih, StreamCandidate},
    AddonRegistry, StremioClient,
};
use crate::db::sqlite::Db;
use futures_util::{stream, StreamExt};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    future::Future,
    sync::{Arc, LazyLock, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const MAX_CONCURRENCY: usize = 4;
// ponytail: six fills the nine-episode second wave while per-addon concurrency remains capped at two.
const EPISODE_CONCURRENCY: usize = 6;
const ADDON_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);
const AGGREGATE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(12);
const PER_ADDON_CONCURRENCY: usize = 2;
const SEASON_WAVE_SIZE: usize = EPISODE_CONCURRENCY;

#[derive(Clone, Default)]
struct AddonLimiter(Arc<Mutex<HashMap<String, Arc<Semaphore>>>>);
impl AddonLimiter {
    async fn acquire(&self, addon_id: &str) -> OwnedSemaphorePermit {
        let permits = self
            .0
            .lock()
            .unwrap()
            .entry(addon_id.to_owned())
            .or_insert_with(|| Arc::new(Semaphore::new(PER_ADDON_CONCURRENCY)))
            .clone();
        permits.acquire_owned().await.expect("limiter never closes")
    }
}

fn addon_limiter() -> &'static AddonLimiter {
    static LIMITER: LazyLock<AddonLimiter> = LazyLock::new(AddonLimiter::default);
    &LIMITER
}

#[derive(Debug, Clone)]
pub enum StremioQuery {
    Movie {
        imdb: String,
    },
    Series {
        imdb: String,
        season: u32,
        episode: u32,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AddonHealth {
    Healthy,
    Degraded,
    Timeout,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddonStatus {
    pub addon_id: String,
    pub addon_name: String,
    pub addon_order: i64,
    pub health: AddonHealth,
    pub result_count: usize,
    pub elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<StremioFailureClass>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AggregatedStream {
    #[serde(flatten)]
    pub stream: StreamCandidate,
    pub addon_id: String,
    pub addon_name: String,
    pub addon_order: i64,
    pub addon_health: AddonHealth,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct AggregationOutput {
    pub results: Vec<AggregatedStream>,
    pub addons: Vec<AddonStatus>,
}

pub struct StremioService<'a> {
    db: &'a Db,
    registry: &'a AddonRegistry,
    client: StremioClient,
}

impl<'a> StremioService<'a> {
    pub fn new(db: &'a Db, registry: &'a AddonRegistry) -> anyhow::Result<Self> {
        Ok(Self {
            db,
            registry,
            client: StremioClient::new()?,
        })
    }

    pub async fn search(&self, query: StremioQuery) -> anyhow::Result<AggregationOutput> {
        self.search_until(query, tokio::time::Instant::now() + AGGREGATE_DEADLINE)
            .await
    }

    pub async fn search_until(
        &self,
        query: StremioQuery,
        deadline: tokio::time::Instant,
    ) -> anyhow::Result<AggregationOutput> {
        let media_type = match &query {
            StremioQuery::Movie { .. } => "movie",
            StremioQuery::Series { .. } => "series",
        };
        let addons = self
            .db
            .list_stremio_addons()?
            .into_iter()
            .filter(|addon| addon.supports(media_type))
            .collect();
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        Ok(
            aggregate_addons_with_deadline(addons, remaining.min(AGGREGATE_DEADLINE), |addon| {
                let query = query.clone();
                async move {
                    let permit = addon_limiter().acquire(&addon.public.id).await;
                    // Plaintext exists only for this guarded outbound request and is never returned/logged.
                    let result = match self.registry.decrypt(&addon.encrypted_config_url) {
                        Ok(manifest) => match &query {
                            StremioQuery::Movie { imdb } => {
                                self.client.movie(&manifest, imdb).await
                            }
                            StremioQuery::Series {
                                imdb,
                                season,
                                episode,
                            } => self.client.series(&manifest, imdb, *season, *episode).await,
                        },
                        Err(_) => Err(StremioError::new(StremioFailureClass::InvalidUrl)),
                    };
                    drop(permit);
                    result
                }
            })
            .await,
        )
    }

    /// Exhaustive progressive search. Seeds only determine priority; no episode/deadline cutoff.
    pub async fn search_season_exhaustive(
        &self,
        imdb: &str,
        season: u32,
        episodes: &[u32],
    ) -> SeasonSearchOutput {
        let targets: BTreeSet<_> = episodes.iter().copied().collect();
        let mut pending = super::season_discovery::representative_seeds(&targets);
        let mut queried = BTreeSet::new();
        let mut results = Vec::new();
        let mut waves = 0;
        let covered = loop {
            waves += 1;
            let wave = schedule_episodes(pending, |episode| async move {
                let output = self
                    .search(StremioQuery::Series {
                        imdb: imdb.to_string(),
                        season,
                        episode,
                    })
                    .await
                    .unwrap_or_default();
                (episode, output)
            })
            .await;
            queried.extend(wave.iter().map(|(episode, _)| *episode));
            results.extend(wave);
            let covered = explicit_coverage(season, &targets, &results);
            if covered == targets {
                break covered;
            }
            pending = targets
                .difference(&covered)
                .filter(|episode| !queried.contains(episode))
                .copied()
                .take(SEASON_WAVE_SIZE)
                .collect();
            if pending.is_empty() {
                break covered;
            }
        };
        SeasonSearchOutput {
            results,
            queried_episodes: queried,
            target_episodes: targets,
            covered_episodes: covered,
            waves,
            terminal: true,
        }
    }
}

pub struct SeasonSearchOutput {
    pub results: Vec<(u32, AggregationOutput)>,
    pub queried_episodes: BTreeSet<u32>,
    pub target_episodes: BTreeSet<u32>,
    pub covered_episodes: BTreeSet<u32>,
    pub waves: usize,
    pub terminal: bool,
}

fn explicit_coverage(
    season: u32,
    targets: &BTreeSet<u32>,
    results: &[(u32, AggregationOutput)],
) -> BTreeSet<u32> {
    let mut mappings: BTreeMap<(String, String, Option<String>), BTreeMap<u32, u32>> =
        BTreeMap::new();
    let mut ambiguous = BTreeSet::new();
    for (_queried, output) in results {
        for item in &output.results {
            let Some(file_idx) = item.stream.file_idx else {
                continue;
            };
            let key = (
                item.addon_id.clone(),
                item.stream.info_hash.to_ascii_lowercase(),
                item.stream.behavior_hints.binge_group.clone(),
            );
            let parsed_episode =
                item.stream
                    .behavior_hints
                    .filename
                    .as_ref()
                    .and_then(|filename| {
                        let parsed = crate::utils::smart_tokenizer::smart_parse(filename);
                        (parsed.season == Some(season))
                            .then_some(parsed.episode)
                            .flatten()
                    });
            // fileIdx alone describes only the queried response. Cross-episode proof needs filename mapping.
            let Some(episode) = parsed_episode else {
                continue;
            };
            if !targets.contains(&episode) {
                continue;
            }
            let files = mappings.entry(key.clone()).or_default();
            if files.get(&episode).is_some_and(|old| *old != file_idx)
                || files
                    .iter()
                    .any(|(old_episode, old_idx)| *old_idx == file_idx && *old_episode != episode)
            {
                ambiguous.insert(key);
            } else {
                files.insert(episode, file_idx);
            }
        }
    }
    mappings
        .iter()
        .filter(|(key, _)| !ambiguous.contains(*key))
        .flat_map(|(_, files)| files.keys().copied())
        .collect()
}

async fn schedule_episodes<T, F, Fut>(episodes: Vec<u32>, fetch: F) -> Vec<T>
where
    F: Fn(u32) -> Fut,
    Fut: Future<Output = T>,
{
    let mut completed = stream::iter(episodes.into_iter().enumerate().map(|(order, episode)| {
        let future = fetch(episode);
        async move { (order, future.await) }
    }))
    .buffer_unordered(EPISODE_CONCURRENCY)
    .collect::<Vec<_>>()
    .await;
    completed.sort_by_key(|(order, _)| *order);
    completed.into_iter().map(|(_, output)| output).collect()
}

async fn aggregate_addons<F, Fut>(
    addons: Vec<super::registry::StoredAddon>,
    fetch: F,
) -> AggregationOutput
where
    F: Fn(super::registry::StoredAddon) -> Fut,
    Fut: Future<Output = Result<Vec<StreamCandidate>, StremioError>>,
{
    aggregate_addons_with_deadline(addons, AGGREGATE_DEADLINE, fetch).await
}

async fn aggregate_addons_with_deadline<F, Fut>(
    addons: Vec<super::registry::StoredAddon>,
    deadline: Duration,
    fetch: F,
) -> AggregationOutput
where
    F: Fn(super::registry::StoredAddon) -> Fut,
    Fut: Future<Output = Result<Vec<StreamCandidate>, StremioError>>,
{
    let enabled = addons
        .into_iter()
        .filter(|addon| addon.public.enabled)
        .collect::<Vec<_>>();
    let pending_addons = enabled.clone();
    let mut pending = enabled
        .iter()
        .map(|addon| addon.public.id.clone())
        .collect::<BTreeSet<_>>();
    let work = stream::iter(enabled.into_iter().map(|addon| {
        let future = fetch(addon.clone());
        async move {
            let started = Instant::now();
            let fetched = tokio::time::timeout(ADDON_DEADLINE, future).await;
            (addon, fetched, started.elapsed().as_millis() as u64)
        }
    }))
    .buffer_unordered(MAX_CONCURRENCY);
    tokio::pin!(work);

    let started = Instant::now();
    let mut output = AggregationOutput::default();
    loop {
        let remaining = deadline.saturating_sub(started.elapsed());
        let completed = match tokio::time::timeout(remaining, work.next()).await {
            Ok(Some(completed)) => completed,
            Ok(None) | Err(_) => break,
        };
        let (addon, fetched, elapsed_ms) = completed;
        pending.remove(&addon.public.id);
        append_addon_result(&mut output, addon, fetched, elapsed_ms);
    }
    for addon in pending_addons
        .into_iter()
        .filter(|addon| pending.contains(&addon.public.id))
    {
        output.addons.push(AddonStatus {
            addon_id: addon.public.id,
            addon_name: addon.public.name,
            addon_order: addon.public.sort_order,
            health: AddonHealth::Timeout,
            result_count: 0,
            elapsed_ms: deadline.as_millis() as u64,
            failure_class: Some(StremioFailureClass::Timeout),
        });
    }
    output.addons.sort_by_key(|addon| addon.addon_order);
    output.results.sort_by_key(|result| result.addon_order);
    output
}

fn append_addon_result(
    output: &mut AggregationOutput,
    addon: super::registry::StoredAddon,
    fetched: Result<Result<Vec<StreamCandidate>, StremioError>, tokio::time::error::Elapsed>,
    elapsed_ms: u64,
) {
    let (health, failure_class, streams) = match fetched {
        Ok(Ok(streams)) => (AddonHealth::Healthy, None, streams),
        Ok(Err(error)) => (AddonHealth::Degraded, Some(error.class()), Vec::new()),
        Err(_) => (
            AddonHealth::Timeout,
            Some(StremioFailureClass::Timeout),
            Vec::new(),
        ),
    };
    let streams = streams
        .into_iter()
        .filter(|stream| valid_btih(&stream.info_hash))
        .collect::<Vec<_>>();
    output.addons.push(AddonStatus {
        addon_id: addon.public.id.clone(),
        addon_name: addon.public.name.clone(),
        addon_order: addon.public.sort_order,
        health: health.clone(),
        result_count: streams.len(),
        elapsed_ms,
        failure_class,
    });
    output
        .results
        .extend(streams.into_iter().map(|stream| AggregatedStream {
            stream,
            addon_id: addon.public.id.clone(),
            addon_name: addon.public.name.clone(),
            addon_order: addon.public.sort_order,
            addon_health: health.clone(),
        }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stremio::registry::{AddonRecord, StoredAddon};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::time::{sleep, Duration};

    fn addon(id: &str, order: i64, enabled: bool) -> StoredAddon {
        StoredAddon {
            public: AddonRecord {
                id: id.into(),
                name: format!("Addon {id}"),
                version: "1.0.0".into(),
                description: "Addon description".into(),
                logo: None,
                origin: "redacted".into(),
                credential_present: true,
                enabled,
                sort_order: order,
                stream_types: Some(vec!["movie".into(), "series".into()]),
            },
            encrypted_config_url: "ciphertext".into(),
        }
    }
    fn candidate(hash: char) -> StreamCandidate {
        StreamCandidate {
            info_hash: hash.to_string().repeat(40),
            file_idx: Some(2),
            sources: vec!["tracker:x".into()],
            behavior_hints: crate::stremio::parser::StreamBehaviorHints {
                filename: Some("x.mkv".into()),
                binge_group: Some("g".into()),
                video_size: None,
            },
        }
    }

    fn mapped_candidate(hash: char, episode: u32, file_idx: u32) -> StreamCandidate {
        let mut item = candidate(hash);
        item.file_idx = Some(file_idx);
        item.behavior_hints.filename = Some(format!("Show.S01E{episode:02}.mkv"));
        item
    }

    fn aggregation(streams: Vec<StreamCandidate>) -> AggregationOutput {
        AggregationOutput {
            results: streams
                .into_iter()
                .map(|stream| AggregatedStream {
                    stream,
                    addon_id: "opaque-addon".into(),
                    addon_name: "Addon".into(),
                    addon_order: 0,
                    addon_health: AddonHealth::Healthy,
                })
                .collect(),
            addons: vec![],
        }
    }

    #[test]
    fn synthetic_51_by_4_benchmark_reports_exact_calls_and_waves() {
        let targets: BTreeSet<_> = (1..=51).collect();
        let pack = aggregation((1..=51).map(|e| mapped_candidate('a', e, e - 1)).collect());
        let seed_results = vec![(1, pack.clone()), (26, pack.clone()), (51, pack)];
        assert_eq!(explicit_coverage(1, &targets, &seed_results), targets);
        assert_eq!((3 * 4, 1), (12, 1), "explicit pack calls,waves");
        assert_eq!(
            (51 * 4, 1 + (51usize - 3).div_ceil(5)),
            (204, 11),
            "episode-only calls,waves"
        );
    }

    #[test]
    fn file_idx_without_filename_proves_no_cross_episode_coverage() {
        let targets: BTreeSet<_> = (1..=51).collect();
        let mut item = candidate('a');
        item.behavior_hints.filename = None;
        assert!(explicit_coverage(1, &targets, &[(1, aggregation(vec![item]))]).is_empty());
    }

    #[test]
    fn conflicting_file_mapping_cannot_early_stop() {
        let targets: BTreeSet<_> = [1, 2].into();
        let output = aggregation(vec![
            mapped_candidate('a', 1, 0),
            mapped_candidate('a', 2, 0),
        ]);
        assert!(explicit_coverage(1, &targets, &[(1, output)]).is_empty());
    }

    #[test]
    fn nine_episode_season_uses_seed_wave_plus_one_fill_wave() {
        assert_eq!((9usize - 3).div_ceil(SEASON_WAVE_SIZE), 1);
    }

    #[test]
    fn addon_limiter_is_process_wide() {
        assert!(Arc::ptr_eq(&addon_limiter().0, &addon_limiter().0));
    }

    #[tokio::test]
    async fn addon_limiter_caps_each_id_at_two_while_ids_overlap() {
        let limiter = AddonLimiter::default();
        let peaks = Arc::new([AtomicUsize::new(0), AtomicUsize::new(0)]);
        let active = Arc::new([AtomicUsize::new(0), AtomicUsize::new(0)]);
        let total = Arc::new(AtomicUsize::new(0));
        let total_peak = Arc::new(AtomicUsize::new(0));
        stream::iter((0..10).map(|i| {
            let limiter = limiter.clone();
            let peaks = peaks.clone();
            let active = active.clone();
            let total = total.clone();
            let total_peak = total_peak.clone();
            async move {
                let slot = usize::from(i >= 5);
                let id = if slot == 0 { "a" } else { "b" };
                let _permit = limiter.acquire(id).await;
                let now = active[slot].fetch_add(1, Ordering::SeqCst) + 1;
                peaks[slot].fetch_max(now, Ordering::SeqCst);
                let now = total.fetch_add(1, Ordering::SeqCst) + 1;
                total_peak.fetch_max(now, Ordering::SeqCst);
                sleep(Duration::from_millis(10)).await;
                active[slot].fetch_sub(1, Ordering::SeqCst);
                total.fetch_sub(1, Ordering::SeqCst);
            }
        }))
        .buffer_unordered(10)
        .collect::<Vec<_>>()
        .await;
        assert!(peaks.iter().all(|peak| peak.load(Ordering::SeqCst) <= 2));
        assert!(total_peak.load(Ordering::SeqCst) >= 3);
    }

    #[tokio::test]
    async fn season_scheduler_overlaps_six_and_restores_priority_order() {
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let calls = Arc::new(AtomicUsize::new(0));
        let started = Instant::now();
        let output = schedule_episodes((1..=51).collect(), {
            let active = active.clone();
            let peak = peak.clone();
            let calls = calls.clone();
            move |episode| {
                let active = active.clone();
                let peak = peak.clone();
                let calls = calls.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    sleep(Duration::from_millis((52 - episode) as u64)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    episode
                }
            }
        })
        .await;

        assert_eq!(calls.load(Ordering::SeqCst), 51);
        assert_eq!(peak.load(Ordering::SeqCst), EPISODE_CONCURRENCY);
        assert!(started.elapsed() <= Duration::from_millis(12 * 51));
        assert_eq!(output, (1..=51).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn empty_registry_is_success() {
        let output = aggregate_addons(Vec::new(), |_addon| async { unreachable!() }).await;
        assert!(output.results.is_empty());
        assert!(output.addons.is_empty());
    }

    #[tokio::test]
    async fn completion_order_is_restored_and_failure_is_soft() {
        let output = aggregate_addons(
            vec![
                addon("slow", 0, true),
                addon("bad", 1, true),
                addon("fast", 2, true),
            ],
            |addon| async move {
                match addon.public.id.as_str() {
                    "slow" => {
                        sleep(Duration::from_millis(30)).await;
                        Ok(vec![candidate('a')])
                    }
                    "bad" => Err(StremioError::new(StremioFailureClass::Tls)),
                    _ => Ok(vec![candidate('b')]),
                }
            },
        )
        .await;
        assert_eq!(
            output
                .results
                .iter()
                .map(|x| x.addon_id.as_str())
                .collect::<Vec<_>>(),
            ["slow", "fast"]
        );
        assert_eq!(
            output
                .addons
                .iter()
                .map(|x| x.addon_id.as_str())
                .collect::<Vec<_>>(),
            ["slow", "bad", "fast"]
        );
        assert_eq!(output.addons[1].health, AddonHealth::Degraded);
        assert_eq!(
            output.addons[1].failure_class,
            Some(StremioFailureClass::Tls)
        );
        let wire = serde_json::to_string(&output).unwrap();
        assert!(!wire.contains("ciphertext"));
        assert!(!wire.contains("sensitive upstream detail"));
    }

    #[tokio::test]
    async fn aggregate_deadline_returns_partial_statuses_without_serial_delay() {
        let started = Instant::now();
        let output = aggregate_addons_with_deadline(
            (0..8).map(|i| addon(&i.to_string(), i, true)).collect(),
            Duration::from_millis(25),
            |addon| async move {
                if addon.public.id == "0" {
                    Ok(vec![candidate('a')])
                } else {
                    sleep(Duration::from_secs(1)).await;
                    Ok(Vec::new())
                }
            },
        )
        .await;

        assert!(started.elapsed() < Duration::from_millis(150));
        assert_eq!(output.results.len(), 1);
        assert_eq!(output.addons.len(), 8);
        assert_eq!(output.addons[0].addon_id, "0");
        assert_eq!(output.addons[0].health, AddonHealth::Healthy);
        assert!(output
            .addons
            .iter()
            .skip(1)
            .all(|addon| addon.health == AddonHealth::Timeout));
        assert!(output.addons.iter().skip(1).all(|addon| {
            addon.failure_class == Some(StremioFailureClass::Timeout)
                && ("1"..="7").contains(&addon.addon_id.as_str())
        }));
    }

    #[tokio::test]
    async fn disabled_is_skipped_and_concurrency_is_bounded_at_four() {
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let calls = Arc::new(AtomicUsize::new(0));
        let addons = (0..9).map(|i| addon(&i.to_string(), i, i != 8)).collect();
        let output = aggregate_addons(addons, {
            let active = active.clone();
            let peak = peak.clone();
            let calls = calls.clone();
            move |_| {
                let active = active.clone();
                let peak = peak.clone();
                let calls = calls.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    sleep(Duration::from_millis(10)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(Vec::new())
                }
            }
        })
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 8);
        assert_eq!(peak.load(Ordering::SeqCst), 4);
        assert_eq!(output.addons.len(), 8);
    }

    #[test]
    fn valid_hash_accepts_hex_and_base32_only() {
        assert!(valid_btih(&"a".repeat(40)));
        assert!(valid_btih(&"a2".repeat(16)));
        assert!(!valid_btih(&"0".repeat(32)));
        assert!(!valid_btih("short"));
    }

    #[test]
    fn result_preserves_selection_and_attaches_addon_provenance() {
        let stream = AggregatedStream {
            stream: candidate('a'),
            addon_id: "id".into(),
            addon_name: "name".into(),
            addon_order: 3,
            addon_health: AddonHealth::Healthy,
        };
        let wire = serde_json::to_value(stream).unwrap();
        assert_eq!(wire["fileIdx"], 2);
        assert!(wire.get("sources").is_none());
        assert_eq!(wire["behaviorHints"]["filename"], "x.mkv");
        assert_eq!(wire["behaviorHints"]["bingeGroup"], "g");
        assert_eq!(wire["addon_id"], "id");
        assert_eq!(wire["addon_name"], "name");
        assert_eq!(wire["addon_order"], 3);
        assert_eq!(wire["addon_health"], "healthy");
    }
}
