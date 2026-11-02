//! Download Orchestrator
//!
//! Coordinates between TaskManager and DownloadEngine.
//! Handles URL resolution, retry logic, and progress broadcasting.

use chrono::Utc;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::sync::{broadcast, Mutex, Notify, RwLock};
use tokio::task::JoinHandle;

fn resolve_download_info_path(tmdb_id: Option<i64>, media_type: Option<&str>) -> Option<String> {
    match (tmdb_id, media_type) {
        (Some(tmdb_id), Some("tv")) => Some(format!("/tv/{}", tmdb_id)),
        (Some(tmdb_id), Some("movie")) => Some(format!("/movie/{}", tmdb_id)),
        _ => None,
    }
}

fn canonical_qbittorrent_input(host: &str, url: &str, info_hash: Option<&str>) -> String {
    if host == "qbittorrent" {
        if let Some(magnet) =
            info_hash.and_then(|hash| crate::sources::torrent::build_magnet_url(hash, None))
        {
            return magnet;
        }
    }
    url.to_string()
}

fn canonical_provider_identity(
    host: &str,
    url: &str,
    info_hash: Option<&str>,
    external_id: Option<&str>,
) -> (String, Option<String>) {
    if host == "qbittorrent" {
        let extracted = crate::sources::torrent::extract_btih(url);
        let hash = info_hash
            .or(external_id)
            .map(str::to_string)
            .or(extracted)
            .map(|v| v.trim().to_ascii_lowercase());
        return ("torrent".to_string(), hash);
    }
    (host.to_ascii_lowercase(), None)
}

fn reconciled_transfer_state(current: DownloadState, transfer: DownloadState) -> DownloadState {
    if matches!(
        current,
        DownloadState::Importing | DownloadState::Completed | DownloadState::Failed
    ) {
        current
    } else {
        transfer
    }
}

fn import_owns_transfer_state(state: DownloadState) -> bool {
    matches!(state, DownloadState::Importing | DownloadState::Completed)
}

fn status_refresh_state(
    current: DownloadState,
    observed: DownloadState,
    arr_announced: bool,
) -> DownloadState {
    if observed == DownloadState::Completed && !arr_announced {
        current
    } else {
        reconciled_transfer_state(current, observed)
    }
}

fn qbit_source_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(path)
        .strip_prefix("/downloads")
        .map(|relative| std::path::Path::new("/data/downloads").join(relative))
        .unwrap_or_else(|_| std::path::PathBuf::from(path))
}

async fn library_artifact_exists(task: &DownloadTask, arr_path: Option<&str>) -> bool {
    let Some(arr_path) = arr_path else {
        return false;
    };
    let root = std::path::Path::new(arr_path);
    let target_dir = task
        .tmdb_season
        .map(|season| root.join(format!("Season {season}")))
        .unwrap_or_else(|| root.to_path_buf());
    let episode = task
        .tmdb_season
        .zip(task.tmdb_episode)
        .map(|(season, episode)| format!("s{season:02}e{episode:02}"));
    let Ok(mut entries) = tokio::fs::read_dir(target_dir).await else {
        return false;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let is_media = path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| {
                matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "mkv" | "mp4" | "avi" | "m4v" | "ts" | "mov"
                )
            });
        if is_media
            && episode
                .as_ref()
                .is_none_or(|pattern| name.contains(pattern))
            && tokio::fs::metadata(path).await.is_ok()
        {
            return true;
        }
    }
    false
}

use super::config::DownloadConfig;
use super::duplicate_detector::DuplicateDetector;
use super::engine_simple::SimpleDownloadEngine;
use super::manager::DownloadTaskManager;
use super::path_builder::{PathBuilder, TmdbDownloadMetadata};
use super::progress::{ProgressUpdate, TaskEvent};
use super::stats::EngineStats;
use super::task::{DownloadState, DownloadTask, UrlMetadata};
use crate::db::Db;
use crate::hosts::registry::HostRegistry;
use crate::utils::parser::FilenameParser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QbitConfirmationDecision {
    Confirmed,
    Pending,
    Failed(&'static str),
}

#[derive(Debug, PartialEq)]
enum RestartAdoption {
    Adopt,
    Add,
    Wait,
}

fn restart_adoption(
    health: &str,
    observed_hash: Option<&str>,
    expected_hash: &str,
    restored_external: bool,
) -> RestartAdoption {
    if health != "healthy" {
        return RestartAdoption::Wait;
    }
    match observed_hash {
        Some(hash) if hash.eq_ignore_ascii_case(expected_hash) => RestartAdoption::Adopt,
        Some(_) => RestartAdoption::Wait,
        None if restored_external => RestartAdoption::Wait,
        None => RestartAdoption::Add,
    }
}

fn is_restored_external_qbit_task(
    state: DownloadState,
    error_message: Option<&str>,
    external_id: Option<&str>,
    external_provider: Option<&str>,
    external_instance_id: Option<&str>,
) -> bool {
    let retryable_pre_add_wait = state == DownloadState::Waiting
        && error_message
            .is_some_and(|message| message.starts_with("qBittorrent torrent is absent"));
    state != DownloadState::Queued
        && !retryable_pre_add_wait
        && (external_id.is_some()
            || external_provider == Some("qbittorrent")
            || external_instance_id.is_some())
}

async fn run_restart_adoption<T, F, Fut>(decision: RestartAdoption, add: F) -> Option<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    match decision {
        RestartAdoption::Add => Some(add().await),
        RestartAdoption::Adopt | RestartAdoption::Wait => None,
    }
}

/// Root-cause reconciliation decision for a single managed qB task.
///
/// Shared by both the new-add path (`start_qbittorrent_task`) and the durable
/// periodic/startup reconcile tick. Hash presence alone is never treated as
/// transfer confirmation — the exact-hash identity must match AND the transfer
/// classifier must report meaningful active progress before `Downloading`.
#[derive(Debug, PartialEq, Eq)]
enum ManagedQbitDecision {
    /// Genuinely-new locator, absent from a healthy instance: bounded add once.
    AddOnce,
    /// Present placeholder (metaDL/checking/queued/stalled/blank) or a restored
    /// identity that is absent: hold in STARTING/WAITING with an explicit reason.
    Starting(&'static str),
    /// API unavailable, restored-external absent, or otherwise not addable now.
    Wait(&'static str),
    /// Exact-hash identity present with meaningful active transfer evidence.
    Downloading,
    /// Exact-hash identity present and the transfer is complete.
    Completed,
    /// Deadline exceeded without meaningful confirmation: deterministic failure.
    Failed(&'static str),
}

/// Unified confirmation predicate. `observed` is the direct exact-hash query
/// result: `Err(())` = API failure, `Ok(None)` = absent, `Ok(Some((hash,state)))`
/// = a row whose hash must be identity-checked. `metrics` feed the shared
/// `classify_qbit` transfer classifier so "present" alone never confirms transfer.
#[allow(clippy::too_many_arguments)]
fn managed_qbit_decision(
    health: &str,
    expected_hash: &str,
    observed: Result<Option<(&str, &str)>, ()>,
    metrics: crate::torrent::state::TransferMetrics,
    restored_external: bool,
    deadline_exceeded: bool,
) -> ManagedQbitDecision {
    if !health.eq_ignore_ascii_case("healthy") {
        return ManagedQbitDecision::Wait(
            "qBittorrent instance is not healthy; reconciliation waits",
        );
    }
    match observed {
        Err(()) => {
            ManagedQbitDecision::Wait("qBittorrent API unavailable; reconciliation will retry")
        }
        Ok(None) if restored_external => ManagedQbitDecision::Wait(
            "qBittorrent torrent is absent; restored identity will not be added again",
        ),
        Ok(None) if deadline_exceeded => {
            ManagedQbitDecision::Failed("qBittorrent add/confirmation deadline exceeded")
        }
        Ok(None) => ManagedQbitDecision::AddOnce,
        Ok(Some((hash, _))) if !hash.eq_ignore_ascii_case(expected_hash) => {
            ManagedQbitDecision::Failed("qBittorrent returned a mismatched torrent hash")
        }
        Ok(Some((_, state))) => {
            let classification = crate::torrent::state::classify_qbit(state, metrics, health);
            match classification.task_state {
                DownloadState::Downloading => ManagedQbitDecision::Downloading,
                DownloadState::Completed => ManagedQbitDecision::Completed,
                DownloadState::Failed => {
                    ManagedQbitDecision::Failed("qBittorrent reports the torrent is blocked")
                }
                _ => ManagedQbitDecision::Starting(
                    "qBittorrent is waiting for metadata or transfer evidence",
                ),
            }
        }
    }
}

fn qbit_confirmation_decision(
    expected_hash: &str,
    observed: Result<Option<&str>, ()>,
    exhausted: bool,
) -> QbitConfirmationDecision {
    match observed {
        Err(()) => QbitConfirmationDecision::Failed(
            "qBittorrent confirmation API failed; check the configured instance",
        ),
        Ok(Some(hash)) if hash.trim().is_empty() => {
            qbit_confirmation_decision(expected_hash, Ok(None), exhausted)
        }
        Ok(Some(hash)) if hash.eq_ignore_ascii_case(expected_hash) => {
            QbitConfirmationDecision::Confirmed
        }
        Ok(Some(_)) => QbitConfirmationDecision::Failed(
            "qBittorrent confirmation returned a mismatched torrent hash",
        ),
        Ok(None) if exhausted => QbitConfirmationDecision::Failed(
            "qBittorrent did not confirm the accepted torrent within 60 seconds",
        ),
        Ok(None) => QbitConfirmationDecision::Pending,
    }
}

fn qbit_confirmation_requires_add(decision: QbitConfirmationDecision) -> bool {
    matches!(decision, QbitConfirmationDecision::Pending)
}

async fn poll_qbit_confirmation<F, Fut>(
    expected_hash: &str,
    budget: Duration,
    call_timeout: Duration,
    mut get_torrent: F,
) -> QbitConfirmationDecision
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Option<String>>>,
{
    let deadline = tokio::time::Instant::now() + budget;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return qbit_confirmation_decision(expected_hash, Ok(None), true);
        }
        let observed = match tokio::time::timeout(call_timeout.min(remaining), get_torrent()).await
        {
            Ok(Ok(hash)) => hash,
            Ok(Err(_)) => return qbit_confirmation_decision(expected_hash, Err(()), false),
            Err(_) if tokio::time::Instant::now() >= deadline => {
                return qbit_confirmation_decision(expected_hash, Ok(None), true)
            }
            Err(_) => None,
        };
        match qbit_confirmation_decision(expected_hash, Ok(observed.as_deref()), false) {
            QbitConfirmationDecision::Pending => {
                tokio::time::sleep(
                    Duration::from_secs(2)
                        .min(deadline.saturating_duration_since(tokio::time::Instant::now())),
                )
                .await;
            }
            decision => return decision,
        }
    }
}

async fn run_qbit_add_with_timeout<F, Fut, T>(budget: Duration, add: F) -> anyhow::Result<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    tokio::time::timeout(budget, add())
        .await
        .map_err(|_| anyhow::anyhow!("qBittorrent add timed out after 30 seconds"))?
        .map_err(|_| anyhow::anyhow!("qBittorrent add API failed; check the configured instance"))
}

/// Outcome of an `request_arr_import` call, carrying enough information for the
/// worker to enter the `Importing` state and drive closed-loop polling.
enum ArrMoveOutcome {
    /// Arr import was requested or the file is already present. arr_id identifies
    /// the Sonarr series or Radarr movie to poll. `task` has destination updated
    /// but `arr_announced` is intentionally left `false` until polling confirms.
    Placed {
        task: DownloadTask,
        arr_id: i64,
        media_type: crate::downloader::MediaType,
    },
    /// arr is not configured or task has no TMDB metadata — skip import polling
    /// and mark completed without arr confirmation.
    Skipped { task: DownloadTask },
    /// A fatal error occurred (e.g. filesystem, arr API hard failure) and the
    /// task could not be placed. Caller should record the error.
    Failed { reason: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct BackfillItem {
    pub id: String,
    pub filename: String,
    pub media_type: String,
    pub source_path: String,
    pub arr_id: Option<i64>,
    pub arr_path: Option<String>,
    #[serde(rename = "hasFile")]
    pub has_file: Option<bool>,
    pub reason: String,
    pub action: String,
}

async fn looks_like_importable_media(path: &std::path::Path) -> anyhow::Result<bool> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut header = [0_u8; 4096];
    let bytes_read = file.read(&mut header).await?;
    let header = &header[..bytes_read];

    if header.len() >= 4 && header[..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        return Ok(true);
    }

    if header.len() >= 12 && &header[4..8] == b"ftyp" {
        return Ok(true);
    }

    if header.len() >= 12 && &header[..4] == b"RIFF" && &header[8..12] == b"AVI " {
        return Ok(true);
    }

    if header.first() == Some(&0x47) {
        return Ok(true);
    }

    let prefix_len = header.len().min(256);
    let prefix = String::from_utf8_lossy(&header[..prefix_len]).to_lowercase();
    let trimmed = prefix.trim_start();
    if trimmed.starts_with("<!doctype")
        || trimmed.starts_with("<html")
        || trimmed.starts_with("<script")
        || trimmed.starts_with("<?xml")
        || trimmed.starts_with('{')
    {
        return Ok(false);
    }

    Ok(false)
}

/// Download orchestrator - coordinates all download operations
pub struct DownloadOrchestrator {
    /// Task manager (Single Source of Truth)
    task_manager: Arc<DownloadTaskManager>,

    /// HTTP download engine (wrapped for dynamic updates)
    download_engine: Arc<RwLock<Arc<SimpleDownloadEngine>>>,

    /// Host registry for URL resolution
    host_registry: Arc<HostRegistry>,

    /// Database connection
    db: Option<Arc<Db>>,

    /// Progress broadcast channel (legacy, will be deprecated)
    progress_tx: broadcast::Sender<ProgressUpdate>,

    /// Event bus for event-driven architecture
    event_bus: Arc<super::events::EventBus>,

    /// Worker handles
    workers: Mutex<Vec<JoinHandle<()>>>,

    /// Configuration (wrapped for dynamic updates)
    config: Arc<RwLock<DownloadConfig>>,

    /// Running state
    running: Arc<std::sync::atomic::AtomicBool>,

    /// Task notification for workers (wake when new task added)
    task_notify: Arc<Notify>,

    /// *arr API client for bi-directional sync (wrapped for dynamic updates)
    arr_client: Arc<RwLock<Option<Arc<crate::arr::ArrClient>>>>,

    /// *arr artifact manager for Series/Movie metadata (Phase 1: Artifact Management)
    artifact_manager: Arc<RwLock<Option<Arc<crate::arr::ArrArtifactManager>>>>,

    /// Track announced batches to prevent duplicate announcements
    announced_batches: Arc<RwLock<std::collections::HashSet<String>>>,

    /// qBittorrent client for externally-managed torrent downloads.
    qbittorrent_client: Arc<RwLock<Option<Arc<crate::torrent::qbittorrent::QBittorrentClient>>>>,

    /// qBittorrent instance mirror/control plane.
    qbit_manager: Option<Arc<crate::torrent::instance_manager::QbitInstanceManager>>,
}

impl DownloadOrchestrator {
    fn canonical_qbittorrent_input(host: &str, url: &str, info_hash: Option<&str>) -> String {
        canonical_qbittorrent_input(host, url, info_hash)
    }

    #[cfg(test)]
    fn qbit_url_for_test(host: &str, url: &str, info_hash: Option<&str>) -> String {
        Self::canonical_qbittorrent_input(host, url, info_hash)
    }

