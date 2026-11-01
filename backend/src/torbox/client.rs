use anyhow::Context;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

pub const TORBOX_API_BASE: &str = "https://api.torbox.app/v1/api";

#[derive(Debug, Clone)]
pub struct TorBoxClient {
    base_url: String,
    api_key: String,
    http: Arc<reqwest::Client>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct TorBoxFile {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub mimetype: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct TorBoxCacheHit {
    pub hash: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub files: Vec<TorBoxFile>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct TorBoxTorrentDetails {
    pub id: u64,
    #[serde(rename = "download_state")]
    pub state: String,
    #[serde(default)]
    pub download_finished: bool,
    #[serde(default)]
    pub download_present: bool,
    #[serde(default)]
    pub files: Vec<TorBoxFile>,
}

/// Minimal account information suitable for server-side display only.
/// Never add identifiers, usage data, settings, or credentials here.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct TorBoxProfile {
    pub email: String,
    pub plan: u8,
    pub is_subscribed: bool,
    #[serde(default)]
    pub premium_expires_at: Option<String>,
}

impl TorBoxTorrentDetails {
    pub fn is_remote_ready(&self) -> bool {
        let state = self.state.trim().to_ascii_lowercase();
        (state == "cached" || (state == "completed" && self.download_finished))
            && self.download_present
            && !self.files.is_empty()
            && self
                .files
                .iter()
                .all(|file| file.size > 0 && !file.name.trim().is_empty())
    }

    pub fn terminal_error(&self) -> Option<String> {
        let state = self.state.trim().to_ascii_lowercase();
        matches!(
            state.as_str(),
            "error" | "reported missing" | "incomplete" | "expired"
        )
        .then(|| format!("TorBox torrent {} entered terminal state {state}", self.id))
    }
}

#[derive(Deserialize)]
struct TorBoxResponse {
    success: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    detail: Option<String>,
    #[serde(default)]
    data: Value,
}

impl TorBoxClient {
    pub fn new(_base_url: String, api_key: String, http: Arc<reqwest::Client>) -> Self {
        Self {
            base_url: TORBOX_API_BASE.to_string(),
            api_key,
            http,
        }
    }

    fn validated_api_key(&self) -> anyhow::Result<&str> {
        crate::api::settings::validate_api_key(&self.api_key).map_err(anyhow::Error::msg)?;
        Ok(&self.api_key)
    }

    pub async fn test_connection(&self) -> anyhow::Result<()> {
        self.profile().await.map(|_| ())
    }

    pub async fn profile(&self) -> anyhow::Result<TorBoxProfile> {
        let body = self.get_authenticated("/user/me?settings=false").await?;
        let parsed: TorBoxResponse =
            serde_json::from_str(&body).context("TorBox profile response parse failed")?;
        if !parsed.success {
            anyhow::bail!(parsed
                .detail
                .unwrap_or_else(|| "TorBox profile request failed".to_string()));
        }
        serde_json::from_value(parsed.data).context("TorBox profile data parse failed")
    }

    pub async fn check_cache(
        &self,
        hashes: &[String],
    ) -> anyhow::Result<HashMap<String, TorBoxCacheHit>> {
        let hashes = normalized_hashes(hashes);
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }
        let api_key = self.validated_api_key()?;
        let resp = self
            .http
            .post(self.endpoint("/torrents/checkcached"))
            .bearer_auth(api_key)
            .json(&serde_json::json!({ "hashes": hashes }))
            .send()
            .await?;
        if matches!(
            resp.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            anyhow::bail!("TorBox authentication failed");
        }
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("TorBox returned HTTP {}", status);
        }
        parse_cache_response(&body)
    }

