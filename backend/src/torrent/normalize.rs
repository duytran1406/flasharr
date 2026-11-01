use crate::torbox::client::{largest_media_file, TorBoxClient};
use crate::AppState;
use axum::http::StatusCode;
use moka::future::Cache;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TorrentAddInput {
    pub url: Option<String>,
    pub download_ref: Option<String>,
    pub filename: Option<String>,
    pub source: Option<String>,
    pub provider: Option<String>,
    pub info_hash: Option<String>,
    pub size: Option<u64>,
    pub external_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MaterializedDownload {
    pub queue_url: String,
    pub host: String,
    pub source: Option<String>,
    pub provider: Option<String>,
    pub size: Option<u64>,
    pub external_id: Option<String>,
    pub info_hash: Option<String>,
}

pub fn build_magnet_url(info_hash: &str, filename: Option<&str>) -> Option<String> {
    let hash = crate::torrent::normalize_info_hash(info_hash)?;
    let mut magnet = format!("magnet:?xt=urn:btih:{hash}");
    if let Some(filename) = filename.map(str::trim).filter(|value| !value.is_empty()) {
        magnet.push_str("&dn=");
        magnet.push_str(&urlencoding::encode(filename));
    }
    Some(magnet)
}

pub fn is_upload_torrent_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    !lower.starts_with("magnet:")
        && (lower.contains("apikey=")
            || lower.contains("/download?")
            || lower.contains("download.php?id=")
            || lower.contains(".torrent"))
}

fn prowlarr_ref_from_url(value: &str) -> Option<&str> {
    value.strip_prefix("prowlarr-ref://")?.split('?').next()
}

fn join_base_url(base: &str, path: &str) -> Option<String> {
    let base = reqwest::Url::parse(base).ok()?;
    Some(base.join(path).ok()?.to_string())
}

fn is_same_origin(base: &str, url: &str) -> bool {
    let Ok(base_url) = reqwest::Url::parse(base) else {
        return false;
    };
    let Ok(target_url) = reqwest::Url::parse(url) else {
        return false;
    };
    base_url.scheme() == target_url.scheme()
        && base_url.host_str() == target_url.host_str()
        && base_url.port_or_known_default() == target_url.port_or_known_default()
}

fn with_api_key_query(url: &str, api_key: &str) -> Option<String> {
    let mut parsed = reqwest::Url::parse(url).ok()?;
    if parsed
        .query_pairs()
        .any(|(key, _)| key.eq_ignore_ascii_case("apikey"))
    {
        return Some(parsed.to_string());
    }
    parsed.query_pairs_mut().append_pair("apikey", api_key);
    Some(parsed.to_string())
}

fn prepare_prowlarr_fetch(state: &Arc<AppState>, raw_url: &str) -> (String, Option<String>) {
    let prowlarr_base = state
        .db
        .get_setting("prowlarr_url")
        .ok()
        .flatten()
        .filter(|v| !v.trim().is_empty());
    let prowlarr_api_key = state
        .db
        .get_setting("prowlarr_api_key")
        .ok()
        .flatten()
        .filter(|value| crate::api::settings::validate_api_key(value).is_ok());

    let request_url = if raw_url.starts_with('/') {
        prowlarr_base
            .as_deref()
            .and_then(|base| join_base_url(base, raw_url))
            .unwrap_or_else(|| raw_url.to_string())
    } else {
        raw_url.to_string()
    };

    match (prowlarr_base.as_deref(), prowlarr_api_key.as_deref()) {
        (Some(base), Some(api_key))
            if raw_url.starts_with('/') || is_same_origin(base, &request_url) =>
        {
            if let Some(url_with_key) = with_api_key_query(&request_url, api_key) {
                (url_with_key, None)
            } else {
                (request_url, Some(api_key.to_string()))
            }
        }
        _ => (request_url, None),
    }
}

pub fn normalize_torrent_url(state: &Arc<AppState>, raw_url: &str) -> String {
    prepare_prowlarr_fetch(state, raw_url).0
}

