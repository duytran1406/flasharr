use regex::Regex;
use std::collections::{HashSet, HashMap};
use std::sync::OnceLock;
use serde::{Deserialize, Serialize};
use crate::utils::parser::FilenameParser;

static FRANCHISE_CONFLICTS: OnceLock<Vec<(&'static str, Vec<&'static str>)>> = OnceLock::new();

pub fn get_franchise_conflicts() -> &'static Vec<(&'static str, Vec<&'static str>)> {
    FRANCHISE_CONFLICTS.get_or_init(|| {
        vec![
            ("predator", vec!["killer of killers", "prey", "dark ages", "alien vs", "requiem"]),
            ("alien", vec!["romulus", "resurrection", "covenant", "prometheus", "vs predator"]),
            ("terminator", vec!["genisys", "dark fate", "salvation", "rise of the machines"]),
            ("matrix", vec!["resurrections", "reloaded", "revolutions"]),
            ("star wars", vec!["rogue one", "solo", "mandalorian", "andor", "clone wars"]),
            ("jurassic", vec!["world", "dominion", "fallen kingdom"]),
            ("fast", vec!["furious", "hobbs", "shaw"]),
            ("mission impossible", vec!["dead reckoning", "fallout", "rogue nation", "ghost protocol"]),
            ("john wick", vec!["chapter 2", "chapter 3", "chapter 4", "parabellum"]),
            ("spider-man", vec!["homecoming", "far from home", "no way home", "into the verse"]),
            ("avengers", vec!["ultron", "infinity war", "endgame"]),
            ("batman", vec!["begins", "dark knight", "rises", "forever", "robin"]),
            ("transformers", vec!["revenge", "dark of the moon", "age of extinction", "last knight"]),
            ("pirates", vec!["dead man", "world's end", "stranger tides", "dead men"]),
            ("harry potter", vec!["chamber", "prisoner", "goblet", "phoenix", "prince", "hallows"]),
            ("lord of the rings", vec!["two towers", "return of the king", "fellowship"]),
            ("hobbit", vec!["unexpected", "desolation", "five armies"]),
            ("hunger games", vec!["catching fire", "mockingjay", "ballad"]),
            ("twilight", vec!["new moon", "eclipse", "breaking dawn"]),
            ("scarlet heart", vec!["ryeo", "thailand"]),
        ]
    })
}