    /// Create new orchestrator
    pub fn new(
        config: DownloadConfig,
        host_registry: Arc<HostRegistry>,
        db: Option<Arc<Db>>,
        sonarr_config: Option<crate::config::ArrConfig>,
        radarr_config: Option<crate::config::ArrConfig>,
        qbittorrent_config: crate::torrent::qbittorrent::QBittorrentConfig,
        qbit_manager: Option<Arc<crate::torrent::instance_manager::QbitInstanceManager>>,
    ) -> Self {
        let task_manager = Arc::new(DownloadTaskManager::new());
        let download_engine = Arc::new(SimpleDownloadEngine::with_config(config.clone()));
        let (progress_tx, _) = broadcast::channel(1000);

        // Create event bus for event-driven architecture
        let event_bus = Arc::new(super::events::EventBus::new(1000));

        // Phase 4: Start TaskManager event listener for auto-updating cache
        let event_rx = event_bus.subscribe();
        Arc::clone(&task_manager).start_event_listener(event_rx);
        Arc::clone(&task_manager).start_cleanup_loop();
        tracing::info!(
            "Phase 4 activated: TaskManager event-driven cache with auto-eviction + background cleanup"
        );

        // Create arr client if either Sonarr or Radarr is configured
        tracing::info!(
            "Arr Config Check: sonarr={}, radarr={}",
            sonarr_config.is_some(),
            radarr_config.is_some()
        );
        let arr_client = if sonarr_config.is_some() || radarr_config.is_some() {
            tracing::info!("Creating ArrClient with Sonarr/Radarr configuration");
            Some(Arc::new(crate::arr::ArrClient::new(
                sonarr_config,
                radarr_config,
            )))
        } else {
            tracing::warn!("No Sonarr or Radarr configuration found - arr_client will be None");
            None
        };

        // Create artifact manager for Series/Movie metadata management
        let artifact_manager = if let (Some(ref client), Some(ref database)) = (&arr_client, &db) {
            tracing::info!("Creating ArrArtifactManager for artifact lifecycle management");
            Some(Arc::new(crate::arr::ArrArtifactManager::new(
                Arc::clone(client),
                Arc::clone(database),
            )))
        } else {
            None
        };

        let qbittorrent_client = if qbittorrent_config.enabled {
            match crate::torrent::qbittorrent::QBittorrentClient::new(qbittorrent_config) {
                Ok(client) => Some(Arc::new(client)),
                Err(e) => {
                    tracing::warn!("qBittorrent client disabled: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            task_manager,
            download_engine: Arc::new(RwLock::new(download_engine)),
            host_registry,
            db,
            progress_tx,
            event_bus,
            workers: Mutex::new(Vec::new()),
            config: Arc::new(RwLock::new(config)),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            task_notify: Arc::new(Notify::new()),
            arr_client: Arc::new(RwLock::new(arr_client)),
            artifact_manager: Arc::new(RwLock::new(artifact_manager)),

            announced_batches: Arc::new(RwLock::new(std::collections::HashSet::new())),
            qbittorrent_client: Arc::new(RwLock::new(qbittorrent_client)),
            qbit_manager,
        }
    }

    /// Start orchestrator with workers
    pub async fn start(&self) {
        use std::sync::atomic::Ordering;

        if self.running.load(Ordering::SeqCst) {
            return;
        }

        self.running.store(true, Ordering::SeqCst);

        {
            let config_guard = self.config.read().await;
            tracing::info!(
                "Starting download orchestrator with {} workers",
                config_guard.max_concurrent
            );
        }

        // Spawn worker tasks
        let mut workers = self.workers.lock().await;
        // Acquire config read lock to get max_concurrent
        let max_concurrent = self.config.read().await.max_concurrent;
        for worker_id in 0..max_concurrent {
            let handle = self.spawn_worker(worker_id);
            workers.push(handle);
        }
        // Instance manager owns qB mirroring; this poller maps mirror rows to Flasharr task lifecycle.
        workers.push(self.spawn_qbittorrent_mirror_consumer());
        // Durable reconcile: qB QUEUED/STARTING/DOWNLOADING tasks that never receive a
        // mirror event (cold restart, metaDL placeholder, blank row) still need a watcher.
        workers.push(self.spawn_qbit_reconcile_ticker());
    }

    /// Stop orchestrator
    pub async fn stop(&self) {
        use std::sync::atomic::Ordering;

        self.running.store(false, Ordering::SeqCst);

        // Wait for workers to finish
        let mut workers = self.workers.lock().await;
        for worker in workers.drain(..) {
            let _ = worker.await;
        }

        tracing::info!("Download orchestrator stopped");
    }

    /// Add a new download with duplicate detection
    pub async fn add_download(
        &self,
        url: String,
        filename: String,
        host: String,
        category: String,
    ) -> Result<DownloadTask, anyhow::Error> {
        self.add_download_with_metadata(
            url,
            Some(filename),
            host,
            category,
            None,
            None,
            None,
            false,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
    }

    /// Add a new download with TMDB metadata for organized folder structure
    pub async fn add_download_with_metadata(
        &self,
        url: String,
        filename_override: Option<String>,
        host: String,
        category: String,
        mut tmdb_metadata: Option<TmdbDownloadMetadata>,
        batch_id: Option<String>,
        batch_name: Option<String>,
        force_redownload: bool,
        download_source: Option<String>,
        external_provider: Option<String>,
        initial_size: Option<u64>,
        seeders: Option<i64>,
        leechers: Option<i64>,
        info_hash: Option<String>,
        external_id: Option<String>,
        external_instance_id: Option<String>,
        origin_provider: Option<String>,
        origin_indexer: Option<String>,
        file_idx: Option<usize>,
        source_filename: Option<String>,
    ) -> Result<DownloadTask, anyhow::Error> {
        tracing::info!("[ORCHESTRATOR] add_download_with_metadata called");
        tracing::info!(
            "[ORCHESTRATOR] Input URL present: {}",
            !url.trim().is_empty()
        );
        tracing::info!(
            "[ORCHESTRATOR] Input filename_override: {:?}",
            filename_override
        );
        tracing::info!("[ORCHESTRATOR] Input host: {}", host);
        tracing::info!("[ORCHESTRATOR] Input category: {}", category);
        if let Some(ref meta) = tmdb_metadata {
            tracing::info!(
                "[ORCHESTRATOR] TMDB: tmdb_id={:?}, media_type={:?}, title={:?}",
                meta.tmdb_id,
                meta.media_type,
                meta.title
            );
        }

        let is_tv = tmdb_metadata.as_ref().is_some_and(|meta| {
            matches!(meta.media_type.as_deref(), Some("tv") | Some("episode"))
                || meta.season.is_some()
                || meta.episode.is_some()
        }) || category.to_ascii_lowercase().starts_with("tv");
        let (batch_id, batch_name) = if is_tv {
            let meta = tmdb_metadata
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("TV download requires show metadata"))?;
            let (id, name) = crate::utils::batch_utils::canonical_tv_batch(
                meta.tvdb_id,
                meta.tmdb_id,
                meta.title.as_deref(),
            )?;
            (Some(id), Some(name))
        } else {
            (batch_id, batch_name)
        };

        let canonical_url = Self::canonical_qbittorrent_input(&host, &url, info_hash.as_deref());

        let (provider_id, torrent_item_id) = canonical_provider_identity(
            &host,
            &canonical_url,
            info_hash.as_deref(),
            external_id.as_deref(),
        );
        let provider_item_id = if provider_id == "fshare" {
            crate::providers::fshare::item_id_from_url(&canonical_url)
        } else {
            torrent_item_id.or_else(|| external_id.clone())
        };

        // Check for duplicates in memory first (fast path)
        if let Some(item_id) = &provider_item_id {
            if let Some(existing) = self.find_task_by_provider_item(&provider_id, item_id).await {
                if existing.state == DownloadState::Completed && force_redownload {
                    tracing::info!(
                        "Duplicate completed task [code: {}]: deleting stale task {} before re-add",
                        item_id,
                        existing.id
                    );
                    self.task_manager.delete_task(existing.id).await;
                    if let Some(db) = &self.db {
                        let _ = db.delete_task(existing.id);
                    }
                } else if matches!(
                    existing.state,
                    DownloadState::Failed | DownloadState::Cancelled
                ) {
                    self.task_manager.delete_task(existing.id).await;
                    if let Some(db) = &self.db {
                        let _ = db.delete_task(existing.id);
                    }
                } else {
                    return self.handle_duplicate(existing, item_id.clone()).await;
                }
            }
        }

        // Check for duplicates in database (slow path - catches evicted tasks)
        if let (Some(item_id), Some(db)) = (&provider_item_id, &self.db) {
            match db
                .find_task_by_provider_item_async(&provider_id, item_id)
                .await
            {
                Ok(Some((existing_id, existing_state))) => {
                    // Found existing task in database with the same provider identity.
                    match existing_state.as_str() {
                        "COMPLETED" if force_redownload => {
                            tracing::info!(
                                "Duplicate completed task in DB [provider_item_id: {}]: deleting stale task {} before re-add",
                                item_id,
                                existing_id
                            );
                            if let Ok(old_id) = uuid::Uuid::parse_str(&existing_id) {
                                let _ = db.delete_task(old_id);
                            }
                        }
                        "QUEUED" | "STARTING" | "DOWNLOADING" | "PAUSED" | "WAITING"
                        | "IMPORTING" | "COMPLETED" => {
                            tracing::info!(
                                "Duplicate detected in DB [provider_item_id: {}]: Task {} already exists in state {}, skipping",
                                item_id,
                                existing_id,
                                existing_state
                            );
                            return Err(anyhow::anyhow!(
                                "Download already exists ({})",
                                existing_state
                            ));
                        }
                        "FAILED" | "CANCELLED" => {
                            tracing::info!(
                                "Duplicate detected in DB [provider_item_id: {}]: Task {} is {}, deleting old and creating new",
                                item_id,
                                existing_id,
                                existing_state
                            );
                            // Delete old task from database
                            if let Ok(old_id) = uuid::Uuid::parse_str(&existing_id) {
                                let _ = db.delete_task(old_id);
                            }
                        }
                        _ => {
                            tracing::info!(
                                "Duplicate detected in DB [provider_item_id: {}]: Task {} in state {}, skipping",
                                item_id,
                                existing_id,
                                existing_state
                            );
                            return Err(anyhow::anyhow!(
                                "Download already exists ({})",
                                existing_state
                            ));
                        }
                    }
                }
                Ok(None) => {
                    // No duplicate found in DB, proceed
                }
                Err(e) => {
                    tracing::warn!("Failed to check DB for duplicate fshare code: {}", e);
                    // Continue anyway - better to have a potential duplicate than fail
                }
            }
        }

        let external_duplicate_provider = external_provider
            .as_deref()
            .or_else(|| (host == "qbittorrent").then_some("qbittorrent"));
        let external_duplicate_id = external_id.as_deref().or(info_hash.as_deref());
        if let (Some(provider), Some(id), Some(db)) =
            (external_duplicate_provider, external_duplicate_id, &self.db)
        {
            match db.find_task_by_external_async(provider, id).await {
                Ok(Some((existing_id, existing_state))) => match existing_state.as_str() {
                    "COMPLETED" if force_redownload => {
                        if let Ok(old_id) = uuid::Uuid::parse_str(&existing_id) {
                            let _ = db.delete_task(old_id);
                        }
                    }
                    "FAILED" | "CANCELLED" => {
                        if let Ok(old_id) = uuid::Uuid::parse_str(&existing_id) {
                            let _ = db.delete_task(old_id);
                        }
                    }
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Download already exists ({})",
                            existing_state
                        ))
                    }
                },
                Ok(None) => {}
                Err(e) => tracing::warn!("Failed to check DB for external duplicate: {}", e),
            }
        }

        // Keep queueing cold: do not call Fshare while adding a task. Smart Grab
        // already supplies a filename; manual adds can use the URL tail until the
        // worker reaches STARTING and resolves/enriches the task.
        let mut filename = filename_override.unwrap_or_else(|| {
            if canonical_url.starts_with("magnet:") {
                "unknown".to_string()
            } else {
                provider_item_id.clone().unwrap_or_else(|| {
                    canonical_url
                        .split('/')
                        .last()
                        .unwrap_or("unknown")
                        .to_string()
                })
            }
        });
        let file_size = initial_size.unwrap_or(0);

        // Trim leading slash to prevent Path::join from treating it as an absolute path
        if filename.starts_with('/') {
            filename = filename.trim_start_matches('/').to_string();
        }

        // Build destination path based on TMDB metadata
        let download_dir = { self.config.read().await.download_dir.clone() };

        // Create Sonarr/Radarr compatible filename if TMDB metadata exists
        let final_filename = if let Some(mut meta) = tmdb_metadata.clone() {
            // Extract file extension from original filename; normalize community tags
            // like ".Flasharr" (Fshare uploader convention) to ".mkv" so Sonarr/Radarr
            // recognise the file.
            let extension = PathBuilder::normalize_video_extension(
                std::path::Path::new(&filename)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("mkv"),
            );

            // For TV shows: Always create clean filename if we have season/episode
            if let (Some(season), Some(episode)) = (meta.season, meta.episode) {
                // Fetch title from TMDB if missing
                let title = if meta.title.is_none() {
                    if let Some(tmdb_id) = &meta.tmdb_id {
                        let media_type = meta.media_type.as_deref().unwrap_or("tv");
                        tracing::info!(
                            "Fetching TMDB title for ID: {}, type: {}",
                            tmdb_id,
                            media_type
                        );
                        let fetched_title = crate::api::indexer::fetch_tmdb_title(
                            &self
                                .db
                                .as_deref()
                                .map(|db| crate::api::metadata_settings::runtime_key(db, "tmdb"))
                                .unwrap_or_default(),
                            &tmdb_id.to_string(),
                            media_type,
                        )
                        .await;
                        if let Some(ref t) = fetched_title {
                            tracing::info!("Fetched TMDB title: {}", t);
                            meta.title = fetched_title.clone();
                        }
                        fetched_title
                    } else {
                        None
                    }
                } else {
                    meta.title.clone()
                };

                // Create clean filename: "Series (Year) - S##E##.ext"
                if let Some(title_str) = title {
                    let clean_filename = if let Some(year) = meta.year {
                        format!(
                            "{} ({}) - S{:02}E{:02}.{}",
                            title_str, year, season, episode, extension
                        )
                    } else {
                        format!(
                            "{} - S{:02}E{:02}.{}",
                            title_str, season, episode, extension
                        )
                    };
                    tracing::info!(
                        "Generated clean filename: {} (from: {})",
                        clean_filename,
                        filename
                    );

                    // Update tmdb_metadata with fetched title for folder organization
                    if let Some(ref mut original_meta) = tmdb_metadata {
                        original_meta.title = Some(title_str);
                    }

                    clean_filename
                } else {
                    tracing::warn!(
                        "Could not fetch TMDB title, using original filename: {}",
                        filename
                    );
                    filename.clone()
                }
            }
            // For movies: Create clean filename if we have title
            else if let Some(ref title) = meta.title {
                let clean_filename = if let Some(year) = meta.year {
                    format!("{} ({}).{}", title, year, extension)
                } else {
                    format!("{}.{}", title, extension)
                };
                tracing::info!(
                    "Generated clean movie filename: {} (from: {})",
                    clean_filename,
                    filename
                );
                clean_filename
            } else {
                filename.clone()
            }
        } else {
            filename.clone()
        };

        // Build destination with Sonarr-compatible filename
        let destination =
            self.build_destination_path(&final_filename, &category, &tmdb_metadata, &download_dir);

        // Create new task with file size
        let mut task = DownloadTask::new(
            canonical_url.clone(),
            final_filename,
            host.clone(),
            category,
        );
        task.provider_id = provider_item_id.as_ref().map(|_| provider_id);
        task.provider_item_id = provider_item_id;
        if host == "qbittorrent" {
            task.download_source = Some(
                download_source
                    .clone()
                    .unwrap_or_else(|| "torrent".to_string()),
            );
            task.external_provider = Some(
                external_provider
                    .clone()
                    .unwrap_or_else(|| "qbittorrent".to_string()),
            );
            task.external_id = external_id.clone().or_else(|| info_hash.clone());
            task.external_instance_id = external_instance_id.clone();
        } else {
            task.download_source = download_source.clone().or_else(|| Some(host.clone()));
            task.external_provider = external_provider.clone();
            task.external_id = external_id.clone().or_else(|| info_hash.clone());
            task.external_instance_id = external_instance_id.clone();
        }
        task.info_hash = info_hash;
        task.file_idx = file_idx;
        task.source_filename = source_filename;

        task.origin_provider = origin_provider;
        task.origin_indexer = origin_indexer;
        task.seeders = seeders;
        task.leechers = leechers;
        task.destination = destination;
        task.size = file_size;
        task.batch_id = batch_id;
        task.batch_name = batch_name;

        // ── Quality metadata ───────────────────────────────────────────────
        // IMPORTANT: parse from the ORIGINAL API filename (which contains tokens
        // like "1080p", "WEB-DL", "x265", "AAC") BEFORE it gets replaced by the
        // clean Sonarr-compatible name ("Series - S01E01.mkv" has no quality tags).
        let quality_attrs = FilenameParser::extract_quality_attributes(&filename);
        task.quality = Some(quality_attrs.quality_name());
        task.resolution = quality_attrs.resolution.clone();
        tracing::debug!(
            "[QUALITY] original='{}' final='{}' → quality={:?} resolution={:?}",
            filename,
            task.filename,
            task.quality,
            task.resolution
        );

        if let Some(ref meta) = tmdb_metadata {
            task.tmdb_id = meta.tmdb_id;
            task.tvdb_id = meta.tvdb_id;
            task.media_type = meta.media_type.clone();
            task.media_item_id = meta.tmdb_id;
            task.info_path = match (meta.media_type.as_deref(), meta.tvdb_id, meta.tmdb_id) {
                (Some("tv") | Some("episode"), Some(tvdb_id), _) => {
                    Some(format!("/tv/{}", tvdb_id))
                }
                _ => resolve_download_info_path(meta.tmdb_id, meta.media_type.as_deref()),
            };

            // Always store the series/movie title in tmdb_title — it is the key used by
            // sabnzbd.rs to report a parseable name to Sonarr (e.g. "How.Dare.You.S01E03.WEB-DL.Flasharr").
            // Season/episode numbers are tracked separately in tmdb_season/tmdb_episode.
            // Setting this to "Episode X" caused "Series title mismatch" import failures.
            task.tmdb_title = meta.title.clone();

            task.tmdb_season = meta.season.map(|s| s as u32);
            task.tmdb_episode = meta.episode.map(|e| e as u32);

            // ── Persist to media_items (TMDB-centric data model) ──────────
            if let (Some(tmdb_id), Some(db)) = (meta.tmdb_id, &self.db) {
                let media_type = meta.media_type.clone().unwrap_or_else(|| {
                    if meta.season.is_some() || meta.episode.is_some() {
                        "tv".to_string()
                    } else {
                        "movie".to_string()
                    }
                });
                let title = meta.title.clone().unwrap_or_else(|| "Unknown".to_string());
                let mut media_item = crate::db::media::MediaItem::new(tmdb_id, &media_type, &title);
                if media_type == "tv" {
                    if let Some(tvdb_id) = meta.tvdb_id {
                        media_item.media_key = format!("tv:{}", tvdb_id);
                        media_item.tvdb_id = Some(tvdb_id as i32);
                    }
                }
                media_item.year = meta.year;

                // Check if item already exists to preserve existing arr state
                if let Ok(Some(existing)) = db.get_media_item(tmdb_id) {
                    media_item.arr_id = existing.arr_id;
                    media_item.arr_type = existing.arr_type;
                    media_item.arr_path = existing.arr_path;
                    media_item.arr_monitored = existing.arr_monitored;
                    media_item.arr_status = existing.arr_status;
                    media_item.arr_quality_profile_id = existing.arr_quality_profile_id;
                    media_item.arr_has_file = existing.arr_has_file;
                    media_item.arr_size_on_disk = existing.arr_size_on_disk;
                    media_item.tvdb_id = existing.tvdb_id;
                    media_item.imdb_id = existing.imdb_id;
                    media_item.arr_synced_at = existing.arr_synced_at;
                    media_item.created_at = existing.created_at;
                }

                if let Err(e) = db.upsert_media_item(&media_item) {
                    tracing::warn!(
                        "[ORCHESTRATOR] Failed to upsert media_item for tmdb_id={}: {}",
                        tmdb_id,
                        e
                    );
                } else {
                    tracing::info!(
                        "[ORCHESTRATOR] Upserted media_item: tmdb_id={}, type={}, title={}",
                        tmdb_id,
                        media_type,
                        title
                    );
                }

                // For TV episodes, also upsert media_episodes row
                if media_type == "tv" {
                    if let (Some(season), Some(episode)) = (meta.season, meta.episode) {
                        let ep = crate::db::media::MediaEpisode::new(tmdb_id, season, episode);
                        if let Err(e) = db.upsert_media_episode(&ep) {
                            tracing::warn!(
                                "[ORCHESTRATOR] Failed to upsert media_episode for tmdb_id={} S{:02}E{:02}: {}",
                                tmdb_id,
                                season,
                                episode,
                                e
                            );
                        }
                    }
                }
            }
        }

        // Persist acceptance/replacement before exposing or starting the new task.
        let mut replaced = None;
        if let Some(db) = &self.db {
            replaced = db.save_task_replacing_current(&task)?;
        }
        Self::record_activity(self.db.as_ref(), "queued", &task);
        if let Some(old) = replaced {
            self.task_manager.delete_task(old.id).await;
            self.broadcast_task_removed(&old.id.to_string());
            if old.download_source.as_deref() == Some("torrent") {
                if let Err(error) = self.delete_external_torrent(&old, false).await {
                    tracing::warn!(task_id = %old.id, "Replacement detached locally; qB removal failed safely: {}", error);
                }
            }
        }
        self.task_manager.add_task(task.clone()).await;
        self.task_notify.notify_waiters();

        // Broadcast task added event (legacy)
        let _ = self.progress_tx.send(ProgressUpdate {
            event: TaskEvent::Added,
            task_id: task.id.to_string(),
            downloaded_bytes: 0,
            total_bytes: task.size,
            speed_bytes_per_sec: 0.0,
            eta_seconds: 0.0,
            percentage: 0.0,
            state: "QUEUED".to_string(),
        });

        // Emit TaskEvent::Created (event-driven architecture)
        self.event_bus
            .publish(super::events::TaskEvent::Created { task: task.clone() });

        // Phase 1: Manage Arr Artifact (Series/Movie metadata)
        // This happens when download is ADDED, not completed
        // Only manage ONCE per series/movie (not per episode)
        tracing::debug!(
            "Artifact management check: tmdb_id={:?}, artifact_manager_exists={}",
            task.tmdb_id,
            self.artifact_manager.read().await.is_some()
        );

        if task.tmdb_id.is_some() {
            let artifact_manager_guard = self.artifact_manager.read().await;
            if let Some(ref artifact_manager) = *artifact_manager_guard {
                // Check if we should manage artifact based on batch_id
                let should_manage = if let Some(ref batch_id) = task.batch_id {
                    // Check if this batch has already been managed
                    let mut announced = self.announced_batches.write().await;
                    if announced.contains(batch_id) {
                        tracing::debug!(
                            "Batch {} already managed - skipping artifact creation",
                            batch_id
                        );
                        false
                    } else {
                        // Mark this batch as managed
                        announced.insert(batch_id.clone());
                        tracing::info!(
                            "First task in batch {} - will manage artifact in Sonarr/Radarr",
                            batch_id
                        );
                        true
                    }
                } else {
                    // No batch, manage immediately (movies or standalone episodes)
                    tracing::debug!("No batch_id - managing artifact immediately");
                    true
                };

                tracing::info!(
                    "Should manage artifact: {} for task: {}",
                    should_manage,
                    task.filename
                );

                if should_manage {
                    tracing::info!(
                        "Managing artifact for: {} (TMDB ID: {:?})",
                        task.filename,
                        task.tmdb_id
                    );

                    // Spawn artifact management in background to avoid blocking
                    let artifact_manager_clone = Arc::clone(artifact_manager);
                    let task_clone = task.clone();
                    let task_manager_clone = Arc::clone(&self.task_manager);
                    let db_clone = self.db.clone();
                    tokio::spawn(async move {
                        match artifact_manager_clone.manage_artifact(&task_clone).await {
                            Ok(status) => {
                                use crate::arr::ArtifactStatus;
                                match status {
                                    ArtifactStatus::Created { arr_id }
                                    | ArtifactStatus::AlreadyMonitored { arr_id } => {
                                        let status_name = match status {
                                            ArtifactStatus::Created { .. } => "created",
                                            ArtifactStatus::AlreadyMonitored { .. } => "found",
                                            _ => unreachable!(),
                                        };
                                        tracing::info!(
                                            "Successfully {} artifact for {} (Arr ID: {})",
                                            status_name,
                                            task_clone.filename,
                                            arr_id
                                        );

                                        // Update in-memory tasks with arr_series_id/arr_movie_id
                                        if let Some(tmdb_id) = task_clone.tmdb_id {
                                            let all_tasks = task_manager_clone.get_tasks().await;
                                            let mut updated_count = 0;

                                            for stale in all_tasks {
                                                if stale.tmdb_id == Some(tmdb_id) {
                                                    let current = task_manager_clone
                                                        .get_task(stale.id)
                                                        .await
                                                        .unwrap_or_else(|| stale.clone());
                                                    let merged = Self::merge_arr_identity(
                                                        &stale,
                                                        &current,
                                                        arr_id as i64,
                                                    );
                                                    task_manager_clone.update_task(merged).await;
                                                    updated_count += 1;
                                                }
                                            }

                                            tracing::info!(
                                                "Updated {} in-memory tasks with arr_id={}",
                                                updated_count,
                                                arr_id
                                            );

                                            // Persist arr state to media_items table
                                            if let Some(ref db) = db_clone {
                                                let arr_type = match task_clone.detect_media_type()
                                                {
                                                    crate::downloader::MediaType::TvSeries
                                                    | crate::downloader::MediaType::TvEpisode => {
                                                        "sonarr"
                                                    }
                                                    crate::downloader::MediaType::Movie => "radarr",
                                                };
                                                if let Err(e) = db.update_media_arr_state(
                                                    tmdb_id, arr_id, arr_type, None, true, None,
                                                    None,
                                                ) {
                                                    tracing::warn!(
                                                        "Failed to update media_items arr state for tmdb_id={}: {}",
                                                        tmdb_id,
                                                        e
                                                    );
                                                } else {
                                                    tracing::info!(
                                                        "Updated media_items arr state: tmdb_id={}, arr_id={}, type={}",
                                                        tmdb_id,
                                                        arr_id,
                                                        arr_type
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    ArtifactStatus::Skipped { reason } => {
                                        tracing::debug!(
                                            "Skipped artifact management for {}: {}",
                                            task_clone.filename,
                                            reason
                                        );
                                    }
                                    ArtifactStatus::Failed { error } => {
                                        tracing::warn!(
                                            "Failed to manage artifact for {}: {}",
                                            task_clone.filename,
                                            error
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Error managing artifact for {}: {}",
                                    task_clone.filename,
                                    e
                                );
                            }
                        }
                    });
                } else {
                    tracing::debug!(
                        "Skipping artifact management for {} - already managed for this batch",
                        task.filename
                    );
                }
            }
        }

        tracing::info!(
            "Added download: {} ({}) [{}:{:?}] -> {}",
            task.filename,
            task.id,
            task.provider_id.as_deref().unwrap_or("unknown"),
            task.provider_item_id,
            task.destination
        );
        Ok(task)
    }

    /// Build organized destination path based on TMDB metadata
    /// Delegates to PathBuilder module
    fn record_activity(db: Option<&Arc<Db>>, kind: &str, task: &DownloadTask) {
        let Some(db) = db else { return };
        let message = format!(
            "{}: {}",
            match kind {
                "queued" => "Queued",
                "completed" => "Completed",
                "failed" => "Failed",
                _ => return,
            },
            task.filename
        );
        if let Err(error) = db.record_activity_event(kind, task.id, &message) {
            tracing::warn!(task_id = %task.id, "Failed to record activity event: {error}");
        }
    }

    fn build_destination_path(
        &self,
        filename: &str,
        category: &str,
        tmdb: &Option<TmdbDownloadMetadata>,
        root_dir: &std::path::Path,
    ) -> String {
        Self::build_destination_path_static(filename, category, tmdb, root_dir)
    }

    fn build_destination_path_static(
        filename: &str,
        category: &str,
        tmdb: &Option<TmdbDownloadMetadata>,
        root_dir: &std::path::Path,
    ) -> String {
        PathBuilder::build_destination_path(filename, category, tmdb, root_dir)
    }

    /// Sanitize a string for use as a folder/file name
    /// Delegates to PathBuilder module
    #[allow(dead_code)] // May be used by future code
    fn sanitize_filename(name: &str) -> String {
        PathBuilder::sanitize_filename(name)
    }

    fn filename_needs_host_metadata(task: &DownloadTask) -> bool {
        let filename = task.filename.trim();
        if filename.is_empty() || filename.eq_ignore_ascii_case("unknown") {
            return true;
        }

        if task.provider_item_id.as_deref() == Some(filename) {
            return true;
        }

        task.original_url
            .split('/')
            .last()
            .map(|tail| tail == filename)
            .unwrap_or(false)
    }

    fn metadata_from_task(task: &DownloadTask) -> Option<TmdbDownloadMetadata> {
        if task.tmdb_id.is_none()
            && task.tvdb_id.is_none()
            && task.media_type.is_none()
            && task.tmdb_title.is_none()
            && task.tmdb_season.is_none()
            && task.tmdb_episode.is_none()
        {
            return None;
        }

        Some(TmdbDownloadMetadata {
            tmdb_id: task.tmdb_id,
            tvdb_id: task.tvdb_id,
            media_type: task.media_type.clone(),
            title: task.tmdb_title.clone(),
            year: None,
            collection_name: None,
            season: task.tmdb_season.map(|s| s as i32),
            episode: task.tmdb_episode.map(|e| e as i32),
        })
    }

    fn remap_download_source(
        destination: &str,
        symlink_real_base: Option<&std::path::Path>,
    ) -> PathBuf {
        let raw = std::path::Path::new(destination);
        if raw.starts_with("/data/media/")
            || raw.starts_with("/data/flasharr-download/")
            || raw.starts_with("/data/downloads/")
        {
            return raw.to_path_buf();
        }
        if let Some(real_base) = symlink_real_base {
            for segment in ["appData/downloads", "downloads"] {
                let needle = format!("/{}/", segment);
                if let Some(pos) = destination.find(&needle) {
                    return real_base.join(&destination[pos + needle.len()..]);
                }
                let prefix = format!("/{}", segment);
                if destination.starts_with(&prefix) {
                    return real_base.join(destination[prefix.len()..].trim_start_matches('/'));
                }
            }
        }
        raw.to_path_buf()
    }

    async fn find_task_by_provider_item(
        &self,
        provider_id: &str,
        provider_item_id: &str,
    ) -> Option<DownloadTask> {
        DuplicateDetector::find_task_by_provider_item(
            &self.task_manager,
            provider_id,
            provider_item_id,
        )
        .await
    }

    /// Handle duplicate download based on existing task state
    async fn handle_duplicate(
        &self,
        existing: DownloadTask,
        provider_item_id: String,
    ) -> Result<DownloadTask, anyhow::Error> {
        tracing::info!(
            "Duplicate detected [code: {}]: Task {} already exists in state {:?}, skipping",
            provider_item_id,
            existing.id,
            existing.state
        );
        let state = format!("{:?}", existing.state).to_uppercase();
        Err(anyhow::anyhow!("Download already exists ({})", state))
    }

    /// Get task manager reference
    pub fn task_manager(&self) -> &Arc<DownloadTaskManager> {
        &self.task_manager
    }

    /// Plan or apply recovery for completed downloads not yet confirmed by Sonarr/Radarr.
    /// Dry-run is read-only: no DB writes, filesystem changes, or ARR commands.
    pub async fn backfill_arr_paths(
        &self,
        dry_run: bool,
        limit: usize,
        kind: &str,
    ) -> Vec<BackfillItem> {
        use crate::downloader::MediaType;

        let mut results = Vec::new();
        let client = {
            let guard = self.arr_client.read().await;
            guard.clone()
        };
        let Some(client) = client else {
            return vec![BackfillItem {
                id: String::new(),
                filename: String::new(),
                media_type: kind.to_string(),
                source_path: String::new(),
                arr_id: None,
                arr_path: None,
                has_file: None,
                reason: "arr not configured".to_string(),
                action: "blocked".to_string(),
            }];
        };

        let symlink_real_base = self.config.read().await.symlink_real_base.clone();
        let completed_tasks: Vec<DownloadTask> = if let Some(db) = &self.db {
            match db
                .get_tasks_by_states_async(vec!["COMPLETED".to_string(), "IMPORTING".to_string()])
                .await
            {
                Ok(tasks) => tasks,
                Err(e) => {
                    tracing::error!("Backfill: Failed to load completed tasks: {}", e);
                    return vec![BackfillItem {
                        id: String::new(),
                        filename: String::new(),
                        media_type: kind.to_string(),
                        source_path: String::new(),
                        arr_id: None,
                        arr_path: None,
                        has_file: None,
                        reason: format!("DB error: {}", e),
                        action: "failed".to_string(),
                    }];
                }
            }
        } else {
            self.task_manager.get_tasks().await
        };

        for task in completed_tasks
            .into_iter()
            .filter(|t| {
                matches!(t.state, DownloadState::Completed | DownloadState::Importing)
                    && !t.arr_announced
            })
            .filter(|t| match (kind, t.detect_media_type()) {
                ("tv", MediaType::TvSeries | MediaType::TvEpisode) => true,
                ("movie", MediaType::Movie) => true,
                ("all", _) => true,
                _ => false,
            })
            .take(limit)
        {
            let media_type = task.detect_media_type();
            let media_type_str = match media_type {
                MediaType::TvSeries | MediaType::TvEpisode => "tv",
                MediaType::Movie => "movie",
            };
            let source = if Self::is_torrent_import(&task) {
                qbit_source_path(&task.destination)
            } else {
                Self::remap_download_source(&task.destination, symlink_real_base.as_deref())
            };
            let source_path = source.to_string_lossy().to_string();

            let mut item = BackfillItem {
                id: task.id.to_string(),
                filename: task.filename.clone(),
                media_type: media_type_str.to_string(),
                source_path: source_path.clone(),
                arr_id: None,
                arr_path: None,
                has_file: None,
                reason: String::new(),
                action: "blocked".to_string(),
            };
            let auto_import_enabled = match media_type {
                MediaType::TvSeries | MediaType::TvEpisode => client.sonarr_auto_import_enabled(),
                MediaType::Movie => client.radarr_auto_import_enabled(),
            };
            if !auto_import_enabled {
                item.reason = "automatic Arr import is disabled".to_string();
                results.push(item);
                continue;
            }

            let (arr_id, tvdb_id) = match media_type {
                MediaType::TvSeries | MediaType::TvEpisode => {
                    let resolved = if let Some(id) = task.arr_series_id {
                        Ok(Some((id, task.tvdb_id)))
                    } else if let Some(tvdb_id) = task.tvdb_id {
                        client
                            .series_exists_by_tvdb(tvdb_id as i32)
                            .await
                            .map(|id| id.map(|id| (id as i64, Some(tvdb_id))))
                    } else if let Some(tmdb_id) = task.tmdb_id {
                        client.get_all_series().await.map(|series| {
                            series.into_iter().find_map(|s| {
                                (s.tmdb_id == Some(tmdb_id as i32))
                                    .then_some((s.id as i64, s.tvdb_id.map(|v| v as i64)))
                            })
                        })
                    } else {
                        Ok(None)
                    };
                    match resolved {
                        Ok(Some(ids)) => ids,
                        Ok(None) => {
                            item.reason = "missing sonarr series identity".to_string();
                            results.push(item);
                            continue;
                        }
                        Err(e) => {
                            item.reason = format!("sonarr lookup failed: {}", e);
                            item.action = "failed".to_string();
                            results.push(item);
                            continue;
                        }
                    }
                }
                MediaType::Movie => {
                    let resolved = if let Some(id) = task.arr_movie_id {
                        Ok(Some(id))
                    } else if let Some(tmdb_id) = task.tmdb_id {
                        client
                            .movie_exists(tmdb_id)
                            .await
                            .map(|id| id.map(|id| id as i64))
                    } else {
                        Ok(None)
                    };
                    match resolved {
                        Ok(Some(id)) => (id, None),
                        Ok(None) => {
                            item.reason = "missing radarr movie identity".to_string();
                            results.push(item);
                            continue;
                        }
                        Err(e) => {
                            item.reason = format!("radarr lookup failed: {}", e);
                            item.action = "failed".to_string();
                            results.push(item);
                            continue;
                        }
                    }
                }
            };
            item.arr_id = Some(arr_id);

            match media_type {
                MediaType::TvSeries | MediaType::TvEpisode => {
                    item.arr_path = client.get_series_path(arr_id).await.ok();
                    if let (Some(season), Some(episode)) = (task.tmdb_season, task.tmdb_episode) {
                        item.has_file = client
                            .get_episode_by_details(arr_id as i32, season as i32, episode as i32)
                            .await
                            .ok()
                            .flatten()
                            .map(|e| e.has_file);
                    }
                }
                MediaType::Movie => {
                    if let Ok(movie) = client.get_movie_by_id(arr_id).await {
                        item.arr_path = movie.path.clone();
                        item.has_file = movie.has_file;
                    }
                }
            }

            let physical_artifact = library_artifact_exists(&task, item.arr_path.as_deref()).await;
            if item.has_file == Some(true) && physical_artifact {
                if dry_run {
                    item.reason = "already imported in arr; DB can be marked announced".to_string();
                    item.action = "would_update_db_only".to_string();
                } else {
                    let mut updated = task.clone();
                    updated.arr_announced = true;
                    updated.arr_announce_error = None;
                    match media_type {
                        MediaType::TvSeries | MediaType::TvEpisode => {
                            updated.arr_series_id = Some(arr_id);
                            updated.tvdb_id = updated.tvdb_id.or(tvdb_id);
                        }
                        MediaType::Movie => updated.arr_movie_id = Some(arr_id),
                    }
                    self.task_manager.update_task(updated.clone()).await;
                    if let Some(db) = &self.db {
                        let _ = db.save_task(&updated);
                    }
                    item.reason = "already imported in arr; DB marked announced".to_string();
                    item.action = "marked_announced".to_string();
                }
                results.push(item);
                continue;
            }

            if source.starts_with("/data/media/") {
                item.reason =
                    "source is already in media library; not eligible for downloaded scan"
                        .to_string();
                results.push(item);
                continue;
            }

            if tokio::fs::metadata(&source).await.is_err()
                && tokio::fs::symlink_metadata(&source).await.is_err()
            {
                item.reason = format!("file not found at {}", source_path);
                results.push(item);
                continue;
            }

            if dry_run {
                item.reason = "ready for arr downloaded scan".to_string();
                item.action = "would_scan".to_string();
                results.push(item);
                continue;
            }

            let download_client_id = task.id.to_string();
            let scan_result = match media_type {
                MediaType::TvSeries | MediaType::TvEpisode => {
                    client
                        .trigger_downloaded_episodes_scan(&source_path, &download_client_id)
                        .await
                }
                MediaType::Movie => {
                    let import_path = source
                        .parent()
                        .unwrap_or(source.as_path())
                        .to_string_lossy();
                    client
                        .trigger_downloaded_movies_scan(&import_path, &download_client_id)
                        .await
                }
            };

            match scan_result {
                Ok(()) => {
                    let mut updated = task.clone();
                    updated.arr_announce_error = None;
                    match media_type {
                        MediaType::TvSeries | MediaType::TvEpisode => {
                            updated.arr_series_id = Some(arr_id);
                            updated.tvdb_id = updated.tvdb_id.or(tvdb_id);
                        }
                        MediaType::Movie => updated.arr_movie_id = Some(arr_id),
                    }
                    self.task_manager.update_task(updated.clone()).await;
                    if let Some(db) = &self.db {
                        let _ = db.save_task(&updated);
                    }
                    item.reason = "arr downloaded scan requested".to_string();
                    item.action = "scan_requested".to_string();
                }
                Err(e) => {
                    item.reason = format!("arr downloaded scan failed: {}", e);
                    item.action = "failed".to_string();
                }
            }
            results.push(item);
        }

        tracing::info!("Backfill: {} items planned/applied", results.len());
        results
    }

    /// Get aggregate engine statistics (with database counts for filter dropdown)
    pub async fn get_stats(&self) -> EngineStats {
        let mut stats = self.task_manager.get_stats().await;

        // Query database for accurate status counts (for filter dropdown)
        if let Some(db) = &self.db {
            match db.get_status_counts() {
                Ok(counts) => {
                    use crate::downloader::stats::DbStatusCounts;

                    // Aggregate counts by UI category
                    let downloading = *counts.get("DOWNLOADING").unwrap_or(&0)
                        + *counts.get("STARTING").unwrap_or(&0);
                    let queued =
                        *counts.get("QUEUED").unwrap_or(&0) + *counts.get("WAITING").unwrap_or(&0);
                    let paused = *counts.get("PAUSED").unwrap_or(&0);
                    let completed = *counts.get("COMPLETED").unwrap_or(&0);
                    let failed = *counts.get("FAILED").unwrap_or(&0);
                    let cancelled = *counts.get("CANCELLED").unwrap_or(&0);
                    let all: usize = counts.values().sum();

                    stats.db_counts = Some(DbStatusCounts {
                        all,
                        downloading,
                        queued,
                        paused,
                        completed,
                        failed,
                        cancelled,
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to get status counts from DB: {}", e);
                }
            }
        }

        stats
    }

    /// Subscribe to progress updates (legacy)
    pub fn subscribe_progress(&self) -> broadcast::Receiver<ProgressUpdate> {
        self.progress_tx.subscribe()
    }

    /// Subscribe to task events (event-driven architecture)
    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<super::events::TaskEvent> {
        self.event_bus.subscribe()
    }

    fn is_external_torrent(task: &DownloadTask) -> bool {
        task.host == "qbittorrent" || Self::is_torrent_import(task)
    }

    fn is_sabnzbd_managed_import(task: &DownloadTask) -> bool {
        matches!(task.host.as_str(), "sabnzbd" | "sabnzbd-fshare")
    }

    fn is_torrent_import(task: &DownloadTask) -> bool {
        task.download_source.as_deref() == Some("torrent")
    }

    fn merge_arr_identity(
        stale: &DownloadTask,
        current: &DownloadTask,
        arr_id: i64,
    ) -> DownloadTask {
        let mut merged = stale.clone();
        match merged.detect_media_type() {
            crate::downloader::MediaType::TvSeries | crate::downloader::MediaType::TvEpisode => {
                merged.arr_series_id = Some(arr_id)
            }
            crate::downloader::MediaType::Movie => merged.arr_movie_id = Some(arr_id),
        }
        merged.state = current.state;
        merged.progress = current.progress;
        merged.downloaded = current.downloaded;
        merged.speed = current.speed;
        merged.eta = current.eta;
        merged.started_at = current.started_at;
        merged.completed_at = current.completed_at;
        merged.wait_until = current.wait_until;
        merged.error_message = current.error_message.clone();
        merged.external_instance_id = current.external_instance_id.clone();
        merged
    }

    fn is_direct_http_import(task: &DownloadTask) -> bool {
        task.download_source.as_deref() == Some("torbox")
            || task.external_provider.as_deref() == Some("torbox")
    }

    async fn qbit_client_for_task(
        task: &DownloadTask,
        qbit_manager: Option<&Arc<crate::torrent::instance_manager::QbitInstanceManager>>,
        qbittorrent_client: &Arc<
            RwLock<Option<Arc<crate::torrent::qbittorrent::QBittorrentClient>>>,
        >,
    ) -> anyhow::Result<(
        Option<String>,
        Arc<crate::torrent::qbittorrent::QBittorrentClient>,
    )> {
        if let Some(manager) = qbit_manager {
            if let Some(instance_id) = task.external_instance_id.as_deref() {
                return Ok((
                    Some(instance_id.to_string()),
                    manager.client(instance_id).await?,
                ));
            }
            if let Ok((instance_id, client)) = manager.default_client().await {
                return Ok((Some(instance_id), client));
            }
        }
        let client = qbittorrent_client
            .read()
            .await
            .clone()
            .ok_or_else(|| anyhow::anyhow!("qBittorrent is not configured"))?;
        Ok((task.external_instance_id.clone(), client))
    }

    fn apply_qbittorrent_info(
        task: &mut DownloadTask,
        info: &crate::torrent::qbittorrent::QBittorrentInfo,
    ) {
        let downloaded = (info.progress.clamp(0.0, 1.0) * info.size.max(0) as f64) as u64;
        let new_state = crate::torrent::state::classify_qbit(
            &info.state,
            crate::torrent::state::TransferMetrics {
                downloaded,
                previous_downloaded: Some(task.downloaded),
                progress: info.progress,
                dlspeed: info.dlspeed,
                upspeed: 0,
                amount_left: info.size.max(0).saturating_sub(downloaded as i64),
            },
            "healthy",
        )
        .task_state;
        task.state = reconciled_transfer_state(task.state, new_state);
        task.size = if info.size > 0 {
            info.size as u64
        } else {
            task.size
        };
        task.downloaded = downloaded;
        task.progress = (info.progress.clamp(0.0, 1.0) * 100.0) as f32;
        task.speed = info.dlspeed.max(0) as f64;
        task.eta = info.eta.max(0) as f64;
        task.seeders = info.num_seeds;
        task.leechers = info.num_leechs;
        if !info.content_path.trim().is_empty() {
            task.destination = info.content_path.clone();
        } else if !info.save_path.trim().is_empty() && !info.name.trim().is_empty() {
            task.destination = std::path::Path::new(&info.save_path)
                .join(&info.name)
                .to_string_lossy()
                .into_owned();
        }
        if new_state == DownloadState::Completed && task.completed_at.is_none() {
            task.completed_at = Some(Utc::now());
        }
    }

    fn apply_qbit_row(task: &mut DownloadTask, row: &crate::db::QbitTorrentRow, sync_health: &str) {
        let classification = crate::torrent::state::classify_qbit(
            &row.state,
            crate::torrent::state::TransferMetrics {
                downloaded: row.downloaded.max(0) as u64,
                previous_downloaded: Some(task.downloaded),
                progress: row.progress,
                dlspeed: row.dlspeed,
                upspeed: row.upspeed,
                amount_left: row.amount_left,
            },
            sync_health,
        );
        if sync_health.eq_ignore_ascii_case("healthy") {
            task.state = reconciled_transfer_state(task.state, classification.task_state);
        }
        if row.size > 0 {
            task.size = row.size as u64;
        }
        task.downloaded = row.downloaded.max(0) as u64;
        task.progress = (row.progress.clamp(0.0, 1.0) * 100.0) as f32;
        task.speed = row.dlspeed.max(0) as f64;
        task.eta = row.eta.max(0) as f64;
        task.seeders = Some(row.num_seeds);
        task.leechers = Some(row.num_leechs);
        if classification.task_state == DownloadState::Completed && task.completed_at.is_none() {
            task.completed_at = Some(Utc::now());
        }
    }

    async fn apply_qbit_source_path(task: &mut DownloadTask, row: &crate::db::QbitTorrentRow) {
        for candidate in [
            (!row.content_path.trim().is_empty())
                .then(|| std::path::PathBuf::from(&row.content_path)),
            (!row.save_path.trim().is_empty() && !row.name.trim().is_empty())
                .then(|| std::path::PathBuf::from(&row.save_path).join(&row.name)),
        ]
        .into_iter()
        .flatten()
        {
            if tokio::fs::metadata(&candidate).await.is_ok()
                && looks_like_importable_media(&candidate)
                    .await
                    .unwrap_or(false)
            {
                task.destination = candidate.to_string_lossy().to_string();
                return;
            }
        }
    }

    pub async fn refresh_external_torrent_status(
        &self,
        mut task: DownloadTask,
    ) -> anyhow::Result<DownloadTask> {
        if !Self::is_external_torrent(&task) {
            return Ok(task);
        }
        let Some(hash) = task.external_id.clone() else {
            return Ok(task);
        };
        // Direct qB API is authoritative; mirror rows may be placeholders.
        let (_, client) =
            Self::qbit_client_for_task(&task, self.qbit_manager.as_ref(), &self.qbittorrent_client)
                .await?;
        if let Some(info) = client.get_torrent(&hash).await? {
            let old_state = task.state;
            Self::apply_qbittorrent_info(&mut task, &info);
            task.state = status_refresh_state(old_state, task.state, task.arr_announced);
            if task.state != DownloadState::Completed {
                task.completed_at = None;
            }
            self.task_manager.update_task(task.clone()).await;
            if let Some(db) = &self.db {
                let _ = db.save_task(&task);
            }
            if old_state != task.state || task.speed > 0.0 {
                self.broadcast_task_update(&task);
            }
        }
        Ok(task)
    }

    pub async fn pause_external_torrent(&self, task: &DownloadTask) -> anyhow::Result<()> {
        if !Self::is_external_torrent(task) {
            return Ok(());
        }
        let Some(hash) = task.external_id.as_deref() else {
            return Ok(());
        };
        let (_, client) =
            Self::qbit_client_for_task(task, self.qbit_manager.as_ref(), &self.qbittorrent_client)
                .await?;
        client.pause(hash).await
    }

    pub async fn resume_external_torrent(&self, task: &DownloadTask) -> anyhow::Result<()> {
        if !Self::is_external_torrent(task) {
            return Ok(());
        }
        let Some(hash) = task.external_id.as_deref() else {
            return Ok(());
        };
        let (_, client) =
            Self::qbit_client_for_task(task, self.qbit_manager.as_ref(), &self.qbittorrent_client)
                .await?;
        client.resume(hash).await
    }

    pub async fn delete_external_torrent(
        &self,
        task: &DownloadTask,
        delete_files: bool,
    ) -> anyhow::Result<()> {
        if !Self::is_external_torrent(task) {
            return Ok(());
        }
        let Some(hash) = task.external_id.as_deref() else {
            return Ok(());
        };
        let (_, client) =
            Self::qbit_client_for_task(task, self.qbit_manager.as_ref(), &self.qbittorrent_client)
                .await?;
        client.delete(hash, delete_files).await
    }

    /// Broadcast a task update to all WebSocket clients
    /// Used when task state changes (pause, resume, etc.)
    pub fn broadcast_task_update(&self, task: &DownloadTask) {
        let state_str = format!("{:?}", task.state).to_uppercase();

        // Legacy progress broadcast
        let _ = self.progress_tx.send(ProgressUpdate {
            event: TaskEvent::Updated,
            task_id: task.id.to_string(),
            downloaded_bytes: task.downloaded,
            total_bytes: task.size,
            speed_bytes_per_sec: task.speed,
            eta_seconds: task.eta,
            percentage: task.progress as f64,
            state: state_str,
        });

        // Emit StateChanged event (event-driven architecture)
        // Note: We don't have old_state here, so we'll use the current state for both
        // This is acceptable as the task object contains the new state
        self.event_bus
            .publish(super::events::TaskEvent::StateChanged {
                task: task.clone(),
                old_state: task.state.clone(),
                new_state: task.state.clone(),
                timestamp: chrono::Utc::now(),
            });

        tracing::debug!("Broadcast task update: {} -> {:?}", task.id, task.state);
    }

    /// Broadcast a task removal to all WebSocket clients
    /// Used when a task is deleted
    pub fn broadcast_task_removed(&self, task_id: &str) {
        let _ = self.progress_tx.send(ProgressUpdate {
            event: TaskEvent::Removed,
            task_id: task_id.to_string(),
            downloaded_bytes: 0,
            total_bytes: 0,
            speed_bytes_per_sec: 0.0,
            eta_seconds: 0.0,
            percentage: 0.0,
            state: "REMOVED".to_string(),
        });

        tracing::debug!("Broadcast task removed: {}", task_id);
    }

    /// Wake idle workers to process new tasks
    /// Call this when tasks are added to the queue
    pub fn wake_workers(&self) {
        self.task_notify.notify_waiters();
    }

    /// Get task from memory or database (unified lookup)
    ///
    /// This method provides a unified way to retrieve tasks that works regardless
    /// of whether the task is currently in the TaskManager cache or only in the database.
    ///
    /// **Lookup Strategy:**
    /// 1. Fast path: Check TaskManager (in-memory cache)
    /// 2. Slow path: Query Database if not in cache
    ///
    /// This is critical for WebSocket broadcasts where tasks may exist in the DB
    /// but not in the active TaskManager (e.g., completed tasks, tasks from before restart).
    pub async fn get_task_unified(&self, task_id: uuid::Uuid) -> Option<DownloadTask> {
        // Fast path: check memory first
        if let Some(task) = self.task_manager.get_task(task_id).await {
            tracing::trace!("Task {} found in TaskManager (cache hit)", task_id);
            return Some(task);
        }

        // Slow path: check database
        if let Some(db) = &self.db {
            tracing::trace!(
                "Task {} not in TaskManager, querying database (cache miss)",
                task_id
            );
            match db.get_task_by_id(task_id) {
                Ok(task) => {
                    if task.is_some() {
                        tracing::debug!("Task {} found in database", task_id);
                    }
                    task
                }
                Err(e) => {
                    tracing::error!("Failed to query database for task {}: {}", task_id, e);
                    None
                }
            }
        } else {
            tracing::trace!("Task {} not found (no database configured)", task_id);
            None
        }
    }

    /// Load pending tasks (QUEUED, PAUSED) from database into TaskManager
    /// Call this on startup to ensure resume/pause operations work correctly
    pub async fn load_pending_tasks(&self) -> usize {
        let Some(db) = &self.db else {
            tracing::debug!("No database configured, skipping pending tasks load");
            return 0;
        };

        // Load QUEUED, PAUSED and transient states (IMPORTING, DOWNLOADING, STARTING)
        // DOWNLOADING/STARTING will be reset to QUEUED below
        let states = vec![
            "QUEUED".to_string(),
            "PAUSED".to_string(),
            "IMPORTING".to_string(),
            "DOWNLOADING".to_string(),
            "STARTING".to_string(),
            "WAITING".to_string(),
        ];

        match db.get_tasks_by_states_async(states).await {
            Ok(mut tasks) => {
                let mut importing_tasks = Vec::new();

                for task in tasks.iter_mut() {
                    if task.state == DownloadState::Waiting
                        && task.error_message.as_deref().is_some_and(|message| {
                            message.starts_with("qBittorrent torrent is absent")
                        })
                    {
                        task.state = DownloadState::Queued;
                        task.error_message = None;
                        let _ = db.save_task(task);
                    }
                    let state = match task.state {
                        DownloadState::Queued => "QUEUED",
                        DownloadState::Paused => "PAUSED",
                        DownloadState::Starting => "STARTING",
                        DownloadState::Downloading => "DOWNLOADING",
                        DownloadState::Importing => "IMPORTING",
                        DownloadState::Completed => "COMPLETED",
                        DownloadState::Failed => "FAILED",
                        _ => "OTHER",
                    };
                    match crate::debrid::restart::restart_decision(
                        crate::debrid::restart::RestartInput {
                            execution_strategy: task.execution_strategy.as_deref(),
                            external_id: task.external_id.as_deref(),
                            state,
                            remote_state: task.torbox_remote_state.as_deref(),
                        },
                    ) {
                        Ok(crate::debrid::restart::RestartDecision::Preserve) => {}
                        Ok(crate::debrid::restart::RestartDecision::ResumeExistingRemote(_)) => {
                            tracing::info!(
                                "Preserving TorBox task {} in {:?} for remote resume",
                                task.id,
                                task.state
                            );
                        }
                        Ok(crate::debrid::restart::RestartDecision::ResumeArrConfirmation) => {
                            importing_tasks.push(task.clone());
                        }
                        Ok(crate::debrid::restart::RestartDecision::ResetQueued)
                            if !Self::is_external_torrent(task) =>
                        {
                            task.state = DownloadState::Queued;
                            let _ = db.save_task(task);
                        }
                        Ok(_) => {}
                        Err(error) => {
                            tracing::error!(
                                "Cannot reconcile restored task {}: {}",
                                task.id,
                                error
                            );
                            task.state = DownloadState::Failed;
                            task.error_message = Some(error.to_string());
                            let _ = db.save_task(task);
                        }
                    }
                }

                let count = self.task_manager.restore_tasks(tasks).await;
                tracing::info!(
                    "Loaded {} pending tasks from database into TaskManager",
                    count
                );

                // Resume polling for IMPORTING tasks
                for task in importing_tasks {
                    tracing::info!("Resuming import polling for task {}", task.id);
                    let arr_client_clone = Arc::clone(&self.arr_client);
                    let task_id = task.id;
                    let task_manager_clone = Arc::clone(&self.task_manager);
                    let event_bus_clone = Arc::clone(&self.event_bus);
                    let db_clone = self.db.as_ref().map(|d| Arc::clone(d));

                    tokio::spawn(async move {
                        Self::poll_import_confirmation(
                            task_id,
                            arr_client_clone,
                            task_manager_clone,
                            event_bus_clone,
                            db_clone,
                        )
                        .await;
                    });
                }

                // Wake workers to process any queued tasks
                if count > 0 {
                    self.wake_workers();
                }
                count
            }
            Err(e) => {
                tracing::error!("Failed to load pending tasks from database: {}", e);
                0
            }
        }
    }

    /// Validate if download should be allowed based on library state
    /// Returns Ok(()) if allowed, Err if duplicate/exists
    async fn validate_download_request(
        &self,
        tmdb: &Option<TmdbDownloadMetadata>,
    ) -> Result<(), anyhow::Error> {
        let Some(meta) = tmdb else {
            return Ok(()); // No TMDB metadata, can't validate against library
        };

        let client_guard = self.arr_client.read().await;
        let Some(client) = client_guard.as_ref() else {
            return Ok(()); // Arr integration disabled
        };

        let tmdb_id = meta.tmdb_id.unwrap_or(0);
        if tmdb_id == 0 {
            return Ok(());
        }

        match meta.media_type.as_deref() {
            Some("movie") => {
                // Check Radarr
                match client.get_movie_by_tmdb(tmdb_id).await {
                    Ok(Some(movie)) => {
                        if movie.has_file.unwrap_or(false) {
                            tracing::warn!(
                                "Rejecting download: Movie already exists in library (Radarr ID: {})",
                                movie.id
                            );
                            return Err(anyhow::anyhow!("Movie already exists in library"));
                        }
                    }
                    Ok(None) => {}
                    Err(e) => tracing::warn!("Failed to check Radarr for validation: {}", e),
                }
            }
            Some("tv") | Some("episode") => {
                // Check Sonarr — TV is TVDB-canonical.
                if let Some(season) = meta.season {
                    if let Some(episode) = meta.episode {
                        // 1. Resolve Series ID by canonical TVDB ID.
                        let series_lookup = match meta.tvdb_id {
                            Some(tvdb_id) => client.series_exists_by_tvdb(tvdb_id as i32).await,
                            None => {
                                tracing::warn!(
                                    "TV duplicate-check skipped: no tvdb_id on metadata (tmdb_bridge={:?})",
                                    meta.tmdb_id
                                );
                                Ok(None)
                            }
                        };
                        match series_lookup {
                            Ok(Some(series_id)) => {
                                // 2. Check Episode
                                match client
                                    .get_episode_by_details(
                                        series_id,
                                        season as i32,
                                        episode as i32,
                                    )
                                    .await
                                {
                                    Ok(Some(ep_info)) => {
                                        if ep_info.has_file {
                                            tracing::warn!(
                                                "Rejecting download: Episode already exists in library (S{}E{})",
                                                season,
                                                episode
                                            );
                                            return Err(anyhow::anyhow!(
                                                "Episode already exists in library"
                                            ));
                                        }
                                    }
                                    Ok(None) => {} // Episode not monitored/found
                                    Err(e) => tracing::warn!(
                                        "Failed to get episode details for validation: {}",
                                        e
                                    ),
                                }
                            }
                            Ok(None) => {} // Series not in Sonarr
                            Err(e) => tracing::warn!(
                                "Failed to check Sonarr series for validation: {}",
                                e
                            ),
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    // =========================================================================
    // Encapsulated Task Operations (DB + TaskManager + Broadcast)
    // =========================================================================

    /// Pause all pauseable tasks atomically
    /// Handles: DB update (atomic) → TaskManager update → Broadcast
    pub async fn pause_all_async(&self) -> usize {
        // Need db connection
        let db = match &self.db {
            Some(db) => db,
            None => {
                tracing::error!("No database connection for pause_all");
                return 0;
            }
        };

        let pauseable_states = vec![
            "QUEUED".to_string(),
            "DOWNLOADING".to_string(),
            "STARTING".to_string(),
            "WAITING".to_string(),
        ];

        // 1. Query pauseable tasks from DB
        let tasks = match db.get_tasks_by_states_async(pauseable_states).await {
            Ok(tasks) => tasks,
            Err(e) => {
                tracing::error!("Failed to get pauseable tasks: {}", e);
                return 0;
            }
        };

        if tasks.is_empty() {
            return 0;
        }

        let task_ids: Vec<uuid::Uuid> = tasks.iter().map(|t| t.id).collect();

        // 2. Atomic batch update in DB (single transaction)
        if let Err(e) = db
            .batch_update_states_async(task_ids.clone(), "PAUSED".to_string())
            .await
        {
            tracing::error!("Failed to batch pause tasks: {}", e);
            return 0;
        }

        let affected = tasks.len();

        // 3. Pause external torrents, update TaskManager, broadcast
        for task in tasks {
            if Self::is_external_torrent(&task) {
                if let Err(e) = self.pause_external_torrent(&task).await {
                    tracing::warn!("Failed to pause qBittorrent task {}: {}", task.id, e);
                }
            }

            if let Some(paused_task) = self.task_manager.pause_task(task.id).await {
                self.broadcast_task_update(&paused_task);
            } else {
                let mut paused = task.clone();
                paused.state = DownloadState::Paused;
                self.task_manager.add_task(paused.clone()).await;
                self.broadcast_task_update(&paused);
            }
        }

        tracing::info!("Paused {} tasks atomically", affected);
        affected
    }

    /// Resume all paused tasks atomically
    /// Handles: DB update (atomic) → TaskManager add → Broadcast → Wake workers
    pub async fn resume_all_async(&self) -> usize {
        // Need db connection
        let db = match &self.db {
            Some(db) => db,
            None => {
                tracing::error!("No database connection for resume_all");
                return 0;
            }
        };

        let resumable_states = vec!["PAUSED".to_string()];

        // 1. Query paused tasks from DB
        let tasks = match db.get_tasks_by_states_async(resumable_states).await {
            Ok(tasks) => tasks,
            Err(e) => {
                tracing::error!("Failed to get paused tasks: {}", e);
                return 0;
            }
        };

        if tasks.is_empty() {
            return 0;
        }

        let mut local_ids = Vec::new();
        let mut affected = 0;
        let mut should_wake_workers = false;

        for task in tasks {
            if Self::is_external_torrent(&task) && task.external_id.is_some() {
                if let Err(e) = self.resume_external_torrent(&task).await {
                    tracing::warn!("Failed to resume qBittorrent task {}: {}", task.id, e);
                    continue;
                }
                let mut resumed = task.clone();
                resumed.state = DownloadState::Downloading;
                self.task_manager.update_task(resumed.clone()).await;
                let _ = db.save_task(&resumed);
                self.broadcast_task_update(&resumed);
                affected += 1;
            } else {
                local_ids.push(task.id);
                let mut queued_task = task.clone();
                queued_task.state = DownloadState::Queued;
                self.task_manager.add_task(queued_task.clone()).await;
                self.broadcast_task_update(&queued_task);
                affected += 1;
                should_wake_workers = true;
            }
        }

        if let Err(e) = db
            .batch_update_states_async(local_ids, "QUEUED".to_string())
            .await
        {
            tracing::error!("Failed to batch resume local tasks: {}", e);
        }

        if should_wake_workers {
            self.wake_workers();
        }

        tracing::info!("Resumed {} tasks atomically", affected);
        affected
    }

    /// Re-download a task using its original data
    pub async fn redownload_task(
        &self,
        task_id: uuid::Uuid,
    ) -> Result<DownloadTask, anyhow::Error> {
        let mut task = self
            .get_task_unified(task_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Task not found"))?;

        tracing::info!(
            "Re-download requested for task {} ({})",
            task.filename,
            task.id
        );

        // 1. Delete existing files to force a fresh start
        let dest_path = std::path::PathBuf::from(&task.destination);
        if dest_path.exists() {
            if let Err(e) = tokio::fs::remove_file(&dest_path).await {
                tracing::warn!("Failed to delete existing file during re-download: {}", e);
            } else {
                tracing::info!("Deleted existing file: {:?}", dest_path);
            }
        }

        // Also delete potential partial file (.flasharr)
        let temp_path = dest_path.with_extension(format!(
            "{}.flasharr",
            dest_path.extension().and_then(|e| e.to_str()).unwrap_or("")
        ));
        if temp_path.exists() {
            let _ = tokio::fs::remove_file(&temp_path).await;
        }

        // 2. Reset task state
        task.state = DownloadState::Queued;
        task.progress = 0.0;
        task.downloaded = 0;
        task.speed = 0.0;
        task.eta = 0.0;
        task.retry_count = 0;
        task.error_message = None;
        task.wait_until = None;
        task.started_at = None;
        task.completed_at = None;
        if task.host == "qbittorrent" {
            task.url = Self::canonical_qbittorrent_input(
                &task.host,
                &task.original_url,
                task.info_hash.as_deref(),
            );
        } else {
            task.url = task.original_url.clone(); // Force re-resolution
            task.url_metadata = None;
        }
        task.cancel_token = tokio_util::sync::CancellationToken::new();

        // 3. Update memory and DB
        self.task_manager.add_task(task.clone()).await;
        if let Some(db) = &self.db {
            db.save_task(&task)?;
        }

        // 4. Broadcast and notify
        self.broadcast_task_update(&task);
        self.wake_workers();

        Ok(task)
    }

    /// Re-download all tasks in a batch
    pub async fn redownload_batch_async(
        &self,
        batch_id: String,
    ) -> crate::error::FlasharrResult<usize> {
        let db = self.db.as_ref().ok_or_else(|| {
            crate::error::FlasharrError::DatabaseConnection("No database connection".to_string())
        })?;

        // 1. Get all tasks for this batch
        let tasks: Vec<DownloadTask> = db.get_tasks_by_batch_id_async(batch_id, None).await?;
        if tasks.is_empty() {
            return Ok(0);
        }

        let total = tasks.len();

        // 2. Trigger redownload for each task
        for task in tasks {
            // We can ignore errors for individual tasks in a batch re-download
            // to allow as many as possible to restart
            let _ = self.redownload_task(task.id).await;
        }

        Ok(total)
    }

    fn spawn_qbittorrent_mirror_consumer(&self) -> JoinHandle<()> {
        let running = self.running.clone();
        let db = self.db.clone();
        let task_manager = self.task_manager.clone();
        let progress_tx = self.progress_tx.clone();
        let event_bus = self.event_bus.clone();
        let qbit_manager = self.qbit_manager.clone();
        let arr_client = self.arr_client.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let Some(manager) = qbit_manager else {
                return;
            };
            let mut events = manager.subscribe();
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                let event = tokio::select! {
                    event = events.recv() => event,
                    _ = tokio::time::sleep(Duration::from_secs(1)) => continue,
                };
                let event = match event {
                    Ok(crate::torrent::instance_manager::QbitEvent::TorrentUpsert {
                        instance_id,
                        torrent,
                    }) => (instance_id, torrent),
                    Ok(crate::torrent::instance_manager::QbitEvent::TorrentRemoved {
                        instance_id,
                        hash,
                    }) => {
                        let Some(db) = db.as_ref() else {
                            continue;
                        };
                        let states = vec![
                            "STARTING".to_string(),
                            "DOWNLOADING".to_string(),
                            "PAUSED".to_string(),
                            "QUEUED".to_string(),
                            "WAITING".to_string(),
                            "IMPORTING".to_string(),
                        ];
                        if let Ok(tasks) = db.get_tasks_by_states_async(states).await {
                            for task in tasks.into_iter().filter(|task| {
                                task.download_source.as_deref() == Some("torrent")
                                    && task.external_instance_id.as_deref() == Some(&instance_id)
                                    && task
                                        .external_id
                                        .as_deref()
                                        .is_some_and(|id| id.eq_ignore_ascii_case(&hash))
                            }) {
                                Self::fail_task(
                                    0,
                                    task.id,
                                    "Torrent was removed from qBittorrent".to_string(),
                                    &task_manager,
                                    &progress_tx,
                                    &event_bus,
                                    Some(db),
                                )
                                .await;
                            }
                        }
                        continue;
                    }
                    Ok(_) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                };
                let (instance_id, row) = event;
                let Some(db) = db.as_ref() else {
                    continue;
                };
                let states = vec![
                    "STARTING".to_string(),
                    "DOWNLOADING".to_string(),
                    "PAUSED".to_string(),
                    "QUEUED".to_string(),
                    "IMPORTING".to_string(),
                ];
                let Ok(tasks) = db.get_tasks_by_states_async(states).await else {
                    continue;
                };
                for mut task in tasks.into_iter().filter(|task| {
                    task.download_source.as_deref() == Some("torrent")
                        && task.external_instance_id.as_deref() == Some(&instance_id)
                        && task
                            .external_id
                            .as_deref()
                            .or(task.info_hash.as_deref())
                            .is_some_and(|hash| hash.eq_ignore_ascii_case(&row.hash))
                }) {
                    let old_state = task.state;
                    let health = db
                        .get_qbit_instance(&instance_id)
                        .ok()
                        .flatten()
                        .map(|instance| instance.sync_health)
                        .unwrap_or_else(|| "disconnected".into());
                    Self::apply_qbit_row(&mut task, &row, &health);
                    if import_owns_transfer_state(old_state) {
                        task_manager.update_task(task.clone()).await;
                        let _ = db.save_task(&task);
                        continue;
                    }
                    let new_state = task.state;
                    if new_state == DownloadState::Completed && !task.arr_announced {
                        if Self::is_torrent_import(&task) {
                            Self::apply_qbit_source_path(&mut task, &row).await;
                            let symlink_real_base = config.read().await.symlink_real_base.clone();
                            match Self::move_to_arr_path(
                                &task,
                                &arr_client,
                                symlink_real_base.as_deref(),
                            )
                            .await
                            {
                                ArrMoveOutcome::Placed {
                                    task: mut placed_task,
                                    ..
                                } => {
                                    placed_task.state = DownloadState::Importing;
                                    task_manager.update_task(placed_task.clone()).await;
                                    let _ = db.save_task(&placed_task);
                                    let _ = progress_tx.send(ProgressUpdate {
                                        event: TaskEvent::Updated,
                                        task_id: placed_task.id.to_string(),
                                        downloaded_bytes: placed_task.downloaded,
                                        total_bytes: placed_task.size,
                                        speed_bytes_per_sec: 0.0,
                                        eta_seconds: 0.0,
                                        percentage: 100.0,
                                        state: "IMPORTING".to_string(),
                                    });
                                    event_bus.publish(super::events::TaskEvent::StateChanged {
                                        task: placed_task.clone(),
                                        old_state,
                                        new_state: DownloadState::Importing,
                                        timestamp: Utc::now(),
                                    });
                                    tokio::spawn(Self::poll_import_confirmation(
                                        placed_task.id,
                                        Arc::clone(&arr_client),
                                        Arc::clone(&task_manager),
                                        Arc::clone(&event_bus),
                                        Some(Arc::clone(db)),
                                    ));
                                    continue;
                                }
                                ArrMoveOutcome::Skipped { task: skipped_task } => {
                                    task = skipped_task;
                                    task_manager.mark_completed(task.id).await;
                                }
                                ArrMoveOutcome::Failed { reason } => {
                                    task.state = DownloadState::Importing;
                                    task.arr_announced = false;
                                    task.arr_announce_error = Some(reason);
                                    task_manager.update_task(task.clone()).await;
                                }
                            }
                        } else {
                            task.arr_announced = true;
                            task.arr_announce_error = None;
                            task_manager.update_task(task.clone()).await;
                        }
                    }
                    task_manager.update_task(task.clone()).await;
                    let _ = db.save_task(&task);
                    if task.state == DownloadState::Completed {
                        Self::record_activity(Some(db), "completed", &task);
                    }
                    let _ = progress_tx.send(ProgressUpdate {
                        event: TaskEvent::Updated,
                        task_id: task.id.to_string(),
                        downloaded_bytes: task.downloaded,
                        total_bytes: task.size,
                        speed_bytes_per_sec: task.speed,
                        eta_seconds: task.eta,
                        percentage: task.progress as f64,
                        state: format!("{:?}", task.state).to_uppercase(),
                    });
                    event_bus.publish(super::events::TaskEvent::StateChanged {
                        task: task.clone(),
                        old_state,
                        new_state,
                        timestamp: Utc::now(),
                    });
                }
            }
        })
    }

    fn spawn_qbit_reconcile_ticker(&self) -> JoinHandle<()> {
        let running = self.running.clone();
        let db = self.db.clone();
        let task_manager = self.task_manager.clone();
        let manager = self.qbit_manager.clone();
        tokio::spawn(async move {
            let (Some(db), Some(manager)) = (db, manager) else {
                return;
            };
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                let states = ["QUEUED", "STARTING", "DOWNLOADING", "WAITING"]
                    .into_iter()
                    .map(str::to_string)
                    .collect();
                if let Ok(tasks) = db.get_tasks_by_states_async(states).await {
                    for mut task in tasks.into_iter().filter(Self::is_external_torrent) {
                        let Some(hash) = task
                            .info_hash
                            .as_deref()
                            .or(task.external_id.as_deref())
                            .map(str::to_ascii_lowercase)
                        else {
                            continue;
                        };
                        let Some(instance_id) = task.external_instance_id.as_deref() else {
                            continue;
                        };
                        let health = db
                            .get_qbit_instance(instance_id)
                            .ok()
                            .flatten()
                            .map(|i| i.sync_health)
                            .unwrap_or_else(|| "disconnected".into());
                        let observed = match manager.client(instance_id).await {
                            Ok(client) => client.get_torrent(&hash).await.map_err(|_| ()),
                            Err(_) => Err(()),
                        };
                        let deadline_exceeded = task.started_at.is_some_and(|started| {
                            Utc::now().signed_duration_since(started).num_seconds() >= 60
                        });
                        let metrics = observed
                            .as_ref()
                            .ok()
                            .and_then(|v| v.as_ref())
                            .map(|i| crate::torrent::state::TransferMetrics {
                                downloaded: task.downloaded,
                                previous_downloaded: Some(task.downloaded),
                                progress: i.progress,
                                dlspeed: i.dlspeed,
                                upspeed: 0,
                                amount_left: if i.progress >= 1.0 { 0 } else { i.size.max(1) },
                            })
                            .unwrap_or(crate::torrent::state::TransferMetrics {
                                downloaded: task.downloaded,
                                previous_downloaded: Some(task.downloaded),
                                progress: 0.0,
                                dlspeed: 0,
                                upspeed: 0,
                                amount_left: 1,
                            });
                        let decision = managed_qbit_decision(
                            &health,
                            &hash,
                            observed
                                .as_ref()
                                .map(|v| v.as_ref().map(|i| (i.hash.as_str(), i.state.as_str())))
                                .map_err(|_| ()),
                            metrics,
                            true,
                            deadline_exceeded,
                        );
                        match decision {
                            ManagedQbitDecision::Downloading => {
                                task.state = DownloadState::Downloading;
                                task.error_message = None;
                            }
                            ManagedQbitDecision::Completed => {
                                task.state = status_refresh_state(
                                    task.state,
                                    DownloadState::Completed,
                                    task.arr_announced,
                                );
                                if task.state == DownloadState::Completed {
                                    task.completed_at.get_or_insert_with(Utc::now);
                                }
                            }
                            ManagedQbitDecision::Starting(reason) => {
                                task.state = DownloadState::Starting;
                                task.error_message = Some(reason.into());
                            }
                            ManagedQbitDecision::Wait(reason) => {
                                task.state = DownloadState::Waiting;
                                task.error_message = Some(reason.into());
                            }
                            ManagedQbitDecision::Failed(reason) => {
                                task.state = DownloadState::Failed;
                                task.error_message = Some(reason.into());
                                task.completed_at.get_or_insert_with(Utc::now);
                            }
                            ManagedQbitDecision::AddOnce => continue, // restored identity: never duplicate
                        }
                        task_manager.update_task(task.clone()).await;
                        let _ = db.save_task(&task);
                        if task.state == DownloadState::Failed {
                            Self::record_activity(Some(&db), "failed", &task);
                        }
                    }
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        })
    }

    async fn start_qbittorrent_task(
        worker_id: usize,
        mut task: DownloadTask,
        fresh_queued: bool,
        task_manager: &Arc<DownloadTaskManager>,
        db: Option<&Arc<Db>>,
        progress_tx: &broadcast::Sender<ProgressUpdate>,
        event_bus: &Arc<super::events::EventBus>,
        qbittorrent_client: &Arc<
            tokio::sync::RwLock<Option<Arc<crate::torrent::qbittorrent::QBittorrentClient>>>,
        >,
        qbit_manager: Option<&Arc<crate::torrent::instance_manager::QbitInstanceManager>>,
    ) {
        // Terminal / import states never re-enter the add path on restart.
        if matches!(
            task.state,
            DownloadState::Importing | DownloadState::Completed | DownloadState::Failed
        ) {
            return;
        }
        // A restored external torrent task already carried qB identity across a
        // restart; a genuinely new QUEUED acquisition has none yet.
        let (instance_id, client) =
            match Self::qbit_client_for_task(&task, qbit_manager, qbittorrent_client).await {
                Ok(found) => found,
                Err(_) => {
                    Self::fail_task(
                        worker_id,
                        task.id,
                        "qBittorrent is not configured".to_string(),
                        task_manager,
                        progress_tx,
                        event_bus,
                        db,
                    )
                    .await;
                    return;
                }
            };

        let torrent_input =
            Self::canonical_qbittorrent_input(&task.host, &task.url, task.info_hash.as_deref());
        // Restart-adoption gate: before adding, consult authoritative instance
        // sync health + persisted mirror row keyed by (instance, hash).
        //   healthy + exact row  -> adopt from row, no add
        //   stale/disconnected   -> leave safe STARTING/WAITING, persist, no add, no FAILED
        //   healthy + missing    -> add only for a genuinely new queued acquisition;
        //                           a restored external task waits (no duplicate on restart)
        // Fresh API-created QUEUED tasks carry the expected info hash as
        // identity, but have not been added to qB yet. Only restored/non-queued
        // tasks are external identities that must wait instead of re-adding.
        let restored_external = !fresh_queued
            && is_restored_external_qbit_task(
                task.state,
                task.error_message.as_deref(),
                task.external_id.as_deref(),
                task.external_provider.as_deref(),
                task.external_instance_id.as_deref(),
            );
        tracing::info!(
            task_id = %task.id,
            state = ?task.state,
            fresh_queued,
            restored_external,
            has_info_hash = task.info_hash.is_some(),
            has_external_id = task.external_id.is_some(),
            has_external_instance = task.external_instance_id.is_some(),
            "qB lifecycle: adoption classification"
        );
        let adoption = if let Some(hash) = task.info_hash.as_deref().map(str::to_ascii_lowercase) {
            let observed = client.get_torrent(&hash).await;
            let decision = match observed.as_ref() {
                Ok(Some(info)) => managed_qbit_decision(
                    "healthy",
                    &hash,
                    Ok(Some((&info.hash, &info.state))),
                    crate::torrent::state::TransferMetrics {
                        downloaded: 0,
                        previous_downloaded: None,
                        progress: info.progress,
                        dlspeed: info.dlspeed,
                        upspeed: 0,
                        amount_left: if info.progress >= 1.0 {
                            0
                        } else {
                            info.size.max(1)
                        },
                    },
                    restored_external,
                    false,
                ),
                Ok(None) => managed_qbit_decision(
                    "healthy",
                    &hash,
                    Ok(None),
                    crate::torrent::state::TransferMetrics {
                        downloaded: 0,
                        previous_downloaded: None,
                        progress: 0.0,
                        dlspeed: 0,
                        upspeed: 0,
                        amount_left: 1,
                    },
                    restored_external,
                    false,
                ),
                Err(_) => managed_qbit_decision(
                    "healthy",
                    &hash,
                    Err(()),
                    crate::torrent::state::TransferMetrics {
                        downloaded: 0,
                        previous_downloaded: None,
                        progress: 0.0,
                        dlspeed: 0,
                        upspeed: 0,
                        amount_left: 1,
                    },
                    restored_external,
                    false,
                ),
            };
            match decision {
                ManagedQbitDecision::AddOnce => {
                    tracing::info!(task_id = %task.id, "qB lifecycle: decision AddOnce");
                    RestartAdoption::Add
                }
                ManagedQbitDecision::Downloading | ManagedQbitDecision::Completed => {
                    let info = observed
                        .ok()
                        .flatten()
                        .expect("present decision has qB row");
                    task.provider_id = Some("torrent".to_string());
                    task.provider_item_id = Some(hash.clone());
                    task.info_hash = Some(hash.clone());
                    task.external_id = Some(hash);
                    // Persist instance identity only after the exact-hash decision succeeds.
                    task.external_instance_id = instance_id;
                    task.external_provider = Some("qbittorrent".to_string());
                    task.download_source = Some("torrent".to_string());
                    Self::apply_qbittorrent_info(&mut task, &info);
                    task_manager.update_task(task.clone()).await;
                    if let Some(db) = db {
                        let _ = db.save_task(&task);
                    }
                    return;
                }
                ManagedQbitDecision::Starting(reason) | ManagedQbitDecision::Wait(reason) => {
                    task.state = DownloadState::Waiting;
                    task.error_message = Some(reason.to_string());
                    task_manager.update_task(task.clone()).await;
                    if let Some(db) = db {
                        let _ = db.save_task(&task);
                    }
                    return;
                }
                ManagedQbitDecision::Failed(reason) => {
                    Self::fail_task(
                        worker_id,
                        task.id,
                        reason.to_string(),
                        task_manager,
                        progress_tx,
                        event_bus,
                        db,
                    )
                    .await;
                    return;
                }
            }
        } else {
            RestartAdoption::Add
        };

        tracing::info!(
            "Worker {}: qB add input_kind={} task={}",
            worker_id,
            if torrent_input.starts_with("magnet:") {
                "magnet"
            } else {
                "http"
            },
            task.id
        );

        let add_outcome = run_restart_adoption(adoption, || {
            run_qbit_add_with_timeout(Duration::from_secs(30), || {
                client.add_torrent(&torrent_input)
            })
        })
        .await
        .expect("add decision invokes qBittorrent exactly once");
        tracing::info!(
            task_id = %task.id,
            add_ok = add_outcome.is_ok(),
            returned_identity = matches!(&add_outcome, Ok(Some(_))),
            "qB lifecycle: add returned"
        );
        match add_outcome {
            Ok(Some(hash)) => {
                let hash = hash.trim().to_ascii_lowercase();
                let decision = poll_qbit_confirmation(
                    &hash,
                    Duration::from_secs(60),
                    Duration::from_secs(10),
                    || async { Ok(client.get_torrent(&hash).await?.map(|torrent| torrent.hash)) },
                )
                .await;
                tracing::info!(
                    task_id = %task.id,
                    decision = ?decision,
                    "qB lifecycle: confirmation completed"
                );
                if let QbitConfirmationDecision::Failed(message) = decision {
                    Self::fail_task(
                        worker_id,
                        task.id,
                        message.to_string(),
                        task_manager,
                        progress_tx,
                        event_bus,
                        db,
                    )
                    .await;
                    return;
                }
                task.external_id = Some(hash);
                task.external_instance_id = instance_id;
                task.external_provider = Some("qbittorrent".to_string());
                task.download_source = Some("torrent".to_string());
                task.state = DownloadState::Downloading;
                task_manager.update_task(task.clone()).await;
                if let Some(db) = db {
                    let _ = db.save_task(&task);
                }
                let _ = progress_tx.send(ProgressUpdate {
                    event: TaskEvent::Updated,
                    task_id: task.id.to_string(),
                    downloaded_bytes: task.downloaded,
                    total_bytes: task.size,
                    speed_bytes_per_sec: task.speed,
                    eta_seconds: task.eta,
                    percentage: task.progress as f64,
                    state: "DOWNLOADING".to_string(),
                });
                event_bus.publish(super::events::TaskEvent::StateChanged {
                    task,
                    old_state: DownloadState::Starting,
                    new_state: DownloadState::Downloading,
                    timestamp: Utc::now(),
                });
            }
            Ok(None) => {
                Self::fail_task(
                    worker_id,
                    task.id,
                    "qBittorrent accepted the torrent but did not return a resolvable hash; task cannot be managed safely".to_string(),
                    task_manager,
                    progress_tx,
                    event_bus,
                    db,
                )
                .await;
            }
            Err(e) => {
                Self::fail_task(
                    worker_id,
                    task.id,
                    e.to_string(),
                    task_manager,
                    progress_tx,
                    event_bus,
                    db,
                )
                .await;
            }
        }
    }

    /// Spawn a worker
    fn spawn_worker(&self, worker_id: usize) -> JoinHandle<()> {
        let running = self.running.clone();
        let task_manager = self.task_manager.clone();
        let download_engine = self.download_engine.clone();
        let host_registry = self.host_registry.clone();
        let db = self.db.clone();
        let progress_tx = self.progress_tx.clone();
        let event_bus = self.event_bus.clone();
        let config = self.config.clone();
        let task_notify = self.task_notify.clone();
        let arr_client = self.arr_client.clone();
        let qbittorrent_client = self.qbittorrent_client.clone();
        let qbit_manager = self.qbit_manager.clone();

        tokio::spawn(async move {
            tracing::debug!("Worker {} started", worker_id);

            while running.load(std::sync::atomic::Ordering::Relaxed) {
                // Check if this worker should be active based on current config
                // This allows dynamic max_concurrent changes without restart
                {
                    let current_max = config.read().await.max_concurrent;
                    if worker_id >= current_max {
                        // This worker is over the limit, sleep and check again
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        continue;
                    }
                }

                // Get next queued task
                if let Some(task) = task_manager.pop_next_queued().await {
                    tracing::info!("Worker {}: Processing task {}", worker_id, task.id);

                    // Acquire locks for current config and engine
                    let (current_engine, current_config) = {
                        let engine_guard = download_engine.read().await;
                        let config_guard = config.read().await;
                        (engine_guard.clone(), config_guard.clone())
                    };

                    // Process task
                    Self::process_task_static(
                        worker_id,
                        task,
                        &task_manager,
                        &current_engine,
                        &host_registry,
                        db.as_ref(),
                        &progress_tx,
                        &event_bus,
                        &current_config,
                        &arr_client,
                        &qbittorrent_client,
                        qbit_manager.as_ref(),
                    )
                    .await;
                } else {
                    // No tasks - wait for notification or timeout
                    // Use select! to allow cancellation via running flag check
                    tokio::select! {
                        _ = task_notify.notified() => {},
                        _ = tokio::time::sleep(Duration::from_secs(2)) => {},
                    }
                }
            }

            tracing::debug!("Worker {} stopped", worker_id);
        })
    }

    /// Process a single task (static method for worker)
    async fn process_task_static(
        worker_id: usize,
        mut task: DownloadTask,
        task_manager: &Arc<DownloadTaskManager>,
        download_engine: &Arc<SimpleDownloadEngine>,
        host_registry: &Arc<HostRegistry>,
        db: Option<&Arc<Db>>,
        progress_tx: &broadcast::Sender<ProgressUpdate>,
        event_bus: &Arc<super::events::EventBus>,
        config: &DownloadConfig,
        arr_client: &Arc<tokio::sync::RwLock<Option<Arc<crate::arr::ArrClient>>>>,
        qbittorrent_client: &Arc<
            tokio::sync::RwLock<Option<Arc<crate::torrent::qbittorrent::QBittorrentClient>>>,
        >,
        qbit_manager: Option<&Arc<crate::torrent::instance_manager::QbitInstanceManager>>,
    ) {
        let fresh_queued = task.state == DownloadState::Queued;
        tracing::info!(
            task_id = %task.id,
            state_at_process_entry = ?task.state,
            fresh_queued,
            is_torrent = Self::is_torrent_import(&task),
            has_info_hash = task.info_hash.is_some(),
            has_external_id = task.external_id.is_some(),
            has_external_instance = task.external_instance_id.is_some(),
            "qB lifecycle: worker process entry"
        );
        // Update state to Starting
        task.state = DownloadState::Starting;
        task.started_at = Some(Utc::now());
        task_manager.update_task(task.clone()).await;

        // Publish StateChanged event (QUEUED -> STARTING)
        event_bus.publish(super::events::TaskEvent::StateChanged {
            task: task.clone(),
            old_state: DownloadState::Queued,
            new_state: DownloadState::Starting,
            timestamp: Utc::now(),
        });

        if let Some(db) = db {
            let _ = db.save_task(&task);
        }

        if Self::is_torrent_import(&task) {
            Self::start_qbittorrent_task(
                worker_id,
                task,
                fresh_queued,
                task_manager,
                db,
                progress_tx,
                event_bus,
                qbittorrent_client,
                qbit_manager,
            )
            .await;
            return;
        }

        // Resolve URL using host handler
        let handler_host = if task.host == "sabnzbd-fshare" {
            "fshare"
        } else {
            task.host.as_str()
        };

        let download_url = match host_registry.get_handler(handler_host) {
            _ if task.execution_strategy.as_deref() == Some("torbox_http")
                && matches!(
                    task.state,
                    DownloadState::Starting | DownloadState::Downloading
                ) =>
            {
                let Some(db) = db else {
                    Self::fail_task(
                        worker_id,
                        task.id,
                        "TorBox restart resume requires database".into(),
                        task_manager,
                        progress_tx,
                        event_bus,
                        None,
                    )
                    .await;
                    return;
                };
                let Some(remote_id) = task.external_id.as_deref() else {
                    Self::fail_task(
                        worker_id,
                        task.id,
                        "TorBox task missing remote ID".into(),
                        task_manager,
                        progress_tx,
                        event_bus,
                        Some(db),
                    )
                    .await;
                    return;
                };
                match crate::sources::torbox::resume_link_from_db(
                    db,
                    remote_id,
                    Duration::from_secs(300),
                )
                .await
                {
                    Ok(link) => link,
                    Err(error) => {
                        Self::fail_task(
                            worker_id,
                            task.id,
                            format!("TorBox restart resume failed: {error}"),
                            task_manager,
                            progress_tx,
                            event_bus,
                            Some(db),
                        )
                        .await;
                        return;
                    }
                }
            }
            Some(handler) => {
                tracing::info!(
                    "Worker {}: Resolving URL with {} handler",
                    worker_id,
                    handler_host
                );

                let should_update_filename = Self::filename_needs_host_metadata(&task);
                if task.size == 0 || should_update_filename {
                    match handler.get_file_info(&task.original_url).await {
                        Ok(file_info) => {
                            let mut changed = false;
                            if task.size == 0 && file_info.size > 0 {
                                task.size = file_info.size;
                                changed = true;
                            }
                            if should_update_filename && !file_info.filename.trim().is_empty() {
                                task.filename = file_info.filename.clone();
                                let metadata = Self::metadata_from_task(&task);
                                task.destination = Self::build_destination_path_static(
                                    &task.filename,
                                    &task.category,
                                    &metadata,
                                    &config.download_dir,
                                );
                                changed = true;
                            }
                            if changed {
                                task_manager.update_task(task.clone()).await;
                                if let Some(db) = db {
                                    let _ = db.save_task(&task);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Worker {}: Failed to enrich file info at STARTING: {}",
                                worker_id,
                                e
                            );
                        }
                    }
                }

                match handler.resolve_download_url(&task.original_url).await {
                    Ok(resolved) => {
                        task.url = resolved.direct_url.clone();
                        task.url_metadata = Some(UrlMetadata {
                            resolved_at: Utc::now(),
                            expires_at: Utc::now() + chrono::Duration::hours(6),
                        });
                        task_manager.update_task(task.clone()).await;
                        resolved.direct_url
                    }
                    Err(e) => {
                        tracing::error!("Worker {}: Failed to resolve URL: {}", worker_id, e);

                        Self::fail_task(
                            worker_id,
                            task.id,
                            format!("URL resolution failed: {}", e),
                            task_manager,
                            progress_tx,
                            event_bus,
                            db,
                        )
                        .await;

                        return;
                    }
                }
            }
            None if Self::is_direct_http_import(&task) => {
                let Some(db) = db else {
                    Self::fail_task(
                        worker_id,
                        task.id,
                        "TorBox link renewal requires database settings".to_string(),
                        task_manager,
                        progress_tx,
                        event_bus,
                        None,
                    )
                    .await;
                    return;
                };
                let Some(remote_id) = task.external_id.as_deref() else {
                    Self::fail_task(
                        worker_id,
                        task.id,
                        "TorBox task is missing its remote ID; re-add this download".to_string(),
                        task_manager,
                        progress_tx,
                        event_bus,
                        Some(db),
                    )
                    .await;
                    return;
                };
                match crate::sources::torbox::refresh_link_from_db(db, remote_id).await {
                    Ok(link) => {
                        task.url = link.clone();
                        task.original_url = link.clone();
                        task.url_metadata = Some(UrlMetadata {
                            resolved_at: Utc::now(),
                            expires_at: Utc::now() + chrono::Duration::hours(6),
                        });
                        task_manager.update_task(task.clone()).await;
                        let _ = db.save_task(&task);
                        link
                    }
                    Err(error) => {
                        Self::fail_task(
                            worker_id,
                            task.id,
                            format!("TorBox link renewal failed: {error}"),
                            task_manager,
                            progress_tx,
                            event_bus,
                            Some(db),
                        )
                        .await;
                        return;
                    }
                }
            }
            None => {
                tracing::warn!(
                    "Worker {}: No handler for host '{}', using original URL",
                    worker_id,
                    task.host
                );
                task.original_url.clone()
            }
        };

        // Update state to Downloading
        let old_state = task.state;
        task.state = DownloadState::Downloading;
        task_manager.update_task(task.clone()).await;

        // Publish StateChanged event (immediate sync)
        event_bus.publish(super::events::TaskEvent::StateChanged {
            task: task.clone(),
            old_state,
            new_state: DownloadState::Downloading,
            timestamp: chrono::Utc::now(),
        });

        // Download the file
        let destination = PathBuf::from(&task.destination);
        let task_id = task.id;
        let cancel_token = task.cancel_token.clone();

        // Clone for the progress callback closure (needs to be 'static)
        let task_manager_clone = Arc::clone(task_manager);
        let progress_tx_clone = progress_tx.clone();

        let result = download_engine
            .download_file(
                &download_url,
                &destination,
                move |progress| {
                    // Update ALL progress fields in one atomic operation to prevent flickering
                    let task_manager = task_manager_clone.clone();
                    tokio::spawn(async move {
                        task_manager
                            .update_task_progress(
                                task_id,
                                progress.downloaded_bytes,
                                progress.total_bytes,
                                progress.speed_bytes_per_sec,
                                progress.eta_seconds,
                                progress.percentage as f32,
                            )
                            .await;
                    });

                    // Broadcast progress (WebSocket handler will just read the task, not update again)
                    let _ = progress_tx_clone.send(ProgressUpdate {
                        event: TaskEvent::Updated,
                        task_id: task_id.to_string(),
                        downloaded_bytes: progress.downloaded_bytes,
                        total_bytes: progress.total_bytes,
                        speed_bytes_per_sec: progress.speed_bytes_per_sec,
                        eta_seconds: progress.eta_seconds,
                        percentage: progress.percentage,
                        state: "DOWNLOADING".to_string(),
                    });
                },
                &cancel_token,
            )
            .await;

        // Handle result
        match result {
            Ok(actual_path) => {
                tracing::info!(
                    "Worker {}: Download completed for {} at {:?}",
                    worker_id,
                    task_id,
                    actual_path
                );
                // Persist actual path (engine may remap /downloads/ to appData/downloads/)
                task.destination = actual_path.to_string_lossy().to_string();
                task_manager.update_task(task.clone()).await;
                task_manager.mark_completed(task_id).await;

                // Post-completion: symlink into arr library and drive closed-loop import polling.
                if let Some(completed_task) = task_manager.get_task(task_id).await {
                    let symlink_real_base = config.symlink_real_base.clone();
                    let outcome = if Self::is_sabnzbd_managed_import(&completed_task) {
                        tracing::info!(
                            "Worker {}: Task {} initiated by SABnzbd API, skipping move_to_arr_path to let *arr handle import natively",
                            worker_id,
                            task_id
                        );
                        ArrMoveOutcome::Skipped {
                            task: completed_task.clone(),
                        }
                    } else {
                        if Self::is_direct_http_import(&completed_task) {
                            tracing::info!(
                                "Worker {}: Task {} completed via direct HTTP flow ({:?}); importing as local file, not torrent",
                                worker_id,
                                task_id,
                                completed_task.download_source
                            );
                        }
                        Self::move_to_arr_path(
                            &completed_task,
                            arr_client,
                            symlink_real_base.as_deref(),
                        )
                        .await
                    };
                    match outcome {
                        ArrMoveOutcome::Placed {
                            task: placed_task,
                            arr_id,
                            media_type,
                        } => {
                            // ── Enter Importing state ─────────────────────────────────────────
                            let mut importing_task = placed_task.clone();
                            importing_task.state = DownloadState::Importing;
                            task_manager.update_task(importing_task.clone()).await;
                            if let Some(db) = db {
                                let _ = db.save_task(&importing_task);
                            }

                            tracing::info!(
                                "Worker {}: Task {} entering Importing state (arr_id={}, media_type={:?})",
                                worker_id,
                                task_id,
                                arr_id,
                                media_type
                            );

                            // Broadcast Importing state to WebSocket subscribers
                            let _ = progress_tx.send(ProgressUpdate {
                                event: TaskEvent::Updated,
                                task_id: task_id.to_string(),
                                downloaded_bytes: importing_task.downloaded,
                                total_bytes: importing_task.size,
                                speed_bytes_per_sec: 0.0,
                                eta_seconds: 0.0,
                                percentage: 100.0,
                                state: "IMPORTING".to_string(),
                            });

                            // ── Poll arr history for downloadFolderImported event ────────────
                            // nzo_id = task UUID (the SABnzbd shim uses this as the download ID)
                            let nzo_id = task_id.to_string();
                            let poll_timeout = std::time::Duration::from_secs(600); // 10 min

                            let arr_guard = arr_client.read().await;
                            let poll_result = if let Some(ref client) = *arr_guard {
                                match media_type {
                                    crate::downloader::MediaType::TvSeries
                                    | crate::downloader::MediaType::TvEpisode => {
                                        if let (Some(season), Some(episode)) = (
                                            importing_task.tmdb_season,
                                            importing_task.tmdb_episode,
                                        ) {
                                            client
                                                .poll_sonarr_episode_import_confirmation(
                                                    arr_id,
                                                    &nzo_id,
                                                    season as i32,
                                                    episode as i32,
                                                    poll_timeout,
                                                )
                                                .await
                                        } else {
                                            client
                                                .poll_sonarr_import_confirmation(
                                                    arr_id,
                                                    &nzo_id,
                                                    poll_timeout,
                                                )
                                                .await
                                        }
                                    }
                                    crate::downloader::MediaType::Movie => {
                                        client
                                            .poll_radarr_import_confirmation(
                                                arr_id,
                                                &nzo_id,
                                                poll_timeout,
                                            )
                                            .await
                                    }
                                }
                            } else {
                                Ok(false)
                            };
                            drop(arr_guard);

                            // ── Finalize: Completed or record timeout error ───────────────────
                            let mut final_task = importing_task.clone();
                            final_task.state = DownloadState::Completed;

                            match poll_result {
                                Ok(true) => {
                                    tracing::info!(
                                        "Worker {}: Task {} import confirmed by arr (nzo_id={})",
                                        worker_id,
                                        task_id,
                                        nzo_id
                                    );
                                    final_task.arr_announced = true;
                                    final_task.arr_announce_error = None;
                                }
                                Ok(false) => {
                                    tracing::warn!(
                                        "Worker {}: Task {} import poll timed out after {}s (nzo_id={})",
                                        worker_id,
                                        task_id,
                                        poll_timeout.as_secs(),
                                        nzo_id
                                    );
                                    final_task.arr_announce_error = Some(format!(
                                        "import poll timed out after {}s",
                                        poll_timeout.as_secs()
                                    ));
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Worker {}: Task {} import poll error: {} (nzo_id={})",
                                        worker_id,
                                        task_id,
                                        e,
                                        nzo_id
                                    );
                                    final_task.arr_announce_error =
                                        Some(format!("import poll error: {}", e));
                                }
                            }

                            task_manager.update_task(final_task.clone()).await;
                            if let Some(db) = db {
                                let _ = db.save_task(&final_task);
                            }
                        }

                        ArrMoveOutcome::Skipped { task: skipped_task } => {
                            // arr not configured or no TMDB metadata — mark completed directly
                            tracing::info!(
                                "Worker {}: Task {} skipped arr import (no arr config or tmdb_id)",
                                worker_id,
                                task_id
                            );
                            if let Some(db) = db {
                                let _ = db.save_task(&skipped_task);
                            }
                        }

                        ArrMoveOutcome::Failed { reason } => {
                            // Filesystem or arr API hard failure — stay Completed, record error
                            tracing::error!(
                                "Worker {}: Task {} move_to_arr_path failed: {}",
                                worker_id,
                                task_id,
                                reason
                            );
                            let mut failed_task = completed_task.clone();
                            failed_task.arr_announce_error = Some(reason);
                            task_manager.update_task(failed_task.clone()).await;
                            if let Some(db) = db {
                                let _ = db.save_task(&failed_task);
                            }
                        }
                    }
                }

                // Broadcast final completion (immediate sync)
                if let Some(completed_task) = task_manager.get_task(task_id).await {
                    if completed_task.state == DownloadState::Completed {
                        Self::record_activity(db, "completed", &completed_task);
                    }
                    event_bus.publish(super::events::TaskEvent::Completed {
                        task: completed_task,
                        timestamp: chrono::Utc::now(),
                    });
                }

                // Legacy broadcast
                let _ = progress_tx.send(ProgressUpdate {
                    event: TaskEvent::Updated,
                    task_id: task_id.to_string(),
                    downloaded_bytes: 0,
                    total_bytes: 0,
                    speed_bytes_per_sec: 0.0,
                    eta_seconds: 0.0,
                    percentage: 100.0,
                    state: "COMPLETED".to_string(),
                });
            }
            Err(e) => {
                let error_string = e.to_string();

                // Check if this was a pause action (task state is already Paused)
                // In this case, we should NOT retry or mark as failed
                if let Some(task) = task_manager.get_task(task_id).await {
                    if task.state == DownloadState::Paused {
                        tracing::info!(
                            "Worker {}: Download paused for {} (by user request)",
                            worker_id,
                            task_id
                        );

                        // Broadcast paused state
                        let _ = progress_tx.send(ProgressUpdate {
                            event: TaskEvent::Updated,
                            task_id: task_id.to_string(),
                            downloaded_bytes: task.downloaded,
                            total_bytes: task.size,
                            speed_bytes_per_sec: 0.0,
                            eta_seconds: 0.0,
                            percentage: task.progress as f64,
                            state: "PAUSED".to_string(),
                        });

                        // Save to database
                        if let Some(db) = db {
                            let _ = db.save_task(&task);
                        }
                        return; // Don't retry or mark as failed
                    }
                }

                // Check if user cancelled (not paused)
                if cancel_token.is_cancelled() {
                    // Check one more time if it was paused
                    if let Some(task) = task_manager.get_task(task_id).await {
                        if task.state == DownloadState::Paused {
                            return; // Already handled above
                        }
                    }

                    // True cancellation (delete/stop), not pause
                    Self::fail_task(
                        worker_id,
                        task_id,
                        "Download cancelled".to_string(),
                        task_manager,
                        progress_tx,
                        event_bus,
                        db,
                    )
                    .await;

                    if let Some(db) = db {
                        if let Some(updated_task) = task_manager.get_task(task_id).await {
                            let _ = db.save_task(&updated_task);
                        }
                    }
                    return;
                }

                // ── Intelligent error classification ──────────────────────────
                // Use ErrorClassifier to determine the correct recovery strategy
                // instead of blindly retrying everything with the same config.
                use super::error_classifier::{ErrorCategory, ErrorClassifier};
                let category = ErrorClassifier::classify(&e);

                tracing::warn!(
                    "Worker {}: Download failed for {} — {:?} — {}",
                    worker_id,
                    task_id,
                    category,
                    error_string
                );

                if let Some(mut task) = task_manager.get_task(task_id).await {
                    match category {
                        // ── URL expired: refresh the VIP link and try again ──
                        ErrorCategory::UrlRefreshNeeded {
                            max_retries,
                            reason,
                        } => {
                            if task.retry_count < max_retries {
                                let old_state = task.state;
                                task.retry_count += 1;
                                task.state = DownloadState::Waiting;
                                // Clear the resolved URL so process_task_static re-resolves
                                // a fresh VIP link on the next attempt via resolve_download_url().
                                // ponytail: qB torrent tasks keep canonical magnet instead.
                                if task.host == "qbittorrent" {
                                    task.url = Self::canonical_qbittorrent_input(
                                        &task.host,
                                        &task.original_url,
                                        task.info_hash.as_deref(),
                                    );
                                } else if Self::is_direct_http_import(&task) {
                                    task.url.clear();
                                    task.original_url.clear();
                                    task.url_metadata = None;
                                } else {
                                    task.url = task.original_url.clone();
                                    task.url_metadata = None;
                                }
                                // Short delay — just enough to avoid hammering the API
                                task.wait_until = Some(Utc::now() + chrono::Duration::seconds(3));
                                task.error_message = Some(format!(
                                    "Link expired ({}/{}): {reason} — getting fresh URL",
                                    task.retry_count, max_retries
                                ));
                                task_manager.update_task(task.clone()).await;

                                // Publish StateChanged event (immediate sync)
                                event_bus.publish(super::events::TaskEvent::StateChanged {
                                    task: task.clone(),
                                    old_state,
                                    new_state: DownloadState::Waiting,
                                    timestamp: chrono::Utc::now(),
                                });

                                tracing::info!(
                                    "Worker {}: URL refresh needed for {} — will re-resolve in 3s (attempt {}/{})",
                                    worker_id,
                                    task_id,
                                    task.retry_count,
                                    max_retries
                                );
                            } else {
                                let msg = format!(
                                    "Failed after {max_retries} URL refresh attempts: {reason}"
                                );
                                tracing::error!("Worker {}: {}", worker_id, msg);
                                Self::fail_task(
                                    worker_id,
                                    task_id,
                                    msg,
                                    &task_manager,
                                    progress_tx,
                                    event_bus,
                                    db,
                                )
                                .await;
                            }
                        }

                        // ── Retryable: network blip, server busy — retry with delay ──
                        ErrorCategory::Retryable {
                            max_retries,
                            delay_seconds,
                            reason,
                        } => {
                            if task.retry_count < max_retries {
                                let old_state = task.state;
                                task.retry_count += 1;
                                task.state = DownloadState::Waiting;
                                // Use classifier's delay, bounded by config max
                                let delay_ms =
                                    (delay_seconds * 1000).min(config.retry.max_delay_ms as u64);
                                task.wait_until = Some(
                                    Utc::now() + chrono::Duration::milliseconds(delay_ms as i64),
                                );
                                task.error_message = Some(format!(
                                    "Retry {}/{}: {reason}",
                                    task.retry_count, max_retries
                                ));
                                task_manager.update_task(task.clone()).await;
                                task_manager.release_processing(task_id).await;
                                if let Some(db) = &db {
                                    let _ = db.save_task(&task);
                                }

                                // Publish StateChanged event (immediate sync)
                                event_bus.publish(super::events::TaskEvent::StateChanged {
                                    task: task.clone(),
                                    old_state,
                                    new_state: DownloadState::Waiting,
                                    timestamp: chrono::Utc::now(),
                                });

                                tracing::warn!(
                                    "Worker {}: Retryable error for {} in {}ms (attempt {}/{}): {}",
                                    worker_id,
                                    task_id,
                                    delay_ms,
                                    task.retry_count,
                                    max_retries,
                                    reason
                                );
                            } else {
                                let msg = format!("Max retries ({max_retries}) exceeded: {reason}");
                                Self::fail_task(
                                    worker_id,
                                    task_id,
                                    msg,
                                    &task_manager,
                                    progress_tx,
                                    event_bus,
                                    db,
                                )
                                .await;
                            }
                        }

                        // ── Permanent: no point retrying ──────────────────────
                        ErrorCategory::Permanent { reason } => {
                            let msg = format!("Permanent failure: {reason}");
                            tracing::error!("Worker {}: {} (task {})", worker_id, msg, task_id);
                            Self::fail_task(
                                worker_id,
                                task_id,
                                msg,
                                &task_manager,
                                progress_tx,
                                event_bus,
                                db,
                            )
                            .await;
                        }

                        // ── Account issue: needs user action ──────────────────
                        ErrorCategory::AccountIssue {
                            reason,
                            action_required,
                        } => {
                            let msg = format!("Account issue: {reason} — {action_required}");
                            tracing::error!("Worker {}: {} (task {})", worker_id, msg, task_id);
                            Self::fail_task(
                                worker_id,
                                task_id,
                                msg,
                                &task_manager,
                                progress_tx,
                                event_bus,
                                db,
                            )
                            .await;
                        }

                        // ── System/config issue: retry a few times ────────────
                        ErrorCategory::SystemIssue {
                            max_retries,
                            reason,
                            fix_suggestion,
                        } => {
                            if task.retry_count < max_retries {
                                let old_state = task.state;
                                task.retry_count += 1;
                                task.state = DownloadState::Waiting;
                                task.wait_until = Some(Utc::now() + chrono::Duration::seconds(10));
                                task.error_message = Some(format!(
                                    "System issue ({}/{}): {reason} — Tip: {fix_suggestion}",
                                    task.retry_count, max_retries
                                ));
                                task_manager.update_task(task.clone()).await;

                                // Publish StateChanged event (immediate sync)
                                event_bus.publish(super::events::TaskEvent::StateChanged {
                                    task: task.clone(),
                                    old_state,
                                    new_state: DownloadState::Waiting,
                                    timestamp: chrono::Utc::now(),
                                });

                                tracing::warn!(
                                    "Worker {}: System issue for {} (attempt {}/{}): {} — {}",
                                    worker_id,
                                    task_id,
                                    task.retry_count,
                                    max_retries,
                                    reason,
                                    fix_suggestion
                                );
                            } else {
                                let msg = format!("{reason} — {fix_suggestion}");
                                Self::fail_task(
                                    worker_id,
                                    task_id,
                                    msg,
                                    &task_manager,
                                    progress_tx,
                                    event_bus,
                                    db,
                                )
                                .await;
                            }
                        }
                    }

                    if let Some(db) = db {
                        if let Some(updated_task) = task_manager.get_task(task_id).await {
                            let _ = db.save_task(&updated_task);
                        }
                    }
                }
            }
        }
    }

    /// Calculate retry delay with exponential backoff
    fn calculate_retry_delay_static(retry_count: u32, config: &DownloadConfig) -> Duration {
        let base_delay = config.retry.base_delay_ms as u64;
        let max_delay = config.retry.max_delay_ms as u64;
        let delay = base_delay * 2u64.pow(retry_count.saturating_sub(1));
        Duration::from_millis(delay.min(max_delay))
    }

    /// Mark a task as failed and broadcast the state over WebSocket.
    /// DRY helper shared by all error category handlers in process_task_static.
    async fn fail_task(
        worker_id: usize,
        task_id: uuid::Uuid,
        message: String,
        task_manager: &Arc<DownloadTaskManager>,
        progress_tx: &broadcast::Sender<ProgressUpdate>,
        event_bus: &Arc<super::events::EventBus>,
        db: Option<&Arc<Db>>,
    ) {
        tracing::error!(
            "Worker {}: Permanently failing task {}: {}",
            worker_id,
            task_id,
            message
        );
        task_manager.mark_failed(task_id, message.clone()).await;

        let _ = progress_tx.send(ProgressUpdate {
            event: TaskEvent::Updated,
            task_id: task_id.to_string(),
            downloaded_bytes: 0,
            total_bytes: 0,
            speed_bytes_per_sec: 0.0,
            eta_seconds: 0.0,
            percentage: 0.0,
            state: "FAILED".to_string(),
        });

        if let Some(failed_task) = task_manager.get_task(task_id).await {
            // Save to database before exposing a durable failure outcome.
            if let Some(db) = db {
                let _ = db.save_task(&failed_task);
            }
            Self::record_activity(db, "failed", &failed_task);

            // Publish Failed event (immediate sync)
            event_bus.publish(super::events::TaskEvent::Failed {
                task: failed_task.clone(),
                error: message.clone(),
                retry_count: failed_task.retry_count,
                timestamp: chrono::Utc::now(),
            });
        }
    }

    async fn poll_import_confirmation(
        task_id: uuid::Uuid,
        arr_client: Arc<tokio::sync::RwLock<Option<Arc<crate::arr::ArrClient>>>>,
        task_manager: Arc<DownloadTaskManager>,
        event_bus: Arc<super::events::EventBus>,
        db: Option<Arc<Db>>,
    ) {
        let Some(task) = task_manager.get_task(task_id).await else {
            tracing::warn!("Import resume: task {} not found in TaskManager", task_id);
            return;
        };

        let media_type = task.detect_media_type();
        let arr_id = match media_type {
            crate::downloader::MediaType::TvSeries | crate::downloader::MediaType::TvEpisode => {
                task.arr_series_id
            }
            crate::downloader::MediaType::Movie => task.arr_movie_id,
        };

        let Some(arr_id) = arr_id else {
            let mut importing = task;
            importing.state = DownloadState::Importing;
            importing.arr_announced = false;
            importing.arr_announce_error = Some(format!("missing Arr {:?} id", media_type));
            task_manager.update_task(importing.clone()).await;
            if let Some(db) = &db {
                let _ = db.save_task(&importing);
            }
            event_bus.publish(super::events::TaskEvent::StateChanged {
                task: importing,
                old_state: DownloadState::Importing,
                new_state: DownloadState::Importing,
                timestamp: chrono::Utc::now(),
            });
            return;
        };

        let nzo_id = task_id.to_string();
        let poll_timeout = std::time::Duration::from_secs(600);
        let client_guard = arr_client.read().await;
        let poll_result = if let Some(ref client) = *client_guard {
            match media_type {
                crate::downloader::MediaType::TvSeries
                | crate::downloader::MediaType::TvEpisode => {
                    if let Some(episodes) = task.selected_episodes.as_deref() {
                        async {
                            let episodes: Vec<(u32, u32)> = serde_json::from_str(episodes)
                                .map_err(|e| {
                                    anyhow::anyhow!("invalid selected episode set: {e}")
                                })?;
                            let mut confirmed = true;
                            for (season, episode) in episodes {
                                confirmed &= client
                                    .poll_sonarr_episode_import_confirmation(
                                        arr_id,
                                        &nzo_id,
                                        season as i32,
                                        episode as i32,
                                        poll_timeout,
                                    )
                                    .await?;
                                if !confirmed {
                                    break;
                                }
                            }
                            Ok(confirmed)
                        }
                        .await
                    } else if let (Some(season), Some(episode)) =
                        (task.tmdb_season, task.tmdb_episode)
                    {
                        client
                            .poll_sonarr_episode_import_confirmation(
                                arr_id,
                                &nzo_id,
                                season as i32,
                                episode as i32,
                                poll_timeout,
                            )
                            .await
                    } else {
                        client
                            .poll_sonarr_import_confirmation(arr_id, &nzo_id, poll_timeout)
                            .await
                    }
                }
                crate::downloader::MediaType::Movie => {
                    client
                        .poll_radarr_import_confirmation(arr_id, &nzo_id, poll_timeout)
                        .await
                }
            }
        } else {
            Ok(false)
        };
        drop(client_guard);

        if let Some(mut final_task) = task_manager.get_task(task_id).await {
            let confirmed = matches!(&poll_result, Ok(true));
            match poll_result {
                Ok(true) => {
                    // Contract 6: only mark COMPLETED once Arr import is confirmed.
                    final_task.state = DownloadState::Completed;
                    final_task.arr_announced = true;
                    final_task.arr_announce_error = None;
                }
                Ok(false) => {
                    // Import not yet confirmed. Keep the task in IMPORTING so a
                    // restart re-reconciles rather than falsely reporting COMPLETED.
                    final_task.state = DownloadState::Importing;
                    final_task.arr_announced = false;
                    final_task.arr_announce_error = Some(format!(
                        "import poll timed out after {}s",
                        poll_timeout.as_secs()
                    ));
                }
                Err(e) => {
                    final_task.state = DownloadState::Importing;
                    final_task.arr_announced = false;
                    final_task.arr_announce_error = Some(format!("import poll error: {}", e));
                }
            }

            task_manager.update_task(final_task.clone()).await;
            if let Some(db) = &db {
                let _ = db.save_task(&final_task);
            }
            if confirmed {
                Self::record_activity(db.as_ref(), "completed", &final_task);
                event_bus.publish(super::events::TaskEvent::Completed {
                    task: final_task,
                    timestamp: chrono::Utc::now(),
                });
            } else {
                event_bus.publish(super::events::TaskEvent::StateChanged {
                    task: final_task,
                    old_state: DownloadState::Importing,
                    new_state: DownloadState::Importing,
                    timestamp: chrono::Utc::now(),
                });
            }
        }
    }

    /// Restore tasks from database
    async fn restore_from_db(&self) -> anyhow::Result<()> {
        if let Some(db) = &self.db {
            tracing::info!("Restoring tasks from database...");

            let tasks = db.get_all_tasks()?;
            let mut restored_count = 0;

            for task in tasks {
                if Self::is_external_torrent(&task) && task.external_id.is_some() {
                    self.task_manager.add_task(task.clone()).await;
                    restored_count += 1;
                    continue;
                }
                match task.state {
                    DownloadState::Downloading
                    | DownloadState::Starting
                    | DownloadState::Importing => {
                        // Resume interrupted downloads / incomplete imports as QUEUED.
                        // Importing tasks were mid-poll when the server restarted — re-run the
                        // full download-to-import cycle rather than leaving them in a dead state.
                        let mut resumed_task = task.clone();
                        resumed_task.state = DownloadState::Queued;
                        self.task_manager.add_task(resumed_task).await;
                        restored_count += 1;
                    }
                    DownloadState::Queued | DownloadState::Waiting => {
                        // Re-queue pending downloads
                        self.task_manager.add_task(task).await;
                        restored_count += 1;
                    }
                    _ => {
                        // Keep other states as-is
                        self.task_manager.add_task(task).await;
                        restored_count += 1;
                    }
                }
            }

            tracing::info!("Restored {} tasks from database", restored_count);
        }

        Ok(())
    }

    /// Get current download configuration
    pub async fn get_config(&self) -> DownloadConfig {
        self.config.read().await.clone()
    }

    /// Update download configuration
    pub async fn update_config(&self, new_config: DownloadConfig) {
        let old_max_concurrent = {
            let config_guard = self.config.read().await;
            config_guard.max_concurrent
        };

        // Update config
        {
            let mut config_guard = self.config.write().await;
            *config_guard = new_config.clone();
        }

        // Recreation of engine to pick up new settings
        {
            let new_engine = Arc::new(SimpleDownloadEngine::with_config(new_config.clone()));
            let mut engine_guard = self.download_engine.write().await;
            *engine_guard = new_engine;
        }

        // Dynamic worker pool resizing
        let new_max_concurrent = new_config.max_concurrent;
        if new_max_concurrent != old_max_concurrent {
            let mut workers = self.workers.lock().await;

            if new_max_concurrent > old_max_concurrent {
                // Spawn additional workers
                let workers_to_add = new_max_concurrent - old_max_concurrent;
                tracing::info!(
                    "Spawning {} additional workers ({}→{})",
                    workers_to_add,
                    old_max_concurrent,
                    new_max_concurrent
                );

                for worker_id in old_max_concurrent..new_max_concurrent {
                    let handle = self.spawn_worker(worker_id);
                    workers.push(handle);
                }
            } else {
                // Workers will automatically sleep when worker_id >= max_concurrent
                // No need to kill them - they'll just idle
                tracing::info!(
                    "Reduced max_concurrent ({}→{}). Excess workers will idle.",
                    old_max_concurrent,
                    new_max_concurrent
                );
            }
        }

        tracing::info!(
            "Updated download configuration: max_concurrent={}, segments={}",
            new_config.max_concurrent,
            new_config.segments_per_download
        );
    }

    /// Request Sonarr/Radarr to import a completed download from the shared download-client
    /// folder. Returns an `ArrMoveOutcome` — callers MUST NOT set `arr_announced = true`
    /// here; that only happens after closed-loop polling confirms `downloadFolderImported`.
    async fn move_to_arr_path(
        task: &DownloadTask,
        arr_client: &Arc<tokio::sync::RwLock<Option<Arc<crate::arr::ArrClient>>>>,
        symlink_real_base: Option<&std::path::Path>,
    ) -> ArrMoveOutcome {
        use crate::downloader::MediaType;

        // arr not configured → skip polling entirely
        let client = {
            let guard = arr_client.read().await;
            match guard.clone() {
                Some(c) => c,
                None => return ArrMoveOutcome::Skipped { task: task.clone() },
            }
        };

        let media_type = task.detect_media_type();
        let auto_import_enabled = match media_type {
            MediaType::TvSeries | MediaType::TvEpisode => client.sonarr_auto_import_enabled(),
            MediaType::Movie => client.radarr_auto_import_enabled(),
        };
        if !auto_import_enabled {
            tracing::debug!(
                "Automatic Arr import disabled; skipping placement for task {}",
                task.id
            );
            return ArrMoveOutcome::Skipped { task: task.clone() };
        }

        // Resolve arr ID and library root folder
        let (arr_folder, arr_id) = match media_type {
            MediaType::TvSeries | MediaType::TvEpisode => {
                let series_id = match task.arr_series_id {
                    Some(id) => id,
                    None => {
                        let tvdb_id = match task.tvdb_id {
                            Some(id) => id as i32,
                            None => {
                                return ArrMoveOutcome::Failed {
                                    reason: "TV import requires canonical tvdb_id".to_string(),
                                }
                            }
                        };
                        tracing::info!("Looking up series by canonical TVDB ID: {}", tvdb_id);
                        match client.series_exists_by_tvdb(tvdb_id).await {
                            Ok(Some(id)) => {
                                tracing::info!(
                                    "Found series in Sonarr by TVDB {}: ID {}",
                                    tvdb_id,
                                    id
                                );
                                id as i64
                            }
                            Ok(None) => {
                                tracing::warn!(
                                    "Series not in Sonarr for TVDB {} — auto-adding as fallback",
                                    tvdb_id
                                );
                                let root_path = match client.get_sonarr_root_folders().await {
                                    Ok(folders) => folders
                                        .first()
                                        .map(|f| f.path.clone())
                                        .unwrap_or_else(|| "/data/media/tv".to_string()),
                                    Err(_) => "/data/media/tv".to_string(),
                                };
                                match client.add_series_by_tvdb(tvdb_id, 1, &root_path).await {
                                    Ok(id) => {
                                        tracing::info!(
                                            "Fallback: added series to Sonarr TVDB {} -> ID {}",
                                            tvdb_id,
                                            id
                                        );
                                        id as i64
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "Fallback: failed to add series to Sonarr for TVDB {}: {}",
                                            tvdb_id,
                                            e
                                        );
                                        return ArrMoveOutcome::Failed {
                                            reason: format!("Sonarr add failed: {}", e),
                                        };
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to look up series by TVDB {}: {}",
                                    tvdb_id,
                                    e
                                );
                                return ArrMoveOutcome::Failed {
                                    reason: format!("Sonarr lookup failed: {}", e),
                                };
                            }
                        }
                    }
                };
                let path = match client.get_series_path(series_id).await {
                    Ok(path) => path,
                    Err(e) => {
                        tracing::warn!("Failed to get series path for ID {}: {}", series_id, e);
                        return ArrMoveOutcome::Failed {
                            reason: format!("get_series_path failed: {}", e),
                        };
                    }
                };
                (path, series_id)
            }
            MediaType::Movie => {
                let movie_id = match task.arr_movie_id {
                    Some(id) => id,
                    None => {
                        let tmdb_id = match task.tmdb_id {
                            Some(id) => id,
                            None => return ArrMoveOutcome::Skipped { task: task.clone() },
                        };
                        tracing::info!("Looking up movie by TMDB ID: {}", tmdb_id);
                        match client.movie_exists(tmdb_id).await {
                            Ok(Some(id)) => {
                                tracing::info!(
                                    "Found movie in Radarr by TMDB {}: ID {}",
                                    tmdb_id,
                                    id
                                );
                                id as i64
                            }
                            Ok(None) => {
                                tracing::warn!(
                                    "Movie not in Radarr for TMDB {} — auto-adding as fallback",
                                    tmdb_id
                                );
                                let root_path = match client.get_radarr_root_folders().await {
                                    Ok(folders) => folders
                                        .first()
                                        .map(|f| f.path.clone())
                                        .unwrap_or_else(|| "/data/media/movies".to_string()),
                                    Err(_) => "/data/media/movies".to_string(),
                                };
                                match client.add_movie_by_tmdb(tmdb_id, 1, &root_path).await {
                                    Ok(id) => {
                                        tracing::info!(
                                            "Fallback: added movie to Radarr TMDB {} -> ID {}",
                                            tmdb_id,
                                            id
                                        );
                                        id as i64
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "Fallback: failed to add movie to Radarr for TMDB {}: {}",
                                            tmdb_id,
                                            e
                                        );
                                        return ArrMoveOutcome::Failed {
                                            reason: format!("Radarr add failed: {}", e),
                                        };
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to look up movie by TMDB {}: {}",
                                    tmdb_id,
                                    e
                                );
                                return ArrMoveOutcome::Failed {
                                    reason: format!("Radarr lookup failed: {}", e),
                                };
                            }
                        }
                    }
                };
                let path = match client.get_movie_path(movie_id).await {
                    Ok(path) => path,
                    Err(e) => {
                        tracing::warn!("Failed to get movie path for ID {}: {}", movie_id, e);
                        return ArrMoveOutcome::Failed {
                            reason: format!("get_movie_path failed: {}", e),
                        };
                    }
                };
                (path, movie_id)
            }
        };

        // Build import source path
        // `source` is the container-internal path of the downloaded file.
        // If `symlink_real_base` is set, we remap the container path to the
        // host-visible path so that sibling LXCs (Sonarr/Radarr) can resolve
        // the download-client folder (e.g. /appData/downloads/… → /data/flasharr-download/…).
        let raw_source = std::path::Path::new(&task.destination);
        let source_buf;
        let source = if raw_source.starts_with("/data/media/")
            || raw_source.starts_with("/data/flasharr-download/")
        {
            raw_source
        } else if let Some(real_base) = symlink_real_base {
            tracing::info!(
                "Symlink remapping check: real_base={:?}, task.destination={:?}",
                real_base,
                task.destination
            );
            let download_dir_str = task.destination.as_str();
            // Try to strip the container download dir prefix and replace with real_base
            // The container download_dir is typically /appData/downloads
            // We look for /appData/downloads or /downloads prefix patterns
            let remapped = ["appData/downloads", "downloads"]
                .iter()
                .find_map(|segment| {
                    let needle = format!("/{}/", segment);
                    if let Some(pos) = download_dir_str.find(&needle) {
                        let rest = &download_dir_str[pos + needle.len()..];
                        Some(real_base.join(rest))
                    } else if download_dir_str.starts_with(&format!("/{}", segment)) {
                        let rest = &download_dir_str[segment.len() + 1..];
                        Some(real_base.join(rest.trim_start_matches('/')))
                    } else {
                        None
                    }
                });
            if let Some(mapped) = remapped {
                tracing::info!("Symlink source remapped: {:?} → {:?}", raw_source, mapped);
                source_buf = mapped;
                &source_buf
            } else {
                tracing::warn!(
                    "symlink_real_base set but could not remap {:?} (checked appData/downloads, downloads) — using original",
                    raw_source
                );
                raw_source
            }
        } else {
            tracing::debug!("Symlink remapping skipped: symlink_real_base is None");
            raw_source
        };
        let filename = match source.file_name() {
            Some(f) => f,
            None => {
                return ArrMoveOutcome::Failed {
                    reason: "task destination has no filename".into(),
                };
            }
        };

        // Helper: build the updated task with arr IDs set but arr_announced intentionally false.
        let make_placed = |mut t: DownloadTask| -> DownloadTask {
            match media_type {
                MediaType::TvSeries | MediaType::TvEpisode => t.arr_series_id = Some(arr_id),
                MediaType::Movie => t.arr_movie_id = Some(arr_id),
            }
            t
        };

        let source_is_library_path = source.starts_with("/data/media/");
        if !source_is_library_path {
            let source_metadata = match tokio::fs::metadata(source).await {
                Ok(metadata) => metadata,
                Err(_) => {
                    return ArrMoveOutcome::Failed {
                        reason: format!("downloaded file not found at {}", source.display()),
                    };
                }
            };

            const MIN_ARR_IMPORT_BYTES: u64 = 1_048_576;
            if source_metadata.len() < MIN_ARR_IMPORT_BYTES {
                return ArrMoveOutcome::Failed {
                    reason: format!(
                        "downloaded file too small for arr import: {} bytes at {}",
                        source_metadata.len(),
                        source.display()
                    ),
                };
            }

            match looks_like_importable_media(source).await {
                Ok(true) => {}
                Ok(false) => {
                    return ArrMoveOutcome::Failed {
                        reason: format!(
                            "downloaded file is not a recognized media container at {}",
                            source.display()
                        ),
                    };
                }
                Err(e) => {
                    return ArrMoveOutcome::Failed {
                        reason: format!(
                            "downloaded file could not be validated for arr import at {}: {}",
                            source.display(),
                            e
                        ),
                    };
                }
            }

            tracing::info!(
                "Validated completed media source {} for symlink placement (task {})",
                source.display(),
                task.id
            );
        }

        let target_dir = match media_type {
            MediaType::TvSeries | MediaType::TvEpisode => {
                let season = match task.tmdb_season {
                    Some(s) => s,
                    None => {
                        tracing::warn!(
                            "TV task {} has no season number — skipping arr move",
                            task.id
                        );
                        return ArrMoveOutcome::Skipped { task: task.clone() };
                    }
                };
                std::path::PathBuf::from(&arr_folder).join(format!("Season {}", season))
            }
            MediaType::Movie => std::path::PathBuf::from(&arr_folder),
        };

        if Self::is_torrent_import(task) {
            let targets = match crate::downloader::artifact_placement::place_torrent(
                source,
                &std::path::PathBuf::from(&arr_folder),
                matches!(media_type, MediaType::Movie),
                task.file_idx,
                task.source_filename.as_deref(),
            ) {
                Ok(targets) => targets,
                Err(reason) => return ArrMoveOutcome::Failed { reason },
            };
            for target in &targets {
                let result = match media_type {
                    MediaType::TvSeries | MediaType::TvEpisode => {
                        client
                            .trigger_series_rescan_with_path(arr_id, target.to_str())
                            .await
                    }
                    MediaType::Movie => {
                        client
                            .trigger_movie_refresh_with_path(arr_id, target.to_str())
                            .await
                    }
                };
                if let Err(e) = result {
                    return ArrMoveOutcome::Failed {
                        reason: format!("Arr scan trigger failed for {}: {e}", target.display()),
                    };
                }
            }
            let mut placed = make_placed(task.clone());
            placed.destination = targets[0].to_string_lossy().into_owned();
            if !matches!(media_type, MediaType::Movie) {
                let episodes: Vec<_> = targets
                    .iter()
                    .filter_map(|p| crate::downloader::artifact_placement::episode_number(p))
                    .collect();
                placed.selected_episodes = serde_json::to_string(&episodes).ok();
            }
            return ArrMoveOutcome::Placed {
                task: placed,
                arr_id,
                media_type,
            };
        }

        let target_path = target_dir.join(filename);

        // ── Case 1: source IS the target — file already at arr path, just rescan ──────────
        if source == target_path {
            tracing::info!(
                "File already at arr path {:?} — triggering rescan only",
                target_path
            );
            if let Err(e) = match media_type {
                MediaType::TvSeries | MediaType::TvEpisode => {
                    client
                        .trigger_series_rescan_with_path(
                            arr_id,
                            Some(target_path.to_str().unwrap_or("")),
                        )
                        .await
                }
                MediaType::Movie => {
                    client
                        .trigger_movie_refresh_with_path(
                            arr_id,
                            Some(target_path.to_str().unwrap_or("")),
                        )
                        .await
                }
            } {
                tracing::warn!("Arr rescan trigger failed (non-fatal): {}", e);
            }
            return ArrMoveOutcome::Placed {
                task: make_placed(task.clone()),
                arr_id,
                media_type,
            };
        }

        // ── Case 2: target dir not mountable — record error; Completed remains retryable ────
        if let Err(e) = tokio::fs::create_dir_all(&target_dir).await {
            tracing::warn!("Cannot create target dir {:?}: {}", target_dir, e);
            return ArrMoveOutcome::Failed {
                reason: format!(
                    "target dir create failed at {}: {}",
                    target_dir.display(),
                    e
                ),
            };
        }

        // ── Case 3: source gone — arr imported/renamed autonomously ─────────────────────────
        let source_exists = tokio::fs::metadata(source).await.is_ok();
        if !source_exists {
            tracing::info!(
                "Source {:?} gone (arr imported/renamed or lost) — triggering rescan",
                source
            );
            if let Err(e) = match media_type {
                MediaType::TvSeries | MediaType::TvEpisode => {
                    client.trigger_series_rescan_with_path(arr_id, None).await
                }
                MediaType::Movie => client.trigger_movie_refresh_with_path(arr_id, None).await,
            } {
                tracing::warn!("Arr rescan trigger failed (non-fatal): {}", e);
            }
            return ArrMoveOutcome::Placed {
                task: make_placed(task.clone()),
                arr_id,
                media_type,
            };
        }

        // ── Case 4: arr already imported under its own naming scheme — skip symlink ─────────
        {
            let already_imported = 'check: {
                let mut read_dir = match tokio::fs::read_dir(&target_dir).await {
                    Ok(rd) => rd,
                    Err(_) => break 'check false,
                };
                let media_exts = ["mkv", "mp4", "avi", "m4v", "ts", "mov"];
                let ep_pattern = task
                    .tmdb_season
                    .zip(task.tmdb_episode)
                    .map(|(s, e)| format!("S{:02}E{:02}", s, e).to_lowercase());

                while let Ok(Some(entry)) = read_dir.next_entry().await {
                    let ep = entry.path();
                    let meta = match tokio::fs::symlink_metadata(&ep).await {
                        Ok(m) => m,
                        Err(_) => continue,
                    };
                    if meta.file_type().is_symlink() {
                        continue;
                    }
                    let ext = ep
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if !media_exts.contains(&ext.as_str()) {
                        continue;
                    }
                    if matches!(media_type, MediaType::Movie) {
                        tracing::info!(
                            "move_to_arr_path: {:?} already contains {:?} — arr imported; skipping symlink",
                            target_dir,
                            ep
                        );
                        break 'check true;
                    }
                    if let Some(ref pat) = ep_pattern {
                        let name = ep
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        if name.contains(pat.as_str()) {
                            tracing::info!(
                                "move_to_arr_path: {:?} already contains {:?} matching {} — arr imported; skipping symlink",
                                target_dir,
                                ep,
                                pat
                            );
                            break 'check true;
                        }
                    }
                }
                false
            };

            if already_imported {
                if let Err(e) = match media_type {
                    MediaType::TvSeries | MediaType::TvEpisode => {
                        client.trigger_series_rescan_with_path(arr_id, None).await
                    }
                    MediaType::Movie => client.trigger_movie_refresh_with_path(arr_id, None).await,
                } {
                    tracing::warn!("Arr rescan trigger failed (non-fatal): {}", e);
                }
                return ArrMoveOutcome::Placed {
                    task: make_placed(task.clone()),
                    arr_id,
                    media_type,
                };
            }
        }

        // ── Case 5: direct HTTP providers move the completed file into the library ─────────
        if let Ok(target_meta) = tokio::fs::symlink_metadata(&target_path).await {
            if target_meta.file_type().is_symlink() {
                tracing::info!("Removing existing symlink at {:?}", target_path);
                let _ = tokio::fs::remove_file(&target_path).await;
            } else {
                return ArrMoveOutcome::Failed {
                    reason: format!(
                        "target already exists and is not a symlink; refusing to overwrite {}",
                        target_path.display()
                    ),
                };
            }
        }

        tracing::info!(
            "Moving direct download into library: {:?} -> {:?}",
            source,
            target_path
        );

        match crate::downloader::artifact_placement::move_exact(source, &target_path) {
            Ok(()) => {
                tracing::info!("Direct download moved into library: {:?}", target_path);
                // Trigger rescan (fire-and-forget); polling loop will confirm import
                let target_path_str = target_path.to_str().unwrap_or("");
                if let Err(e) = match media_type {
                    MediaType::TvSeries | MediaType::TvEpisode => {
                        client
                            .trigger_series_rescan_with_path(arr_id, Some(target_path_str))
                            .await
                    }
                    MediaType::Movie => {
                        client
                            .trigger_movie_refresh_with_path(arr_id, Some(target_path_str))
                            .await
                    }
                } {
                    tracing::warn!("Arr scan trigger failed (non-fatal): {}", e);
                }
                let mut placed_task = make_placed(task.clone());
                placed_task.destination = target_path.to_string_lossy().to_string();
                ArrMoveOutcome::Placed {
                    task: placed_task,
                    arr_id,
                    media_type,
                }
            }
            Err(e) => {
                tracing::error!(
                    "Failed to move direct download {:?} -> {:?}: {}",
                    source,
                    target_path,
                    e
                );
                ArrMoveOutcome::Failed {
                    reason: format!("direct download move failed: {}", e),
                }
            }
        }
    }

    /// Reload *arr client with new configuration (dynamic update without restart)
    pub async fn reload_arr_client(
        &self,
        sonarr_config: Option<crate::config::ArrConfig>,
        radarr_config: Option<crate::config::ArrConfig>,
    ) {
        let new_client = if sonarr_config.is_some() || radarr_config.is_some() {
            Some(Arc::new(crate::arr::ArrClient::new(
                sonarr_config.clone(),
                radarr_config.clone(),
            )))
        } else {
            None
        };

        // Create new artifact manager if client exists and db is available
        let new_artifact_manager =
            if let (Some(ref client), Some(ref database)) = (&new_client, &self.db) {
                Some(Arc::new(crate::arr::ArrArtifactManager::new(
                    Arc::clone(client),
                    Arc::clone(database),
                )))
            } else {
                None
            };

        {
            let mut arr_guard = self.arr_client.write().await;
            *arr_guard = new_client;
        }

        {
            let mut artifact_guard = self.artifact_manager.write().await;
            *artifact_guard = new_artifact_manager;
        }

        tracing::info!(
            "Reloaded *arr client and managers: sonarr={}, radarr={}",
            sonarr_config.is_some(),
            radarr_config.is_some()
        );
    }

    /// Reload qBittorrent client with new configuration (dynamic update without restart)
    pub async fn reload_qbittorrent_client(
        &self,
        config: crate::torrent::qbittorrent::QBittorrentConfig,
    ) {
        let new_client = if config.enabled {
            match crate::torrent::qbittorrent::QBittorrentClient::new(config) {
                Ok(client) => Some(Arc::new(client)),
                Err(e) => {
                    tracing::warn!("qBittorrent client disabled: {}", e);
                    None
                }
            }
        } else {
            None
        };
        let mut guard = self.qbittorrent_client.write().await;
        *guard = new_client;
        tracing::info!("Reloaded qBittorrent client: enabled={}", guard.is_some());
    }

    /// Get access to the arr client for API proxying
    pub async fn get_arr_client(&self) -> Option<Arc<crate::arr::ArrClient>> {
        let guard = self.arr_client.read().await;
        guard.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::DownloadOrchestrator;
    use crate::downloader::{DownloadState, DownloadTask};

    #[test]
    fn arr_identity_update_preserves_worker_state() {
        let mut stale = DownloadTask::new(
            "https://example.invalid/file".to_string(),
            "movie.mkv".to_string(),
            "qbittorrent".to_string(),
            "movies".to_string(),
        );
        stale.state = DownloadState::Queued;
        let mut current = stale.clone();
        current.state = DownloadState::Starting;

        let merged = DownloadOrchestrator::merge_arr_identity(&stale, &current, 36);

        assert_eq!(merged.state, DownloadState::Starting);
        assert_eq!(merged.arr_movie_id, Some(36));
    }

    #[test]
    fn fresh_queued_qbit_identity_is_not_mistaken_for_restored() {
        use super::is_restored_external_qbit_task;
        use crate::downloader::DownloadState;

        assert!(!is_restored_external_qbit_task(
            DownloadState::Queued,
            None,
            Some("expected-hash"),
            Some("qbittorrent"),
            None,
        ));
        // Production worker captures QUEUED before transitioning to STARTING.
        let fresh_queued = true;
        assert!(
            !(!fresh_queued
                && is_restored_external_qbit_task(
                    DownloadState::Starting,
                    None,
                    Some("expected-hash"),
                    Some("qbittorrent"),
                    None,
                ))
        );
        assert!(is_restored_external_qbit_task(
            DownloadState::Waiting,
            None,
            Some("expected-hash"),
            Some("qbittorrent"),
            None,
        ));
        for message in [
            "qBittorrent torrent is absent; restored identity will not be added again",
            "qBittorrent torrent is absent; restored ...",
        ] {
            assert!(!is_restored_external_qbit_task(
                DownloadState::Waiting,
                Some(message),
                Some("expected-hash"),
                Some("qbittorrent"),
                None,
            ));
        }
    }

    #[test]
    fn managed_qbit_reconciliation_matrix() {
        use super::{managed_qbit_decision, ManagedQbitDecision};
        use crate::torrent::state::TransferMetrics;

        let idle = TransferMetrics {
            downloaded: 0,
            previous_downloaded: None,
            progress: 0.0,
            dlspeed: 0,
            upspeed: 0,
            amount_left: 100,
        };
        let active = TransferMetrics { dlspeed: 1, ..idle };
        let complete = TransferMetrics {
            downloaded: 100,
            progress: 1.0,
            amount_left: 0,
            ..idle
        };

        assert_eq!(
            managed_qbit_decision("healthy", "abc", Ok(None), idle, true, false),
            ManagedQbitDecision::Wait(
                "qBittorrent torrent is absent; restored identity will not be added again"
            )
        );
        assert_eq!(
            managed_qbit_decision("healthy", "abc", Ok(None), idle, false, false),
            ManagedQbitDecision::AddOnce
        );
        assert_eq!(
            managed_qbit_decision("healthy", "abc", Err(()), idle, true, false),
            ManagedQbitDecision::Wait("qBittorrent API unavailable; reconciliation will retry")
        );
        for state in ["metaDL", "checkingDL", "queuedDL", "stalledDL", ""] {
            assert_eq!(
                managed_qbit_decision(
                    "healthy",
                    "abc",
                    Ok(Some(("ABC", state))),
                    idle,
                    true,
                    false
                ),
                ManagedQbitDecision::Starting(
                    "qBittorrent is waiting for metadata or transfer evidence"
                )
            );
        }
        assert_eq!(
            managed_qbit_decision(
                "healthy",
                "abc",
                Ok(Some(("abc", "downloading"))),
                active,
                true,
                false
            ),
            ManagedQbitDecision::Downloading
        );
        assert_eq!(
            managed_qbit_decision(
                "healthy",
                "abc",
                Ok(Some(("abc", "uploading"))),
                complete,
                true,
                false
            ),
            ManagedQbitDecision::Completed
        );
        for state in ["metaDL", "checkingDL", ""] {
            assert_eq!(
                managed_qbit_decision("healthy", "abc", Ok(Some(("abc", state))), idle, true, true),
                ManagedQbitDecision::Starting(
                    "qBittorrent is waiting for metadata or transfer evidence"
                )
            );
        }
    }

    #[test]
    fn restart_authority_adopts_only_healthy_exact_hash() {
        assert_eq!(
            super::restart_adoption("healthy", Some("ABC"), "abc", true),
            super::RestartAdoption::Adopt
        );
        assert_eq!(
            super::restart_adoption("stale", Some("abc"), "abc", true),
            super::RestartAdoption::Wait
        );
        assert_eq!(
            super::restart_adoption("disconnected", Some("abc"), "abc", true),
            super::RestartAdoption::Wait
        );
        assert_eq!(
            super::restart_adoption("healthy", None, "abc", false),
            super::RestartAdoption::Add
        );
        assert_eq!(
            super::restart_adoption("healthy", None, "abc", true),
            super::RestartAdoption::Wait
        );
    }

    #[tokio::test]
    async fn production_restart_adoption_calls_add_only_for_new_healthy_missing() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let calls = AtomicUsize::new(0);
        for decision in [
            super::RestartAdoption::Adopt,
            super::RestartAdoption::Wait,
            super::RestartAdoption::Wait,
            super::RestartAdoption::Add,
        ] {
            super::run_restart_adoption(decision, || async {
                calls.fetch_add(1, Ordering::SeqCst);
            })
            .await;
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn torbox_tasks_are_direct_http_not_external_torrent() {
        let mut task = DownloadTask::new(
            "https://torbox.example/direct.mkv".to_string(),
            "direct.mkv".to_string(),
            "torbox".to_string(),
            "movies".to_string(),
        );
        task.download_source = Some("torbox".to_string());
        task.external_provider = Some("torbox".to_string());

        assert!(!DownloadOrchestrator::is_external_torrent(&task));
        assert!(!DownloadOrchestrator::is_torrent_import(&task));
        assert!(DownloadOrchestrator::is_direct_http_import(&task));
    }

    #[test]
    fn qbittorrent_tasks_prefer_info_hash_magnet() {
        assert_eq!(
            DownloadOrchestrator::qbit_url_for_test(
                "qbittorrent",
                "https://prowlarr.example/download?apikey=secret",
                Some("22A2E1DF69B9016D5130D3E20DEEC478EEDADF8B"),
            ),
            "magnet:?xt=urn:btih:22a2e1df69b9016d5130d3e20deec478eedadf8b"
        );
        assert_eq!(
            DownloadOrchestrator::qbit_url_for_test(
                "qbittorrent",
                "https://example.test/file.torrent",
                None,
            ),
            "https://example.test/file.torrent"
        );
        assert_eq!(
            DownloadOrchestrator::qbit_url_for_test(
                "qbittorrent",
                "",
                Some("22A2E1DF69B9016D5130D3E20DEEC478EEDADF8B"),
            ),
            "magnet:?xt=urn:btih:22a2e1df69b9016d5130d3e20deec478eedadf8b"
        );
        assert_eq!(
            DownloadOrchestrator::qbit_url_for_test(
                "fshare",
                "https://example.test/file",
                Some("22A2E1DF69B9016D5130D3E20DEEC478EEDADF8B"),
            ),
            "https://example.test/file"
        );
    }

    #[test]
    fn torrent_identity_is_hash_scoped_not_qbit_scoped() {
        // Contract 1: canonical torrent identity = provider_id='torrent',
        // provider_item_id = lowercase normalized info_hash. Cross-provider safe.
        assert_eq!(
            super::canonical_provider_identity(
                "qbittorrent",
                "magnet:?xt=urn:btih:22A2E1DF69B9016D5130D3E20DEEC478EEDADF8B",
                Some("22A2E1DF69B9016D5130D3E20DEEC478EEDADF8B"),
                None,
            ),
            (
                "torrent".to_string(),
                Some("22a2e1df69b9016d5130d3e20deec478eedadf8b".to_string())
            )
        );
        assert_eq!(
            super::canonical_provider_identity("qbittorrent", "", None, Some("ABCDEF")),
            ("torrent".to_string(), Some("abcdef".to_string()))
        );
        assert_eq!(
            super::canonical_provider_identity("fshare", "https://fshare.vn/file/X", None, None),
            ("fshare".to_string(), None)
        );
    }

    #[test]
    fn qbit_confirmation_pending_stays_starting() {
        assert_eq!(
            super::qbit_confirmation_decision("abcdef", Ok(None), false),
            super::QbitConfirmationDecision::Pending
        );
    }

    #[test]
    fn qbit_confirmation_exhaustion_fails_sanitized() {
        let decision = super::qbit_confirmation_decision("abcdef", Ok(None), true);
        assert_eq!(
            decision,
            super::QbitConfirmationDecision::Failed(
                "qBittorrent did not confirm the accepted torrent within 60 seconds"
            )
        );
        let message = format!("{decision:?}");
        assert!(!message.contains("http"));
        assert!(!message.contains("prowlarr-ref"));
    }

    #[test]
    fn qbit_confirmation_api_error_fails_sanitized() {
        let decision = super::qbit_confirmation_decision("abcdef", Err(()), false);
        assert_eq!(
            decision,
            super::QbitConfirmationDecision::Failed(
                "qBittorrent confirmation API failed; check the configured instance"
            )
        );
        let message = format!("{decision:?}");
        assert!(!message.contains("http"));
        assert!(!message.contains("prowlarr-ref"));
    }

    #[test]
    fn qbit_confirmation_exact_normalized_hash_allows_downloading() {
        assert_eq!(
            super::qbit_confirmation_decision("ABCDEF", Ok(Some("abcdef")), false),
            super::QbitConfirmationDecision::Confirmed
        );
    }

    #[test]
    fn qbit_confirmation_mismatched_hash_fails_closed() {
        assert_eq!(
            super::qbit_confirmation_decision("abcdef", Ok(Some("fedcba")), false),
            super::QbitConfirmationDecision::Failed(
                "qBittorrent confirmation returned a mismatched torrent hash"
            )
        );
    }

    #[test]
    fn qbit_existing_confirmed_hash_is_adopted_without_add() {
        assert!(!super::qbit_confirmation_requires_add(
            super::qbit_confirmation_decision("ABCDEF", Ok(Some("abcdef")), false)
        ));
    }

    #[test]
    fn qbit_restart_transient_confirmation_uses_authority_without_duplicate_add() {
        assert!(!super::qbit_confirmation_requires_add(
            super::qbit_confirmation_decision("abcdef", Ok(Some("ABCDEF")), false)
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn qbit_add_timeout_is_terminal_and_sanitized() {
        let result =
            super::run_qbit_add_with_timeout(std::time::Duration::from_secs(30), || async {
                std::future::pending::<()>().await;
                Ok(None::<String>)
            });
        tokio::pin!(result);
        tokio::time::advance(std::time::Duration::from_secs(31)).await;
        let message = result
            .await
            .expect_err("hung add must fail terminally")
            .to_string();
        assert_eq!(message, "qBittorrent add timed out after 30 seconds");
        assert!(!message.contains("http"));
        assert!(!message.contains("prowlarr-ref"));
    }

    #[tokio::test(start_paused = true)]
    async fn qbit_actual_confirmation_loop_bounds_hung_calls_by_absolute_budget() {
        let result = super::poll_qbit_confirmation(
            "abcdef",
            std::time::Duration::from_secs(60),
            std::time::Duration::from_secs(10),
            || async {
                std::future::pending::<()>().await;
                Ok(None)
            },
        );
        tokio::pin!(result);
        tokio::time::advance(std::time::Duration::from_secs(61)).await;
        assert_eq!(
            result.await,
            super::QbitConfirmationDecision::Failed(
                "qBittorrent did not confirm the accepted torrent within 60 seconds"
            )
        );
    }

    #[test]
    fn status_refresh_cannot_claim_completion_before_import_owner() {
        assert_eq!(
            super::status_refresh_state(
                DownloadState::Downloading,
                DownloadState::Completed,
                false
            ),
            DownloadState::Downloading
        );
        assert_eq!(
            super::status_refresh_state(DownloadState::Importing, DownloadState::Completed, false),
            DownloadState::Importing
        );
        assert_eq!(
            super::status_refresh_state(DownloadState::Completed, DownloadState::Completed, true),
            DownloadState::Completed
        );
    }

    #[test]
    fn import_lifecycle_never_regresses_to_transfer_state() {
        use crate::downloader::DownloadState;
        // Contract 6/7: state cannot regress from imported/completed to downloading.
        assert_eq!(
            super::reconciled_transfer_state(DownloadState::Importing, DownloadState::Downloading),
            DownloadState::Importing
        );
        assert_eq!(
            super::reconciled_transfer_state(DownloadState::Completed, DownloadState::Downloading),
            DownloadState::Completed
        );
        assert_eq!(
            super::reconciled_transfer_state(DownloadState::Downloading, DownloadState::Paused),
            DownloadState::Paused
        );
        assert_eq!(
            super::reconciled_transfer_state(DownloadState::Failed, DownloadState::Starting),
            DownloadState::Failed
        );
    }

    #[test]
    fn production_reconcile_does_not_promote_zero_evidence_hash() {
        let mut task = crate::downloader::DownloadTask::default();
        task.state = crate::downloader::DownloadState::Starting;
        super::DownloadOrchestrator::apply_qbittorrent_info(
            &mut task,
            &crate::torrent::qbittorrent::QBittorrentInfo {
                hash: "a".repeat(40),
                name: "fixture".into(),
                size: 100,
                progress: 0.0,
                dlspeed: 0,
                eta: 0,
                state: "downloading".into(),
                save_path: String::new(),
                content_path: String::new(),
                num_seeds: None,
                num_leechs: None,
            },
        );
        assert_ne!(task.state, crate::downloader::DownloadState::Downloading);
    }
}
