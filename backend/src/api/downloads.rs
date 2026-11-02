//! Downloads API Routes
//!
//! REST endpoints for managing download tasks.

use crate::downloader::{orchestrator::BackfillItem, DownloadState, DownloadTask, EngineStats};
use crate::sources::model::ExecutionStrategy;
use crate::utils::status_utils::StatusCounts;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

fn resolve_info_path(tmdb_id: Option<i64>, media_type: Option<&str>) -> Option<String> {
    match (tmdb_id, media_type) {
        (Some(tmdb_id), Some("tv")) => Some(format!("/tv/{}", tmdb_id)),
        (Some(tmdb_id), Some("movie")) => Some(format!("/movie/{}", tmdb_id)),
        _ => None,
    }
}

fn should_hide_download_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    !lower.starts_with("magnet:")
        && (lower.starts_with("prowlarr-ref://")
            || lower.contains("apikey=")
            || lower.contains("/download?"))
}

fn redact_torrent_urls(mut download: DownloadTask) -> DownloadTask {
    if is_torrent_task(&download) {
        if should_hide_download_url(&download.url) {
            download.url = "prowlarr-ref://redacted".to_string();
        }
        if should_hide_download_url(&download.original_url) {
            download.original_url = "prowlarr-ref://redacted".to_string();
        }
    }
    if download.download_source.as_deref() == Some("torbox") {
        download.url = "torbox://redacted".to_string();
        download.original_url = "torbox://redacted".to_string();
    }
    download
}

fn enrich_download(mut download: DownloadTask) -> DownloadTask {
    download.info_path = resolve_info_path(download.tmdb_id, download.media_type.as_deref());
    redact_torrent_urls(download)
}

pub type QbitTaskMirror = HashMap<(String, String), crate::db::QbitTorrentRow>;
pub type QbitInstanceMirror = HashMap<String, crate::db::QbitInstance>;

pub fn qbit_task_mirrors(state: &AppState) -> (QbitTaskMirror, QbitInstanceMirror) {
    let rows = state
        .db
        .qbit_snapshot_full()
        .unwrap_or_default()
        .into_iter()
        .flat_map(|(_, rows)| rows)
        .map(|row| {
            (
                (row.instance_id.clone(), row.hash.to_ascii_lowercase()),
                row,
            )
        })
        .collect();
    let instances = state
        .db
        .list_qbit_instances()
        .unwrap_or_default()
        .into_iter()
        .map(|instance| (instance.id.clone(), instance))
        .collect();
    (rows, instances)
}

pub fn enrich_task_semantics(
    mut task: DownloadTask,
    rows: &QbitTaskMirror,
    instances: &QbitInstanceMirror,
) -> DownloadTask {
    if is_torrent_task(&task) {
        let instance_id = task.external_instance_id.as_deref().unwrap_or("default");
        let hash = task.info_hash.as_deref().or(task.external_id.as_deref());
        if let Some(row) =
            hash.and_then(|hash| rows.get(&(instance_id.to_string(), hash.to_ascii_lowercase())))
        {
            let meta = instances.get(instance_id);
            let health = meta
                .map(|i| i.sync_health.as_str())
                .unwrap_or("disconnected");
            let classified = crate::torrent::state::classify_qbit(
                &row.state,
                crate::torrent::state::TransferMetrics {
                    downloaded: row.downloaded.max(0) as u64,
                    previous_downloaded: None,
                    progress: row.progress,
                    dlspeed: row.dlspeed,
                    upspeed: row.upspeed,
                    amount_left: row.amount_left,
                },
                health,
            );
            task.task_state = Some(format!("{:?}", classified.task_state).to_uppercase());
            task.qbit_state = Some(row.state.clone());
            task.sync_health = Some(health.to_string());
            task.last_sync_at = meta.and_then(|i| i.last_sync_at.clone());
            task.transfer_state = Some(classified.transfer_state);
            task.transfer_active = classified.transfer_active;
        }
    }
    enrich_download(task)
}

pub fn project_managed_torrents(
    tasks: impl IntoIterator<Item = DownloadTask>,
    rows: impl IntoIterator<Item = crate::db::QbitTorrentRow>,
    instances: &QbitInstanceMirror,
) -> Vec<UnifiedTorrentRow> {
    let managed = tasks
        .into_iter()
        .filter(|task| is_torrent_task(task))
        .filter_map(|task| {
            let instance = task
                .external_instance_id
                .clone()
                .unwrap_or_else(|| "default".into());
            let hash = task
                .info_hash
                .clone()
                .or_else(|| task.external_id.clone())?;
            Some(((instance, hash.to_ascii_lowercase()), task))
        })
        .collect::<HashMap<_, _>>();
    rows.into_iter()
        .filter_map(|row| {
            let task = managed
                .get(&(row.instance_id.clone(), row.hash.to_ascii_lowercase()))?
                .clone();
            let meta = instances.get(&row.instance_id);
            Some(UnifiedTorrentRow::project_managed(
                task,
                row,
                meta.map(|item| item.sync_health.as_str())
                    .unwrap_or("disconnected"),
                meta.and_then(|item| item.last_sync_at.clone()),
            ))
        })
        .collect()
}

fn is_torrent_task(task: &DownloadTask) -> bool {
    task.download_source.as_deref() == Some("torrent")
}

fn is_torbox_task(task: &DownloadTask) -> bool {
    task.download_source.as_deref() == Some("torbox")
        || task.external_provider.as_deref() == Some("torbox")
}

