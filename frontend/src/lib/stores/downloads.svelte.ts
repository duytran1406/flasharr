import { wsClient } from './websocket';
import { toasts } from './toasts';
import { fetchLibraryOverview } from './arr';
import {
  removeFromBatchItemCache,
  upsertBatchItemCache,
} from '$lib/components/downloads/mediaQueue';
import type { UnifiedTorrentRow } from './qbitReducer';

/**
 * Download State (backend DownloadState)
 */
export type DownloadState =
  | 'QUEUED'
  | 'STARTING'
  | 'DOWNLOADING'
  | 'PAUSED'
  | 'WAITING'
  | 'COMPLETED'
  | 'FAILED'
  | 'CANCELLED'
  | 'EXTRACTING'
  | 'SKIPPED'
  | 'IMPORTING';

/**
 * Status priority for urgency-based sorting (lower = higher priority).
 *
 * NOTE: in the 2026-09-02 build an identical table is compiled into the
 * downloads page node, not into this module's chunk — it was most likely
 * moved to a downloads-page helper. Kept here for compatibility.
 */
export const STATUS_PRIORITY: Record<DownloadState, number> = {
  FAILED: 1,
  CANCELLED: 2,
  DOWNLOADING: 3,
  EXTRACTING: 4,
  IMPORTING: 4.5,
  STARTING: 5,
  WAITING: 6,
  PAUSED: 7,
  QUEUED: 8,
  SKIPPED: 9,
  COMPLETED: 10,
};

/**
 * Download task as serialized by the backend (downloader/task.rs DownloadTask).
 * url/original_url/destination are no longer serialized (skip_serializing)
 * and are kept optional for older callers.
 */
export interface DownloadTask {
  id: string;
  url?: string;
  original_url?: string;
  destination?: string;
  filename: string;
  state: DownloadState;
  progress: number;
  size: number;
  downloaded?: number;
  speed?: number;
  eta?: number;
  host: string;
  category: string;
  priority: number;
  segments: number;
  retry_count: number;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
  wait_until: string | null;
  error_message: string | null;

  // Source / provider
  fshare_code?: string | null;
  provider_id?: string | null;
  provider_item_id?: string | null;
  /** fshare | torrent | debrid | qbittorrent | flasharr-http ... */
  download_source?: string | null;
  external_provider?: string | null;
  external_id?: string | null;
  external_instance_id?: string | null;
  info_hash?: string | null;
  file_idx?: number | null;
  source_filename?: string | null;
  selected_episodes?: string | null;
  execution_strategy?: string | null;
  torbox_remote_state?: string | null;
  origin_provider?: string | null;
  origin_indexer?: string | null;
  seeders?: number | null;
  leechers?: number | null;

  // Torrent transfer details (managed torrents)
  task_state?: string | null;
  qbit_state?: string | null;
  sync_health?: string | null;
  last_sync_at?: string | null;
  transfer_state?: string | null;
  transfer_active?: boolean;

  /** Batch ID for grouping related downloads (e.g., TV season episodes) */
  batch_id?: string | null;
  /** Batch display name (e.g., "Breaking Bad S01") */
  batch_name?: string | null;

  // Media identity (TV: tvdb_id canonical; movie: tmdb_id)
  tvdb_id?: number | null;
  tmdb_id?: number | null;
  media_type?: string | null;
  media_item_id?: number | null;
  tmdb_title?: string | null;
  tmdb_season?: number | null;
  tmdb_episode?: number | null;
  /** Detail route resolved by the backend (`/tv/:id` or `/movie/:id`) */
  info_path?: string | null;

  /** Quality description (e.g., "WEB-DL-1080p") */
  quality?: string | null;
  /** Resolution shorthand (e.g., "1080p", "2160p") */
  resolution?: string | null;

  arr_series_id?: number | null;
  arr_movie_id?: number | null;
  arr_announced?: boolean;
  arr_announce_error?: string | null;
}

/**
 * Batch summary for batch-first pagination
 * Represents a batch as a single row in the downloads table
 */
export interface BatchSummary {
  batch_id: string;
  batch_name: string;
  total_items: number;
  completed_items: number;
  failed_items: number;
  downloading_items: number;
  paused_items: number;
  queued_items: number;
  total_size: number;
  downloaded_size: number;
  progress: number;
  speed: number;
  created_at: string;
  /** Aggregate source of the batch (backend summary) */
  source: string;
  state: DownloadState;
}

/**
 * Authoritative row order from /api/downloads/batches.
 * kind `download` → task id, `batch` → batch_id, `torrent` → `instance:hash`.
 */
export interface DisplayRowRef {
  kind: 'download' | 'batch' | 'torrent' | (string & {});
  id: string;
}

/** Unmanaged qBittorrent torrent row shown in the downloads queue. */
export type DownloadTorrentRow = UnifiedTorrentRow;

/**
 * Database-sourced status counts (for filter dropdown)
 */
export interface DbStatusCounts {
  all: number;
  downloading: number;
  queued: number;
  paused: number;
  completed: number;
  failed: number;
  cancelled: number;
}

/**
 * Engine Statistics
 */
export interface EngineStats {
  active_downloads: number;
  queued: number;
  completed: number;
  failed: number;
  paused: number;
  cancelled: number;
  total_speed: number;
  /** Database-sourced status counts (for filter dropdown) */
  db_counts?: DbStatusCounts;
}

/**
 * TMDB metadata for organized folder structure (backend TmdbMetadata).
 */
