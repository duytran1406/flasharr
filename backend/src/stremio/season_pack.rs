use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeTarget {
    pub episode: u32,
    pub aired: bool,
    pub monitored: bool,
    pub missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeCandidate {
    pub episode: u32,
    pub addon_id: String,
    pub provider: String,
    pub info_hash: String,
    pub binge_group: Option<String>,
    pub manifest_position: usize,
    pub provider_file_id: Option<String>,
    pub file_idx: Option<u32>,
    pub identity_match: bool,
    pub mapping_match: bool,
    pub provider_allowed: bool,
    pub quality_language_match: bool,
    pub cached: bool,
    pub quality_score: i32,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackKey {
    pub addon_id: String,
    pub provider: String,
    pub info_hash: String,
    pub binge_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeFile {
    pub file_idx: u32,
    pub manifest_position: usize,
    pub provider_file_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pack {
    pub key: PackKey,
    pub episode_files: BTreeMap<u32, EpisodeFile>,
    pub cached: bool,
    pub quality_score: i32,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SelectionKey {
    pub episode: u32,
    pub pack: PackKey,
    pub file_idx: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanMode {
    Consistent,
    Fast,
    Manual,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Coverage {
    Full,
    Partial,
    Missing,
    Ambiguous,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedFile {
    pub pack: PackKey,
    pub file_idx: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonPlan {
    pub target_episodes: BTreeSet<u32>,
    pub packs: Vec<Pack>,
    pub selected: BTreeMap<u32, SelectedFile>,
    pub coverage: Coverage,
    pub discovery_status: String,
    pub queried_episodes: BTreeSet<u32>,
    pub request_bound: usize,
}

/// Pure Smart Search integration seam. Call after episode fan-out normalization.
pub fn plan_season(
    targets: &[EpisodeTarget],
    candidates: &[EpisodeCandidate],
    mode: PlanMode,
    manual: &BTreeSet<SelectionKey>,
) -> SeasonPlan {
    let target_episodes: BTreeSet<_> = targets
        .iter()
        .filter(|e| e.aired && e.monitored && e.missing)
        .map(|e| e.episode)
        .collect();
    let mut grouped: BTreeMap<PackKey, Pack> = BTreeMap::new();
    let mut ambiguous = false;
    for c in candidates.iter().filter(|c| {
        target_episodes.contains(&c.episode)
            && c.identity_match
            && c.mapping_match
            && c.provider_allowed
            && c.quality_language_match
    }) {
        let Some(file_idx) = c.file_idx else { continue };
        let provider_file_id = c.provider_file_id.clone();
        let key = PackKey {
            addon_id: c.addon_id.clone(),
            provider: c.provider.clone(),
            info_hash: c.info_hash.to_ascii_lowercase(),
            binge_group: c.binge_group.clone(),
        };
        let pack = grouped.entry(key.clone()).or_insert(Pack {
            key,
            episode_files: BTreeMap::new(),
            cached: c.cached,
            quality_score: c.quality_score,
            size_bytes: c.size_bytes,
        });
        let file = EpisodeFile {
            file_idx,
            manifest_position: c.manifest_position,
            provider_file_id,
        };
        if pack
            .episode_files
            .get(&c.episode)
            .is_some_and(|old| old != &file)
        {
            ambiguous = true;
        }
        pack.episode_files.entry(c.episode).or_insert(file);
        pack.cached &= c.cached;
        pack.quality_score = pack.quality_score.min(c.quality_score);
        pack.size_bytes = pack.size_bytes.max(c.size_bytes);
    }
    let mut packs: Vec<_> = grouped.into_values().collect();
    packs.sort_by_key(|p| {
        (
            std::cmp::Reverse(p.episode_files.len()),
            std::cmp::Reverse(consistency(p, &target_episodes)),
            std::cmp::Reverse(p.cached),
            std::cmp::Reverse(p.quality_score),
            1usize,
            p.size_bytes,
        )
    });
    let mut selected = BTreeMap::new();
    if !ambiguous {
        match mode {
            PlanMode::Manual => {
                for choice in manual {
                    if target_episodes.contains(&choice.episode)
                        && packs.iter().any(|p| {
                            p.key == choice.pack
                                && p.episode_files
                                    .get(&choice.episode)
                                    .is_some_and(|f| f.file_idx == choice.file_idx)
                        })
                    {
                        selected.insert(
                            choice.episode,
                            SelectedFile {
                                pack: choice.pack.clone(),
                                file_idx: choice.file_idx,
                            },
                        );
                    }
                }
            }
            PlanMode::Consistent => {
                if let Some(pack) = packs.first() {
                    select_pack(pack, &mut selected);
                }
            }
            PlanMode::Fast => {
                for episode in &target_episodes {
                    if let Some(pack) = packs
                        .iter()
                        .filter(|p| p.episode_files.contains_key(episode))
                        .min_by_key(|p| {
                            (!p.cached, std::cmp::Reverse(p.quality_score), p.size_bytes)
                        })
                    {
                        let file = &pack.episode_files[episode];
                        selected.insert(
                            *episode,
                            SelectedFile {
                                pack: pack.key.clone(),
                                file_idx: file.file_idx,
                            },
                        );
                    }
                }
            }
        }
    }
    let coverage = if ambiguous {
        Coverage::Ambiguous
    } else if target_episodes.is_empty() || selected.len() == target_episodes.len() {
        Coverage::Full
    } else if selected.is_empty() {
        Coverage::Missing
    } else {
        Coverage::Partial
    };
    SeasonPlan {
        queried_episodes: target_episodes.clone(),
        request_bound: target_episodes.len(),
        discovery_status: "complete".into(),
        target_episodes,
        packs,
        selected,
        coverage,
    }
}
fn consistency(pack: &Pack, targets: &BTreeSet<u32>) -> bool {
    pack.episode_files.keys().all(|e| targets.contains(e))
}
fn select_pack(pack: &Pack, selected: &mut BTreeMap<u32, SelectedFile>) {
    for (episode, file) in &pack.episode_files {
        selected.insert(
            *episode,
            SelectedFile {
                pack: pack.key.clone(),
                file_idx: file.file_idx,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn target(episode: u32) -> EpisodeTarget {
        EpisodeTarget {
            episode,
            aired: true,
            monitored: true,
            missing: true,
        }
    }
    fn candidate(episode: u32, hash: &str, file_idx: Option<u32>) -> EpisodeCandidate {
        EpisodeCandidate {
            episode,
            addon_id: "addon-p".into(),
            provider: "p".into(),
            info_hash: hash.into(),
            binge_group: Some("g".into()),
            manifest_position: episode as usize,
            provider_file_id: file_idx.map(|n| n.to_string()),
            file_idx,
            identity_match: true,
            mapping_match: true,
            provider_allowed: true,
            quality_language_match: true,
            cached: true,
            quality_score: 10,
            size_bytes: 100,
        }
    }
    #[test]
    fn aggregates_same_pack_without_losing_episode_file_mapping() {
        let p = plan_season(
            &[target(1), target(2)],
            &[candidate(1, "hash", Some(3)), candidate(2, "hash", Some(8))],
            PlanMode::Consistent,
            &BTreeSet::new(),
        );
        assert_eq!(p.packs.len(), 1);
        assert_eq!(p.packs[0].episode_files[&1].file_idx, 3);
        assert_eq!(
            p.packs[0].episode_files[&2],
            EpisodeFile {
                file_idx: 8,
                manifest_position: 2,
                provider_file_id: Some("8".into())
            }
        );
        assert_eq!(p.coverage, Coverage::Full);
    }
    #[test]
    fn distinct_addon_ids_never_merge_even_when_names_match() {
        let mut first = candidate(1, "hash", Some(3));
        first.provider = "shared-name".into();
        first.addon_id = "addon-a".into();
        let mut second = candidate(2, "hash", Some(8));
        second.provider = "shared-name".into();
        second.addon_id = "addon-b".into();

        let plan = plan_season(
            &[target(1), target(2)],
            &[first, second],
            PlanMode::Consistent,
            &BTreeSet::new(),
        );

        assert_eq!(plan.packs.len(), 2);
        assert_ne!(plan.packs[0].key.addon_id, plan.packs[1].key.addon_id);
    }

    #[test]
    fn ambiguity_fails_closed() {
        let mut c = candidate(1, "hash", Some(4));
        c.manifest_position = 99;
        let p = plan_season(
            &[target(1)],
            &[candidate(1, "hash", Some(3)), c],
            PlanMode::Fast,
            &BTreeSet::new(),
        );
        assert_eq!(p.coverage, Coverage::Ambiguous);
        assert!(p.selected.is_empty());
    }
    #[test]
    fn manual_selection_is_per_episode() {
        let key = PackKey {
            addon_id: "addon-p".into(),
            provider: "p".into(),
            info_hash: "hash".into(),
            binge_group: Some("g".into()),
        };
        let choices = [SelectionKey {
            episode: 1,
            pack: key,
            file_idx: 3,
        }]
        .into();
        let p = plan_season(
            &[target(1)],
            &[candidate(1, "hash", Some(3))],
            PlanMode::Manual,
            &choices,
        );
        assert_eq!(p.selected[&1].file_idx, 3);
    }
    #[test]
    fn filters_targets_and_hard_gates() {
        let ignored = EpisodeTarget {
            episode: 2,
            aired: false,
            monitored: true,
            missing: true,
        };
        let mut blocked = candidate(1, "hash", Some(3));
        blocked.quality_language_match = false;
        let p = plan_season(
            &[target(1), ignored],
            &[blocked],
            PlanMode::Consistent,
            &BTreeSet::new(),
        );
        assert_eq!(p.target_episodes, [1].into());
        assert_eq!(p.coverage, Coverage::Missing);
    }
}
