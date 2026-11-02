//! Folder Cache Service
//!
//! Handles recursive scanning of Fshare folders and populating the FTS5 cache.
//! Provides search interface over the cached data.

use super::folder::FolderSourceEntry;
use crate::db::sqlite::{CachedFolderItem, FolderCacheSnapshotItem};
use crate::db::Db;
use crate::services::tmdb_service::TmdbService;
use crate::utils::parser::FilenameParser;
use futures_util::stream::{self, StreamExt};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;
use serde_json::json;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

/// Maximum recursion depth for subfolder scanning
const MAX_DEPTH: u32 = 5;
/// Items per page for v3 API
const PER_PAGE: usize = 50;
/// Batch size for DB inserts
const INSERT_BATCH_SIZE: usize = 500;
/// Max total pages per folder to prevent runaway scanning
const MAX_PAGES_PER_FOLDER: u32 = 200;
/// Retry missing TMDB titles weekly; transport failures are never cached.
const TMDB_NEGATIVE_CACHE_TTL_SECS: i64 = 7 * 24 * 60 * 60;
const MAX_SOURCE_LIST_BYTES: usize = 1024 * 1024;
const MAX_SOURCE_LIST_LINES: usize = 10_000;

pub struct FolderCacheService {
    db: Arc<Db>,
    tmdb: Arc<TmdbService>,
    sync_lock: Arc<tokio::sync::Mutex<()>>,
    progress: Arc<std::sync::Mutex<SyncProgress>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SyncProgress {
    pub phase: String,
    pub total_sources: usize,
    pub completed_sources: usize,
    pub folders_scanned: usize,
    pub items_found: usize,
    pub current_source: Option<String>,
    pub current_folder: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub outcome: Option<String>,
    pub errors: Vec<String>,
}

impl Default for SyncProgress {
    fn default() -> Self {
        Self {
            phase: "idle".to_string(),
            total_sources: 0,
            completed_sources: 0,
            folders_scanned: 0,
            items_found: 0,
            current_source: None,
            current_folder: None,
            started_at: None,
            completed_at: None,
            outcome: None,
            errors: Vec::new(),
        }
    }
}

/// Report from a sync operation
#[derive(Debug, Serialize, Clone)]
pub struct SyncReport {
    pub total_items: usize,
    pub total_sources: usize,
    pub total_folders_scanned: usize,
    pub total_folders_skipped: usize,
    pub tmdb_mapped: usize,
    pub tmdb_skipped: usize,
    pub duration_secs: f64,
    pub errors: Vec<String>,
}

/// Candidate item for TMDB mapping (any depth, any type)
#[derive(Debug, Clone)]
struct TmdbCandidate {
    linkcode: String,
    title: String,
    year: Option<u32>,
    category: String,
}

#[derive(Default)]
struct SourceCrawlResult {
    items: Vec<CachedFolderItem>,
    snapshots: HashMap<String, FolderCacheSnapshotItem>,
    tmdb_candidates: Vec<TmdbCandidate>,
    folder_modified: HashMap<String, i64>,
    folders_scanned: usize,
    folders_skipped: usize,
    errors: Vec<String>,
    failed: bool,
}

/// Status of the folder cache
#[derive(Debug, Serialize)]
pub struct SyncStatus {
    pub last_sync: Option<String>,
    pub total_items: u64,
    pub folder_count: u64,
    pub file_count: u64,
    pub is_syncing: bool,
    pub last_diff_added: u64,
    pub last_diff_removed: u64,
    pub last_diff_changed: u64,
    pub last_sync_report: Option<String>,
    pub last_error: Option<String>,
    pub progress: SyncProgress,
}

#[derive(Debug, Clone, Copy)]
struct FolderDiffSummary {
    added: usize,
    removed: usize,
    changed: usize,
}

impl FolderDiffSummary {
    fn is_empty(self) -> bool {
        self.added == 0 && self.removed == 0 && self.changed == 0
    }
}

/// A source item to process recursively or directly.
enum SourceToScan {
    Folder {
        linkcode: String,
        category: String,
        label: String,
        depth: u32,
        parent_linkcode: String,
        token: String,
    },
    File {
        linkcode: String,
        category: String,
        label: String,
        parent_linkcode: String,
        token: String,
    },
}

/// Raw item parsed from Fshare v3 XML response
#[derive(Debug, Clone, Default)]
struct FshareXmlItem {
    linkcode: String,
    name: String,
    r#type: String,
    size: String,
    mimetype: String,
    path: String,
    modified: Option<i64>,
}

impl FolderCacheService {
    pub fn new(db: Arc<Db>, tmdb: Arc<TmdbService>) -> Self {
        Self {
            db,
            tmdb,
            sync_lock: Arc::new(tokio::sync::Mutex::new(())),
            progress: Arc::new(std::sync::Mutex::new(SyncProgress::default())),
        }
    }

