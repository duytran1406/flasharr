//! Smart Token-Based Filename Parser
//! 
//! This module implements a Guessit-style tokenization approach for parsing
//! media filenames. Instead of relying on positional regex patterns, it:
//! 1. Tokenizes the filename into individual parts
//! 2. Classifies each token using priority-based pattern matching
//! 3. Infers the title from unclassified tokens before the first metadata token

use regex::Regex;
use serde::{Deserialize, Serialize};

use std::sync::OnceLock;

// ============================================================================
// Token Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TokenType {
    // High priority - easy to identify
    Extension,      // .mkv, .mp4, etc.
    Year,           // 1999, 2023, etc.
    SeasonEpisode,  // S01E01, 1x01
    Episode,        // E01, EP01, standalone episode
    Season,         // S01, Season 1
    
    // Medium priority - known vocabulary
    Resolution,     // 2160p, 1080p, 720p, 4K, UHD
    Source,         // BluRay, WEB-DL, HDTV, etc.
    VideoCodec,     // x265, HEVC, x264, AVC
    AudioCodec,     // DTS-HD, Atmos, AAC, AC3
    AudioChannels,  // 5.1, 7.1, 2.0
    BitDepth,       // 10bit, 12bit
    HDR,            // HDR, HDR10, Dolby Vision, DV
    
    // Language/Localization
    VietDub,        // Lồng tiếng, Long tieng, Thuyết minh
    VietSub,        // Vietsub, Phụ đề
    Language,       // ViE, ENG, etc.
    
    // Structural
    ReleaseGroup,   // -GROUP, [Group], etc.
    BracketGroup,   // [Something], (Something)
    
    // Fallback
    Title,          // Inferred title part
    Unknown,        // Unclassified token
}

// ============================================================================
// Classified Token
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedToken {
    pub text: String,
    pub token_type: TokenType,
    pub value: Option<String>,  // Extracted value (e.g., "01" for episode)
    pub position: usize,        // Position in original token list
}

