use crate::utils::unified_scorer::{calculate_match_score, is_valid_match};
use crate::utils::title_matcher::calculate_unified_similarity;

/// Test cases based on real-world successes and failures
#[cfg(test)]
mod search_scoring_tests {
    use super::*;

    // ============================================================================
    // SUCCESS CASES - These should score HIGH
    // ============================================================================

    #[test]
    fn test_success_scarlet_heart_vietnamese() {
        // SUCCESS: Bộ Bộ Kinh Tâm matched to Scarlet Heart (2011)
        let score = calculate_match_score(
            "Scarlet Heart",
            "Bộ.Bộ.Kinh.Tâm.2011.S01E01.1080p.mkv",
            Some(2011),
            Some(2011),
            &["Bộ Bộ Kinh Tâm".to_string()],
            true, // TV series
        );
        println!("Scarlet Heart (VN alias) score: {:.3}", score);
        assert!(score >= 0.80, "Vietnamese alias match should score high: got {}", score);
    }

    #[test]
    fn test_success_doraemon_collection() {
        // SUCCESS: Doraemon franchise matching
        let score = calculate_match_score(
            "Doraemon: Nobita's New Dinosaur",
            "Doraemon.Nobitas.New.Dinosaur.2020.1080p.BluRay.mkv",
            Some(2020),
            Some(2020),
            &[],
            false, // Movie
        );
        println!("Doraemon collection score: {:.3}", score);
        assert!(score >= 0.85, "Exact title+year match should score very high: got {}", score);
    }

    // ============================================================================
    // FAILURE CASES - These were returning wrong TMDB IDs
    // ============================================================================

    #[test]
    fn test_failure_bride_of_covenant_year_mismatch() {
        // FAILURE: Bride.of.the.Covenant.2025 should match TMDB 2024/2025
        // File has 2025, TMDB might list 2024
        let score_2024 = calculate_match_score(
            "Bride of the Covenant",
            "Bride.of.the.Covenant.2025.1080p.WEB-DL.AAC2.0.H.264-HETTIEN.mkv",
            Some(2025),
            Some(2024), // TMDB year
            &[],
            false,
        );
        println!("Bride of Covenant (year off by 1) score: {:.3}", score_2024);
        assert!(score_2024 >= 0.75, "Off-by-one year should still be valid: got {}", score_2024);
    }

    #[test]
    fn test_failure_gia_su_nu_quai_spaces() {
        // FAILURE: "Gia Sư Nữ Quái 2012" - Vietnamese with spaces
        let score = calculate_match_score(
            "Gia Sư Nữ Quái",
            "Gia Sư Nữ Quái 2012 1080p WEB-DL H.264 AAC2.0-3cTWeB.mkv",
            Some(2012),
            Some(2012),
            &[],
            false,
        );
        println!("Gia Sư Nữ Quái score: {:.3}", score);
        assert!(score >= 0.80, "Exact Vietnamese title match should score high: got {}", score);
    }

    #[test]
    fn test_failure_avatar_fire_and_ash_webscreener() {
        // FAILURE: Avatar.Fire.and.Ash.2025.WEBSCREENER
        let score = calculate_match_score(
            "Avatar Fire and Ash",
            "Avatar.Fire.and.Ash.2025.2160p.WEBSCREENER.H.265.Dual YG (Vietsub).mkv",
            Some(2025),
            Some(2025),
            &[],
            false,
        );
        println!("Avatar Fire and Ash score: {:.3}", score);
        assert!(score >= 0.80, "Exact match with WEBSCREENER should score high: got {}", score);
    }

    #[test]
    fn test_failure_lam_giau_voi_ma_long_subtitle() {
        // FAILURE: "Làm Giàu Với Ma - Cuộc Chiến Hột Xoàn" - Long Vietnamese title
        let score = calculate_match_score(
            "Làm Giàu Với Ma",
            "Làm.Giàu.Với.Ma.-.Cuộc.Chiến.Hột.Xoàn.2025.1080p.WEB-DL.DDP5.1.H.264-HBO.mkv",
            Some(2025),
            Some(2025),
            &[],
            false,
        );
        println!("Làm Giàu Với Ma (with subtitle) score: {:.3}", score);
        assert!(score >= 0.75, "Title with subtitle should still match: got {}", score);
    }