    fn begin_progress(&self) {
        let mut progress = self
            .progress
            .lock()
            .expect("folder cache progress lock poisoned");
        *progress = SyncProgress {
            phase: "fetching_sources".to_string(),
            started_at: Some(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        };
    }

    fn finish_progress(&self, outcome: &str, errors: Vec<String>) {
        let mut progress = self
            .progress
            .lock()
            .expect("folder cache progress lock poisoned");
        progress.phase = if outcome == "success" {
            "complete"
        } else {
            "failed"
        }
        .to_string();
        progress.outcome = Some(outcome.to_string());
        progress.completed_at = Some(chrono::Utc::now().to_rfc3339());
        progress.current_folder = None;
        progress.errors = errors;
        let _ = self.db.set_folder_cache_meta("last_outcome", outcome);
        let _ = self.db.set_folder_cache_meta(
            "last_error",
            if outcome == "failed" {
                "Cache index could not be refreshed."
            } else {
                ""
            },
        );
    }

    fn update_progress(&self, update: impl FnOnce(&mut SyncProgress)) {
        update(
            &mut self
                .progress
                .lock()
                .expect("folder cache progress lock poisoned"),
        );
    }

    pub fn sync_progress(&self) -> SyncProgress {
        self.progress
            .lock()
            .expect("folder cache progress lock poisoned")
            .clone()
    }

    pub fn start_sync(self: Arc<Self>) -> bool {
        let Ok(lock) = Arc::clone(&self.sync_lock).try_lock_owned() else {
            return false;
        };
        self.begin_progress();
        let _ = self.db.set_folder_cache_meta("sync_status", "syncing");
        tokio::spawn(async move {
            let result = self.sync_all_sources_inner(false).await;
            let errors = result
                .as_ref()
                .map(|report| report.errors.clone())
                .unwrap_or_else(|error| vec![error.to_string()]);
            let _ = self.db.set_folder_cache_meta("sync_status", "idle");
            self.finish_progress(if result.is_ok() { "success" } else { "failed" }, errors);
            drop(lock);
        });
        true
    }

    /// Start a background refresh of one Fshare folder subtree.
    pub fn start_folder_refresh(self: Arc<Self>, folder_url: String) -> Result<(), String> {
        validate_refresh_folder_url(&folder_url)?;
        let Ok(lock) = Arc::clone(&self.sync_lock).try_lock_owned() else {
            return Err("Folder cache sync already running".to_string());
        };
        self.begin_progress();
        let _ = self.db.set_folder_cache_meta("sync_status", "syncing");
        tokio::spawn(async move {
            let result = self.refresh_folder_inner(&folder_url).await;
            let errors = result
                .as_ref()
                .map(|report| report.errors.clone())
                .unwrap_or_else(|error| vec![error.to_string()]);
            let _ = self.db.set_folder_cache_meta("sync_status", "idle");
            self.finish_progress(if result.is_ok() { "success" } else { "failed" }, errors);
            if let Err(error) = result {
                tracing::error!("[FOLDER-CACHE] Scoped refresh failed: {error}");
            }
            drop(lock);
        });
        Ok(())
    }

    async fn run_exclusive(&self, nightly_only_changed: bool) -> anyhow::Result<SyncReport> {
        let _lock = self
            .sync_lock
            .try_lock()
            .map_err(|_| anyhow::anyhow!("Folder cache sync already running"))?;
        self.begin_progress();
        let _ = self.db.set_folder_cache_meta("sync_status", "syncing");
        let result = self.sync_all_sources_inner(nightly_only_changed).await;
        let errors = result
            .as_ref()
            .map(|report| report.errors.clone())
            .unwrap_or_else(|error| vec![error.to_string()]);
        let _ = self.db.set_folder_cache_meta("sync_status", "idle");
        self.finish_progress(if result.is_ok() { "success" } else { "failed" }, errors);
        result
    }

    /// Sync all folder sources if cache is stale (older than 24 hours) or empty
    pub async fn sync_if_stale(&self) {
        self.sync_if_stale_with_mode(false).await;
    }

    pub async fn sync_if_stale_with_mode(&self, nightly_only_changed: bool) {
        let is_stale = match self.db.get_folder_cache_meta("last_sync") {
            Ok(Some(ts)) => {
                // Parse timestamp and check if older than 24h
                if let Ok(last) = ts.parse::<i64>() {
                    let now = chrono::Utc::now().timestamp();
                    now - last > 86400
                } else {
                    true
                }
            }
            _ => true, // No metadata = never synced
        };

        let item_count = self.db.get_folder_cache_count().unwrap_or(0);

        if is_stale || item_count == 0 {
            tracing::info!(
                "[FOLDER-CACHE] Cache is stale or empty (items: {}, stale: {}), starting sync",
                item_count,
                is_stale
            );
            match self
                .sync_all_sources_with_options(nightly_only_changed)
                .await
            {
                Ok(report) => {
                    tracing::info!(
                        "[FOLDER-CACHE] Sync complete: {} items from {} sources ({} folders) in {:.1}s",
                        report.total_items,
                        report.total_sources,
                        report.total_folders_scanned,
                        report.duration_secs
                    );
                }
                Err(e) => {
                    tracing::error!("[FOLDER-CACHE] Sync failed: {}", e);
                }
            }
        } else {
            tracing::info!(
                "[FOLDER-CACHE] Cache is fresh ({} items), skipping sync",
                item_count
            );
        }
    }

    /// Perform a full sync: fetch gist, recursively scan all folders, populate FTS5 cache.
    /// Computes a diff against the previous snapshot for logging before replacing the cache.
    pub async fn sync_all_sources(&self) -> anyhow::Result<SyncReport> {
        self.run_exclusive(false).await
    }

    pub async fn sync_all_sources_with_options(
        &self,
        nightly_only_changed: bool,
    ) -> anyhow::Result<SyncReport> {
        self.run_exclusive(nightly_only_changed).await
    }

    async fn sync_all_sources_inner(
        &self,
        nightly_only_changed: bool,
    ) -> anyhow::Result<SyncReport> {
        let start = std::time::Instant::now();
        let mut errors = Vec::new();

        let gist_url = self
            .db
            .get_setting("folder_sources_gist_url")
            .ok()
            .flatten()
            .unwrap_or_default();
        let imported_source_text = self
            .db
            .get_setting("folder_sources_imported_list")
            .ok()
            .flatten()
            .unwrap_or_default();

        if gist_url.is_empty() && imported_source_text.is_empty() {
            return Err(anyhow::anyhow!("No Fshare folder sources configured"));
        }

        let _ = self.db.set_folder_cache_meta("sync_status", "syncing");
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let semaphore = Arc::new(tokio::sync::Semaphore::new(12));
        let mut source_urls =
            normalize_optional_source_list(&imported_source_text).map_err(anyhow::Error::msg)?;

        if !gist_url.is_empty() {
            let body = match client.get(&gist_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    if resp
                        .content_length()
                        .is_some_and(|size| size > MAX_SOURCE_LIST_BYTES as u64)
                    {
                        return Err(anyhow::anyhow!("Remote source list exceeds 1 MiB"));
                    }
                    let body = resp.text().await?;
                    if body.len() > MAX_SOURCE_LIST_BYTES {
                        return Err(anyhow::anyhow!("Remote source list exceeds 1 MiB"));
                    }
                    body
                }
                Ok(resp) => {
                    let msg = format!("Source list fetch failed: HTTP {}", resp.status());
                    let _ = self.db.set_folder_cache_meta("sync_status", "idle");
                    return Err(anyhow::anyhow!(msg));
                }
                Err(error) => {
                    let _ = self.db.set_folder_cache_meta("sync_status", "idle");
                    return Err(anyhow::anyhow!("Source list fetch failed: {error}"));
                }
            };
            source_urls.extend(normalize_source_list(&body).map_err(anyhow::Error::msg)?);
        }

        let mut seen_codes = HashSet::new();
        let entries: Vec<FolderSourceEntry> = source_urls
            .into_iter()
            .filter_map(|url| {
                let code = extract_source_code(&url)?;
                seen_codes
                    .insert(code.clone())
                    .then_some(FolderSourceEntry {
                        category: "auto".to_string(),
                        label: code,
                        folder_url: url,
                    })
            })
            .collect();
        if entries.is_empty() {
            return Err(anyhow::anyhow!("No valid Fshare folder sources configured"));
        }

        tracing::info!("[FOLDER-CACHE] Syncing {} folder sources", entries.len());
        self.update_progress(|progress| {
            progress.phase = "scanning".to_string();
            progress.total_sources = entries.len();
        });

        // ── PRE-APPLY DIFF SNAPSHOT ─────────────────────────────────────────────
        // Read current cache state before clearing, so we can diff and log.
        let prev_snapshot = self.db.snapshot_folder_cache_files().unwrap_or_default();

        let total_sources = entries.len();
        let mut total_items = 0;
        let mut total_folders_scanned = 0;
        let mut total_folders_skipped = 0;
        let mut tmdb_candidates: Vec<TmdbCandidate> = Vec::new();
        let known_folder_modified =
            Arc::new(self.db.get_all_folder_modified_times().unwrap_or_default());
        let mut next_folder_modified = HashMap::new();
        let mut retained_items_by_label = if nightly_only_changed {
            self.db
                .get_folder_cache_items_grouped_by_label()
                .unwrap_or_default()
        } else {
            HashMap::new()
        };
        let mut seen_candidate_titles: HashSet<String> = HashSet::new();
        let mut all_items: Vec<CachedFolderItem> = Vec::new();
        // Track new snapshot for diff computation
        let mut new_snapshot: HashMap<String, FolderCacheSnapshotItem> = HashMap::new();

        let crawl_inputs = entries.into_iter().enumerate().map(|(index, entry)| {
            let retained_items = retained_items_by_label
                .remove(&entry.label)
                .unwrap_or_default();
            (index, entry, retained_items)
        });
        let mut crawl_results = stream::iter(crawl_inputs)
            .map(|(index, entry, retained_items)| {
                let client = client.clone();
                let semaphore = Arc::clone(&semaphore);
                let known_folder_modified = Arc::clone(&known_folder_modified);
                async move {
                    (
                        index,
                        self.crawl_source(
                            entry,
                            &client,
                            semaphore,
                            nightly_only_changed,
                            known_folder_modified,
                            retained_items,
                        )
                        .await,
                    )
                }
            })
            .buffer_unordered(10)
            .collect::<Vec<_>>()
            .await;

        // buffer_unordered completes nondeterministically; merge gist order for stable dedupe.
        crawl_results.sort_unstable_by_key(|(index, _)| *index);
        for (_, result) in crawl_results {
            total_folders_scanned += result.folders_scanned;
            total_folders_skipped += result.folders_skipped;
            total_items += result.items.len();
            errors.extend(result.errors);
            next_folder_modified.extend(result.folder_modified);
            new_snapshot.extend(result.snapshots);
            for candidate in result.tmdb_candidates {
                let normalized = candidate.title.trim().to_lowercase();
                if seen_candidate_titles.insert(normalized) {
                    tmdb_candidates.push(candidate);
                }
            }
            all_items.extend(result.items);
            self.update_progress(|progress| {
                progress.completed_sources += 1;
                progress.folders_scanned = total_folders_scanned;
                progress.items_found = total_items;
                progress.current_folder = None;
            });
        }

        if all_items.is_empty() {
            let msg = "Sync produced 0 items; keeping existing cache".to_string();
            errors.push(msg.clone());
            let _ = self.db.set_folder_cache_meta("sync_status", "idle");
            return Err(anyhow::anyhow!(msg));
        }

        // Fshare linkcodes are global artifact identities. Curated roots may overlap, so
        // merge the same item once before the atomic relational-cache replacement.
        let raw_item_count = all_items.len();
        let mut unique_items = std::collections::BTreeMap::new();
        for item in all_items {
            unique_items.insert(item.linkcode.clone(), item);
        }
        let duplicate_linkcodes = raw_item_count.saturating_sub(unique_items.len());
        if duplicate_linkcodes > 0 {
            tracing::warn!(
                "[FOLDER-CACHE] Merged {} duplicate cross-source linkcodes",
                duplicate_linkcodes
            );
        }
        let all_items = unique_items.into_values().collect::<Vec<_>>();
        total_items = all_items.len();

        // ── DIFF COMPUTATION AND LOGGING ────────────────────────────────────────
        self.update_progress(|progress| {
            progress.phase = "updating_cache".to_string();
            progress.current_folder = None;
        });
        let diff = compute_folder_diff(&prev_snapshot, &new_snapshot);
        tracing::info!(
            "[FOLDER-CACHE] Diff — added: {}, removed: {}, changed: {}",
            diff.added,
            diff.removed,
            diff.changed
        );

        let cache_changed = !diff.is_empty();
        if cache_changed {
            total_items = self
                .db
                .replace_folder_cache(&all_items)
                .map_err(|e| anyhow::anyhow!("Failed to replace folder cache: {}", e))?;
        } else {
            tracing::info!("[FOLDER-CACHE] No content diff; retaining existing FTS cache");
            total_items = all_items.len();
        }

        // ── TMDB MAPPING PASS ───────────────────────────────────────────────────
        let (tmdb_mapped, tmdb_skipped) = if tmdb_candidates.is_empty() {
            (0, 0)
        } else {
            self.update_progress(|progress| progress.phase = "mapping_metadata".to_string());
            tracing::info!(
                "[FOLDER-CACHE] Starting TMDB mapping for {} unique titles",
                tmdb_candidates.len()
            );
            self.run_tmdb_mapping(&tmdb_candidates, &mut errors).await
        };

        // Individual source failures are retained in the report; a successful
        // replacement still advances the search generation for indexed sources.
        // ponytail: per-source atomic cache swaps if partial-source rollback is needed.

        // Update metadata
        let now = chrono::Utc::now().timestamp().to_string();
        let _ = self.db.set_folder_cache_meta("last_sync", &now);
        let generation = self
            .db
            .get_folder_cache_meta("sync_generation")
            .ok()
            .flatten()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0)
            .saturating_add(1);
        let _ = self
            .db
            .set_folder_cache_meta("sync_generation", &generation.to_string());
        let _ = self.db.set_folder_cache_meta("sync_status", "idle");
        let _ = self
            .db
            .set_folder_cache_meta("total_items", &total_items.to_string());
        let _ = self
            .db
            .set_folder_cache_meta("last_diff_added", &diff.added.to_string());
        let _ = self
            .db
            .set_folder_cache_meta("last_diff_removed", &diff.removed.to_string());
        let _ = self
            .db
            .set_folder_cache_meta("last_diff_changed", &diff.changed.to_string());
        let report_json = json!({
            "total_items": total_items,
            "total_sources": total_sources,
            "total_folders_scanned": total_folders_scanned,
            "total_folders_skipped": total_folders_skipped,
            "tmdb_mapped": tmdb_mapped,
            "diff_added": diff.added,
            "diff_removed": diff.removed,
            "diff_changed": diff.changed,
            "errors": errors.len(),
            "synced_at": now,
        })
        .to_string();
        let _ = self
            .db
            .set_folder_cache_meta("last_sync_report", &report_json);

        let _ = self.db.replace_folder_modified_times(&next_folder_modified);

        let duration = start.elapsed().as_secs_f64();

        if !errors.is_empty() {
            tracing::warn!("[FOLDER-CACHE] Sync completed with {} errors", errors.len());
            for err in &errors {
                tracing::warn!("[FOLDER-CACHE]   - {}", err);
            }
        }

        Ok(SyncReport {
            total_items,
            total_sources,
            total_folders_scanned,
            total_folders_skipped,
            tmdb_mapped,
            tmdb_skipped,
            duration_secs: duration,
            errors,
        })
    }

