import { writable } from 'svelte/store';

/**
 * Download Settings Interface
 */
export interface DownloadSettings {
  directory: string;
  max_concurrent: number;
  segments_per_download: number;
}

/**
 * Indexer Settings Interface
 */
export interface IndexerSettings {
  api_key: string;
  indexer_url: string;
}

/**
 * Sonarr Settings Interface
 */
export interface SonarrSettings {
  enabled: boolean;
  url: string;
  api_key: string;
  auto_import: boolean;
}

/**
 * Radarr Settings Interface
 */
export interface RadarrSettings {
  enabled: boolean;
  url: string;
  api_key: string;
  auto_import: boolean;
}

/**
 * Log Entry Interface
 */
export interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
}

// ============================================================================
// SEPARATE INDEPENDENT STORES
// Each store is independent - updating one does NOT trigger others to re-emit
// ============================================================================

/**
 * Download Settings Store
 */
export const downloadSettings = writable<DownloadSettings>({
  directory: '/downloads',
  max_concurrent: 3,
  segments_per_download: 4,
});

/**
 * Indexer Settings Store
 */
export const indexerSettings = writable<IndexerSettings>({
  api_key: '',
  indexer_url: '',
});

/**
 * Sonarr Settings Store
 */
export const sonarrSettings = writable<SonarrSettings>({
  enabled: false,
  url: 'http://localhost:8989',
  api_key: '',
  auto_import: true,
});

/**
 * Radarr Settings Store
 */
export const radarrSettings = writable<RadarrSettings>({
  enabled: false,
  url: 'http://localhost:7878',
  api_key: '',
  auto_import: true,
});

/**
 * System Logs Store
 */
export const systemLogs = writable<LogEntry[]>([]);

/**
 * Loading State Store
 */
export const systemLoading = writable<boolean>(false);

/**
 * Error State Store
 */
export const systemError = writable<string | null>(null);

// ============================================================================
// API FUNCTIONS
// Each function only updates its specific store
// ============================================================================

const API_BASE = '/api';

/**
 * Fetch download settings - ONLY updates downloadSettings store
 */
export async function fetchDownloadSettings(): Promise<void> {
  try {
    const response = await fetch(`${API_BASE}/settings/downloads`);
    if (response.ok) {
      const data = await response.json();
      downloadSettings.set(data);
    }
  } catch (err) {
    console.error('[SystemStore] Fetch download settings error:', err);
  }
}

/**
 * Save download settings
 */
export async function saveDownloadSettings(settings: DownloadSettings): Promise<{ success: boolean; message?: string }> {
  try {
    const response = await fetch(`${API_BASE}/settings/downloads`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(settings),
    });

    if (response.ok) {
      const data = await response.json();
      downloadSettings.set(settings);
      return { success: data.success, message: data.message };
    }
    return { success: false, message: 'Failed to save settings' };
  } catch (err) {
    console.error('[SystemStore] Save download settings error:', err);
    return { success: false, message: 'Network error' };
  }
}

/**
 * Fetch indexer settings - ONLY updates indexerSettings store
 */
export async function fetchIndexerSettings(): Promise<void> {
  try {
    const response = await fetch(`${API_BASE}/settings/indexer`);
    if (response.ok) {
      const data = await response.json();
      indexerSettings.set(data);
    }
  } catch (err) {
    console.error('[SystemStore] Fetch indexer settings error:', err);
  }
}

/**
 * Generate new indexer API key
 */
export async function generateIndexerApiKey(): Promise<string | null> {
  try {
    const response = await fetch(`${API_BASE}/settings/indexer/generate-key`);
    if (response.ok) {
      const data = await response.json();
      indexerSettings.update(state => ({
        ...state,
        api_key: data.api_key,
      }));
      return data.api_key;
    }
    return null;
  } catch (err) {
    console.error('[SystemStore] Generate API key error:', err);
    return null;
  }
}