    #[test]
    fn test_failure_fnaf_sequel_confusion() {
        // FAILURE: Five Nights at Freddy's 2025 was matching 2019 fan film
        
        // Correct match: 2025 movie
        let score_2025 = calculate_match_score(
            "Five Nights at Freddy's",
            "Five.Nights.at.Freddys.2025.1080p.WEB-DL.mkv",
            Some(2025),
            Some(2025),
            &[],
            false,
        );
        
        // Wrong match: 2019 short film "The Interview"
        let score_2019 = calculate_match_score(
            "Five Nights at Freddy's",
            "Five.Nights.at.Freddys.The.Interview.2019.720p.mkv",
            Some(2019),
            Some(2025), // Searching for 2025
            &[],
            false,
        );
        
        println!("FNAF 2025 (correct) score: {:.3}", score_2025);
        println!("FNAF 2019 (wrong) score: {:.3}", score_2019);
        
        assert!(score_2025 > score_2019, 
            "2025 version should score higher than 2019: {:.3} vs {:.3}", 
            score_2025, score_2019);
        assert!(score_2019 < 0.70, 
            "6-year mismatch should be invalid: got {:.3}", score_2019);
    }

    // ============================================================================
    // EDGE CASES
    // ============================================================================

    #[test]
    fn test_tv_series_year_tolerance() {
        // TV show from 2011, searching for Season 5 from 2015
        let score = calculate_match_score(
            "Game of Thrones",
            "Game.of.Thrones.S05E01.2015.1080p.mkv",
            Some(2015),
            Some(2011), // Show premiered in 2011
            &[],
            true,
        );
        println!("TV series year tolerance score: {:.3}", score);
        assert!(score >= 0.65, "TV series should tolerate year mismatch: got {}", score);
    }

    #[test]
    fn test_district_9_numeric_title() {
        // Numeric title preservation
        let score = calculate_match_score(
            "District 9",
            "District.9.2009.1080p.Bluray.x264.mkv",
            Some(2009),
            Some(2009),
            &[],
            false,
        );
        println!("District 9 (numeric title) score: {:.3}", score);
        assert!(score >= 0.90, "Exact numeric title match should score very high: got {}", score);
    }

    // ============================================================================
    // COMPARATIVE TESTS - Different Scoring Approaches
    // ============================================================================

    #[test]
    fn test_approach_comparison_year_weight() {
        // Compare: High year weight (30%) vs Low year weight (10%)
        let filename = "Five.Nights.at.Freddys.2019.720p.mkv";
        
        // Approach A: Current (30% year weight for movies)
        let score_high_year = calculate_match_score(
            "Five Nights at Freddy's",
            filename,
            Some(2019),
            Some(2025),
            &[],
            false,
        );
        
        // Approach B: Low year weight (treat like TV)
        let score_low_year = calculate_match_score(
            "Five Nights at Freddy's",
            filename,
            Some(2019),
            Some(2025),
            &[],
            true, // Simulate low year weight
        );
        
        println!("High year weight (30%): {:.3}", score_high_year);
        println!("Low year weight (10%): {:.3}", score_low_year);
        
        // High year weight should penalize more
        assert!(score_high_year < score_low_year, 
            "High year weight should penalize year mismatch more");
    }

    #[test]
    fn test_old_vs_new_similarity_algorithm() {
        let filename = "Làm.Giàu.Với.Ma.-.Cuộc.Chiến.Hột.Xoàn.2025.1080p.mkv";
        
        // New unified approach
        let new_score = calculate_match_score(
            "Làm Giàu Với Ma",
            filename,
            Some(2025),
            Some(2025),
            &[],
            false,
        );
        
        // Old approach (for comparison)
        let old_sim = calculate_unified_similarity(
            "Làm Giàu Với Ma",
            filename,
            &[],
        );
        
        println!("New unified score: {:.3}", new_score);
        println!("Old similarity score: {:.3}", old_sim.score);
        
        // New approach should handle long titles better
        assert!(new_score >= 0.75, "New approach should handle subtitles: got {}", new_score);
    }
}