    async fn crawl_source(
        &self,
        entry: FolderSourceEntry,
        client: &reqwest::Client,
        semaphore: Arc<tokio::sync::Semaphore>,
        nightly_only_changed: bool,
        known_folder_modified: Arc<HashMap<String, i64>>,
        mut retained_items: Vec<CachedFolderItem>,
    ) -> SourceCrawlResult {
        let mut result = SourceCrawlResult::default();
        let Some(source) = parse_source_entry(&entry.folder_url, &entry.category, &entry.label)
        else {
            result.errors.push(format!(
                "Invalid URL for '{}': {}",
                entry.label, entry.folder_url
            ));
            return result;
        };

        let mut queue = VecDeque::from([source]);
        let mut seen_folders = HashSet::new();
        let mut seen_files = HashSet::new();
        let mut seen_candidate_titles = HashSet::new();

        while let Some(source) = queue.pop_front() {
            match source {
                SourceToScan::Folder {
                    linkcode,
                    category,
                    label,
                    depth,
                    parent_linkcode,
                    token,
                } => {
                    if depth > MAX_DEPTH {
                        result.errors.push(format!(
                            "Remote crawl degraded: depth safety cap {MAX_DEPTH} reached at {linkcode}"
                        ));
                        continue;
                    }
                    if !seen_folders.insert(linkcode.clone()) {
                        continue;
                    }

                    result.folders_scanned += 1;
                    let mut page = 1;
                    let mut skip_current_folder = false;

                    loop {
                        let xml_body = match fetch_folder_page(
                            client,
                            semaphore.as_ref(),
                            &linkcode,
                            &token,
                            page,
                        )
                        .await
                        {
                            Ok(body) => body,
                            Err(error) => {
                                result
                                    .errors
                                    .push(format!("API error for {linkcode}: {error}"));
                                result.failed = true;
                                break;
                            }
                        };

                        let (items, has_next) = parse_folder_response(&xml_body);
                        if page == 1 {
                            if let Some(modified) =
                                items.iter().filter_map(|item| item.modified).max()
                            {
                                result.folder_modified.insert(linkcode.clone(), modified);
                                let unchanged = nightly_only_changed
                                    && known_folder_modified.get(&linkcode).copied()
                                        == Some(modified);
                                if unchanged {
                                    result.folders_skipped += 1;
                                    for retained in retained_items.drain(..) {
                                        if !seen_files.insert(retained.linkcode.clone()) {
                                            continue;
                                        }
                                        result.snapshots.insert(
                                            retained.linkcode.clone(),
                                            FolderCacheSnapshotItem {
                                                name: retained.name.clone(),
                                                parent_linkcode: retained.parent_linkcode.clone(),
                                                size: retained.size,
                                                path: retained.path.clone(),
                                                is_directory: retained.is_directory,
                                            },
                                        );
                                        let parsed = FilenameParser::parse(&retained.name);
                                        let normalized = parsed.title.trim().to_lowercase();
                                        if !normalized.is_empty()
                                            && is_likely_content_title(&normalized)
                                            && seen_candidate_titles.insert(normalized)
                                        {
                                            result.tmdb_candidates.push(TmdbCandidate {
                                                linkcode: retained.linkcode.clone(),
                                                title: parsed.title,
                                                year: parsed.year,
                                                category: category.clone(),
                                            });
                                        }
                                        result.items.push(retained);
                                    }
                                    skip_current_folder = true;
                                    break;
                                }
                            }
                        }

                        if items.is_empty() {
                            break;
                        }

                        for item in &items {
                            let looks_like_folder = item.r#type.trim() == "0"
                                || item.path.contains("/folder/")
                                || item.mimetype == "application/vnd.fshare-folder";
                            let size = item.size.parse().unwrap_or(0);

                            if looks_like_folder {
                                if !item.linkcode.is_empty() {
                                    queue.push_back(SourceToScan::Folder {
                                        linkcode: item.linkcode.clone(),
                                        category: category.clone(),
                                        label: label.clone(),
                                        depth: depth + 1,
                                        parent_linkcode: linkcode.clone(),
                                        token: token.clone(),
                                    });
                                    let parsed = FilenameParser::parse(&item.name);
                                    result.snapshots.insert(
                                        item.linkcode.clone(),
                                        FolderCacheSnapshotItem {
                                            name: item.name.clone(),
                                            parent_linkcode: linkcode.clone(),
                                            size,
                                            path: item.path.clone(),
                                            is_directory: true,
                                        },
                                    );
                                    result.items.push(CachedFolderItem {
                                        linkcode: item.linkcode.clone(),
                                        name: item.name.clone(),
                                        title: parsed.title,
                                        category: category.clone(),
                                        label: label.clone(),
                                        parent_linkcode: linkcode.clone(),
                                        fshare_url: format!(
                                            "https://www.fshare.vn/folder/{}",
                                            item.linkcode
                                        ),
                                        year: parsed.year,
                                        season: parsed.season,
                                        episode: parsed.episode,
                                        is_series: parsed.is_series,
                                        is_directory: true,
                                        size,
                                        quality: parsed.quality_attrs.quality_name(),
                                        path: item.path.clone(),
                                        tmdb_id: None,
                                        media_type_hint: None,
                                        poster_path: None,
                                    });
                                }
                                continue;
                            }

                            if item.linkcode.is_empty() || !seen_files.insert(item.linkcode.clone())
                            {
                                continue;
                            }

                            let parsed = FilenameParser::parse(&item.name);
                            result.snapshots.insert(
                                item.linkcode.clone(),
                                FolderCacheSnapshotItem {
                                    name: item.name.clone(),
                                    parent_linkcode: linkcode.clone(),
                                    size,
                                    path: item.path.clone(),
                                    is_directory: false,
                                },
                            );
                            let normalized = parsed.title.trim().to_lowercase();
                            if !normalized.is_empty()
                                && is_likely_content_title(&normalized)
                                && seen_candidate_titles.insert(normalized)
                            {
                                result.tmdb_candidates.push(TmdbCandidate {
                                    linkcode: item.linkcode.clone(),
                                    title: parsed.title.clone(),
                                    year: parsed.year,
                                    category: category.clone(),
                                });
                            }
                            result.items.push(CachedFolderItem {
                                linkcode: item.linkcode.clone(),
                                name: item.name.clone(),
                                title: parsed.title,
                                category: category.clone(),
                                label: label.clone(),
                                parent_linkcode: linkcode.clone(),
                                fshare_url: format!("https://www.fshare.vn/file/{}", item.linkcode),
                                year: parsed.year,
                                season: parsed.season,
                                episode: parsed.episode,
                                is_series: parsed.is_series,
                                is_directory: false,
                                size,
                                quality: parsed.quality_attrs.quality_name(),
                                path: item.path.clone(),
                                tmdb_id: None,
                                media_type_hint: None,
                                poster_path: None,
                            });
                        }

                        if !has_next || items.len() < PER_PAGE {
                            break;
                        }
                        page += 1;
                        if page > MAX_PAGES_PER_FOLDER {
                            tracing::warn!("[FOLDER-CACHE] Hit page limit for folder {linkcode}");
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }

                    if skip_current_folder {
                        tracing::info!("[FOLDER-CACHE] Skipped unchanged folder {linkcode}");
                    }
                    let _ = parent_linkcode;
                }
                SourceToScan::File {
                    linkcode,
                    category,
                    label,
                    parent_linkcode,
                    token: _,
                } => match fetch_file_info(client, semaphore.as_ref(), &linkcode).await {
                    Ok(file_item) => {
                        if !seen_files.insert(file_item.linkcode.clone()) {
                            continue;
                        }

                        let size = file_item.size.parse().unwrap_or(0);
                        let parsed = FilenameParser::parse(&file_item.name);
                        result.snapshots.insert(
                            file_item.linkcode.clone(),
                            FolderCacheSnapshotItem {
                                name: file_item.name.clone(),
                                parent_linkcode: parent_linkcode.clone(),
                                size,
                                path: file_item.path.clone(),
                                is_directory: false,
                            },
                        );
                        let normalized = parsed.title.trim().to_lowercase();
                        if !normalized.is_empty()
                            && is_likely_content_title(&normalized)
                            && seen_candidate_titles.insert(normalized)
                        {
                            result.tmdb_candidates.push(TmdbCandidate {
                                linkcode: file_item.linkcode.clone(),
                                title: parsed.title.clone(),
                                year: parsed.year,
                                category: category.clone(),
                            });
                        }
                        result.items.push(CachedFolderItem {
                            linkcode: file_item.linkcode.clone(),
                            name: file_item.name.clone(),
                            title: parsed.title,
                            category,
                            label,
                            parent_linkcode: parent_linkcode.clone(),
                            fshare_url: format!(
                                "https://www.fshare.vn/file/{}",
                                file_item.linkcode
                            ),
                            year: parsed.year,
                            season: parsed.season,
                            episode: parsed.episode,
                            is_series: parsed.is_series,
                            is_directory: false,
                            size,
                            quality: parsed.quality_attrs.quality_name(),
                            path: file_item.path,
                            tmdb_id: None,
                            media_type_hint: None,
                            poster_path: None,
                        });
                    }
                    Err(error) => {
                        result
                            .errors
                            .push(format!("File info error for {linkcode}: {error}"));
                        result.failed = true;
                    }
                },
            }
        }

        if result.failed {
            // A broken source retains its old snapshot; other sources still replace atomically.
            result.items.clear();
            result.snapshots.clear();
            result.tmdb_candidates.clear();
            for retained in retained_items {
                result.snapshots.insert(
                    retained.linkcode.clone(),
                    FolderCacheSnapshotItem {
                        name: retained.name.clone(),
                        parent_linkcode: retained.parent_linkcode.clone(),
                        size: retained.size,
                        path: retained.path.clone(),
                        is_directory: retained.is_directory,
                    },
                );
                result.items.push(retained);
            }
            result.errors.push(format!(
                "Source '{}' failed; retained previous snapshot and continued",
                entry.label
            ));
        }

        result
    }

    async fn refresh_folder_inner(&self, folder_url: &str) -> anyhow::Result<SyncReport> {
        let start = std::time::Instant::now();
        let normalized = normalize_source_url(folder_url).map_err(anyhow::Error::msg)?;
        let root_linkcode = extract_folder_code(&normalized)
            .ok_or_else(|| anyhow::anyhow!("Only Fshare folder URLs can be refreshed"))?;
        let (category, label) = self
            .db
            .get_folder_cache_context(&root_linkcode)
            .unwrap_or(None)
            .unwrap_or_else(|| ("auto".to_string(), root_linkcode.clone()));
        let entry = FolderSourceEntry {
            category,
            label,
            folder_url: normalized,
        };
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        self.update_progress(|progress| {
            progress.phase = "scanning".to_string();
            progress.total_sources = 1;
            progress.current_source = Some(root_linkcode.clone());
        });
        let result = self
            .crawl_source(
                entry,
                &client,
                Arc::new(tokio::sync::Semaphore::new(6)),
                false,
                Arc::new(HashMap::new()),
                Vec::new(),
            )
            .await;
        if result.failed || result.items.is_empty() {
            return Err(anyhow::anyhow!(
                "Folder sync produced no usable items; keeping existing cache"
            ));
        }

        let previous = self
            .db
            .snapshot_folder_cache_subtree(&root_linkcode)
            .unwrap_or_default();
        let diff = compute_folder_diff(&previous, &result.snapshots);
        self.update_progress(|progress| {
            progress.phase = "updating_cache".to_string();
            progress.folders_scanned = result.folders_scanned;
            progress.items_found = result.items.len();
        });
        let generation = self
            .db
            .get_folder_cache_meta("sync_generation")
            .ok()
            .flatten()
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0)
            .saturating_add(1);
        let total_items = self
            .db
            .replace_folder_cache_subtree(&root_linkcode, &result.items, generation)
            .map_err(|error| anyhow::anyhow!("Failed to replace folder subtree: {error}"))?;
        let mut errors = result.errors;
        let (tmdb_mapped, tmdb_skipped) = self
            .run_tmdb_mapping(&result.tmdb_candidates, &mut errors)
            .await;
        let now = chrono::Utc::now().timestamp().to_string();
        let cache_total = self.db.get_folder_cache_count().unwrap_or(0);
        let _ = self.db.set_folder_cache_meta("last_sync", &now);
        let _ = self
            .db
            .set_folder_cache_meta("sync_generation", &generation.to_string());
        let _ = self
            .db
            .set_folder_cache_meta("total_items", &cache_total.to_string());
        let _ = self
            .db
            .set_folder_cache_meta("last_diff_added", &diff.added.to_string());
        let _ = self
            .db
            .set_folder_cache_meta("last_diff_removed", &diff.removed.to_string());
        let _ = self
            .db
            .set_folder_cache_meta("last_diff_changed", &diff.changed.to_string());
        let report_json = json!({
            "total_items": total_items,
            "cache_total_items": cache_total,
            "total_sources": 1,
            "total_folders_scanned": result.folders_scanned,
            "tmdb_mapped": tmdb_mapped,
            "diff_added": diff.added,
            "diff_removed": diff.removed,
            "diff_changed": diff.changed,
            "errors": errors.len(),
            "synced_at": now,
            "scope": root_linkcode,
        })
        .to_string();
        let _ = self
            .db
            .set_folder_cache_meta("last_sync_report", &report_json);
        self.update_progress(|progress| {
            progress.completed_sources = 1;
            progress.items_found = total_items;
            progress.current_folder = None;
        });
        Ok(SyncReport {
            total_items,
            total_sources: 1,
            total_folders_scanned: result.folders_scanned,
            total_folders_skipped: 0,
            tmdb_mapped,
            tmdb_skipped,
            duration_secs: start.elapsed().as_secs_f64(),
            errors,
        })
    }

