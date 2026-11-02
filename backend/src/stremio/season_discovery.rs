//! Pure, bounded seed-first season discovery. No acquisition side effects.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReleaseKey {
    pub provider: String,
    pub info_hash: String,
    pub binge_group: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub provider: String,
    pub info_hash: String,
    pub binge_group: Option<String>,
    pub file_idx: Option<u32>,
    /// Episode inferred from a trusted filename/parser. `None` is not guessed.
    pub observed_episode: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub request_budget: usize,
    pub wave_size: usize,
    pub coverage_target_bps: u16,
    pub consistency_target_bps: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    TargetReached,
    Exhausted,
    Budget,
}

#[derive(Debug, Clone)]
pub struct Discovery {
    pub queried_episodes: BTreeSet<u32>,
    pub requests: usize,
    pub covered: BTreeSet<u32>,
    pub selected: Vec<ReleaseKey>,
    pub ambiguous: BTreeSet<ReleaseKey>,
    pub stop: StopReason,
}

/// Search only. Implementations may call an addon in production; tests stay deterministic.
pub trait SeedSearch {
    fn query(&mut self, provider: &str, episode: u32) -> Vec<Candidate>;
}

/// Optional, read-only torrent manifest capability. Deliberately separate from acquisition.
pub trait ManifestInspector {
    fn available(&self, provider: &str) -> bool;
    fn inspect(&mut self, key: &ReleaseKey) -> Option<Vec<(u32, u32)>>; // (file_idx, episode)
}

pub fn representative_seeds(targets: &BTreeSet<u32>) -> Vec<u32> {
    if targets.is_empty() {
        return vec![];
    }
    let values: Vec<_> = targets.iter().copied().collect();
    let mut seeds = vec![values[0], values[values.len() / 2], *values.last().unwrap()];
    seeds.sort_unstable();
    seeds.dedup();
    seeds
}

/// Request bound: `min(request_budget, A * E + U)`, where `A` providers, `E`
/// targets, `U` unique inspectable release keys. Without inspection: at most `A * E`;
/// successful seed-pack case: at most `3 * A`. CPU: `O(C log C + P * E)` for
/// `C` observations and `P` grouped releases. Memory: `O(C + E)`.
pub fn discover<S: SeedSearch, I: ManifestInspector>(
    targets: &BTreeSet<u32>,
    providers: &[String],
    config: Config,
    search: &mut S,
    inspector: &mut I,
) -> Discovery {
    let mut queried = BTreeSet::new();
    let mut groups: BTreeMap<ReleaseKey, BTreeMap<u32, u32>> = BTreeMap::new(); // episode -> file_idx
    let mut ambiguous = BTreeSet::new();
    let mut inspected = BTreeSet::new();
    let mut requests = 0;
    let mut pending = representative_seeds(targets);
    let wave = config.wave_size.max(1);

    loop {
        let mut progressed = false;
        for episode in pending.into_iter().take(wave) {
            if queried.contains(&episode) {
                continue;
            }
            for provider in providers {
                if requests == config.request_budget {
                    break;
                }
                requests += 1;
                progressed = true;
                for candidate in search.query(provider, episode) {
                    let key = ReleaseKey {
                        provider: candidate.provider,
                        info_hash: candidate.info_hash.to_ascii_lowercase(),
                        binge_group: candidate.binge_group,
                    };
                    let (Some(file_idx), Some(mapped_episode)) =
                        (candidate.file_idx, candidate.observed_episode)
                    else {
                        continue;
                    };
                    if !targets.contains(&mapped_episode) {
                        continue;
                    }
                    insert_mapping(&mut groups, &mut ambiguous, key, file_idx, mapped_episode);
                }
            }
            queried.insert(episode);
            if requests == config.request_budget {
                break;
            }
        }

        // Inspect each deduped provider+hash+bingeGroup at most once. Inspection consumes budget.
        let keys: Vec<_> = groups
            .keys()
            .filter(|k| !inspected.contains(*k))
            .cloned()
            .collect();
        for key in keys {
            if requests == config.request_budget {
                break;
            }
            inspected.insert(key.clone());
            if !inspector.available(&key.provider) {
                continue;
            }
            requests += 1;
            if let Some(files) = inspector.inspect(&key) {
                for (file_idx, episode) in files {
                    if targets.contains(&episode) {
                        insert_mapping(&mut groups, &mut ambiguous, key.clone(), file_idx, episode);
                    }
                }
            }
        }

        let (covered, selected, consistency_bps) = cover(targets, &groups, &ambiguous);
        let coverage_bps = ratio(covered.len(), targets.len());
        if coverage_bps >= config.coverage_target_bps
            && consistency_bps >= config.consistency_target_bps
        {
            return Discovery {
                queried_episodes: queried,
                requests,
                covered,
                selected,
                ambiguous,
                stop: StopReason::TargetReached,
            };
        }
        if requests == config.request_budget {
            return Discovery {
                queried_episodes: queried,
                requests,
                covered,
                selected,
                ambiguous,
                stop: StopReason::Budget,
            };
        }
        pending = targets
            .difference(&covered)
            .filter(|e| !queried.contains(e))
            .copied()
            .take(wave)
            .collect();
        if pending.is_empty() || !progressed {
            return Discovery {
                queried_episodes: queried,
                requests,
                covered,
                selected,
                ambiguous,
                stop: StopReason::Exhausted,
            };
        }
    }
}