async fn refresh_torbox_link(state: &Arc<AppState>, task: &mut DownloadTask) -> anyhow::Result<()> {
    let remote_id = task
        .external_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("TorBox task is missing its remote ID"))?;
    let link = crate::sources::torbox::refresh_link(state, remote_id).await?;
    task.url = link.clone();
    task.original_url = link;
    Ok(())
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_downloads))
        .route("/", post(add_download))
        .route("/:id", get(get_download))
        .route("/:id/status", get(get_download_status))
        .route("/:id", delete(delete_download))
        .route("/:id/pause", post(pause_download))
        .route("/:id/resume", post(resume_download))
        .route("/:id/retry", post(retry_download))
        .route("/:id/redownload", post(redownload_download))
        .route("/pause-all", post(pause_all))
        .route("/resume-all", post(resume_all))
        .route("/stats", get(get_stats))
        // Batch actions
        .route("/batch/:batch_id/pause", post(pause_batch))
        .route("/batch/:batch_id/resume", post(resume_batch))
        .route("/batch/:batch_id", delete(delete_batch))
        .route("/batch/:batch_id/redownload", post(redownload_batch))
        .route("/batch/:batch_id/progress", get(get_batch_progress))
        .route("/batch/:batch_id/items", get(get_batch_items))
        // Batch summaries for pagination
        .route("/batches", get(list_batch_summaries))
        // Maintenance
        .route("/backfill", post(backfill_arr_paths))
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Serialize)]
struct DownloadsResponse {
    downloads: Vec<DownloadTask>,
    stats: EngineStats,
    /// Per-status counts from database (for filter dropdown)
    status_counts: StatusCounts,
    /// Pagination info
    total: u64,
    page: u32,
    limit: u32,
    total_pages: u32,
}

// StatusCounts imported from crate::utils::status_utils

/// Query parameters for list downloads
#[derive(Debug, Deserialize)]
struct ListDownloadsQuery {
    /// Page number (1-indexed, default 1)
    page: Option<u32>,
    /// Items per page (default 20, max 100)
    limit: Option<u32>,
    /// Sort column: "added" (default), "status", "filename", "size", "progress"
    sort_by: Option<String>,
    /// Sort direction: "asc" or "desc" (default)
    sort_dir: Option<String>,
    /// Filter by status (e.g., "DOWNLOADING", "QUEUED", "COMPLETED", "FAILED")
    status: Option<String>,
    /// Filter by source: "fshare", "torrent", "torbox", or "direct"
    source: Option<String>,
}

#[derive(Serialize)]
struct ActionResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Serialize)]
struct BulkActionResponse {
    success: bool,
    affected: usize,
}

#[derive(Serialize)]
struct DownloadStatusResponse {
    id: String,
    state: DownloadState,
    progress: f32,
    size: u64,
    downloaded: u64,
    speed: f64,
    eta: f64,
    source: Option<String>,
    provider: Option<String>,
    external_id: Option<String>,
    seeders: Option<i64>,
    leechers: Option<i64>,
    error_message: Option<String>,
}

/// Batch summary for batch-first pagination
/// Represents a batch as a single row in the downloads table
#[derive(Serialize, Clone)]
pub struct BatchSummary {
    pub batch_id: String,
    pub batch_name: String,
    pub total_items: usize,
    pub completed_items: usize,
    pub failed_items: usize,
    pub downloading_items: usize,
    pub paused_items: usize,
    pub queued_items: usize,
    pub total_size: u64,
    pub downloaded_size: u64,
    pub progress: f32,
    pub speed: f64,
    pub created_at: String,
    pub source: String,
    pub state: String, // Aggregate state: DOWNLOADING, PAUSED, COMPLETED, FAILED, QUEUED
}

#[derive(Serialize, Clone)]
pub struct DisplayRowRef {
    pub kind: String,
    pub id: String,
}

/// Unified torrent row — qB torrents not tracked by a Flasharr download task.
/// Safe subset of QbitTorrentRow (raw_json excluded).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UnifiedTorrentRow {
    pub instance_id: String,
    pub hash: String,
    pub name: String,
    /// Raw qB state string (e.g. "downloading", "pausedUP")
    pub state: String,
    /// Mapped Flasharr state (DOWNLOADING, PAUSED, COMPLETED, QUEUED, FAILED)
    pub flasharr_state: String,
    pub task_state: String,
    pub qbit_state: String,
    pub sync_health: String,
    pub last_sync_at: Option<String>,
    pub transfer_state: String,
    pub transfer_active: bool,
    /// 0.0–1.0
    pub progress: f64,
    pub size: i64,
    pub downloaded: i64,
    pub uploaded: i64,
    pub dlspeed: i64,
    pub upspeed: i64,
    pub eta: i64,
    pub ratio: f64,
    pub category: String,
    pub tags: String,
    pub added_on: i64,
    pub completion_on: i64,
    pub amount_left: i64,
    pub availability: f64,
    pub num_seeds: i64,
    pub num_leechs: i64,
    pub seen_complete: i64,
    pub last_activity: i64,
    pub updated_at: String,
    pub media_key: Option<String>,
    pub media_type: Option<String>,
    pub media_title: Option<String>,
    pub tvdb_id: Option<i64>,
    pub tmdb_id: Option<i64>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub source_filename: Option<String>,
    pub payload: Vec<serde_json::Value>,
    pub import_state: TorrentImportState,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TorrentImportState {
    pub arr_announced: bool,
    pub arr_announce_error: Option<String>,
    pub arr_series_id: Option<i64>,
    pub arr_movie_id: Option<i64>,
}

#[cfg(test)]
mod managed_torrent_contract_tests {
    use super::*;

    fn qbit_row() -> crate::db::QbitTorrentRow {
        crate::db::QbitTorrentRow {
            instance_id: "qbit".into(),
            hash: "abc".into(),
            name: "Immutable.Release.Name".into(),
            state: "downloading".into(),
            progress: 0.5,
            size: 100,
            downloaded: 50,
            uploaded: 0,
            dlspeed: 10,
            upspeed: 1,
            eta: 5,
            ratio: 0.1,
            category: "media".into(),
            tags: "".into(),
            save_path: "/private".into(),
            content_path: "/private/file".into(),
            tracker: "https://secret".into(),
            added_on: 1,
            completion_on: 0,
            amount_left: 50,
            availability: 1.0,
            num_seeds: 2,
            num_leechs: 1,
            seen_complete: 0,
            last_activity: 1,
            raw_json: "{}".into(),
            updated_at: "2026-07-17T00:00:00Z".into(),
        }
    }

