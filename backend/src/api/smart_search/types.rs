use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::SeasonGroup;
use crate::utils::title_matcher::QualityGroup;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SmartSearchMode {
    Season,
    Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SmartSearchKind {
    Movie,
    Episode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartSearchIntent {
    Season {
        title: String,
        year: Option<Value>,
        tmdb_id: Option<Value>,
        tvdb_id: Option<Value>,
        season: u32,
    },
    TargetMovie {
        title: String,
        year: Option<Value>,
        tmdb_id: Option<Value>,
    },
    TargetEpisode {
        title: String,
        year: Option<Value>,
        tmdb_id: Option<Value>,
        tvdb_id: Option<Value>,
        season: u32,
        episode: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestContract {
    Legacy,
    Explicit,
}

#[derive(Debug, Clone)]
pub struct ParsedSmartSearchRequest {
    pub contract: RequestContract,
    pub intent: SmartSearchIntent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartSearchRequest {
    pub title: String,
    pub year: Option<Value>,
    pub r#type: String,
    #[serde(alias = "tmdb_id")]
    pub tmdb_id: Option<Value>,
    #[serde(alias = "tvdb_id")]
    pub tvdb_id: Option<Value>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SmartSearchSourceState {
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addons: Option<Vec<StremioAddonState>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry: Option<SourceTelemetry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum SourceTelemetry {
    Stremio(StremioTelemetry),
    Prowlarr(ProwlarrTelemetry),
}

impl From<StremioTelemetry> for SourceTelemetry {
    fn from(value: StremioTelemetry) -> Self {
        Self::Stremio(value)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProwlarrTelemetry {
    pub planned: usize,
    pub attempted: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub timed_out: usize,
    pub truncated: usize,
    pub hash_resolved: usize,
    pub hash_cache_hits: usize,
    pub hash_omitted: usize,
    pub inspections_selected: usize,
    pub inspections_completed: usize,
    pub inspections_timed_out: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StremioAddonState {
    pub id: String,
    pub name: String,
    pub health: String,
    pub result_count: usize,
    pub elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<crate::stremio::client::StremioFailureClass>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct StremioTelemetry {
    pub total_calls: usize,
    pub target_episodes: usize,
    pub queried_episodes: usize,
    pub covered_episodes: usize,
    pub waves: usize,
    pub truncated: bool,
    pub by_health: std::collections::BTreeMap<String, usize>,
    pub by_failure_class: std::collections::BTreeMap<String, usize>,
}

pub type SmartSearchSourceStatus =
    std::collections::BTreeMap<crate::sources::model::SourceKind, SmartSearchSourceState>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeasonPlanState {
    Ok,
    Partial,
    IdentityUnavailable,
    RegistryUnavailable,
    AddonUnavailable,
    NoTargets,
    NoResults,
}

#[derive(Debug, Clone)]
pub struct SeasonPlanOutcome {
    pub state: SeasonPlanState,
    pub plan: Option<crate::stremio::season_pack::SeasonPlan>,
    pub seasons: Vec<super::SeasonGroup>,
    pub addons: Vec<StremioAddonState>,
    pub telemetry: Option<StremioTelemetry>,
}

impl SeasonPlanOutcome {
    pub fn new(
        state: SeasonPlanState,
        plan: Option<crate::stremio::season_pack::SeasonPlan>,
    ) -> Self {
        Self {
            state,
            plan,
            seasons: Vec::new(),
            addons: Vec::new(),
            telemetry: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SmartSearchResponse {
    pub query: String,
    pub total_found: usize,
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub groups: Option<Vec<QualityGroup>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seasons: Option<Vec<SeasonGroup>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<SmartSearchSourceStatus>,
    #[serde(rename = "seasonPlan", skip_serializing_if = "Option::is_none")]
    pub season_plan: Option<crate::stremio::season_pack::SeasonPlan>,
}

#[derive(Debug, Clone)]
pub struct SmartSearchValidationError {
    pub message: String,
}

impl SmartSearchValidationError {
    pub fn into_response(self) -> axum::response::Response {
        (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}

impl SmartSearchIntent {
    pub fn title(&self) -> &str {
        match self {
            Self::Season { title, .. }
            | Self::TargetMovie { title, .. }
            | Self::TargetEpisode { title, .. } => title,
        }
    }

    pub fn legacy_request(&self) -> SmartSearchRequest {
        match self {
            Self::Season {
                title,
                year,
                tmdb_id,
                tvdb_id,
                season,
            } => SmartSearchRequest {
                title: title.clone(),
                year: year.clone(),
                r#type: "tv".to_string(),
                tmdb_id: tmdb_id.clone(),
                tvdb_id: tvdb_id.clone(),
                season: Some(*season),
                episode: None,
            },
            Self::TargetMovie {
                title,
                year,
                tmdb_id,
            } => SmartSearchRequest {
                title: title.clone(),
                year: year.clone(),
                r#type: "movie".to_string(),
                tmdb_id: tmdb_id.clone(),
                tvdb_id: None,
                season: None,
                episode: None,
            },
            Self::TargetEpisode {
                title,
                year,
                tmdb_id,
                tvdb_id,
                season,
                episode,
            } => SmartSearchRequest {
                title: title.clone(),
                year: year.clone(),
                r#type: "tv".to_string(),
                tmdb_id: tmdb_id.clone(),
                tvdb_id: tvdb_id.clone(),
                season: Some(*season),
                episode: Some(*episode),
            },
        }
    }

    pub fn mode_kind(&self) -> (&'static str, Option<&'static str>) {
        match self {
            Self::Season { .. } => ("season", None),
            Self::TargetMovie { .. } => ("target", Some("movie")),
            Self::TargetEpisode { .. } => ("target", Some("episode")),
        }
    }
}

pub fn parse_smart_search_request(
    value: Value,
) -> Result<ParsedSmartSearchRequest, SmartSearchValidationError> {
    if value.get("mode").is_some() {
        parse_explicit(value)
    } else {
        parse_legacy(value)
    }
}

fn parse_explicit(value: Value) -> Result<ParsedSmartSearchRequest, SmartSearchValidationError> {
    let mode = value
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let title = value
        .get("title")
        .or_else(|| value.get("seriesTitle"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let year = value.get("year").cloned();
    let tmdb_id = value
        .get("tmdbId")
        .or_else(|| value.get("tmdb_id"))
        .cloned();
    let tvdb_id = value
        .get("tvdbId")
        .or_else(|| value.get("tvdb_id"))
        .cloned();

    validate_title(&title)?;

    let intent = match mode {
        "season" => {
            if value.get("episode").is_some() {
                return err("season mode must not include episode");
            }
            let season = required_u32(&value, "season")?;
            SmartSearchIntent::Season {
                title,
                year,
                tmdb_id,
                tvdb_id,
                season,
            }
        }
        "target" => match value
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "movie" => SmartSearchIntent::TargetMovie {
                title,
                year,
                tmdb_id,
            },
            "episode" => {
                let season = required_u32(&value, "season")?;
                let episode = required_u32(&value, "episode")?;
                SmartSearchIntent::TargetEpisode {
                    title,
                    year,
                    tmdb_id,
                    tvdb_id,
                    season,
                    episode,
                }
            }
            _ => return err("target mode requires kind movie or episode"),
        },
        _ => return err("mode must be season or target"),
    };

    Ok(ParsedSmartSearchRequest {
        contract: RequestContract::Explicit,
        intent,
    })
}

fn parse_legacy(value: Value) -> Result<ParsedSmartSearchRequest, SmartSearchValidationError> {
    let req: SmartSearchRequest =
        serde_json::from_value(value).map_err(|e| SmartSearchValidationError {
            message: format!("invalid legacy smart search request: {}", e),
        })?;

    validate_title(req.title.trim())?;

    let intent = if req.r#type != "tv" {
        SmartSearchIntent::TargetMovie {
            title: req.title,
            year: req.year,
            tmdb_id: req.tmdb_id,
        }
    } else if let (Some(season), Some(episode)) = (req.season, req.episode) {
        SmartSearchIntent::TargetEpisode {
            title: req.title,
            year: req.year,
            tmdb_id: req.tmdb_id,
            tvdb_id: req.tvdb_id,
            season,
            episode,
        }
    } else if let Some(season) = req.season {
        SmartSearchIntent::Season {
            title: req.title,
            year: req.year,
            tmdb_id: req.tmdb_id,
            tvdb_id: req.tvdb_id,
            season,
        }
    } else {
        return err("legacy tv search requires season");
    };

    Ok(ParsedSmartSearchRequest {
        contract: RequestContract::Legacy,
        intent,
    })
}

fn validate_title(title: &str) -> Result<(), SmartSearchValidationError> {
    if title.is_empty() {
        return err("title is required");
    }
    if title.chars().count() > 256 {
        return err("title must be at most 256 characters");
    }
    Ok(())
}

fn required_u32(value: &Value, key: &str) -> Result<u32, SmartSearchValidationError> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| SmartSearchValidationError {
            message: format!("{} must be a positive integer", key),
        })
}

fn err<T>(message: &str) -> Result<T, SmartSearchValidationError> {
    Err(SmartSearchValidationError {
        message: message.to_string(),
    })
}

use axum::response::IntoResponse;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn smart_search_title_has_a_256_character_boundary() {
        assert!(parse_smart_search_request(json!({
            "mode": "target",
            "kind": "movie",
            "title": "x".repeat(256)
        }))
        .is_ok());
        let error = parse_smart_search_request(json!({
            "mode": "target",
            "kind": "movie",
            "title": "x".repeat(257)
        }))
        .unwrap_err();
        assert_eq!(error.message, "title must be at most 256 characters");
        assert!(parse_smart_search_request(json!({
            "title": "x".repeat(256),
            "type": "movie"
        }))
        .is_ok());
        assert_eq!(
            parse_smart_search_request(json!({
                "title": "x".repeat(257),
                "type": "movie"
            }))
            .unwrap_err()
            .message,
            "title must be at most 256 characters"
        );
    }

    #[test]
    fn legacy_tv_season_with_stale_episode_becomes_target_episode() {
        let parsed =
            parse_smart_search_request(json!({"title":"Show","type":"tv","season":1,"episode":2}))
                .unwrap();
        assert!(matches!(
            parsed.intent,
            SmartSearchIntent::TargetEpisode {
                season: 1,
                episode: 2,
                ..
            }
        ));
    }

    #[test]
    fn legacy_tv_season_without_episode_becomes_season() {
        let parsed =
            parse_smart_search_request(json!({"title":"Show","type":"tv","season":1})).unwrap();
        assert!(matches!(
            parsed.intent,
            SmartSearchIntent::Season { season: 1, .. }
        ));
    }

    #[test]
    fn explicit_season_with_episode_is_rejected() {
        assert!(parse_smart_search_request(
            json!({"mode":"season","title":"Show","season":1,"episode":2})
        )
        .is_err());
    }

    #[test]
    fn target_episode_requires_season_episode() {
        assert!(parse_smart_search_request(
            json!({"mode":"target","kind":"episode","title":"Show","season":1})
        )
        .is_err());
    }
}
