//! Download Progress Tracking
//!
//! Tracks download progress with speed calculation and ETA estimation.

use serde::{Deserialize, Serialize};

/// Progress information for a download
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DownloadProgress {
    /// Bytes downloaded so far
    pub downloaded_bytes: u64,
    
    /// Total file size in bytes
    pub total_bytes: u64,
    
    /// Current download speed in bytes/sec
    pub speed_bytes_per_sec: f64,
    
    /// Estimated time remaining in seconds
    pub eta_seconds: f64,
    
    /// Download completion percentage (0-100)
    pub percentage: f64,
    
    /// Bytes already present when session started (for resume)
    #[serde(skip)]
    #[allow(dead_code)]
    pub initial_bytes: u64,
}

#[allow(dead_code)]
impl DownloadProgress {
    /// Create new progress tracker
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Create progress with known total size
    pub fn with_total(total_bytes: u64) -> Self {
        Self {
            total_bytes,
            ..Default::default()
        }
    }
    
    /// Update progress calculations
    ///
    /// # Arguments
    /// * `downloaded` - Total bytes downloaded
    /// * `total` - Total file size
    /// * `elapsed_seconds` - Time elapsed since download start
    pub fn update(&mut self, downloaded: u64, total: u64, elapsed_seconds: f64) {
        self.downloaded_bytes = if total > 0 { downloaded.min(total) } else { downloaded };
        self.total_bytes = total;
        
        // Calculate percentage
        if total > 0 {
            self.percentage = ((self.downloaded_bytes as f64 / total as f64) * 100.0).min(100.0);
        }
        
        // Calculate speed based on session bytes (excluding resumed bytes)
        if elapsed_seconds > 0.0 {
            let session_downloaded = self.downloaded_bytes.saturating_sub(self.initial_bytes);
            self.speed_bytes_per_sec = (session_downloaded as f64 / elapsed_seconds).max(0.0);
            
            // Calculate ETA
            if self.speed_bytes_per_sec > 0.0 && total > 0 {
                let remaining = total.saturating_sub(self.downloaded_bytes);
                self.eta_seconds = (remaining as f64 / self.speed_bytes_per_sec).max(0.0);
            } else {
                self.eta_seconds = 0.0;
            }
        }
    }
    
    /// Mark download as complete
    pub fn complete(&mut self) {
        self.downloaded_bytes = self.total_bytes;
        self.percentage = 100.0;
        self.speed_bytes_per_sec = 0.0;
        self.eta_seconds = 0.0;
    }
    
    /// Reset progress (for retries)
    pub fn reset(&mut self) {
        self.downloaded_bytes = 0;
        self.speed_bytes_per_sec = 0.0;
        self.eta_seconds = 0.0;
        self.percentage = 0.0;
        self.initial_bytes = 0;
    }
    
    /// Get human-readable speed string
    pub fn speed_string(&self) -> String {
        format_bytes_per_sec(self.speed_bytes_per_sec)
    }
    
    /// Get human-readable ETA string
    pub fn eta_string(&self) -> String {
        format_duration(self.eta_seconds)
    }
}

/// Format bytes per second as human-readable string
#[allow(dead_code)]
pub fn format_bytes_per_sec(bps: f64) -> String {
    if bps >= 1_000_000_000.0 {
        format!("{:.2} GB/s", bps / 1_000_000_000.0)
    } else if bps >= 1_000_000.0 {
        format!("{:.2} MB/s", bps / 1_000_000.0)
    } else if bps >= 1_000.0 {
        format!("{:.2} KB/s", bps / 1_000.0)
    } else {
        format!("{:.0} B/s", bps)
    }
}

/// Format duration in seconds as human-readable string
#[allow(dead_code)]
pub fn format_duration(seconds: f64) -> String {
    let secs = seconds as u64;
    if secs >= 3600 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}

/// Broadcast update for WebSocket clients
#[derive(Debug, Clone, Serialize)]
pub struct ProgressUpdate {
    pub task_id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub speed_bytes_per_sec: f64,
    pub eta_seconds: f64,
    pub percentage: f64,
    pub state: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_update() {
        let mut progress = DownloadProgress::new();
        progress.update(500, 1000, 1.0);
        
        assert_eq!(progress.downloaded_bytes, 500);
        assert_eq!(progress.total_bytes, 1000);
        assert!((progress.percentage - 50.0).abs() < 0.01);
        assert!((progress.speed_bytes_per_sec - 500.0).abs() < 0.01);
    }

    #[test]
    fn test_progress_with_resume() {
        let mut progress = DownloadProgress::new();
        progress.initial_bytes = 200; // Started with 200 bytes
        progress.update(500, 1000, 1.0);
        
        // Speed should be based on session bytes (500 - 200 = 300)
        assert!((progress.speed_bytes_per_sec - 300.0).abs() < 0.01);
    }

    #[test]
    fn test_format_bytes_per_sec() {
        assert_eq!(format_bytes_per_sec(500.0), "500 B/s");
        assert_eq!(format_bytes_per_sec(1500.0), "1.50 KB/s");
        assert_eq!(format_bytes_per_sec(1_500_000.0), "1.50 MB/s");
        assert_eq!(format_bytes_per_sec(1_500_000_000.0), "1.50 GB/s");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(45.0), "45s");
        assert_eq!(format_duration(125.0), "2m 5s");
        assert_eq!(format_duration(3725.0), "1h 2m");
    }
}