export interface TmdbMetadata {
  /** Movie: canonical. TV: legacy bridge only. */
  tmdb_id?: number | null;
  /** TV: canonical identity for Sonarr. */
  tvdb_id?: number | null;
  media_type?: 'movie' | 'tv';
  title?: string | null;
  year?: string | number | null;
  collection_name?: string;
  season?: number | null;
  episode?: number | null;
}

/**
 * Add Download Request (backend AddDownloadRequest).
 * Either `url` or `download_ref` must be present.
 */
export interface AddDownloadRequest {
  url?: string;
  download_ref?: string;
  filename?: string;
  category?: string;
  priority?: string;
  /** TMDB metadata for folder organization */
  tmdb?: TmdbMetadata;
  /** Batch ID for grouping downloads from the same request */
  batch_id?: string;
  /** Batch name for display purposes */
  batch_name?: string;
  /** fshare | torrent | debrid */
  source?: string;
  provider?: string;
  size?: number;
  seeders?: number;
  leechers?: number;
  info_hash?: string;
  file_idx?: number;
  source_filename?: string;
  external_id?: string;
  origin_provider?: string;
  origin_indexer?: string;
  qbit_instance_id?: string;
  /** Re-download an existing completed task with the same Fshare code. */
  force_redownload?: boolean;
  /** torbox_http | local_qb (debrid) */
  strategy?: string;
  selected_episodes?: string;
}

/** Options for downloadStore.addDownload(). */
export interface AddDownloadOptions {
  /** Show success/error toasts (default true). */
  notify?: boolean;
}

/**
 * Result wrapper returned by every store action.
 */
export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

interface DownloadsResponse {
  downloads: DownloadTask[];
  stats: EngineStats;
  status_counts: StatusCounts;
  total: number;
  page: number;
  limit: number;
  total_pages: number;
}

/** Response from /api/downloads/batches */
interface BatchSummariesResponse {
  batches: BatchSummary[];
  standalone: DownloadTask[];
  rows: DisplayRowRef[];
  /** qB torrents not tracked as Flasharr download tasks */
  torrents?: UnifiedTorrentRow[];
  stats: EngineStats;
  status_counts: StatusCounts;
  total: number; // Total display units
  page: number;
  limit: number;
  total_pages: number;
}

export interface AddDownloadResponse {
  success: boolean;
  task_id: string;
  filename: string;
  state: string;
}

export interface AffectedResponse {
  affected: number;
}

/**
 * Pagination state
 */
export interface PaginationState {
  page: number;
  limit: number;
  total: number;
  totalPages: number;
  sortBy: 'added' | 'status' | 'filename' | 'size' | 'progress';
  sortDir: 'asc' | 'desc';
  /** Optional status filter for server-side filtering */
  statusFilter?: DownloadState | null;
  /** Server-side source filter (`direct`, `fshare`, `torrent`, `torbox`); null = all. Default `direct`. */
  sourceFilter?: string | null;
}

/**
 * Status counts from database (for filter dropdown)
 */
export interface StatusCounts {
  downloading: number;
  queued: number;
  paused: number;
  completed: number;
  failed: number;
  cancelled: number;
  all: number;
}

/** Download store events re-broadcast for UI (onDownloadEvent). */
export type DownloadEventType = 'TASK_UPDATED' | 'TASK_STARTED' | 'TASK_COMPLETED';

const API_BASE = '/api';

/**
 * DownloadStoreState — snapshot shape exposed by the Svelte 4 subscribe compat method.
 */
export interface DownloadStoreState {
  downloads: Map<string, DownloadTask>;
  batches: Map<string, BatchSummary>;
  /** Expanded batch items keyed `"<batch_id>|<sourceFilter>"` */
  batchItems: Map<string, DownloadTask[]>;
  /** Unmanaged torrents keyed `"<instance_id>:<hash>"` */
  torrentRows: Map<string, UnifiedTorrentRow>;
  displayRows: DisplayRowRef[];
  stats: EngineStats;
  statusCounts: StatusCounts;
  pagination: PaginationState;
  loading: boolean;
  error: string | null;
}

/** Map a raw qBittorrent state + progress (0..1) to a Flasharr state. */
function qbitStateToDownloadState(state: string | undefined | null, progress: number): DownloadState {
  const s = (state ?? '').toLowerCase();
  if (progress >= 1 && (s.includes('up') || s.includes('paused'))) return 'COMPLETED';
  if (s.includes('pause')) return 'PAUSED';
  if (s.includes('error') || s.includes('missing')) return 'FAILED';
  if (s.includes('check') || s.includes('meta')) return 'STARTING';
  if (s.includes('queue')) return 'QUEUED';
  return 'DOWNLOADING';
}

/** Build a torrent row from a raw QBIT_* websocket torrent. */
function toTorrentRow(instanceId: string, t: any): UnifiedTorrentRow {
  return {
    instance_id: instanceId,
    hash: t.hash ?? '',
    name: t.name ?? '',
    state: t.state ?? '',
    flasharr_state: qbitStateToDownloadState(t.state, t.progress),
    progress: t.progress ?? 0,
    size: t.size ?? 0,
    downloaded: t.downloaded ?? 0,
    uploaded: t.uploaded ?? 0,
    dlspeed: t.dlspeed ?? 0,
    upspeed: t.upspeed ?? 0,
    eta: t.eta ?? 0,
    ratio: t.ratio ?? 0,
    category: t.category ?? '',
    tags: t.tags ?? '',
    tracker: t.tracker ?? '',
    added_on: t.added_on ?? 0,
    completion_on: t.completion_on ?? 0,
    amount_left: t.amount_left ?? 0,
    num_seeds: t.num_seeds ?? 0,
    num_leechs: t.num_leechs ?? 0,
    updated_at: t.updated_at ?? new Date().toISOString(),
  };
}

