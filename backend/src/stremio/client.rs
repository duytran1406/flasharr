use super::parser::{parse_streams, Manifest, StreamCandidate};
use futures_util::StreamExt;
use reqwest::redirect::Policy;
use std::{
    collections::{HashMap, VecDeque},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};
use url::Url;

const BODY_CAP: usize = 1024 * 1024;
const CLIENT_CACHE_CAP: usize = 32;
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum StremioFailureClass {
    Dns,
    AddressRejected,
    Connect,
    Timeout,
    Tls,
    #[serde(rename = "status_4xx")]
    Status4xx,
    #[serde(rename = "status_429")]
    Status429,
    #[serde(rename = "status_5xx")]
    Status5xx,
    Redirect,
    ResponseTooLarge,
    ResponseRead,
    ResponseParse,
    InvalidUrl,
    InvalidStream,
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct StremioError(StremioFailureClass);
impl StremioError {
    pub(crate) fn new(class: StremioFailureClass) -> Self {
        Self(class)
    }
    pub fn class(&self) -> StremioFailureClass {
        self.0
    }
}
impl std::fmt::Debug for StremioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("StremioError").field(&self.0).finish()
    }
}
impl std::fmt::Display for StremioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "stremio failure: {:?}", self.0)
    }
}
impl std::error::Error for StremioError {}
type Result<T> = std::result::Result<T, StremioError>;

fn reqwest_class(error: &reqwest::Error) -> StremioFailureClass {
    if error.is_timeout() {
        StremioFailureClass::Timeout
    } else if let Some(status) = error.status() {
        if status.as_u16() == 429 {
            StremioFailureClass::Status429
        } else if status.is_client_error() {
            StremioFailureClass::Status4xx
        } else if status.is_server_error() {
            StremioFailureClass::Status5xx
        } else {
            StremioFailureClass::Other
        }
    } else if error.is_connect() {
        StremioFailureClass::Connect
    } else if error.is_body() || error.is_decode() {
        StremioFailureClass::ResponseRead
    } else if error.is_builder() {
        StremioFailureClass::InvalidUrl
    } else {
        StremioFailureClass::Other
    }
}
fn reqwest_error(error: reqwest::Error) -> StremioError {
    StremioError::new(reqwest_class(&error))
}
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct PinnedOrigin {
    scheme: String,
    host: String,
    port: u16,
    addresses: Vec<SocketAddr>,
}
#[derive(Default)]
struct ClientCache {
    clients: HashMap<PinnedOrigin, Arc<reqwest::Client>>,
    order: VecDeque<PinnedOrigin>,
}
#[derive(Clone)]
pub struct StremioClient {
    timeout: Duration,
    clients: Arc<Mutex<ClientCache>>,
    #[cfg(test)]
    allow_test_loopback: bool,
}
impl StremioClient {
    pub fn new() -> std::result::Result<Self, reqwest::Error> {
        Ok(Self {
            timeout: Duration::from_secs(10),
            clients: Default::default(),
            #[cfg(test)]
            allow_test_loopback: false,
        })
    }
    #[cfg(test)]
    fn for_test(timeout: Duration) -> Self {
        Self {
            timeout,
            clients: Default::default(),
            allow_test_loopback: true,
        }
    }
    pub fn validate_url(url: &Url) -> Result<()> {
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
            return Err(StremioError::new(StremioFailureClass::InvalidUrl));
        }
        let host = url
            .host()
            .ok_or_else(|| StremioError::new(StremioFailureClass::InvalidUrl))?;
        if matches!(host, url::Host::Domain(domain) if domain.eq_ignore_ascii_case("localhost"))
            || matches!(host, url::Host::Ipv4(ip) if !is_public(IpAddr::V4(ip)))
            || matches!(host, url::Host::Ipv6(ip) if !is_public(IpAddr::V6(ip)))
        {
            return Err(StremioError::new(StremioFailureClass::AddressRejected));
        }
        Ok(())
    }
    async fn get(&self, url: Url) -> Result<Vec<u8>> {
        #[cfg(test)]
        let test_loopback = self.allow_test_loopback
            && url.scheme() == "http"
            && url.host_str() == Some("127.0.0.1");
        #[cfg(not(test))]
        let test_loopback = false;
        if !test_loopback {
            Self::validate_url(&url)?;
        }
        let host = url
            .host_str()
            .ok_or_else(|| StremioError::new(StremioFailureClass::InvalidUrl))?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| StremioError::new(StremioFailureClass::InvalidUrl))?;
        let resolved: Vec<_> = tokio::net::lookup_host((host, port))
            .await
            .map_err(|_| StremioError::new(StremioFailureClass::Dns))?
            .collect();
        let resolved = if test_loopback {
            resolved
        } else {
            validate_resolved(&resolved)?.to_vec()
        };
        let http = self
            .pinned_client(url.scheme(), host, port, &resolved)
            .map_err(reqwest_error)?;
        let r = http.get(url).send().await.map_err(reqwest_error)?;
        if r.status().is_redirection() {
            return Err(StremioError::new(StremioFailureClass::Redirect));
        }
        let r = r.error_for_status().map_err(reqwest_error)?;
        if r.content_length().is_some_and(|n| n as usize > BODY_CAP) {
            return Err(StremioError::new(StremioFailureClass::ResponseTooLarge));
        }
        let mut body = Vec::new();
        let mut stream = r.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(reqwest_error)?;
            if body.len() + chunk.len() > BODY_CAP {
                return Err(StremioError::new(StremioFailureClass::ResponseTooLarge));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }
    fn pinned_client(
        &self,
        scheme: &str,
        host: &str,
        port: u16,
        addresses: &[SocketAddr],
    ) -> std::result::Result<Arc<reqwest::Client>, reqwest::Error> {
        let mut addresses = addresses.to_vec();
        addresses.sort_unstable();
        addresses.dedup();
        let key = PinnedOrigin {
            scheme: scheme.to_owned(),
            host: host.to_ascii_lowercase(),
            port,
            addresses,
        };
        let mut cache = self.clients.lock().unwrap();
        if let Some(client) = cache.clients.get(&key) {
            return Ok(client.clone());
        }
        let client = Arc::new(with_pinned_client(host, &key.addresses, |host, addrs| {
            reqwest::Client::builder()
                .redirect(Policy::none())
                .timeout(self.timeout)
                .resolve_to_addrs(host, addrs)
                .build()
        })?);
        if cache.order.len() == CLIENT_CACHE_CAP {
            if let Some(oldest) = cache.order.pop_front() {
                cache.clients.remove(&oldest);
            }
        }
        cache.order.push_back(key.clone());
        cache.clients.insert(key, client.clone());
        Ok(client)
    }
    #[cfg(test)]
    fn cached_client_count(&self) -> usize {
        self.clients.lock().unwrap().clients.len()
    }
    pub async fn manifest(&self, url: Url) -> Result<Manifest> {
        serde_json::from_slice(&self.get(url).await?)
            .map_err(|_| StremioError::new(StremioFailureClass::ResponseParse))
    }
    pub async fn movie(&self, manifest: &Url, imdb: &str) -> Result<Vec<StreamCandidate>> {
        self.stream(manifest, &format!("movie/{imdb}")).await
    }
    pub async fn series(
        &self,
        manifest: &Url,
        imdb: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamCandidate>> {
        self.stream(manifest, &format!("series/{imdb}:{season}:{episode}"))
            .await
    }
    async fn stream(&self, manifest: &Url, id: &str) -> Result<Vec<StreamCandidate>> {
        let mut u = manifest.clone();
        let base = u
            .path()
            .trim_end_matches("manifest.json")
            .trim_end_matches('/');
        u.set_path(&format!("{base}/stream/{id}.json"));
        parse_streams(&self.get(u).await?)
            .map_err(|_| StremioError::new(StremioFailureClass::ResponseParse))
    }
}
fn validate_resolved(addresses: &[SocketAddr]) -> Result<&[SocketAddr]> {
    if addresses.is_empty() || addresses.iter().any(|address| !is_public(address.ip())) {
        return Err(StremioError::new(StremioFailureClass::AddressRejected));
    }
    Ok(addresses)
}