    #[test]
    fn managed_projection_exposes_flasharr_identity_and_import_state_without_private_qbit_fields() {
        let mut task = DownloadTask::default();
        task.download_source = Some("torrent".into());
        task.info_hash = Some("abc".into());
        task.external_instance_id = Some("qbit".into());
        task.media_type = Some("tv".into());
        task.tvdb_id = Some(42);
        task.tmdb_id = Some(84);
        task.tmdb_title = Some("Shōgun".into());
        task.tmdb_season = Some(1);
        task.tmdb_episode = Some(7);
        task.source_filename = Some("Shogun.S01E07.mkv".into());
        task.selected_episodes =
            Some("[{\"season\":1,\"episode\":7,\"title\":\"A Stick of Time\"}]".into());
        task.arr_announced = true;
        let value = serde_json::to_value(UnifiedTorrentRow::project_managed(
            task,
            qbit_row(),
            "healthy",
            Some("2026-07-17T00:00:00Z".into()),
        ))
        .unwrap();
        assert_eq!(value["media_key"], "tv:42");
        assert_eq!(value["media_type"], "tv");
        assert_eq!(value["media_title"], "Shōgun");
        assert_eq!(value["payload"][0]["title"], "A Stick of Time");
        assert_eq!(value["import_state"]["arr_announced"], true);
        for forbidden in ["save_path", "content_path", "tracker", "raw_json"] {
            assert!(value.get(forbidden).is_none(), "leaked {forbidden}");
        }
    }

    #[test]
    fn managed_projection_omits_external_qbit_rows() {
        let mut task = DownloadTask::default();
        task.download_source = Some("torrent".into());
        task.info_hash = Some("abc".into());
        task.external_instance_id = Some("qbit".into());
        let mut external = qbit_row();
        external.hash = "external".into();
        let rows = project_managed_torrents([task], [qbit_row(), external], &HashMap::new());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].hash, "abc");
    }
}

impl UnifiedTorrentRow {
    pub fn project_managed(
        task: DownloadTask,
        row: crate::db::QbitTorrentRow,
        sync_health: &str,
        last_sync_at: Option<String>,
    ) -> Self {
        let media_key = task.canonical_media_leaf_key().map(|key| {
            if key.starts_with("episode:") {
                format!("tv:{}", task.tvdb_id.unwrap_or_default())
            } else {
                key
            }
        });
        let payload = task
            .selected_episodes
            .as_deref()
            .and_then(|json| serde_json::from_str::<Vec<serde_json::Value>>(json).ok())
            .unwrap_or_default();
        let import_state = TorrentImportState {
            arr_announced: task.arr_announced,
            arr_announce_error: task.arr_announce_error.clone(),
            arr_series_id: task.arr_series_id,
            arr_movie_id: task.arr_movie_id,
        };
        let mut projected = Self::project(row, sync_health, last_sync_at);
        projected.media_key = media_key;
        projected.media_type = task.media_type;
        projected.media_title = task.tmdb_title;
        projected.tvdb_id = task.tvdb_id;
        projected.tmdb_id = task.tmdb_id;
        projected.season = task.tmdb_season;
        projected.episode = task.tmdb_episode;
        projected.source_filename = task.source_filename.or(Some(task.filename));
        projected.payload = payload;
        projected.import_state = import_state;
        projected
    }

    pub fn project(
        row: crate::db::QbitTorrentRow,
        sync_health: &str,
        last_sync_at: Option<String>,
    ) -> Self {
        let classification = crate::torrent::state::classify_qbit(
            &row.state,
            crate::torrent::state::TransferMetrics {
                downloaded: row.downloaded.max(0) as u64,
                previous_downloaded: None,
                progress: row.progress,
                dlspeed: row.dlspeed,
                upspeed: row.upspeed,
                amount_left: row.amount_left,
            },
            sync_health,
        );
        let flasharr_state = format!("{:?}", classification.task_state).to_uppercase();
        Self {
            instance_id: row.instance_id,
            hash: row.hash,
            name: row.name,
            state: row.state.clone(),
            task_state: flasharr_state.clone(),
            qbit_state: row.state,
            sync_health: sync_health.into(),
            last_sync_at,
            transfer_state: classification.transfer_state,
            transfer_active: classification.transfer_active,
            flasharr_state,
            progress: row.progress,
            size: row.size,
            downloaded: row.downloaded,
            uploaded: row.uploaded,
            dlspeed: row.dlspeed,
            upspeed: row.upspeed,
            eta: row.eta,
            ratio: row.ratio,
            category: row.category,
            tags: row.tags,
            added_on: row.added_on,
            completion_on: row.completion_on,
            amount_left: row.amount_left,
            availability: row.availability,
            num_seeds: row.num_seeds,
            num_leechs: row.num_leechs,
            seen_complete: row.seen_complete,
            last_activity: row.last_activity,
            updated_at: row.updated_at,
            media_key: None,
            media_type: None,
            media_title: None,
            tvdb_id: None,
            tmdb_id: None,
            season: None,
            episode: None,
            source_filename: None,
            payload: vec![],
            import_state: TorrentImportState::default(),
        }
    }
}

/// Response for /api/downloads/batches endpoint
#[derive(Serialize)]
struct BatchSummariesResponse {
    batches: Vec<BatchSummary>,
    standalone: Vec<DownloadTask>,
    rows: Vec<DisplayRowRef>,
    /// qB torrents not tracked as Flasharr download tasks (externally-added)
    torrents: Vec<UnifiedTorrentRow>,
    stats: EngineStats,
    status_counts: StatusCounts,
    total: u64, // Total display units (batches + standalone)
    page: u32,
    limit: u32,
    total_pages: u32,
}

// ============================================================================
// Request Types
// ============================================================================

/// TMDB Metadata for organized folder structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TmdbMetadata {
    /// TMDB ID. Movie: canonical. TV: legacy bridge only.
    #[serde(default)]
    pub tmdb_id: Option<i64>,
    /// TVDB ID. TV: canonical identity for Sonarr.
    #[serde(default, alias = "tvdbId")]
    pub tvdb_id: Option<i64>,
    /// Media type: "movie" or "tv"
    #[serde(default)]
    pub media_type: Option<String>,
    /// Movie/Show title
    #[serde(default)]
    pub title: Option<String>,
    /// Release year — optional; if omitted from JSON body, defaults to None
    #[serde(default, deserialize_with = "deserialize_year")]
    pub year: Option<i32>,
    /// Collection name (for movies in a collection)
    #[serde(default)]
    pub collection_name: Option<String>,
    /// Season number (for TV)
    #[serde(default)]
    pub season: Option<i32>,
    /// Episode number (for TV)
    #[serde(default)]
    pub episode: Option<i32>,
}

