use super::model::{DownloadSelection, MaterializedDownload, SourceFailure, SourceKind};
use crate::AppState;
use futures_util::StreamExt;
use moka::future::Cache;
use std::net::IpAddr;
use std::sync::Arc;

pub fn extract_btih(value: &str) -> Option<String> {
    crate::torrent::extract_btih(value)
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

fn prepare_prowlarr_fetch(state: &Arc<AppState>, raw_url: &str) -> (String, Option<String>, bool) {
    let base = state
        .db
        .get_setting("prowlarr_url")
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty());
    let api_key = state
        .db
        .get_setting("prowlarr_api_key")
        .ok()
        .flatten()
        .filter(|value| crate::api::settings::validate_api_key(value).is_ok());
    let request_url = if raw_url.starts_with('/') {
        base.as_deref()
            .and_then(|base| join_base_url(base, raw_url))
            .unwrap_or_else(|| raw_url.to_string())
    } else {
        raw_url.to_string()
    };

    let allow_private_initial = base
        .as_deref()
        .is_some_and(|base| allow_private_initial_target(base, &request_url));
    match (base.as_deref(), api_key.as_deref()) {
        (Some(base), Some(api_key))
            if raw_url.starts_with('/') || is_same_origin(base, &request_url) =>
        {
            with_api_key_query(&request_url, api_key)
                .map(|url| (url, None, allow_private_initial))
                .unwrap_or_else(|| {
                    (
                        request_url,
                        Some(api_key.to_string()),
                        allow_private_initial,
                    )
                })
        }
        _ => (request_url, None, allow_private_initial),
    }
}

fn allow_private_initial_target(configured_prowlarr: &str, target: &str) -> bool {
    is_same_origin(configured_prowlarr, target)
}

async fn resolve_download_ref(
    refs: &Cache<String, String>,
    url: Option<&str>,
    download_ref: Option<&str>,
) -> Option<String> {
    let explicit = download_ref
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let url_ref = url.and_then(|url| prowlarr_ref_from_url(url.trim()));
    let download_ref = explicit.or(url_ref)?;
    refs.get(download_ref).await
}

fn supplied_info_hash(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| crate::torrent::qbittorrent::valid_torrent_hash(value))
        .map(str::to_ascii_lowercase)
}

async fn materialize_info_hash<F>(
    supplied_hash: Option<String>,
    url: Option<&str>,
    derive: F,
) -> Result<Option<String>, (axum::http::StatusCode, SourceFailure)>
where
    F: std::future::Future<
        Output = Result<Option<String>, (axum::http::StatusCode, SourceFailure)>,
    >,
{
    if let Some(hash) = supplied_hash.or_else(|| url.and_then(extract_btih)) {
        return Ok(Some(hash));
    }
    derive.await
}

fn queue_url_from_locator_or_hash(
    url: Option<String>,
    info_hash: Option<&str>,
    filename: Option<&str>,
) -> Result<String, (axum::http::StatusCode, SourceFailure)> {
    if url
        .as_deref()
        .is_some_and(|value| value.starts_with("magnet:"))
    {
        url
    } else {
        info_hash
            .and_then(|hash| build_magnet_url(hash, filename))
            .or(url)
    }
    .ok_or_else(|| SourceFailure::bad_request("url, download_ref, or info_hash is required"))
}

const MAX_TORRENT_BYTES: u64 = 10 * 1024 * 1024;
const MAX_REDIRECTS: usize = 4;
const TORRENT_FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

fn is_disallowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, ..] = ip.octets();
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_unspecified()
                || ip.is_broadcast()
                || a == 0
                || (a == 100 && (64..=127).contains(&b))
        }
        IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return is_disallowed_ip(IpAddr::V4(mapped));
            }
            let first = ip.segments()[0];
            ip.is_loopback()
                || ip.is_multicast()
                || ip.is_unspecified()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
        }
    }
}

#[derive(Debug)]
struct RedirectPlan {
    url: reqwest::Url,
    host: String,
    forward_credentials: bool,
}