    /// Run TMDB mapping for top-level folders that don't already have a mapping.
    /// Returns (mapped_count, skipped_count).
    async fn run_tmdb_mapping(
        &self,
        items: &[TmdbCandidate],
        errors: &mut Vec<String>,
    ) -> (usize, usize) {
        let existing = match self.db.get_all_folder_tmdb_mappings() {
            Ok(mappings) => mappings,
            Err(e) => {
                errors.push(format!("Failed to load TMDB mappings: {}", e));
                return (0, 0);
            }
        };
        let negative_keys = match self.db.get_folder_tmdb_negative_keys(
            chrono::Utc::now().timestamp() - TMDB_NEGATIVE_CACHE_TTL_SECS,
        ) {
            Ok(keys) => keys,
            Err(e) => {
                errors.push(format!("Failed to load TMDB negative cache: {}", e));
                return (0, 0);
            }
        };

        let mut title_groups: HashMap<String, Vec<&TmdbCandidate>> = HashMap::new();
        let mut skipped = 0usize;
        for item in items {
            if existing.contains_key(&item.linkcode) {
                skipped += 1;
                continue;
            }
            let media_type = tmdb_media_type(item);
            let cache_key = tmdb_negative_cache_key(media_type, &item.title, item.year);
            if negative_keys.contains(&cache_key) {
                skipped += 1;
                continue;
            }
            title_groups.entry(cache_key).or_default().push(item);
        }

        tracing::info!(
            "[FOLDER-CACHE] TMDB: {} titles to map, {} cached/skipped",
            title_groups.len(),
            skipped
        );

        let mut mapped_count = 0usize;
        let mut batch_mappings: Vec<(String, i64, String, String)> = Vec::new();
        let mut misses = Vec::new();

        for (cache_key, group) in &title_groups {
            let sample = group[0];
            let media_type = tmdb_media_type(sample);
            let search_query = match sample.year {
                Some(year) => format!("{} {}", sample.title, year),
                None => sample.title.clone(),
            };

            let mut matched = false;
            match self.tmdb.search_checked(media_type, &search_query).await {
                Ok(Some(data)) => {
                    if let Some(first) = data
                        .get("results")
                        .and_then(|r| r.as_array())
                        .and_then(|r| r.first())
                    {
                        let tmdb_id = first.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                        if tmdb_id > 0 {
                            let poster = first
                                .get("poster_path")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            for item in group {
                                batch_mappings.push((
                                    item.linkcode.clone(),
                                    tmdb_id,
                                    media_type.to_string(),
                                    poster.clone(),
                                ));
                            }
                            mapped_count += group.len();
                            matched = true;
                        }
                    }
                    if !matched {
                        misses.push(cache_key.clone());
                    }
                }
                Ok(None) => misses.push(cache_key.clone()),
                Err(error) => {
                    errors.push(format!("TMDB search '{}' failed: {}", search_query, error))
                }
            }

            if batch_mappings.len() >= 50 {
                if let Err(e) = self.db.set_folder_tmdb_mappings_batch(&batch_mappings) {
                    errors.push(format!("TMDB batch insert error: {}", e));
                }
                batch_mappings.clear();
            }
            tokio::time::sleep(std::time::Duration::from_millis(260)).await;
        }

        if !batch_mappings.is_empty() {
            if let Err(e) = self.db.set_folder_tmdb_mappings_batch(&batch_mappings) {
                errors.push(format!("TMDB batch insert error: {}", e));
            }
        }
        if let Err(e) = self.db.set_folder_tmdb_negative_keys(&misses) {
            errors.push(format!("TMDB negative cache insert error: {}", e));
        }

        tracing::info!(
            "[FOLDER-CACHE] TMDB mapping complete: {} mapped, {} skipped, {} misses cached",
            mapped_count,
            skipped,
            misses.len()
        );
        (mapped_count, skipped)
    }

    /// Search the FTS5 cache
    pub async fn search(&self, query: &str, limit: u32) -> anyhow::Result<Vec<CachedFolderItem>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let results = self
            .db
            .search_folder_cache_async(query.to_string(), limit)
            .await
            .map_err(|e| anyhow::anyhow!("Search failed: {}", e))?;
        Ok(results)
    }

    pub async fn search_season_candidates(
        &self,
        query: &str,
    ) -> anyhow::Result<Vec<CachedFolderItem>> {
        self.search(query, 200).await
    }

    pub async fn search_target_candidates(
        &self,
        query: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<CachedFolderItem>> {
        self.search(query, limit).await
    }

    /// Get the current sync status
    pub fn get_sync_status(&self) -> SyncStatus {
        let last_sync = self.db.get_folder_cache_meta("last_sync").ok().flatten();
        let total_items = self.db.get_folder_cache_count().unwrap_or(0);
        let (folder_count, file_count) = self.db.get_folder_cache_counts().unwrap_or((0, 0));
        let sync_status = self
            .db
            .get_folder_cache_meta("sync_status")
            .ok()
            .flatten()
            .unwrap_or_else(|| "idle".to_string());
        let last_diff_added = self
            .db
            .get_folder_cache_meta("last_diff_added")
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let last_diff_removed = self
            .db
            .get_folder_cache_meta("last_diff_removed")
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let last_diff_changed = self
            .db
            .get_folder_cache_meta("last_diff_changed")
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let last_sync_report = self
            .db
            .get_folder_cache_meta("last_sync_report")
            .ok()
            .flatten();
        let last_error = self
            .db
            .get_folder_cache_meta("last_error")
            .ok()
            .flatten()
            .filter(|message| !message.is_empty());

        let progress = self.sync_progress();
        SyncStatus {
            last_sync,
            total_items,
            folder_count,
            file_count,
            is_syncing: sync_status == "syncing"
                || progress.phase != "idle" && progress.outcome.is_none(),
            last_diff_added,
            last_diff_removed,
            last_diff_changed,
            last_sync_report,
            last_error,
            progress,
        }
    }
}

// ============================================================================
// Private helpers
// ============================================================================
/// Heuristic: is this parsed title likely a real movie/show name?
/// Filters out structural folder names, season labels, pure numbers, etc.
fn tmdb_media_type(item: &TmdbCandidate) -> &'static str {
    if item.category == "movie" {
        "movie"
    } else {
        "tv"
    }
}

fn tmdb_negative_cache_key(media_type: &str, title: &str, year: Option<u32>) -> String {
    format!(
        "{}:{}:{}",
        media_type,
        title.trim().to_lowercase(),
        year.map(|value| value.to_string()).unwrap_or_default()
    )
}

fn is_likely_content_title(title_lower: &str) -> bool {
    // Too short to be a real title
    if title_lower.len() < 2 {
        return false;
    }

    // Pure numbers (e.g. "01", "2024")
    if title_lower.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    // Generic structural folder names
    static SKIP_PATTERNS: &[&str] = &[
        "season",
        "s01",
        "s02",
        "s03",
        "s04",
        "s05",
        "s06",
        "s07",
        "s08",
        "s09",
        "s10",
        "folder",
        "phim",
        "movie",
        "film",
        "video",
        "collection",
        "complete",
        "subs",
        "subtitle",
        "extra",
        "bonus",
        "featurette",
        "sample",
        "disc",
        "disk",
        "cd",
        "dvd",
        "bluray",
        "remux",
        "temp",
        "tmp",
        "new",
        "old",
        "backup",
    ];

    // Skip if title IS one of these generic words
    if SKIP_PATTERNS.contains(&title_lower) {
        return false;
    }

    // Skip if title starts with common non-content patterns
    if title_lower.starts_with("season ") {
        return false;
    }
    if title_lower.starts_with("disc ") {
        return false;
    }
    if title_lower.starts_with("01_")
        || title_lower.starts_with("02_")
        || title_lower.starts_with("03_")
    {
        return false;
    }

    true
}

/// Extract folder linkcode from URL
fn compute_folder_diff(
    previous: &HashMap<String, FolderCacheSnapshotItem>,
    current: &HashMap<String, FolderCacheSnapshotItem>,
) -> FolderDiffSummary {
    let added = current
        .keys()
        .filter(|linkcode| !previous.contains_key(*linkcode))
        .count();
    let removed = previous
        .keys()
        .filter(|linkcode| !current.contains_key(*linkcode))
        .count();
    let changed = current
        .iter()
        .filter(|(linkcode, item)| previous.get(*linkcode).is_some_and(|prev| prev != *item))
        .count();

    FolderDiffSummary {
        added,
        removed,
        changed,
    }
}

fn extract_folder_code(folder_url: &str) -> Option<String> {
    extract_code_for_kind(folder_url, "folder")
}