function apiHeaders(withJsonBody = false): HeadersInit {
  return withJsonBody ? { 'Content-Type': 'application/json' } : {};
}

/** Error text from a failed response: JSON `error`/`message`, raw text, or `HTTP <status>`. */
async function responseErrorMessage(response: Response): Promise<string> {
  const text = await response.text();
  if (!text) return `HTTP ${response.status}`;
  try {
    const data = JSON.parse(text);
    return data.error || data.message || `HTTP ${response.status}`;
  } catch {
    return text || `HTTP ${response.status}`;
  }
}

/** True when a Flasharr task already tracks this torrent hash. */
function isTrackedTorrent(downloads: Map<string, DownloadTask>, hash: string | undefined): boolean {
  const needle = hash?.toLowerCase();
  return Array.from(downloads.values()).some(
    (d) =>
      (d.info_hash && d.info_hash.toLowerCase() === needle) ||
      (d.external_id && d.external_id.toLowerCase() === needle),
  );
}

/** All cached items of one batch across source-filter cache keys. */
function cachedBatchItems(batchItems: Map<string, DownloadTask[]>, batchId: string): DownloadTask[] {
  return [...batchItems].filter(([key]) => key.split('|', 1)[0] === batchId).flatMap(([, items]) => items);
}

/**
 * DownloadStore — Svelte 5 runes class
 */
class DownloadStore {
  // ---------------------------------------------------------------------------
  // Reactive state
  // ---------------------------------------------------------------------------
  downloads = $state<Map<string, DownloadTask>>(new Map());
  /** Batch summaries for batch-first pagination */
  batches = $state<Map<string, BatchSummary>>(new Map());
  /** Cache of expanded batch items, keyed `"<batch_id>|<sourceFilter>"` */
  batchItems = $state<Map<string, DownloadTask[]>>(new Map());
  /** Unmanaged qBittorrent torrents, keyed `"<instance_id>:<hash>"` */
  torrentRows = $state<Map<string, UnifiedTorrentRow>>(new Map());
  /** Backend row order of the current page */
  displayRows = $state<DisplayRowRef[]>([]);
  stats = $state<EngineStats>({
    active_downloads: 0,
    queued: 0,
    completed: 0,
    failed: 0,
    paused: 0,
    cancelled: 0,
    total_speed: 0,
  });
  statusCounts = $state<StatusCounts>({
    downloading: 0,
    queued: 0,
    paused: 0,
    completed: 0,
    failed: 0,
    cancelled: 0,
    all: 0,
  });
  pagination = $state<PaginationState>({
    page: 1,
    limit: 15,
    total: 0,
    totalPages: 0,
    sortBy: 'added',
    sortDir: 'desc',
    statusFilter: null,
    sourceFilter: 'direct',
  });
  loading = $state<boolean>(false);
  error = $state<string | null>(null);

  // Private notification system for legacy store subscribers
  private _subscribers = new Set<(v: DownloadStoreState) => void>();

  private snapshot(): DownloadStoreState {
    return {
      downloads: this.downloads,
      batches: this.batches,
      batchItems: this.batchItems,
      torrentRows: this.torrentRows,
      displayRows: this.displayRows,
      stats: this.stats,
      statusCounts: this.statusCounts,
      pagination: this.pagination,
      loading: this.loading,
      error: this.error,
    };
  }

  private _notify() {
    if (this._subscribers.size === 0) return;
    const snapshot = this.snapshot();
    this._subscribers.forEach((fn) => fn(snapshot));
  }

  // ---------------------------------------------------------------------------
  // Derived values
  // ---------------------------------------------------------------------------