fn plan_redirect(
    credential_origin: &str,
    target: &str,
    allow_cross_origin: bool,
) -> Result<RedirectPlan, (axum::http::StatusCode, SourceFailure)> {
    let mut url = reqwest::Url::parse(target).map_err(|_| {
        SourceFailure::bad_request("torrent redirect rejected: target_scheme_other")
    })?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(SourceFailure::bad_request(
            "torrent redirect rejected: target_userinfo",
        ));
    }
    if !matches!(url.scheme(), "http" | "https") {
        let reason = if url.scheme() == "magnet" {
            "target_magnet"
        } else {
            "target_scheme_other"
        };
        return Err(SourceFailure::bad_request(format!(
            "torrent redirect rejected: {reason}"
        )));
    }
    let forward_credentials = is_same_origin(credential_origin, url.as_str());
    // Same-origin hops keep the initial scheme (http allowed). Any cross-origin
    // hop is only permitted when explicitly allowed AND must be https.
    if !forward_credentials {
        if !allow_cross_origin {
            return Err(SourceFailure::bad_request(
                "torrent redirect rejected: strict_same_origin",
            ));
        }
        if url.scheme() != "https" {
            return Err(SourceFailure::bad_request(
                "torrent redirect rejected: cross_origin_non_https",
            ));
        }
    }
    if !forward_credentials {
        let pairs = url
            .query_pairs()
            .filter(|(key, _)| !key.eq_ignore_ascii_case("apikey"))
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        url.query_pairs_mut().clear().extend_pairs(pairs);
        if url.query() == Some("") {
            url.set_query(None);
        }
    }
    let host = url
        .host_str()
        .ok_or_else(|| {
            SourceFailure::bad_request("torrent redirect rejected: target_host_missing")
        })?
        .to_string();
    Ok(RedirectPlan {
        url,
        host,
        forward_credentials,
    })
}

fn validate_resolved_ips(
    ips: impl IntoIterator<Item = IpAddr>,
) -> Result<(), (axum::http::StatusCode, SourceFailure)> {
    let mut found = false;
    for ip in ips {
        found = true;
        if is_disallowed_ip(ip) {
            return Err(SourceFailure::bad_request(
                "torrent redirect rejected: target_private_ip",
            ));
        }
    }
    if !found {
        return Err(SourceFailure::bad_request(
            "torrent redirect rejected: dns_resolution_failed",
        ));
    }
    Ok(())
}

fn redirect_pin(
    plan: &RedirectPlan,
    existing: Option<(String, Vec<std::net::SocketAddr>)>,
    resolved_cross_origin: Vec<std::net::SocketAddr>,
) -> Result<Option<(String, Vec<std::net::SocketAddr>)>, (axum::http::StatusCode, SourceFailure)> {
    if plan.forward_credentials {
        return Ok(existing);
    }
    validate_resolved_ips(resolved_cross_origin.iter().map(|address| address.ip()))?;
    Ok(Some((plan.host.clone(), resolved_cross_origin)))
}

async fn resolve_redirect_pin(
    plan: &RedirectPlan,
    existing: Option<(String, Vec<std::net::SocketAddr>)>,
) -> Result<Option<(String, Vec<std::net::SocketAddr>)>, (axum::http::StatusCode, SourceFailure)> {
    if plan.forward_credentials {
        return Ok(existing);
    }
    let port = plan.url.port_or_known_default().unwrap_or(443);
    let addresses = tokio::net::lookup_host((plan.host.as_str(), port))
        .await
        .map_err(|_| {
            SourceFailure::bad_request("torrent redirect rejected: dns_resolution_failed")
        })?
        .collect();
    redirect_pin(plan, existing, addresses)
}

async fn resolve_url_addrs(
    url: &reqwest::Url,
    host: &str,
) -> Result<Vec<std::net::SocketAddr>, (axum::http::StatusCode, SourceFailure)> {
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| {
            SourceFailure::bad_request("torrent redirect rejected: dns_resolution_failed")
        })?
        .collect::<Vec<_>>();
    validate_resolved_ips(addresses.iter().map(|address| address.ip()))?;
    Ok(addresses)
}

