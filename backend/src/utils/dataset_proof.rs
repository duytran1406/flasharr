#[cfg(test)]
mod tests {
    use crate::utils::parser::FilenameParser;
    use crate::utils::smart_tokenizer::{smart_parse, MediaType};
    use std::collections::HashMap;

    /// Proof 1: Manual Search Quality (Dataset 1 - Doraemon Movies)
    /// 
    /// Manual search requires CLEAN titles for TMDB matching.
    /// Dirty titles (with codecs, release groups, etc.) fail to match.
    #[test]
    fn proof_manual_search_quality_dataset1() {
        // Sample problematic filenames from dataset_1.json
        let test_cases = vec![
            // Case 1: Audio codec pollution
            ("2010.Doraemon.The.Great.Battle.of.the.Mermaid.King.Bluray.VIE.1080p.AVC.DTS.5.1.mkv",
             "Doraemon The Great Battle of the Mermaid King", // Expected clean title
             "Doraemon The Great Battle of the Mermaid King 5"), // Old method includes "5" from "5.1"
            
            // Case 2: Bracket handling for release groups
            ("[J-Zone].Doraemon.Specials.The.Night.Before.A.Wedding.KITES.VN.mkv",
             "Doraemon Specials The Night Before A Wedding",
             "Doraemon Specials The Night Before A Wedding KITES VN"), // Old keeps junk
            
            // Case 3: File extension pollution
            ("plaza-doraemon.story.of.seasons.iso",
             "plaza doraemon story of seasons",
             "plaza-doraemon story of seasons iso"), // Old keeps extension
            
            // Case 4: Year-first with codec confusion
            ("2014.Doraemon.Stand.by.Me.Doraemon.Bluray.VIE.1080p.AVC.TrueHD.5.1.mkv",
             "Doraemon Stand by Me Doraemon",
             "Doraemon Stand by Me Doraemon 5"), // Old mistakes audio for title
        ];

        let mut old_clean_count = 0;
        let mut new_clean_count = 0;

        for (filename, expected_clean, expected_dirty) in test_cases {
            let old = FilenameParser::parse(filename);
            let new = smart_parse(filename);

            println!("\n📁 {}", filename);
            println!("  Old: '{}'", old.title);
            println!("  New: '{}'", new.title);
            println!("  Expected Clean: '{}'", expected_clean);

            // Check if new method produces clean title
            if new.title.trim() == expected_clean {
                new_clean_count += 1;
                println!("  ✅ NEW produces clean title for TMDB matching");
            }

            // Check if old method produces dirty title
            if old.title.contains("5.1") || old.title.contains("iso") || 
               old.title.contains("VN") || old.title.contains("DD") {
                println!("  ❌ OLD contains metadata junk: '{}'", old.title);
            } else {
                old_clean_count += 1;
            }
        }

        println!("\n📊 Manual Search Quality Results:");
        println!("   Old Method Clean Titles: {}/4", old_clean_count);
        println!("   New Method Clean Titles: {}/4", new_clean_count);
        
        assert!(new_clean_count > old_clean_count, 
                "New method should produce more clean titles for manual TMDB search");
    }