  /** Downloads as array (sorted by created_at desc) */
  downloadList: DownloadTask[] = $derived.by(() =>
    Array.from(this.downloads.values()).sort(
      (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime(),
    ),
  );

  /** Batches as array (sorted by created_at desc) */
  batchList: BatchSummary[] = $derived.by(() =>
    Array.from(this.batches.values()).sort(
      (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime(),
    ),
  );

  /** Unmanaged torrents (newest first) */
  torrentList: UnifiedTorrentRow[] = $derived.by(() =>
    Array.from(this.torrentRows.values()).sort((a, b) => b.added_on - a.added_on),
  );

  /** Active downloads */
  activeDownloadsList: DownloadTask[] = $derived.by(() =>
    this.downloadList.filter((d) => d.state === 'DOWNLOADING' || d.state === 'STARTING' || d.state === 'IMPORTING'),
  );

  /** Queued downloads */
  queuedDownloadsList: DownloadTask[] = $derived.by(() => this.downloadList.filter((d) => d.state === 'QUEUED'));

  /** Completed downloads */
  completedDownloadsList: DownloadTask[] = $derived.by(() =>
    this.downloadList.filter((d) => d.state === 'COMPLETED'),
  );

  /** Failed downloads */
  failedDownloadsList: DownloadTask[] = $derived.by(() => this.downloadList.filter((d) => d.state === 'FAILED'));

  /** Paused downloads */
  pausedDownloadsList: DownloadTask[] = $derived.by(() => this.downloadList.filter((d) => d.state === 'PAUSED'));

  // ---------------------------------------------------------------------------
  // Private helpers
  // ---------------------------------------------------------------------------

  /**
   * Clear error_message when a task enters a working/active state.
   */
  private clearErrorIfActive(task: DownloadTask): DownloadTask {
    const activeStates: DownloadState[] = ['QUEUED', 'STARTING', 'DOWNLOADING', 'EXTRACTING', 'WAITING', 'IMPORTING'];
    return activeStates.includes(task.state) ? { ...task, error_message: null } : task;
  }

  private dedupeTasks(tasks: DownloadTask[]): DownloadTask[] {
    return Array.from(new Map(tasks.map((t) => [t.id, t])).values());
  }

  private dedupeRows(rows: DisplayRowRef[] = []): DisplayRowRef[] {
    const seen = new Set<string>();
    return rows.filter((row) => (seen.has(row.id) ? false : (seen.add(row.id), true)));
  }

  /**
   * Route an incoming task into the caches: batch members go to batchItems
   * (and are removed from standalone downloads), others are merged into
   * downloads. Returns new Maps.
   */
  private applyTaskToCaches(
    task: DownloadTask,
    downloads: Map<string, DownloadTask>,
    batchItems: Map<string, DownloadTask[]>,
  ): { downloads: Map<string, DownloadTask>; batchItems: Map<string, DownloadTask[]> } {
    const incoming = this.clearErrorIfActive(task);
    const nextDownloads = new Map(downloads);
    const nextBatchItems = new Map(batchItems);
    if (incoming.batch_id) {
      nextDownloads.delete(incoming.id);
      return { downloads: nextDownloads, batchItems: upsertBatchItemCache(nextBatchItems, incoming) };
    }
    const existing = nextDownloads.get(incoming.id);
    nextDownloads.set(incoming.id, existing ? { ...existing, ...incoming } : incoming);
    return { downloads: nextDownloads, batchItems: nextBatchItems };
  }

  /**
   * Map a DownloadState to the corresponding BatchSummary counter field.
   */
  private getStateBucket(
    state: DownloadState,
  ): 'downloading_items' | 'paused_items' | 'queued_items' | 'completed_items' | 'failed_items' | null {
    if (state === 'DOWNLOADING' || state === 'STARTING' || state === 'EXTRACTING' || state === 'IMPORTING')
      return 'downloading_items';
    if (state === 'PAUSED') return 'paused_items';
    if (state === 'QUEUED' || state === 'WAITING') return 'queued_items';
    if (state === 'COMPLETED') return 'completed_items';
    if (state === 'FAILED' || state === 'CANCELLED' || state === 'SKIPPED') return 'failed_items';
    return null;
  }

  /**
   * Apply a delta patch to a BatchSummary in the batches Map when a task state changes.
   */
  private patchBatchCounts(
    batches: Map<string, BatchSummary>,
    batchId: string,
    fromState: DownloadState | undefined,
    toState: DownloadState,
  ): Map<string, BatchSummary> {
    const batch = batches.get(batchId);
    if (!batch || fromState === toState) return batches;

    const fromBucket = fromState ? this.getStateBucket(fromState) : null;
    const toBucket = this.getStateBucket(toState);
    if (fromBucket === toBucket) return batches;

    const next = new Map(batches);
    const updated = { ...batch };
    if (fromBucket) updated[fromBucket] = Math.max(0, updated[fromBucket] - 1);
    if (toBucket) updated[toBucket] = updated[toBucket] + 1;
    next.set(batchId, updated);
    return next;
  }

  /**
   * Recalculate live batch metrics (progress, speed, downloaded_size, total_size)
   * from all cached items of the batch.
   */
  private patchBatchMetrics(
    batches: Map<string, BatchSummary>,
    batchId: string,
    cachedItems: DownloadTask[],
  ): Map<string, BatchSummary> {
    if (cachedItems.length === 0) return batches;
    const batch = batches.get(batchId);
    if (!batch) return batches;

    let totalDownloaded = 0;
    let totalSize = 0;
    let totalSpeed = 0;
    for (const item of cachedItems) {
      totalSize += item.size || 0;
      totalDownloaded += item.downloaded ?? 0;
      if (
        item.state === 'DOWNLOADING' ||
        item.state === 'STARTING' ||
        item.state === 'EXTRACTING' ||
        item.state === 'IMPORTING'
      ) {
        totalSpeed += item.speed ?? 0;
      }
    }

    const newTotalSize = totalSize > 0 ? totalSize : batch.total_size;
    const newProgress = newTotalSize > 0 ? (totalDownloaded / newTotalSize) * 100 : batch.progress;

    const next = new Map(batches);
    next.set(batchId, {
      ...batch,
      progress: Math.min(100, newProgress),
      speed: totalSpeed,
      downloaded_size: totalDownloaded,
      total_size: newTotalSize,
    });
    return next;
  }

  // ---------------------------------------------------------------------------
  // Data fetching
  // ---------------------------------------------------------------------------

  /**
   * Fetch a flat page of downloads (/api/downloads) with pagination, sorting
   * and the status/source filters.
   */
  async fetchPage(page?: number): Promise<void> {
    this.loading = true;
    this.error = null;
    try {
      const p = page ?? this.pagination.page;
      const { limit, sortBy, sortDir, statusFilter, sourceFilter } = this.pagination;
      let url = `${API_BASE}/downloads?page=${p}&limit=${limit}&sort_by=${sortBy}&sort_dir=${sortDir}`;
      if (statusFilter) url += `&status=${statusFilter}`;
      if (sourceFilter) url += `&source=${sourceFilter}`;

      const response = await fetch(url, { headers: apiHeaders() });
      if (!response.ok) throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      const data: DownloadsResponse = await response.json();

      const downloadsMap = new Map<string, DownloadTask>();
      data.downloads.forEach((download) => downloadsMap.set(download.id, download));

      this.downloads = downloadsMap;
      this.stats = data.stats;
      this.statusCounts = data.status_counts || this.statusCounts;
      this.pagination = { ...this.pagination, page: data.page, total: data.total, totalPages: data.total_pages };
      this.loading = false;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to fetch downloads';
      console.error('[DownloadStore] Fetch error:', errorMsg);
      this.loading = false;
      this.error = errorMsg;
    }
  }

  /**
   * Fetch a page of display units (/api/downloads/batches): batch summaries,
   * standalone tasks, unmanaged torrents and the authoritative row order.
   */
  async fetchBatches(page?: number): Promise<void> {
    const p = page ?? this.pagination.page;
    const { limit, sortBy, sortDir, statusFilter, sourceFilter } = this.pagination;
    this.loading = true;
    this.error = null;
    try {
      const params = new URLSearchParams({
        page: p.toString(),
        limit: limit.toString(),
        sort_by: sortBy,
        sort_dir: sortDir,
      });
      if (statusFilter) params.append('status', statusFilter);
      if (sourceFilter) params.append('source', sourceFilter);

      const response = await fetch(`/api/downloads/batches?${params}`, { headers: apiHeaders() });
      if (!response.ok) throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      const data: BatchSummariesResponse = await response.json();

      const batchesMap = new Map<string, BatchSummary>();
      data.batches.forEach((batch) => batchesMap.set(batch.batch_id, batch));
      const standaloneMap = new Map<string, DownloadTask>();
      data.standalone.forEach((task) => standaloneMap.set(task.id, task));
      const torrentsMap = new Map<string, UnifiedTorrentRow>();
      (data.torrents || []).forEach((torrent) => torrentsMap.set(`${torrent.instance_id}:${torrent.hash}`, torrent));

      this.batches = batchesMap;
      this.downloads = standaloneMap;
      this.torrentRows = torrentsMap;
      this.displayRows = this.dedupeRows(data.rows);
      this.stats = data.stats;
      this.statusCounts = data.status_counts || this.statusCounts;
      this.pagination = { ...this.pagination, page: data.page, total: data.total, totalPages: data.total_pages };
      this.loading = false;
      this._notify();
      console.log('[Store] fetchAll complete, items:', this.downloads.size);
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to fetch batches';
      console.error('[DownloadStore] Fetch batches error:', errorMsg);
      this.loading = false;
      this.error = errorMsg;
    }
  }

  /**
   * Fetch (and cache) the items of a batch for the current source filter.
   * Cache key: `"<batch_id>|<sourceFilter>"`. Throws on failure.
   */
  async fetchBatchItems(batchId: string): Promise<DownloadTask[]> {
    const { sourceFilter } = this.pagination;
    const cacheKey = `${batchId}|${sourceFilter ?? ''}`;
    const cached = this.batchItems.get(cacheKey);
    if (cached) return cached;

    try {
      const params = new URLSearchParams();
      if (sourceFilter) params.append('source', sourceFilter);
      const query = params.size > 0 ? `?${params}` : '';
      const response = await fetch(`/api/downloads/batch/${batchId}/items${query}`, { headers: apiHeaders() });
      if (!response.ok) throw new Error(`HTTP ${response.status}: ${response.statusText}`);

      const items = this.dedupeTasks(await response.json());
      const next = new Map(this.batchItems);
      next.set(cacheKey, items);
      this.batchItems = next;
      return items;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to fetch batch items';
      console.error('[DownloadStore] Fetch batch items error:', errorMsg);
      throw err;
    }
  }

  /** Drop all cached batch items. */
  invalidateBatchItemCache() {
    this.batchItems = new Map();
  }

  /**
   * Fetch all downloads (alias for fetchBatches with page 1)
   */
  async fetchAll(): Promise<void> {
    return this.fetchBatches(1);
  }

  // ---------------------------------------------------------------------------
  // Pagination
  // ---------------------------------------------------------------------------

  async setPage(page: number): Promise<void> {
    this.pagination = { ...this.pagination, page };
    return this.fetchBatches(page);
  }

  async setSort(sortBy: PaginationState['sortBy'], sortDir: PaginationState['sortDir']): Promise<void> {
    this.pagination = { ...this.pagination, sortBy, sortDir, page: 1 };
    return this.fetchBatches(1);
  }

  async setLimit(limit: number): Promise<void> {
    this.pagination = { ...this.pagination, limit, page: 1 };
    return this.fetchBatches(1);
  }

  /** @param status - Status to filter by, or null for all */
  async setStatusFilter(status: DownloadState | null): Promise<void> {
    this.pagination = { ...this.pagination, statusFilter: status, page: 1 };
    return this.fetchBatches(1);
  }

  /** Change the server-side source filter; clears the batch item cache. */
  async setSourceFilter(source: string | null): Promise<void> {
    this.pagination = { ...this.pagination, sourceFilter: source, page: 1 };
    this.invalidateBatchItemCache();
    return this.fetchBatches(1);
  }

  // ---------------------------------------------------------------------------
  // Actions
  // ---------------------------------------------------------------------------

  /**
   * Add a new download. Toasts unless `options.notify === false`.
   */
  async addDownload(
    request: AddDownloadRequest,
    options: AddDownloadOptions = {},
  ): Promise<ApiResponse<AddDownloadResponse>> {
    try {
      const response = await fetch(`${API_BASE}/downloads`, {
        method: 'POST',
        headers: apiHeaders(true),
        body: JSON.stringify(request),
      });
      const data = await response.json();
      if (!response.ok) throw new Error(data.error || data.message || `HTTP ${response.status}`);
      if (options.notify !== false) toasts.success(`Download added: ${data.filename}`);
      return { success: true, data };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to add download';
      console.error('[DownloadStore] Add error:', errorMsg);
      if (options.notify !== false) toasts.error(`Failed to add download: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  async pauseDownload(id: string): Promise<ApiResponse<void>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/${id}/pause`, { method: 'POST', headers: apiHeaders() });
      if (!response.ok) throw new Error(await responseErrorMessage(response));
      toasts.info('Download paused');
      return { success: true };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to pause download';
      console.error('[DownloadStore] Pause error:', errorMsg);
      toasts.error(`Failed to pause: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  async resumeDownload(id: string): Promise<ApiResponse<void>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/${id}/resume`, { method: 'POST', headers: apiHeaders() });
      if (!response.ok) throw new Error(await responseErrorMessage(response));
      toasts.success('Download resumed');
      return { success: true };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to resume download';
      console.error('[DownloadStore] Resume error:', errorMsg);
      toasts.error(`Failed to resume: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  /**
   * Delete a download. Removes it from downloads and every batch item cache;
   * batch members trigger a batch refetch. Refreshes the library overview.
   */
  async deleteDownload(id: string): Promise<ApiResponse<void>> {
    try {
      let batchKey: string | undefined;
      const task = this.downloads.get(id);
      if (task?.batch_id) {
        batchKey = task.batch_id;
      } else {
        for (const [key, items] of this.batchItems) {
          if (items.some((item) => item.id === id)) {
            batchKey = key;
            break;
          }
        }
      }

      const response = await fetch(`${API_BASE}/downloads/${id}`, { method: 'DELETE', headers: apiHeaders() });
      if (!response.ok) throw new Error(await responseErrorMessage(response));

      const next = new Map(this.downloads);
      next.delete(id);
      this.downloads = next;
      if (batchKey) this.batchItems = removeFromBatchItemCache(this.batchItems, id);

      toasts.info('Download deleted');
      if (batchKey) {
        this.fetchBatches().catch((err) => console.error('Failed to refetch batches after delete:', err));
      }
      fetchLibraryOverview().catch(() => {});
      return { success: true };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to delete download';
      console.error('[DownloadStore] Delete error:', errorMsg);
      toasts.error(`Failed to delete: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  async retryDownload(id: string): Promise<ApiResponse<void>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/${id}/retry`, { method: 'POST', headers: apiHeaders() });
      if (!response.ok) throw new Error(await responseErrorMessage(response));
      toasts.success('Download retry initiated');
      return { success: true };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to retry download';
      console.error('[DownloadStore] Retry error:', errorMsg);
      toasts.error(`Failed to retry: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  async redownload(id: string): Promise<ApiResponse<void>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/${id}/redownload`, {
        method: 'POST',
        headers: apiHeaders(),
      });
      if (!response.ok) throw new Error(await responseErrorMessage(response));
      toasts.success('Re-download initiated');
      return { success: true };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to re-download';
      console.error('[DownloadStore] Redownload error:', errorMsg);
      toasts.error(`Failed to re-download: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  async redownloadBatch(id: string): Promise<ApiResponse<AffectedResponse>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/batch/${id}/redownload`, {
        method: 'POST',
        headers: apiHeaders(),
      });
      if (!response.ok) throw new Error(await responseErrorMessage(response));
      const data: AffectedResponse = await response.json();
      toasts.success(`Re-download initiated for ${data.affected} task${data.affected !== 1 ? 's' : ''}`);
      this.fetchBatches().catch(() => {});
      return { success: true, data };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to re-download batch';
      console.error('[DownloadStore] Redownload batch error:', errorMsg);
      toasts.error(`Failed to re-download batch: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  async pauseAll(): Promise<ApiResponse<AffectedResponse>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/pause-all`, { method: 'POST', headers: apiHeaders() });
      const data = await response.json();
      if (!response.ok) throw new Error(data.error || data.message || `HTTP ${response.status}`);
      toasts.info(`Paused ${data.affected} download${data.affected !== 1 ? 's' : ''}`);
      return { success: true, data };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to pause all downloads';
      console.error('[DownloadStore] Pause all error:', errorMsg);
      toasts.error(`Failed to pause all: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  async resumeAll(): Promise<ApiResponse<AffectedResponse>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/resume-all`, { method: 'POST', headers: apiHeaders() });
      const data = await response.json();
      if (!response.ok) throw new Error(data.error || data.message || `HTTP ${response.status}`);
      toasts.success(`Resumed ${data.affected} download${data.affected !== 1 ? 's' : ''}`);
      return { success: true, data };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to resume all downloads';
      console.error('[DownloadStore] Resume all error:', errorMsg);
      toasts.error(`Failed to resume all: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  async pauseBatch(batchId: string): Promise<ApiResponse<AffectedResponse>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/batch/${batchId}/pause`, {
        method: 'POST',
        headers: apiHeaders(),
      });
      const data = await response.json();
      if (!response.ok) throw new Error(data.error || `HTTP ${response.status}`);
      toasts.info(`Paused ${data.affected} download${data.affected !== 1 ? 's' : ''} in batch`);
      this.fetchBatches().catch((err) => console.error('Failed to refetch batches after pause:', err));
      return { success: true, data };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to pause batch';
      toasts.error(`Failed to pause batch: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  async resumeBatch(batchId: string): Promise<ApiResponse<AffectedResponse>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/batch/${batchId}/resume`, {
        method: 'POST',
        headers: apiHeaders(),
      });
      const data = await response.json();
      if (!response.ok) throw new Error(data.error || `HTTP ${response.status}`);
      toasts.success(`Resumed ${data.affected} download${data.affected !== 1 ? 's' : ''} in batch`);
      this.fetchBatches().catch((err) => console.error('Failed to refetch batches after resume:', err));
      return { success: true, data };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to resume batch';
      toasts.error(`Failed to resume batch: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  async deleteBatch(batchId: string): Promise<ApiResponse<AffectedResponse>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/batch/${batchId}`, {
        method: 'DELETE',
        headers: apiHeaders(),
      });
      const data = await response.json();
      if (!response.ok) throw new Error(data.error || `HTTP ${response.status}`);
      toasts.info(`Deleted ${data.affected} download${data.affected !== 1 ? 's' : ''} in batch`);
      await this.fetchBatches();
      fetchLibraryOverview().catch(() => {});
      return { success: true, data };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to delete batch';
      toasts.error(`Failed to delete batch: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  // ---------------------------------------------------------------------------
  // WebSocket
  // ---------------------------------------------------------------------------

  /**
   * Register websocket handlers (call once). Re-fetches a snapshot on every
   * reconnect after the first connection.
   */
  initWebSocket(): void {
    let connectedBefore = false;
    wsClient.status.subscribe((status) => {
      if (status !== 'connected') return;
      if (connectedBefore) {
        this.fetchBatches().catch((err) => console.error('[DownloadStore] Reconnect snapshot failed:', err));
      }
      connectedBefore = true;
    });

    // SYNC_ALL — full task list on connect
    wsClient.on('SYNC_ALL', (message) => {
      if (!message.tasks || !Array.isArray(message.tasks)) return;
      let downloads = new Map(this.downloads);
      let batchItems = new Map(this.batchItems);
      for (const task of message.tasks as DownloadTask[]) {
        ({ downloads, batchItems } = this.applyTaskToCaches(task, downloads, batchItems));
      }
      this.downloads = downloads;
      this.batchItems = batchItems;
      this._notify();
    });

    wsClient.on('TASK_ADDED', (message) => {
      const task: DownloadTask | undefined = message.task;
      if (!task) return;
      emitDownloadEvent('TASK_UPDATED', task);
      const state = String(task.state || '').toUpperCase();
      if (state && state !== 'COMPLETED') emitDownloadEvent('TASK_STARTED', task);

      const { downloads, batchItems } = this.applyTaskToCaches(task, this.downloads, this.batchItems);
      this.downloads = downloads;
      this.batchItems = batchItems;
      if (task.batch_id) {
        this.fetchBatches().catch((err) => console.error('Failed to refetch batches:', err));
      }
      this._notify();
    });

    wsClient.on('TASK_UPDATED', (message) => {
      const task: DownloadTask | undefined = message.task;
      if (!task || !task.id) return;
      emitDownloadEvent('TASK_UPDATED', task);
      if (String(task.state || '').toUpperCase() === 'COMPLETED') emitDownloadEvent('TASK_COMPLETED', task);

      const taskId = task.id;
      const batchId = task.batch_id;
      const newState = task.state;
      const previous =
        this.downloads.get(taskId) ??
        (batchId ? cachedBatchItems(this.batchItems, batchId).find((item) => item.id === taskId) : undefined);
      const previousState = previous?.state;

      const applied = this.applyTaskToCaches(this.clearErrorIfActive(task), this.downloads, this.batchItems);
      this.downloads = applied.downloads;
      const batchItems = applied.batchItems;
      let batches = this.batches;

      if (batchId) {
        // Live batch header metrics + counters
        batches = this.patchBatchMetrics(batches, batchId, cachedBatchItems(batchItems, batchId));
        if (previousState !== newState) {
          batches = this.patchBatchCounts(batches, batchId, previousState, newState);
        }
      }

      this.batchItems = batchItems;
      this.batches = batches;
      this._notify();

      // Full server resync only for terminal state changes
      if (batchId && (newState === 'COMPLETED' || newState === 'FAILED')) {
        this.fetchBatches().catch((err) => console.error('Failed to refetch batches:', err));
      }
    });

    // TASK_BATCH_UPDATE — coalesced task updates (every ~500ms)
    wsClient.on('TASK_BATCH_UPDATE', (message) => {
      if (!message.tasks || !Array.isArray(message.tasks) || message.tasks.length === 0) return;

      let downloads = new Map(this.downloads);
      let batchItems = new Map(this.batchItems);
      let batches = this.batches;

      for (const task of message.tasks as DownloadTask[]) {
        if (!task || !task.id) continue;
        const previous =
          downloads.get(task.id) ??
          (task.batch_id ? cachedBatchItems(batchItems, task.batch_id).find((item) => item.id === task.id) : undefined);
        const previousState = previous?.state;
        ({ downloads, batchItems } = this.applyTaskToCaches(this.clearErrorIfActive(task), downloads, batchItems));
        if (task.batch_id && previousState !== task.state) {
          batches = this.patchBatchCounts(batches, task.batch_id, previousState, task.state);
        }
      }

      const touchedBatches = new Set(
        (message.tasks as DownloadTask[]).filter((task) => task?.batch_id).map((task) => task.batch_id as string),
      );
      for (const batchId of touchedBatches) {
        batches = this.patchBatchMetrics(batches, batchId, cachedBatchItems(batchItems, batchId));
      }

      this.downloads = downloads;
      this.batchItems = batchItems;
      this.batches = batches;
      this._notify();
    });

    wsClient.on('TASK_REMOVED', (message) => {
      const taskId: string | undefined = message.task_id;
      if (!taskId) return;
      const wasBatchMember = !!this.downloads.get(taskId)?.batch_id;

      const next = new Map(this.downloads);
      next.delete(taskId);
      this.downloads = next;
      this._notify();
      this.batchItems = removeFromBatchItemCache(this.batchItems, taskId);

      if (wasBatchMember) {
        this.fetchBatches().catch((err) => console.error('Failed to refetch batches:', err));
      }
      fetchLibraryOverview().catch(() => {});
    });

    wsClient.on('ENGINE_STATS', (message) => {
      if (!message.stats) return;
      this.stats = message.stats;
      this._notify();
    });

    // Unmanaged qBittorrent torrents (torrents already tracked by a task are ignored)
    wsClient.on('QBIT_TORRENT_UPSERT', (message) => {
      const torrent = message.torrent;
      if (!torrent || !message.instance_id) return;
      if (isTrackedTorrent(this.downloads, torrent.hash)) return;
      const next = new Map(this.torrentRows);
      next.set(`${message.instance_id}:${torrent.hash}`, toTorrentRow(message.instance_id, torrent));
      this.torrentRows = next;
      this._notify();
    });

    wsClient.on('QBIT_TORRENT_REMOVED', (message) => {
      if (!message.instance_id || !message.hash) return;
      const key = `${message.instance_id}:${message.hash}`;
      if (!this.torrentRows.has(key)) return;
      const next = new Map(this.torrentRows);
      next.delete(key);
      this.torrentRows = next;
      this._notify();
    });

    wsClient.on('QBIT_SYNC_INIT', (message) => {
      if (!message.instance_id || !Array.isArray(message.torrents)) return;
      const next = new Map(this.torrentRows);
      for (const torrent of message.torrents) {
        if (isTrackedTorrent(this.downloads, torrent.hash)) continue;
        next.set(`${message.instance_id}:${torrent.hash}`, toTorrentRow(message.instance_id, torrent));
      }
      this.torrentRows = next;
      this._notify();
    });
  }

  // ---------------------------------------------------------------------------
  // Svelte 4 store contract (backward compat)
  // ---------------------------------------------------------------------------

  /**
   * Subscribe to state snapshots (called immediately, then after each
   * fetch/websocket update). Returns the unsubscribe function.
   */
  subscribe(fn: (v: DownloadStoreState) => void): () => void {
    this._subscribers.add(fn);
    fn(this.snapshot());
    return () => {
      this._subscribers.delete(fn);
    };
  }
}

// ============================================================================
// Download events (UI side-channel, e.g. completion toasts / sounds)
// ============================================================================

const downloadEvents = typeof window !== 'undefined' ? new EventTarget() : null;

function emitDownloadEvent(type: DownloadEventType, task: DownloadTask) {
  downloadEvents?.dispatchEvent(new CustomEvent(type, { detail: task }));
}

/**
 * Listen for TASK_UPDATED / TASK_STARTED / TASK_COMPLETED re-broadcast by the
 * store (`event.detail` is the DownloadTask). Returns an unsubscribe function
 * (no-op during SSR).
 */
export function onDownloadEvent(
  type: DownloadEventType,
  listener: (event: CustomEvent<DownloadTask>) => void,
): () => void {
  if (!downloadEvents) return () => {};
  const handler = listener as EventListener;
  downloadEvents.addEventListener(type, handler);
  return () => downloadEvents.removeEventListener(type, handler);
}

/**
 * Global store instance
 */
export const downloadStore = new DownloadStore();

// ============================================================================
// Svelte 4 compat readable stores (re-emit on every store notification)
// ============================================================================

function makeReadable<T>(getter: () => T) {
  return {
    subscribe(fn: (value: T) => void): () => void {
      return downloadStore.subscribe(() => {
        fn(getter());
      });
    },
  };
}

/** The only compat readable referenced by the 2026-09-02 build. */
export const engineStats = makeReadable<EngineStats>(() => downloadStore.stats);

// The following were exported by the 2026-05-14 baseline; none is referenced by
// the 2026-09-02 build (removed or tree-shaken — cannot tell).
export const downloads = makeReadable<DownloadTask[]>(() => downloadStore.downloadList);
export const batches = makeReadable<BatchSummary[]>(() => downloadStore.batchList);
export const activeDownloads = makeReadable<DownloadTask[]>(() => downloadStore.activeDownloadsList);
export const queuedDownloads = makeReadable<DownloadTask[]>(() => downloadStore.queuedDownloadsList);
export const completedDownloads = makeReadable<DownloadTask[]>(() => downloadStore.completedDownloadsList);
export const failedDownloads = makeReadable<DownloadTask[]>(() => downloadStore.failedDownloadsList);
export const pausedDownloads = makeReadable<DownloadTask[]>(() => downloadStore.pausedDownloadsList);
export const statusCounts = makeReadable<StatusCounts>(() => downloadStore.statusCounts);
export const paginationState = makeReadable<PaginationState>(() => downloadStore.pagination);
export const isLoading = makeReadable<boolean>(() => downloadStore.loading);
export const storeError = makeReadable<string | null>(() => downloadStore.error);

// ============================================================================
// Formatting helpers
// ============================================================================

/** 1024-based, 2 decimals. Non-finite/negative/zero → "0 B". */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0 || bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
}

export function formatSpeed(bytesPerSec: number | string): string {
  return `${formatBytes(Number(bytesPerSec))}/s`;
}

/** "1h 5m" / "3m 20s" / "45s"; "--" when unknown. */
export function formatETA(seconds: number | undefined | null): string {
  if (!seconds || seconds <= 0) return '--';
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${secs}s`;
  return `${secs}s`;
}

/** Baseline helper; not referenced by the 2026-09-02 build. */
export function getStateColor(state: DownloadState): string {
  switch (state) {
    case 'DOWNLOADING':
    case 'STARTING':
      return 'blue';
    case 'COMPLETED':
      return 'green';
    case 'FAILED':
      return 'red';
    case 'PAUSED':
      return 'orange';
    default:
      return 'gray';
  }
}