async fn read_bounded_torrent_response(
    response: reqwest::Response,
    limit: u64,
) -> Result<Vec<u8>, (axum::http::StatusCode, SourceFailure)> {
    if response.content_length().is_some_and(|size| size > limit) {
        return Err(SourceFailure::bad_request("torrent body is too large"));
    }
    let mut bytes = Vec::new();
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(|_| SourceFailure::bad_request("torrent body read failed"))?;
        if bytes.len().saturating_add(chunk.len()) > limit as usize {
            return Err(SourceFailure::bad_request("torrent body is too large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(crate) async fn fetch_torrent_bytes(
    _client: &reqwest::Client,
    url: &str,
    header_api_key: Option<&str>,
    allow_cross_origin: bool,
) -> Result<Vec<u8>, (axum::http::StatusCode, SourceFailure)> {
    fetch_torrent_bytes_with_policy(url, header_api_key, allow_cross_origin, false).await
}

async fn fetch_torrent_bytes_with_policy(
    url: &str,
    header_api_key: Option<&str>,
    allow_cross_origin: bool,
    allow_private_initial: bool,
) -> Result<Vec<u8>, (axum::http::StatusCode, SourceFailure)> {
    tokio::time::timeout(
        TORRENT_FETCH_TIMEOUT,
        fetch_torrent_bytes_inner(
            url,
            header_api_key,
            allow_cross_origin,
            allow_private_initial,
        ),
    )
    .await
    .map_err(|_| SourceFailure::bad_request("torrent URL fetch timed out"))?
}

async fn fetch_torrent_bytes_inner(
    url: &str,
    header_api_key: Option<&str>,
    allow_cross_origin: bool,
    allow_private_initial: bool,
) -> Result<Vec<u8>, (axum::http::StatusCode, SourceFailure)> {
    let initial = reqwest::Url::parse(url)
        .map_err(|_| SourceFailure::bad_request("torrent URL is malformed"))?;
    if !matches!(initial.scheme(), "http" | "https") {
        return Err(SourceFailure::bad_request(
            "torrent URL must use http or https",
        ));
    }
    let initial_host = initial
        .host_str()
        .ok_or_else(|| SourceFailure::bad_request("torrent URL host is missing"))?
        .to_string();
    let initial_addresses = if allow_private_initial {
        let port = initial.port_or_known_default().unwrap_or(443);
        tokio::net::lookup_host((initial_host.as_str(), port))
            .await
            .map_err(|_| {
                SourceFailure::bad_request("torrent redirect rejected: dns_resolution_failed")
            })?
            .collect()
    } else {
        resolve_url_addrs(&initial, &initial_host).await?
    };
    fetch_torrent_bytes_from_validated(
        initial.clone(),
        initial,
        Some((initial_host, initial_addresses)),
        true,
        header_api_key,
        allow_cross_origin,
    )
    .await
}

async fn fetch_torrent_bytes_from_validated(
    initial: reqwest::Url,
    mut current: reqwest::Url,
    mut pinned_host: Option<(String, Vec<std::net::SocketAddr>)>,
    mut forward_credentials: bool,
    header_api_key: Option<&str>,
    allow_cross_origin: bool,
) -> Result<Vec<u8>, (axum::http::StatusCode, SourceFailure)> {
    for hop in 0..=MAX_REDIRECTS {
        let mut client = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
        if let Some((host, addresses)) = pinned_host.as_ref() {
            client = client.resolve_to_addrs(host, addresses);
        }
        let client = client
            .build()
            .map_err(|_| SourceFailure::bad_request("torrent HTTP client unavailable"))?;
        let mut request = client.get(current.clone());
        if forward_credentials {
            if let Some(api_key) = header_api_key {
                request = request.header("X-Api-Key", api_key);
            }
        }
        let response = request.send().await.map_err(|error| {
            SourceFailure::bad_request(format!("torrent URL fetch failed: {error}"))
        })?;
        if response.status().is_redirection() {
            if hop == MAX_REDIRECTS {
                return Err(SourceFailure::bad_request(
                    "torrent redirect rejected: redirect_limit",
                ));
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| {
                    SourceFailure::bad_request(
                        "torrent redirect rejected: location_missing_or_invalid",
                    )
                })?;
            let next = current.join(location).map_err(|_| {
                SourceFailure::bad_request("torrent redirect rejected: location_join_failed")
            })?;
            let plan = plan_redirect(initial.as_str(), next.as_str(), allow_cross_origin)?;
            pinned_host = resolve_redirect_pin(&plan, pinned_host).await?;
            forward_credentials = plan.forward_credentials;
            current = plan.url;
            continue;
        }
        if !response.status().is_success() {
            return Err(SourceFailure::bad_request(format!(
                "torrent URL returned HTTP {}",
                response.status()
            )));
        }
        if let Some(content_type) = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
        {
            if !content_type.split(';').next().is_some_and(|value| {
                matches!(
                    value.trim(),
                    "application/x-bittorrent" | "application/octet-stream"
                )
            }) {
                return Err(SourceFailure::bad_request(
                    "torrent response type is invalid",
                ));
            }
        }
        return read_bounded_torrent_response(response, MAX_TORRENT_BYTES).await;
    }
    unreachable!()
}

async fn fetch_torrent_info_hash(
    _client: &reqwest::Client,
    url: &str,
    header_api_key: Option<&str>,
) -> Result<Option<String>, (axum::http::StatusCode, SourceFailure)> {
    fetch_torrent_info_hash_with_policy(url, header_api_key, false).await
}

async fn fetch_torrent_info_hash_with_policy(
    url: &str,
    header_api_key: Option<&str>,
    allow_private_initial: bool,
) -> Result<Option<String>, (axum::http::StatusCode, SourceFailure)> {
    tokio::time::timeout(TORRENT_FETCH_TIMEOUT, async {
        let initial = reqwest::Url::parse(url)
            .map_err(|_| SourceFailure::bad_request("torrent URL is malformed"))?;
        if !matches!(initial.scheme(), "http" | "https") {
            return Err(SourceFailure::bad_request(
                "torrent URL must use http or https",
            ));
        }
        let host = initial
            .host_str()
            .ok_or_else(|| SourceFailure::bad_request("torrent URL host is missing"))?
            .to_string();
        let addresses = if allow_private_initial {
            let port = initial.port_or_known_default().unwrap_or(443);
            tokio::net::lookup_host((host.as_str(), port))
                .await
                .map_err(|_| {
                    SourceFailure::bad_request("torrent redirect rejected: dns_resolution_failed")
                })?
                .collect()
        } else {
            resolve_url_addrs(&initial, &host).await?
        };
        let redirect_probe = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .resolve_to_addrs(&host, &addresses)
            .build()
            .map_err(|_| SourceFailure::bad_request("torrent HTTP client unavailable"))?;
        let mut request = redirect_probe.get(initial.clone());
        if let Some(api_key) = header_api_key {
            request = request.header("X-Api-Key", api_key);
        }
        let response = request
            .send()
            .await
            .map_err(|_| SourceFailure::bad_request("torrent URL fetch failed"))?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| {
                    SourceFailure::bad_request(
                        "torrent redirect rejected: location_missing_or_invalid",
                    )
                })?;
            if let Some(hash) = extract_btih(location) {
                return Ok(Some(hash));
            }
            let target = initial.join(location).map_err(|_| {
                SourceFailure::bad_request("torrent redirect rejected: location_join_failed")
            })?;
            let plan = plan_redirect(initial.as_str(), target.as_str(), true)?;
            let pinned =
                resolve_redirect_pin(&plan, Some((host.clone(), addresses.clone()))).await?;
            return Ok(crate::torrent::torrent_info_hash(
                &fetch_torrent_bytes_from_validated(
                    initial,
                    plan.url,
                    pinned,
                    plan.forward_credentials,
                    header_api_key,
                    true,
                )
                .await?,
            ));
        }
        if !response.status().is_success() {
            return Err(SourceFailure::bad_request(format!(
                "torrent URL returned HTTP {}",
                response.status()
            )));
        }
        let bytes = read_bounded_torrent_response(response, MAX_TORRENT_BYTES).await?;
        Ok(crate::torrent::torrent_info_hash(&bytes))
    })
    .await
    .map_err(|_| SourceFailure::bad_request("torrent URL fetch timed out"))?
}

pub(crate) async fn resolve_download_ref_info_hash(
    state: &Arc<AppState>,
    download_ref: &str,
) -> Result<Option<String>, String> {
    let raw = resolve_download_ref(&state.torrent_download_refs, None, Some(download_ref))
        .await
        .ok_or("download ref unavailable")?;
    if !is_upload_torrent_url(&raw) {
        return Err("locator is not a torrent download".into());
    }
    let (url, key, allow_private_initial) = prepare_prowlarr_fetch(state, &raw);
    fetch_torrent_info_hash_with_policy(&url, key.as_deref(), allow_private_initial)
        .await
        .map_err(|error| error.1.message)
}

pub(crate) async fn fetch_download_ref_bytes(
    state: &Arc<AppState>,
    download_ref: &str,
) -> Result<Vec<u8>, String> {
    let raw = resolve_download_ref(&state.torrent_download_refs, None, Some(download_ref))
        .await
        .ok_or("download ref unavailable")?;
    if !is_upload_torrent_url(&raw) {
        return Err("locator is not a torrent download".into());
    }
    let (url, key, allow_private_initial) = prepare_prowlarr_fetch(state, &raw);
    fetch_torrent_bytes_with_policy(&url, key.as_deref(), true, allow_private_initial)
        .await
        .map_err(|error| error.1.message)
}

async fn derive_info_hash_from_url(
    state: &Arc<AppState>,
    url: &str,
) -> Result<Option<String>, (axum::http::StatusCode, SourceFailure)> {
    if !is_upload_torrent_url(url) {
        return Ok(None);
    }
    let (url, header_api_key, allow_private_initial) = prepare_prowlarr_fetch(state, url);
    fetch_torrent_info_hash_with_policy(&url, header_api_key.as_deref(), allow_private_initial)
        .await
}

pub async fn materialize(
    state: &Arc<AppState>,
    refs: &Cache<String, String>,
    selection: DownloadSelection,
) -> Result<MaterializedDownload, (axum::http::StatusCode, SourceFailure)> {
    let supplied_hash = supplied_info_hash(selection.info_hash.as_deref());
    let resolved_url = resolve_download_ref(
        refs,
        selection.url.as_deref(),
        selection.download_ref.as_deref(),
    )
    .await
    .or_else(|| {
        selection
            .url
            .as_deref()
            .filter(|url| prowlarr_ref_from_url(url.trim()).is_none())
            .map(str::to_string)
    });
    let url = resolved_url
        .as_deref()
        .map(|url| prepare_prowlarr_fetch(state, url).0);
    let info_hash = materialize_info_hash(supplied_hash, url.as_deref(), async {
        match url.as_deref() {
            Some(url) => derive_info_hash_from_url(state, url).await,
            None => Ok(None),
        }
    })
    .await?;
    let queue_url =
        queue_url_from_locator_or_hash(url, info_hash.as_deref(), selection.filename.as_deref())?;

    Ok(MaterializedDownload {
        queue_url,
        host: "qbittorrent".to_string(),
        source: SourceKind::Torrent,
        provider: "qbittorrent".to_string(),
        size: selection.size,
        external_id: selection.external_id,
        info_hash,
        execution_strategy: super::model::ExecutionStrategy::LocalQb,
        remote_state: None,
        remote_completed_at: None,
        selected_remote_file_ids: None,
        selected_artifacts: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn serve_redirect_then_torrent(
        location: &'static str,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            for response in [
                format!("HTTP/1.1 301 Moved Permanently\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
                "HTTP/1.1 200 OK\r\nContent-Type: application/x-bittorrent\r\nContent-Length: 63\r\nConnection: close\r\n\r\nd8:announce15:https://tracker4:infod6:lengthi123e4:name4:testee".to_string(),
            ] {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0; 2048];
                let _ = stream.read(&mut request).await.unwrap();
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        (format!("http://{addr}/start"), handle)
    }

    #[tokio::test]
    async fn initial_private_locator_is_rejected_before_request() {
        let error = fetch_torrent_info_hash(
            &reqwest::Client::new(),
            "http://127.0.0.1/private.torrent",
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(
            error.1.message,
            "torrent redirect rejected: target_private_ip"
        );
    }

    #[tokio::test]
    async fn chunked_torrent_body_is_stream_bounded() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let (mut stream, _) = listener.accept().await.unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/x-bittorrent\r\nTransfer-Encoding: chunked\r\n\r\n")
                .await
                .unwrap();
            let chunk = vec![b'x'; 16 * 1024];
            for _ in 0..=4 {
                stream
                    .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
                    .await
                    .unwrap();
                stream.write_all(&chunk).await.unwrap();
                stream.write_all(b"\r\n").await.unwrap();
            }
            let _ = stream.write_all(b"0\r\n\r\n").await;
        });
        let response = reqwest::Client::new()
            .get(format!("http://{addr}/"))
            .send()
            .await
            .unwrap();
        let error = read_bounded_torrent_response(response, 64 * 1024)
            .await
            .unwrap_err();
        assert_eq!(error.1.message, "torrent body is too large");
    }

    #[tokio::test]
    async fn follows_same_origin_relative_redirect() {
        let (url, server) = serve_redirect_then_torrent("/final").await;
        let hash = fetch_torrent_info_hash_with_policy(&url, Some("secret"), true)
            .await
            .unwrap();
        server.await.unwrap();
        assert!(hash.is_some());
    }

    #[tokio::test]
    async fn accepts_magnet_redirect_as_canonical_hash() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let (mut stream, _) = listener.accept().await.unwrap();
            stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: magnet:?xt=urn:btih:ABCDEF0123456789ABCDEF0123456789ABCDEF01\r\nContent-Length: 0\r\n\r\n").await.unwrap();
        });
        let hash = fetch_torrent_info_hash_with_policy(&format!("http://{addr}/"), None, true)
            .await
            .unwrap();
        assert_eq!(
            hash.as_deref(),
            Some("abcdef0123456789abcdef0123456789abcdef01")
        );
    }

    #[tokio::test]
    async fn rejects_non_http_redirect() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let (mut stream, _) = listener.accept().await.unwrap();
            stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: file:///etc/passwd\r\nContent-Length: 0\r\n\r\n").await.unwrap();
        });
        let error = fetch_torrent_info_hash_with_policy(&format!("http://{addr}/"), None, true)
            .await
            .unwrap_err();
        assert_eq!(
            error.1.message,
            "torrent redirect rejected: target_scheme_other"
        );
    }

    #[test]
    fn known_hash_uses_canonical_magnet_not_signed_locator() {
        let queue_url = queue_url_from_locator_or_hash(
            Some("https://prowlarr.invalid/download?apikey=secret".into()),
            Some("ABCDEF0123456789ABCDEF0123456789ABCDEF01"),
            Some("Movie"),
        )
        .unwrap();
        assert_eq!(
            queue_url,
            "magnet:?xt=urn:btih:abcdef0123456789abcdef0123456789abcdef01&dn=Movie"
        );
    }

    #[test]
    fn hash_fallback_builds_magnet_for_expired_reference() {
        let queue_url = queue_url_from_locator_or_hash(
            None,
            Some("ABCDEF0123456789ABCDEF0123456789ABCDEF01"),
            Some("A Movie 2026.mkv"),
        )
        .unwrap();

        assert_eq!(
            queue_url,
            "magnet:?xt=urn:btih:abcdef0123456789abcdef0123456789abcdef01&dn=A%20Movie%202026.mkv"
        );
    }

    #[test]
    fn missing_reference_without_hash_is_rejected() {
        let error = queue_url_from_locator_or_hash(None, None, None).unwrap_err();

        assert_eq!(error.0, axum::http::StatusCode::BAD_REQUEST);
        assert_eq!(error.1.error, "download_selection_invalid");
    }

    #[test]
    fn direct_locator_wins_over_hash_fallback() {
        let queue_url = queue_url_from_locator_or_hash(
            Some("magnet:?xt=urn:btih:source".to_string()),
            Some("fallback"),
            None,
        )
        .unwrap();

        assert_eq!(queue_url, "magnet:?xt=urn:btih:source");
    }

    #[test]
    fn trims_and_normalizes_supplied_hash() {
        assert_eq!(
            supplied_info_hash(Some(" ABCDEF0123456789ABCDEF0123456789ABCDEF01 ")),
            Some("abcdef0123456789abcdef0123456789abcdef01".to_string())
        );
        assert_eq!(supplied_info_hash(Some("  ")), None);
        assert_eq!(supplied_info_hash(Some("not-a-hash")), None);
    }

    #[tokio::test]
    async fn valid_supplied_hash_skips_locator_fetch() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let requests = AtomicUsize::new(0);
        let hash = materialize_info_hash(
            supplied_info_hash(Some("ABCDEF0123456789ABCDEF0123456789ABCDEF01")),
            Some("https://prowlarr.invalid/download?id=1"),
            async {
                requests.fetch_add(1, Ordering::SeqCst);
                Err(SourceFailure::bad_request(
                    "torrent redirect rejected: target_scheme_other",
                ))
            },
        )
        .await
        .unwrap();
        let queue_url = queue_url_from_locator_or_hash(
            Some("https://prowlarr.invalid/download?id=1".into()),
            hash.as_deref(),
            Some("Movie"),
        )
        .unwrap();

        assert_eq!(requests.load(Ordering::SeqCst), 0);
        assert_eq!(
            hash.as_deref(),
            Some("abcdef0123456789abcdef0123456789abcdef01")
        );
        assert_eq!(
            queue_url,
            "magnet:?xt=urn:btih:abcdef0123456789abcdef0123456789abcdef01&dn=Movie"
        );
    }

    #[test]
    fn is_disallowed_ip_blocks_private_and_special_ranges() {
        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

        // Public IPs must be allowed.
        assert!(!is_disallowed_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_disallowed_ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
        assert!(!is_disallowed_ip(IpAddr::V6(Ipv6Addr::new(
            0x2001, 0x4860, 0, 0, 0, 0, 0, 0x8888
        ))));

        // Loopback.
        assert!(is_disallowed_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_disallowed_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)));
        // Private v4.
        assert!(is_disallowed_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))));
        assert!(is_disallowed_ip(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_disallowed_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        // Link-local v4 (169.254/16) and CGNAT (100.64/10).
        assert!(is_disallowed_ip(IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1))));
        assert!(is_disallowed_ip(IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1))));
        // Unspecified.
        assert!(is_disallowed_ip(IpAddr::V4(Ipv4Addr::UNSPECIFIED)));
        assert!(is_disallowed_ip(IpAddr::V6(Ipv6Addr::UNSPECIFIED)));
        // Broadcast + multicast.
        assert!(is_disallowed_ip(IpAddr::V4(Ipv4Addr::new(
            255, 255, 255, 255
        ))));
        assert!(is_disallowed_ip(IpAddr::V4(Ipv4Addr::new(224, 0, 0, 1))));
        assert!(is_disallowed_ip(IpAddr::V6(Ipv6Addr::new(
            0xff02, 0, 0, 0, 0, 0, 0, 1
        ))));
        // ULA (fc00::/7) + link-local v6 (fe80::/10).
        assert!(is_disallowed_ip(IpAddr::V6(Ipv6Addr::new(
            0xfd00, 0, 0, 0, 0, 0, 0, 1
        ))));
        assert!(is_disallowed_ip(IpAddr::V6(Ipv6Addr::new(
            0xfe80, 0, 0, 0, 0, 0, 0, 1
        ))));
        // IPv4-mapped IPv6 of a private address must be blocked.
        assert!(is_disallowed_ip(IpAddr::V6(
            Ipv4Addr::new(10, 0, 0, 1).to_ipv6_mapped()
        )));
    }

    #[test]
    fn configured_private_prowlarr_is_allowed_but_arbitrary_private_locator_is_not() {
        assert!(allow_private_initial_target(
            "http://192.168.1.112:9696",
            "http://192.168.1.112:9696/1/download?apikey=secret"
        ));
        assert!(!allow_private_initial_target(
            "http://192.168.1.112:9696",
            "http://127.0.0.1/private.torrent"
        ));
    }

    #[test]
    fn info_hash_redirect_plan_keeps_private_same_origin_pin_and_credentials() {
        let existing = Some((
            "192.168.1.112".to_string(),
            vec!["192.168.1.112:9696".parse().unwrap()],
        ));
        let plan = plan_redirect(
            "http://192.168.1.112:9696/download?apikey=secret",
            "http://192.168.1.112:9696/final",
            true,
        )
        .unwrap();

        assert!(plan.forward_credentials);
        assert_eq!(
            redirect_pin(&plan, existing.clone(), Vec::<std::net::SocketAddr>::new()).unwrap(),
            existing
        );
    }

    #[test]
    fn info_hash_redirect_plan_allows_public_cross_origin_and_strips_credentials() {
        let plan = plan_redirect(
            "http://192.168.1.112:9696/download?apikey=secret",
            "https://203.0.113.10/file.torrent?apikey=secret&id=1",
            true,
        )
        .unwrap();
        let pin = redirect_pin(
            &plan,
            Some((
                "192.168.1.112".to_string(),
                vec!["192.168.1.112:9696".parse().unwrap()],
            )),
            vec!["203.0.113.10:443".parse().unwrap()],
        )
        .unwrap();

        assert!(!plan.forward_credentials);
        assert!(!plan.url.as_str().contains("apikey="));
        assert_eq!(pin.unwrap().0, "203.0.113.10");
    }

    #[test]
    fn info_hash_redirect_plan_rejects_private_cross_origin_pin() {
        let plan = plan_redirect(
            "http://192.168.1.112:9696/download",
            "https://127.0.0.1/file.torrent",
            true,
        )
        .unwrap();
        let error = redirect_pin(&plan, None, vec!["127.0.0.1:443".parse().unwrap()]).unwrap_err();

        assert_eq!(
            error.1.message,
            "torrent redirect rejected: target_private_ip"
        );
    }

    #[test]
    fn same_origin_redirect_keeps_existing_dns_pin() {
        let existing = Some((
            "prowlarr.host".to_string(),
            vec!["203.0.113.10:443".parse().unwrap()],
        ));
        let plan = plan_redirect(
            "https://prowlarr.host/download?id=1",
            "https://prowlarr.host/final?id=1",
            true,
        )
        .unwrap();

        assert_eq!(
            redirect_pin(&plan, existing.clone(), Vec::<std::net::SocketAddr>::new()).unwrap(),
            existing
        );
    }

    #[test]
    fn plan_redirect_strips_credentials_on_cross_origin_hop() {
        // Same-origin hop keeps credentials.
        let plan = plan_redirect(
            "https://prowlarr.host/download?id=1",
            "https://prowlarr.host/final?id=1",
            true,
        )
        .unwrap();
        assert!(plan.forward_credentials);
        assert_eq!(plan.host, "prowlarr.host");

        // Cross-origin hop must NOT forward credentials.
        let plan = plan_redirect(
            "https://prowlarr.host/download?id=1",
            "https://indexer.other/get.torrent",
            true,
        )
        .unwrap();
        assert!(!plan.forward_credentials);
        assert_eq!(plan.host, "indexer.other");
    }

    #[test]
    fn plan_redirect_rejects_userinfo_in_target() {
        let error = plan_redirect(
            "https://prowlarr.host/download",
            "https://user:pass@indexer.other/get.torrent",
            true,
        )
        .unwrap_err();
        assert_eq!(
            error.1.message,
            "torrent redirect rejected: target_userinfo"
        );
    }

    #[test]
    fn plan_redirect_reports_bounded_target_shape_reasons() {
        let malformed =
            plan_redirect("https://prowlarr.host/download", "://SECRET", true).unwrap_err();
        assert_eq!(
            malformed.1.message,
            "torrent redirect rejected: target_scheme_other"
        );
        assert!(!malformed.1.message.contains("SECRET"));

        let missing_host =
            plan_redirect("https://prowlarr.host/download", "file:///tmp/x", true).unwrap_err();
        assert_eq!(
            missing_host.1.message,
            "torrent redirect rejected: target_scheme_other"
        );

        let magnet = plan_redirect(
            "https://prowlarr.host/download",
            "magnet:?xt=urn:btih:SECRET",
            true,
        )
        .unwrap_err();
        assert_eq!(magnet.1.message, "torrent redirect rejected: target_magnet");
        assert!(!magnet.1.message.contains("SECRET"));
    }

    #[test]
    fn plan_redirect_rejects_non_https_cross_origin() {
        let error = plan_redirect(
            "https://prowlarr.host/download",
            "http://indexer.other/get.torrent",
            true,
        )
        .unwrap_err();
        assert!(error.1.message.contains("https"));
    }

    #[test]
    fn plan_redirect_rejects_cross_origin_when_not_allowed() {
        let error = plan_redirect(
            "https://prowlarr.host/download",
            "https://indexer.other/get.torrent",
            false,
        )
        .unwrap_err();
        assert_eq!(
            error.1.message,
            "torrent redirect rejected: strict_same_origin"
        );
    }

    #[test]
    fn plan_redirect_allows_same_origin_http_when_not_cross() {
        // Same-origin http redirect stays permitted regardless of allow_cross_origin.
        let plan = plan_redirect(
            "http://prowlarr.host/download",
            "http://prowlarr.host/final",
            false,
        )
        .unwrap();
        assert!(plan.forward_credentials);
    }

    #[tokio::test]
    async fn rejects_cross_origin_redirect_to_private_ip() {
        // Server issues cross-origin redirect to a loopback host; SSRF guard must block.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let (mut stream, _) = listener.accept().await.unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 302 Found\r\nLocation: https://localhost.localdomain.invalid/x\r\nContent-Length: 0\r\n\r\n",
                )
                .await
                .unwrap();
        });
        let error = fetch_torrent_bytes_with_policy(&format!("http://{addr}/"), None, true, true)
            .await
            .unwrap_err();
        // Either DNS fails (unresolvable) or resolves to a disallowed IP; both are rejections.
        assert!(!error.1.message.is_empty());
    }

    #[tokio::test]
    async fn acquisition_still_rejects_cross_origin_redirect() {
        // allow_cross_origin=false path must keep strict same-origin behavior.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let (mut stream, _) = listener.accept().await.unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 302 Found\r\nLocation: https://indexer.other/get.torrent\r\nContent-Length: 0\r\n\r\n",
                )
                .await
                .unwrap();
        });
        let error = fetch_torrent_bytes_with_policy(&format!("http://{addr}/"), None, false, true)
            .await
            .unwrap_err();
        assert_eq!(
            error.1.message,
            "torrent redirect rejected: strict_same_origin"
        );
    }

    #[tokio::test]
    async fn invalid_supplied_hash_does_not_skip_derivation() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let requests = AtomicUsize::new(0);
        let hash = materialize_info_hash(
            supplied_info_hash(Some("not-a-hash")),
            Some("https://prowlarr.invalid/download?id=1"),
            async {
                requests.fetch_add(1, Ordering::SeqCst);
                Ok(Some("0123456789abcdef0123456789abcdef01234567".into()))
            },
        )
        .await
        .unwrap();

        assert_eq!(requests.load(Ordering::SeqCst), 1);
        assert_eq!(
            hash.as_deref(),
            Some("0123456789abcdef0123456789abcdef01234567")
        );
    }
}