fn insert_mapping(
    groups: &mut BTreeMap<ReleaseKey, BTreeMap<u32, u32>>,
    ambiguous: &mut BTreeSet<ReleaseKey>,
    key: ReleaseKey,
    file_idx: u32,
    episode: u32,
) {
    let files = groups.entry(key.clone()).or_default();
    if files.get(&episode).is_some_and(|old| *old != file_idx)
        || files
            .iter()
            .any(|(old_episode, old_idx)| *old_idx == file_idx && *old_episode != episode)
    {
        ambiguous.insert(key); // fail closed: entire release excluded
    } else {
        files.insert(episode, file_idx);
    }
}

fn cover(
    targets: &BTreeSet<u32>,
    groups: &BTreeMap<ReleaseKey, BTreeMap<u32, u32>>,
    ambiguous: &BTreeSet<ReleaseKey>,
) -> (BTreeSet<u32>, Vec<ReleaseKey>, u16) {
    let mut uncovered = targets.clone();
    let mut selected = vec![];
    let mut largest = 0;
    loop {
        let best = groups
            .iter()
            .filter(|(k, _)| !ambiguous.contains(*k))
            .map(|(k, files)| (files.keys().filter(|e| uncovered.contains(e)).count(), k))
            .filter(|(gain, _)| *gain > 0)
            .max_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(a.1)));
        let Some((_, key)) = best else { break };
        let episodes: BTreeSet<_> = groups[key].keys().copied().collect();
        largest = largest.max(episodes.intersection(targets).count());
        uncovered.retain(|e| !episodes.contains(e));
        selected.push(key.clone());
    }
    let covered = targets
        .difference(&uncovered)
        .copied()
        .collect::<BTreeSet<_>>();
    (covered.clone(), selected, ratio(largest, covered.len()))
}

fn ratio(n: usize, d: usize) -> u16 {
    if d == 0 {
        10_000
    } else {
        ((n * 10_000) / d) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Search {
        calls: usize,
        pack: bool,
    }
    impl SeedSearch for Search {
        fn query(&mut self, provider: &str, episode: u32) -> Vec<Candidate> {
            self.calls += 1;
            let mapped = if self.pack { 1..=12 } else { episode..=episode };
            mapped
                .map(|e| Candidate {
                    provider: provider.into(),
                    info_hash: if self.pack { "ABC" } else { "single" }.into(),
                    binge_group: Some("g".into()),
                    file_idx: Some(e - 1),
                    observed_episode: Some(e),
                })
                .collect()
        }
    }
    struct NoInspect;
    impl ManifestInspector for NoInspect {
        fn available(&self, _: &str) -> bool {
            false
        }
        fn inspect(&mut self, _: &ReleaseKey) -> Option<Vec<(u32, u32)>> {
            unreachable!()
        }
    }
    fn targets() -> BTreeSet<u32> {
        (1..=12).collect()
    }
    fn cfg(budget: usize) -> Config {
        Config {
            request_budget: budget,
            wave_size: 3,
            coverage_target_bps: 10_000,
            consistency_target_bps: 10_000,
        }
    }

    #[test]
    fn seeds_are_first_middle_latest() {
        assert_eq!(representative_seeds(&targets()), vec![1, 7, 12]);
    }
    #[test]
    fn pack_stops_after_seed_wave_and_beats_naive_episode_times_addon() {
        let mut search = Search {
            calls: 0,
            pack: true,
        };
        let result = discover(
            &targets(),
            &["a".into(), "b".into()],
            cfg(100),
            &mut search,
            &mut NoInspect,
        );
        assert_eq!(result.stop, StopReason::TargetReached);
        assert_eq!(result.requests, 6); // adaptive 3 seeds × 2 addons; naive 12 × 2 = 24
        assert_eq!(result.covered, targets());
        assert_eq!(result.selected.len(), 1); // greedy cover needs one provider's complete pack
    }
    #[test]
    fn strict_budget_returns_partial() {
        let mut search = Search {
            calls: 0,
            pack: false,
        };
        let result = discover(
            &targets(),
            &["a".into(), "b".into()],
            cfg(5),
            &mut search,
            &mut NoInspect,
        );
        assert_eq!(result.stop, StopReason::Budget);
        assert_eq!(result.requests, 5);
        assert!(result.covered.len() < targets().len());
    }
    #[test]
    fn conflicting_mapping_fails_closed() {
        struct Bad;
        impl SeedSearch for Bad {
            fn query(&mut self, provider: &str, _: u32) -> Vec<Candidate> {
                vec![1, 2]
                    .into_iter()
                    .map(|e| Candidate {
                        provider: provider.into(),
                        info_hash: "same".into(),
                        binge_group: None,
                        file_idx: Some(0),
                        observed_episode: Some(e),
                    })
                    .collect()
            }
        }
        let mut bad = Bad;
        let result = discover(
            &[1, 2].into_iter().collect(),
            &["a".into()],
            cfg(10),
            &mut bad,
            &mut NoInspect,
        );
        assert!(result.covered.is_empty());
        assert_eq!(result.ambiguous.len(), 1);
    }
}