/// Custom deserializer for year field - accepts both string and integer
fn deserialize_year<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Deserialize};

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum YearValue {
        Int(i32),
        String(String),
    }

    match Option::<YearValue>::deserialize(deserializer)? {
        None => Ok(None),
        Some(YearValue::Int(i)) => Ok(Some(i)),
        Some(YearValue::String(s)) => s
            .parse::<i32>()
            .map(Some)
            .map_err(|_| de::Error::custom(format!("invalid year string: {}", s))),
    }
}

#[derive(Deserialize)]
pub struct AddDownloadRequest {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub download_ref: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub priority: Option<String>,
    /// TMDB metadata for folder organization
    #[serde(default)]
    pub tmdb: Option<TmdbMetadata>,
    /// Batch ID for grouping related downloads (e.g., TV season)
    #[serde(default)]
    pub batch_id: Option<String>,
    /// Batch display name (e.g., "Breaking Bad S01")
    #[serde(default)]
    pub batch_name: Option<String>,
    /// Source/provider hints for non-Fshare downloads.
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub seeders: Option<i64>,
    #[serde(default)]
    pub leechers: Option<i64>,
    #[serde(default)]
    pub info_hash: Option<String>,
    #[serde(default)]
    pub file_idx: Option<usize>,
    #[serde(default)]
    pub source_filename: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default)]
    pub origin_provider: Option<String>,
    #[serde(default)]
    pub origin_indexer: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub qbit_instance_id: Option<String>,
    /// Re-download an existing completed task with the same Fshare code.
    #[serde(default)]
    pub force_redownload: bool,
    #[serde(default)]
    pub strategy: Option<ExecutionStrategy>,
    #[serde(default)]
    pub selected_episodes: Option<String>,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/downloads - List downloads with pagination
///
/// Query params:
/// - page: Page number (1-indexed, default 1)
/// - limit: Items per page (default 20, max 100)
/// - sort_by: Column to sort by ("added", "status", "filename", "size", "progress")
/// - sort_dir: Sort direction ("asc" or "desc")
/// - status: Filter by status (e.g., "DOWNLOADING", "QUEUED", "COMPLETED", "FAILED")
async fn list_downloads(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListDownloadsQuery>,
) -> Json<DownloadsResponse> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).min(100).max(1);
    let sort_by = params.sort_by.unwrap_or_else(|| "added".to_string());
    let sort_dir = params.sort_dir.unwrap_or_else(|| "desc".to_string());
    let status_filter = params.status; // None means "all"
    let source_filter = params.source;

    // Query from database with pagination, sorting, and optional filters (async to prevent blocking)
    let (mut downloads, total) = state
        .db
        .get_tasks_paginated_sorted_filtered_async(
            page,
            limit,
            &sort_by,
            &sort_dir,
            status_filter.as_deref(),
            source_filter.as_deref(),
        )
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to get tasks from DB: {}", e);
            (Vec::new(), 0)
        });

    // Get status counts from database for filter dropdown
    let db_status_counts = state
        .db
        .get_status_counts_filtered_async(source_filter.as_deref())
        .await
        .unwrap_or_default();
    let status_counts = StatusCounts::from_db_counts(db_status_counts);

    // Merge real-time progress from active downloads in memory
    let active_tasks = state
        .download_orchestrator
        .task_manager()
        .get_active_tasks()
        .await;
    for download in downloads.iter_mut() {
        if let Some(active) = active_tasks.iter().find(|t| t.id == download.id) {
            download.progress = active.progress;
            download.speed = active.speed;
            download.eta = active.eta;
            download.downloaded = active.downloaded;
            download.state = active.state;
        }
    }
    let (qbit_rows, qbit_instances) = qbit_task_mirrors(&state);
    let downloads = downloads
        .into_iter()
        .map(|task| enrich_task_semantics(task, &qbit_rows, &qbit_instances))
        .collect();

    let stats = state.download_orchestrator.task_manager().get_stats().await;
    let total_pages = ((total as f64) / (limit as f64)).ceil() as u32;

    Json(DownloadsResponse {
        downloads,
        stats,
        status_counts,
        total,
        page,
        limit,
        total_pages,
    })
}

/// GET /api/downloads/:id - Get single download
async fn get_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<DownloadTask>, StatusCode> {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let mut task = state
        .download_orchestrator
        .get_task_unified(uuid)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    if is_torbox_task(&task) {
        if let Err(error) = refresh_torbox_link(&state, &mut task).await {
            tracing::warn!("Failed to refresh TorBox link for {}: {}", uuid, error);
        }
    }

    if is_torrent_task(&task) {
        match state
            .download_orchestrator
            .refresh_external_torrent_status(task.clone())
            .await
        {
            Ok(refreshed) => return Ok(Json(enrich_download(refreshed))),
            Err(e) => tracing::warn!("Failed to refresh qBittorrent status for {}: {}", uuid, e),
        }
    }

    Ok(Json(enrich_download(task)))
}

/// GET /api/downloads/:id/status - Get live task status
async fn get_download_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<DownloadStatusResponse>, StatusCode> {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let mut task = state
        .download_orchestrator
        .get_task_unified(uuid)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    if is_torrent_task(&task) {
        match state
            .download_orchestrator
            .refresh_external_torrent_status(task.clone())
            .await
        {
            Ok(refreshed) => task = refreshed,
            Err(e) => tracing::warn!("Failed to refresh qBittorrent status for {}: {}", uuid, e),
        }
    }

    Ok(Json(DownloadStatusResponse {
        id: task.id.to_string(),
        state: task.state,
        progress: task.progress,
        size: task.size,
        downloaded: task.downloaded,
        speed: task.speed,
        eta: task.eta,
        source: task.download_source,
        provider: task.external_provider,
        external_id: task.external_id,
        seeders: task.seeders,
        leechers: task.leechers,
        error_message: task.error_message,
    }))
}

