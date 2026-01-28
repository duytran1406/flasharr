//! Download Engine Configuration
//!
//! Configuration struct for the download engine with sensible defaults.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Smart segment calculation constants
/// **DEPRECATED**: Multi-segment downloads are no longer used
#[allow(dead_code)]
pub const MIN_SEGMENT_SIZE: u64 = 50 * 1024 * 1024;   // 50 MB minimum per segment
#[allow(dead_code)]
pub const MAX_SEGMENT_SIZE: u64 = 200 * 1024 * 1024;  // 200 MB maximum per segment
#[allow(dead_code)]
pub const MIN_SEGMENTS: u32 = 1;
#[allow(dead_code)]
pub const MAX_SEGMENTS: u32 = 8;
#[allow(dead_code)]
pub const SMALL_FILE_THRESHOLD: u64 = 100 * 1024 * 1024; // Files < 100MB use single stream

/// Download engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    /// Maximum concurrent file downloads (default: 2)
    pub max_concurrent: usize,
    
    /// Default number of segments per download (default: 4)
    pub segments_per_download: usize,
    
    /// Chunk size for reading from network (default: 1MB)
    pub chunk_size: usize,
    
    /// Number of retry attempts for failed segments (default: 3)
    pub retry_attempts: u32,
    
    /// Base retry backoff in seconds (default: 30)
    pub retry_backoff_base: u64,
    
    /// Maximum retry wait time in seconds (default: 300)
    pub retry_max_wait: u64,
    
    /// Download directory path
    pub download_dir: PathBuf,
    
    /// Retry configuration for failed downloads
    pub retry: RetryConfig,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 2,
            segments_per_download: 4,
            chunk_size: 1024 * 1024, // 1 MB
            retry_attempts: 3,
            retry_backoff_base: 30,
            retry_max_wait: 300,
            download_dir: PathBuf::from("/downloads"),
            retry: RetryConfig::default(),
        }
    }
}

/// Retry configuration for failed downloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (default: 3)
    pub max_retries: u32,
    
    /// Base delay in seconds for exponential backoff (default: 2)
    pub base_delay_secs: u64,
    
    /// Maximum delay in seconds (default: 300 = 5 minutes)
    pub max_delay_secs: u64,
    
    /// Base delay in milliseconds (for compatibility)
    pub base_delay_ms: u32,
    
    /// Maximum delay in milliseconds (for compatibility)
    pub max_delay_ms: u32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_secs: 2,
            max_delay_secs: 300,
            base_delay_ms: 1000,  // 1 second
            max_delay_ms: 60000,  // 60 seconds
        }
    }
}

impl DownloadConfig {
    /// Create config with custom download directory
    pub fn with_download_dir(download_dir: PathBuf) -> Self {
        Self {
            download_dir,
            ..Default::default()
        }
    }
}

/// Calculate optimal segment count based on file size.
///
/// **DEPRECATED**: Multi-segment downloads are no longer used
#[allow(dead_code)]
/// The user's configured max (user_max) acts as the CEILING.
/// Smart calculation can return LESS than user's setting but NEVER MORE.
///
/// Assumes ~50 MB/s download speed per connection.
/// Target: Each segment should take 10-30 seconds to download.
pub fn calculate_optimal_segments(file_size: u64, user_max: u32) -> u32 {
    // Validate user_max
    let user_max = user_max.clamp(MIN_SEGMENTS, MAX_SEGMENTS);
    
    if file_size == 0 {
        return 1;
    }
    
    // Small files: single stream is more efficient
    if file_size < SMALL_FILE_THRESHOLD {
        return 1;
    }
    
    // Calculate based on target segment size
    let optimal = (file_size / MIN_SEGMENT_SIZE) as u32;
    
    // Clamp to reasonable range based on file size
    let optimal = if file_size < 500 * 1024 * 1024 {
        // < 500MB
        optimal.min(4)
    } else if file_size < 1024 * 1024 * 1024 {
        // < 1GB
        optimal.min(8)
    } else {
        optimal
    };
    
    // Never exceed user's configured maximum, ensure at least 1
    optimal.clamp(1, user_max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_optimal_segments_small_file() {
        // Small files should use single segment
        assert_eq!(calculate_optimal_segments(50 * 1024 * 1024, 8), 1);
        assert_eq!(calculate_optimal_segments(99 * 1024 * 1024, 8), 1);
    }

    #[test]
    fn test_calculate_optimal_segments_medium_file() {
        // 200MB file should use ~4 segments
        let segments = calculate_optimal_segments(200 * 1024 * 1024, 8);
        assert!(segments >= 2 && segments <= 4);
    }

    #[test]
    fn test_calculate_optimal_segments_respects_user_max() {
        // Should never exceed user max
        assert_eq!(calculate_optimal_segments(1024 * 1024 * 1024, 2), 2);
        assert_eq!(calculate_optimal_segments(1024 * 1024 * 1024, 1), 1);
    }

    #[test]
    fn test_calculate_optimal_segments_zero_size() {
        assert_eq!(calculate_optimal_segments(0, 8), 1);
    }
}