    /// Proof 2: Smart TV Series Search (Dataset 2 - Scarlet Heart)
    /// 
    /// Smart search for TV series requires:
    /// 1. Correct title extraction (same across all episodes)
    /// 2. Accurate episode detection (S01E01, _01_, etc.)
    /// 3. Handling multiple naming conventions
    #[test]
    fn proof_smart_series_search_dataset2() {
        // Real filenames from dataset_2.json showing different conventions
        let test_cases = vec![
            // Convention 1: Standard S01E## format
            ("Bộ Bộ Kinh Tâm - S01E17 - CH Bo Bo Kinh Tam 17.mkv", 1, 17),
            ("Bộ Bộ Kinh Tâm - S01E01 - CH Bo Bo Kinh Tam 01.mkv", 1, 1),
            
            // Convention 2: Underscore with episode number
            ("Bo Bo Kinh Tam_33_720P.mkv", 1, 33),
            ("Bo Bo Kinh Tam_01_720P.mkv", 1, 1),
            ("Bo Bo Kinh Tam_40End_720P.mkv", 1, 40),
            
            // Convention 3: Bracket with episode number
            ("[Phim Media] Bo Bo Kinh Tam 01.mkv", 1, 1),
            ("[Phim Media] Bo Bo Kinh Tam 35.mkv", 1, 35),
            
            // Convention 4: Leading episode number
            ("05_Bo Bo kinh Tam_4K_Long tieng.mp4", 1, 5),
            ("22_Bo Bo kinh Tam_4K_Long tieng.mp4", 1, 22),
        ];

        let mut old_detected = 0;
        let mut new_detected = 0;
        let mut title_consistency_old = HashMap::new();
        let mut title_consistency_new = HashMap::new();

        for (filename, expected_season, expected_episode) in &test_cases {
            let old = FilenameParser::parse(filename);
            let new = smart_parse(filename);

            println!("\n📺 {}", filename);
            
            // Check episode detection
            let old_ep_match = old.episode == Some(*expected_episode);
            let new_ep_match = new.episode == Some(*expected_episode);

            if old_ep_match {
                old_detected += 1;
                println!("  Old: ✅ S{:?}E{:?}", old.season, old.episode);
            } else {
                println!("  Old: ❌ S{:?}E{:?} (expected E{})", old.season, old.episode, expected_episode);
            }

            if new_ep_match {
                new_detected += 1;
                println!("  New: ✅ S{:?}E{:?}", new.season, new.episode);
            } else {
                println!("  New: ❌ S{:?}E{:?} (expected E{})", new.season, new.episode, expected_episode);
            }

            // Track title consistency for series grouping
            *title_consistency_old.entry(old.title.clone()).or_insert(0) += 1;
            *title_consistency_new.entry(new.title.clone()).or_insert(0) += 1;
        }

        println!("\n📊 Smart Series Search Results:");
        println!("   Episodes Detected:");
        println!("     Old Method: {}/{}", old_detected, test_cases.len());
        println!("     New Method: {}/{}", new_detected, test_cases.len());
        
        println!("\n   Title Consistency (for series grouping):");
        println!("     Old Method unique titles: {}", title_consistency_old.len());
        for (title, count) in &title_consistency_old {
            println!("       '{}': {} episodes", title, count);
        }
        
        println!("     New Method unique titles: {}", title_consistency_new.len());
        for (title, count) in &title_consistency_new {
            println!("       '{}': {} episodes", title, count);
        }

        // Assertions
        assert!(new_detected > old_detected, 
                "New method should detect more episodes across different naming conventions");
        
        assert!(title_consistency_new.len() <= title_consistency_old.len(),
                "New method should produce more consistent titles for series grouping");
    }

    /// Proof 3: Vietnamese Episode Pattern Support
    #[test]
    fn proof_vietnamese_pattern_support() {
        let vietnamese_patterns = vec![
            ("Doraemon TVRip MPEG2_Tap01.ts", 1),  // "Tap" = Episode in Vietnamese
            ("Doraemon TVRip MPEG2_Tap31.ts", 31),
            ("Doraemonドラえもん(1979)／Vol.019 「(DVDRip).mkv", 19), // Volume notation
            ("[Ep 164] Tạm biệt Doraemon.mp4", 164), // Bracket episode
        ];

        let mut old_success = 0;
        let mut new_success = 0;

        println!("\n🇻🇳 Vietnamese Episode Pattern Detection:");
        
        for (filename, expected_ep) in vietnamese_patterns {
            let old = FilenameParser::parse(filename);
            let new = smart_parse(filename);

            println!("\n  {}", filename);
            println!("    Expected: E{}", expected_ep);
            println!("    Old: {:?}", old.episode);
            println!("    New: {:?}", new.episode);

            if old.episode == Some(expected_ep) {
                old_success += 1;
                println!("    Old: ✅");
            } else {
                println!("    Old: ❌ (Failed to detect Vietnamese pattern)");
            }

            if new.episode == Some(expected_ep) {
                new_success += 1;
                println!("    New: ✅");
            } else {
                println!("    New: ❌");
            }
        }

        println!("\n📊 Vietnamese Pattern Support:");
        println!("   Old Method: {}/4", old_success);
        println!("   New Method: {}/4", new_success);

        assert_eq!(new_success, 4, "New method should detect all Vietnamese patterns");
        assert!(old_success < new_success, "Old method lacks Vietnamese pattern support");
    }
}
