use crate::torrent::metainfo::{is_media_path, Metainfo};
use crate::utils::smart_tokenizer::smart_parse;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const SCHEMA_VERSION: u16 = 1;
const MAX_EPISODES_PER_FILE: u32 = 100;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PayloadRole {
    PrimaryMedia,
    Sample,
    Trailer,
    Extra,
    Padding,
    NonMedia,
    ZeroByte,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MappingState {
    Exact,
    Contextual,
    Ambiguous,
    Unmapped,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceBasis {
    Filename,
    ParentFolder,
    FilenameAndFolder,
    Absolute,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(crate) struct EpisodeRef {
    pub(crate) season: Option<u32>,
    pub(crate) episode: u32,
    pub(crate) basis: EvidenceBasis,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct FileQualityEvidence {
    pub(crate) resolution: Option<String>,
    pub(crate) source: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct FileParseEvidence {
    pub(crate) basename: String,
    pub(crate) title: String,
    pub(crate) year: Option<u32>,
    pub(crate) season: Option<u32>,
    pub(crate) episode: Option<u32>,
    pub(crate) confidence: f32,
    pub(crate) quality: FileQualityEvidence,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct TorrentFileEvidence {
    pub(crate) file_idx: usize,
    pub(crate) path_components: Vec<String>,
    pub(crate) path: String,
    pub(crate) bytes: u64,
    pub(crate) role: PayloadRole,
    pub(crate) parsed: Option<FileParseEvidence>,
    pub(crate) episodes: Vec<EpisodeRef>,
    pub(crate) mapping_state: MappingState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) conflict: Option<FileClaimConflict>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct FileClaimConflict {
    pub(crate) basename_season: u32,
    pub(crate) folder_season: u32,
    pub(crate) episodes: Vec<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TorrentScope {
    Movie,
    SingleEpisode,
    MultiEpisode,
    SeasonPack,
    MultiSeasonPack,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct EpisodeConflict {
    pub(crate) season: Option<u32>,
    pub(crate) episode: u32,
    pub(crate) file_indices: Vec<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SizeCheck {
    Match,
    WithinTolerance,
    Mismatch,
    Unavailable,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum QualityConsistency {
    Consistent,
    Mixed,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceConfidence {
    High,
    Medium,
    Low,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct TorrentEvidence {
    pub(crate) schema_version: u16,
    pub(crate) root_name: String,
    pub(crate) total_size: u64,
    pub(crate) files: Vec<TorrentFileEvidence>,
    pub(crate) covered_episodes: Vec<EpisodeRef>,
    pub(crate) duplicate_mappings: Vec<EpisodeConflict>,
    pub(crate) ambiguous_files: Vec<usize>,
    pub(crate) unmapped_media_files: Vec<usize>,
    pub(crate) scope: TorrentScope,
    pub(crate) size_check: SizeCheck,
    pub(crate) quality_consistency: QualityConsistency,
    pub(crate) confidence: EvidenceConfidence,
}

fn role(path: &str, bytes: u64) -> PayloadRole {
    if bytes == 0 {
        return PayloadRole::ZeroByte;
    }
    let lower = path.to_ascii_lowercase();
    let components = lower.split('/').collect::<Vec<_>>();
    if components.iter().any(|c| {
        let tokens = c
            .split(|x: char| !x.is_ascii_alphanumeric())
            .collect::<Vec<_>>();
        tokens.windows(2).any(|parts| parts == ["padding", "file"])
            || *c == ".padding"
            || *c == ".pad"
    }) {
        PayloadRole::Padding
    } else if components.iter().any(|c| {
        c.split(|x: char| !x.is_ascii_alphanumeric())
            .any(|x| x == "sample")
    }) {
        PayloadRole::Sample
    } else if components.iter().any(|c| {
        c.split(|x: char| !x.is_ascii_alphanumeric())
            .any(|x| x == "trailer")
    }) {
        PayloadRole::Trailer
    } else if components
        .iter()
        .any(|c| ["extras", "extra", "featurette", "featurettes"].contains(c))
        || components.iter().any(|c| c == &"behind the scenes")
    {
        PayloadRole::Extra
    } else if !is_media_path(path) {
        PayloadRole::NonMedia
    } else {
        PayloadRole::PrimaryMedia
    }
}

fn explicit_claims(name: &str) -> Result<Option<(u32, Vec<u32>)>, ()> {
    static START: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)(?:^|[^A-Za-z0-9])S(\d{1,2})E").unwrap()
    });
    let Some(c) = START.captures(name) else {
        return Ok(None);
    };
    let season = c[1].parse().map_err(|_| ())?;
    let start = c.get(0).unwrap().end() - 1;
    let tail = &name[start..];
    static TOKEN: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)^E(\d{1,4})((?:E|\+E)(\d{1,4}))*(?:[^A-Za-z0-9]|$)").unwrap()
    });
    static RANGE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)^E(\d{1,4})-(?:E)?(\d{1,4})(?:[^A-Za-z0-9]|$)").unwrap()
    });
    if tail.starts_with('E')
        && (tail.contains("EAA") || tail.contains("E01-") && RANGE.captures(tail).is_none())
    {
        return Err(());
    }
    let mut episodes: Vec<u32> = if let Some(r) = RANGE.captures(tail) {
        let a: u32 = r[1].parse().map_err(|_| ())?;
        let b: u32 = r[2].parse().map_err(|_| ())?;
        if a == 0 || b < a || b - a + 1 > MAX_EPISODES_PER_FILE {
            return Err(());
        }
        (a..=b).collect()
    } else if let Some(m) = TOKEN.find(tail) {
        static NUM: once_cell::sync::Lazy<regex::Regex> =
            once_cell::sync::Lazy::new(|| regex::Regex::new(r"(?i)E(\d{1,4})").unwrap());
        NUM.captures_iter(m.as_str())
            .map(|x| x[1].parse::<u32>().unwrap())
            .collect()
    } else {
        return Err(());
    };
    if episodes.iter().any(|e| *e == 0) {
        return Err(());
    }
    episodes.sort_unstable();
    episodes.dedup();
    Ok(Some((season, episodes)))
}

fn folder_season(parts: &[String]) -> Option<u32> {
    static RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)(?:season\s*|^S)(\d{1,2})(?:\D|$)").unwrap()
    });
    parts
        .iter()
        .rev()
        .skip(1)
        .find_map(|p| RE.captures(p).and_then(|c| c[1].parse().ok()))
}