/// POST /api/downloads - Add new download
async fn add_download(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AddDownloadRequest>,
) -> Result<Json<DownloadTask>, axum::response::Response> {
    tracing::info!("[API] add_download called");
    tracing::info!("[API] Incoming URL present: {}", payload.url.is_some());
    tracing::info!(
        "[API] Incoming download_ref present: {}",
        payload.download_ref.is_some()
    );
    tracing::info!("[API] Incoming filename: {:?}", payload.filename);
    tracing::info!("[API] Incoming category: {:?}", payload.category);
    tracing::info!(
        "[API] Incoming info_hash present: {}",
        payload.info_hash.is_some()
    );
    if let Some(ref tmdb) = payload.tmdb {
        tracing::info!(
            "[API] media metadata: tmdb_id={:?}, tvdb_id={:?}, media_type={:?}, title={:?}, year={:?}, season={:?}, episode={:?}",
            tmdb.tmdb_id,
            tmdb.tvdb_id,
            tmdb.media_type,
            tmdb.title,
            tmdb.year,
            tmdb.season,
            tmdb.episode
        );
    } else {
        tracing::info!("[API] No TMDB metadata provided");
    }

    // Category from payload or derive from TMDB metadata
    // CRITICAL: Trust TMDB media_type over UI's provided category if they mismatch
    let category = if let Some(ref tmdb) = payload.tmdb {
        match tmdb.media_type.as_deref() {
            Some("movie") => "movies".to_string(), // Force to movies if TMDB says so
            Some("tv") => "tv".to_string(),        // Force to tv if TMDB says so
            _ => payload.category.unwrap_or_else(|| "other".to_string()),
        }
    } else {
        payload.category.unwrap_or_else(|| "other".to_string())
    };

    // Normalize filename (strip leading slash) at the API entry point
    let final_filename_api = payload.filename.as_ref().map(|f| {
        if f.starts_with('/') {
            f.trim_start_matches('/').to_string()
        } else {
            f.clone()
        }
    });

    // Convert TMDB metadata to orchestrator format
    let tmdb_metadata = payload
        .tmdb
        .map(|t| crate::downloader::TmdbDownloadMetadata {
            tmdb_id: t.tmdb_id,
            tvdb_id: t.tvdb_id,
            media_type: t.media_type,
            title: t.title,
            year: t.year,
            collection_name: t.collection_name,
            season: t.season,
            episode: t.episode,
        });

    // TV grouping is canonicalized once in the orchestrator. Non-TV callers retain
    // their explicit grouping fields.
    let (batch_id, batch_name) = (payload.batch_id.clone(), payload.batch_name.clone());

    if let Some(instance_id) = payload.qbit_instance_id.as_deref() {
        let instance = state
            .db
            .get_qbit_instance(instance_id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
            .filter(|instance| instance.enabled)
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "Selected qBittorrent instance is unavailable",
                )
                    .into_response()
            })?;
        if instance.id.trim().is_empty() {
            return Err((StatusCode::BAD_REQUEST, "Invalid qBittorrent instance").into_response());
        }
    }

    let materialized = crate::sources::materialize_download(
        &state,
        &state.torrent_download_refs,
        crate::sources::model::DownloadSelection {
            url: payload.url.clone(),
            download_ref: payload.download_ref.clone(),
            filename: payload.filename.clone(),
            source: payload.source.clone(),
            provider: payload.provider.clone(),
            info_hash: payload.info_hash.clone(),
            size: payload.size,
            external_id: payload.external_id.clone(),
            file_idx: payload.file_idx,
            source_filename: payload.source_filename.clone(),
            selected_episodes: payload.selected_episodes.clone(),
            strategy: payload.strategy.unwrap_or_else(|| {
                if payload.source.as_deref() == Some("torbox")
                    || payload.provider.as_deref() == Some("torbox")
                {
                    ExecutionStrategy::TorboxHttp
                } else {
                    ExecutionStrategy::LocalQb
                }
            }),
        },
    )
    .await
    .map_err(|(status, error)| (status, Json(error)).into_response())?;

    let task = match state
        .download_orchestrator
        .add_download_with_metadata(
            materialized.queue_url,
            final_filename_api,
            materialized.host,
            category,
            tmdb_metadata,
            batch_id,
            batch_name,
            payload.force_redownload,
            Some(materialized.source.as_str().to_string()),
            Some(materialized.provider),
            materialized.size,
            payload.seeders,
            payload.leechers,
            materialized.info_hash,
            materialized.external_id,
            payload.qbit_instance_id,
            payload.origin_provider,
            payload.origin_indexer,
            payload.file_idx,
            payload.source_filename,
        )
        .await
    {
        Ok(mut task) => {
            task.execution_strategy = Some(
                match materialized.execution_strategy {
                    ExecutionStrategy::LocalQb => "local_qb",
                    ExecutionStrategy::TorboxHttp => "torbox_http",
                }
                .to_string(),
            );
            task.selected_episodes = payload.selected_episodes;
            task.torbox_remote_state = materialized.remote_state;
            task.torbox_remote_completed_at = materialized.remote_completed_at;
            task.selected_remote_file_ids = materialized.selected_remote_file_ids;
            task.selected_artifacts = materialized.selected_artifacts;
            state
                .db
                .save_task(&task)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
            state
                .download_orchestrator
                .task_manager()
                .update_task(task.clone())
                .await;
            enrich_download(task)
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            if error_msg.contains("already exists") {
                tracing::info!("Duplicate download skipped: {}", error_msg);
                return Err((StatusCode::CONFLICT, error_msg).into_response());
            }
            tracing::error!("Failed to add download: {}", e);
            return Err(
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed: {}", e)).into_response(),
            );
        }
    };

    // Return full task object so frontend gets batch_id, batch_name, etc.
    Ok(Json(task))
}

/// DELETE /api/downloads/:id - Delete download
async fn delete_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse>, StatusCode> {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;

    let task_before_delete = state.download_orchestrator.get_task_unified(uuid).await;
    if let Some(task) = &task_before_delete {
        if !is_torbox_task(task) {
            if let Err(error) = state
                .download_orchestrator
                .delete_external_torrent(task, false)
                .await
            {
                tracing::warn!(
                    "Failed to delete external torrent for task {}: {}",
                    uuid,
                    error
                );
                return Err(StatusCode::BAD_GATEWAY);
            }
        }
    }

    let in_memory = state
        .download_orchestrator
        .task_manager()
        .delete_task(uuid)
        .await;

    let from_db = match state.db.delete_task(uuid) {
        Ok(_) => {
            tracing::info!("Deleted task {} from database", uuid);
            true
        }
        Err(e) => {
            tracing::warn!("Failed to delete task {} from database: {}", uuid, e);
            false
        }
    };

    let success = in_memory || from_db;
    if success {
        state.download_orchestrator.broadcast_task_removed(&id);
    }

    Ok(Json(ActionResponse {
        success,
        message: if success {
            None
        } else {
            Some("Task not found".to_string())
        },
    }))
}

