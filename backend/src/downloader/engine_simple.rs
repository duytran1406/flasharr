//! Simple Download Engine
//!
//! Pure HTTP download logic without task management.
//! Uses single-stream downloading for simplicity and reliability.

use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use futures_util::StreamExt;

use super::config::DownloadConfig;
use super::progress::DownloadProgress;

/// Simple download engine - pure HTTP single-stream downloader
pub struct SimpleDownloadEngine {
    /// HTTP client
    http_client: reqwest::Client,
    
    /// Configuration
    #[allow(dead_code)]
    config: DownloadConfig,
}

impl SimpleDownloadEngine {
    /// Create new engine with configuration
    pub fn with_config(config: DownloadConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .pool_max_idle_per_host(10)
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            http_client,
            config,
        }
    }
    
    /// Download a file with progress tracking (single-stream)
    pub async fn download_file<F>(
        &self,
        url: &str,
        destination: &Path,
        progress_callback: F,
        cancel_token: &CancellationToken,
    ) -> anyhow::Result<()>
    where
        F: Fn(DownloadProgress) + Send + Sync + 'static,
    {
        let destination_buf = destination.to_path_buf();
        
        tracing::info!("Starting single-stream download: {} -> {:?}", url, destination);
        
        let total_downloaded = self.download_single_stream(
            url,
            &destination_buf,
            cancel_token,
            progress_callback,
        ).await?;
        
        tracing::info!("Download completed: {} bytes", total_downloaded);
        
        Ok(())
    }
    
    /// Download using single stream (with resume support)
    async fn download_single_stream<F>(
        &self,
        url: &str,
        destination: &PathBuf,
        cancel_token: &CancellationToken,
        progress_callback: F,
    ) -> anyhow::Result<u64>
    where
        F: Fn(DownloadProgress) + Send + Sync + 'static,
    {
        // Create parent directory if needed
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        // Check for existing partial download
        let initial_bytes = if destination.exists() {
            tokio::fs::metadata(&destination).await?.len()
        } else {
            0
        };
        
        // Build request with Range header for resume
        let mut request = self.http_client.get(url);
        if initial_bytes > 0 {
            request = request.header("Range", format!("bytes={}-", initial_bytes));
            tracing::info!("Resuming download from byte {}", initial_bytes);
        }
        
        let response = request.send().await?;
        
        // Check response status
        let status = response.status();
        if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            // File is already complete
            tracing::info!("File already complete (416)");
            return Ok(initial_bytes);
        }
        
        if !status.is_success() {
            anyhow::bail!("HTTP error: {}", status);
        }
        
        // Get content length
        let content_length = response.content_length().unwrap_or(0);
        let total_size = if initial_bytes > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT {
            initial_bytes + content_length
        } else {
            content_length
        };
        
        // Check if server ignored Range header (returned full content)
        let resume_position = if initial_bytes > 0 && status == reqwest::StatusCode::OK {
            tracing::warn!("Server ignored Range header, starting from beginning");
            0
        } else {
            initial_bytes
        };
        
        // Open file for writing
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(resume_position == 0)
            .append(resume_position > 0)
            .open(&destination)
            .await?;
        
        // Download with progress
        let mut downloaded = resume_position;
        let start_time = Instant::now();
        let mut stream = response.bytes_stream();
        
        // Progress update interval (every 250ms)
        let mut last_progress_update = Instant::now();
        let progress_interval = std::time::Duration::from_millis(250);
        
        while let Some(chunk_result) = stream.next().await {
            // Check cancellation
            if cancel_token.is_cancelled() {
                anyhow::bail!("Download cancelled");
            }
            
            let chunk = chunk_result?;
            let chunk_len = chunk.len() as u64;
            
            // Write chunk
            file.write_all(&chunk).await?;
            downloaded += chunk_len;
            
            // Update progress (throttled)
            if last_progress_update.elapsed() >= progress_interval {
                let elapsed = start_time.elapsed().as_secs_f64();
                let speed = if elapsed > 0.0 {
                    (downloaded - resume_position) as f64 / elapsed
                } else {
                    0.0
                };
                let eta = if speed > 0.0 && total_size > downloaded {
                    (total_size - downloaded) as f64 / speed
                } else {
                    0.0
                };
                let percentage = if total_size > 0 {
                    (downloaded as f64 / total_size as f64) * 100.0
                } else {
                    0.0
                };
                
                progress_callback(DownloadProgress {
                    downloaded_bytes: downloaded,
                    total_bytes: total_size,
                    speed_bytes_per_sec: speed,
                    eta_seconds: eta,
                    percentage,
                    initial_bytes: resume_position,
                });
                
                last_progress_update = Instant::now();
            }
        }
        
        file.flush().await?;
        
        // Final progress update
        let elapsed = start_time.elapsed().as_secs_f64();
        let speed = if elapsed > 0.0 {
            (downloaded - resume_position) as f64 / elapsed
        } else {
            0.0
        };
        
        progress_callback(DownloadProgress {
            downloaded_bytes: downloaded,
            total_bytes: total_size,
            speed_bytes_per_sec: speed,
            eta_seconds: 0.0,
            percentage: 100.0,
            initial_bytes: resume_position,
        });
        
        Ok(downloaded)
    }
}