    pub async fn torrent_info(&self, hash: &str) -> anyhow::Result<TorBoxCacheHit> {
        let url = format!(
            "{}/torrents/torrentinfo?hash={}&timeout=30&use_cache_lookup=true",
            self.base_url,
            urlencoding::encode(hash)
        );
        let response = self
            .http
            .get(url)
            .bearer_auth(self.validated_api_key()?)
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("TorBox torrentinfo returned HTTP {status}");
        }
        let parsed: TorBoxResponse =
            serde_json::from_str(&body).context("TorBox torrentinfo response parse failed")?;
        if !parsed.success {
            anyhow::bail!(parsed
                .detail
                .unwrap_or_else(|| "TorBox torrentinfo failed".to_string()));
        }
        parse_torrent_info(parsed.data, hash)
    }

    pub async fn torrent_details(&self, torrent_id: u64) -> anyhow::Result<TorBoxTorrentDetails> {
        let body = self
            .get_authenticated(&format!("/torrents/mylist?id={torrent_id}&files=true"))
            .await?;
        parse_torrent_response(&body)
    }

    pub async fn list_files(&self, torrent_id: u64) -> anyhow::Result<Vec<TorBoxFile>> {
        Ok(self.torrent_details(torrent_id).await?.files)
    }

    pub async fn poll_remote_ready(
        &self,
        torrent_id: u64,
        interval: Duration,
        deadline: Duration,
    ) -> anyhow::Result<TorBoxTorrentDetails> {
        anyhow::ensure!(!interval.is_zero(), "TorBox poll interval must be positive");
        anyhow::ensure!(!deadline.is_zero(), "TorBox poll deadline must be positive");
        let expires = tokio::time::Instant::now() + deadline;
        loop {
            let details = self.torrent_details(torrent_id).await?;
            if let Some(error) = details.terminal_error() {
                anyhow::bail!(error);
            }
            if details.is_remote_ready() {
                return Ok(details);
            }
            tokio::time::timeout_at(expires, tokio::time::sleep(interval))
                .await
                .map_err(|_| {
                    anyhow::anyhow!("TorBox torrent {torrent_id} polling deadline exceeded")
                })?;
        }
    }

    async fn get_authenticated(&self, path: &str) -> anyhow::Result<String> {
        let response = self
            .http
            .get(self.endpoint(path))
            .bearer_auth(self.validated_api_key()?)
            .send()
            .await?;
        match response.status() {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                anyhow::bail!("TorBox authentication failed")
            }
            StatusCode::TOO_MANY_REQUESTS => {
                let retry_after = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("unknown");
                anyhow::bail!("TorBox rate limited; retry after {retry_after}")
            }
            status if !status.is_success() => anyhow::bail!("TorBox returned HTTP {status}"),
            _ => Ok(response
                .text()
                .await
                .context("TorBox response read failed")?),
        }
    }

    /// Create a cached torrent and return the internal TorBox torrent_id.
    ///
    /// For cached torrents TorBox creates the entry instantly (not rate-limited). The
    /// `hash` must be a lowercase hex string. Returns `(torrent_id, queued_id)`.
    pub async fn create_cached_torrent(&self, magnet_or_hash: &str) -> anyhow::Result<u64> {
        // Build magnet if bare hash
        let magnet = if magnet_or_hash.starts_with("magnet:") {
            magnet_or_hash.to_string()
        } else {
            format!(
                "magnet:?xt=urn:btih:{}&dn=flasharr",
                magnet_or_hash.trim().to_ascii_lowercase()
            )
        };
        let api_key = self.validated_api_key()?;
        let resp = self
            .http
            .post(self.endpoint("/torrents/createtorrent"))
            .bearer_auth(api_key)
            .multipart(reqwest::multipart::Form::new().text("magnet", magnet))
            .send()
            .await?;
        if matches!(
            resp.status(),
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
        ) {
            anyhow::bail!("TorBox authentication failed");
        }
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!(
                "TorBox createtorrent returned HTTP {}: {}",
                status,
                &body[..body.len().min(200)]
            );
        }
        let parsed: TorBoxResponse =
            serde_json::from_str(&body).context("TorBox createtorrent parse failed")?;
        if !parsed.success {
            anyhow::bail!(parsed
                .detail
                .unwrap_or_else(|| "TorBox createtorrent failed".to_string()));
        }
        // data.torrent_id (float)
        let torrent_id = parsed
            .data
            .get("torrent_id")
            .and_then(|v| v.as_f64())
            .map(|v| v as u64)
            .ok_or_else(|| {
                anyhow::anyhow!("TorBox createtorrent: missing torrent_id in response")
            })?;
        Ok(torrent_id)
    }

    pub async fn delete_torrent(&self, torrent_id: u64) -> anyhow::Result<()> {
        let api_key = self.validated_api_key()?;
        let resp = self
            .http
            .post(self.endpoint("/torrents/controltorrent"))
            .bearer_auth(api_key)
            .json(&serde_json::json!({ "torrent_id": torrent_id, "operation": "delete" }))
            .send()
            .await?;
        if matches!(
            resp.status(),
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
        ) {
            anyhow::bail!("TorBox authentication failed");
        }
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("TorBox delete torrent returned HTTP {}", status);
        }
        let parsed: TorBoxResponse =
            serde_json::from_str(&body).context("TorBox delete torrent parse failed")?;
        if !parsed.success {
            anyhow::bail!(parsed
                .detail
                .unwrap_or_else(|| "TorBox delete torrent failed".to_string()));
        }
        Ok(())
    }

    /// Request a time-limited CDN download link.
    ///
    /// `torrent_id` is the numeric ID returned by `create_cached_torrent`.
    /// `file_id` is `TorBoxFile::id` for the desired file; if 0 a zip link is requested.
    pub async fn get_download_link(&self, torrent_id: u64, file_id: u64) -> anyhow::Result<String> {
        self.validated_api_key()?;
        // token goes in query param (per TorBox docs)
        let url = self.request_download_endpoint(torrent_id, file_id);
        let resp = self.http.get(&url).send().await?;
        if matches!(
            resp.status(),
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
        ) {
            anyhow::bail!("TorBox authentication failed");
        }
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!(
                "TorBox requestdl returned HTTP {}: {}",
                status,
                &body[..body.len().min(200)]
            );
        }
        let parsed: TorBoxResponse =
            serde_json::from_str(&body).context("TorBox requestdl parse failed")?;
        if !parsed.success {
            anyhow::bail!(parsed
                .detail
                .unwrap_or_else(|| "TorBox requestdl failed".to_string()));
        }
        parsed
            .data
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("TorBox requestdl: data is not a string URL"))
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn request_download_endpoint(&self, torrent_id: u64, file_id: u64) -> String {
        // TorBox assigns file IDs from 0, so 0 is a valid single-file selection —
        // always request the explicit file, never fall back to a whole-torrent zip.
        format!(
            "{}/torrents/requestdl?token={}&torrent_id={}&file_id={}",
            self.base_url,
            urlencoding::encode(&self.api_key),
            torrent_id,
            file_id
        )
    }
}

