//! Simple Download Engine
//!
//! Pure HTTP download logic without task management.
//! Uses single-stream downloading for simplicity and reliability.

use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

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
            .connect_timeout(std::time::Duration::from_secs(60))
            .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
            .timeout(std::time::Duration::from_secs(86400)) // 24h total timeout for large files
            .build()
            .expect("Failed to create HTTP client");

        Self {
            http_client,
            config,
        }
    }

    /// Download a file with progress tracking (single-stream).
    ///
    /// Includes two layers of resilience:
    /// 1. `remap_if_not_writable`: if `/downloads` is not accessible (stale mount, legacy
    ///    path), transparently remaps to `{appData}/downloads/`.
    /// 2. Rescue fallback: if the target parent directory can't be created (e.g. a
    ///    root-owned subdir left by a docker exec debug session), retries in
    ///    `{appData}/downloads/rescue/` so the download still completes.
    /// Returns the actual path where the file was written (may differ from `destination`
    /// if `/downloads` was not writable and the engine remapped to appData/downloads/).
    pub async fn download_file<F>(
        &self,
        url: &str,
        destination: &Path,
        progress_callback: F,
        cancel_token: &CancellationToken,
    ) -> anyhow::Result<PathBuf>
    where
        F: Fn(DownloadProgress) + Send + Sync + 'static,
    {
        // Remap legacy /downloads paths if the mount is not writable
        let dest_path = self.remap_if_not_writable(destination.to_path_buf());

        // Create parent directory; on failure fall back to a rescue path inside appData
        let parent_writable = if let Some(parent) = dest_path.parent() {
            match tokio::fs::create_dir_all(parent).await {
                Err(e) => {
                    tracing::error!(
                        "Failed to create download directory {}: {}. Falling back to rescue path.",
                        parent.display(),
                        e
                    );
                    false
                }
                Ok(()) => {
                    // Directory exists (or was just created), but it might be owned by root
                    // (e.g. left by a docker exec debug session). Probe write access.
                    let probe = parent.join(".flasharr_write_probe");
                    let writable = std::fs::File::create(&probe).is_ok();
                    let _ = std::fs::remove_file(&probe);
                    if !writable {
                        tracing::warn!(
                            "Parent directory {} exists but is not writable. Falling back to rescue path.",
                            parent.display()
                        );
                    }
                    writable
                }
            }
        } else {
            true
        };

        if !parent_writable {
            let app_data_dir = std::env::var("FLASHARR_APPDATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("appData"));
            let rescue_dir = app_data_dir.join("downloads").join("rescue");
            tokio::fs::create_dir_all(&rescue_dir).await?;
            let filename = dest_path.file_name().unwrap_or_default();
            let rescue_path = rescue_dir.join(filename);
            tracing::warn!("Rescue path: {:?}", rescue_path);
            return self
                .download_single_stream_with_rename(
                    url,
                    rescue_path,
                    progress_callback,
                    cancel_token,
                )
                .await;
        }

        self.download_single_stream_with_rename(url, dest_path, progress_callback, cancel_token)
            .await
    }

    /// Downloads to a `.flasharr` temp file then renames on completion, so Sonarr/Radarr
    /// never pick up an incomplete file.
    async fn download_single_stream_with_rename<F>(
        &self,
        url: &str,
        dest_path: PathBuf,
        progress_callback: F,
        cancel_token: &CancellationToken,
    ) -> anyhow::Result<PathBuf>
    where
        F: Fn(DownloadProgress) + Send + Sync + 'static,
    {
        let temp_destination = dest_path.with_extension(format!(
            "{}.flasharr",
            dest_path.extension().and_then(|e| e.to_str()).unwrap_or("")
        ));

        tracing::info!(
            "Starting single-stream download: {} -> {:?}",
            url,
            temp_destination
        );

        let total_downloaded = self
            .download_single_stream(url, &temp_destination, cancel_token, progress_callback)
            .await?;

        tracing::info!("Download completed: {} bytes", total_downloaded);

        // Rename temp file to final name
        if temp_destination.exists() {
            tokio::fs::rename(&temp_destination, &dest_path).await?;
            tracing::info!("Renamed {:?} -> {:?}", temp_destination, dest_path);
        }

        Ok(dest_path)
    }

    /// Remaps a `/downloads/...` path to `{appData}/downloads/...` when `/downloads`
    /// is not writable (e.g. mount missing or Docker created it as root).
    fn remap_if_not_writable(&self, path: PathBuf) -> PathBuf {
        if path.starts_with("/downloads") {
            let test_file = Path::new("/downloads").join(".write_test");
            if std::fs::File::create(&test_file).is_ok() {
                let _ = std::fs::remove_file(test_file);
                path
            } else {
                let app_data_dir = std::env::var("FLASHARR_APPDATA_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| {
                        std::env::current_dir().unwrap_or_default().join("appData")
                    });
                let rel_path = path.strip_prefix("/downloads").unwrap_or(&path);
                let fallback = app_data_dir.join("downloads").join(rel_path);
                tracing::warn!(
                    "Remapping {} -> {} (permission issues on /downloads)",
                    path.display(),
                    fallback.display()
                );
                fallback
            }
        } else {
            path
        }
    }

    /// Core single-stream HTTP downloader with resume support.
    async fn download_single_stream<F>(
        &self,
        url: &str,
        destination: &Path,
        cancel_token: &CancellationToken,
        progress_callback: F,
    ) -> anyhow::Result<u64>
    where
        F: Fn(DownloadProgress) + Send + Sync + 'static,
    {
        // Check for existing partial download
        let initial_bytes = if destination.exists() {
            tokio::fs::metadata(destination).await?.len()
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
            .open(destination)
            .await?;

        // Download with progress
        let mut downloaded = resume_position;
        let start_time = Instant::now();
        let mut stream = response.bytes_stream();

        let mut last_progress_update = Instant::now();
        let progress_interval = std::time::Duration::from_millis(250);

        while let Some(chunk_result) = stream.next().await {
            if cancel_token.is_cancelled() {
                anyhow::bail!("Download cancelled");
            }

            let chunk = chunk_result?;
            let chunk_len = chunk.len() as u64;

            file.write_all(&chunk).await?;
            downloaded += chunk_len;

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