fn with_pinned_client<T, E>(
    host: &str,
    addresses: &[SocketAddr],
    build: impl FnOnce(&str, &[SocketAddr]) -> std::result::Result<T, E>,
) -> std::result::Result<T, E> {
    build(host, addresses)
}

fn ipv4_in(ip: Ipv4Addr, base: [u8; 4], prefix: u32) -> bool {
    let mask = u32::MAX.checked_shl(32 - prefix).unwrap_or(0);
    u32::from(ip) & mask == u32::from(Ipv4Addr::from(base)) & mask
}

fn ipv6_in(ip: Ipv6Addr, base: [u16; 8], prefix: u32) -> bool {
    let mask = u128::MAX.checked_shl(128 - prefix).unwrap_or(0);
    u128::from(ip) & mask == u128::from(Ipv6Addr::from(base)) & mask
}

fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => ![
            ([0, 0, 0, 0], 8),
            ([10, 0, 0, 0], 8),
            ([100, 64, 0, 0], 10),
            ([127, 0, 0, 0], 8),
            ([169, 254, 0, 0], 16),
            ([172, 16, 0, 0], 12),
            ([192, 0, 0, 0], 24),
            ([192, 0, 2, 0], 24),
            ([192, 88, 99, 0], 24),
            ([192, 168, 0, 0], 16),
            ([198, 18, 0, 0], 15),
            ([198, 51, 100, 0], 24),
            ([203, 0, 113, 0], 24),
            ([224, 0, 0, 0], 4),
            ([240, 0, 0, 0], 4),
        ]
        .iter()
        .any(|(base, prefix)| ipv4_in(v, *base, *prefix)),
        IpAddr::V6(v) => {
            if let Some(mapped) = v.to_ipv4_mapped() {
                return is_public(IpAddr::V4(mapped));
            }
            ipv6_in(v, [0x2000, 0, 0, 0, 0, 0, 0, 0], 3)
                && ![
                    ([0x2001, 0, 0, 0, 0, 0, 0, 0], 32),      // Teredo
                    ([0x2001, 2, 0, 0, 0, 0, 0, 0], 48),      // benchmarking
                    ([0x2001, 0x10, 0, 0, 0, 0, 0, 0], 28),   // ORCHID
                    ([0x2001, 0x20, 0, 0, 0, 0, 0, 0], 28),   // ORCHIDv2
                    ([0x2001, 0x0db8, 0, 0, 0, 0, 0, 0], 32), // documentation
                ]
                .iter()
                .any(|(base, prefix)| ipv6_in(v, *base, *prefix))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    async fn fixture(response: &'static [u8], delay: Duration) -> (Url, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let paths = Arc::new(Mutex::new(Vec::new()));
        let seen = paths.clone();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut request = vec![0; 4096];
                let n = socket.read(&mut request).await.unwrap();
                let first = String::from_utf8_lossy(&request[..n]);
                seen.lock()
                    .unwrap()
                    .push(first.split_whitespace().nth(1).unwrap().to_string());
                tokio::time::sleep(delay).await;
                socket.write_all(response).await.unwrap();
            }
        });
        (
            Url::parse(&format!("http://{addr}/addon/manifest.json")).unwrap(),
            paths,
        )
    }

    const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 73\r\nConnection: close\r\n\r\n{\"id\":\"fixture\",\"name\":\"Fixture\",\"version\":\"1\",\"resources\":[],\"types\":[]}";
    const STREAMS: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 177\r\nConnection: close\r\n\r\n{\"streams\":[{\"infoHash\":\"0123456789abcdef0123456789abcdef01234567\",\"fileIdx\":2,\"sources\":[\"tracker:x\"],\"behaviorHints\":{\"filename\":\"x.mkv\"}},{\"url\":\"https://secret.example/x\"}]}";

    #[tokio::test]
    async fn failures_have_bounded_sanitized_classes() {
        let client = StremioClient::for_test(Duration::from_millis(25));
        for (response, delay, expected) in [
            (b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\n\r\n" as &[u8], Duration::ZERO, StremioFailureClass::Status429),
            (b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n", Duration::ZERO, StremioFailureClass::Status5xx),
            (b"HTTP/1.1 302 Found\r\nLocation: http://secret.invalid/token\r\nContent-Length: 0\r\n\r\n", Duration::ZERO, StremioFailureClass::Redirect),
            (OK, Duration::from_millis(100), StremioFailureClass::Timeout),
        ] {
            let (url, _) = fixture(response, delay).await;
            assert_eq!(client.manifest(url).await.unwrap_err().class(), expected);
        }
        let malformed = b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n{"
            .to_vec()
            .leak();
        let (url, _) = fixture(malformed, Duration::ZERO).await;
        assert_eq!(
            client.manifest(url).await.unwrap_err().class(),
            StremioFailureClass::ResponseParse
        );
        let oversized = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
            BODY_CAP + 1
        )
        .into_bytes()
        .leak();
        let (url, _) = fixture(oversized, Duration::ZERO).await;
        assert_eq!(
            client.manifest(url).await.unwrap_err().class(),
            StremioFailureClass::ResponseTooLarge
        );
        assert_eq!(
            StremioClient::validate_url(&Url::parse("https://127.0.0.1/x").unwrap())
                .unwrap_err()
                .class(),
            StremioFailureClass::AddressRejected
        );
    }

    #[test]
    fn client_cache_reuses_only_the_same_pinned_origin() {
        let client = StremioClient::for_test(Duration::from_secs(1));
        let a = ["93.184.216.34:443".parse().unwrap()];
        let changed = ["93.184.216.35:443".parse().unwrap()];
        let first = client
            .pinned_client("https", "example.com", 443, &a)
            .unwrap();
        let same = client
            .pinned_client("https", "example.com", 443, &a)
            .unwrap();
        let other_origin = client
            .pinned_client("https", "other.example", 443, &a)
            .unwrap();
        let other_address = client
            .pinned_client("https", "example.com", 443, &changed)
            .unwrap();
        assert!(Arc::ptr_eq(&first, &same));
        assert!(!Arc::ptr_eq(&first, &other_origin));
        assert!(!Arc::ptr_eq(&first, &other_address));
        assert_eq!(client.cached_client_count(), 3);
    }

    #[tokio::test]
    async fn local_http_manifest_movie_series_matrix() {
        let client = StremioClient::for_test(Duration::from_secs(1));
        let (manifest_url, manifest_paths) = fixture(OK, Duration::ZERO).await;
        assert_eq!(client.manifest(manifest_url).await.unwrap().id, "fixture");
        assert_eq!(&*manifest_paths.lock().unwrap(), &["/addon/manifest.json"]);

        let (movie_url, movie_paths) = fixture(STREAMS, Duration::ZERO).await;
        let movie = client.movie(&movie_url, "tt123").await.unwrap();
        assert_eq!(movie.len(), 1);
        assert_eq!(movie[0].file_idx, Some(2));
        assert_eq!(
            &*movie_paths.lock().unwrap(),
            &["/addon/stream/movie/tt123.json"]
        );

        let (series_url, series_paths) = fixture(STREAMS, Duration::ZERO).await;
        let series = client.series(&series_url, "tt456", 2, 3).await.unwrap();
        assert!(series[0].sources.is_empty());
        assert_eq!(
            series[0].behavior_hints.filename.as_deref().unwrap(),
            "x.mkv"
        );
        assert_eq!(
            &*series_paths.lock().unwrap(),
            &["/addon/stream/series/tt456:2:3.json"]
        );
    }

    #[tokio::test]
    async fn local_http_rejects_redirect_oversize_chunked_and_timeout() {
        let client = StremioClient::for_test(Duration::from_millis(50));
        let (url, _) = fixture(
            b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1/other\r\nContent-Length: 0\r\n\r\n",
            Duration::ZERO,
        )
        .await;
        assert_eq!(
            client.manifest(url).await.unwrap_err().class(),
            StremioFailureClass::Redirect
        );

        let declared = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
            BODY_CAP + 1
        )
        .into_bytes()
        .leak();
        let (url, _) = fixture(declared, Duration::ZERO).await;
        assert_eq!(
            client.manifest(url).await.unwrap_err().class(),
            StremioFailureClass::ResponseTooLarge
        );

        let body = vec![b'x'; BODY_CAP + 1];
        let chunked = format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
            body.len(),
            String::from_utf8(body).unwrap()
        )
        .into_bytes()
        .leak();
        let (url, _) = fixture(chunked, Duration::ZERO).await;
        assert_eq!(
            client.manifest(url).await.unwrap_err().class(),
            StremioFailureClass::ResponseTooLarge
        );

        let (url, _) = fixture(OK, Duration::from_millis(200)).await;
        assert_eq!(
            client.manifest(url).await.unwrap_err().class(),
            StremioFailureClass::Timeout
        );
    }

    #[tokio::test]
    async fn resolved_private_destination_is_rejected_by_strict_client() {
        let error = StremioClient::new()
            .unwrap()
            .manifest(Url::parse("https://localhost/manifest.json").unwrap())
            .await
            .unwrap_err();
        assert_eq!(error.class(), StremioFailureClass::AddressRejected);
    }
    #[test]
    fn rejects_http_userinfo_and_non_global_addresses() {
        for url in [
            "http://example.com/manifest.json",
            "https://user:pass@example.com/manifest.json",
            "https://127.0.0.1/manifest.json",
            "https://100.64.0.1/manifest.json",
            "https://224.0.0.1/manifest.json",
            "https://240.0.0.1/manifest.json",
            "https://192.0.2.1/manifest.json",
            "https://[::ffff:10.0.0.1]/manifest.json",
            "https://[ff00::1]/manifest.json",
            "https://[2001:db8::1]/manifest.json",
        ] {
            assert!(
                StremioClient::validate_url(&Url::parse(url).unwrap()).is_err(),
                "{url}"
            );
        }
        assert!(StremioClient::validate_url(
            &Url::parse("https://example.com/manifest.json").unwrap()
        )
        .is_ok());
    }

    #[test]
    fn mixed_dns_answers_are_rejected() {
        let answers = [
            "93.184.216.34:443".parse().unwrap(),
            "10.0.0.1:443".parse().unwrap(),
        ];
        assert!(validate_resolved(&answers).is_err());
    }

    #[test]
    fn pinned_builder_seam_receives_exact_validated_addresses() {
        let answers = [
            "93.184.216.34:443".parse().unwrap(),
            "[2606:2800:220:1:248:1893:25c8:1946]:443".parse().unwrap(),
        ];
        let validated = validate_resolved(&answers).unwrap();
        let received = with_pinned_client("example.com", &validated, |host, addrs| {
            Ok::<_, reqwest::Error>((host.to_owned(), addrs.to_vec()))
        })
        .unwrap();
        assert_eq!(received, ("example.com".to_owned(), answers.to_vec()));
    }
}
