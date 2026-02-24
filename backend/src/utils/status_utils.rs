//! Status Utilities
//!
//! Reusable functions for status counting and aggregation.

use std::collections::HashMap;
use serde::Serialize;

/// Status counts for UI filter dropdowns
#[derive(Debug, Clone, Serialize, Default)]
pub struct StatusCounts {
    pub downloading: usize,
    pub queued: usize,
    pub paused: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub all: usize,
}

impl StatusCounts {
    /// Build StatusCounts from a HashMap of state -> count
    /// Handles state grouping (e.g., STARTING -> downloading, WAITING -> queued)
    pub fn from_db_counts(counts: HashMap<String, usize>) -> Self {
        let downloading = *counts.get("DOWNLOADING").unwrap_or(&0) 
            + *counts.get("STARTING").unwrap_or(&0);
        let queued = *counts.get("QUEUED").unwrap_or(&0) 
            + *counts.get("WAITING").unwrap_or(&0);
        let paused = *counts.get("PAUSED").unwrap_or(&0);
        let completed = *counts.get("COMPLETED").unwrap_or(&0);
        let failed = *counts.get("FAILED").unwrap_or(&0);
        let cancelled = *counts.get("CANCELLED").unwrap_or(&0);
        let all = counts.values().sum();
        
        Self {
            downloading,
            queued,
            paused,
            completed,
            failed,
            cancelled,
            all,
        }
    }
}

/// Normalize status filter strings for database queries
/// Maps UI-friendly names to database state values
#[allow(dead_code)]
pub fn normalize_status_filter(status: &str) -> Vec<&'static str> {
    match status.to_uppercase().as_str() {
        "DOWNLOADING" => vec!["DOWNLOADING", "STARTING"],
        "QUEUED" => vec!["QUEUED", "WAITING"],
        "PAUSED" => vec!["PAUSED"],
        "COMPLETED" => vec!["COMPLETED"],
        "FAILED" => vec!["FAILED"],
        "CANCELLED" => vec!["CANCELLED"],
        _ => vec![],
    }
}

/// Check if a state string represents an "active" download
#[allow(dead_code)]
pub fn is_active_state(state: &str) -> bool {
    matches!(state.to_uppercase().as_str(), 
        "DOWNLOADING" | "STARTING" | "QUEUED" | "WAITING")
}

/// Check if a state string represents a "pauseable" download
#[allow(dead_code)]
pub fn is_pauseable_state(state: &str) -> bool {
    matches!(state.to_uppercase().as_str(), 
        "DOWNLOADING" | "STARTING" | "QUEUED" | "WAITING")
}

/// Check if a state string represents a "resumable" download
#[allow(dead_code)]
pub fn is_resumable_state(state: &str) -> bool {
    matches!(state.to_uppercase().as_str(), "PAUSED")
}

/// Check if a state string represents a "retryable" download
#[allow(dead_code)]
pub fn is_retryable_state(state: &str) -> bool {
    matches!(state.to_uppercase().as_str(), "FAILED" | "CANCELLED")
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_status_counts_from_db() {
        let mut counts = HashMap::new();
        counts.insert("DOWNLOADING".to_string(), 2);
        counts.insert("STARTING".to_string(), 1);
        counts.insert("QUEUED".to_string(), 5);
        counts.insert("WAITING".to_string(), 3);
        counts.insert("COMPLETED".to_string(), 10);
        
        let status = StatusCounts::from_db_counts(counts);
        
        assert_eq!(status.downloading, 3); // 2 + 1
        assert_eq!(status.queued, 8); // 5 + 3
        assert_eq!(status.completed, 10);
        assert_eq!(status.all, 21);
    }
    
    #[test]
    fn test_normalize_status_filter() {
        assert_eq!(normalize_status_filter("downloading"), vec!["DOWNLOADING", "STARTING"]);
        assert_eq!(normalize_status_filter("QUEUED"), vec!["QUEUED", "WAITING"]);
        assert_eq!(normalize_status_filter("unknown"), Vec::<&str>::new());
    }
    
    #[test]
    fn test_state_checks() {
        assert!(is_active_state("DOWNLOADING"));
        assert!(is_active_state("starting"));
        assert!(!is_active_state("COMPLETED"));
        
        assert!(is_pauseable_state("QUEUED"));
        assert!(!is_pauseable_state("PAUSED"));
        
        assert!(is_resumable_state("PAUSED"));
        assert!(!is_resumable_state("DOWNLOADING"));
        
        assert!(is_retryable_state("FAILED"));
        assert!(!is_retryable_state("COMPLETED"));
    }
}