// ============================================================================
// Parsed Result
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MediaType {
    Movie,
    TvShow,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartParsedMedia {
    pub original: String,
    pub title: String,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub resolution: Option<String>,
    pub source: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub audio_channels: Option<String>,
    pub hdr: bool,
    pub dolby_vision: bool,
    pub viet_dub: bool,
    pub viet_sub: bool,
    pub release_group: Option<String>,
    pub tokens: Vec<ClassifiedToken>,
    
    // Scoring & Classification
    pub media_type: MediaType,
    pub confidence: f32, // 0.0 to 1.0
}

impl SmartParsedMedia {
    pub fn quality_name(&self) -> String {
        let src = self.source.as_deref().unwrap_or("Unknown");
        let res = self.resolution.as_deref().unwrap_or("SD");
        if src == "Remux" {
            format!("Remux-{}", res)
        } else {
            format!("{}-{}", src, res)
        }
    }

    pub fn quality_score(&self) -> i32 {
        use crate::utils::parser::{Resolution, Source};
        
        let src = match self.source.as_deref() {
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
        };
        
        let res = match self.resolution.as_deref() {
            Some("2160p") | Some("4K") => Resolution::UHD,
            Some("1080p") => Resolution::FHD,
            Some("720p") => Resolution::HD,
            _ => Resolution::SD,
        };
        
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

        if let Some(ac) = &self.audio_codec {
            let ac_lower = ac.to_lowercase();
            if ac_lower.contains("atmos") { score += 15; }
            else if ac_lower.contains("truehd") { score += 12; }
            else if ac_lower.contains("dts-hd") || ac_lower.contains("dtshd") { score += 10; }
            else if ac_lower.contains("dd+") || ac_lower.contains("eac3") { score += 5; }
        }

        score
    }

    pub fn total_score(&self) -> i32 {
        self.quality_score() + self.custom_format_score()
    }

    pub fn normalized_score(&self) -> f32 {
        (self.total_score() as f32 / 300.0).min(1.0)
    }
}

// ============================================================================
// Token Classifiers (Static Patterns)
// ============================================================================

static EXTENSION_RE: OnceLock<Regex> = OnceLock::new();
static YEAR_RE: OnceLock<Regex> = OnceLock::new();
static SE_RE: OnceLock<Regex> = OnceLock::new();
static SEASON_RE: OnceLock<Regex> = OnceLock::new();
static EPISODE_RE: OnceLock<Regex> = OnceLock::new();
static RESOLUTION_RE: OnceLock<Regex> = OnceLock::new();
static SOURCE_RE: OnceLock<Regex> = OnceLock::new();
static VIDEO_CODEC_RE: OnceLock<Regex> = OnceLock::new();
static AUDIO_CODEC_RE: OnceLock<Regex> = OnceLock::new();
static AUDIO_CHANNELS_RE: OnceLock<Regex> = OnceLock::new();
static BIT_DEPTH_RE: OnceLock<Regex> = OnceLock::new();
static HDR_RE: OnceLock<Regex> = OnceLock::new();
static VIETDUB_RE: OnceLock<Regex> = OnceLock::new();
static VIETSUB_RE: OnceLock<Regex> = OnceLock::new();
static LANGUAGE_RE: OnceLock<Regex> = OnceLock::new();
static BRACKET_RE: OnceLock<Regex> = OnceLock::new();
static RELEASE_GROUP_RE: OnceLock<Regex> = OnceLock::new();

fn get_classifiers() -> Vec<(TokenType, &'static Regex)> {
    vec![
        // High priority patterns
        (TokenType::Extension, EXTENSION_RE.get_or_init(|| 
            Regex::new(r"(?i)^(mkv|mp4|avi|mov|wmv|flv|webm|ts|m2ts|m4v|iso|rar|zip|7z)$").unwrap())),
        (TokenType::SeasonEpisode, SE_RE.get_or_init(|| 
            Regex::new(r"(?i)^S(\d{1,2})E(\d{1,4})$").unwrap())),
        (TokenType::Season, SEASON_RE.get_or_init(|| 
            Regex::new(r"(?i)^(?:S(\d{1,2})|Season\s*(\d{1,2}))$").unwrap())),
        (TokenType::Episode, EPISODE_RE.get_or_init(|| 
            Regex::new(r"(?i)^(?:E\s*(\d{1,4})|EP\s*(\d{1,4})|Tap\s*(\d{1,4})|Chapter\s*(\d{1,4})|Ep\s*(\d{1,4}))$").unwrap())),
        (TokenType::Year, YEAR_RE.get_or_init(|| 
            Regex::new(r"^(19[2-9]\d|20[0-4]\d)$").unwrap())),
        
        // Resolution
        (TokenType::Resolution, RESOLUTION_RE.get_or_init(|| 
            Regex::new(r"(?i)^(2160p|4k|uhd|1080p|1080i|720p|576p|480p|360p|fhd|hd)$").unwrap())),
        
        // Source
        (TokenType::Source, SOURCE_RE.get_or_init(|| 
            Regex::new(r"(?i)^(bluray|blu-ray|bdrip|brrip|bd25|bd50|web-dl|webdl|webrip|web-rip|hdtv|pdtv|tvrip|dvdrip|dvd|remux|cam|ts|tc|hdcam|webscreener|screener|scr|webscr)$").unwrap())),
        
        // Video Codec
        (TokenType::VideoCodec, VIDEO_CODEC_RE.get_or_init(|| 
            Regex::new(r"(?i)^(x265|x264|hevc|avc|h\.?265|h\.?264|mpeg-?2|mpeg-?4|xvid|divx|vp9|av1)$").unwrap())),
        
        // Audio Codec
        (TokenType::AudioCodec, AUDIO_CODEC_RE.get_or_init(|| 
            Regex::new(r"(?i)^(aac.*|ac3|dts|dts-hd|dtshd|dts-x|truehd|atmos|eac3|dd\+?|ddp|flac|mp3|opus)$").unwrap())),
        
        // Audio Channels
        (TokenType::AudioChannels, AUDIO_CHANNELS_RE.get_or_init(|| 
            Regex::new(r"^([257]\.[01])$").unwrap())),
        
        // Bit Depth
        (TokenType::BitDepth, BIT_DEPTH_RE.get_or_init(|| 
            Regex::new(r"(?i)^(10bit|10-bit|12bit|12-bit|8bit)$").unwrap())),
        
        // HDR
        (TokenType::HDR, HDR_RE.get_or_init(|| 
            Regex::new(r"(?i)^(hdr|hdr10|hdr10\+|dv|dolby\s*vision|sdr)$").unwrap())),
        
        // Vietnamese Audio
        (TokenType::VietDub, VIETDUB_RE.get_or_init(|| 
            Regex::new(r"(?i)^(lồng\s*tiếng|long\s*tieng|thuyết\s*minh|thuyet\s*minh|vietdub|vie\.?dub|tmph|tvp|tmpd|tm|lt)$").unwrap())),
        
        // Vietnamese Subtitles
        (TokenType::VietSub, VIETSUB_RE.get_or_init(|| 
            Regex::new(r"(?i)^(vietsub|sub\s*viet|phụ\s*đề|phu\s*de|vs)$").unwrap())),
        
        // Language codes
        (TokenType::Language, LANGUAGE_RE.get_or_init(|| 
            Regex::new(r"(?i)^(vie|eng|jpn|kor|chi|chn|tha|vn|dub|dual)$").unwrap())),
        
        // Release group (must start with dash, e.g. -GROUP)
        (TokenType::ReleaseGroup, RELEASE_GROUP_RE.get_or_init(|| 
            Regex::new(r"^-[A-Za-z0-9]+$").unwrap())),
    ]
}

// ============================================================================
// Tokenizer
// ============================================================================

/// Tokenize a filename into individual parts
pub fn tokenize(filename: &str) -> Vec<String> {
    // Step 1: Remove extension first and track it
    let (name, ext) = split_extension(filename);
    
    // Step 2: Extract bracketed groups first [Group], (Group), 【Group】
    let bracket_re = BRACKET_RE.get_or_init(|| 
        Regex::new(r"\[([^\]]+)\]|\(([^\)]+)\)|【([^】]+)】").unwrap());
    
    let mut tokens = Vec::new();
    let mut last_end = 0;
    
    for caps in bracket_re.captures_iter(&name) {
        let m = caps.get(0).unwrap();
        
        // Tokenize content before this bracket
        let before = &name[last_end..m.start()];
        tokens.extend(split_by_separators(before));
        
        // Add the bracket content as a single token (without brackets)
        if let Some(inner) = caps.get(1).or(caps.get(2)).or(caps.get(3)) {
            tokens.push(format!("[{}]", inner.as_str()));
        }
        
        last_end = m.end();
    }
    
    // Tokenize remaining content
    let remaining = &name[last_end..];
    tokens.extend(split_by_separators(remaining));
    
    // Add extension as last token
    if !ext.is_empty() {
        tokens.push(ext);
    }
    
    // Filter out empty tokens
    tokens.into_iter().filter(|t| !t.is_empty()).collect()
}

/// Split a string by common separators (dot, underscore, dash, space)
fn split_by_separators(s: &str) -> Vec<String> {
    // Split by common separators but keep multi-word tokens together
    // Custom split to protect floats (e.g. 5.1, 2.0) and codecs (e.g. H.264)
    let mut parts: Vec<String> = Vec::new(); 
    let mut current = String::new();
    let chars: Vec<char> = s.chars().collect();
    
    for i in 0..chars.len() {
        let c = chars[i];
        // Treat underscore and space as hard separators
        if c == '_' || c == ' ' {
            if !current.is_empty() { parts.push(current.clone()); current.clear(); }
        } 
        // Treat dot as separator UNLESS it looks like a float or a codec version
        else if c == '.' {
            let prev_char = current.chars().last();
            let prev_is_digit = prev_char.map_or(false, |l| l.is_ascii_digit());
            // Codec protection: H.264, h.264, x.264
            let prev_is_codec_start = prev_char.map_or(false, |l| l == 'H' || l == 'h' || l == 'x');
            
            // Look ahead to find digits
            let mut digits_after = 0;
            for k in i+1..chars.len() {
                if chars[k].is_ascii_digit() {
                    digits_after += 1;
                } else {
                    break;
                }
            }
            
            // Protect pattern matching H.264 or x.264 or 5.1
            if (prev_is_digit || prev_is_codec_start) && digits_after > 0 && digits_after <= 3 {
                current.push(c); 
            } else {
                if !current.is_empty() { parts.push(current.clone()); current.clear(); }
            }
        }
        else {
            current.push(c);
        }
    }
    if !current.is_empty() { parts.push(current); }

    // Convert to ref for the loop below
    let parts_refs: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
    let parts_iter = parts_refs.into_iter(); // Use iterator for logic below
    
    // Handle dash specially - it might be part of a compound word
    let mut result = Vec::new();
    for part in parts_iter {
        // Check if this part contains a dash that should split
        if part.contains('-') {
            // Don't split if it's a known compound (DTS-HD, WEB-DL, etc.)
            let lower = part.to_lowercase();
            if lower == "dts-hd" || lower == "web-dl" || lower == "dts-x" || 
               lower == "blu-ray" || lower == "hdr10+" || lower == "10-bit" ||
               lower == "web-rip" || lower.starts_with("dd") || 
               lower.starts_with("mpeg") || lower.starts_with("aac") || 
               lower == "e-ac3" || lower == "true-hd" {
                result.push(part.to_string());
            } else {
                // Split by dash
                for sub in part.split('-').filter(|s| !s.is_empty()) {
                    result.push(sub.to_string());
                }
            }
        } else {
            result.push(part.to_string());
        }
    }
    result
}

/// Split extension from filename
fn split_extension(filename: &str) -> (String, String) {
    let ext_re = EXTENSION_RE.get_or_init(|| 
        Regex::new(r"(?i)^(mkv|mp4|avi|mov|wmv|flv|webm|ts|m2ts|m4v|iso|rar|zip|7z)$").unwrap());
    
    if let Some(dot_pos) = filename.rfind('.') {
        let ext = &filename[dot_pos + 1..];
        if ext_re.is_match(ext) {
            return (filename[..dot_pos].to_string(), ext.to_string());
        }
    }
    (filename.to_string(), String::new())
}

// ============================================================================
// Token Classification
// ============================================================================

/// Classify a single token
pub fn classify_token(token: &str, position: usize, is_last: bool) -> ClassifiedToken {
    let classifiers = get_classifiers();
    
    // Handle bracket groups specially
    if token.starts_with('[') && token.ends_with(']') {
        let inner = &token[1..token.len()-1];
        
        // Try to classify the inner content first
        // If it's single-token metadata (like Year, Resolution), treat it as such
        /* recursion would be nice but simple check is enough */
        let classifiers = get_classifiers();
        for (token_type, regex) in &classifiers {
            // Skip structural types for inner check
            if *token_type == TokenType::BracketGroup || *token_type == TokenType::ReleaseGroup {
                continue;
            }
            if regex.is_match(inner) {
                 return classify_token(inner, position, is_last);
            }
        }

        // If not metadata, check if it's a Release Group (strict) or Title
        // If it contains spaces, it's likely a Title (or composite info check later)
        if inner.contains(' ') {
             // For now, keep as BracketGroup, but we'll use it for Title fallback
             return ClassifiedToken {
                text: token.to_string(),
                token_type: TokenType::BracketGroup,
                value: Some(inner.to_string()),
                position,
            };
        }

        // Single word in brackets - likely Release Group or Hash
        return ClassifiedToken {
            text: token.to_string(),
            token_type: TokenType::ReleaseGroup,
            value: Some(inner.to_string()),
            position,
        };
    }
    
    // Try each classifier
    for (token_type, regex) in &classifiers {
        if regex.is_match(token) {
            // Strict ID check for ReleaseGroup (must not be all digits)
            if *token_type == TokenType::ReleaseGroup {
                 // Check if it's purely digits - if so, it's NOT a release group (likely Episode/Year)
                 if token.chars().all(|c| c.is_ascii_digit()) {
                     continue;
                 }
                 if !is_last {
                     continue;
                 }
            }

            // Extract value based on token type
            let value = match token_type {
                TokenType::Year => Some(token.to_string()),
                TokenType::SeasonEpisode => {
                    if let Some(caps) = regex.captures(token) {
                        let s = caps.get(1).map(|m| m.as_str()).unwrap_or("1");
                        let e = caps.get(2).map(|m| m.as_str()).unwrap_or("0");
                        Some(format!("S{}E{}", s, e))
                    } else {
                        None
                    }
                },
                TokenType::Season => {
                    if let Some(caps) = regex.captures(token) {
                        let s = caps.get(1).or(caps.get(2)).map(|m| m.as_str()).unwrap_or("1");
                        Some(s.to_string())
                    } else {
                        None
                    }
                },
                TokenType::Episode => {
                    if let Some(caps) = regex.captures(token) {
                        let e = caps.get(1).or(caps.get(2)).or(caps.get(3)).or(caps.get(4))
                            .map(|m| m.as_str()).unwrap_or("0");
                        Some(e.to_string())
                    } else {
                        None
                    }
                },
                TokenType::Resolution | TokenType::Source | TokenType::VideoCodec |
                TokenType::AudioCodec | TokenType::AudioChannels | TokenType::BitDepth |
                TokenType::HDR => Some(token.to_lowercase()),
                _ => Some(token.to_string()),
            };
            
            return ClassifiedToken {
                text: token.to_string(),
                token_type: token_type.clone(),
                value,
                position,
            };
        }
    }

    
    // Check for standalone numbers (potential episode)
    if let Ok(num) = token.parse::<u32>() {
        if num >= 1 && num <= 999 {
            return ClassifiedToken {
                text: token.to_string(),
                token_type: TokenType::Episode,
                value: Some(num.to_string()),
                position,
            };
        }
    }
    
    // Pattern-based fallback: Extract leading digits from tokens like "40End", "22Fixed"
    // This handles cases where episode numbers are followed by descriptive text
    if token.len() >= 2 {
        let leading_digits: String = token.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        
        if !leading_digits.is_empty() {
            if let Ok(num) = leading_digits.parse::<u32>() {
                if num >= 1 && num <= 999 {
                    // Only treat as episode if the remaining part looks like metadata
                    let remaining = &token[leading_digits.len()..];
                    let metadata_keywords = ["end", "fixed", "repack", "proper", "final", "complete"];
                    
                    if metadata_keywords.iter().any(|&kw| remaining.to_lowercase().starts_with(kw)) {
                        return ClassifiedToken {
                            text: token.to_string(),
                            token_type: TokenType::Episode,
                            value: Some(num.to_string()),
                            position,
                        };
                    }
                }
            }
        }
    }
    
    // Unknown token
    ClassifiedToken {
        text: token.to_string(),
        token_type: TokenType::Unknown,
        value: None,
        position,
    }
}

/// Classify all tokens in a filename
pub fn classify_tokens(tokens: &[String]) -> Vec<ClassifiedToken> {
    let len = tokens.len();
    tokens.iter().enumerate()
        .map(|(i, t)| classify_token(t, i, i == len - 1 || i == len - 2))
        .collect()
}

// ============================================================================
// Title Extraction
// ============================================================================

/// Find the title boundary - position of first metadata token
fn find_title_boundary(tokens: &[ClassifiedToken]) -> usize {
    // Skip leading bracket groups (release groups at start)
    let mut start = 0;
    for token in tokens {
        match token.token_type {
            TokenType::BracketGroup | TokenType::ReleaseGroup | 
            TokenType::VietSub | TokenType::VietDub | 
            TokenType::Resolution | TokenType::HDR | TokenType::Year |
            TokenType::AudioChannels | TokenType::Source => {
                start = token.position + 1;
            },
            _ => break,
        }
    }
    
    // Find first metadata token after start
    for token in tokens.iter().skip(start) {
        match token.token_type {
            TokenType::Unknown | TokenType::Title => continue,
            TokenType::Episode => {
                // Episode at position 0-1 is likely leading episode number
                if token.position <= 1 {
                    continue;
                }
                return token.position;
            },
            _ => return token.position,
        }
    }
    
    tokens.len()
}

/// Extract title from classified tokens
fn extract_title(tokens: &[ClassifiedToken], boundary: usize) -> String {
    let title_tokens: Vec<&str> = tokens.iter()
        .filter(|t| t.position < boundary && t.token_type == TokenType::Unknown)
        .map(|t| t.text.as_str())
        .collect();
    

    
    let mut title_part = title_tokens.join(" ");
    
    // Fallback: If no title found (e.g. filename is "[Title] Metadata"), use the first bracket group
    if title_part.is_empty() {
        if let Some(bracket_token) = tokens.iter().find(|t| t.token_type == TokenType::BracketGroup && t.position < boundary) {
            let text = &bracket_token.text;
            // Strip brackets [] or () or 【】
            if text.len() >= 2 {
                title_part = text[1..text.len()-1].to_string();
            } else {
                title_part = text.to_string();
            }
        }
    }
    
    title_part
}

// ============================================================================
// Main Parse Function
// ============================================================================

/// Parse a filename using smart tokenization
pub fn smart_parse(filename: &str) -> SmartParsedMedia {
    // Step 1: Tokenize
    let tokens = tokenize(filename);
    
    // Step 2: Classify each token
    let mut classified = classify_tokens(&tokens);

    // Step 2.5: Refine classifications based on context
    // This phase fixes common misclassifications like "Movie 44" being seen as "Episode 44"
    for i in 0..classified.len().saturating_sub(1) {
        let current_text = classified[i].text.to_lowercase();
        let next_text = classified[i+1].text.to_lowercase();

        // 1. Vietnamese Audio Phrases: "Lồng tiếng", "Thuyết minh"
        if ((current_text == "lồng" || current_text == "long") && (next_text == "tiếng" || next_text == "tiéng" || next_text == "tieng")) || 
           ((current_text == "thuyết" || current_text == "thuyet") && next_text == "minh") 
        {
             classified[i].token_type = TokenType::VietDub;
             classified[i+1].token_type = TokenType::VietDub;
        }
        
        // 2. Keyword Context: "Movie 44", "Part 2", "Vol 1", "Cot moc 23"
        // If an episode-looking number follows a title keyword, it's part of the title
        let title_keywords = ["movie", "film", "collection", "part", "vol", "volume", "chap", "chapter", "cot", "moc", "milestone"];
        if title_keywords.contains(&current_text.as_str()) 
            && classified[i+1].token_type == TokenType::Episode 
        {
             // Only protect standalone numbers (e.g. "44", not "E44")
             if let Some(val) = &classified[i+1].value {
                 if val == &classified[i+1].text {
                      classified[i+1].token_type = TokenType::Unknown; 
                      classified[i+1].value = None;
                 }
             }
        }

        // 3. Structural Context: "Title 23 2011"
        // If a standalone number is followed by a Year, it's very likely part of the title
        if classified[i].token_type == TokenType::Unknown 
            && classified[i+1].token_type == TokenType::Episode
        {
            if let Some(val) = &classified[i+1].value {
                if val == &classified[i+1].text {
                    // Look ahead for Year (Resolution is too ambiguous for TV shows like "Show 01 1080p")
                    if i + 2 < classified.len() {
                        let next_type = &classified[i+2].token_type;
                        if *next_type == TokenType::Year {
                            classified[i+1].token_type = TokenType::Unknown;
                            classified[i+1].value = None;
                        }
                    }
                }
            }
        }
    }
    
    // Step 3: Find title boundary and extract title
    let boundary = find_title_boundary(&classified);
    
    let title = extract_title(&classified, boundary);
    
    // Step 4: Mark title tokens
    for token in &mut classified {
        if token.position < boundary && token.token_type == TokenType::Unknown {
            token.token_type = TokenType::Title;
        }
    }
    
    // Step 5: Extract metadata from classified tokens
    let mut result = SmartParsedMedia {
        original: filename.to_string(),
        title,
        year: None,
        season: None,
        episode: None,
        resolution: None,
        source: None,
        video_codec: None,
        audio_codec: None,
        audio_channels: None,
        hdr: false,
        dolby_vision: false,
        viet_dub: false,
        viet_sub: false,
        release_group: None,
        tokens: classified.clone(),
        media_type: MediaType::Unknown,
        confidence: 0.0,
    };
    
    for token in &classified {
        match token.token_type {
            TokenType::Year => {
                if let Some(ref v) = token.value {
                    result.year = v.parse().ok();
                }
            },
            TokenType::SeasonEpisode => {
                if let Some(ref v) = token.value {
                    // Parse S1E2 format
                    let parts: Vec<&str> = v.split(|c| c == 'S' || c == 'E')
                        .filter(|s| !s.is_empty())
                        .collect();
                    if parts.len() >= 2 {
                        result.season = parts[0].parse().ok();
                        result.episode = parts[1].parse().ok();
                    }
                }
            },
            TokenType::Season => {
                if let Some(ref v) = token.value {
                    result.season = v.parse().ok();
                }
            },
            TokenType::Episode => {
                if result.episode.is_none() {
                    if let Some(ref v) = token.value {
                        result.episode = v.parse().ok();
                    }
                }
            },
            TokenType::Resolution => {
                if result.resolution.is_none() {
                    result.resolution = token.value.clone();
                }
            },
            TokenType::Source => {
                let new_val = token.value.clone().unwrap_or_default().to_lowercase();
                let current_val = result.source.clone().unwrap_or_default().to_lowercase();
                
                // Priority refinement: REMUX always wins
                if current_val == "remux" {
                    // Keep existing
                } else if new_val == "remux" {
                    result.source = Some("remux".to_string());
                } else if result.source.is_none() {
                    result.source = Some(new_val);
                }
            },
            TokenType::VideoCodec => {
                if result.video_codec.is_none() {
                    result.video_codec = token.value.clone();
                }
            },
            TokenType::AudioCodec => {
                if result.audio_codec.is_none() {
                    result.audio_codec = token.value.clone();
                }
            },
            TokenType::AudioChannels => {
                if result.audio_channels.is_none() {
                    result.audio_channels = token.value.clone();
                }
            },
            TokenType::HDR => {
                if let Some(ref v) = token.value {
                    let lower = v.to_lowercase();
                    if lower.contains("dv") || lower.contains("dolby") {
                        result.dolby_vision = true;
                        result.hdr = true;
                    } else if lower.contains("hdr") {
                        result.hdr = true;
                    }
                }
            },
            TokenType::VietDub => {
                result.viet_dub = true;
            },
            TokenType::VietSub => {
                result.viet_sub = true;
            },
            TokenType::BracketGroup | TokenType::ReleaseGroup => {
                if result.release_group.is_none() {
                    result.release_group = token.value.clone();
                }
            },
            _ => {}
        }
    }
    
    // Default season to 1 if episode is found but no season
    if result.episode.is_some() && result.season.is_none() {
        result.season = Some(1);
    }
    
    // Step 6: Determine Media Type based on Weights
    let (media_type, confidence) = determine_media_type(&result);
    // If confidence is high for Movie, and we have an episode number which looks suspicious (like Movie 44), 
    // we might want to unset the episode to prevent confusion downstream?
    // But for now, just setting the media_type hint is enough for the searcher.
    result.media_type = media_type;
    result.confidence = confidence;

    result
}

/// Determine likely media type based on weighted signals
fn determine_media_type(media: &SmartParsedMedia) -> (MediaType, f32) {
    let mut movie_score = 0.0f32;
    let mut series_score = 0.0f32;
    
    // 1. Year Signal (Movies usually have specific years, Series often do too but less critical for identity if S/E exists)
    if media.year.is_some() {
        movie_score += 10.0;
        series_score += 5.0; // Series can have years too (e.g. 2025)
    }
    
    // 2. Season/Episode Signals
    if media.season.is_some() {
        series_score += 20.0; // Strong indicator
    }
    if media.episode.is_some() {
        series_score += 15.0;
    }
    
    // 3. Keyword Signals in Title
    let title_lower = media.title.to_lowercase();
    if title_lower.contains("movie") || title_lower.contains("film") || title_lower.contains("theatrical") {
        movie_score += 15.0; // "Doraemon Movie" -> Strong Movie signal
    }
    if title_lower.contains("season") || title_lower.contains("series") || title_lower.contains("collection") {
        series_score += 15.0;
    }
    
    // 4. Pattern Specifics
    // If we have "Movie" keyword AND an Episode number, it's often a Movie Sequence (Movie 44), not TV S1E44
    if (title_lower.contains("movie") || title_lower.contains("film")) && media.episode.is_some() {
        // Boost movie score significantly to override series score from Episode presence
        movie_score += 15.0; 
    }
    
    // 5. Title Length (Heuristic)
    // Movies often have longer subtitles (e.g. "Nobita's Art World Tales")
    let word_count = media.title.split_whitespace().count();
    if word_count > 4 {
        movie_score += 2.0; 
    }

    // Calculate Final Verdict
    let total = movie_score + series_score;
    if total == 0.0 {
        return (MediaType::Unknown, 0.0);
    }
    
    if movie_score > series_score {
        // Normalize confidence
        let conf = (movie_score / (movie_score + series_score * 0.5)).min(1.0);
        (MediaType::Movie, conf)
    } else {
        let conf = (series_score / (series_score + movie_score * 0.5)).min(1.0);
        (MediaType::TvShow, conf)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_parse(filename: &str) -> SmartParsedMedia {
        let result = smart_parse(filename);
        println!("\n=== {} ===", filename);
        println!("Title: '{}'", result.title);
        println!("Year: {:?}, Season: {:?}, Episode: {:?}", result.year, result.season, result.episode);
        println!("Resolution: {:?}, Source: {:?}", result.resolution, result.source);
        println!("VietDub: {}, VietSub: {}", result.viet_dub, result.viet_sub);
        println!("Tokens: {:?}", result.tokens.iter().map(|t| (&t.text, &t.token_type)).collect::<Vec<_>>());
        result
    }

    #[test]
    fn test_scarlet_heart_samples() {
        // Standard S01E01 format
        let r = test_parse("Bộ Bộ Kinh Tâm - S01E17 - CH Bo Bo Kinh Tam 17.mkv");
        assert_eq!(r.season, Some(1));
        assert_eq!(r.episode, Some(17));
        
        // [Group] prefix with trailing episode
        let r = test_parse("[Phim Media] Bo Bo Kinh Tam 01.mkv");
        assert_eq!(r.episode, Some(1));
        assert!(r.release_group.is_some());
        
        // Leading episode number with 4K
        let r = test_parse("01_Bo Bo kinh Tam_4K_Long tieng.mp4");
        assert_eq!(r.episode, Some(1));
        assert_eq!(r.resolution, Some("4k".to_string()));
        assert!(r.viet_dub);
        
        // Trailing episode with resolution
        let r = test_parse("Bo Bo Kinh Tam_33_720P.mkv");
        assert_eq!(r.episode, Some(33));
        assert_eq!(r.resolution, Some("720p".to_string()));
    }

    #[test]
    fn test_doraemon_samples() {
        // Complex Doraemon movie
        let r = test_parse("Doraemon.Movie.44.Nobitas.Art.World.Tales.2025.ViE.DUB.1080p.BDRip.HEVC.10bit.AAC.2.0-JadViE.mkv");
        assert_eq!(r.year, Some(2025));
        assert_eq!(r.resolution, Some("1080p".to_string()));
        assert_eq!(r.source, Some("bdrip".to_string()));
        assert_eq!(r.video_codec, Some("hevc".to_string()));
        
        // [Group] prefix with year
        let r = test_parse("[J-Zone].Doraemon.Movie.2010.Nobita.Great.Battle.Of.The.Mermaid.King.KITES.VN.mkv");
        assert_eq!(r.year, Some(2010));
        assert!(r.release_group.is_some());
        
        // Vietnamese with mixed language
        let r = test_parse("Doraemon Movie 43- Nobita Và Bản Giao Hưởng Địa Cầu 1080p BluRay REMUX Lồng tiếng_Vietsub.mkv");
        assert_eq!(r.resolution, Some("1080p".to_string()));
        assert_eq!(r.source, Some("remux".to_string()));
        assert!(r.viet_dub);
        assert!(r.viet_sub);
        
        // Parentheses group
        let r = test_parse("(Vietsub) Doraemon The Movie 2023 - Nobitas Sky Utopia (1920x1080 BDRip)-KM.mkv");
        assert_eq!(r.year, Some(2023));
        assert!(r.viet_sub);
    }

    #[test]
    fn test_numeric_titles() {
        // Vietnamese movie with number in title
        let r = test_parse("Phim.Viet.Nam.Cot.Moc.23.2011.1080p.m2ts");
        assert_eq!(r.title, "Phim Viet Nam Cot Moc 23");
        assert_eq!(r.year, Some(2011));
        
        // English movie with number in title
        let r = test_parse("District.9.2009.1080p.Bluray.x264.mkv");
        assert_eq!(r.title, "District 9");
        assert_eq!(r.year, Some(2009));

        // Part/Vol keywords
        let r = test_parse("Harry.Potter.And.The.Deathly.Hallows.Part.2.2011.1080p.mkv");
        assert!(r.title.contains("Part 2"));
    }

    #[test]
    fn test_user_reported_issues() {
        // Issue 1: Bride of the Covenant (Fshare 2025, TMDB 2024/2025)
        let r = test_parse("Bride.of.the.Covenant.2025.1080p.WEB-DL.AAC2.0.H.264-HETTIEN.mkv");
        assert_eq!(r.title, "Bride of the Covenant");
        assert_eq!(r.year, Some(2025));

        // Issue 2: Gia Sư Nữ Quái (Space in title, Vietnamese)
        let r = test_parse("Gia Sư Nữ Quái 2012 1080p WEB-DL H.264 AAC2.0-3cTWeB.mkv");
        assert_eq!(r.title, "Gia Sư Nữ Quái");
        assert_eq!(r.year, Some(2012));

        // Issue 3: Avatar Fire and Ash (WEBSCREENER keyword)
        let r = test_parse("Avatar.Fire.and.Ash.2025.2160p.WEBSCREENER.H.265.Dual YG (Vietsub).mkv");
        assert_eq!(r.title, "Avatar Fire and Ash");
        assert_eq!(r.year, Some(2025));

        // Issue 4: Làm Giàu Với Ma (Long title/subtitle)
        let r = test_parse("Làm.Giàu.Với.Ma.-.Cuộc.Chiến.Hột.Xoàn.2025.1080p.WEB-DL.DDP5.1.H.264-HBO.mkv");
        assert!(r.title.contains("Làm Giàu Với Ma"));
        assert_eq!(r.year, Some(2025));
    }
}
