import { wsClient } from './websocket';
import { toasts } from './toasts';
import { fetchLibraryOverview } from './arr';

/**
 * Download State (from contract-fe-bridge.md)
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
 * Status priority for urgency-based sorting (lower = higher priority)
 * Based on user attention funnel: errors need immediate action, completed needs least
 */
export const STATUS_PRIORITY: Record<DownloadState, number> = {
  FAILED: 1,      // Requires immediate intervention
  CANCELLED: 2,   // User-cancelled, may want to re-add
  DOWNLOADING: 3, // Active, consuming bandwidth
  EXTRACTING: 4,  // Post-download, still working
  IMPORTING: 4.5, // Remapping and notifying Sonarr/Radarr
  STARTING: 5,    // Transitional to DOWNLOADING
  WAITING: 6,     // Rate-limited, auto-retry soon
  PAUSED: 7,      // User-initiated freeze
  QUEUED: 8,      // Waiting in queue
  SKIPPED: 9,     // Intentionally skipped
  COMPLETED: 10,  // Done, archive-ready
};

/**
 * Download Task Interface (from contract-fe-bridge.md)
 */
export interface DownloadTask {
  id: string;
  url: string;
  original_url: string;
  filename: string;
  destination: string;
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
  /** Batch ID for grouping related downloads (e.g., TV season episodes) */
  batch_id?: string;
  /** Batch display name (e.g., "Breaking Bad S01") */
  batch_name?: string;
  /** Quality description (e.g., "WEB-DL-1080p") */
  quality?: string;
  /** Resolution shorthand (e.g., "1080p", "2160p") */
  resolution?: string;
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
  state: DownloadState;
}

/**
 * Database-sourced status counts (for filter dropdown)
 * These are total counts from the database, not just in-memory tasks
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
 * Engine Statistics (from contract-fe-bridge.md)
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
 * TMDB Metadata for organized folder structure
 */
export interface TmdbMetadata {
  tmdb_id?: number;
  media_type?: 'movie' | 'tv';
  title?: string;
  year?: string;
  collection_name?: string;
  season?: number;
  episode?: number;
}

/**
 * Add Download Request
 */
export interface AddDownloadRequest {
  url: string;
  filename?: string;
  category?: string;
  priority?: 'NORMAL' | 'HIGH' | 'LOW';
  /** TMDB metadata for folder organization */
  tmdb?: TmdbMetadata;
  /** Batch ID for grouping downloads from the same request */
  batch_id?: string;
  /** Batch name for display purposes */
  batch_name?: string;
}

/**
 * API Response Types
 */
interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

interface DownloadsResponse {
  downloads: DownloadTask[];
  stats: EngineStats;
  /** Per-status counts from database (for filter dropdown) */
  status_counts: StatusCounts;
  /** Pagination info */
  total: number;
  page: number;
  limit: number;
  total_pages: number;
}

/** Response from /api/downloads/batches endpoint */
interface BatchSummariesResponse {
  batches: BatchSummary[];
  standalone: DownloadTask[];
  stats: EngineStats;
  status_counts: StatusCounts;
  total: number; // Total display units (batches + standalone)
  page: number;
  limit: number;
  total_pages: number;
}