pub(crate) fn analyze_metainfo(meta: &Metainfo, declared_size: Option<u64>) -> TorrentEvidence {
    let mut files = Vec::with_capacity(meta.files.len());
    for file in &meta.files {
        let basename = file.path_components.last().cloned().unwrap_or_default();
        let role = role(&file.path, file.length);
        let smart = smart_parse(&basename);
        let parsed = Some(FileParseEvidence {
            basename: basename.clone(),
            title: smart.title,
            year: smart.year,
            season: smart.season,
            episode: smart.episode,
            confidence: smart.confidence,
            quality: FileQualityEvidence {
                resolution: smart.resolution,
                source: smart.source,
            },
        });
        let parent = folder_season(&file.path_components);
        let (episodes, mapping_state, conflict) = if role != PayloadRole::PrimaryMedia {
            (vec![], MappingState::Unmapped, None)
        } else {
            match explicit_claims(&basename) {
                Err(()) => (vec![], MappingState::Ambiguous, None),
                Ok(Some((season, eps))) if parent.is_some_and(|p| p != season) => (
                    vec![],
                    MappingState::Ambiguous,
                    Some(FileClaimConflict {
                        basename_season: season,
                        folder_season: parent.unwrap(),
                        episodes: eps,
                    }),
                ),
                Ok(Some((season, eps))) => (
                    eps.into_iter()
                        .map(|episode| EpisodeRef {
                            season: Some(season),
                            episode,
                            basis: if parent == Some(season) {
                                EvidenceBasis::FilenameAndFolder
                            } else {
                                EvidenceBasis::Filename
                            },
                        })
                        .collect(),
                    MappingState::Exact,
                    None,
                ),
                Ok(None) => {
                    static E: once_cell::sync::Lazy<regex::Regex> =
                        once_cell::sync::Lazy::new(|| {
                            regex::Regex::new(r"(?i)(?:^|\D)E(\d{1,4})(?:\D|$)").unwrap()
                        });
                    if let (Some(season), Some(c)) = (parent, E.captures(&basename)) {
                        let ep = c[1].parse().unwrap_or(0);
                        if ep > 0 {
                            (
                                vec![EpisodeRef {
                                    season: Some(season),
                                    episode: ep,
                                    basis: EvidenceBasis::ParentFolder,
                                }],
                                MappingState::Contextual,
                                None,
                            )
                        } else {
                            (vec![], MappingState::Ambiguous, None)
                        }
                    } else {
                        (vec![], MappingState::Unmapped, None)
                    }
                }
            }
        };
        files.push(TorrentFileEvidence {
            file_idx: file.index,
            path_components: file.path_components.clone(),
            path: file.path.clone(),
            bytes: file.length,
            role,
            parsed,
            episodes,
            mapping_state,
            conflict,
        });
    }
    let eligible = files
        .iter()
        .filter(|f| f.role == PayloadRole::PrimaryMedia)
        .collect::<Vec<_>>();
    let mut map: BTreeMap<(Option<u32>, u32), Vec<usize>> = BTreeMap::new();
    for file in &eligible {
        for ep in &file.episodes {
            map.entry((ep.season, ep.episode))
                .or_default()
                .push(file.file_idx);
        }
    }
    let covered_episodes = map
        .keys()
        .map(|(season, episode)| EpisodeRef {
            season: *season,
            episode: *episode,
            basis: EvidenceBasis::Filename,
        })
        .collect::<Vec<_>>();
    let duplicate_mappings = map
        .iter()
        .filter(|(_, f)| f.len() > 1)
        .map(|((season, episode), file_indices)| EpisodeConflict {
            season: *season,
            episode: *episode,
            file_indices: file_indices.clone(),
        })
        .collect::<Vec<_>>();
    let ambiguous_files = eligible
        .iter()
        .filter(|f| f.mapping_state == MappingState::Ambiguous)
        .map(|f| f.file_idx)
        .collect::<Vec<_>>();
    let unmapped_media_files = eligible
        .iter()
        .filter(|f| f.mapping_state == MappingState::Unmapped)
        .map(|f| f.file_idx)
        .collect::<Vec<_>>();
    let seasons = map.keys().filter_map(|x| x.0).collect::<BTreeSet<_>>();
    let multi_file = eligible.iter().any(|f| f.episodes.len() > 1);
    let scope = if map.is_empty() {
        if eligible.len() == 1 {
            TorrentScope::Movie
        } else {
            TorrentScope::Unknown
        }
    } else if multi_file && eligible.len() == 1 {
        TorrentScope::MultiEpisode
    } else if map.len() == 1 {
        TorrentScope::SingleEpisode
    } else if seasons.len() > 1 {
        TorrentScope::MultiSeasonPack
    } else {
        TorrentScope::SeasonPack
    };
    let size_check = declared_size.map_or(SizeCheck::Unavailable, |d| {
        let delta = d.abs_diff(meta.total_size);
        if delta == 0 {
            SizeCheck::Match
        } else if delta <= (meta.total_size / 100).max(1024 * 1024) {
            SizeCheck::WithinTolerance
        } else {
            SizeCheck::Mismatch
        }
    });
    let qualities = eligible
        .iter()
        .filter_map(|f| f.parsed.as_ref())
        .map(|p| (p.quality.resolution.clone(), p.quality.source.clone()))
        .filter(|q| q.0.is_some() || q.1.is_some())
        .collect::<BTreeSet<_>>();
    let quality_consistency = if qualities.is_empty() {
        QualityConsistency::Unknown
    } else if qualities.len() == 1 {
        QualityConsistency::Consistent
    } else {
        QualityConsistency::Mixed
    };
    let confidence = if !ambiguous_files.is_empty()
        || !duplicate_mappings.is_empty()
        || size_check == SizeCheck::Mismatch
    {
        EvidenceConfidence::Low
    } else if !unmapped_media_files.is_empty() {
        EvidenceConfidence::Medium
    } else {
        EvidenceConfidence::High
    };
    TorrentEvidence {
        schema_version: SCHEMA_VERSION,
        root_name: meta.root_name.clone(),
        total_size: meta.total_size,
        files,
        covered_episodes,
        duplicate_mappings,
        ambiguous_files,
        unmapped_media_files,
        scope,
        size_check,
        quality_consistency,
        confidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::torrent::metainfo::MetainfoFile;
    fn meta(paths: &[(&str, u64)]) -> Metainfo {
        let files = paths
            .iter()
            .enumerate()
            .map(|(index, (path, length))| MetainfoFile {
                index,
                path_components: path.split('/').map(str::to_string).collect(),
                path: (*path).into(),
                length: *length,
            })
            .collect::<Vec<_>>();
        Metainfo {
            root_name: "Pack".into(),
            total_size: files.iter().map(|f| f.length).sum(),
            file_count: files.len(),
            media_file_count: files.iter().filter(|f| is_media_path(&f.path)).count(),
            files,
        }
    }
    #[test]
    fn preserves_payload_identity() {
        let m = meta(&[("Season 01/Show.S01E01.mkv", 100), ("readme.txt", 7)]);
        let e = analyze_metainfo(&m, Some(107));
        assert_eq!(
            (
                e.files[0].file_idx,
                e.files[0].path.as_str(),
                e.files[0].bytes
            ),
            (0, "Season 01/Show.S01E01.mkv", 100)
        );
        assert_eq!(e.files[0].path_components, m.files[0].path_components);
        assert_eq!(e.files[1].role, PayloadRole::NonMedia);
    }
    #[test]
    fn compound_ranges_and_specials_are_bounded() {
        for (name, want) in [
            ("Show.S01E01E02.mkv", vec![1, 2]),
            ("Show.S01E01-E03.mkv", vec![1, 2, 3]),
            ("Show.S01E01-03.mkv", vec![1, 2, 3]),
            ("Show.S01E01+E02.mkv", vec![1, 2]),
            ("Show.S00E01.mkv", vec![1]),
        ] {
            let e = analyze_metainfo(&meta(&[(name, 1)]), None);
            assert_eq!(
                e.files[0]
                    .episodes
                    .iter()
                    .map(|x| x.episode)
                    .collect::<Vec<_>>(),
                want,
                "{name}"
            );
        }
        for name in [
            "Show.S01E03-E01.mkv",
            "Show.S01E01-E101.mkv",
            "Show.S01E01-.mkv",
            "Show.S01E0000.mkv",
        ] {
            assert_eq!(
                analyze_metainfo(&meta(&[(name, 1)]), None).files[0].mapping_state,
                MappingState::Ambiguous,
                "{name}"
            );
        }
    }
    #[test]
    fn roles_cannot_prove_coverage() {
        for (path, role) in [
            ("Sample/Show.S01E01.mkv", PayloadRole::Sample),
            ("Extras/featurette.mp4", PayloadRole::Extra),
            ("trailer.mp4", PayloadRole::Trailer),
            (".padding/123", PayloadRole::Padding),
            ("Show.S01E01.mkv", PayloadRole::ZeroByte),
        ] {
            let bytes = u64::from(path != "Show.S01E01.mkv");
            let e = analyze_metainfo(&meta(&[(path, bytes)]), None);
            let expected = role;
            assert_eq!(e.files[0].role, expected);
            assert!(e.covered_episodes.is_empty());
        }
    }
    #[test]
    fn folder_context_conflict_and_absolute_fail_closed() {
        let contextual = analyze_metainfo(&meta(&[("Season 02/Show.E03.mkv", 1)]), None);
        assert_eq!(contextual.files[0].mapping_state, MappingState::Contextual);
        assert_eq!(contextual.files[0].episodes[0].season, Some(2));
        let conflict = analyze_metainfo(&meta(&[("Season 02/Show.S01E03.mkv", 1)]), None);
        assert_eq!(conflict.files[0].mapping_state, MappingState::Ambiguous);
        let claims = conflict.files[0].conflict.as_ref().unwrap();
        assert_eq!((claims.basename_season, claims.folder_season), (1, 2));
        assert_eq!(claims.episodes, vec![3]);
        assert!(conflict.covered_episodes.is_empty());
        let absolute = analyze_metainfo(&meta(&[("Show.123.mkv", 1)]), None);
        assert!(absolute.covered_episodes.is_empty());
    }
    #[test]
    fn role_and_episode_patterns_require_component_boundaries() {
        for name in [
            "notpadding_file_movie.mkv",
            "exampleshow.S01E01.mkv",
            "resampled.movie.mkv",
        ] {
            assert_eq!(
                analyze_metainfo(&meta(&[(name, 1)]), None).files[0].role,
                PayloadRole::PrimaryMedia,
                "{name}"
            );
        }
        for name in ["prefixS01E01.mkv", "Show.S01E01suffix.mkv"] {
            assert!(
                analyze_metainfo(&meta(&[(name, 1)]), None)
                    .covered_episodes
                    .is_empty(),
                "{name}"
            );
        }
    }
    #[test]
    fn aggregates_duplicates_scope_size_and_quality() {
        let e = analyze_metainfo(
            &meta(&[
                ("Show.S01E01.1080p.WEB-DL.mkv", 100),
                ("Show.S01E01.2160p.BluRay.mkv", 100),
                ("Show.S02E02.mkv", 100),
            ]),
            Some(999),
        );
        assert_eq!(e.scope, TorrentScope::MultiSeasonPack);
        assert_eq!(e.duplicate_mappings[0].file_indices, vec![0, 1]);
        assert_eq!(e.quality_consistency, QualityConsistency::Mixed);
        assert_eq!(e.size_check, SizeCheck::WithinTolerance);
        assert_eq!(e.confidence, EvidenceConfidence::Low);
    }
}
