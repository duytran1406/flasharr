use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub resources: Vec<serde_json::Value>,
    #[serde(default)]
    pub types: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StreamCandidate {
    pub info_hash: String,
    pub file_idx: Option<u32>,
    #[serde(default, skip_serializing)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub behavior_hints: StreamBehaviorHints,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StreamBehaviorHints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binge_group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_size: Option<u64>,
}

pub fn valid_btih(hash: &str) -> bool {
    (hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        || (hash.len() == 32
            && hash
                .bytes()
                .all(|byte| matches!(byte.to_ascii_lowercase(), b'a'..=b'z' | b'2'..=b'7')))
}

#[derive(Deserialize)]
struct Streams {
    #[serde(default)]
    streams: Vec<RawStream>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawStream {
    info_hash: Option<String>,
    file_idx: Option<u32>,
    #[serde(default)]
    sources: Vec<String>,
    #[serde(default)]
    behavior_hints: serde_json::Value,
}

pub fn parse_install_url(input: &str) -> Result<Url, &'static str> {
    let mut url = Url::parse(input).map_err(|_| "invalid install URL")?;
    match url.scheme() {
        "stremio" => {
            url = Url::parse(&format!(
                "https://{}",
                input.trim_start_matches("stremio://")
            ))
            .map_err(|_| "invalid install URL")?;
        }
        "https" => {}
        _ => return Err("HTTPS required"),
    }
    if url.username() != "" || url.password().is_some() || url.host_str().is_none() {
        return Err("invalid authority");
    }
    let path = url.path().trim_end_matches('/');
    let base = path.strip_suffix("/manifest.json").unwrap_or(path);
    url.set_path(&format!("{base}/manifest.json"));
    url.set_fragment(None);
    Ok(url)
}

pub fn parse_streams(body: &[u8]) -> Result<Vec<StreamCandidate>, serde_json::Error> {
    let parsed: Streams = serde_json::from_slice(body)?;
    Ok(parsed
        .streams
        .into_iter()
        .filter_map(|s| {
            let hash = s.info_hash?.to_ascii_lowercase();
            valid_btih(&hash).then_some(StreamCandidate {
                info_hash: hash,
                file_idx: s.file_idx,
                sources: Vec::new(),
                behavior_hints: sanitize_behavior_hints(&s.behavior_hints),
            })
        })
        .collect())
}

fn safe_hint(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    (value.len() <= 256
        && !value.chars().any(char::is_control)
        && ![
            "://",
            "magnet:",
            "token=",
            "key=",
            "password=",
            "credential",
        ]
        .iter()
        .any(|needle| lower.contains(needle)))
    .then(|| value.to_string())
}

fn sanitize_behavior_hints(raw: &serde_json::Value) -> StreamBehaviorHints {
    const MAX_VIDEO_SIZE: u64 = 1 << 50;
    StreamBehaviorHints {
        filename: raw
            .get("filename")
            .and_then(|v| v.as_str())
            .and_then(safe_hint),
        binge_group: raw
            .get("bingeGroup")
            .and_then(|v| v.as_str())
            .and_then(safe_hint),
        video_size: raw
            .get("videoSize")
            .and_then(|v| v.as_u64())
            .filter(|v| *v <= MAX_VIDEO_SIZE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn install_url_normalizes_manifest() {
        assert_eq!(
            parse_install_url("stremio://example.com/a/key/manifest.json")
                .unwrap()
                .as_str(),
            "https://example.com/a/key/manifest.json"
        );
    }

    #[test]
    fn manifest_logo_is_optional() {
        let with_logo: Manifest = serde_json::from_str(
            r#"{"id":"addon","name":"Addon","logo":"https://cdn.example/logo.png"}"#,
        )
        .unwrap();
        let without_logo: Manifest =
            serde_json::from_str(r#"{"id":"addon","name":"Addon"}"#).unwrap();
        assert_eq!(
            with_logo.logo.as_deref(),
            Some("https://cdn.example/logo.png")
        );
        assert_eq!(without_logo.logo, None);
    }
    #[test]
    fn only_hash_streams_survive_sanitized() {
        let v=parse_streams(br#"{"streams":[{"infoHash":"0123456789abcdef0123456789abcdef01234567","fileIdx":2,"sources":["tracker:x"],"behaviorHints":{"filename":"x.mkv","bingeGroup":"g"}},{"url":"https://secret/x"}]}"#).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].file_idx, Some(2));
        assert!(v[0].sources.is_empty());
        assert_eq!(v[0].behavior_hints.binge_group.as_deref(), Some("g"));
    }

    #[test]
    fn typed_hints_bound_video_size_and_drop_unknown_fields() {
        for (size, expected) in [(0, Some(0)), (42, Some(42)), ((1_u64 << 50) + 1, None)] {
            let body = format!(
                r#"{{"streams":[{{"infoHash":"{}","behaviorHints":{{"filename":"x.mkv","bingeGroup":"g","videoSize":{size},"sentinel":"SECRET"}}}}]}}"#,
                "a".repeat(40)
            );
            let hints = parse_streams(body.as_bytes())
                .unwrap()
                .remove(0)
                .behavior_hints;
            assert_eq!(hints.video_size, expected);
            let wire = serde_json::to_string(&hints).unwrap();
            assert!(wire.contains("bingeGroup"));
            assert_eq!(wire.contains("videoSize"), expected.is_some());
            assert!(!wire.contains("sentinel"));
            assert!(!wire.contains("SECRET"));
        }
    }

    #[test]
    fn adversarial_locators_and_tokens_never_serialize() {
        let body = br#"{"streams":[{"infoHash":"0123456789abcdef0123456789abcdef01234567","sources":["magnet:?xt=SOURCES_SENTINEL"],"behaviorHints":{"filename":"https://signed/x?token=HINT_SENTINEL","bingeGroup":"magnet:BINGE_SENTINEL","configUrl":"https://CONFIG_SENTINEL","ciphertext":"CIPHER_SENTINEL"},"url":"https://TOP_SENTINEL"}]}"#;
        let wire = serde_json::to_string(&parse_streams(body).unwrap()).unwrap();
        for sentinel in [
            "SOURCES_SENTINEL",
            "HINT_SENTINEL",
            "BINGE_SENTINEL",
            "CONFIG_SENTINEL",
            "CIPHER_SENTINEL",
            "TOP_SENTINEL",
            "magnet:",
            "https://",
        ] {
            assert!(!wire.contains(sentinel), "leaked {sentinel}: {wire}");
        }
    }

    #[test]
    fn btih_accepts_hex_and_base32_but_excludes_url_only_streams() {
        let streams = parse_streams(
            br#"{"streams":[
            {"infoHash":"0123456789ABCDEF0123456789ABCDEF01234567"},
            {"infoHash":"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"},
            {"url":"https://credential.example/stream/secret"}
        ]}"#,
        )
        .unwrap();

        assert_eq!(streams.len(), 2);
        assert_eq!(
            streams[0].info_hash,
            "0123456789abcdef0123456789abcdef01234567"
        );
        assert_eq!(streams[1].info_hash, "abcdefghijklmnopqrstuvwxyz234567");
        assert!(!serde_json::to_string(&streams)
            .unwrap()
            .contains("credential.example"));
    }
}