interface AddDownloadResponse {
  success: boolean;
  task_id: string;
  filename: string;
  state: string;
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

/**
 * API Base URL
 */
const API_BASE = '/api';

/**
 * DownloadStoreState — snapshot shape exposed by the Svelte 4 subscribe compat method.
 * Consumers that use `$downloadStore.downloads` etc. receive this type.
 */
export interface DownloadStoreState {
  downloads: Map<string, DownloadTask>;
  batches: Map<string, BatchSummary>;
  batchItems: Map<string, DownloadTask[]>;
  stats: EngineStats;
  statusCounts: StatusCounts;
  pagination: PaginationState;
  loading: boolean;
  error: string | null;
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
  /** Cache of expanded batch items (batch_id -> items) */
  batchItems = $state<Map<string, DownloadTask[]>>(new Map());
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
  });
  loading = $state<boolean>(false);
  error = $state<string | null>(null);

  // Private notification system for legacy store subscribers
  private _subscribers = new Set<(v: DownloadStoreState) => void>();

  private _notify() {
    if (this._subscribers.size === 0) return;
    const snapshot = {
      downloads: this.downloads,
      batches: this.batches,
      batchItems: this.batchItems,
      stats: this.stats,
      statusCounts: this.statusCounts,
      pagination: this.pagination,
      loading: this.loading,
      error: this.error,
    };
    this._subscribers.forEach(fn => fn(snapshot));
  }

  // ---------------------------------------------------------------------------
  // Derived values
  // ---------------------------------------------------------------------------

  /** Downloads as array (sorted by created_at desc) */
  downloadList: DownloadTask[] = $derived.by(() => {
    const list = Array.from(this.downloads.values()).sort(
      (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
    );
    return list;
  });

  /** Batches as array (sorted by created_at desc) */
  batchList: BatchSummary[] = $derived.by(() =>
    Array.from(this.batches.values()).sort(
      (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
    )
  );

  /** Active downloads */
  activeDownloadsList: DownloadTask[] = $derived.by(() =>
    this.downloadList.filter(d => d.state === 'DOWNLOADING' || d.state === 'STARTING' || d.state === 'IMPORTING')
  );

  /** Queued downloads */
  queuedDownloadsList: DownloadTask[] = $derived.by(() =>
    this.downloadList.filter(d => d.state === 'QUEUED')
  );

  /** Completed downloads */
  completedDownloadsList: DownloadTask[] = $derived.by(() =>
    this.downloadList.filter(d => d.state === 'COMPLETED')
  );

  /** Failed downloads */
  failedDownloadsList: DownloadTask[] = $derived.by(() =>
    this.downloadList.filter(d => d.state === 'FAILED')
  );

  /** Paused downloads */
  pausedDownloadsList: DownloadTask[] = $derived.by(() =>
    this.downloadList.filter(d => d.state === 'PAUSED')
  );

  // ---------------------------------------------------------------------------
  // Private helpers
  // ---------------------------------------------------------------------------

  /**
   * Clear error_message when a task enters a working/active state.
   */
  private clearErrorIfActive(task: DownloadTask): DownloadTask {
    const activeStates: DownloadState[] = ['QUEUED', 'STARTING', 'DOWNLOADING', 'EXTRACTING', 'WAITING', 'IMPORTING'];
    if (activeStates.includes(task.state)) {
      return { ...task, error_message: null };
    }
    return task;
  }

  /**
   * Map a DownloadState to the corresponding BatchSummary counter field.
   */
  private getStateBucket(
    state: DownloadState
  ): 'downloading_items' | 'paused_items' | 'queued_items' | 'completed_items' | 'failed_items' | null {
    if (state === 'DOWNLOADING' || state === 'STARTING' || state === 'EXTRACTING' || state === 'IMPORTING') return 'downloading_items';
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
    toState: DownloadState
  ): Map<string, BatchSummary> {
    const batch = batches.get(batchId);
    if (!batch || fromState === toState) return batches;

    const fromBucket = fromState ? this.getStateBucket(fromState) : null;
    const toBucket = this.getStateBucket(toState);
    if (fromBucket === toBucket) return batches;

    const newBatches = new Map(batches);
    const updated = { ...batch };
    if (fromBucket) updated[fromBucket] = Math.max(0, updated[fromBucket] - 1);
    if (toBucket) updated[toBucket] = updated[toBucket] + 1;
    newBatches.set(batchId, updated);
    return newBatches;
  }

  /**
   * Recalculate live batch metrics (progress, speed, downloaded_size, total_size)
   * from all cached items in batchItems.
   */
  private patchBatchMetrics(
    batches: Map<string, BatchSummary>,
    batchId: string,
    cachedItems: DownloadTask[]
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
      if (item.state === 'DOWNLOADING' || item.state === 'STARTING' || item.state === 'EXTRACTING' || item.state === 'IMPORTING') {
        totalSpeed += item.speed ?? 0;
      }
    }

    const newTotalSize = totalSize > 0 ? totalSize : batch.total_size;
    const newProgress = newTotalSize > 0 ? (totalDownloaded / newTotalSize) * 100 : batch.progress;

    const newBatches = new Map(batches);
    newBatches.set(batchId, {
      ...batch,
      progress: Math.min(100, newProgress),
      speed: totalSpeed,
      downloaded_size: totalDownloaded,
      total_size: newTotalSize,
    });
    return newBatches;
  }

  // ---------------------------------------------------------------------------
  // Data fetching
  // ---------------------------------------------------------------------------

  /**
   * Fetch a page of downloads from the API (with pagination, sorting, and optional status filter)
   */
  async fetchPage(page?: number): Promise<void> {
    this.loading = true;
    this.error = null;

    try {
      const p = page ?? this.pagination.page;
      const { limit, sortBy, sortDir, statusFilter } = this.pagination;

      let url = `${API_BASE}/downloads?page=${p}&limit=${limit}&sort_by=${sortBy}&sort_dir=${sortDir}`;
      if (statusFilter) {
        url += `&status=${statusFilter}`;
      }

      const response = await fetch(url);

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const data: DownloadsResponse = await response.json();

      const downloadsMap = new Map<string, DownloadTask>();
      data.downloads.forEach((download: DownloadTask) => {
        downloadsMap.set(download.id, download);
      });

      this.downloads = downloadsMap;
      this.stats = data.stats;
      this.statusCounts = data.status_counts || this.statusCounts;
      this.pagination = {
        ...this.pagination,
        page: data.page,
        total: data.total,
        totalPages: data.total_pages,
      };
      this.loading = false;

    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to fetch downloads';
      console.error('[DownloadStore] Fetch error:', errorMsg);
      this.loading = false;
      this.error = errorMsg;
    }
  }

  /**
   * Fetch batch summaries and standalone downloads (batch-first pagination)
   */
  async fetchBatches(page?: number): Promise<void> {
    const pageToFetch = page ?? this.pagination.page;
    const { limit, sortBy, sortDir, statusFilter } = this.pagination;

    this.loading = true;
    this.error = null;

    try {
      const params = new URLSearchParams({
        page: pageToFetch.toString(),
        limit: limit.toString(),
        sort_by: sortBy,
        sort_dir: sortDir,
      });

      if (statusFilter) {
        params.append('status', statusFilter);
      }

      const response = await fetch(`/api/downloads/batches?${params}`);
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const data: BatchSummariesResponse = await response.json();

      const batchesMap = new Map<string, BatchSummary>();
      data.batches.forEach((batch: BatchSummary) => {
        batchesMap.set(batch.batch_id, batch);
      });

      // Replace standalone downloads with exactly what the API returned for
      // this page to avoid items from other pages persisting when paginating.
      const newDownloads = new Map<string, DownloadTask>();
      data.standalone.forEach((download: DownloadTask) => {
        newDownloads.set(download.id, download);
      });

      this.batches = batchesMap;
      this.downloads = newDownloads;
      this.stats = data.stats;
      this.statusCounts = data.status_counts || this.statusCounts;
      this.pagination = {
        ...this.pagination,
        page: data.page,
        total: data.total,
        totalPages: data.total_pages,
      };
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
   * Fetch items for a specific batch (lazy loading)
   */
  async fetchBatchItems(batchId: string): Promise<DownloadTask[]> {
    const cached = this.batchItems.get(batchId);
    if (cached) {
      return cached;
    }

    try {
      const response = await fetch(`/api/downloads/batch/${batchId}/items`);
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const items: DownloadTask[] = await response.json();

      const newBatchItems = new Map(this.batchItems);
      newBatchItems.set(batchId, items);
      this.batchItems = newBatchItems;

      return items;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to fetch batch items';
      console.error('[DownloadStore] Fetch batch items error:', errorMsg);
      throw err;
    }
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

  /**
   * Set pagination parameters and refetch
   */
  async setPage(page: number): Promise<void> {
    this.pagination = { ...this.pagination, page };
    return this.fetchBatches(page);
  }

  /**
   * Set sort parameters and refetch from page 1
   */
  async setSort(sortBy: PaginationState['sortBy'], sortDir: PaginationState['sortDir']): Promise<void> {
    this.pagination = { ...this.pagination, sortBy, sortDir, page: 1 };
    return this.fetchBatches(1);
  }

  /**
   * Set items per page and refetch from page 1
   */
  async setLimit(limit: number): Promise<void> {
    this.pagination = { ...this.pagination, limit, page: 1 };
    return this.fetchBatches(1);
  }

  /**
   * Set status filter and refetch from page 1
   * @param status - Status to filter by, or null/undefined for all
   */
  async setStatusFilter(status: DownloadState | null): Promise<void> {
    this.pagination = { ...this.pagination, statusFilter: status, page: 1 };
    return this.fetchBatches(1);
  }

  // ---------------------------------------------------------------------------
  // Actions
  // ---------------------------------------------------------------------------

  /**
   * Add a new download
   */
  async addDownload(request: AddDownloadRequest): Promise<ApiResponse<AddDownloadResponse>> {
    try {
      const response = await fetch(`${API_BASE}/downloads`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(request),
      });

      const data = await response.json();

      if (!response.ok) {
        throw new Error(data.error || data.message || `HTTP ${response.status}`);
      }

      toasts.success(`Download added: ${data.filename}`);

      return { success: true, data };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to add download';
      console.error('[DownloadStore] Add error:', errorMsg);
      toasts.error(`Failed to add download: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  /**
   * Pause a download
   */
  async pauseDownload(id: string): Promise<ApiResponse<void>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/${id}/pause`, {
        method: 'POST',
      });

      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      toasts.info('Download paused');
      return { success: true };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to pause download';
      console.error('[DownloadStore] Pause error:', errorMsg);
      toasts.error(`Failed to pause: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  /**
   * Resume a download
   */
  async resumeDownload(id: string): Promise<ApiResponse<void>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/${id}/resume`, {
        method: 'POST',
      });

      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || `HTTP ${response.status}`);
      }

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
   * Delete a download
   */
  async deleteDownload(id: string): Promise<ApiResponse<void>> {
    try {
      // Capture batch_id BEFORE deleting from the map
      let batchId: string | undefined;
      const task = this.downloads.get(id);
      if (task?.batch_id) {
        batchId = task.batch_id;
      } else {
        // Search in batchItems cache (batch children aren't always in downloads map)
        for (const [bid, items] of this.batchItems) {
          if (items.some(item => item.id === id)) {
            batchId = bid;
            break;
          }
        }
      }

      const response = await fetch(`${API_BASE}/downloads/${id}`, {
        method: 'DELETE',
      });

      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      // Remove from local store immediately for instant UI feedback
      const newDownloads = new Map(this.downloads);
      newDownloads.delete(id);
      this.downloads = newDownloads;

      // Also remove from batch items cache if it was a batch item
      if (batchId) {
        const newBatchItems = new Map(this.batchItems);
        const items = newBatchItems.get(batchId);
        if (items) {
          newBatchItems.set(batchId, items.filter(item => item.id !== id));
        }
        this.batchItems = newBatchItems;
      }

      toasts.info('Download deleted');

      // If it was a batch item, refetch batch summaries to update counts/progress
      if (batchId) {
        this.fetchBatches().catch(err => console.error('Failed to refetch batches after delete:', err));
      }

      // Always refresh library overview after any delete since the cost is one
      // lightweight /api/arr/library call and the user explicitly requested sync.
      fetchLibraryOverview().catch(() => { /* silently ignore if Arr is offline */ });

      return { success: true };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to delete download';
      console.error('[DownloadStore] Delete error:', errorMsg);
      toasts.error(`Failed to delete: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  /**
   * Retry a failed download
   */
  async retryDownload(id: string): Promise<ApiResponse<void>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/${id}/retry`, {
        method: 'POST',
      });

      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      toasts.success('Download retry initiated');
      return { success: true };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to retry download';
      console.error('[DownloadStore] Retry error:', errorMsg);
      toasts.error(`Failed to retry: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  /**
   * Re-download a completed download
   */
  async redownload(id: string): Promise<ApiResponse<void>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/${id}/redownload`, {
        method: 'POST',
      });

      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      toasts.success('Re-download initiated');
      return { success: true };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to re-download';
      console.error('[DownloadStore] Redownload error:', errorMsg);
      toasts.error(`Failed to re-download: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  /**
   * Re-download all tasks in a batch
   */
  async redownloadBatch(id: string): Promise<ApiResponse<{ affected: number }>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/batch/${id}/redownload`, {
        method: 'POST',
      });

      const data = await response.json();

      if (!response.ok) {
        throw new Error(data.error || `HTTP ${response.status}`);
      }

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

  /**
   * Pause all active downloads
   */
  async pauseAll(): Promise<ApiResponse<{ affected: number }>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/pause-all`, {
        method: 'POST',
      });

      const data = await response.json();

      if (!response.ok) {
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      toasts.info(`Paused ${data.affected} download${data.affected !== 1 ? 's' : ''}`);
      return { success: true, data };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to pause all downloads';
      console.error('[DownloadStore] Pause all error:', errorMsg);
      toasts.error(`Failed to pause all: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  /**
   * Resume all paused downloads
   */
  async resumeAll(): Promise<ApiResponse<{ affected: number }>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/resume-all`, {
        method: 'POST',
      });

      const data = await response.json();

      if (!response.ok) {
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      toasts.success(`Resumed ${data.affected} download${data.affected !== 1 ? 's' : ''}`);
      return { success: true, data };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to resume all downloads';
      console.error('[DownloadStore] Resume all error:', errorMsg);
      toasts.error(`Failed to resume all: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  // ---------------------------------------------------------------------------
  // Batch Actions
  // ---------------------------------------------------------------------------

  /**
   * Pause all downloads in a batch
   */
  async pauseBatch(batchId: string): Promise<ApiResponse<{ affected: number }>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/batch/${batchId}/pause`, {
        method: 'POST',
      });

      const data = await response.json();

      if (!response.ok) {
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      toasts.info(`Paused ${data.affected} download${data.affected !== 1 ? 's' : ''} in batch`);

      // Refetch batches to update batch row status
      this.fetchBatches().catch(err => console.error('Failed to refetch batches after pause:', err));

      return { success: true, data };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to pause batch';
      toasts.error(`Failed to pause batch: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  /**
   * Resume all downloads in a batch
   */
  async resumeBatch(batchId: string): Promise<ApiResponse<{ affected: number }>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/batch/${batchId}/resume`, {
        method: 'POST',
      });

      const data = await response.json();

      if (!response.ok) {
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      toasts.success(`Resumed ${data.affected} download${data.affected !== 1 ? 's' : ''} in batch`);

      // Refetch batches to update batch row status
      this.fetchBatches().catch(err => console.error('Failed to refetch batches after resume:', err));

      return { success: true, data };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to resume batch';
      toasts.error(`Failed to resume batch: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  /**
   * Delete all downloads in a batch
   */
  async deleteBatch(batchId: string): Promise<ApiResponse<{ affected: number }>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/batch/${batchId}`, {
        method: 'DELETE',
      });

      const data = await response.json();

      if (!response.ok) {
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      toasts.info(`Deleted ${data.affected} download${data.affected !== 1 ? 's' : ''} in batch`);

      // Refetch batches to get fresh data from server and update UI
      await this.fetchBatches();

      // Batch may contain completed items that were imported — sync library counts.
      fetchLibraryOverview().catch(() => {});

      return { success: true, data };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to delete batch';
      toasts.error(`Failed to delete batch: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  // ---------------------------------------------------------------------------
  // WebSocket initialisation
  // ---------------------------------------------------------------------------

  /**
   * Initialize WebSocket handlers
   */
  initWebSocket(): void {
    // Handle SYNC_ALL - server sends only ACTIVE tasks on WS connect.
    // Merge (upsert) into the existing Map rather than replace it, otherwise
    // REST-loaded tasks vanish the moment the WebSocket fires.
    wsClient.on('SYNC_ALL', (message) => {
      if (message.tasks && Array.isArray(message.tasks)) {
        const newDownloads = new Map(this.downloads);
        (message.tasks as DownloadTask[]).forEach((task: DownloadTask) => {
          newDownloads.set(task.id, task);
        });
        this.downloads = newDownloads;
        this._notify();
      }
    });

    // Handle TASK_ADDED - new task added
    wsClient.on('TASK_ADDED', (message) => {
      if (message.task) {
        const newDownloads = new Map(this.downloads);
        newDownloads.set(message.task.id, message.task);
        this.downloads = newDownloads;

        // If task has batch_id, update batchItems cache if it exists (expanded batch)
        if (message.task.batch_id) {
          const cachedItems = this.batchItems.get(message.task.batch_id);
          if (cachedItems) {
            if (!cachedItems.some(i => i.id === message.task!.id)) {
              const newBatchItems = new Map(this.batchItems);
              newBatchItems.set(message.task.batch_id, [...cachedItems, message.task]);
              this.batchItems = newBatchItems;
            }
          }
          this.fetchBatches().catch(err => console.error('Failed to refetch batches:', err));
        }

        this._notify();
      }
    });

    // Handle TASK_UPDATED - task state/progress changed
    wsClient.on('TASK_UPDATED', (message) => {
      if (!message.task || !message.task.id) return;

      const taskId: string = message.task.id;
      const batchId: string | undefined = message.task.batch_id;
      const newState: DownloadState = message.task.state;

      // --- 1. Resolve previous state ---
      const existingInDownloads = this.downloads.get(taskId);
      const existingInBatch = batchId
        ? this.batchItems.get(batchId)?.find(i => i.id === taskId)
        : undefined;
      const previousState: DownloadState | undefined =
        existingInDownloads?.state ?? existingInBatch?.state;

      // --- 2. Update downloads Map (standalone items) ---
      const newDownloads = new Map(this.downloads);
      const incomingTask = this.clearErrorIfActive(message.task);
      if (existingInDownloads) {
        newDownloads.set(taskId, { ...existingInDownloads, ...incomingTask });
      } else if (!batchId) {
        newDownloads.set(taskId, incomingTask);
      }
      this.downloads = newDownloads;

      let newBatchItems = this.batchItems;
      let newBatches = this.batches;

      if (batchId) {
        // --- 3. Patch individual item in batchItems cache ---
        const cachedItems = this.batchItems.get(batchId);
        if (cachedItems) {
          const isExisting = cachedItems.some(item => item.id === taskId);
          const updated = isExisting
            ? cachedItems.map(item => item.id === taskId ? { ...item, ...incomingTask } : item)
            : [...cachedItems, incomingTask];

          newBatchItems = new Map(this.batchItems);
          newBatchItems.set(batchId, updated);

          // --- 5. Recalculate live metrics (progress, speed, downloaded_size) ---
          newBatches = this.patchBatchMetrics(newBatches, batchId, updated);
        }

        // --- 4. Delta-patch the BatchSummary so the batch header row syncs immediately ---
        if (previousState !== newState) {
          newBatches = this.patchBatchCounts(newBatches, batchId, previousState, newState);
        }
      }

      this.batchItems = newBatchItems;
      this.batches = newBatches;
      this._notify();

      // Full server resync only for terminal state changes
      if (batchId && (newState === 'COMPLETED' || newState === 'FAILED')) {
        this.fetchBatches().catch(err => console.error('Failed to refetch batches:', err));
      }
    });

    // Handle TASK_BATCH_UPDATE - batch of task updates (sent every 500ms)
    wsClient.on('TASK_BATCH_UPDATE', (message) => {
      if (!message.tasks || !Array.isArray(message.tasks) || message.tasks.length === 0) return;

      const newDownloads = new Map(this.downloads);
      const newBatchItems = new Map(this.batchItems);
      let newBatches = this.batches;

      for (const task of message.tasks) {
        if (!task || !task.id) continue;

        const existingInDownloads = newDownloads.get(task.id);
        const existingInBatch = task.batch_id
          ? newBatchItems.get(task.batch_id)?.find((i: DownloadTask) => i.id === task.id)
          : undefined;
        const previousState: DownloadState | undefined =
          existingInDownloads?.state ?? existingInBatch?.state;

        const incomingTask = this.clearErrorIfActive(task);

        if (existingInDownloads) {
          newDownloads.set(task.id, { ...existingInDownloads, ...incomingTask });
        } else if (!task.batch_id) {
          newDownloads.set(task.id, incomingTask);
        }

        if (task.batch_id) {
          const cachedItems = newBatchItems.get(task.batch_id);
          if (cachedItems) {
            const isExisting = cachedItems.some((item: DownloadTask) => item.id === task.id);
            const updated = isExisting
              ? cachedItems.map((item: DownloadTask) => (item.id === task.id ? { ...item, ...incomingTask } : item))
              : [...cachedItems, incomingTask];

            newBatchItems.set(task.batch_id, updated);
          }

          if (previousState !== task.state) {
            newBatches = this.patchBatchCounts(newBatches, task.batch_id, previousState, task.state);
          }
        }
      }

      // Recalculate live metrics for every affected batch that has items in cache
      const affectedBatchIds: Set<string> = new Set(
        (message.tasks as DownloadTask[])
          .filter(t => t?.batch_id)
          .map(t => t.batch_id as string)
      );
      for (const bid of affectedBatchIds) {
        const items = newBatchItems.get(bid);
        if (items) {
          newBatches = this.patchBatchMetrics(newBatches, bid, items);
        }
      }

      this.downloads = newDownloads;
      this.batchItems = newBatchItems;
      this.batches = newBatches;
      this._notify();
    });

    // Handle TASK_REMOVED - task deleted
    wsClient.on('TASK_REMOVED', (message) => {
      if (message.task_id) {
        let wasBatchItem = false;
        let batchId: string | undefined;

        const task = this.downloads.get(message.task_id);
        if (task?.batch_id) {
          wasBatchItem = true;
          batchId = task.batch_id;
        }

        const newDownloads = new Map(this.downloads);
        newDownloads.delete(message.task_id);
        this.downloads = newDownloads;
        this._notify();

        if (batchId) {
          const newBatchItems = new Map(this.batchItems);
          const items = newBatchItems.get(batchId);
          if (items) {
            newBatchItems.set(batchId, items.filter(item => item.id !== message.task_id));
          }
          this.batchItems = newBatchItems;
        }

        if (wasBatchItem) {
          this.fetchBatches().catch(err => console.error('Failed to refetch batches:', err));
        }

        // Sync library overview whenever a task is removed
        fetchLibraryOverview().catch(() => {});
      }
    });

    // Handle ENGINE_STATS - engine statistics update
    wsClient.on('ENGINE_STATS', (message) => {
      if (message.stats) {
        this.stats = message.stats;
        this._notify();
      }
    });
  }

  // ---------------------------------------------------------------------------
  // Svelte 4 store compat: subscribe method
  // Allows $downloadStore auto-subscription in legacy .svelte templates.
  // ---------------------------------------------------------------------------
  subscribe(fn: (v: DownloadStoreState) => void): () => void {
    this._subscribers.add(fn);
    
    // Initial sync
    fn({
      downloads: this.downloads,
      batches: this.batches,
      batchItems: this.batchItems,
      stats: this.stats,
      statusCounts: this.statusCounts,
      pagination: this.pagination,
      loading: this.loading,
      error: this.error,
    });

    return () => {
      this._subscribers.delete(fn);
    };
  }
}

/**
 * Global download store singleton
 */
export const downloadStore = new DownloadStore();

// ---------------------------------------------------------------------------
// Backward-compat store wrappers
// These wrap singleton state so existing Svelte 4 $store auto-subscriptions
// in .svelte files continue to work without modification.
// In new Svelte 5 components, access downloadStore properties directly.
// ---------------------------------------------------------------------------

function makeReadable<T>(getter: () => T) {
  return {
    subscribe(fn: (v: T) => void): () => void {
      // Subscribe to the main store to get updates
      return downloadStore.subscribe(() => {
        fn(getter());
      });
    },
  };
}

/** Downloads as sorted array — use downloadStore.downloadList in new code */
export const downloads = makeReadable<DownloadTask[]>(() => downloadStore.downloadList);

/** Batches as sorted array — use downloadStore.batchList in new code */
export const batches = makeReadable<BatchSummary[]>(() => downloadStore.batchList);

export const activeDownloads = makeReadable<DownloadTask[]>(() => downloadStore.activeDownloadsList);
export const queuedDownloads = makeReadable<DownloadTask[]>(() => downloadStore.queuedDownloadsList);
export const completedDownloads = makeReadable<DownloadTask[]>(() => downloadStore.completedDownloadsList);
export const failedDownloads = makeReadable<DownloadTask[]>(() => downloadStore.failedDownloadsList);
export const pausedDownloads = makeReadable<DownloadTask[]>(() => downloadStore.pausedDownloadsList);
export const engineStats = makeReadable<EngineStats>(() => downloadStore.stats);
export const statusCounts = makeReadable<StatusCounts>(() => downloadStore.statusCounts);
export const paginationState = makeReadable<PaginationState>(() => downloadStore.pagination);
export const isLoading = makeReadable<boolean>(() => downloadStore.loading);
export const storeError = makeReadable<string | null>(() => downloadStore.error);

// ---------------------------------------------------------------------------
// Helper functions (unchanged)
// ---------------------------------------------------------------------------

/**
 * Helper: Format bytes to human readable
 */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
}

/**
 * Helper: Format speed (bytes/sec to human readable)
 */
export function formatSpeed(bytesPerSec: number): string {
  return `${formatBytes(bytesPerSec)}/s`;
}

/**
 * Helper: Format ETA (seconds to human readable)
 */
export function formatETA(seconds: number | undefined): string {
  if (!seconds || seconds <= 0) return '--';

  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  } else if (minutes > 0) {
    return `${minutes}m ${secs}s`;
  } else {
    return `${secs}s`;
  }
}

/**
 * Helper: Get state color
 */
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
    case 'QUEUED':
    case 'WAITING':
      return 'gray';
    default:
      return 'gray';
  }
}
