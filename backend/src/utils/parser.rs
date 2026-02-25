use serde::{Deserialize, Serialize};
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Resolution {
    SD = 1,      // 480p, 576p
    HD = 2,      // 720p
    FHD = 3,     // 1080p
    UHD = 4,     // 2160p, 4K
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    CAM = 1,
    TS = 2,
    HDTV = 3,
    DVDRip = 4,
    WEBRip = 5,
    WebDL = 6,
    BDRip = 7,
    BluRay = 8,
    Remux = 9,
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityAttributes {
    pub resolution: Option<String>,
    pub source: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub hdr: bool,
    pub dolby_vision: bool,
    pub bit_depth: u8,
    pub viet_sub: bool,
    pub viet_dub: bool,
    pub is_tv: bool,
    pub is_movie: bool,
    pub is_hd: bool,
}

impl Default for QualityAttributes {
    fn default() -> Self {
        Self {
            resolution: None,
            source: None,
            video_codec: None,
            audio_codec: None,
            hdr: false,
            dolby_vision: false,
            bit_depth: 8,
            viet_sub: false,
            viet_dub: false,
            is_tv: false,
            is_movie: false,
            is_hd: false,
        }
    }
}

impl QualityAttributes {
    pub fn quality_name(&self) -> String {
        let src = self.source.as_deref().unwrap_or("Unknown");
        match self.resolution.as_deref() {
            Some(res) => {
                if src == "Remux" {
                    format!("Remux-{}", res)
                } else {
                    format!("{}-{}", src, res)
                }
            }
            None => {
                // No resolution detected — don't fraudulently label as SD
                if src == "Unknown" {
                    "Unknown".to_string()
                } else {
                    format!("{}-Unknown", src)
                }
            }
        }
    }

    pub fn resolution_enum(&self) -> Resolution {
        match self.resolution.as_deref() {
            Some("2160p") => Resolution::UHD,
            Some("1080p") => Resolution::FHD,
            Some("720p") => Resolution::HD,
            _ => Resolution::SD,
        }
    }

    pub fn source_enum(&self) -> Source {
        match self.source.as_deref() {
            Some("Remux") => Source::Remux,
            Some("BluRay") => Source::BluRay,
            Some("BDRip") => Source::BDRip,
            Some("WebDL") | Some("WEB-DL") => Source::WebDL,
            Some("WEBRip") => Source::WEBRip,
            Some("HDTV") => Source::HDTV,
            Some("DVD") | Some("DVDRip") => Source::DVDRip,
            Some("TS") => Source::TS,
            Some("CAM") => Source::CAM,
            _ => Source::WEBRip,
        }
    }

    pub fn quality_score(&self) -> i32 {
        let src = self.source_enum();
        let res = self.resolution_enum();
        match (src, res) {
            (Source::Remux, Resolution::UHD) => 180,
            (Source::BluRay, Resolution::UHD) => 170,
            (Source::WebDL | Source::WEBRip, Resolution::UHD) => 160,
            (Source::HDTV, Resolution::UHD) => 150,

            (Source::Remux, Resolution::FHD) => 140,
            (Source::BluRay | Source::BDRip, Resolution::FHD) => 130,
            (Source::WebDL | Source::WEBRip, Resolution::FHD) => 120,
            (Source::HDTV, Resolution::FHD) => 80,

            (Source::BluRay | Source::BDRip, Resolution::HD) => 110,
            (Source::WebDL | Source::WEBRip, Resolution::HD) => 100,
            (Source::HDTV, Resolution::HD) => 70,

            (Source::BluRay, Resolution::SD) => 50,
            (Source::DVDRip, Resolution::SD) => 40,
            (Source::WebDL | Source::WEBRip, Resolution::SD) => 30,
            (Source::HDTV, Resolution::SD) => 20,
            (Source::TS, Resolution::SD) => 15,
            (Source::CAM, Resolution::SD) => 10,
            _ => 20,
        }
    }

    pub fn custom_format_score(&self) -> i32 {
        let mut score = 0;
        if self.dolby_vision { score += 50; }
        else if self.hdr { score += 30; }

        if self.viet_dub { score += 100; }
        else if self.viet_sub { score += 10; }

        if self.video_codec.as_deref() == Some("x265") || self.video_codec.as_deref() == Some("hevc") {
            score += 10;
        }

        // Simplified audio scoring based on strings
        if let Some(ac) = &self.audio_codec {
            let ac_lower = ac.to_lowercase();
            if ac_lower.contains("atmos") { score += 15; }
            else if ac_lower.contains("truehd") { score += 12; }
            else if ac_lower.contains("dts-hd") || ac_lower.contains("dtshd") { score += 10; }
            else if ac_lower.contains("dd+") || ac_lower.contains("eac3") { score += 5; }
        }

        if self.bit_depth >= 10 { score += 5; }

        score
    }

    pub fn total_score(&self) -> i32 {
        self.quality_score() + self.custom_format_score()
    }

    pub fn normalized_score(&self) -> f32 {
        let q_score = self.quality_score();
        let quality = (q_score as f32 / 180.0 * 40.0).min(40.0);
        let lang = if self.viet_dub { 25.0 } else if self.viet_sub { 15.0 } else { 10.0 };
        let hdr = if self.dolby_vision { 15.0 } else if self.hdr { 10.0 } else { 0.0 };
        
        let mut audio = 2.0;
        if let Some(ac) = &self.audio_codec {
            let ac_lower = ac.to_lowercase();
            audio = if ac_lower.contains("atmos") { 10.0 }
            else if ac_lower.contains("truehd") { 9.0 }
            else if ac_lower.contains("dts-hd") || ac_lower.contains("dtshd") { 8.0 }
            else if ac_lower.contains("dts") { 6.0 }
            else if ac_lower.contains("dd+") || ac_lower.contains("eac3") { 5.0 }
            else if ac_lower.contains("ac3") { 4.0 }
            else if ac_lower.contains("aac") { 3.0 }
            else if ac_lower.contains("mp3") { 1.0 }
            else { 2.0 };
        }
        
        let codec = if self.video_codec.as_deref() == Some("x265") || self.video_codec.as_deref() == Some("hevc") {
            10.0
        } else {
            5.0
        };
        (quality + lang + hdr + audio + codec).min(100.0)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedFilename {
    pub original_filename: String,
    pub title: String,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub year: Option<u32>,
    pub is_series: bool,
    pub quality_attrs: QualityAttributes,
}

pub struct FilenameParser;
#[allow(dead_code)]

static SE_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
static YEAR_PATTERN: OnceLock<Regex> = OnceLock::new();
static QUALITY_PATTERN: OnceLock<Regex> = OnceLock::new();
    #[allow(dead_code)]

impl FilenameParser {
    fn se_patterns() -> &'static Vec<Regex> {
        SE_PATTERNS.get_or_init(|| {
            vec![
                Regex::new(r"(?i)S(\d{1,4})E(\d{1,3})").unwrap(),           // S01E14
                Regex::new(r"(?i)S(\d{1,4})\s*EP?(\d{1,3})").unwrap(),      // S01 E14, S01 EP14
                Regex::new(r"(?i)\bE(\d{1,3})\b").unwrap(),                 // E14 (standalone)
                Regex::new(r"(?i)\bEP(\d{1,3})\b").unwrap(),                // EP14 (standalone)
                Regex::new(r"(?i)\bChapter\s*(\d{1,4})\b").unwrap(),        // Chapter 14
                Regex::new(r"(?i)\s-\s(\d{1,4})(?:\D|$)").unwrap(),         // - 01 (Anime)
                Regex::new(r"(?i)\bTap\s*(\d{1,3})\b").unwrap(),            // Tap 01 (Vietnamese)
                Regex::new(r"^(\d{1,3})[._\s]").unwrap(),                   // 01_Title (Leading digit)
                Regex::new(r"[\s._-](\d{1,3})$").unwrap(),                  // Title 01 (Trailing digit before ext)
            ]
        })
    }

    pub fn year_pattern() -> &'static Regex {
        YEAR_PATTERN.get_or_init(|| Regex::new(r"\b(19|20)\d{2}\b").unwrap())
    }

    pub fn quality_pattern() -> &'static Regex {
        QUALITY_PATTERN.get_or_init(|| {
            let keywords = vec![
                // Resolutions
                "2160p", "4k", "uhd", "8k", "4320p", "1080p", "1080i", "720p", "576p", "480p", "360p",
                // Sources
                "bluray", "blu-ray", "bdrip", "brrip", "bd25", "bd50", "web-dl", "webdl", "web-rip", "webrip",
                "hdtv", "pdtv", "tvrip", "dvdrip", "dvd", "dvd5", "dvd9", "remux", "cam", "ts", "tc",
                // Codecs
                "x265", "x264", "hevc", "avc", "h.265", "h.264", "h265", "h264", "mpeg2", "xvid", "divx", "vp9", "av1",
                // Audio
                "aac", "ac3", "dts", "dts-hd", "dtshd", "dts-x", "truehd", "atmos", "d[\\s.]*d[p\\s.]*[0-9]\\.[0-9]", "d[\\s.]*d[p\\s.]*[0-9][0-9]", "ddp", "dd\\+", "eac3", "flac", "mp3", "5\\.1", "7\\.1", "2\\.0", "2\\.1",
                // HDR/Bit Depth
                "hdr", "hdr10", "hdr10+", "dv", "dolby vision", "sdr", "10bit", "10-bit", "12bit",
                // Language/Dub Tags
                "vie", "eng", "vietsub", "thuyet minh", "long tieng", "phim long tieng", "phim thuyet minh", "tmph", "tvp", "tmpd", "sub viet", "phu de", "dub", "non-dub", "vietdub",
                // Group/Release Tags
                "jadvie", "proper", "repack", "real", "rerip", "hybrid", "dual-audio", "dual", "audio", "multi", "internal", "amzn", "nf", "hulu", "dsnp"
            ];
            let pattern = format!(r"(?i)\b({})\b", keywords.join("|"));
            Regex::new(&pattern).unwrap()
        })
    }

    #[allow(dead_code)]
    pub fn parse(filename: &str) -> ParsedFilename {
        static EXT_RE: OnceLock<Regex> = OnceLock::new();
        let name = EXT_RE.get_or_init(|| Regex::new(r"(?i)\.(mkv|mp4|avi|ts|m4v|mov|wmv|flv|webm|m2ts)$").unwrap())
            .replace(filename, "").to_string();

        let mut season = None;
        let mut episode = None;
        let mut is_series = false;

        // 1. Detect S/E Marker
        let mut pivot_start = name.len();
        let mut pivot_end = name.len();

        for pattern in Self::se_patterns() {
            if let Some(caps) = pattern.captures(&name) {
                if let Some(m) = caps.get(0) {
                    pivot_start = m.start();
                    pivot_end = m.end();
                    
                    if caps.len() == 3 {
                        season = caps.get(1).and_then(|m| m.as_str().parse().ok());
                        episode = caps.get(2).and_then(|m| m.as_str().parse().ok());
                        is_series = true;
                    } else if caps.len() == 2 {
                        season = Some(1);
                        episode = caps.get(1).and_then(|m| m.as_str().parse().ok());
                        is_series = true;
                    }
                    break;
                }
            }
        }

        // 2. Identify Year
        let year_match = Self::year_pattern().find(&name);
        let year = year_match.and_then(|m| m.as_str().parse().ok());
        
        // If no S/E found, year can be a pivot
        if !is_series {
            if let Some(m) = year_match {
                pivot_start = m.start();
                pivot_end = m.end();
            }
        }

        // 3. Extract Quality Attributes
        let quality_attrs = Self::extract_quality_attributes(filename);
        
        // 4. Determine Title Part using Pivot
        let mut title_part = if pivot_start == 0 {
            // Leading pivot (e.g. "01_Title") - title is AFTER
            if pivot_end < name.len() {
                name[pivot_end..].to_string()
            } else {
                name.to_string()
            }
        } else if pivot_start < name.len() {
            // Middle pivot (e.g. "Title.S01E01.Metadata") - title is BEFORE
            name[..pivot_start].to_string()
        } else {
            // No pivot found or at the end
            name.to_string()
        };

        // 5. Advanced Title Cleaning
        // 5a. Remove Release Group prefixes: [Group], (Group), 【Group】
        static GROUP_RE: OnceLock<Regex> = OnceLock::new();
        title_part = GROUP_RE.get_or_init(|| Regex::new(r"^(\[.*?\]|\(.*?\)|【.*?】)").unwrap())
            .replace(&title_part, "")
            .to_string();

        // 5b. Remove quality keywords from title part (case-insensitive)
        title_part = Self::quality_pattern().replace_all(&title_part, "").to_string();
        
        // 5c. Remove year from title part if it existed
        title_part = Self::year_pattern().replace_all(&title_part, "").to_string();
        
        // 5d. Handle "Double Dash" or "Separator Overload"
        title_part = title_part.replace(" - ", " ").replace('_', " ").replace('.', " ");

        // 6. Final normalization
        let mut clean_title = title_part
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        // Fallback: If title is empty after stripping (e.g. "S01E01.x264.mkv"), 
        // use the original name without the pivot
        if clean_title.is_empty() {
             clean_title = name.replace(&name[pivot_start..pivot_end], "")
                .replace('.', " ")
                .replace('_', " ")
                .trim()
                .to_string();
        }

        ParsedFilename {
            original_filename: filename.to_string(),
            title: clean_title,
            season,
            episode,
            year,
            is_series,
    #[allow(dead_code)]
            quality_attrs,
        }
    }

    pub fn extract_quality_attributes(filename: &str) -> QualityAttributes {
        let fl = filename.to_lowercase();
        let mut attrs = QualityAttributes::default();

        // Resolution
        if fl.contains("2160p") || fl.contains("4k") || fl.contains("uhd") {
            attrs.resolution = Some("2160p".to_string());
            attrs.is_hd = true;
        } else if fl.contains("1080p") || fl.contains("1080i") {
            attrs.resolution = Some("1080p".to_string());
            attrs.is_hd = true;
        } else if fl.contains("720p") {
            attrs.resolution = Some("720p".to_string());
            attrs.is_hd = true;
        }

        // Source — word-boundary regexes to avoid false positives like "ts" inside "subtitle"
        static TS_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        static TC_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        static DVD_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        let ts_re  = TS_RE.get_or_init(|| Regex::new(r"(?i)\b(ts|telesync|telecine)\b").unwrap());
        let tc_re  = TC_RE.get_or_init(|| Regex::new(r"(?i)\b(tc|telecine)\b").unwrap());
        let dvd_re = DVD_RE.get_or_init(|| Regex::new(r"(?i)\b(dvd|dvdrip|dvd5|dvd9)\b").unwrap());

        if fl.contains("remux") {
            attrs.source = Some("Remux".to_string());
        } else if fl.contains("bluray") || fl.contains("blu-ray") {
            attrs.source = Some("BluRay".to_string());
        } else if fl.contains("bdrip") || fl.contains("brrip") {
            attrs.source = Some("BDRip".to_string());
        } else if fl.contains("web-dl") || fl.contains("webdl") {
            attrs.source = Some("WebDL".to_string());
        } else if fl.contains("webrip") || fl.contains("web-rip") {
            attrs.source = Some("WEBRip".to_string());
        } else if fl.contains("hdtv") || fl.contains("pdtv") {
            attrs.source = Some("HDTV".to_string());
        } else if dvd_re.is_match(&fl) {
            attrs.source = Some("DVDRip".to_string());
        } else if ts_re.is_match(&fl) || tc_re.is_match(&fl) {
            attrs.source = Some("TS".to_string());
        } else if fl.contains("cam") {
            attrs.source = Some("CAM".to_string());
        }

        // Codec
        if fl.contains("x265") || fl.contains("hevc") || fl.contains("h.265") {
            attrs.video_codec = Some("x265".to_string());
        } else if fl.contains("x264") || fl.contains("avc") || fl.contains("h.264") {
            attrs.video_codec = Some("x264".to_string());
        }

        // Audio
        static DD_RE: OnceLock<Regex> = OnceLock::new();
        let dd_re = DD_RE.get_or_init(|| Regex::new(r"(?i)dd[p\s.]*[0-9](\.[0-9])?").unwrap());
        
        if fl.contains("atmos") {
            attrs.audio_codec = Some("Atmos".to_string());
        } else if fl.contains("truehd") {
            attrs.audio_codec = Some("TrueHD".to_string());
        } else if fl.contains("dts-hd") || fl.contains("dtshd") || fl.contains("dts:x") {
            attrs.audio_codec = Some("DTS-HD".to_string());
        } else if fl.contains("dts") {
            attrs.audio_codec = Some("DTS".to_string());
        } else if fl.contains("eac3") || fl.contains("dd+") || fl.contains("ddp") {
            attrs.audio_codec = Some("EAC3".to_string());
        } else if dd_re.is_match(&fl) || fl.contains("ac3") {
            attrs.audio_codec = Some("AC3".to_string());
        } else if fl.contains("aac") {
            attrs.audio_codec = Some("AAC".to_string());
        } else if fl.contains("mp3") {
            attrs.audio_codec = Some("MP3".to_string());
        }

        // HDR
        if fl.contains("hdr") || fl.contains("hdr10") {
            attrs.hdr = true;
        }
        if fl.contains("dv") || fl.contains("dolby vision") {
            attrs.dolby_vision = true;
            attrs.hdr = true;
        }

        // Bit Depth
        if fl.contains("10bit") || fl.contains("10 bit") {
            attrs.bit_depth = 10;
        } else if fl.contains("12bit") || fl.contains("12 bit") {
            attrs.bit_depth = 12;
        }

        // Vietnamese
        let viet_sub_markers = ["vietsub", "sub viet", "phu de", "phụ đề"];
        let viet_dub_markers = ["vie", "thuyet minh", "thuyết minh", "long tieng", "lồng tiếng", "tmph", "tvp", "tmpd", "vietdub"];
        
        if viet_sub_markers.iter().any(|m| fl.contains(m)) {
            attrs.viet_sub = true;
        }
        if viet_dub_markers.iter().any(|m| fl.contains(m)) {
            attrs.viet_dub = true;
        }

        attrs
    }
}




