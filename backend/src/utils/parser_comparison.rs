
#[cfg(test)]
mod tests {
    use crate::utils::parser::FilenameParser;
    use crate::utils::smart_tokenizer::{smart_parse, MediaType};

    fn get_mapping(title: &str, year: &str, season: &Option<u32>, episode: &Option<u32>) -> String {
        
        // Manual mapping for Doraemon movies based on Year
        if title.to_lowercase().contains("doraemon") && season.is_none() && episode.is_none() {
            match year {
                "1980" => return "Doraemon: Nobita's Dinosaur (1980)".to_string(),
                "1985" => return "Doraemon: Nobita's Little Star Wars (1985)".to_string(),
                "1986" => return "Doraemon: Nobita and the Steel Troops (1986)".to_string(),
                "1990" => return "Doraemon: Nobita and the Animal Planet (1990)".to_string(),
                "1991" => return "Doraemon: Nobita's Dorabian Nights (1991)".to_string(),
                "2016" => return "Doraemon: Nobita and the Birth of Japan (2016)".to_string(),
                "2023" => return "Doraemon: Nobita's Sky Utopia (2023)".to_string(),
                "2024" => return "Doraemon: Nobita's Earth Symphony (2024)".to_string(),
                "2025" => return "Doraemon: Nobita's Art World Tales (2025)".to_string(),
                _ => {}
            }
        }
        
        // Series Mapping
        let s_str = season.map(|n| format!("S{:02}", n)).unwrap_or("S??".to_string());
        let e_str = episode.map(|n| format!("E{:02}", n)).unwrap_or("E??".to_string());
        
        if !year.is_empty() {
            format!("{} ({}) {} {}", title, year, s_str, e_str)
        } else {
            format!("{} {} {}", title, s_str, e_str)
        }
    }

    #[test]
    fn compare_parsers() {
        // Load dataset from JSON
        use std::fs::File;
        use std::io::BufReader;
        
        #[derive(serde::Deserialize)]
        struct DatasetItem {
            name: String,
        }

        #[derive(serde::Deserialize)]
        struct Dataset {
            data: Vec<DatasetItem>,
        }

        let file_path = "/Users/blavkbeav/Documents/Workspace/Flasharr/Flasharr/debug_log/dataset_1.json";
        let file = File::open(file_path).expect("Failed to open dataset file");
        let reader = BufReader::new(file);
        let dataset: Dataset = serde_json::from_reader(reader).expect("Failed to parse JSON");
        
        // Extract filenames
        let samples: Vec<String> = dataset.data.into_iter().map(|item| item.name).collect();
        // Use references for the loop
        let samples: Vec<&str> = samples.iter().map(|s| s.as_str()).collect();

        println!("Loaded {} samples from {}", samples.len(), file_path);

        use std::io::Write;
        let mut file = std::fs::File::create("comparison_report.txt").unwrap();
        
        let mut old_valid_title = 0;
        let mut new_valid_title = 0;
        let mut old_identified_movies = 0;
        let mut new_identified_movies = 0;
        let mut old_ep_found = 0;
        let mut new_ep_found = 0;
        let mut total_items = 0;
        
        writeln!(file, "{:<100} | {:<60} | {:<60} | {}", 
            "Filename", "Old (FilenameParser)", "New (SmartTokenizer)", "Verdict").unwrap();
        writeln!(file, "{}", "-".repeat(240)).unwrap();

        for filename in samples {
            total_items += 1;
            
            // Old Parser
            let old = FilenameParser::parse(filename);
            let old_map = format!("Title: {}, Year: {:?}, S: {:?}, E: {:?}", 
                old.title, old.year, old.season, old.episode);

            // New Parser
            let new = smart_parse(filename);
            let new_map = format!("Title: {}, Year: {:?}, S: {:?}, E: {:?} [Type: {:?}, Conf: {:.2}]",
                new.title, new.year, new.season, new.episode, new.media_type, new.confidence);

            // Simple diff check
            let verdict = if old.title == new.title && old.year == new.year && old.season == new.season && old.episode == new.episode {
                "SAME"
            } else {
                "DIFFERENT"
            };

            writeln!(file, "{:<100} | {:<60} | {:<60} | {}", 
                filename, old_map, new_map, verdict).unwrap();
                
            // Stats counting
            if !old.title.trim().is_empty() { old_valid_title += 1; }
            if !new.title.trim().is_empty() { new_valid_title += 1; }
            
            // Movie identification Logic
            let old_is_movie = old.episode.is_none() && old.season.is_none() && (old.year.is_some() || old.title.to_lowercase().contains("movie"));
            if old_is_movie { old_identified_movies += 1; }

            if new.media_type == MediaType::Movie { new_identified_movies += 1; }

            if old.episode.is_some() { old_ep_found += 1; }
            if new.episode.is_some() { new_ep_found += 1; }
        }
        
        writeln!(file, "{}", "-".repeat(240)).unwrap();
        writeln!(file, "STATISTICS (N={})", total_items).unwrap();
        
        writeln!(file, "\nTable 1: Filename Parsing Success (Filter Pass)").unwrap();
        writeln!(file, "{:<25} | {:<10} | {:<10}", "Method", "Passed", "Failed").unwrap();
        writeln!(file, "{:<25} | {:<10} | {:<10}", "Old (FilenameParser)", old_valid_title, total_items - old_valid_title).unwrap();
        writeln!(file, "{:<25} | {:<10} | {:<10}", "New (SmartTok)", new_valid_title, total_items - new_valid_title).unwrap();

        writeln!(file, "\nTable 2: TMDB Movie Identification").unwrap();
        writeln!(file, "{:<25} | {:<15}", "Method", "Movies Found").unwrap();
        writeln!(file, "{:<25} | {:<15}", "Old (Inferred)", old_identified_movies).unwrap();
        writeln!(file, "{:<25} | {:<15}", "New (Explicit)", new_identified_movies).unwrap();
        
        writeln!(file, "\nEpisodes Found: Old={}, New={}", old_ep_found, new_ep_found).unwrap();
        println!("Report generated at backend/comparison_report.txt");
    }
}