fn extract_code_for_kind(source_url: &str, kind: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(source_url.trim()).ok()?;
    let mut segments = parsed.path_segments()?;
    if segments.next()? != kind {
        return None;
    }
    let code = segments.next()?.trim();
    if code.is_empty() || segments.any(|segment| !segment.is_empty()) {
        return None;
    }
    Some(code.to_string())
}

pub(crate) fn normalize_source_url(source_url: &str) -> Result<String, String> {
    let parsed = reqwest::Url::parse(source_url.trim())
        .map_err(|_| "Enter a valid Fshare folder or file URL".to_string())?;
    if parsed.scheme() != "https"
        || !matches!(parsed.host_str(), Some("fshare.vn") | Some("www.fshare.vn"))
    {
        return Err("Only HTTPS Fshare URLs are accepted".to_string());
    }
    let (kind, code) = if let Some(code) = extract_code_for_kind(parsed.as_str(), "folder") {
        ("folder", code)
    } else if let Some(code) = extract_code_for_kind(parsed.as_str(), "file") {
        ("file", code)
    } else {
        return Err("Enter an Fshare folder or file URL".to_string());
    };
    if !code
        .chars()
        .all(|character| character.is_ascii_alphanumeric())
    {
        return Err("Fshare link code is invalid".to_string());
    }
    let token = parsed
        .query_pairs()
        .find_map(|(key, value)| (key == "token").then_some(value.into_owned()));
    if parsed.query_pairs().any(|(key, _)| key != "token") {
        return Err("Fshare URL contains unsupported parameters".to_string());
    }
    let mut normalized = format!("https://www.fshare.vn/{kind}/{code}");
    if let Some(token) = token.filter(|value| !value.is_empty()) {
        let mut url = reqwest::Url::parse(&normalized).expect("canonical Fshare URL");
        url.query_pairs_mut().append_pair("token", &token);
        normalized = url.into();
    }
    Ok(normalized)
}

fn normalize_optional_source_list(source_text: &str) -> Result<Vec<String>, String> {
    if source_text.lines().all(|line| {
        let line = line.trim();
        line.is_empty() || line.starts_with('#')
    }) {
        return Ok(Vec::new());
    }
    normalize_source_list(source_text)
}

pub(crate) fn normalize_source_list(source_text: &str) -> Result<Vec<String>, String> {
    if source_text.len() > MAX_SOURCE_LIST_BYTES {
        return Err("Source list exceeds 1 MiB".to_string());
    }
    let lines = source_text.lines().collect::<Vec<_>>();
    if lines.len() > MAX_SOURCE_LIST_LINES {
        return Err("Source list contains too many lines".to_string());
    }
    let mut seen = HashSet::new();
    let mut sources = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let normalized = normalize_source_url(line)?;
        let code = extract_source_code(&normalized).expect("normalized Fshare URL has a code");
        if seen.insert(code) {
            sources.push(normalized);
        }
    }
    if sources.is_empty() {
        return Err("Source list contains no Fshare URLs".to_string());
    }
    Ok(sources)
}

fn validate_refresh_folder_url(url: &str) -> Result<(), String> {
    let parsed =
        reqwest::Url::parse(url.trim()).map_err(|_| "Invalid Fshare folder URL".to_string())?;
    if normalize_source_url(parsed.as_str()).is_err()
        || extract_folder_code(parsed.as_str()).is_none()
    {
        return Err("Enter an HTTPS Fshare folder URL".to_string());
    }
    Ok(())
}

fn extract_file_code(file_url: &str) -> Option<String> {
    extract_code_for_kind(file_url, "file")
}

fn extract_source_code(url: &str) -> Option<String> {
    extract_folder_code(url).or_else(|| extract_file_code(url))
}

fn parse_source_entry(url: &str, category: &str, label: &str) -> Option<SourceToScan> {
    let token = extract_token(url);
    if let Some(linkcode) = extract_folder_code(url) {
        return Some(SourceToScan::Folder {
            linkcode,
            category: category.to_string(),
            label: label.to_string(),
            depth: 0,
            parent_linkcode: String::new(),
            token,
        });
    }

    extract_file_code(url).map(|linkcode| SourceToScan::File {
        linkcode,
        category: category.to_string(),
        label: label.to_string(),
        parent_linkcode: String::new(),
        token,
    })
}

fn extract_token(url: &str) -> String {
    url.split('?')
        .nth(1)
        .and_then(|qs| qs.split('&').find(|p| p.starts_with("token=")))
        .and_then(|p| p.strip_prefix("token="))
        .unwrap_or("")
        .to_string()
}

async fn fetch_file_info(
    client: &reqwest::Client,
    semaphore: &tokio::sync::Semaphore,
    linkcode: &str,
) -> anyhow::Result<FshareXmlItem> {
    let _permit = semaphore.acquire().await?;

    let mut backoff_ms: u64 = 500;
    for attempt in 0..7u32 {
        let resp = client
            .get("https://www.fshare.vn/api/v3/files/folder")
            .query(&[("linkcode", linkcode)])
            .header("Accept", "application/json, text/plain, */*")
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                let parsed: serde_json::Value = r.json().await?;
                if parsed["status"].as_i64() == Some(404) {
                    return Err(anyhow::anyhow!("File not found"));
                }

                let current = &parsed["current"];
                let name = current["name"].as_str().unwrap_or("").to_string();
                if name.is_empty() {
                    return Err(anyhow::anyhow!("Missing file name"));
                }

                return Ok(FshareXmlItem {
                    linkcode: current["linkcode"].as_str().unwrap_or(linkcode).to_string(),
                    name,
                    r#type: current["type"].as_i64().unwrap_or(1).to_string(),
                    size: current["size"].as_i64().unwrap_or(0).to_string(),
                    mimetype: current["mimetype"].as_str().unwrap_or("").to_string(),
                    path: current["path"].as_str().unwrap_or("").to_string(),
                    modified: current["modified"].as_i64(),
                });
            }
            Ok(r) => {
                let code = r.status().as_u16();
                let retryable = matches!(code, 429 | 500 | 502 | 503 | 504);
                if !retryable {
                    return Err(anyhow::anyhow!("HTTP {}", r.status()));
                }
            }
            Err(e) => {
                if attempt >= 6 {
                    return Err(anyhow::anyhow!(e));
                }
            }
        }

        let jitter = backoff_ms / 3;
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as u64;
        let sleep_ms =
            (backoff_ms.saturating_sub(jitter)) + (seed % (jitter.saturating_mul(2) + 1));
        tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;
        backoff_ms = (backoff_ms.saturating_mul(2)).min(30_000);
    }

    Err(anyhow::anyhow!("Retry budget exhausted"))
}

async fn fetch_folder_page(
    client: &reqwest::Client,
    semaphore: &tokio::sync::Semaphore,
    linkcode: &str,
    token: &str,
    page: u32,
) -> anyhow::Result<String> {
    let _permit = semaphore.acquire().await?;

    let page_str = page.to_string();
    let per_page_str = PER_PAGE.to_string();
    let mut query_params: Vec<(&str, &str)> = vec![
        ("linkcode", linkcode),
        ("sort", "type"),
        ("page", &page_str),
        ("per-page", &per_page_str),
    ];

    if !token.is_empty() {
        query_params.push(("token", token));
    }

    let mut backoff_ms: u64 = 500;
    for attempt in 0..7u32 {
        let resp = client
            .get("https://www.fshare.vn/api/v3/files/folder")
            .query(&query_params)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => return Ok(r.text().await?),
            Ok(r) => {
                let code = r.status().as_u16();
                let retryable = matches!(code, 429 | 500 | 502 | 503 | 504);
                if !retryable {
                    return Err(anyhow::anyhow!("HTTP {}", r.status()));
                }
            }
            Err(e) => {
                if attempt >= 6 {
                    return Err(anyhow::anyhow!(e));
                }
            }
        }

        let jitter = backoff_ms / 3;
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as u64;
        let sleep_ms =
            (backoff_ms.saturating_sub(jitter)) + (seed % (jitter.saturating_mul(2) + 1));
        tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;
        backoff_ms = (backoff_ms.saturating_mul(2)).min(30_000);
    }

    Err(anyhow::anyhow!("Retry budget exhausted"))
}

/// Parse Fshare v3 API response — auto-detects JSON or XML format
fn parse_folder_response(body: &str) -> (Vec<FshareXmlItem>, bool) {
    let trimmed = body.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        parse_folder_json(trimmed)
    } else {
        parse_folder_xml(trimmed)
    }
}

/// Parse JSON response from Fshare v3 API
fn parse_folder_json(json_text: &str) -> (Vec<FshareXmlItem>, bool) {
    let parsed: serde_json::Value = match serde_json::from_str(json_text) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("[FOLDER-CACHE] JSON parse error: {}", e);
            return (vec![], false);
        }
    };

    let mut items = Vec::new();

    if let Some(arr) = parsed.get("items").and_then(|v| v.as_array()) {
        for item in arr {
            let linkcode = item
                .get("linkcode")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if linkcode.is_empty() || name.is_empty() {
                continue;
            }

            let modified = item.get("modified").and_then(|v| match v {
                serde_json::Value::Number(n) => n.as_i64(),
                serde_json::Value::String(s) => s.parse().ok(),
                _ => None,
            });

            let item_type = item
                .get("type")
                .map(|v| match v {
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::String(s) => s.clone(),
                    _ => String::new(),
                })
                .unwrap_or_default();

            let size = item
                .get("size")
                .map(|v| match v {
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::String(s) => s.clone(),
                    _ => "0".to_string(),
                })
                .unwrap_or_else(|| "0".to_string());

            let mimetype = item
                .get("mimetype")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let path = item
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            items.push(FshareXmlItem {
                linkcode,
                name,
                r#type: item_type,
                size,
                mimetype,
                path,
                modified,
            });
        }
    }

    // Check for pagination
    let has_next = parsed
        .get("_links")
        .and_then(|links| links.get("next"))
        .map(|next| !next.is_null())
        .unwrap_or(false);

    (items, has_next)
}

