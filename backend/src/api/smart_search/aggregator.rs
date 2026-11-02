use std::collections::HashSet;

use super::types::{RequestContract, SmartSearchIntent, SmartSearchResponse};
use super::SeasonGroup;
use crate::utils::title_matcher::QualityGroup;

#[derive(Debug, Clone)]
pub enum AggregatedSmartSearchResult {
    MovieGroups {
        query: String,
        groups: Vec<QualityGroup>,
        sources: Option<super::types::SmartSearchSourceStatus>,
    },
    Seasons {
        query: String,
        seasons: Vec<SeasonGroup>,
        is_target_episode: bool,
        sources: Option<super::types::SmartSearchSourceStatus>,
    },
}

pub fn aggregate_legacy_response(
    response: SmartSearchResponse,
    intent: &SmartSearchIntent,
) -> AggregatedSmartSearchResult {
    match response.seasons {
        Some(seasons) => AggregatedSmartSearchResult::Seasons {
            query: response.query,
            seasons: dedupe_seasons(seasons),
            is_target_episode: matches!(intent, SmartSearchIntent::TargetEpisode { .. }),
            sources: response.sources,
        },
        None => AggregatedSmartSearchResult::MovieGroups {
            query: response.query,
            groups: response.groups.unwrap_or_default(),
            sources: response.sources,
        },
    }
}

pub fn serialize_result(
    result: AggregatedSmartSearchResult,
    intent: &SmartSearchIntent,
    contract: RequestContract,
) -> SmartSearchResponse {
    let (mode, kind) = intent.mode_kind();
    let explicit = matches!(contract, RequestContract::Explicit);

    match result {
        AggregatedSmartSearchResult::MovieGroups {
            query,
            groups,
            sources,
        } => SmartSearchResponse {
            query,
            total_found: groups.iter().map(|g| g.count).sum(),
            r#type: "movie".to_string(),
            mode: explicit.then(|| mode.to_string()),
            kind: explicit.then_some(kind).flatten().map(str::to_string),
            groups: Some(groups),
            seasons: None,
            sources,
            season_plan: None,
        },
        AggregatedSmartSearchResult::Seasons {
            query,
            mut seasons,
            is_target_episode,
            sources,
        } => {
            if let SmartSearchIntent::Season { season, .. } = intent {
                seasons.retain(|group| group.season == *season);
            }
            SmartSearchResponse {
                query,
                total_found: seasons
                    .iter()
                    .flat_map(|s| &s.episodes_grouped)
                    .map(|e| e.files.len())
                    .sum(),
                r#type: if is_target_episode { "episode" } else { "tv" }.to_string(),
                mode: explicit.then(|| mode.to_string()),
                kind: explicit.then_some(kind).flatten().map(str::to_string),
                groups: None,
                seasons: Some(seasons),
                sources,
                season_plan: None,
            }
        }
    }
}

fn dedupe_seasons(mut seasons: Vec<SeasonGroup>) -> Vec<SeasonGroup> {
    // Canonical conversion preserves file_idx, sources, behavior_hints, download_ref,
    // origin_provider, and origin_indexer while grouping.
    for season in &mut seasons {
        for episode in &mut season.episodes_grouped {
            let mut seen = HashSet::new();
            episode.files.retain(|file| {
                let source = file.result_source.as_deref().unwrap_or("unknown");
                let key = crate::sources::search::CanonicalSearchResult::from(&*file)
                    .validate()
                    .map(|canonical| canonical.dedupe_key(source, file.size))
                    .unwrap_or_else(|_| format!("invalid:{source}:{}", file.url));
                seen.insert(key)
            });
        }
    }
    seasons
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_result(intent: &SmartSearchIntent) -> AggregatedSmartSearchResult {
        match intent {
            SmartSearchIntent::TargetMovie { .. } => AggregatedSmartSearchResult::MovieGroups {
                query: "sentinel".into(),
                groups: vec![],
                sources: None,
            },
            _ => AggregatedSmartSearchResult::Seasons {
                query: "sentinel".into(),
                seasons: vec![],
                is_target_episode: matches!(intent, SmartSearchIntent::TargetEpisode { .. }),
                sources: None,
            },
        }
    }

    #[test]
    fn season_plan_serializes_with_frontend_contract_key() {
        let value = serde_json::to_value(SmartSearchResponse {
            query: "Show".into(),
            total_found: 0,
            r#type: "tv".into(),
            mode: Some("season".into()),
            kind: None,
            groups: Some(vec![]),
            seasons: None,
            sources: None,
            season_plan: Some(crate::stremio::season_pack::SeasonPlan {
                target_episodes: [1].into_iter().collect(),
                packs: vec![],
                selected: Default::default(),
                coverage: crate::stremio::season_pack::Coverage::Missing,
                discovery_status: "complete".into(),
                queried_episodes: Default::default(),
                request_bound: 1,
            }),
        })
        .unwrap();
        assert!(value.get("seasonPlan").is_some());
        assert!(value.get("season_plan").is_none());
    }

    #[test]
    fn explicit_mode_kind_serialization_is_exact() {
        let cases = [
            (
                SmartSearchIntent::Season {
                    title: "Show".into(),
                    year: None,
                    tmdb_id: None,
                    tvdb_id: None,
                    season: 1,
                },
                "season",
                None,
            ),
            (
                SmartSearchIntent::TargetMovie {
                    title: "Movie".into(),
                    year: None,
                    tmdb_id: None,
                },
                "target",
                Some("movie"),
            ),
            (
                SmartSearchIntent::TargetEpisode {
                    title: "Show".into(),
                    year: None,
                    tmdb_id: None,
                    tvdb_id: None,
                    season: 1,
                    episode: 2,
                },
                "target",
                Some("episode"),
            ),
        ];
        for (intent, mode, kind) in cases {
            let value = serde_json::to_value(serialize_result(
                empty_result(&intent),
                &intent,
                RequestContract::Explicit,
            ))
            .unwrap();
            assert_eq!(value["mode"], mode);
            assert_eq!(value.get("kind").and_then(|v| v.as_str()), kind);
        }
    }

    #[test]
    fn season_response_excludes_parsed_other_seasons() {
        let intent = SmartSearchIntent::Season {
            title: "Test Series".into(),
            year: None,
            tmdb_id: None,
            tvdb_id: None,
            season: 1,
        };
        let season = |number| SeasonGroup {
            season: number,
            episode_count: 0,
            aired_episode_count: 0,
            packages: vec![],
            episodes_grouped: vec![],
        };
        let response = serialize_result(
            AggregatedSmartSearchResult::Seasons {
                query: "Test Series".into(),
                seasons: vec![season(1), season(2), season(3)],
                is_target_episode: false,
                sources: None,
            },
            &intent,
            RequestContract::Legacy,
        );

        assert_eq!(
            response
                .seasons
                .unwrap()
                .iter()
                .map(|s| s.season)
                .collect::<Vec<_>>(),
            [1]
        );
    }
}