/// Pick the largest file whose name looks like a media file.
/// Falls back to the globally largest file if no media file is found.
pub fn largest_media_file(files: &[TorBoxFile]) -> Option<&TorBoxFile> {
    if files.is_empty() {
        return None;
    }
    let media: Vec<&TorBoxFile> = files
        .iter()
        .filter(|f| is_media_filename(&f.name))
        .collect();
    let pool = if media.is_empty() {
        files.iter().collect::<Vec<_>>()
    } else {
        media
    };
    pool.into_iter().max_by_key(|f| f.size)
}

pub fn cache_display_file(hit: &TorBoxCacheHit) -> Option<&TorBoxFile> {
    largest_media_file(&hit.files)
}

fn is_media_filename(name: &str) -> bool {
    const EXTS: &[&str] = &[
        "mkv", "mp4", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "m2ts",
    ];
    name.rfind('.')
        .map(|dot| EXTS.contains(&name[dot + 1..].to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn normalized_hashes(hashes: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    hashes
        .iter()
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| !h.is_empty() && seen.insert(h.clone()))
        .collect()
}

fn parse_cache_response(body: &str) -> anyhow::Result<HashMap<String, TorBoxCacheHit>> {
    let parsed: TorBoxResponse =
        serde_json::from_str(body).context("TorBox cache response parse failed")?;
    if !parsed.success {
        anyhow::bail!(parsed
            .detail
            .unwrap_or_else(|| "TorBox cache check failed".to_string()));
    }
    Ok(parse_cache_data(parsed.data))
}

fn parse_torrent_info(mut data: Value, hash: &str) -> anyhow::Result<TorBoxCacheHit> {
    let object = data
        .as_object_mut()
        .context("TorBox torrentinfo data was not an object")?;
    object
        .entry("hash")
        .or_insert_with(|| Value::String(hash.to_ascii_lowercase()));
    serde_json::from_value(data).context("TorBox torrentinfo data parse failed")
}

fn parse_torrent_response(body: &str) -> anyhow::Result<TorBoxTorrentDetails> {
    let parsed: TorBoxResponse =
        serde_json::from_str(body).context("TorBox torrent response parse failed")?;
    if !parsed.success {
        let code = parsed.error.unwrap_or_else(|| "UNKNOWN_ERROR".to_string());
        let detail = parsed
            .detail
            .unwrap_or_else(|| "TorBox torrent request failed".to_string());
        anyhow::bail!("TorBox {code}: {detail}");
    }
    let data = match parsed.data {
        Value::Array(mut items) if items.len() == 1 => items.remove(0),
        Value::Array(items) if items.is_empty() => anyhow::bail!("TorBox torrent not found"),
        Value::Array(_) => anyhow::bail!("TorBox torrent response was ambiguous"),
        value => value,
    };
    serde_json::from_value(data).context("TorBox torrent data parse failed")
}

fn parse_cache_data(data: Value) -> HashMap<String, TorBoxCacheHit> {
    let mut out = HashMap::new();
    match data {
        Value::Object(map) => {
            for (key, mut value) in map {
                let hit = match value {
                    Value::Bool(true) => Some(TorBoxCacheHit {
                        hash: key.clone(),
                        name: None,
                        size: 0,
                        files: vec![],
                    }),
                    Value::Bool(false) => None,
                    Value::Object(ref mut object) => {
                        object
                            .entry("hash")
                            .or_insert_with(|| Value::String(key.clone()));
                        serde_json::from_value::<TorBoxCacheHit>(value).ok()
                    }
                    _ => serde_json::from_value::<TorBoxCacheHit>(value).ok(),
                };
                if let Some(mut hit) = hit {
                    if hit.hash.trim().is_empty() {
                        hit.hash = key;
                    }
                    out.insert(hit.hash.to_ascii_lowercase(), hit);
                }
            }
        }
        Value::Array(items) => {
            for value in items {
                if let Ok(hit) = serde_json::from_value::<TorBoxCacheHit>(value) {
                    out.insert(hit.hash.to_ascii_lowercase(), hit);
                }
            }
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_source_pinned_api_endpoints() {
        let client = TorBoxClient::new(
            TORBOX_API_BASE.into(),
            "test-key".into(),
            Arc::new(reqwest::Client::new()),
        );
        assert_eq!(
            client.endpoint("/user/me?settings=false"),
            "https://api.torbox.app/v1/api/user/me?settings=false"
        );
        assert_eq!(
            client.endpoint("/torrents/checkcached"),
            "https://api.torbox.app/v1/api/torrents/checkcached"
        );
        assert_eq!(
            client.endpoint("/torrents/createtorrent"),
            "https://api.torbox.app/v1/api/torrents/createtorrent"
        );
        assert_eq!(
            client.endpoint("/torrents/mylist?id=7&files=true"),
            "https://api.torbox.app/v1/api/torrents/mylist?id=7&files=true"
        );
        assert_eq!(client.request_download_endpoint(7, 9), "https://api.torbox.app/v1/api/torrents/requestdl?token=test-key&torrent_id=7&file_id=9");
        assert_eq!(client.request_download_endpoint(57_493_756, 0), "https://api.torbox.app/v1/api/torrents/requestdl?token=test-key&torrent_id=57493756&file_id=0");
    }

    #[test]
    fn parses_minimal_profile_without_serializing_secrets() {
        let parsed: TorBoxResponse = serde_json::from_str(r#"{"success":true,"data":{"email":"user@example.com","plan":2,"is_subscribed":true,"premium_expires_at":"2027-01-01T00:00:00+00:00","id":7,"settings":{"api_key":"secret"}}}"#).unwrap();
        let profile: TorBoxProfile = serde_json::from_value(parsed.data).unwrap();
        let json = serde_json::to_string(&profile).unwrap();
        assert_eq!(profile.email, "user@example.com");
        assert!(profile.is_subscribed);
        assert!(!json.contains("secret"));
        assert!(!json.contains("settings"));
        assert!(!json.contains("\"id\""));
    }

    #[test]
    fn parses_object_cache_hits() {
        let hits =
            parse_cache_response(r#"{"success":true,"data":{"ABC":{"name":"Movie","size":10}}}"#)
                .unwrap();
        assert_eq!(hits["abc"].name.as_deref(), Some("Movie"));
        assert_eq!(hits["abc"].hash, "ABC");
    }

    #[test]
    fn parses_boolean_cache_availability() {
        let hits =
            parse_cache_response(r#"{"success":true,"data":{"ABC":true,"DEF":false}}"#).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits["abc"].hash, "ABC");
        assert_eq!(hits["abc"].size, 0);
        assert!(hits["abc"].files.is_empty());
        assert!(!hits.contains_key("def"));
    }

    #[test]
    fn parses_list_cache_hits() {
        let hits = parse_cache_response(
            r#"{"success":true,"data":[{"name":"Movie","size":10,"hash":"DEF"}]}"#,
        )
        .unwrap();
        assert!(hits.contains_key("def"));
    }

    #[test]
    fn parses_file_list() {
        let hits = parse_cache_response(
            r#"{"success":true,"data":{"abc":{"hash":"abc","files":[{"id":7,"name":"x.mkv","size":99,"mimetype":"video/x-matroska"}]}}}"#,
        )
        .unwrap();
        assert_eq!(hits["abc"].files[0].id, 7);
    }

    #[test]
    fn failed_response_is_error() {
        let err = parse_cache_response(r#"{"success":false,"detail":"bad"}"#).unwrap_err();
        assert!(err.to_string().contains("bad"));
    }

    #[test]
    fn normalizes_hashes() {
        assert_eq!(
            normalized_hashes(&[" ABC ".into(), "abc".into(), "".into(), "DEF".into()]),
            vec!["abc".to_string(), "def".to_string()]
        );
    }

    #[test]
    fn largest_media_file_picks_largest_media() {
        let files = vec![
            TorBoxFile {
                id: 1,
                name: "subtitle.srt".to_string(),
                size: 999999,
                mimetype: None,
            },
            TorBoxFile {
                id: 2,
                name: "movie.mkv".to_string(),
                size: 1_000_000_000,
                mimetype: None,
            },
            TorBoxFile {
                id: 3,
                name: "sample.mp4".to_string(),
                size: 50_000_000,
                mimetype: None,
            },
        ];
        assert_eq!(largest_media_file(&files).map(|f| f.id), Some(2));
    }

    #[test]
    fn largest_media_file_falls_back_to_any_if_no_media_ext() {
        let files = vec![
            TorBoxFile {
                id: 1,
                name: "data.dat".to_string(),
                size: 100,
                mimetype: None,
            },
            TorBoxFile {
                id: 2,
                name: "other.bin".to_string(),
                size: 9999,
                mimetype: None,
            },
        ];
        assert_eq!(largest_media_file(&files).map(|f| f.id), Some(2));
    }

    #[test]
    fn largest_media_file_empty_returns_none() {
        assert!(largest_media_file(&[]).is_none());
    }

    #[test]
    fn torrent_info_injects_requested_hash_when_response_omits_it() {
        let response: TorBoxResponse = serde_json::from_str(include_str!(
            "../../tests/fixtures/torbox/phase9c_cached_file_zero.json"
        ))
        .unwrap();
        let hit =
            parse_torrent_info(response.data, "ABCDEF0123456789ABCDEF0123456789ABCDEF01").unwrap();
        assert_eq!(hit.hash, "abcdef0123456789abcdef0123456789abcdef01");
        assert_eq!(hit.files[0].size, 276_134_947);
    }

    #[test]
    fn parses_completed_torrent_details_with_manifest() {
        let torrent = parse_torrent_response(
            r#"{"success":true,"data":{"id":42,"download_state":"completed","download_finished":true,"download_present":true,"files":[{"id":7,"name":"movie.mkv","size":99}]}}"#,
        )
        .unwrap();
        assert_eq!(torrent.id, 42);
        assert_eq!(torrent.state, "completed");
        assert_eq!(torrent.files[0].id, 7);
        assert!(torrent.is_remote_ready());
    }

    #[test]
    fn production_cached_payload_with_file_zero_is_remote_ready() {
        let torrent = parse_torrent_response(include_str!(
            "../../tests/fixtures/torbox/phase9c_cached_file_zero.json"
        ))
        .unwrap();
        assert_eq!(torrent.id, 57_493_756);
        assert_eq!(torrent.state, "cached");
        assert!(!torrent.download_finished);
        assert!(torrent.download_present);
        assert_eq!(torrent.files[0].id, 0);
        assert_eq!(torrent.files[0].size, 276_134_947);
        assert_eq!(
            torrent.files[0].name,
            "bbb_sunflower_1080p_30fps_normal/bbb_sunflower_1080p_30fps_normal.mp4"
        );
        assert!(torrent.is_remote_ready());
    }

    #[test]
    fn cached_or_completed_requires_present_nonempty_valid_manifest() {
        let mut torrent = parse_torrent_response(include_str!(
            "../../tests/fixtures/torbox/phase9c_cached_file_zero.json"
        ))
        .unwrap();
        assert!(torrent.is_remote_ready());
        torrent.state = "completed".into();
        assert!(!torrent.is_remote_ready());
        torrent.download_finished = true;
        assert!(torrent.is_remote_ready());
        torrent.download_present = false;
        assert!(!torrent.is_remote_ready());
        torrent.download_present = true;
        torrent.files.clear();
        assert!(!torrent.is_remote_ready());
    }

    #[test]
    fn ready_manifest_rejects_zero_size_empty_name_and_missing_id() {
        let fixture = include_str!("../../tests/fixtures/torbox/phase9c_cached_file_zero.json");
        let mut torrent = parse_torrent_response(fixture).unwrap();
        torrent.files[0].size = 0;
        assert!(!torrent.is_remote_ready());

        torrent = parse_torrent_response(fixture).unwrap();
        torrent.files[0].name = "  ".into();
        assert!(!torrent.is_remote_ready());

        let missing_id = fixture.replace("\"id\": 0,", "");
        assert!(parse_torrent_response(&missing_id).is_err());
    }

    #[test]
    fn remote_ready_rejects_incomplete_or_invalid_manifest() {
        let mut torrent = TorBoxTorrentDetails {
            id: 42,
            state: "downloading".into(),
            download_finished: true,
            download_present: true,
            files: vec![TorBoxFile {
                id: 7,
                name: "movie.mkv".into(),
                size: 99,
                mimetype: None,
            }],
        };
        assert!(!torrent.is_remote_ready());
        torrent.state = "completed".into();
        torrent.files[0].name.clear();
        assert!(!torrent.is_remote_ready());
    }

    #[test]
    fn parses_singleton_list_and_rejects_terminal_state() {
        let torrent = parse_torrent_response(
            r#"{"success":true,"data":[{"id":42,"download_state":"reported missing","files":[]}]}"#,
        )
        .unwrap();
        assert!(torrent
            .terminal_error()
            .unwrap()
            .contains("reported missing"));
    }

    #[test]
    fn parser_preserves_api_error_code() {
        let error = parse_torrent_response(
            r#"{"success":false,"error":"AUTH_ERROR","detail":"bad token","data":null}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("AUTH_ERROR"));
        assert!(error.contains("bad token"));
    }

    #[tokio::test]
    async fn poll_rejects_zero_interval_or_deadline() {
        let client = TorBoxClient::new(
            "http://127.0.0.1:1".into(),
            "valid-test-key".into(),
            Arc::new(reqwest::Client::new()),
        );
        let error = client
            .poll_remote_ready(
                42,
                std::time::Duration::ZERO,
                std::time::Duration::from_secs(1),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("poll interval"));
    }
}
