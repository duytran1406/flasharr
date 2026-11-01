use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Cached,
    PartiallyCached,
    NotCached,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStrategy {
    LocalQb,
    TorboxHttp,
}

impl ExecutionStrategy {
    pub const fn for_availability(availability: Availability) -> Self {
        match availability {
            Availability::Cached => ExecutionStrategy::TorboxHttp,
            Availability::PartiallyCached => ExecutionStrategy::LocalQb,
            Availability::NotCached => ExecutionStrategy::LocalQb,
            Availability::Unknown => ExecutionStrategy::LocalQb,
        }
    }

    pub const fn for_debrid_availability(availability: crate::debrid::Availability) -> Self {
        match availability {
            crate::debrid::Availability::Cached => ExecutionStrategy::TorboxHttp,
            crate::debrid::Availability::PartiallyCached => ExecutionStrategy::LocalQb,
            crate::debrid::Availability::NotCached => ExecutionStrategy::LocalQb,
            crate::debrid::Availability::Unknown => ExecutionStrategy::LocalQb,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Fshare,
    Torrent,
    Torbox,
    Stremio,
}

impl SourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fshare => "fshare",
            Self::Torrent => "torrent",
            Self::Torbox => "torbox",
            Self::Stremio => "stremio",
        }
    }

    pub fn resolve(
        source: Option<&str>,
        provider: Option<&str>,
        locator: Option<&str>,
    ) -> Result<Self, SourceFailure> {
        let source = normalized_hint(source);
        let provider = normalized_hint(provider);
        if source.is_some()
            && provider.is_some()
            && source != provider
            && !(source.as_deref() == Some("torrent")
                && matches!(provider.as_deref(), Some("stremio") | Some("prowlarr")))
        {
            return Err(SourceFailure::invalid("source and provider must agree"));
        }
        let explicit = source.or(provider);
        match explicit.as_deref() {
            Some("fshare") | Some("flasharr-http") => locator
                .filter(|value| crate::providers::fshare::parse_locator(value).is_some())
                .map(|_| Self::Fshare)
                .ok_or_else(|| SourceFailure::invalid("canonical Fshare URL is required")),
            Some("torrent") | Some("qbittorrent") => Ok(Self::Torrent),
            Some("torbox") => Ok(Self::Torbox),
            Some(value) => Err(SourceFailure::invalid(format!(
                "unsupported provider: {value}"
            ))),
            None => match locator.map(str::trim).filter(|value| !value.is_empty()) {
                Some(value) if crate::providers::fshare::parse_locator(value).is_some() => {
                    Ok(Self::Fshare)
                }
                Some(value)
                    if value.starts_with("magnet:")
                        || value.starts_with("prowlarr-ref://")
                        || value.ends_with(".torrent") =>
                {
                    Ok(Self::Torrent)
                }
                Some(value) if value.starts_with("torbox://") => Ok(Self::Torbox),
                _ => Err(SourceFailure::invalid(
                    "source/provider is required for unrecognized downloads",
                )),
            },
        }
    }
}

fn normalized_hint(value: Option<&str>) -> Option<String> {
    let value = value
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_ascii_lowercase();
    Some(match value.as_str() {
        "flasharr-http" => "fshare".to_string(),
        "qbittorrent" => "torrent".to_string(),
        _ => value,
    })
}

#[derive(Debug, Clone)]
pub struct DownloadSelection {
    pub url: Option<String>,
    pub download_ref: Option<String>,
    pub filename: Option<String>,
    pub source: Option<String>,
    pub provider: Option<String>,
    pub info_hash: Option<String>,
    pub size: Option<u64>,
    pub external_id: Option<String>,
    pub file_idx: Option<usize>,
    pub source_filename: Option<String>,
    pub selected_episodes: Option<String>,
    pub strategy: ExecutionStrategy,
}

#[derive(Debug, Clone)]
pub struct MaterializedDownload {
    pub queue_url: String,
    pub host: String,
    pub source: SourceKind,
    pub provider: String,
    pub size: Option<u64>,
    pub external_id: Option<String>,
    pub info_hash: Option<String>,
    pub execution_strategy: ExecutionStrategy,
    pub remote_state: Option<String>,
    pub remote_completed_at: Option<String>,
    pub selected_remote_file_ids: Option<String>,
    pub selected_artifacts: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TorboxFallback {
    pub kind: &'static str,
    pub label: &'static str,
    pub info_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceFailure {
    pub error: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<TorboxFallback>,
}

impl SourceFailure {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            error: "download_selection_invalid",
            message: message.into(),
            fallback: None,
        }
    }

    pub fn bad_request(message: impl Into<String>) -> (StatusCode, Self) {
        (StatusCode::BAD_REQUEST, Self::invalid(message))
    }

    pub fn torbox(message: impl Into<String>, info_hash: String) -> (StatusCode, Self) {
        (
            StatusCode::CONFLICT,
            Self {
                error: "torbox_resolution_failed",
                message: message.into(),
                fallback: Some(TorboxFallback {
                    kind: "choose_qbittorrent",
                    label: "Download with qBittorrent instead",
                    info_hash,
                }),
            },
        )
    }
}