/// POST /api/downloads/:id/pause - Pause download
async fn pause_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse>, StatusCode> {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let Some(mut task) = state.download_orchestrator.get_task_unified(uuid).await else {
        return Ok(Json(ActionResponse {
            success: false,
            message: Some("Task not found".to_string()),
        }));
    };

    if !task.state.can_pause() {
        return Ok(Json(ActionResponse {
            success: false,
            message: Some("Task cannot be paused".to_string()),
        }));
    }

    if is_torrent_task(&task) {
        if let Err(e) = state
            .download_orchestrator
            .pause_external_torrent(&task)
            .await
        {
            tracing::warn!("Failed to pause external torrent for task {}: {}", uuid, e);
            return Ok(Json(ActionResponse {
                success: false,
                message: Some(format!("qBittorrent pause failed: {}", e)),
            }));
        }
    }

    task.cancel_token.cancel();
    task.state = DownloadState::Paused;
    state
        .download_orchestrator
        .task_manager()
        .update_task(task.clone())
        .await;
    let _ = state.db.save_task(&task);
    state.download_orchestrator.broadcast_task_update(&task);

    Ok(Json(ActionResponse {
        success: true,
        message: None,
    }))
}

/// POST /api/downloads/:id/resume - Resume paused download
async fn resume_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse>, StatusCode> {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let Some(mut task) = state.download_orchestrator.get_task_unified(uuid).await else {
        return Ok(Json(ActionResponse {
            success: false,
            message: Some("Task not found".to_string()),
        }));
    };

    if !task.state.can_resume() && task.state != DownloadState::Queued {
        return Ok(Json(ActionResponse {
            success: false,
            message: Some("Task cannot be resumed".to_string()),
        }));
    }

    task.cancel_token = tokio_util::sync::CancellationToken::new();
    task.retry_count = 0;
    task.wait_until = None;
    task.error_message = None;

    if is_torrent_task(&task) && task.external_id.is_some() {
        if let Err(e) = state
            .download_orchestrator
            .resume_external_torrent(&task)
            .await
        {
            tracing::warn!("Failed to resume external torrent for task {}: {}", uuid, e);
            return Ok(Json(ActionResponse {
                success: false,
                message: Some(format!("qBittorrent resume failed: {}", e)),
            }));
        }
        task.state = DownloadState::Downloading;
    } else {
        task.state = DownloadState::Queued;
        state.download_orchestrator.wake_workers();
    }

    state
        .download_orchestrator
        .task_manager()
        .update_task(task.clone())
        .await;
    let _ = state.db.save_task(&task);
    state.download_orchestrator.broadcast_task_update(&task);

    Ok(Json(ActionResponse {
        success: true,
        message: None,
    }))
}

/// POST /api/downloads/:id/retry - Retry failed download
async fn retry_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse>, StatusCode> {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // For retry, we handle failed/cancelled tasks
    let task_result = state
        .download_orchestrator
        .task_manager()
        .retry_task(uuid)
        .await;
    let success = task_result.is_some();

    // Wake idle workers to process the retried task
    if success {
        state.download_orchestrator.wake_workers();
    }

    Ok(Json(ActionResponse {
        success,
        message: if !success {
            Some("Task not found or cannot be retried".to_string())
        } else {
            None
        },
    }))
}

/// POST /api/downloads/:id/redownload - Re-download task using original data
async fn redownload_download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse>, StatusCode> {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;

    match state.download_orchestrator.redownload_task(uuid).await {
        Ok(_) => Ok(Json(ActionResponse {
            success: true,
            message: None,
        })),
        Err(e) => {
            tracing::error!("Failed to re-download task {}: {}", uuid, e);
            Ok(Json(ActionResponse {
                success: false,
                message: Some(format!("Failed to re-download: {}", e)),
            }))
        }
    }
}

/// POST /api/downloads/pause-all - Pause all active downloads
/// Delegates to Orchestrator which handles: DB (atomic) → TaskManager → Broadcast
async fn pause_all(State(state): State<Arc<AppState>>) -> Json<BulkActionResponse> {
    let affected = state.download_orchestrator.pause_all_async().await;

    Json(BulkActionResponse {
        success: true,
        affected,
    })
}

/// POST /api/downloads/resume-all - Resume all paused downloads
/// Delegates to Orchestrator which handles: DB (atomic) → TaskManager → Broadcast → Wake
async fn resume_all(State(state): State<Arc<AppState>>) -> Json<BulkActionResponse> {
    let affected = state.download_orchestrator.resume_all_async().await;

    Json(BulkActionResponse {
        success: true,
        affected,
    })
}

/// GET /api/downloads/stats - Get engine statistics
async fn get_stats(State(state): State<Arc<AppState>>) -> Json<EngineStats> {
    Json(state.download_orchestrator.task_manager().get_stats().await)
}

// ============================================================================
// Batch Action Handlers
// ============================================================================

/// POST /api/downloads/batch/:batch_id/pause - Pause all downloads in a batch
async fn pause_batch(
    State(state): State<Arc<AppState>>,
    Path(batch_id): Path<String>,
) -> Json<BulkActionResponse> {
    match state.download_service.pause_batch(&batch_id).await {
        Ok(affected) => Json(BulkActionResponse {
            success: true,
            affected,
        }),
        Err(e) => {
            tracing::error!("Failed to pause batch {}: {}", batch_id, e);
            Json(BulkActionResponse {
                success: false,
                affected: 0,
            })
        }
    }
}