pub async fn resolve_download_ref(
    refs: &Cache<String, String>,
    url: Option<&str>,
    download_ref: Option<&str>,
) -> Result<Option<String>, (StatusCode, String)> {
    let explicit_ref = download_ref.map(str::trim).filter(|v| !v.is_empty());
    let url_ref = url.and_then(|url| prowlarr_ref_from_url(url.trim()));
    if let Some(download_ref) = explicit_ref.or(url_ref) {
        refs.get(download_ref).await.map(Some).ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "download_ref is missing or expired".to_string(),
            )
        })
    } else {
        Ok(None)
    }
}

async fn derive_info_hash_from_fetch_request(
    client: &reqwest::Client,
    request_url: &str,
    header_api_key: Option<&str>,
) -> Result<Option<String>, (StatusCode, String)> {
    let mut request = client.get(request_url);
    if let Some(api_key) = header_api_key {
        request = request.header("X-Api-Key", api_key);
    }

    let response = tokio::time::timeout(std::time::Duration::from_secs(15), request.send())
        .await
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "torrent result is missing info_hash and protected torrent URL fetch timed out"
                    .to_string(),
            )
        })?
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!(
                    "torrent result is missing info_hash and protected torrent URL could not be fetched: {}",
                    e
                ),
            )
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "torrent result is missing info_hash and protected torrent URL returned HTTP {}",
                status
            ),
        ));
    }

    let bytes = response.bytes().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!(
                "torrent result is missing info_hash and protected torrent body could not be read: {}",
                e
            ),
        )
    })?;
    let hash = crate::torrent::torrent_info_hash(&bytes);
    if hash.is_none() {
        tracing::warn!(
            status = %status,
            bytes = bytes.len(),
            "trusted torrent locator returned a non-metainfo body"
        );
    }
    Ok(hash)
}

pub async fn derive_info_hash_from_torrent_url(
    state: &Arc<AppState>,
    url: &str,
) -> Result<Option<String>, (StatusCode, String)> {
    if !is_upload_torrent_url(url) {
        return Ok(None);
    }

    derive_info_hash_from_resolved_torrent_url(state, url).await
}

/// Fetch a locator previously stored server-side by Flasharr.
pub async fn derive_info_hash_from_resolved_torrent_url(
    state: &Arc<AppState>,
    url: &str,
) -> Result<Option<String>, (StatusCode, String)> {
    let (request_url, header_api_key) = prepare_prowlarr_fetch(state, url);
    derive_info_hash_from_fetch_request(&state.http_client, &request_url, header_api_key.as_deref())
        .await
}

fn should_fallback_from_torbox(status: StatusCode, message: &str) -> bool {
    status == StatusCode::BAD_GATEWAY
        || status == StatusCode::GATEWAY_TIMEOUT
        || (status == StatusCode::BAD_REQUEST
            && message.to_ascii_lowercase().contains("torbox cache miss"))
}

async fn resolve_torbox_direct_url(
    state: &Arc<AppState>,
    info_hash: &str,
) -> Result<(String, String, Option<u64>), (StatusCode, String)> {
    let settings = crate::api::settings::load_torbox_settings_public(state);
    if !settings.enabled || settings.api_key.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "TorBox is not configured".to_string(),
        ));
    }
    let hash = info_hash.trim().to_ascii_lowercase();
    if hash.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "info_hash is required for TorBox downloads".to_string(),
        ));
    }
    let client = TorBoxClient::new(
        settings.base_url,
        settings.api_key,
        Arc::clone(&state.http_client),
    );
    let hits = tokio::time::timeout(
        std::time::Duration::from_secs(settings.cache_timeout_secs),
        client.check_cache(&[hash.clone()]),
    )
    .await
    .map_err(|_| {
        (
            StatusCode::GATEWAY_TIMEOUT,
            "TorBox cache check timed out".to_string(),
        )
    })?
    .map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("TorBox cache check failed: {}", e),
        )
    })?;
    let hit = hits
        .get(&hash)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "TorBox cache miss".to_string()))?;
    let file = largest_media_file(&hit.files);
    let file_id = file.map(|f| f.id).unwrap_or(0);
    let size = file
        .map(|f| f.size)
        .or_else(|| (hit.size > 0).then_some(hit.size));
    let torrent_id = client.create_cached_torrent(&hash).await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("TorBox create torrent failed: {}", e),
        )
    })?;
    let url = client
        .get_download_link(torrent_id, file_id)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                format!("TorBox link failed: {}", e),
            )
        })?;
    Ok((url, format!("{}:{}", torrent_id, file_id), size))
}