/**
 * Fetch Sonarr settings - ONLY updates sonarrSettings store
 */
export async function fetchSonarrSettings(): Promise<void> {
  try {
    const response = await fetch(`${API_BASE}/settings/sonarr`);
    if (response.ok) {
      const data = await response.json();
      sonarrSettings.set(data);
    }
  } catch (err) {
    console.error('[SystemStore] Fetch Sonarr settings error:', err);
  }
}

/**
 * Save Sonarr settings
 */
export async function saveSonarrSettings(settings: SonarrSettings): Promise<{ success: boolean; message?: string }> {
  try {
    const response = await fetch(`${API_BASE}/settings/sonarr`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(settings),
    });

    if (response.ok) {
      const data = await response.json();
      sonarrSettings.set(settings);
      return { success: data.success, message: data.message };
    }
    return { success: false };
  } catch (err) {
    console.error('[SystemStore] Save Sonarr settings error:', err);
    return { success: false };
  }
}

/**
 * Test Sonarr connection
 */
export async function testSonarrConnection(settings: SonarrSettings): Promise<{ success: boolean; message?: string }> {
  try {
    const response = await fetch(`${API_BASE}/settings/sonarr/test`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(settings),
    });

    const data = await response.json();
    return { success: data.success, message: data.message };
  } catch (err) {
    console.error('[SystemStore] Test Sonarr connection error:', err);
    return { success: false, message: 'Connection failed' };
  }
}

/**
 * Fetch Radarr settings - ONLY updates radarrSettings store
 */
export async function fetchRadarrSettings(): Promise<void> {
  try {
    const response = await fetch(`${API_BASE}/settings/radarr`);
    if (response.ok) {
      const data = await response.json();
      radarrSettings.set(data);
    }
  } catch (err) {
    console.error('[SystemStore] Fetch Radarr settings error:', err);
  }
}

/**
 * Save Radarr settings
 */
export async function saveRadarrSettings(settings: RadarrSettings): Promise<{ success: boolean; message?: string }> {
  try {
    const response = await fetch(`${API_BASE}/settings/radarr`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(settings),
    });

    if (response.ok) {
      const data = await response.json();
      radarrSettings.set(settings);
      return { success: data.success, message: data.message };
    }
    return { success: false };
  } catch (err) {
    console.error('[SystemStore] Save Radarr settings error:', err);
    return { success: false };
  }
}

/**
 * Test Radarr connection
 */
export async function testRadarrConnection(settings: RadarrSettings): Promise<{ success: boolean; message?: string }> {
  try {
    const response = await fetch(`${API_BASE}/settings/radarr/test`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(settings),
    });

    const data = await response.json();
    return { success: data.success, message: data.message };
  } catch (err) {
    console.error('[SystemStore] Test Radarr connection error:', err);
    return { success: false, message: 'Connection failed' };
  }
}

/**
 * Fetch system logs - ONLY updates systemLogs store
 * This will NOT affect sonarr/radarr/indexer settings!
 */
export async function fetchLogs(lines: number = 50): Promise<void> {
  try {
    const response = await fetch(`${API_BASE}/system/logs?lines=${lines}`);
    if (response.ok) {
      const data = await response.json();
      systemLogs.set(data.logs || []);
    }
  } catch (err) {
    console.error('[SystemStore] Fetch logs error:', err);
  }
}

/**
 * Clear logs (client-side only)
 */
export function clearLogs(): void {
  systemLogs.set([]);
}

// ============================================================================
// BACKWARD COMPATIBILITY
// Expose a systemStore object with the same API as before
// ============================================================================

export const systemStore = {
  fetchDownloadSettings,
  saveDownloadSettings,
  fetchIndexerSettings,
  generateIndexerApiKey,
  fetchSonarrSettings,
  saveSonarrSettings,
  testSonarrConnection,
  fetchRadarrSettings,
  saveRadarrSettings,
  testRadarrConnection,
  fetchLogs,
  clearLogs,
};