/// POST /api/downloads/batch/:batch_id/resume - Resume all downloads in a batch
async fn resume_batch(
    State(state): State<Arc<AppState>>,
    Path(batch_id): Path<String>,
) -> Json<BulkActionResponse> {
    match state.download_service.resume_batch(&batch_id).await {
        Ok(affected) => Json(BulkActionResponse {
            success: true,
            affected,
        }),
        Err(e) => {
            tracing::error!("Failed to resume batch {}: {}", batch_id, e);
            Json(BulkActionResponse {
                success: false,
                affected: 0,
            })
        }
    }
}

/// DELETE /api/downloads/batch/:batch_id - Delete all downloads in a batch
async fn delete_batch(
    State(state): State<Arc<AppState>>,
    Path(batch_id): Path<String>,
) -> Json<BulkActionResponse> {
    match state.download_service.delete_batch(&batch_id).await {
        Ok(affected) => Json(BulkActionResponse {
            success: true,
            affected,
        }),
        Err(e) => {
            tracing::error!("Failed to delete batch {}: {}", batch_id, e);
            Json(BulkActionResponse {
                success: false,
                affected: 0,
            })
        }
    }
}

/// POST /api/downloads/batch/:batch_id/redownload - Re-download all tasks in a batch
async fn redownload_batch(
    State(state): State<Arc<AppState>>,
    Path(batch_id): Path<String>,
) -> Json<BulkActionResponse> {
    match state
        .download_orchestrator
        .redownload_batch_async(batch_id.clone())
        .await
    {
        Ok(affected) => {
            tracing::info!("Re-downloaded batch {}: {} tasks", batch_id, affected);
            Json(BulkActionResponse {
                success: true,
                affected,
            })
        }
        Err(e) => {
            tracing::error!("Failed to re-download batch {}: {}", batch_id, e);
            Json(BulkActionResponse {
                success: false,
                affected: 0,
            })
        }
    }
}

// ============================================================================
// Batch Progress
// ============================================================================

#[derive(Serialize)]
pub struct BatchProgress {
    pub batch_id: String,
    pub batch_name: String,
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub downloading_tasks: usize,
    pub paused_tasks: usize,
    pub queued_tasks: usize,
    pub overall_progress: f32,
    pub total_size: u64,
    pub downloaded_size: u64,
    pub combined_speed: f64,
    pub estimated_time_remaining: f64,
}

/// GET /api/downloads/batch/:batch_id/progress - Get batch progress aggregation
async fn get_batch_progress(
    State(state): State<Arc<AppState>>,
    Path(batch_id): Path<String>,
) -> Result<Json<BatchProgress>, StatusCode> {
    let all_tasks = state.download_orchestrator.task_manager().get_tasks().await;

    // Filter tasks by batch_id
    let batch_tasks: Vec<_> = all_tasks
        .iter()
        .filter(|t| t.batch_id.as_deref() == Some(&batch_id))
        .collect();

    if batch_tasks.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    let batch_name = batch_tasks[0].batch_name.clone().unwrap_or_default();
    let total_tasks = batch_tasks.len();

    // Count by state
    let completed_tasks = batch_tasks
        .iter()
        .filter(|t| matches!(t.state, DownloadState::Completed))
        .count();
    let failed_tasks = batch_tasks
        .iter()
        .filter(|t| matches!(t.state, DownloadState::Failed))
        .count();
    let downloading_tasks = batch_tasks
        .iter()
        .filter(|t| matches!(t.state, DownloadState::Downloading))
        .count();
    let paused_tasks = batch_tasks
        .iter()
        .filter(|t| matches!(t.state, DownloadState::Paused))
        .count();
    let queued_tasks = batch_tasks
        .iter()
        .filter(|t| matches!(t.state, DownloadState::Queued))
        .count();

    // Calculate totals
    let total_size: u64 = batch_tasks.iter().map(|t| t.size).sum();
    let downloaded_size: u64 = batch_tasks
        .iter()
        .map(|t| {
            if matches!(t.state, DownloadState::Completed) {
                t.size
            } else if t.downloaded > 0 {
                t.downloaded.min(t.size)
            } else {
                ((t.progress as f64 / 100.0) * t.size as f64) as u64
            }
        })
        .sum();

    // Calculate overall progress
    let overall_progress = if total_size > 0 {
        (downloaded_size as f64 / total_size as f64 * 100.0) as f32
    } else {
        0.0
    };

    // Calculate combined speed (only downloading tasks)
    let combined_speed: f64 = batch_tasks
        .iter()
        .filter(|t| matches!(t.state, DownloadState::Downloading))
        .map(|t| t.speed)
        .sum();

    // Calculate ETA
    let remaining_bytes = total_size.saturating_sub(downloaded_size);
    let estimated_time_remaining = if combined_speed > 0.0 {
        remaining_bytes as f64 / combined_speed
    } else {
        0.0
    };

    Ok(Json(BatchProgress {
        batch_id,
        batch_name,
        total_tasks,
        completed_tasks,
        failed_tasks,
        downloading_tasks,
        paused_tasks,
        queued_tasks,
        overall_progress,
        total_size,
        downloaded_size,
        combined_speed,
        estimated_time_remaining,
    }))
}

// ============================================================================
// Batch Summaries for Pagination
// ============================================================================