pub async fn materialize_download(
    state: &Arc<AppState>,
    refs: &Cache<String, String>,
    input: TorrentAddInput,
) -> Result<MaterializedDownload, (StatusCode, String)> {
    let explicit_torbox =
        input.source.as_deref() == Some("torbox") || input.provider.as_deref() == Some("torbox");
    let explicit_torrent = input.source.as_deref() == Some("torrent")
        || input.provider.as_deref() == Some("qbittorrent");

    let resolved_url =
        match resolve_download_ref(refs, input.url.as_deref(), input.download_ref.as_deref())
            .await?
        {
            Some(url) => Some(url),
            None => input.url.clone().filter(|v| !v.trim().is_empty()),
        };

    let normalized_url = resolved_url
        .as_deref()
        .map(|url| normalize_torrent_url(state, url));
    let torrent_url = normalized_url.clone().unwrap_or_default();
    let is_torrent = explicit_torrent
        || (!explicit_torbox
            && (!torrent_url.is_empty()
                && (torrent_url.to_ascii_lowercase().starts_with("magnet:")
                    || is_upload_torrent_url(&torrent_url))));

    let effective_info_hash = match input.info_hash.clone() {
        Some(hash) => Some(hash),
        None => match normalized_url
            .as_deref()
            .and_then(crate::torrent::extract_btih)
        {
            Some(hash) => Some(hash),
            None if !torrent_url.is_empty() && (explicit_torbox || is_torrent) => {
                derive_info_hash_from_torrent_url(state, &torrent_url)
                    .await
                    .ok()
                    .flatten()
            }
            None => None,
        },
    };

    let qb_queue_url = if !torrent_url.is_empty() {
        torrent_url.clone()
    } else if let Some(magnet) = effective_info_hash
        .as_deref()
        .and_then(|hash| build_magnet_url(hash, input.filename.as_deref()))
    {
        magnet
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            "url, download_ref, or info_hash is required".to_string(),
        ));
    };

    if explicit_torbox {
        if let Some(hash) = effective_info_hash.clone() {
            match resolve_torbox_direct_url(state, &hash).await {
                Ok((direct_url, torbox_external_id, torbox_size)) => {
                    return Ok(MaterializedDownload {
                        queue_url: direct_url,
                        host: "torbox".to_string(),
                        source: Some("torbox".to_string()),
                        provider: Some("torbox".to_string()),
                        size: torbox_size.or(input.size),
                        external_id: Some(input.external_id.unwrap_or(torbox_external_id)),
                        info_hash: Some(hash),
                    });
                }
                Err((status, message)) if should_fallback_from_torbox(status, &message) => {}
                Err(err) => return Err(err),
            }
        }

        return Ok(MaterializedDownload {
            queue_url: qb_queue_url,
            host: "qbittorrent".to_string(),
            source: Some("torrent".to_string()),
            provider: Some("qbittorrent".to_string()),
            size: input.size,
            external_id: None,
            info_hash: effective_info_hash,
        });
    }

    if is_torrent {
        return Ok(MaterializedDownload {
            queue_url: qb_queue_url,
            host: "qbittorrent".to_string(),
            source: Some("torrent".to_string()),
            provider: Some("qbittorrent".to_string()),
            size: input.size,
            external_id: None,
            info_hash: effective_info_hash,
        });
    }

    let queue_url = normalized_url.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "url or download_ref is required".to_string(),
        )
    })?;
    Ok(MaterializedDownload {
        queue_url,
        host: "fshare".to_string(),
        source: input.source,
        provider: input.provider,
        size: input.size,
        external_id: input.external_id,
        info_hash: effective_info_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_magnet_from_hash_and_name() {
        assert_eq!(
            build_magnet_url(
                "22A2E1DF69B9016D5130D3E20DEEC478EEDADF8B",
                Some("Lion King")
            ),
            Some(
                "magnet:?xt=urn:btih:22a2e1df69b9016d5130d3e20deec478eedadf8b&dn=Lion%20King"
                    .to_string()
            )
        );
        assert_eq!(build_magnet_url("", Some("x")), None);
    }

    #[test]
    fn detects_upload_style_torrent_urls() {
        assert!(is_upload_torrent_url(
            "http://prowlarr/1/download?apikey=secret&link=x"
        ));
        assert!(is_upload_torrent_url(
            "https://example.test/download.php?id=204885"
        ));
        assert!(is_upload_torrent_url("https://example.test/file.torrent"));
        assert!(!is_upload_torrent_url("magnet:?xt=urn:btih:ABC123"));
    }

    #[test]
    fn joins_relative_prowlarr_path() {
        let joined = join_base_url("http://prowlarr.local:9696", "/download/client/abc");
        assert_eq!(
            joined.as_deref(),
            Some("http://prowlarr.local:9696/download/client/abc")
        );
    }

    #[test]
    fn matches_same_origin() {
        assert!(is_same_origin(
            "http://prowlarr.local:9696",
            "http://prowlarr.local:9696/download/client/abc"
        ));
        assert!(!is_same_origin(
            "http://prowlarr.local:9696",
            "https://prowlarr.local:9696/download/client/abc"
        ));
    }

    #[test]
    fn appends_api_key_when_missing() {
        let actual = with_api_key_query("http://prowlarr.local:9696/download/client/abc", "secret");
        assert_eq!(
            actual.as_deref(),
            Some("http://prowlarr.local:9696/download/client/abc?apikey=secret")
        );
    }

    #[tokio::test]
    async fn derive_info_hash_from_protected_torrent_url() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr = listener.local_addr().expect("listener addr");
        let torrent = b"d8:announce15:https://tracker4:infod6:lengthi123e4:name4:testee";
        let expected = crate::torrent::torrent_info_hash(torrent);

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept connection");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/x-bittorrent\r\nConnection: close\r\n\r\n",
                torrent.len()
            );
            use tokio::io::AsyncWriteExt;
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write headers");
            stream.write_all(torrent).await.expect("write body");
        });

        let client = reqwest::Client::new();
        let actual = derive_info_hash_from_fetch_request(
            &client,
            &format!("http://{}/download?apikey=secret", addr),
            None,
        )
        .await
        .unwrap();
        server.await.expect("server join");

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn resolves_prowlarr_ref_or_fails_400() {
        let refs = Cache::new(10);
        refs.insert(
            "abc123".to_string(),
            "https://provider/download?apikey=secret".to_string(),
        )
        .await;

        assert_eq!(
            resolve_download_ref(&refs, Some("prowlarr-ref://abc123"), None)
                .await
                .unwrap(),
            Some("https://provider/download?apikey=secret".to_string())
        );
        assert_eq!(
            resolve_download_ref(&refs, None, Some("abc123"))
                .await
                .unwrap(),
            Some("https://provider/download?apikey=secret".to_string())
        );
        assert_eq!(
            resolve_download_ref(&refs, Some("https://example.test/file.torrent"), None)
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            resolve_download_ref(&refs, Some("prowlarr-ref://expired"), None)
                .await
                .unwrap_err()
                .0,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn torbox_failures_fallback_only_for_upstreamish_cases() {
        assert!(should_fallback_from_torbox(
            StatusCode::BAD_GATEWAY,
            "TorBox link failed: HTTP 502"
        ));
        assert!(should_fallback_from_torbox(
            StatusCode::GATEWAY_TIMEOUT,
            "TorBox cache check timed out"
        ));
        assert!(should_fallback_from_torbox(
            StatusCode::BAD_REQUEST,
            "TorBox cache miss"
        ));
        assert!(!should_fallback_from_torbox(
            StatusCode::BAD_REQUEST,
            "TorBox is not configured"
        ));
    }
}
