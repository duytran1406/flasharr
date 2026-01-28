import { writable, derived, get } from 'svelte/store';
import { wsClient } from './websocket';
import { toasts } from './toasts';

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
}

/**
 * Engine Statistics (from contract-fe-bridge.md)
 */
export interface EngineStats {
  active_downloads: number;
  queued: number;
  completed: number;
  failed: number;
  total_speed: number;
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
}

interface AddDownloadResponse {
  success: boolean;
  task_id: string;
  filename: string;
  state: string;
}

/**
 * Download Store State
 */
interface DownloadStoreState {
  downloads: Map<string, DownloadTask>;
  stats: EngineStats;
  loading: boolean;
  error: string | null;
}

/**
 * Create the download store
 */
function createDownloadStore() {
  const initialState: DownloadStoreState = {
    downloads: new Map(),
    stats: {
      active_downloads: 0,
      queued: 0,
      completed: 0,
      failed: 0,
      total_speed: 0,
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
   * Fetch all downloads from API
   */
  async function fetchAll(): Promise<void> {
    update(state => ({ ...state, loading: true, error: null }));

    try {
      const response = await fetch(`${API_BASE}/downloads`);
      
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const data: DownloadsResponse = await response.json();
      
      // Convert array to Map for efficient lookups
      const downloadsMap = new Map<string, DownloadTask>();
      data.downloads.forEach(download => {
        downloadsMap.set(download.id, download);
      });

      update(state => ({
        ...state,
        downloads: downloadsMap,
        stats: data.stats,
        loading: false,
      }));

      console.log('[DownloadStore] Fetched', downloadsMap.size, 'downloads');
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
        return { ...state, downloads: newDownloads };
      });

      console.log('[DownloadStore] Deleted download:', id);
      toasts.info('Download deleted');
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

  /**
   * Initialize WebSocket handlers
   */
  function initWebSocket(): void {
    // Handle SYNC_ALL - full sync of all tasks
    wsClient.on('SYNC_ALL', (message) => {
      console.log('[DownloadStore] SYNC_ALL received:', message.tasks?.length || 0, 'tasks');
      
      if (message.tasks && Array.isArray(message.tasks)) {
        const downloadsMap = new Map<string, DownloadTask>();
        message.tasks.forEach((task: DownloadTask) => {
          downloadsMap.set(task.id, task);
        });

        update(state => ({
          ...state,
          downloads: downloadsMap,
        }));
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
      }
    });

    // Handle TASK_UPDATED - task state/progress changed
    wsClient.on('TASK_UPDATED', (message) => {
      if (message.task && message.task.id) {
        update(state => {
          const newDownloads = new Map(state.downloads);
          const existing = newDownloads.get(message.task.id);
          
          if (existing) {
            // Merge updates (delta update)
            newDownloads.set(message.task.id, {
              ...existing,
              ...message.task,
            });
          } else {
            // Task doesn't exist, add it
            newDownloads.set(message.task.id, message.task);
          }
          
          return { ...state, downloads: newDownloads };
        });
      }
    });

    // Handle TASK_REMOVED - task deleted
    wsClient.on('TASK_REMOVED', (message) => {
      console.log('[DownloadStore] TASK_REMOVED:', message.task_id);
      
      if (message.task_id) {
        update(state => {
          const newDownloads = new Map(state.downloads);
          newDownloads.delete(message.task_id);
          return { ...state, downloads: newDownloads };
        });
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
    fetchAll,
    addDownload,
    pauseDownload,
    resumeDownload,
    deleteDownload,
    retryDownload,
    pauseAll,
    resumeAll,
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