/// GET /api/downloads/batches - List batch summaries and standalone downloads
/// This endpoint supports batch-first pagination where batches are treated as single rows
async fn list_batch_summaries(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListDownloadsQuery>,
) -> Json<BatchSummariesResponse> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).min(100).max(1);
    let sort_by = params.sort_by.unwrap_or_else(|| "added".to_string());
    let sort_dir = params.sort_dir.unwrap_or_else(|| "desc".to_string());
    let status_filter = params.status;

    // Get batch summaries from database
    let (batches, standalone, rows, untracked_torrents, total) = state
        .db
        .get_batch_summaries_paginated_async(
            page,
            limit,
            status_filter.as_deref(),
            params.source.as_deref(),
            &sort_by,
            &sort_dir,
        )
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to get batch summaries from DB: {}", e);
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), 0)
        });

    // Merge real-time progress from active downloads for standalone items
    let active_tasks = state
        .download_orchestrator
        .task_manager()
        .get_active_tasks()
        .await;
    let mut standalone_with_realtime = standalone;
    for download in standalone_with_realtime.iter_mut() {
        if let Some(active) = active_tasks.iter().find(|t| t.id == download.id) {
            download.progress = active.progress;
            download.speed = active.speed;
            download.eta = active.eta;
            download.downloaded = active.downloaded;
            download.state = active.state;
        }
    }
    let (qbit_rows, qbit_instances) = qbit_task_mirrors(&state);
    let standalone_with_realtime = standalone_with_realtime
        .into_iter()
        .map(|task| enrich_task_semantics(task, &qbit_rows, &qbit_instances))
        .collect();

    // Update batch speeds and real-time progress from active tasks
    let mut batches_with_realtime = batches;
    for batch in batches_with_realtime.iter_mut() {
        // Find all active tasks belonging to this batch
        let batch_active: Vec<_> = active_tasks
            .iter()
            .filter(|t| t.batch_id.as_ref() == Some(&batch.batch_id))
            .collect();

        if !batch_active.is_empty() {
            let mut live_speed: f64 = 0.0;
            let mut live_downloaded: u64 = 0;

            for task in &batch_active {
                live_speed += task.speed;
                live_downloaded += task.downloaded;
            }

            batch.speed = live_speed;

            // Fix double-counting: The DB query summed 'downloaded' for ALL tasks including active ones.
            // But for active tasks, the DB value is stale. We need to:
            // 1. Subtract the number of active tasks * their stale DB contribution (usually 0)
            // 2. Add the live values from memory
            //
            // Since active tasks typically have 0 in DB until paused/completed, the DB sum
            // represents completed tasks. Adding live_downloaded gives us the accurate total.
            // However, if tasks were resumed with partial progress saved, we might double-count.
            //
            // Safe approach: Replace progress calculation entirely with:
            // (DB downloaded for non-active tasks) + (memory downloaded for active tasks)
            //
            // Since we know downloading_items count, we can approximate:
            // DB already has 0 for active tasks' downloaded (they haven't been persisted since starting),
            // so batch.downloaded_size is already "completed + paused" progress.
            // We just add live progress for currently active tasks.
            batch.downloaded_size += live_downloaded;

            // Recalculate progress
            if batch.total_size > 0 {
                batch.progress =
                    (batch.downloaded_size as f64 / batch.total_size as f64 * 100.0) as f32;
            }
        }
    }

    // Get status counts from database
    let db_status_counts = state
        .db
        .get_status_counts_filtered_async(params.source.as_deref())
        .await
        .unwrap_or_default();
    let status_counts = StatusCounts::from_db_counts(db_status_counts);

    let stats = state.download_orchestrator.task_manager().get_stats().await;
    let total_pages = ((total as f64) / (limit as f64)).ceil() as u32;
    // qB rows without a Flasharr task are deliberately invisible.
    let _ = untracked_torrents;

    Json(BatchSummariesResponse {
        batches: batches_with_realtime,
        standalone: standalone_with_realtime,
        rows,
        torrents: vec![],
        stats,
        status_counts,
        total,
        page,
        limit,
        total_pages,
    })
}

/// GET /api/downloads/batch/:batch_id/items - Get all items in a batch
/// Used for lazy loading when user expands a batch
async fn get_batch_items(
    State(state): State<Arc<AppState>>,
    Path(batch_id): Path<String>,
    Query(params): Query<ListDownloadsQuery>,
) -> Result<Json<Vec<DownloadTask>>, StatusCode> {
    // Get all tasks with this batch_id from database
    let tasks = match state
        .db
        .get_tasks_by_batch_id_async(batch_id.clone(), params.source.as_deref())
        .await
    {
        Ok(tasks) => tasks,
        Err(e) => {
            tracing::error!("Failed to get tasks for batch {}: {}", batch_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Return 200 empty instead of 404 when filter matches zero children —
    // the batch header is still valid; only its source-filtered children are empty.
    // Callers should treat empty arrays as "no items pass this filter" not "missing batch".

    // Merge real-time progress from active downloads
    let active_tasks = state
        .download_orchestrator
        .task_manager()
        .get_active_tasks()
        .await;
    let mut tasks_with_realtime = tasks;
    for task in tasks_with_realtime.iter_mut() {
        if let Some(active) = active_tasks.iter().find(|t| t.id == task.id) {
            task.progress = active.progress;
            task.speed = active.speed;
            task.eta = active.eta;
            task.downloaded = active.downloaded;
            task.state = active.state;
        }
    }

    Ok(Json(
        tasks_with_realtime
            .into_iter()
            .map(enrich_download)
            .collect(),
    ))
}

// ============================================================================
// Backfill / Maintenance
// ============================================================================

#[derive(Debug, Deserialize)]
struct BackfillQuery {
    dry_run: Option<bool>,
    limit: Option<usize>,
    kind: Option<String>,
}

#[derive(Serialize)]
struct BackfillResponse {
    success: bool,
    dry_run: bool,
    total_processed: usize,
    applied: usize,
    skipped: usize,
    failed: usize,
    results: Vec<BackfillItem>,
}

/// POST /api/downloads/backfill
/// Plans recovery by default; only mutates with dry_run=false.
async fn backfill_arr_paths(
    State(state): State<Arc<AppState>>,
    Query(params): Query<BackfillQuery>,
) -> Result<Json<BackfillResponse>, (StatusCode, String)> {
    let dry_run = params.dry_run.unwrap_or(true);
    let limit = params.limit.unwrap_or(50).clamp(1, 500);
    let kind = params
        .kind
        .unwrap_or_else(|| "all".to_string())
        .to_lowercase();
    if !matches!(kind.as_str(), "all" | "tv" | "movie") {
        return Err((
            StatusCode::BAD_REQUEST,
            "kind must be all, tv, or movie".into(),
        ));
    }

    let results = state
        .download_orchestrator
        .backfill_arr_paths(dry_run, limit, &kind)
        .await;

    let total_processed = results.len();
    let applied = results
        .iter()
        .filter(|r| matches!(r.action.as_str(), "scan_requested" | "marked_announced"))
        .count();
    let failed = results.iter().filter(|r| r.action == "failed").count();
    let skipped = total_processed.saturating_sub(applied + failed);

    Ok(Json(BackfillResponse {
        success: true,
        dry_run,
        total_processed,
        applied,
        skipped,
        failed,
        results,
    }))
}