pub fn extract_core_title(text: &str) -> String {
    static EXT_RE: OnceLock<Regex> = OnceLock::new();
    static SEP_RE: OnceLock<Regex> = OnceLock::new();
    static POSS_RE: OnceLock<Regex> = OnceLock::new();
    static PUNC_RE: OnceLock<Regex> = OnceLock::new();
    static TV_NUM_RE: OnceLock<Regex> = OnceLock::new();
    static DUAL_TITLE_RE: OnceLock<Regex> = OnceLock::new();
    
    let mut name = text.to_lowercase();
    
    // 1. Remove obvious quality/source/year noise first to isolate the titles
    name = FilenameParser::quality_pattern().replace_all(&name, " ").to_string();
    name = FilenameParser::year_pattern().replace_all(&name, " ").to_string();
    name = EXT_RE.get_or_init(|| Regex::new(r"(?i)\.(mkv|mp4|avi|m4v|wmv|flv|webm|ts|m2ts)$").unwrap())
        .replace_all(&name, "").to_string();
    
    // 2. Handle Dual Titles (e.g. "Doraemon và Đảo Kho Báu - Doraemon the Movie 2018")
    // If there's a clear Vietnamese - English separator, we might want to prioritize the English part for TMDB-style searches, 
    // or just keep both as words. 
    // Usually " - " or " | " or " -- " are separators.
    let dual_re = DUAL_TITLE_RE.get_or_init(|| Regex::new(r"\s+[-|]\s+").unwrap());
    if dual_re.is_match(&name) {
        // If it's a dual title, we keep it as one string for now but ensure separators are spaces
        name = dual_re.replace_all(&name, " ").to_string();
    }
    
    // 3. Normalized Possessives (Nobita's -> Nobitas)
    // We remove the quote but keep the 's' for now, we'll handle matching logic in _match_against_title
    name = POSS_RE.get_or_init(|| Regex::new(r"(?i)['’]s\b").unwrap())
        .replace_all(&name, "s ").to_string();
    
    // 4. Punctuation
    name = PUNC_RE.get_or_init(|| Regex::new(r#"[:'\"!?,;]"#).unwrap())
        .replace_all(&name, " ").to_string();
    
    // 5. Separators (Dots, underscores, dashes)
    name = SEP_RE.get_or_init(|| Regex::new(r"[._-]+").unwrap())
        .replace_all(&name, " ").to_string();

    // 6. Remove leading episode/chapter numbers
    name = TV_NUM_RE.get_or_init(|| Regex::new(r"(?i)^(?:tap|ep|episode|chapter)?\s?\d{1,3}[._\s-]").unwrap())
        .replace(&name, " ").to_string();
    
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn get_title_keywords(title: &str) -> HashSet<String> {
    let core = extract_core_title(title);
    
    // V2 Stop Words
    let stop_words: HashSet<&str> = [
        "the", "a", "an", "and", "or", "but", "in", "on", "at", 
        "to", "for", "of", "with", "by", "from", "as", "is", "was",
        "are", "were", "be", "been", "being", "have", "has", "had",
        "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x",
        "va" // Vietnamese "and"
    ].iter().cloned().collect();
    
    core.split_whitespace()
        .filter(|w| !stop_words.contains(w) && w.len() > 1)
        .map(|w| w.to_string())
        .collect()
}

pub fn is_different_franchise_entry(search_title: &str, filename: &str) -> bool {
    let search_lower = search_title.to_lowercase();
    let file_lower = filename.to_lowercase();
    
    for (franchise, conflicts) in get_franchise_conflicts() {
        if search_lower.contains(franchise) {
            for conflict in conflicts {
                if file_lower.contains(conflict) && !search_lower.contains(conflict) {
                    return true;
                }
            }
        }
    }
    false
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SimilarityResult {
    pub score: f32,
    pub is_valid: bool,
    pub match_type: String,
}

pub fn calculate_unified_similarity(
    search_title: &str, 
    filename: &str,
    aliases: &[String]
) -> SimilarityResult {
    let result = _match_against_title(search_title, filename);
    if result.is_valid { return result; }
    
    for alias in aliases {
        let mut alias_result = _match_against_title(alias, filename);
        if alias_result.is_valid {
            alias_result.match_type = "alias".to_string();
            alias_result.score = 1.0; 
            return alias_result;
        }
    }
    result
}

fn _match_against_title(search_title: &str, filename: &str) -> SimilarityResult {
    if is_different_franchise_entry(search_title, filename) {
        return SimilarityResult { score: 0.0, is_valid: false, match_type: "franchise_conflict".to_string() };
    }

    let search_core = extract_core_title(search_title);
    let file_core = extract_core_title(filename);
    let file_words = get_title_keywords(filename);
    
    let n_search_core = normalize_vietnamese(&search_core);
    let n_file_core = normalize_vietnamese(&file_core);

    if n_search_core == n_file_core && !n_search_core.is_empty() {
        return SimilarityResult { score: 1.0, is_valid: true, match_type: "exact".to_string() };
    }
    
    let search_words = get_title_keywords(search_title);
    if search_words.is_empty() {
        return SimilarityResult { score: 0.1, is_valid: false, match_type: "no_keywords".to_string() };
    }
    
    let n_file_words: HashSet<String> = file_words.iter().map(|w| normalize_vietnamese(w)).collect();
    
    let mut missing_words = Vec::new();
    let mut common_count = 0;

    for sw in &search_words {
        let n_sw = normalize_vietnamese(sw);
        // Primary check
        if n_file_words.contains(&n_sw) || n_file_core.contains(&n_sw) {
            common_count += 1;
        } else {
            // Possessive Smart Check: "nobita" should match "nobitas" in file, and "nobitas" should match "nobita"
            let sw_s = format!("{}s", n_sw);
            let mut found_possessive = false;
            
            if n_file_words.contains(&sw_s) || n_file_core.contains(&sw_s) {
                found_possessive = true;
            } else if n_sw.len() > 3 && n_sw.ends_with('s') {
                let sw_singular = &n_sw[..n_sw.len()-1];
                if n_file_words.contains(sw_singular) || n_file_core.contains(sw_singular) {
                    found_possessive = true;
                }
            }

            if found_possessive {
                common_count += 1;
            } else {
                missing_words.push(sw);
            }
        }
    }
    
    let match_ratio = common_count as f32 / search_words.len() as f32;
    
    // Dual Title Handling: If the file is much longer than the search query (likely Vietnamese - English dual name)
    // we allow a partial match if the core search terms ARE present.
    if !missing_words.is_empty() {
        // Tightened threshold for dual titles to avoid "wild" results
        if match_ratio >= 0.7 && common_count >= 2 {
            return SimilarityResult { 
                score: match_ratio * 0.85, 
                is_valid: true, 
                match_type: "keyword_overlap".to_string() 
            };
        }
        return SimilarityResult { score: match_ratio * 0.5, is_valid: false, match_type: "missing_keywords".to_string() };
    }
    
    let extra_words = if file_words.len() > common_count { file_words.len() - common_count } else { 0 };
    
    // Improved Scoring: If we found ALL search keywords, the score should be high 
    // even if there are extra words (which are often taglines or quality info)
    let score = if extra_words == 0 { 
        0.95 
    } else {
        // Much gentler decay: 0.02 per extra word (was 0.05)
        // e.g., 8 extra words = 0.95 - (8 * 0.02) = 0.79 (valid)
        // Floor at 0.75 instead of 0.60 for complete keyword matches
        (0.95 - (extra_words as f32 * 0.02)).max(0.75)
    };
    
    SimilarityResult { score, is_valid: true, match_type: "all_keywords".to_string() }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SmartSearchResult {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub score: i32,
    pub quality_name: String,
    pub quality_score: i32,
    pub custom_format_score: i32,
    pub total_score: i32,
    pub normalized_score: f32,
    pub match_type: String,
    pub quality_attrs: crate::utils::parser::QualityAttributes,
    pub tmdb_id: Option<u64>,
    pub poster_path: Option<String>,
    pub vietdub: bool,
    pub vietsub: bool,
    pub hdr: bool,
    pub dolby_vision: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QualityGroup {
    pub quality: String,
    pub score: i32,
    pub count: usize,
    pub files: Vec<SmartSearchResult>,
}

pub fn group_by_quality(results: Vec<SmartSearchResult>) -> Vec<QualityGroup> {
    let mut groups: HashMap<String, Vec<SmartSearchResult>> = HashMap::new();
    
    for r in results {
        groups.entry(r.quality_name.clone()).or_default().push(r);
    }
    
    let mut sorted_groups = Vec::new();
    for (qname, mut items) in groups {
        items.sort_by(|a, b| b.total_score.cmp(&a.total_score));
        let max_score = items[0].total_score;
        sorted_groups.push(QualityGroup {
            quality: qname,
            score: max_score,
            count: items.len(),
            files: items,
        });
    }
    
    sorted_groups.sort_by(|a, b| b.score.cmp(&a.score));
    sorted_groups
}

pub fn is_vietnamese_title(text: &str) -> bool {
    // Unicode literals to avoid editor/tooling character corruption
    let vietnamese_specific = "\u{0103}\u{00E2}\u{0111}\u{00EA}\u{00F4}\u{01A1}\u{01B0}\u{1EB3}\u{1EB5}\u{1EB7}\u{1EA9}\u{1EAB}\u{1EAD}\u{1EC3}\u{1EC5}\u{1EC7}\u{1EC9}\u{1ECB}\u{1EED}\u{1EF1}\u{1ECC}\u{1ED5}\u{1ED7}\u{1ED9}\u{1EDF}\u{1EE1}\u{1EE3}\u{1EE7}\u{1EE9}\u{1EE5}\u{1EED}\u{1EEF}\u{1EF1}";
    text.to_lowercase().chars().any(|c| vietnamese_specific.contains(c))
}

pub fn normalize_vietnamese(text: &str) -> String {
    let mut result = text.to_lowercase();
    let mappings = [
        ("\u{00E0}\u{00E1}\u{1EA3}\u{00E3}\u{1EA1}\u{0103}\u{1EB1}\u{1EB3}\u{1EB5}\u{1EB7}\u{00E2}\u{1EA7}\u{1EA5}\u{1EA9}\u{1EAB}\u{1EAD}", 'a'),
        ("\u{00E8}\u{00E9}\u{1EBB}\u{1EBD}\u{1EB9}\u{00EA}\u{1EC1}\u{1EBF}\u{1EC3}\u{1EC5}\u{1EC7}", 'e'),
        ("\u{00EC}\u{00ED}\u{1EC9}\u{0129}\u{1ECB}", 'i'),
        ("\u{00F2}\u{00F3}\u{1ECF}\u{00F5}\u{1ECD}\u{00F4}\u{1ED3}\u{1ED5}\u{1ED7}\u{1ED9}\u{01A1}\u{1EDD}\u{1EDB}\u{1EDF}\u{1EE1}\u{1EE3}", 'o'),
        ("\u{00F9}\u{00FA}\u{1EE7}\u{0169}\u{1EE5}\u{01B0}\u{1EEB}\u{1EED}\u{1EEF}\u{1EF1}", 'u'),
        ("\u{1EF3}\u{00FD}\u{1EF7}\u{1EF9}\u{1EF1}", 'y'),
        ("\u{0111}", 'd'),
    ];
    for (chars, replacement) in mappings {
        for c in chars.chars() {
            result = result.replace(c, &replacement.to_string());
        }
    }
    result
}

pub fn detect_badges(filename: &str) -> (bool, bool, bool, bool) {
    let lower = filename.to_lowercase();
    let vietdub = lower.contains("vietdub") || lower.contains("long tieng");
    let vietsub = (lower.contains("vietsub") || lower.contains("phu de")) && !vietdub;
    let hdr = lower.contains("hdr") || lower.contains("hdr10");
    let dolby_vision = lower.contains("dv") || lower.contains("dolby vision");
    (vietdub, vietsub, hdr, dolby_vision)
}
