import { writable, derived } from 'svelte/store';

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

/**
 * System Store State
 */
interface SystemStoreState {
  downloadSettings: DownloadSettings;
  indexerSettings: IndexerSettings;
  sonarrSettings: SonarrSettings;
  radarrSettings: RadarrSettings;
  logs: LogEntry[];
  loading: boolean;
  error: string | null;
}

/**
 * Create the system store
 */
function createSystemStore() {
  const initialState: SystemStoreState = {
    downloadSettings: {
      directory: '/downloads',
      max_concurrent: 3,
      segments_per_download: 4,
    },
    indexerSettings: {
      api_key: '',
      indexer_url: '',
    },
    sonarrSettings: {
      enabled: false,
      url: 'http://localhost:8989',
      api_key: '',
      auto_import: true,
    },
    radarrSettings: {
      enabled: false,
      url: 'http://localhost:7878',
      api_key: '',
      auto_import: true,
    },
    logs: [],
    loading: false,
    error: null,
  };

  const { subscribe, set, update } = writable<SystemStoreState>(initialState);

  const API_BASE = '/api';

  /**
   * Fetch download settings
   */
  async function fetchDownloadSettings(): Promise<void> {
    try {
      const response = await fetch(`${API_BASE}/settings/downloads`);
      if (response.ok) {
        const data = await response.json();
        update(state => ({
          ...state,
          downloadSettings: data,
        }));
      }
    } catch (err) {
      console.error('[SystemStore] Fetch download settings error:', err);
    }
  }

  /**
   * Save download settings
   */
  async function saveDownloadSettings(settings: DownloadSettings): Promise<{ success: boolean; message?: string }> {
    try {
      const response = await fetch(`${API_BASE}/settings/downloads`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(settings),
      });

      if (response.ok) {
        const data = await response.json();
        update(state => ({
          ...state,
          downloadSettings: settings,
        }));
        return { success: data.success, message: data.message };
      }
      return { success: false, message: 'Failed to save settings' };
    } catch (err) {
      console.error('[SystemStore] Save download settings error:', err);
      return { success: false, message: 'Network error' };
    }
  }

  /**
   * Fetch indexer settings
   */
  async function fetchIndexerSettings(): Promise<void> {
    try {
      const response = await fetch(`${API_BASE}/settings/indexer`);
      if (response.ok) {
        const data = await response.json();
        update(state => ({
          ...state,
          indexerSettings: data,
        }));
      }
    } catch (err) {
      console.error('[SystemStore] Fetch indexer settings error:', err);
    }
  }

  /**
   * Generate new indexer API key
   */
  async function generateIndexerApiKey(): Promise<string | null> {
    try {
      const response = await fetch(`${API_BASE}/settings/indexer/generate-key`);
      if (response.ok) {
        const data = await response.json();
        update(state => ({
          ...state,
          indexerSettings: {
            ...state.indexerSettings,
            api_key: data.api_key,
          },
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
   * Fetch Sonarr settings
   */
  async function fetchSonarrSettings(): Promise<void> {
    try {
      const response = await fetch(`${API_BASE}/settings/sonarr`);
      if (response.ok) {
        const data = await response.json();
        update(state => ({
          ...state,
          sonarrSettings: data,
        }));
      }
    } catch (err) {
      console.error('[SystemStore] Fetch Sonarr settings error:', err);
    }
  }

  /**
   * Save Sonarr settings
   */
  async function saveSonarrSettings(settings: SonarrSettings): Promise<{ success: boolean; message?: string }> {
    try {
      const response = await fetch(`${API_BASE}/settings/sonarr`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(settings),
      });

      if (response.ok) {
        const data = await response.json();
        update(state => ({
          ...state,
          sonarrSettings: settings,
        }));
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
  async function testSonarrConnection(settings: SonarrSettings): Promise<{ success: boolean; message?: string }> {
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
   * Fetch Radarr settings
   */
  async function fetchRadarrSettings(): Promise<void> {
    try {
      const response = await fetch(`${API_BASE}/settings/radarr`);
      if (response.ok) {
        const data = await response.json();
        update(state => ({
          ...state,
          radarrSettings: data,
        }));
      }
    } catch (err) {
      console.error('[SystemStore] Fetch Radarr settings error:', err);
    }
  }

  /**
   * Save Radarr settings
   */
  async function saveRadarrSettings(settings: RadarrSettings): Promise<{ success: boolean; message?: string }> {
    try {
      const response = await fetch(`${API_BASE}/settings/radarr`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(settings),
      });

      if (response.ok) {
        const data = await response.json();
        update(state => ({
          ...state,
          radarrSettings: settings,
        }));
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
  async function testRadarrConnection(settings: RadarrSettings): Promise<{ success: boolean; message?: string }> {
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
   * Fetch system logs
   */
  async function fetchLogs(lines: number = 50): Promise<void> {
    try {
      const response = await fetch(`${API_BASE}/system/logs?lines=${lines}`);
      if (response.ok) {
        const data = await response.json();
        update(state => ({
          ...state,
          logs: data.logs || [],
        }));
      }
    } catch (err) {
      console.error('[SystemStore] Fetch logs error:', err);
    }
  }

  /**
   * Clear logs (client-side only)
   */
  function clearLogs(): void {
    update(state => ({
      ...state,
      logs: [],
    }));
  }

  return {
    subscribe,
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
}

/**
 * Global system store instance
 */
export const systemStore = createSystemStore();

/**
 * Derived stores
 */
export const downloadSettings = derived(
  systemStore,
  $store => $store.downloadSettings
);

export const indexerSettings = derived(
  systemStore,
  $store => $store.indexerSettings
);

export const sonarrSettings = derived(
  systemStore,
  $store => $store.sonarrSettings
);

export const radarrSettings = derived(
  systemStore,
  $store => $store.radarrSettings
);

export const systemLogs = derived(
  systemStore,
  $store => $store.logs
);
