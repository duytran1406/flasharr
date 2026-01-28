use crate::utils::title_matcher::{calculate_unified_similarity, normalize_vietnamese};

/// Unified scoring function that considers title similarity, year match, and aliases
/// with media-type-aware year weighting
pub fn calculate_match_score(
    search_title: &str,
    filename: &str,
    file_year: Option<u32>,
    target_year: Option<u32>,
    aliases: &[String],
    is_tv_series: bool,
) -> f32 {
    let mut score = 0.0;
    
    // Phase 1: Title Similarity (70% weight for movies, 85% for TV)
    // Increased from 60%/80% because title match is more reliable than year
    let sim_result = calculate_unified_similarity(search_title, filename, aliases);
    let title_weight = if is_tv_series { 0.85 } else { 0.70 };
    score += sim_result.score * title_weight;
    
    // Phase 2: Year Match (20% weight for movies, 5% for TV)
    // Reduced from 30%/10% to be less strict on year mismatches
    // TV series use the show's first air date, so year matching is less reliable
    if !is_tv_series {
        if let (Some(fy), Some(ty)) = (file_year, target_year) {
            let year_diff = (fy as i32 - ty as i32).abs();
            score += match year_diff {
                0 => 0.20,      // Perfect match
                1 => 0.15,      // Off by one (common for late releases)
                2 => 0.08,      // Tolerable
                _ => 0.0,       // Too far off
            };
        }
    } else {
        // For TV: only give a small bonus if years match, but don't penalize mismatches
        if let (Some(fy), Some(ty)) = (file_year, target_year) {
            if fy == ty {
                score += 0.05;
            }
        }
    }
    
    // Phase 3: Alias Boost (10% weight)
    let filename_lower = filename.to_lowercase();
    let has_alias_match = aliases.iter().any(|alias| {
        let alias_norm = normalize_vietnamese(alias);
        filename_lower.contains(&alias_norm) || filename_lower.contains(&alias.to_lowercase())
    });
    
    if has_alias_match {
        score += 0.10;
    }
    
    score.min(1.0) // Cap at 1.0
}

/// Determines if a match score is valid (above threshold)
pub fn is_valid_match(score: f32, is_tv_series: bool) -> bool {
    // Lowered thresholds based on test results
    // Movies: 0.65 (was 0.70) - allows year±1 mismatches
    // TV: 0.55 (was 0.65) - very lenient for multi-season shows
    let threshold = if is_tv_series { 0.55 } else { 0.65 };
    score >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_movie_year_match() {
        let score = calculate_match_score(
            "Five Nights at Freddy's",
            "Five.Nights.at.Freddys.2025.1080p.mkv",
            Some(2025),
            Some(2025),
            &[],
            false,
        );
        assert!(score > 0.85, "Perfect year match should score high");
    }

    #[test]
    fn test_movie_year_mismatch() {
        let score = calculate_match_score(
            "Five Nights at Freddy's",
            "Five.Nights.at.Freddys.2019.1080p.mkv",
            Some(2019),
            Some(2025),
            &[],
            false,
        );
        // With 6-year mismatch, year score is 0, but title similarity is ~0.70
        // Total: 0.70 * 0.70 (title weight) = ~0.49
        assert!(score < 0.90, "Large year mismatch should not score as high as perfect match");
        assert!(score > 0.40, "But title similarity should still contribute significantly");
    }

    #[test]
    fn test_tv_series_year_tolerance() {
        // TV show from 2011, but searching for Season 5 from 2015
        let score = calculate_match_score(
            "Scarlet Heart",
            "Scarlet.Heart.S05E01.2015.1080p.mkv",
            Some(2015),
            Some(2011), // Show's first air date
            &[],
            true,
        );
        assert!(score > 0.65, "TV series should not be penalized for year mismatch");
    }

    #[test]
    fn test_alias_boost() {
        let score = calculate_match_score(
            "Scarlet Heart",
            "Bộ.Bộ.Kinh.Tâm.2011.1080p.mkv",
            Some(2011),
            Some(2011),
            &["Bộ Bộ Kinh Tâm".to_string()],
            true,
        );
        assert!(score > 0.80, "Alias match should provide significant boost");
    }
}
