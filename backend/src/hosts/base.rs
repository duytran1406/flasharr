//! HostHandler trait - Plugin interface for file hosts

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

use crate::downloader::task::DownloadTask;

/// File information from host
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub filename: String,
    pub size: u64,
    pub original_url: String,
}

/// Resolved download URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedUrl {
    pub direct_url: String,
    pub headers: HashMap<String, String>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Account status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountStatus {
    pub can_download: bool,
    pub reason: Option<String>,
    pub account_email: String,
    pub premium: bool,
    pub valid_until: Option<u64>,
    pub traffic_left: Option<String>,
}

/// Host handler trait - must be implemented by all host plugins
#[async_trait]
pub trait HostHandler: Send + Sync {
    /// Get host name (e.g., "fshare", "gdrive")
    fn get_host_name(&self) -> &str;
    
    /// Check if this handler can handle the given URL
    fn can_handle(&self, url: &str) -> bool;
    
    /// Get file information (filename, size) from URL
    async fn get_file_info(&self, url: &str) -> anyhow::Result<FileInfo>;
    
    /// Check account status (can download, quota left)
    async fn check_account_status(&self) -> anyhow::Result<AccountStatus>;
    
    /// Resolve URL to direct download link
    async fn resolve_download_url(&self, url: &str) -> anyhow::Result<ResolvedUrl>;
    
    /// Validate if resolved URL is still valid
    async fn validate_download_url(&self, url: &str) -> anyhow::Result<bool>;
    
    /// Refresh expired download URL
    async fn refresh_download_url(&self, original_url: &str) -> anyhow::Result<ResolvedUrl>;
    
    /// Check if host supports resume
    fn supports_resume(&self) -> bool;
    
    /// Check if host supports multi-segment download
    fn supports_multi_segment(&self) -> bool;
    
    /// Get maximum number of segments
    fn get_max_segments(&self) -> u32;
    
    /// Pre-download hook (called before download starts)
    async fn pre_download_hook(&self, _task: &mut DownloadTask) -> anyhow::Result<()> {
        Ok(())
    }
    
    /// Post-download hook (called after download completes)
    async fn post_download_hook(&self, _task: &mut DownloadTask) -> anyhow::Result<()> {
        Ok(())
    }

    /// Logout - clear in-memory sessions/credentials
    async fn logout(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
