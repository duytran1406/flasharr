import { writable, derived, get } from 'svelte/store';
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
  | 'SKIPPED';

/**
 * Status priority for urgency-based sorting (lower = higher priority)
 * Based on user attention funnel: errors need immediate action, completed needs least
 */
export const STATUS_PRIORITY: Record<DownloadState, number> = {
  FAILED: 1,      // Requires immediate intervention
  CANCELLED: 2,   // User-cancelled, may want to re-add
  DOWNLOADING: 3, // Active, consuming bandwidth
  EXTRACTING: 4,  // Post-download, still working
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
 * Download Store State
 */
interface DownloadStoreState {
  downloads: Map<string, DownloadTask>;
  /** Batch summaries for batch-first pagination */
  batches: Map<string, BatchSummary>;
  /** Cache of expanded batch items (batch_id -> items) */
  batchItems: Map<string, DownloadTask[]>;
  stats: EngineStats;
  statusCounts: StatusCounts;
  pagination: PaginationState;
  loading: boolean;
  error: string | null;
}

/**
 * Create the download store
 */
function createDownloadStore() {
  const initialState: DownloadStoreState = {
    downloads: new Map(),
    batches: new Map(),
    batchItems: new Map(),
    stats: {
      active_downloads: 0,
      queued: 0,
      completed: 0,
      failed: 0,
      paused: 0,
      cancelled: 0,
      total_speed: 0,
    },
    statusCounts: {
      downloading: 0,
      queued: 0,
      paused: 0,
      completed: 0,
      failed: 0,
      cancelled: 0,
      all: 0,
    },
    pagination: {
      page: 1,
      limit: 15,
      total: 0,
      totalPages: 0,
      sortBy: 'added',
      sortDir: 'desc',
      statusFilter: null,
    },
    loading: false,
    error: null,
  };

  const { subscribe, set, update } = writable<DownloadStoreState>(initialState);

  /**
   * API Base URL
   */
  const API_BASE = '/api';

  /**
   * Fetch a page of downloads from the API (with pagination, sorting, and optional status filter)
   */
  async function fetchPage(page?: number): Promise<void> {
    update(state => ({ ...state, loading: true, error: null }));

    try {
      const currentState = get({ subscribe });
      const p = page ?? currentState.pagination.page;
      const { limit, sortBy, sortDir, statusFilter } = currentState.pagination;
      
      // Build URL with optional status filter
      let url = `${API_BASE}/downloads?page=${p}&limit=${limit}&sort_by=${sortBy}&sort_dir=${sortDir}`;
      if (statusFilter) {
        url += `&status=${statusFilter}`;
      }
      
      const response = await fetch(url);
      
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const data: DownloadsResponse = await response.json();
      
      // Convert array to Map for efficient lookups
      const downloadsMap = new Map<string, DownloadTask>();
      data.downloads.forEach((download: DownloadTask) => {
        downloadsMap.set(download.id, download);
      });

      update(state => ({
        ...state,
        downloads: downloadsMap,
        stats: data.stats,
        statusCounts: data.status_counts || state.statusCounts,
        pagination: {
          ...state.pagination,
          page: data.page,
          total: data.total,
          totalPages: data.total_pages,
        },
        loading: false,
      }));

      console.log('[DownloadStore] Fetched page', data.page, 'of', data.total_pages, '(', data.downloads.length, 'items, total:', data.total, statusFilter ? `, filter: ${statusFilter}` : '', ')');
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to fetch downloads';
      console.error('[DownloadStore] Fetch error:', errorMsg);
      
      update(state => ({
        ...state,
        loading: false,
        error: errorMsg,
      }));
    }
  }

  /**
   * Fetch batch summaries and standalone downloads (batch-first pagination)
   */
  async function fetchBatches(page?: number): Promise<void> {
    let pageToFetch = page ?? 1;
    let limit = 15;
    let sortBy: 'added' | 'status' | 'filename' | 'size' | 'progress' = 'added';
    let sortDir: 'asc' | 'desc' = 'desc';
    let statusFilter: DownloadState | null = null;

    // Get current pagination state
    update(state => {
      pageToFetch = page ?? state.pagination.page;
      limit = state.pagination.limit;
      sortBy = state.pagination.sortBy;
      sortDir = state.pagination.sortDir;
      statusFilter = state.pagination.statusFilter ?? null;
      return { ...state, loading: true, error: null };
    });



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

      // Convert batches array to Map
      const batchesMap = new Map<string, BatchSummary>();
      data.batches.forEach((batch: BatchSummary) => {
        batchesMap.set(batch.batch_id, batch);
      });


      update(state => {
        // Merge standalone downloads instead of blindly replacing state.downloads.
        //
        // Problem: fetchBatches() used to set downloads = downloadsMap (standalone only).
        // Tasks that arrived via TASK_BATCH_UPDATE WebSocket *before* this HTTP response
        // were already in state.downloads. Replacing wholesale wiped them → dashboard
        // appeared empty right after the first batch-update tick.
        //
        // Strategy:
        //   1. Start from the existing map (keeps in-flight WS tasks)
        //   2. Upsert API-fresh standalone entries (authoritative for non-active tasks)
        //   3. Remove tasks whose batch_id is now covered by a batch summary row
        //      (they moved standalone → batch and should no longer show in downloads)
        const newDownloads = new Map<string, DownloadTask>(state.downloads);

        data.standalone.forEach((download: DownloadTask) => {
          newDownloads.set(download.id, download);
        });

        // Remove tasks that belong to a batch we now know about
        for (const [id, task] of newDownloads) {
          if (task.batch_id && batchesMap.has(task.batch_id)) {
            newDownloads.delete(id);
          }
        }

        return {
          ...state,
          batches: batchesMap,
          downloads: newDownloads,
          stats: data.stats,
          statusCounts: data.status_counts || state.statusCounts,
          pagination: {
            ...state.pagination,
            page: data.page,
            total: data.total,
            totalPages: data.total_pages,
          },
          loading: false,
        };
      });


      console.log('[DownloadStore] Fetched batches page', data.page, ':', data.batches.length, 'batches +', data.standalone.length, 'standalone (total:', data.total, ')');
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to fetch batches';
      console.error('[DownloadStore] Fetch batches error:', errorMsg);

      update(state => ({
        ...state,
        loading: false,
        error: errorMsg,
      }));
    }
  }

  /**
   * Fetch items for a specific batch (lazy loading)
   */
  async function fetchBatchItems(batchId: string): Promise<DownloadTask[]> {
    // Check cache first
    let cached: DownloadTask[] | undefined;
    update(state => {
      cached = state.batchItems.get(batchId);
      return state;
    });
    if (cached) {
      console.log('[DownloadStore] Using cached items for batch', batchId);
      return cached;
    }

    try {
      const response = await fetch(`/api/downloads/batch/${batchId}/items`);
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const items: DownloadTask[] = await response.json();

      // Cache the items
      update(state => {
        const newBatchItems = new Map(state.batchItems);
        newBatchItems.set(batchId, items);
        return {
          ...state,
          batchItems: newBatchItems,
        };
      });

      console.log('[DownloadStore] Fetched', items.length, 'items for batch', batchId);
      return items;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to fetch batch items';
      console.error('[DownloadStore] Fetch batch items error:', errorMsg);
      throw err;
    }
  }

  /**
   * Fetch all downloads (alias for fetchPage with page 1)
   */
  async function fetchAll(): Promise<void> {
    return fetchBatches(1); // Use batch-first pagination by default
  }

  /**
   * Set pagination parameters and refetch
   */
  async function setPage(page: number): Promise<void> {
    update(state => ({
      ...state,
      pagination: { ...state.pagination, page }
    }));
    return fetchBatches(page); // Use batch-first pagination
  }

  /**
   * Set sort parameters and refetch from page 1
   */
  async function setSort(sortBy: PaginationState['sortBy'], sortDir: PaginationState['sortDir']): Promise<void> {
    update(state => ({
      ...state,
      pagination: { ...state.pagination, sortBy, sortDir, page: 1 }
    }));
    return fetchBatches(1);
  }

  /**
   * Set items per page and refetch from page 1
   */
  async function setLimit(limit: number): Promise<void> {
    update(state => ({
      ...state,
      pagination: { ...state.pagination, limit, page: 1 }
    }));
    return fetchBatches(1);
  }

  /**
   * Set status filter and refetch from page 1
   * @param status - Status to filter by, or null/undefined for all
   */
  async function setStatusFilter(status: DownloadState | null): Promise<void> {
    update(state => ({
      ...state,
      pagination: { ...state.pagination, statusFilter: status, page: 1 }
    }));
    return fetchBatches(1);
  }

  /**
   * Add a new download
   */
  async function addDownload(request: AddDownloadRequest): Promise<ApiResponse<AddDownloadResponse>> {
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

      console.log('[DownloadStore] Added download:', data.task_id);
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
  async function pauseDownload(id: string): Promise<ApiResponse<void>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/${id}/pause`, {
        method: 'POST',
      });

      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      console.log('[DownloadStore] Paused download:', id);
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
  async function resumeDownload(id: string): Promise<ApiResponse<void>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/${id}/resume`, {
        method: 'POST',
      });

      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      console.log('[DownloadStore] Resumed download:', id);
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
  async function deleteDownload(id: string): Promise<ApiResponse<void>> {
    try {
      // Capture batch_id BEFORE deleting from the map
      // Items may be in downloads map OR in batchItems cache (for expanded batch children)
      let batchId: string | undefined;
      update(state => {
        const task = state.downloads.get(id);
        if (task?.batch_id) {
          batchId = task.batch_id;
        } else {
          // Search in batchItems cache (batch children aren't always in downloads map)
          for (const [bid, items] of state.batchItems) {
            if (items.some(item => item.id === id)) {
              batchId = bid;
              break;
            }
          }
        }
        return state;
      });

      const response = await fetch(`${API_BASE}/downloads/${id}`, {
        method: 'DELETE',
      });

      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      // Remove from local store immediately for instant UI feedback
      update(state => {
        const newDownloads = new Map(state.downloads);
        newDownloads.delete(id);

        // Also remove from batch items cache if it was a batch item
        const newBatchItems = new Map(state.batchItems);
        if (batchId) {
          const items = newBatchItems.get(batchId);
          if (items) {
            newBatchItems.set(batchId, items.filter(item => item.id !== id));
          }
        }

        return { ...state, downloads: newDownloads, batchItems: newBatchItems };
      });

      console.log('[DownloadStore] Deleted download:', id);
      toasts.info('Download deleted');

      // If it was a batch item, refetch batch summaries to update counts/progress
      if (batchId) {
        console.log('[DownloadStore] Batch item deleted, refetching batches...');
        fetchBatches().catch(err => console.error('Failed to refetch batches after delete:', err));
      }

      // If the deleted task was COMPLETED it may have been imported into the Arr
      // library — refresh library overview so dashboard counts stay in sync.
      {
        let wasCompleted = false;
        update(state => {
          // Task already removed from map, check batchItems cache too
          wasCompleted = false; // can't re-check after removal; we captured state above
          return state;
        });
        // We check the pre-deletion state stored in the captured batchId search loop above.
        // Simplest reliable approach: always refresh after any delete since the cost is one
        // lightweight /api/arr/library call and the user explicitly requested sync.
        fetchLibraryOverview().catch(() => {/* silently ignore if Arr is offline */});
      }

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
  async function retryDownload(id: string): Promise<ApiResponse<void>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/${id}/retry`, {
        method: 'POST',
      });

      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      console.log('[DownloadStore] Retrying download:', id);
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
   * Pause all active downloads
   */
  async function pauseAll(): Promise<ApiResponse<{ affected: number }>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/pause-all`, {
        method: 'POST',
      });

      const data = await response.json();

      if (!response.ok) {
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      console.log('[DownloadStore] Paused all downloads:', data.affected);
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
  async function resumeAll(): Promise<ApiResponse<{ affected: number }>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/resume-all`, {
        method: 'POST',
      });

      const data = await response.json();

      if (!response.ok) {
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      console.log('[DownloadStore] Resumed all downloads:', data.affected);
      toasts.success(`Resumed ${data.affected} download${data.affected !== 1 ? 's' : ''}`);
      return { success: true, data };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to resume all downloads';
      console.error('[DownloadStore] Resume all error:', errorMsg);
      toasts.error(`Failed to resume all: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  // ============================================================================
  // Batch Actions
  // ============================================================================

  /**
   * Pause all downloads in a batch
   */
  async function pauseBatch(batchId: string): Promise<ApiResponse<{ affected: number }>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/batch/${batchId}/pause`, {
        method: 'POST',
      });

      const data = await response.json();

      if (!response.ok) {
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      console.log('[DownloadStore] Paused batch:', batchId, 'affected:', data.affected);
      toasts.info(`Paused ${data.affected} download${data.affected !== 1 ? 's' : ''} in batch`);
      
      // Refetch batches to update batch row status
      fetchBatches().catch(err => console.error('Failed to refetch batches after pause:', err));
      
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
  async function resumeBatch(batchId: string): Promise<ApiResponse<{ affected: number }>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/batch/${batchId}/resume`, {
        method: 'POST',
      });

      const data = await response.json();

      if (!response.ok) {
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      console.log('[DownloadStore] Resumed batch:', batchId, 'affected:', data.affected);
      toasts.success(`Resumed ${data.affected} download${data.affected !== 1 ? 's' : ''} in batch`);
      
      // Refetch batches to update batch row status
      fetchBatches().catch(err => console.error('Failed to refetch batches after resume:', err));
      
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
  async function deleteBatch(batchId: string): Promise<ApiResponse<{ affected: number }>> {
    try {
      const response = await fetch(`${API_BASE}/downloads/batch/${batchId}`, {
        method: 'DELETE',
      });

      const data = await response.json();

      if (!response.ok) {
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      console.log('[DownloadStore] Deleted batch:', batchId, 'affected:', data.affected);
      toasts.info(`Deleted ${data.affected} download${data.affected !== 1 ? 's' : ''} in batch`);
      
      // Refetch batches to get fresh data from server and update UI
      await fetchBatches();

      // Batch may contain completed items that were imported — sync library counts.
      fetchLibraryOverview().catch(() => {});
      
      return { success: true, data };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to delete batch';
      toasts.error(`Failed to delete batch: ${errorMsg}`);
      return { success: false, error: errorMsg };
    }
  }

  /**
   * Clear error_message when a task enters a working/active state.
   * The backend sometimes retains the old error string in the payload even after
   * a successful resume/retry, so we sanitize it on the frontend side.
   */
  function clearErrorIfActive(task: DownloadTask): DownloadTask {
    const activeStates: DownloadState[] = ['QUEUED', 'STARTING', 'DOWNLOADING', 'EXTRACTING', 'WAITING'];
    if (activeStates.includes(task.state)) {
      return { ...task, error_message: null };
    }
    return task;
  }

  /**
   * Map a DownloadState to the corresponding BatchSummary counter field.
   * Used to apply instant delta patches when individual tasks change state
   * so the batch header row syncs without waiting for a fetchBatches() call.
   */
  function getStateBucket(
    state: DownloadState
  ): 'downloading_items' | 'paused_items' | 'queued_items' | 'completed_items' | 'failed_items' | null {
    if (state === 'DOWNLOADING' || state === 'STARTING' || state === 'EXTRACTING') return 'downloading_items';
    if (state === 'PAUSED') return 'paused_items';
    if (state === 'QUEUED' || state === 'WAITING') return 'queued_items';
    if (state === 'COMPLETED') return 'completed_items';
    if (state === 'FAILED' || state === 'CANCELLED' || state === 'SKIPPED') return 'failed_items';
    return null;
  }

  /**
   * Apply a delta patch to a BatchSummary in the batches Map when a task state changes.
   * Decrements the old bucket, increments the new one — O(1), no API call.
   */
  function patchBatchCounts(
    batches: Map<string, BatchSummary>,
    batchId: string,
    fromState: DownloadState | undefined,
    toState: DownloadState
  ): Map<string, BatchSummary> {
    const batch = batches.get(batchId);
    if (!batch || fromState === toState) return batches;

    const fromBucket = fromState ? getStateBucket(fromState) : null;
    const toBucket = getStateBucket(toState);
    if (fromBucket === toBucket) return batches; // same bucket, no change needed

    const newBatches = new Map(batches);
    const updated = { ...batch };
    if (fromBucket) updated[fromBucket] = Math.max(0, updated[fromBucket] - 1);
    if (toBucket) updated[toBucket] = updated[toBucket] + 1;
    newBatches.set(batchId, updated);
    return newBatches;
  }

  /**
   * Recalculate live batch metrics (progress, speed, downloaded_size, total_size)
   * from all cached items in batchItems. Called after TASK_BATCH_UPDATE / TASK_UPDATED
   * so the batch header row stays in sync without a fetchBatches() round-trip.
   *
   * Only patches when cachedItems.length > 0 and the batch exists in the map.
   */
  function patchBatchMetrics(
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
      if (item.state === 'DOWNLOADING' || item.state === 'STARTING' || item.state === 'EXTRACTING') {
        totalSpeed += item.speed ?? 0;
      }
    }

    // Only override server total_size if we have reliable data (all items loaded)
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

  /**
   * Initialize WebSocket handlers
   */
  function initWebSocket(): void {
    // Handle SYNC_ALL - server sends only ACTIVE tasks on WS connect.
    // Historical tasks are loaded via paginated REST. We must MERGE (upsert)
    // into the existing Map rather than replace it, otherwise REST-loaded tasks
    // vanish the moment the WebSocket fires — causing the Active Queue blink.
    wsClient.on('SYNC_ALL', (message) => {
      console.log('[DownloadStore] SYNC_ALL received:', message.tasks?.length || 0, 'active tasks');
      
      if (message.tasks && Array.isArray(message.tasks)) {
        update(state => {
          // Upsert active tasks into the existing map — don't replace the whole map.
          // The server explicitly sends only active tasks here; the rest of the
          // map comes from fetchDownloads() pagination calls.
          const newDownloads = new Map(state.downloads);
          (message.tasks as DownloadTask[]).forEach((task: DownloadTask) => {
            newDownloads.set(task.id, task);
          });
          return { ...state, downloads: newDownloads };
        });
      }
    });

    // Handle TASK_ADDED - new task added
    wsClient.on('TASK_ADDED', (message) => {
      console.log('[DownloadStore] TASK_ADDED:', message.task?.id);
      
      if (message.task) {
        update(state => {
          const newDownloads = new Map(state.downloads);
          newDownloads.set(message.task.id, message.task);
          return { ...state, downloads: newDownloads };
        });

        // If task has batch_id, refetch batches to update grouping
        if (message.task.batch_id) {
          console.log('[DownloadStore] Task added to batch, refetching batches...');
          fetchBatches().catch(err => console.error('Failed to refetch batches:', err));
        }
      }
    });

    // Handle TASK_UPDATED - task state/progress changed
    wsClient.on('TASK_UPDATED', (message) => {
      if (!message.task || !message.task.id) return;

      const taskId: string = message.task.id;
      const batchId: string | undefined = message.task.batch_id;
      const newState: DownloadState = message.task.state;

      update(state => {
        // --- 1. Resolve previous state ---
        // Batch items are NOT in the downloads Map (only standalone items are).
        // Look up previous state from batchItems cache when not found in downloads.
        const existingInDownloads = state.downloads.get(taskId);
        const existingInBatch = batchId
          ? state.batchItems.get(batchId)?.find(i => i.id === taskId)
          : undefined;
        const previousState: DownloadState | undefined =
          existingInDownloads?.state ?? existingInBatch?.state;

        // --- 2. Update downloads Map (standalone items) ---
        const newDownloads = new Map(state.downloads);
        // Apply error-clearing: remove stale error_message when task enters an active state
        const incomingTask = clearErrorIfActive(message.task);
        if (existingInDownloads) {
          newDownloads.set(taskId, { ...existingInDownloads, ...incomingTask });
        } else if (!batchId) {
          // Only add to downloads Map if it's NOT a batch item
          newDownloads.set(taskId, incomingTask);
        }

        let newBatchItems = state.batchItems;
        let newBatches = state.batches;

        if (batchId) {
          // --- 3. Patch individual item in batchItems cache ---
          const cachedItems = state.batchItems.get(batchId);
          if (cachedItems) {
            const updated = cachedItems.map(item =>
              item.id === taskId ? { ...item, ...incomingTask } : item
            );
            newBatchItems = new Map(state.batchItems);
            newBatchItems.set(batchId, updated);

            // --- 5. Recalculate live metrics (progress, speed, downloaded_size) ---
            newBatches = patchBatchMetrics(newBatches, batchId, updated);
          }

          // --- 4. Delta-patch the BatchSummary so the batch header row syncs immediately ---
          if (previousState !== newState) {
            newBatches = patchBatchCounts(newBatches, batchId, previousState, newState);
          }
        }

        return { ...state, downloads: newDownloads, batchItems: newBatchItems, batches: newBatches };
      });

      // Full server resync only for terminal state changes
      if (batchId && (newState === 'COMPLETED' || newState === 'FAILED')) {
        console.log('[DownloadStore] Task state changed to', newState, 'refetching batches...');
        fetchBatches().catch(err => console.error('Failed to refetch batches:', err));
      }
    });

    // Handle TASK_BATCH_UPDATE - batch of task updates (sent every 500ms)
    wsClient.on('TASK_BATCH_UPDATE', (message) => {
      if (!message.tasks || !Array.isArray(message.tasks) || message.tasks.length === 0) return;

      update(state => {
        const newDownloads = new Map(state.downloads);
        const newBatchItems = new Map(state.batchItems);
        let newBatches = state.batches;

        for (const task of message.tasks) {
          if (!task || !task.id) continue;

          // Resolve previous state: check downloads Map first, then batchItems cache
          const existingInDownloads = newDownloads.get(task.id);
          const existingInBatch = task.batch_id
            ? newBatchItems.get(task.batch_id)?.find((i: DownloadTask) => i.id === task.id)
            : undefined;
          const previousState: DownloadState | undefined =
            existingInDownloads?.state ?? existingInBatch?.state;

          // Clear error_message when task enters an active state (same as TASK_UPDATED)
          const incomingTask = clearErrorIfActive(task);

          // Update downloads Map (standalone items only)
          if (existingInDownloads) {
            newDownloads.set(task.id, { ...existingInDownloads, ...incomingTask });
          } else if (!task.batch_id) {
            newDownloads.set(task.id, incomingTask);
          }

          if (task.batch_id) {
            // Update batchItems cache for expanded rows
            const cachedItems = newBatchItems.get(task.batch_id);
            if (cachedItems) {
              newBatchItems.set(
                task.batch_id,
                cachedItems.map((item: DownloadTask) => (item.id === task.id ? { ...item, ...incomingTask } : item)),
              );
            }

            // Delta-patch batch summary counts on state transition
            if (previousState !== task.state) {
              newBatches = patchBatchCounts(newBatches, task.batch_id, previousState, task.state);
            }
          }
        }

        // Recalculate live metrics (progress, speed, downloaded_size) for every
        // affected batch that has items in cache — O(batches * items), runs every 500ms.
        const affectedBatchIds: Set<string> = new Set(
          (message.tasks as DownloadTask[])
            .filter(t => t?.batch_id)
            .map(t => t.batch_id as string)
        );
        for (const bid of affectedBatchIds) {
          const items = newBatchItems.get(bid);
          if (items) {
            newBatches = patchBatchMetrics(newBatches, bid, items);
          }
        }

        return { ...state, downloads: newDownloads, batchItems: newBatchItems, batches: newBatches };
      });
    });

    // Handle TASK_REMOVED - task deleted
    wsClient.on('TASK_REMOVED', (message) => {
      console.log('[DownloadStore] TASK_REMOVED:', message.task_id);
      
      if (message.task_id) {
        // Check if this was part of a batch before removing
        let wasBatchItem = false;
        let batchId: string | undefined;
        
        update(state => {
          const task = state.downloads.get(message.task_id);
          if (task?.batch_id) {
            wasBatchItem = true;
            batchId = task.batch_id;
          }

          const newDownloads = new Map(state.downloads);
          newDownloads.delete(message.task_id);

          // Also remove from batch items cache if present
          const newBatchItems = new Map(state.batchItems);
          if (batchId) {
            const items = newBatchItems.get(batchId);
            if (items) {
              newBatchItems.set(batchId, items.filter(item => item.id !== message.task_id));
            }
          }

          return { ...state, downloads: newDownloads, batchItems: newBatchItems };
        });

        // If it was a batch item or if we need to check for empty batches, refetch
        if (wasBatchItem) {
          console.log('[DownloadStore] Batch item removed, refetching batches...');
          fetchBatches().catch(err => console.error('Failed to refetch batches:', err));
        }

        // Sync library overview whenever a task is removed — it may have been
        // a COMPLETED item that was imported, affecting counts on the dashboard.
        fetchLibraryOverview().catch(() => {});
      }
    });

    // Handle ENGINE_STATS - engine statistics update
    wsClient.on('ENGINE_STATS', (message) => {
      if (message.stats) {
        update(state => ({
          ...state,
          stats: message.stats,
        }));
      }
    });

    console.log('[DownloadStore] WebSocket handlers initialized');
  }

  return {
    subscribe,
    // Data fetching
    fetchAll,
    fetchPage,
    fetchBatches,
    fetchBatchItems,
    // Pagination
    setPage,
    setLimit,
    setSort,
    setStatusFilter,
    // Actions
    addDownload,
    pauseDownload,
    resumeDownload,
    deleteDownload,
    retryDownload,
    pauseAll,
    resumeAll,
    pauseBatch,
    resumeBatch,
    deleteBatch,
    initWebSocket,
  };
}

/**
 * Global download store instance
 */
export const downloadStore = createDownloadStore();

/**
 * Derived store: Downloads as array (sorted by created_at desc)
 */
export const downloads = derived(
  downloadStore,
  $store => Array.from($store.downloads.values())
    .sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime())
);

/**
 * Derived store: Batches as array (sorted by created_at desc)
 */
export const batches = derived(
  downloadStore,
  $store => Array.from($store.batches.values())
    .sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime())
);

/**
 * Derived store: Active downloads
 */
export const activeDownloads = derived(
  downloads,
  $downloads => $downloads.filter(d => 
    d.state === 'DOWNLOADING' || d.state === 'STARTING'
  )
);

/**
 * Derived store: Queued downloads
 */
export const queuedDownloads = derived(
  downloads,
  $downloads => $downloads.filter(d => d.state === 'QUEUED')
);

/**
 * Derived store: Completed downloads
 */
export const completedDownloads = derived(
  downloads,
  $downloads => $downloads.filter(d => d.state === 'COMPLETED')
);

/**
 * Derived store: Failed downloads
 */
export const failedDownloads = derived(
  downloads,
  $downloads => $downloads.filter(d => d.state === 'FAILED')
);

/**
 * Derived store: Paused downloads
 */
export const pausedDownloads = derived(
  downloads,
  $downloads => $downloads.filter(d => d.state === 'PAUSED')
);

/**
 * Derived store: Engine stats
 */
export const engineStats = derived(
  downloadStore,
  $store => $store.stats
);

/**
 * Derived store: Status counts from database (for filter dropdown)
 */
export const statusCounts = derived(
  downloadStore,
  $store => $store.statusCounts
);

/**
 * Derived store: Pagination state
 */
export const paginationState = derived(
  downloadStore,
  $store => $store.pagination
);

/**
 * Derived store: Loading state
 */
export const isLoading = derived(
  downloadStore,
  $store => $store.loading
);

/**
 * Derived store: Error state
 */
export const storeError = derived(
  downloadStore,
  $store => $store.error
);

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

/**
 * Helper: Get state icon
 */
export function getStateIcon(state: DownloadState): string {
  switch (state) {
    case 'DOWNLOADING':
      return '⬇️';
    case 'STARTING':
      return '🔄';
    case 'COMPLETED':
      return '✅';
    case 'FAILED':
      return '❌';
    case 'PAUSED':
      return '⏸️';
    case 'QUEUED':
      return '⏳';
    case 'WAITING':
      return '⏱️';
    case 'CANCELLED':
      return '🚫';
    case 'EXTRACTING':
      return '📦';
    default:
      return '❓';
  }
}