/// Parse Fshare v3 XML response (legacy format)
fn parse_folder_xml(xml_text: &str) -> (Vec<FshareXmlItem>, bool) {
    let mut reader = Reader::from_str(xml_text);
    reader.trim_text(true);

    let mut items = Vec::new();
    let mut current_item: Option<FshareXmlItem> = None;
    let mut current_tag = String::new();
    let mut in_items = false;
    let mut in_item = false;
    let mut in_links = false;
    let mut has_next_page = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag_name.as_str() {
                    "items" => in_items = true,
                    "item" if in_items => {
                        in_item = true;
                        current_item = Some(FshareXmlItem::default());
                    }
                    "_links" => in_links = true,
                    "next" if in_links => {
                        has_next_page = true;
                    }
                    _ => {}
                }
                current_tag = tag_name;
            }
            Ok(Event::End(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag_name.as_str() {
                    "items" => in_items = false,
                    "item" if in_items => {
                        if let Some(item) = current_item.take() {
                            if !item.linkcode.is_empty() && !item.name.is_empty() {
                                items.push(item);
                            }
                        }
                        in_item = false;
                    }
                    "_links" => in_links = false,
                    _ => {}
                }
                current_tag.clear();
            }
            Ok(Event::Text(ref e)) => {
                if in_item {
                    if let Some(ref mut item) = current_item {
                        let text = e.unescape().unwrap_or_default().to_string();
                        match current_tag.as_str() {
                            "linkcode" => item.linkcode = text,
                            "name" => item.name = text,
                            "type" => item.r#type = text,
                            "size" => item.size = text,
                            "mimetype" => item.mimetype = text,
                            "path" => item.path = text,
                            "modified" => item.modified = text.parse().ok(),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::error!("[FOLDER-CACHE] XML parse error: {}", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    (items, has_next_page)
}

pub fn expand_folder_cache_query_variants(query: &str) -> Vec<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut variants = Vec::new();
    let mut seen = HashSet::new();
    let push_variant = |value: String, variants: &mut Vec<String>, seen: &mut HashSet<String>| {
        let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
        let key = normalized.to_lowercase();
        if !normalized.is_empty() && seen.insert(key) {
            variants.push(normalized);
        }
    };

    push_variant(trimmed.to_string(), &mut variants, &mut seen);

    let tv_pattern = regex::Regex::new(r"(?i)\bS(\d{1,2})\s*E(\d{1,2})\b").unwrap();
    if let Some(captures) = tv_pattern.captures(trimmed) {
        let season = captures
            .get(1)
            .and_then(|m| m.as_str().parse::<u32>().ok())
            .unwrap_or(0);
        let episode = captures
            .get(2)
            .and_then(|m| m.as_str().parse::<u32>().ok())
            .unwrap_or(0);
        let matched = captures.get(0).map(|m| m.as_str()).unwrap_or("");
        if season > 0 && episode > 0 {
            let replacements = [
                format!("S{:02}E{:02}", season, episode),
                format!("S{}E{}", season, episode),
                format!("S{:02} E{:02}", season, episode),
                format!("S{} E{}", season, episode),
                format!("Season {} Episode {}", season, episode),
                format!("Episode {}", episode),
                format!("E{:02}", episode),
            ];
            for replacement in replacements {
                push_variant(
                    trimmed.replace(matched, &replacement),
                    &mut variants,
                    &mut seen,
                );
            }
        }
    }

    variants
}

// ============================================================================
// Tests — Standalone, no server required
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    /// Create a fresh test DB (temp file, auto-cleaned up)
    fn test_db() -> (Arc<Db>, NamedTempFile) {
        let tmp = NamedTempFile::new().expect("Failed to create temp file");
        let db = Db::new(tmp.path()).expect("Failed to create test DB");
        (Arc::new(db), tmp)
    }

    /// Build a set of realistic cached folder items for testing
    fn sample_items() -> Vec<CachedFolderItem> {
        vec![
            CachedFolderItem {
                linkcode: "VE41H65WVF9M".to_string(),
                name: "Reality Z - Chương Trình Thực Tế Z Season 1_Việt Sub".to_string(),
                title: "Reality Z".to_string(),
                category: "tv".to_string(),
                label: "Phim Bộ Mỹ".to_string(),
                parent_linkcode: "ROOT1".to_string(),
                fshare_url: "https://www.fshare.vn/folder/VE41H65WVF9M".to_string(),
                year: None,
                season: Some(1),
                episode: None,
                is_series: true,
                is_directory: true,
                size: 0,
                quality: "Unknown".to_string(),
                path: "/01_Folder Chính/Phim Bộ Mỹ".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
            CachedFolderItem {
                linkcode: "R53RB1UZ5HMH".to_string(),
                name: "Traces.S01.1080p.AMZN.WEBRip.DDP2.0.x264-Cinefeel".to_string(),
                title: "Traces".to_string(),
                category: "tv".to_string(),
                label: "Phim Bộ Mỹ".to_string(),
                parent_linkcode: "ROOT1".to_string(),
                fshare_url: "https://www.fshare.vn/folder/R53RB1UZ5HMH".to_string(),
                year: None,
                season: Some(1),
                episode: None,
                is_series: true,
                is_directory: true,
                size: 0,
                quality: "WEBRip-1080p".to_string(),
                path: "/01_Folder Chính/Phim Bộ Mỹ".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
            CachedFolderItem {
                linkcode: "BN243UOLOA7U".to_string(),
                name: "Mrs.America.S01.1080p.AMZN.WEBRip.DDP5.1.x264-NTb".to_string(),
                title: "Mrs America".to_string(),
                category: "tv".to_string(),
                label: "Phim Bộ Mỹ".to_string(),
                parent_linkcode: "ROOT1".to_string(),
                fshare_url: "https://www.fshare.vn/folder/BN243UOLOA7U".to_string(),
                year: None,
                season: Some(1),
                episode: None,
                is_series: true,
                is_directory: true,
                size: 0,
                quality: "WEBRip-1080p".to_string(),
                path: "/01_Folder Chính/Phim Bộ Mỹ".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
            CachedFolderItem {
                linkcode: "454TL817M8Z47GC".to_string(),
                name: "Peaky Blinders - Bóng Ma Anh Quốc Season 6".to_string(),
                title: "Peaky Blinders".to_string(),
                category: "tv".to_string(),
                label: "Phim Bộ Mỹ".to_string(),
                parent_linkcode: "ROOT1".to_string(),
                fshare_url: "https://www.fshare.vn/folder/454TL817M8Z47GC".to_string(),
                year: None,
                season: Some(6),
                episode: None,
                is_series: true,
                is_directory: true,
                size: 0,
                quality: "Unknown".to_string(),
                path: "/01_Folder Chính/Phim Bộ Mỹ".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
            CachedFolderItem {
                linkcode: "FILE001ABC".to_string(),
                name: "Inventing.Anna.S01E01.1080p.NF.WEB-DL.DDP5.1.x264.mkv".to_string(),
                title: "Inventing Anna".to_string(),
                category: "tv".to_string(),
                label: "Phim Bộ Mỹ".to_string(),
                parent_linkcode: "FOLDER_ANNA".to_string(),
                fshare_url: "https://www.fshare.vn/file/FILE001ABC".to_string(),
                year: None,
                season: Some(1),
                episode: Some(1),
                is_series: true,
                is_directory: false,
                size: 2_500_000_000,
                quality: "WEB-DL-1080p".to_string(),
                path: "/01_Folder Chính/Phim Bộ Mỹ/Inventing Anna".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
            CachedFolderItem {
                linkcode: "MOVIE123".to_string(),
                name: "Dune Part Two 2024 2160p WEB-DL DDP5.1 Atmos DV HDR x265".to_string(),
                title: "Dune Part Two".to_string(),
                category: "movie".to_string(),
                label: "Phim Lẻ".to_string(),
                parent_linkcode: "ROOT2".to_string(),
                fshare_url: "https://www.fshare.vn/file/MOVIE123".to_string(),
                year: Some(2024),
                season: None,
                episode: None,
                is_series: false,
                is_directory: false,
                size: 15_000_000_000,
                quality: "WEB-DL-2160p".to_string(),
                path: "/02_Phim Lẻ".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
            CachedFolderItem {
                linkcode: "MOVIE456".to_string(),
                name: "Oppenheimer 2023 1080p BluRay x264-SPARKS".to_string(),
                title: "Oppenheimer".to_string(),
                category: "movie".to_string(),
                label: "Phim Lẻ".to_string(),
                parent_linkcode: "ROOT2".to_string(),
                fshare_url: "https://www.fshare.vn/file/MOVIE456".to_string(),
                year: Some(2023),
                season: None,
                episode: None,
                is_series: false,
                is_directory: false,
                size: 10_000_000_000,
                quality: "BluRay-1080p".to_string(),
                path: "/02_Phim Lẻ".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
            CachedFolderItem {
                linkcode: "VIET789".to_string(),
                name: "Vikings Valhalla - Huyền Thoại Vikings Valhalla 2022_Việt Sub".to_string(),
                title: "Vikings Valhalla".to_string(),
                category: "tv".to_string(),
                label: "Phim Bộ Mỹ".to_string(),
                parent_linkcode: "ROOT1".to_string(),
                fshare_url: "https://www.fshare.vn/folder/VIET789".to_string(),
                year: Some(2022),
                season: None,
                episode: None,
                is_series: false,
                is_directory: true,
                size: 0,
                quality: "Unknown".to_string(),
                path: "/01_Folder Chính/Phim Bộ Mỹ".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            },
        ]
    }

    // ── Test: Basic insert and count ──────────────────────────────────

    #[test]
    fn test_insert_and_count() {
        let (db, _tmp) = test_db();
        let items = sample_items();

        let inserted = db.insert_folder_cache_batch(&items).unwrap();
        assert_eq!(inserted, 8, "Should insert all 8 items");

        let count = db.get_folder_cache_count().unwrap();
        assert_eq!(count, 8, "Count should be 8");
    }

    // ── Test: FTS5 search by title ───────────────────────────────────

    #[test]
    fn test_search_by_title() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();

        // Search for "Peaky" — should find Peaky Blinders
        let results = db.search_folder_cache("Peaky", 50).unwrap();
        assert!(!results.is_empty(), "Should find Peaky Blinders");
        assert!(results.iter().any(|r| r.name.contains("Peaky Blinders")));

        println!("\n🔍 Search 'Peaky': {} results", results.len());
        for r in &results {
            println!("  - {} ({})", r.name, r.fshare_url);
        }
    }

    // ── Test: FTS5 search by raw filename ─────────────────────────────

    #[test]
    fn test_search_by_filename() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();

        // Search for "Traces" — matches the scene release name
        let results = db.search_folder_cache("Traces", 50).unwrap();
        assert!(!results.is_empty(), "Should find Traces");
        assert!(results.iter().any(|r| r.linkcode == "R53RB1UZ5HMH"));

        println!("\n🔍 Search 'Traces': {} results", results.len());
        for r in &results {
            println!("  - {} [{}]", r.name, r.quality);
        }
    }

    // ── Test: FTS5 prefix matching ───────────────────────────────────

    #[test]
    fn test_search_prefix_matching() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();

        // Search for "Vik" — prefix should match "Vikings"
        let results = db.search_folder_cache("Vik", 50).unwrap();
        assert!(!results.is_empty(), "Prefix 'Vik' should match Vikings");
        assert!(results.iter().any(|r| r.name.contains("Vikings")));

        println!("\n🔍 Search 'Vik' (prefix): {} results", results.len());
        for r in &results {
            println!("  - {}", r.name);
        }
    }

    // ── Test: Multi-word search ──────────────────────────────────────

    #[test]
    fn test_search_multi_word() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();

        // "Dune Part" should match "Dune Part Two"
        let results = db.search_folder_cache("Dune Part", 50).unwrap();
        assert!(!results.is_empty(), "Should find Dune Part Two");
        assert!(results.iter().any(|r| r.name.contains("Dune Part Two")));

        println!("\n🔍 Search 'Dune Part': {} results", results.len());
        for r in &results {
            println!(
                "  - {} (year: {:?}, quality: {})",
                r.name, r.year, r.quality
            );
        }
    }

    // ── Test: Search by category/label ────────────────────────────────

    #[test]
    fn test_search_by_category() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();

        // Search by label "Phim Lẻ" — should find movies
        let results = db.search_folder_cache("Phim Lẻ", 50).unwrap();
        println!("\n🔍 Search 'Phim Lẻ': {} results", results.len());
        for r in &results {
            println!("  - {} [cat: {}, label: {}]", r.name, r.category, r.label);
        }
        // Should find items with label "Phim Lẻ"
        assert!(results.iter().any(|r| r.label == "Phim Lẻ"));
    }

    // ── Test: Empty query returns nothing ─────────────────────────────

    #[test]
    fn test_search_empty_query() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();

        // Empty string should not match (we'd handle this in the API layer)
        // But FTS5 MATCH with an empty string will error, so the API catches that
        let results = db.search_folder_cache("xyznonexistent", 50).unwrap();
        assert!(results.is_empty(), "Nonexistent query should return empty");
    }

    // ── Test: Clear cache ────────────────────────────────────────────

    #[test]
    fn test_clear_cache() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();
        assert_eq!(db.get_folder_cache_count().unwrap(), 8);

        db.clear_folder_cache().unwrap();
        assert_eq!(
            db.get_folder_cache_count().unwrap(),
            0,
            "Cache should be empty after clear"
        );

        // Search after clear = no results
        let results = db.search_folder_cache("Peaky", 50).unwrap();
        assert!(results.is_empty());
    }

    // ── Test: Metadata operations ────────────────────────────────────

    #[test]
    fn test_metadata() {
        let (db, _tmp) = test_db();

        // Initially no metadata
        assert!(db.get_folder_cache_meta("last_sync").unwrap().is_none());

        // Set and get
        db.set_folder_cache_meta("last_sync", "1709000000").unwrap();
        let val = db.get_folder_cache_meta("last_sync").unwrap();
        assert_eq!(val, Some("1709000000".to_string()));

        // Update
        db.set_folder_cache_meta("last_sync", "1709100000").unwrap();
        let val = db.get_folder_cache_meta("last_sync").unwrap();
        assert_eq!(val, Some("1709100000".to_string()));
    }

    // ── Test: Search result data integrity ────────────────────────────

    #[test]
    fn test_search_result_fields() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();

        let results = db.search_folder_cache("Oppenheimer", 50).unwrap();
        assert_eq!(results.len(), 1);

        let r = &results[0];
        assert_eq!(r.linkcode, "MOVIE456");
        assert_eq!(r.title, "Oppenheimer");
        assert_eq!(r.category, "movie");
        assert_eq!(r.year, Some(2023));
        assert!(!r.is_directory);
        assert!(!r.is_series);
        assert_eq!(r.quality, "BluRay-1080p");
        assert_eq!(r.size, 10_000_000_000);
        assert_eq!(r.fshare_url, "https://www.fshare.vn/file/MOVIE456");

        println!("\n✅ Oppenheimer data integrity verified:");
        println!("  linkcode: {}", r.linkcode);
        println!("  year: {:?}", r.year);
        println!("  quality: {}", r.quality);
        println!("  size: {} GB", r.size / 1_000_000_000);
    }

    // ── Test: Batch insert large dataset ──────────────────────────────

    #[test]
    fn test_large_batch_insert() {
        let (db, _tmp) = test_db();

        // Create 1000 items
        let mut items = Vec::new();
        for i in 0..1000 {
            items.push(CachedFolderItem {
                linkcode: format!("CODE{:04}", i),
                name: format!("Movie.{}.2024.1080p.BluRay.x264-GROUP", i),
                title: format!("Movie {}", i),
                category: "movie".to_string(),
                label: "Test".to_string(),
                parent_linkcode: "ROOT".to_string(),
                fshare_url: format!("https://www.fshare.vn/file/CODE{:04}", i),
                year: Some(2024),
                season: None,
                episode: None,
                is_series: false,
                is_directory: false,
                size: 5_000_000_000,
                quality: "BluRay-1080p".to_string(),
                path: "/test".to_string(),
                tmdb_id: None,
                media_type_hint: None,
                poster_path: None,
            });
        }

        let start = std::time::Instant::now();
        let inserted = db.insert_folder_cache_batch(&items).unwrap();
        let insert_time = start.elapsed();

        assert_eq!(inserted, 1000);
        assert_eq!(db.get_folder_cache_count().unwrap(), 1000);

        // Now search
        let start = std::time::Instant::now();
        let results = db.search_folder_cache("Movie 500", 10).unwrap();
        let search_time = start.elapsed();

        println!("\n⚡ Performance (1000 items):");
        println!("  Insert: {:.2}ms", insert_time.as_secs_f64() * 1000.0);
        println!("  Search: {:.3}ms", search_time.as_secs_f64() * 1000.0);
        println!("  Results: {}", results.len());
        for r in &results {
            println!("    - {}", r.name);
        }

        assert!(!results.is_empty(), "Should find results for 'Movie 500'");
    }

    // ── Test: XML parsing ────────────────────────────────────────────

    #[test]
    fn test_xml_parsing() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<response>
    <items>
        <item>
            <linkcode>ABC123</linkcode>
            <name>Test Movie 2024</name>
            <type>1</type>
            <size>5000000000</size>
            <mimetype>video/x-matroska</mimetype>
            <path>/movies</path>
        </item>
        <item>
            <linkcode>DEF456</linkcode>
            <name>TV Show Season 1</name>
            <type>0</type>
            <size>0</size>
            <mimetype></mimetype>
            <path>/shows</path>
        </item>
    </items>
    <_links>
        <next>/v3/files/folder?page=2</next>
    </_links>
</response>"#;

        let (items, has_next) = parse_folder_xml(xml);

        assert_eq!(items.len(), 2);
        assert!(has_next);

        assert_eq!(items[0].linkcode, "ABC123");
        assert_eq!(items[0].name, "Test Movie 2024");
        assert_eq!(items[0].r#type, "1"); // file
        assert_eq!(items[0].size, "5000000000");

        assert_eq!(items[1].linkcode, "DEF456");
        assert_eq!(items[1].r#type, "0"); // folder
    }

    // ── Test: extract_folder_code ─────────────────────────────────────

    #[test]
    fn test_extract_folder_code() {
        assert_eq!(
            extract_folder_code("https://www.fshare.vn/folder/ABC123?token=xyz"),
            Some("ABC123".to_string())
        );
        assert_eq!(
            extract_folder_code("https://www.fshare.vn/folder/XEVZ47FBZSR4"),
            Some("XEVZ47FBZSR4".to_string())
        );
        assert_eq!(extract_folder_code("https://fshare.vn/file/ABC"), None);
        assert_eq!(extract_folder_code("garbage"), None);
    }

    #[test]
    fn test_validate_refresh_folder_url() {
        assert!(validate_refresh_folder_url("https://www.fshare.vn/folder/ABC123?token=x").is_ok());
        for url in [
            "",
            "http://www.fshare.vn/folder/ABC123",
            "https://example.com/folder/ABC123",
            "https://www.fshare.vn/file/ABC123",
            "https://www.fshare.vn/folder/",
        ] {
            assert!(validate_refresh_folder_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn normalize_source_list_accepts_gist_format_and_deduplicates() {
        let sources = normalize_source_list(
            "# folders\n\nhttps://fshare.vn/folder/ABC123\nhttps://www.fshare.vn/folder/ABC123\nhttps://www.fshare.vn/file/XYZ789?token=private",
        )
        .unwrap();
        assert_eq!(
            sources,
            vec![
                "https://www.fshare.vn/folder/ABC123",
                "https://www.fshare.vn/file/XYZ789?token=private",
            ]
        );
    }

    #[test]
    fn optional_local_source_list_allows_remote_only_sync() {
        assert!(normalize_optional_source_list("").unwrap().is_empty());
        assert!(
            normalize_optional_source_list("# uploaded list cleared\n\n")
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            normalize_optional_source_list("https://www.fshare.vn/folder/ABC123").unwrap(),
            vec!["https://www.fshare.vn/folder/ABC123"]
        );
    }

    #[test]
    fn normalize_source_list_rejects_untrusted_urls() {
        for source in [
            "http://www.fshare.vn/folder/ABC123",
            "https://example.com/folder/ABC123",
            "https://www.fshare.vn/folder/ABC123?redirect=https://evil.test",
            "https://www.fshare.vn/folder/ABC123/extra",
        ] {
            assert!(normalize_source_list(source).is_err(), "{source}");
        }
    }

    #[test]
    fn folder_cache_counts_split_directories_and_files() {
        let (db, _tmp) = test_db();
        db.insert_folder_cache_batch(&sample_items()).unwrap();
        assert_eq!(db.get_folder_cache_counts().unwrap(), (5, 3));
    }

    #[test]
    fn test_replace_folder_cache_subtree_preserves_unrelated_items() {
        let (db, _tmp) = test_db();
        let mut old_root = sample_items()[0].clone();
        old_root.linkcode = "OLD_ROOT_FILE".to_string();
        old_root.parent_linkcode = "HOT_FOLDER".to_string();
        old_root.path = "/tv/hot".to_string();
        let mut old_season = sample_items()[1].clone();
        old_season.linkcode = "SEASON_ONE".to_string();
        old_season.parent_linkcode = "HOT_FOLDER".to_string();
        old_season.path = "/tv/hot".to_string();
        let mut old_nested = sample_items()[4].clone();
        old_nested.linkcode = "OLD_NESTED_FILE".to_string();
        old_nested.parent_linkcode = "SEASON_ONE".to_string();
        old_nested.path = "/tv/hot/season-1".to_string();
        let mut unrelated = sample_items()[2].clone();
        unrelated.linkcode = "UNRELATED_FILE".to_string();
        unrelated.parent_linkcode = "OTHER_FOLDER".to_string();
        unrelated.path = "/tv/other".to_string();
        db.replace_folder_cache(&[old_root, old_season, old_nested, unrelated])
            .unwrap();
        db.set_folder_tmdb_mapping("OLD_NESTED_FILE", 1, "tv", "")
            .unwrap();
        db.set_folder_tmdb_mapping("UNRELATED_FILE", 2, "tv", "")
            .unwrap();

        let mut replacement = sample_items()[3].clone();
        replacement.linkcode = "NEW_HOT_FILE".to_string();
        replacement.parent_linkcode = "HOT_FOLDER".to_string();
        replacement.path = "/tv/hot".to_string();
        db.replace_folder_cache_subtree("HOT_FOLDER", &[replacement], 7)
            .unwrap();

        assert_eq!(db.get_folder_cache_count().unwrap(), 2);
        assert!(db
            .search_folder_cache("NEW HOT", 10)
            .unwrap()
            .iter()
            .any(|item| item.linkcode == "NEW_HOT_FILE"));
        assert!(db.search_folder_cache("OLD NESTED", 10).unwrap().is_empty());
        assert!(db
            .search_folder_cache("UNRELATED", 10)
            .unwrap()
            .iter()
            .any(|item| item.linkcode == "UNRELATED_FILE"));
        let mappings = db.get_all_folder_tmdb_mappings().unwrap();
        assert!(!mappings.contains_key("OLD_NESTED_FILE"));
        assert!(mappings.contains_key("UNRELATED_FILE"));
        assert!(db
            .folder_cache_descendants("HOT_FOLDER")
            .unwrap()
            .iter()
            .any(|item| item.linkcode == "NEW_HOT_FILE"));
        assert!(db
            .folder_cache_descendants("OTHER_FOLDER")
            .unwrap()
            .iter()
            .any(|item| item.linkcode == "UNRELATED_FILE"));
    }

    #[test]
    fn test_replace_folder_cache_swaps_complete_snapshot() {
        let (db, _tmp) = test_db();
        let original = sample_items();
        db.insert_folder_cache_batch(&original).unwrap();

        let replacement = vec![original[0].clone()];
        assert_eq!(db.replace_folder_cache(&replacement).unwrap(), 1);
        assert_eq!(db.get_folder_cache_count().unwrap(), 1);
        assert!(db.search_folder_cache("Reality", 10).unwrap().len() == 1);
        assert!(db.search_folder_cache("Peaky", 10).unwrap().is_empty());
    }

    #[test]
    fn test_tmdb_negative_cache_keeps_recent_miss() {
        let (db, _tmp) = test_db();
        let key = tmdb_negative_cache_key("movie", "Missing Movie", Some(2024));
        db.set_folder_tmdb_negative_keys(std::slice::from_ref(&key))
            .unwrap();

        let active = db
            .get_folder_tmdb_negative_keys(
                chrono::Utc::now().timestamp() - TMDB_NEGATIVE_CACHE_TTL_SECS,
            )
            .unwrap();
        assert!(active.contains(&key));
    }

    #[test]
    fn test_compute_folder_diff() {
        let mut previous = HashMap::new();
        previous.insert(
            "OLD".to_string(),
            FolderCacheSnapshotItem {
                name: "Old File.mkv".to_string(),
                parent_linkcode: "P1".to_string(),
                size: 100,
                path: "/old".to_string(),
                is_directory: false,
            },
        );
        previous.insert(
            "CHANGED".to_string(),
            FolderCacheSnapshotItem {
                name: "Episode 2.mkv".to_string(),
                parent_linkcode: "P1".to_string(),
                size: 200,
                path: "/show".to_string(),
                is_directory: false,
            },
        );

        let mut current = HashMap::new();
        current.insert(
            "CHANGED".to_string(),
            FolderCacheSnapshotItem {
                name: "Episode 2.mkv".to_string(),
                parent_linkcode: "P1".to_string(),
                size: 300,
                path: "/show".to_string(),
                is_directory: false,
            },
        );
        current.insert(
            "NEW".to_string(),
            FolderCacheSnapshotItem {
                name: "New File.mkv".to_string(),
                parent_linkcode: "P2".to_string(),
                size: 400,
                path: "/new".to_string(),
                is_directory: false,
            },
        );

        let diff = compute_folder_diff(&previous, &current);
        assert_eq!(diff.added, 1);
        assert_eq!(diff.removed, 1);
        assert_eq!(diff.changed, 1);
        assert!(!diff.is_empty());

        assert!(compute_folder_diff(&current, &current).is_empty());
    }

    #[test]
    fn test_expand_folder_cache_query_variants() {
        let variants = expand_folder_cache_query_variants("zomvivor 2025 S01E02");
        assert!(variants.iter().any(|v| v == "zomvivor 2025 S01E02"));
        assert!(variants.iter().any(|v| v == "zomvivor 2025 S01 E02"));
        assert!(variants.iter().any(|v| v == "zomvivor 2025 S1E2"));
        assert!(variants.iter().any(|v| v == "zomvivor 2025 S1 E2"));
        assert!(variants
            .iter()
            .any(|v| v == "zomvivor 2025 Season 1 Episode 2"));
        assert!(variants.iter().any(|v| v == "zomvivor 2025 Episode 2"));
        assert!(variants.iter().any(|v| v == "zomvivor 2025 E02"));
    }

    // ══════════════════════════════════════════════════════════════════
    // Live Integration Test — fetches REAL data from Fshare
    // Run with: cargo test live_folder_scan -- --ignored --nocapture
    // ══════════════════════════════════════════════════════════════════

    #[tokio::test]
    #[ignore] // Requires network access — run explicitly
    async fn live_folder_scan_and_search() {
        println!("\n╔══════════════════════════════════════════════════════════╗");
        println!("║      Live Folder Cache Test (Real Fshare Folders)       ║");
        println!("╚══════════════════════════════════════════════════════════╝\n");

        // Source entries — real Fshare folders
        let entries = vec![
            (
                "tv",
                "Phim Bộ Mỹ",
                "https://www.fshare.vn/folder/XEVZ47FBZSR4?token=1772074278",
            ),
            (
                "movie",
                "Phim Lẻ",
                "https://www.fshare.vn/folder/QOV7Z7MDMTXY?token=1772075966",
            ),
        ];

        // Create temp DB
        let tmp = NamedTempFile::new().unwrap();
        let db = Arc::new(Db::new(tmp.path()).unwrap());

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();
        let semaphore = tokio::sync::Semaphore::new(6);

        let mut total_items = 0usize;
        let mut total_folders = 0usize;
        let start = std::time::Instant::now();

        for (category, label, url) in &entries {
            let folder_code = extract_folder_code(url).unwrap();
            let token = url
                .split('?')
                .nth(1)
                .and_then(|qs| qs.split('&').find(|p| p.starts_with("token=")))
                .and_then(|p| p.strip_prefix("token="))
                .unwrap_or("")
                .to_string();

            println!("📂 Scanning '{}' (code: {})...", label, folder_code);

            // BFS scan — depth 0 only for speed (just top-level items)
            let mut queue = VecDeque::new();
            queue.push_back((folder_code.clone(), 0u32));

            let mut batch: Vec<CachedFolderItem> = Vec::new();

            while let Some((linkcode, depth)) = queue.pop_front() {
                if depth > 1 {
                    continue;
                } // Only top level + 1 deep for test speed
                total_folders += 1;
                let mut page = 1;

                loop {
                    let body = match fetch_folder_page(&client, &semaphore, &linkcode, &token, page)
                        .await
                    {
                        Ok(b) => b,
                        Err(e) => {
                            eprintln!("   ⚠️  API error for {} page {}: {}", linkcode, page, e);
                            break;
                        }
                    };

                    let (items, has_next) = parse_folder_response(&body);
                    if items.is_empty() {
                        break;
                    }

                    println!(
                        "   📄 Page {} (depth {}): {} items",
                        page,
                        depth,
                        items.len()
                    );

                    for item in &items {
                        let is_dir = item.r#type == "0" || item.mimetype.is_empty();
                        let size: u64 = item.size.parse().unwrap_or(0);
                        let fshare_url = if is_dir {
                            format!("https://www.fshare.vn/folder/{}", item.linkcode)
                        } else {
                            format!("https://www.fshare.vn/file/{}", item.linkcode)
                        };
                        let parsed = FilenameParser::parse(&item.name);

                        batch.push(CachedFolderItem {
                            linkcode: item.linkcode.clone(),
                            name: item.name.clone(),
                            title: parsed.title,
                            category: category.to_string(),
                            label: label.to_string(),
                            parent_linkcode: linkcode.clone(),
                            fshare_url,
                            year: parsed.year,
                            season: parsed.season,
                            episode: parsed.episode,
                            is_series: parsed.is_series,
                            is_directory: is_dir,
                            size,
                            quality: parsed.quality_attrs.quality_name(),
                            path: item.path.clone(),
                            tmdb_id: None,
                            media_type_hint: None,
                            poster_path: None,
                        });

                        if is_dir && depth < 1 {
                            queue.push_back((item.linkcode.clone(), depth + 1));
                        }
                    }

                    if !has_next || items.len() < PER_PAGE {
                        break;
                    }
                    page += 1;
                    if page > 10 {
                        break;
                    } // Safety limit for test

                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }

            if !batch.is_empty() {
                let count = db.insert_folder_cache_batch(&batch).unwrap();
                total_items += count;
                println!("   ✅ Inserted {} items from '{}'\n", count, label);
            }
        }

        let scan_time = start.elapsed();
        println!("═══════════════════════════════════════════════════════");
        println!("📊 Scan Summary:");
        println!("   Total items:   {}", total_items);
        println!("   Total folders: {}", total_folders);
        println!("   Duration:      {:.1}s", scan_time.as_secs_f64());
        println!("   Cache count:   {}", db.get_folder_cache_count().unwrap());
        println!("═══════════════════════════════════════════════════════\n");

        // ── Search tests ─────────────────────────────────────────────

        let search_queries = vec![
            "Vĩnh Dạ Tinh Hà",
            "Loki",
            "Peaky",
            "Vikings",
            "Season",
            "1080p",
            "Dune",
        ];

        for query in &search_queries {
            println!("🔍 Search: \"{}\"", query);
            let search_start = std::time::Instant::now();
            let results = db.search_folder_cache(query, 10).unwrap();
            let search_ms = search_start.elapsed().as_secs_f64() * 1000.0;

            if results.is_empty() {
                println!("   (no results)  [{:.3}ms]\n", search_ms);
            } else {
                println!("   {} results  [{:.3}ms]", results.len(), search_ms);
                for (i, r) in results.iter().enumerate() {
                    let size_str = if r.size > 0 {
                        format!("{:.1}GB", r.size as f64 / 1_000_000_000.0)
                    } else {
                        "📁 dir".to_string()
                    };
                    println!("   {}. {} [{}] [{}]", i + 1, r.name, r.quality, size_str);
                    println!("      → {}", r.fshare_url);
                }
                println!();
            }
        }

        // Assertions — cache should have items
        assert!(
            total_items > 0,
            "Should have scanned some items from Fshare"
        );
        println!("✅ Live integration test complete!");
    }
}
