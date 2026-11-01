#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocatorKind {
    File,
    Folder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locator {
    pub kind: LocatorKind,
    pub item_id: String,
}

pub fn parse_locator(value: &str) -> Option<Locator> {
    let parsed = url::Url::parse(value).ok()?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(parsed.host_str(), Some("fshare.vn" | "www.fshare.vn"))
    {
        return None;
    }
    let mut segments = parsed.path_segments()?;
    let kind = match segments.next()? {
        "file" => LocatorKind::File,
        "folder" => LocatorKind::Folder,
        _ => return None,
    };
    let item_id = segments.next().filter(|item_id| !item_id.is_empty())?;
    if segments.next().is_some() {
        return None;
    }
    Some(Locator {
        kind,
        item_id: item_id.to_string(),
    })
}

pub fn is_fshare_url(value: &str) -> bool {
    parse_locator(value).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FshareSearchBackend {
    FolderCache,
}

pub const fn active_search_backend() -> FshareSearchBackend {
    FshareSearchBackend::FolderCache
}

pub const fn owns_folder_sources() -> bool {
    true
}

pub const fn canonical_route_prefix() -> &'static str {
    "/api/providers/fshare"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcquisitionSource {
    FshareUrl(String),
    TorrentInfoHash(String),
}

impl AcquisitionSource {
    fn torrent_info_hash(&self) -> Option<&str> {
        match self {
            Self::TorrentInfoHash(hash) => Some(hash),
            Self::FshareUrl(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeNzbMetadata {
    pub source: AcquisitionSource,
    pub tmdb_id: Option<String>,
    pub tvdb_id: Option<String>,
    pub title: Option<String>,
    pub media_type: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub category: Option<String>,
}

impl FakeNzbMetadata {
    pub fn fshare(source_url: String) -> Self {
        Self {
            source: AcquisitionSource::FshareUrl(source_url),
            tmdb_id: None,
            tvdb_id: None,
            title: None,
            media_type: None,
            season: None,
            episode: None,
            category: None,
        }
    }

    pub fn torrent(info_hash: String) -> Self {
        Self {
            source: AcquisitionSource::TorrentInfoHash(info_hash),
            tmdb_id: None,
            tvdb_id: None,
            title: None,
            media_type: None,
            season: None,
            episode: None,
            category: None,
        }
    }

    pub fn fshare_url(&self) -> Option<&str> {
        match &self.source {
            AcquisitionSource::FshareUrl(url) => Some(url),
            _ => None,
        }
    }

    pub fn torrent_info_hash(&self) -> Option<&str> {
        self.source.torrent_info_hash()
    }
}

pub const fn canonical_routes() -> &'static [&'static str] {
    &[
        "/api/providers/fshare/status",
        "/api/providers/fshare/setup",
        "/api/providers/fshare/accounts",
    ]
}

pub const fn legacy_routes() -> &'static [&'static str] {
    &["/api/health/status", "/api/setup/fshare", "/api/accounts"]
}

fn normalize_info_hash(value: &str) -> Option<String> {
    let value = value.trim();
    (value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| value.to_ascii_lowercase())
}

pub fn encode_fake_nzb(metadata: &FakeNzbMetadata) -> Option<String> {
    let (source_tag, subject) = match &metadata.source {
        AcquisitionSource::FshareUrl(url) if is_fshare_url(url) => (
            format!("        <meta type=\"fshare_url\">{url}</meta>\n"),
            "Fshare_Download",
        ),
        AcquisitionSource::TorrentInfoHash(hash) => {
            let hash = normalize_info_hash(hash)?;
            (
                format!("        <meta type=\"torrent_info_hash\">{hash}</meta>\n"),
                "Torrent_Download",
            )
        }
        _ => return None,
    };

    let mut tags = source_tag;
    for (name, value) in [
        ("tmdb_id", metadata.tmdb_id.clone()),
        ("tvdb_id", metadata.tvdb_id.clone()),
        ("title", metadata.title.clone()),
        ("media_type", metadata.media_type.clone()),
        ("season", metadata.season.map(|value| value.to_string())),
        ("episode", metadata.episode.map(|value| value.to_string())),
        ("category", metadata.category.clone()),
    ] {
        if let Some(value) = value {
            tags.push_str(&format!(
                "        <meta type=\"{name}\">{}</meta>\n",
                quick_xml::escape::escape(&value)
            ));
        }
    }

    Some(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<nzb xmlns="http://www.newznab.com/DTD/2010/nzb">
    <head>
{tags}    </head>
    <file poster="Flasharr" date="1706534400" subject="{subject}">
        <groups><group>alt.binaries.test</group></groups>
        <segments><segment bytes="1" number="1">fake</segment></segments>
    </file>
</nzb>"#
    ))
}

pub fn decode_fake_nzb(content: &str) -> Option<FakeNzbMetadata> {
    let fshare_urls = extract_metadata_all(content, "fshare_url");
    let torrent_hashes = extract_metadata_all(content, "torrent_info_hash");
    let source = match (fshare_urls.as_slice(), torrent_hashes.as_slice()) {
        ([url], []) if is_fshare_url(url) => AcquisitionSource::FshareUrl(url.clone()),
        ([], [hash]) => AcquisitionSource::TorrentInfoHash(normalize_info_hash(hash)?),
        _ => return None,
    };

    Some(FakeNzbMetadata {
        source,
        tmdb_id: extract_metadata(content, "tmdb_id"),
        tvdb_id: extract_metadata(content, "tvdb_id"),
        title: extract_metadata(content, "title"),
        media_type: extract_metadata(content, "media_type"),
        season: extract_metadata(content, "season").and_then(|value| value.parse().ok()),
        episode: extract_metadata(content, "episode").and_then(|value| value.parse().ok()),
        category: extract_metadata(content, "category"),
    })
}

fn extract_metadata_all(content: &str, metadata_type: &str) -> Vec<String> {
    let tag = format!(r#"<meta type="{metadata_type}">"#);
    let mut remainder = content;
    let mut values = Vec::new();
    while let Some(start) = remainder.find(&tag) {
        let value = &remainder[start + tag.len()..];
        let Some(end) = value.find("</meta>") else {
            return Vec::new();
        };
        let Ok(value) = quick_xml::escape::unescape(&value[..end]) else {
            return Vec::new();
        };
        values.push(value.into_owned());
        remainder = &remainder[start + tag.len() + end + "</meta>".len()..];
    }
    values
}

fn extract_metadata(content: &str, metadata_type: &str) -> Option<String> {
    extract_metadata_all(content, metadata_type)
        .into_iter()
        .next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_fshare_nzb_full_wire_fixture_is_byte_stable() {
        let metadata = FakeNzbMetadata {
            source: AcquisitionSource::FshareUrl(
                "https://www.fshare.vn/file/ABCDEF123456".to_string(),
            ),
            tmdb_id: Some("84".to_string()),
            tvdb_id: Some("42".to_string()),
            title: Some("Test Series".to_string()),
            media_type: Some("tv".to_string()),
            season: Some(2),
            episode: Some(7),
            category: Some("tv".to_string()),
        };
        assert_eq!(
            encode_fake_nzb(&metadata),
            Some(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<nzb xmlns="http://www.newznab.com/DTD/2010/nzb">
    <head>
        <meta type="fshare_url">https://www.fshare.vn/file/ABCDEF123456</meta>
        <meta type="tmdb_id">84</meta>
        <meta type="tvdb_id">42</meta>
        <meta type="title">Test Series</meta>
        <meta type="media_type">tv</meta>
        <meta type="season">2</meta>
        <meta type="episode">7</meta>
        <meta type="category">tv</meta>
    </head>
    <file poster="Flasharr" date="1706534400" subject="Fshare_Download">
        <groups><group>alt.binaries.test</group></groups>
        <segments><segment bytes="1" number="1">fake</segment></segments>
    </file>
</nzb>"#
                    .to_string()
            )
        );
    }

    #[test]
    fn legacy_fshare_nzb_keeps_exact_subject_wire_format() {
        let mut metadata =
            FakeNzbMetadata::fshare("https://www.fshare.vn/file/ABCDEF123456".to_string());
        metadata.title = Some("Legacy".to_string());
        let encoded = encode_fake_nzb(&metadata).expect("legacy NZB");
        assert!(encoded.contains("subject=\"Fshare_Download\""));
    }

    #[test]
    fn fake_nzb_accepts_exactly_one_validated_acquisition_source() {
        let hash = "AABBCCDDEEFF00112233445566778899AABBCCDD";
        let torrent = FakeNzbMetadata {
            source: AcquisitionSource::TorrentInfoHash(hash.to_string()),
            tmdb_id: None,
            tvdb_id: Some("42".to_string()),
            title: Some("Show.S01.mkv".to_string()),
            media_type: Some("tv".to_string()),
            season: Some(1),
            episode: None,
            category: Some("tv".to_string()),
        };
        let encoded = encode_fake_nzb(&torrent).expect("torrent NZB");
        assert_eq!(
            decode_fake_nzb(&encoded)
                .and_then(|metadata| metadata.source.torrent_info_hash().map(str::to_string)),
            Some(hash.to_ascii_lowercase())
        );
        for invalid in [
            "<nzb><head></head></nzb>",
            "<meta type=\"torrent_info_hash\">bad</meta>",
            "<meta type=\"fshare_url\">http://www.fshare.vn/file/ABC</meta>",
            "<meta type=\"fshare_url\">https://www.fshare.vn/file/ABC</meta><meta type=\"torrent_info_hash\">aabbccddeeff00112233445566778899aabbccdd</meta>",
        ] {
            assert!(decode_fake_nzb(invalid).is_none());
        }
    }
}
