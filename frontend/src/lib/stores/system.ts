import { derived, writable } from 'svelte/store';
import { wsClient } from './websocket';

// ============================================================================
// Types
// ============================================================================

export interface DownloadSettings {
  directory: string;
  max_concurrent: number;
  segments_per_download: number;
}

export interface IndexerSettings {
  /** Returned by GET /api/settings/indexer; set to true after generating a key. */
  enabled?: boolean;
  api_key: string;
  indexer_url: string;
}

export interface SonarrSettings {
  enabled: boolean;
  url: string;
  api_key: string;
  auto_import: boolean;
}

export interface RadarrSettings {
  enabled: boolean;
  url: string;
  api_key: string;
  auto_import: boolean;
}

export interface ProwlarrSettings {
  enabled: boolean;
  url: string;
  api_key: string;
  categories?: string | null;
  limit: number | null;
}

export interface QBittorrentSettings {
  enabled: boolean;
  url: string;
  username: string;
  password: string;
  category: string | null;
  save_path: string | null;
  poll_interval_secs: number;
}

export interface TorBoxSettings {
  enabled: boolean;
  base_url: string;
  api_key: string;
  cache_timeout_secs: number;
  show_cached_as_source: boolean;
}

/** GET /api/settings/torbox/profile */
export interface TorBoxProfile {
  email: string;
  plan: string;
  status: string;
  premium_expires_at: string | null;
}

/**
 * Metadata provider row (GET /api/settings/metadata, camelCase on the wire)
 * plus client-side connection-test state.
 */
export interface MetadataProvider {
  provider: string;
  configured: boolean;
  source: string;
  maskedKey: string;
  /** null = not tested in this session */
  reachable: boolean | null;
  message: string;
}

/** One service entry of GET /api/settings/integration-status */
export interface IntegrationHealth {
  enabled: boolean;
  configured: boolean;
  reachable: boolean | null;
  message: string;
  checked_at: string | null;
}

export interface IntegrationStatusState {
  sonarr: IntegrationHealth | null;
  radarr: IntegrationHealth | null;
  prowlarr: IntegrationHealth | null;
  loading: boolean;
  error: string | null;
}

export type IntegrationStatusKind =
  | 'checking'
  | 'disabled'
  | 'unconfigured'
  | 'connected'
  | 'unreachable'
  | 'unknown';

/** GET /api/health */
export interface SystemHealth {
  status: string;
  version: string;
}

/** GET /api/settings/storage */
export interface StorageStatus {
  path: string;
  mounted: boolean;
  read_only: boolean;
  total_bytes: number;
  available_bytes: number;
  used_bytes: number;
  used_percent: number;
  message: string;
}

export interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
}

/** GET /api/system/activity → activities[] */
export interface ActivityEvent {
  id: number;
  timestamp: string;
  kind: string;
  task_id: string;
  message: string;
}

export interface SaveResult {
  success: boolean;
  message?: string;
}

export interface FolderSourceImportResult {
  success: boolean;
  imported: number;
  message: string;
}

// ============================================================================
// Integration status helpers
// ============================================================================

export function integrationStatusKind(
  status: IntegrationHealth | null | undefined,
  checking = false,
): IntegrationStatusKind {
  if (checking) return 'checking';
  if (!status) return 'unknown';
  if (!status.enabled) return 'disabled';
  if (!status.configured) return 'unconfigured';
  if (status.reachable === true) return 'connected';
  if (status.reachable === false) return 'unreachable';
  return 'unknown';
}

const INTEGRATION_STATUS_LABELS: Record<IntegrationStatusKind, string> = {
  checking: 'Checking',
  disabled: 'Disabled',
  unconfigured: 'Not configured',
  connected: 'Connected',
  unreachable: 'Unreachable',
  unknown: 'Unknown',
};

export function integrationStatusLabel(
  status: IntegrationHealth | null | undefined,
  checking = false,
): string {
  return INTEGRATION_STATUS_LABELS[integrationStatusKind(status, checking)];
}

// ============================================================================
// Stores — each one independent; updating one does not re-emit the others
// ============================================================================

const DEFAULT_TORBOX_SETTINGS: TorBoxSettings = {
  enabled: false,
  base_url: 'https://api.torbox.app/v1/api',
  api_key: '',
  cache_timeout_secs: 3,
  show_cached_as_source: true,
};

export const downloadSettings = writable<DownloadSettings>({
  directory: '/downloads',
  max_concurrent: 3,
  segments_per_download: 4,
});

export const indexerSettings = writable<IndexerSettings>({
  api_key: '',
  indexer_url: '',
});

export const sonarrSettings = writable<SonarrSettings>({
  enabled: false,
  url: 'http://localhost:8989',
  api_key: '',
  auto_import: true,
});

export const radarrSettings = writable<RadarrSettings>({
  enabled: false,
  url: 'http://localhost:7878',
  api_key: '',
  auto_import: true,
});

export const prowlarrSettings = writable<ProwlarrSettings>({
  enabled: false,
  url: 'http://localhost:9696',
  api_key: '',
  categories: '',
  limit: 50,
});

/** Live reachability of Sonarr/Radarr/Prowlarr (server-side checks). */
export const integrationStatus = writable<IntegrationStatusState>({
  sonarr: null,
  radarr: null,
  prowlarr: null,
  loading: false,
  error: null,
});

function hasArrCredentials(settings: SonarrSettings | RadarrSettings): boolean {
  return !!(settings.enabled && settings.url.trim() && settings.api_key.trim());
}

/** Sonarr is enabled and has a URL + API key (client-side config check only). */
export const sonarrConfigured = derived(sonarrSettings, hasArrCredentials);
/** Radarr is enabled and has a URL + API key (client-side config check only). */
export const radarrConfigured = derived(radarrSettings, hasArrCredentials);
/** At least one of Sonarr/Radarr is configured. */
export const arrConfigured = derived(
  [sonarrConfigured, radarrConfigured],
  ([$sonarr, $radarr]) => $sonarr || $radarr,
);

export const qbittorrentSettings = writable<QBittorrentSettings>({
  enabled: false,
  url: 'http://192.168.1.112:8080',
  username: 'admin',
  password: '',
  category: '',
  save_path: '',
  poll_interval_secs: 5,
});

export const torboxSettings = writable<TorBoxSettings>({ ...DEFAULT_TORBOX_SETTINGS });
export const torboxProfile = writable<TorBoxProfile | null>(null);
export const metadataProviders = writable<MetadataProvider[]>([]);
export const systemHealth = writable<SystemHealth | null>(null);
export const storageStatus = writable<StorageStatus | null>(null);
export const systemLogs = writable<LogEntry[]>([]);
export const systemActivity = writable<ActivityEvent[]>([]);
export const folderSourceUrl = writable<string>('');
/** Number of folder sources currently imported (from config/import responses). */
export const importedFolderSources = writable<number>(0);

/** Baseline stores; never written and not referenced by the 2026-09-02 build. */
export const systemLoading = writable<boolean>(false);
export const systemError = writable<string | null>(null);

// ============================================================================
// API
// ============================================================================

const API_BASE = '/api';

/** Logs older than this ISO timestamp are hidden after clearLogs(). */
let logsClearedAt: string | null = null;

function apiHeaders(withJsonBody = false): HeadersInit {
  return withJsonBody ? { 'Content-Type': 'application/json' } : {};
}

function isJsonResponse(response: Response): boolean {
  return (response.headers.get('content-type') || '').includes('application/json');
}

function unauthorizedOr(response: Response, message: string): string {
  return response.status === 401 ? 'Unauthorized — sign in again' : message;
}

export async function fetchDownloadSettings(): Promise<boolean> {
  try {
    const response = await fetch(`${API_BASE}/settings/downloads`, { headers: apiHeaders() });
    if (!response.ok) return false;
    downloadSettings.set(await response.json());
    return true;
  } catch (err) {
    console.error('[SystemStore] Fetch download settings error:', err);
    return false;
  }
}

export async function saveDownloadSettings(settings: DownloadSettings): Promise<SaveResult> {
  try {
    const response = await fetch(`${API_BASE}/settings/downloads`, {
      method: 'PUT',
      headers: apiHeaders(true),
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

export async function fetchIndexerSettings(): Promise<boolean> {
  try {
    const response = await fetch(`${API_BASE}/settings/indexer`, { headers: apiHeaders() });
    if (!response.ok) return false;
    indexerSettings.set(await response.json());
    return true;
  } catch (err) {
    console.error('[SystemStore] Fetch indexer settings error:', err);
    return false;
  }
}

/** POST /api/settings/indexer/generate — returns the new key (or null). */
export async function generateIndexerApiKey(): Promise<string | null> {
  try {
    const response = await fetch(`${API_BASE}/settings/indexer/generate`, {
      method: 'POST',
      headers: apiHeaders(),
    });
    if (!response.ok) return null;
    const { api_key } = await response.json();
    indexerSettings.update((state) => ({ ...state, enabled: true, api_key }));
    return api_key;
  } catch (err) {
    console.error('[SystemStore] Generate indexer API key error:', err);
    return null;
  }
}

export async function fetchSonarrSettings(): Promise<boolean> {
  try {
    const response = await fetch(`${API_BASE}/settings/sonarr`, { headers: apiHeaders() });
    if (!response.ok) return false;
    sonarrSettings.set(await response.json());
    return true;
  } catch (err) {
    console.error('[SystemStore] Fetch Sonarr settings error:', err);
    return false;
  }
}

/** Save Sonarr settings, then refresh integration config + status. */
export async function saveSonarrSettings(settings: SonarrSettings): Promise<SaveResult> {
  try {
    const response = await fetch(`${API_BASE}/settings/sonarr`, {
      method: 'PUT',
      headers: apiHeaders(true),
      body: JSON.stringify(settings),
    });
    const data = await response.json().catch(() => null);
    if (response.ok && data?.success) {
      await refreshIntegrationState();
      return { success: true, message: data.message };
    }
    return {
      success: false,
      message: unauthorizedOr(response, data?.message || 'Failed to save Sonarr settings'),
    };
  } catch (err) {
    console.error('[SystemStore] Save Sonarr settings error:', err);
    return { success: false };
  }
}

export async function testSonarrConnection(settings: SonarrSettings): Promise<SaveResult> {
  try {
    const response = await fetch(`${API_BASE}/settings/sonarr/test`, {
      method: 'POST',
      headers: apiHeaders(true),
      body: JSON.stringify(settings),
    });
    const data = await response.json().catch(() => null);
    return {
      success: response.ok && data?.success === true,
      message: unauthorizedOr(response, data?.message || 'Connection failed'),
    };
  } catch (err) {
    console.error('[SystemStore] Test Sonarr connection error:', err);
    return { success: false, message: 'Connection failed' };
  }
}

export async function fetchRadarrSettings(): Promise<boolean> {
  try {
    const response = await fetch(`${API_BASE}/settings/radarr`, { headers: apiHeaders() });
    if (!response.ok) return false;
    radarrSettings.set(await response.json());
    return true;
  } catch (err) {
    console.error('[SystemStore] Fetch Radarr settings error:', err);
    return false;
  }
}

/** Save Radarr settings, then refresh integration config + status. */
export async function saveRadarrSettings(settings: RadarrSettings): Promise<SaveResult> {
  try {
    const response = await fetch(`${API_BASE}/settings/radarr`, {
      method: 'PUT',
      headers: apiHeaders(true),
      body: JSON.stringify(settings),
    });
    const data = await response.json().catch(() => null);
    if (response.ok && data?.success) {
      await refreshIntegrationState();
      return { success: true, message: data.message };
    }
    return {
      success: false,
      message: unauthorizedOr(response, data?.message || 'Failed to save Radarr settings'),
    };
  } catch (err) {
    console.error('[SystemStore] Save Radarr settings error:', err);
    return { success: false };
  }
}

export async function testRadarrConnection(settings: RadarrSettings): Promise<SaveResult> {
  try {
    const response = await fetch(`${API_BASE}/settings/radarr/test`, {
      method: 'POST',
      headers: apiHeaders(true),
      body: JSON.stringify(settings),
    });
    const data = await response.json().catch(() => null);
    return {
      success: response.ok && data?.success === true,
      message: unauthorizedOr(response, data?.message || 'Connection failed'),
    };
  } catch (err) {
    console.error('[SystemStore] Test Radarr connection error:', err);
    return { success: false, message: 'Connection failed' };
  }
}

export async function fetchProwlarrSettings(): Promise<boolean> {
  try {
    const response = await fetch(`${API_BASE}/settings/prowlarr`, { headers: apiHeaders() });
    if (!response.ok) return false;
    prowlarrSettings.set(await response.json());
    return true;
  } catch (err) {
    console.error('[SystemStore] Fetch Prowlarr settings error:', err);
    return false;
  }
}

// ----------------------------------------------------------------------------
// Integration state (config + live status) with stale-response protection.
// Each request takes a generation number; only the latest one may write.
// ----------------------------------------------------------------------------

let configGeneration = 0;
let statusGeneration = 0;
let integrationSyncStarted = false;

function getApi(path: string): Promise<Response> {
  return fetch(`${API_BASE}${path}`, { headers: apiHeaders() });
}

function beginStatusRequest(): number {
  const generation = ++statusGeneration;
  integrationStatus.update((state) => ({ ...state, loading: true, error: null }));
  return generation;
}

function failStatusRequest(generation: number, err: unknown): false {
  if (generation === statusGeneration) {
    integrationStatus.set({
      sonarr: null,
      radarr: null,
      prowlarr: null,
      loading: false,
      error: 'Integration status unavailable',
    });
    console.error('[SystemStore] Integration state error:', err);
  }
  return false;
}

type IntegrationStatusResponse = Pick<IntegrationStatusState, 'sonarr' | 'radarr' | 'prowlarr'>;

/** GET /api/settings/integration-status → integrationStatus store. */
export async function fetchIntegrationStatus(): Promise<boolean> {
  const generation = beginStatusRequest();
  try {
    const response = await getApi('/settings/integration-status');
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const data: IntegrationStatusResponse = await response.json();
    if (generation !== statusGeneration) return false;
    integrationStatus.set({ ...data, loading: false, error: null });
    return true;
  } catch (err) {
    return failStatusRequest(generation, err);
  }
}

/** Fetch Sonarr + Radarr + Prowlarr settings together (all or nothing). */
export async function fetchIntegrationConfiguration(): Promise<boolean> {
  const generation = ++configGeneration;
  try {
    const responses = await Promise.all([
      getApi('/settings/sonarr'),
      getApi('/settings/radarr'),
      getApi('/settings/prowlarr'),
    ]);
    if (!responses.every((r) => r.ok)) throw new Error('Integration configuration unavailable');
    const [sonarr, radarr, prowlarr] = await Promise.all(responses.map((r) => r.json()));
    if (generation !== configGeneration) return false;
    sonarrSettings.set(sonarr);
    radarrSettings.set(radarr);
    prowlarrSettings.set(prowlarr);
    return true;
  } catch (err) {
    console.error('[SystemStore] Integration configuration error:', err);
    return false;
  }
}

/**
 * Refresh integration configuration and live status in parallel.
 * Resolves true when the configuration part succeeded.
 */
export async function refreshIntegrationState(): Promise<boolean> {
  const cfgGeneration = ++configGeneration;
  const stGeneration = beginStatusRequest();

  const configRequest = Promise.all([
    getApi('/settings/sonarr'),
    getApi('/settings/radarr'),
    getApi('/settings/prowlarr'),
  ]).then(async (responses) => {
    if (!responses.every((r) => r.ok)) throw new Error('Integration configuration unavailable');
    return Promise.all(responses.map((r) => r.json())) as Promise<
      [SonarrSettings, RadarrSettings, ProwlarrSettings]
    >;
  });

  const statusRequest = getApi('/settings/integration-status').then((response) => {
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return response.json() as Promise<IntegrationStatusResponse>;
  });

  const [config, status] = await Promise.allSettled([configRequest, statusRequest]);

  let configured = false;
  if (config.status === 'fulfilled' && cfgGeneration === configGeneration) {
    const [sonarr, radarr, prowlarr] = config.value;
    sonarrSettings.set(sonarr);
    radarrSettings.set(radarr);
    prowlarrSettings.set(prowlarr);
    configured = true;
  } else if (config.status === 'rejected') {
    console.error('[SystemStore] Integration configuration error:', config.reason);
  }

  if (status.status === 'fulfilled' && stGeneration === statusGeneration) {
    integrationStatus.set({ ...status.value, loading: false, error: null });
  } else if (status.status === 'rejected') {
    failStatusRequest(stGeneration, status.reason);
  }

  return configured;
}

/**
 * Keep integration state fresh: on INTEGRATION_SETTINGS_CHANGED, on every
 * websocket (re)connect, on window focus and when the tab becomes visible.
 * Idempotent — only the first call installs listeners.
 */
export function initIntegrationStatusSync(): void {
  if (integrationSyncStarted) return;
  integrationSyncStarted = true;

  wsClient.on('INTEGRATION_SETTINGS_CHANGED', () => {
    void refreshIntegrationState();
  });

  let wasConnected = false;
  wsClient.status.subscribe((status) => {
    const connected = status === 'connected';
    if (connected && !wasConnected) void refreshIntegrationState();
    wasConnected = connected;
  });

  window.addEventListener('focus', () => {
    void refreshIntegrationState();
  });
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') void refreshIntegrationState();
  });
}

/** Save Prowlarr settings; refreshes integration state on success. */
export async function saveProwlarrSettings(settings: ProwlarrSettings): Promise<SaveResult> {
  try {
    const response = await fetch(`${API_BASE}/settings/prowlarr`, {
      method: 'PUT',
      headers: apiHeaders(true),
      body: JSON.stringify(settings),
    });
    const data = await response.json();
    if (response.ok && data.success) await refreshIntegrationState();
    return { success: data.success, message: data.message };
  } catch (err) {
    console.error('[SystemStore] Save Prowlarr settings error:', err);
    return { success: false, message: 'Network error' };
  }
}

export async function testProwlarrConnection(settings: ProwlarrSettings): Promise<SaveResult> {
  try {
    const response = await fetch(`${API_BASE}/settings/prowlarr/test`, {
      method: 'POST',
      headers: apiHeaders(true),
      body: JSON.stringify(settings),
    });
    return await response.json();
  } catch (err) {
    console.error('[SystemStore] Test Prowlarr connection error:', err);
    return { success: false, message: 'Connection failed' };
  }
}

// ----------------------------------------------------------------------------
// TorBox
// ----------------------------------------------------------------------------

export async function fetchTorBoxSettings(): Promise<void> {
  try {
    const response = await fetch(`${API_BASE}/settings/torbox`, { headers: apiHeaders() });
    if (response.ok) torboxSettings.set(await response.json());
  } catch (err) {
    console.error('[SystemStore] Fetch TorBox settings error:', err);
  }
}

/** Sets torboxProfile to the profile, or null when unavailable/not JSON. */
export async function fetchTorBoxProfile(): Promise<void> {
  try {
    const response = await fetch(`${API_BASE}/settings/torbox/profile`, { headers: apiHeaders() });
    if (!response.ok || !isJsonResponse(response)) {
      torboxProfile.set(null);
      return;
    }
    torboxProfile.set(await response.json());
  } catch (err) {
    console.error('[SystemStore] Fetch TorBox profile error:', err);
    torboxProfile.set(null);
  }
}

export async function saveTorBoxSettings(settings: TorBoxSettings): Promise<SaveResult> {
  try {
    const response = await fetch(`${API_BASE}/settings/torbox`, {
      method: 'PUT',
      headers: apiHeaders(true),
      body: JSON.stringify(settings),
    });
    const data = await response.json();
    if (response.ok && data.success) torboxSettings.set(settings);
    return { success: data.success, message: data.message };
  } catch (err) {
    console.error('[SystemStore] Save TorBox settings error:', err);
    return { success: false, message: 'Network error' };
  }
}

export async function testTorBoxConnection(settings: TorBoxSettings): Promise<SaveResult> {
  try {
    const response = await fetch(`${API_BASE}/settings/torbox/test`, {
      method: 'POST',
      headers: apiHeaders(true),
      body: JSON.stringify(settings),
    });
    return await response.json();
  } catch (err) {
    console.error('[SystemStore] Test TorBox connection error:', err);
    return { success: false, message: 'Connection failed' };
  }
}

/** DELETE /api/settings/torbox — resets settings + profile stores on success. */
export async function clearTorBoxSettings(): Promise<SaveResult> {
  try {
    const response = await fetch(`${API_BASE}/settings/torbox`, {
      method: 'DELETE',
      headers: apiHeaders(),
    });
    const data = await response.json();
    if (response.ok && data.success) {
      torboxSettings.set({ ...DEFAULT_TORBOX_SETTINGS });
      torboxProfile.set(null);
    }
    return { success: data.success, message: data.message };
  } catch (err) {
    console.error('[SystemStore] Clear TorBox settings error:', err);
    return { success: false, message: 'Network error' };
  }
}

// ----------------------------------------------------------------------------
// Metadata providers (TMDB / TVDB API keys)
// ----------------------------------------------------------------------------

type MetadataProviderResponse = Omit<MetadataProvider, 'reachable' | 'message'>;

function withUntestedState(provider: MetadataProviderResponse): MetadataProvider {
  return { ...provider, reachable: null, message: '' };
}

export async function fetchMetadataProviders(): Promise<boolean> {
  try {
    const response = await fetch(`${API_BASE}/settings/metadata`, { headers: apiHeaders() });
    if (!response.ok || !isJsonResponse(response)) return false;
    const data: { providers?: MetadataProviderResponse[] } = await response.json();
    metadataProviders.set((data.providers || []).map(withUntestedState));
    return true;
  } catch (err) {
    console.error('[SystemStore] Fetch metadata settings error:', err);
    return false;
  }
}

/**
 * Test a provider, optionally with an unsaved key. Records the result on the
 * matching metadataProviders row.
 */
export async function testMetadataProvider(provider: string, apiKey?: string): Promise<SaveResult> {
  try {
    const response = await fetch(`${API_BASE}/settings/metadata/${provider}/test`, {
      method: 'POST',
      headers: apiHeaders(true),
      body: JSON.stringify(apiKey?.trim() ? { apiKey: apiKey.trim() } : {}),
    });
    const data = await response.json().catch(() => null);
    const result = {
      success: response.ok && data?.success === true,
      message: data?.message || 'Connection test failed',
    };
    metadataProviders.update((rows) =>
      rows.map((row) =>
        row.provider === provider ? { ...row, reachable: result.success, message: result.message } : row,
      ),
    );
    return result;
  } catch (err) {
    console.error('[SystemStore] Test metadata provider error:', err);
    return { success: false, message: 'Network error' };
  }
}

/** PUT /api/settings/metadata/:provider with `{ apiKey }`. */
export async function saveMetadataProvider(provider: string, apiKey: string): Promise<SaveResult> {
  try {
    const response = await fetch(`${API_BASE}/settings/metadata/${provider}`, {
      method: 'PUT',
      headers: apiHeaders(true),
      body: JSON.stringify({ apiKey: apiKey.trim() }),
    });
    const data = await response.json().catch(() => null);
    if (!response.ok || !data?.provider) {
      return { success: false, message: data?.message || 'Failed to save API key' };
    }
    metadataProviders.update((rows) =>
      rows.map((row) => (row.provider === provider ? withUntestedState(data) : row)),
    );
    return { success: true, message: `${provider.toUpperCase()} API key saved` };
  } catch (err) {
    console.error('[SystemStore] Save metadata provider error:', err);
    return { success: false, message: 'Network error' };
  }
}

// ----------------------------------------------------------------------------
// Health / storage / qBittorrent
// ----------------------------------------------------------------------------

export async function fetchSystemHealth(): Promise<boolean> {
  try {
    const response = await fetch(`${API_BASE}/health`, { headers: apiHeaders() });
    if (!response.ok) return false;
    systemHealth.set(await response.json());
    return true;
  } catch (err) {
    console.error('[SystemStore] Fetch system health error:', err);
    return false;
  }
}

export async function fetchStorageStatus(): Promise<boolean> {
  try {
    const response = await fetch(`${API_BASE}/settings/storage`, { headers: apiHeaders() });
    if (!response.ok) return false;
    storageStatus.set(await response.json());
    return true;
  } catch (err) {
    console.error('[SystemStore] Fetch storage status error:', err);
    return false;
  }
}

export async function fetchQBittorrentSettings(): Promise<void> {
  try {
    const response = await fetch(`${API_BASE}/settings/qbittorrent`, { headers: apiHeaders() });
    if (response.ok) qbittorrentSettings.set(await response.json());
  } catch (err) {
    console.error('[SystemStore] Fetch qBittorrent settings error:', err);
  }
}

export async function saveQBittorrentSettings(settings: QBittorrentSettings): Promise<SaveResult> {
  try {
    const response = await fetch(`${API_BASE}/settings/qbittorrent`, {
      method: 'PUT',
      headers: apiHeaders(true),
      body: JSON.stringify(settings),
    });
    const data = await response.json();
    if (response.ok && data.success) qbittorrentSettings.set(settings);
    return { success: data.success, message: data.message };
  } catch (err) {
    console.error('[SystemStore] Save qBittorrent settings error:', err);
    return { success: false, message: 'Network error' };
  }
}

export async function testQBittorrentConnection(settings: QBittorrentSettings): Promise<SaveResult> {
  try {
    const response = await fetch(`${API_BASE}/settings/qbittorrent/test`, {
      method: 'POST',
      headers: apiHeaders(true),
      body: JSON.stringify(settings),
    });
    return await response.json();
  } catch (err) {
    console.error('[SystemStore] Test qBittorrent connection error:', err);
    return { success: false, message: 'Connection failed' };
  }
}

// ----------------------------------------------------------------------------
// Logs / activity
// ----------------------------------------------------------------------------

/** Fetch the last `lines` log lines, hiding anything older than the last clearLogs(). */
export async function fetchLogs(lines: number = 50): Promise<void> {
  try {
    const response = await fetch(`${API_BASE}/system/logs?lines=${lines}`);
    if (response.ok) {
      const data = await response.json();
      const logs: LogEntry[] = data.logs || [];
      systemLogs.set(logsClearedAt ? logs.filter((log) => log.timestamp > logsClearedAt!) : logs);
    }
  } catch (err) {
    console.error('[SystemStore] Fetch logs error:', err);
  }
}

/** Client-side clear: empties the store and hides older lines on later fetches. */
export function clearLogs(): void {
  logsClearedAt = new Date().toISOString();
  systemLogs.set([]);
}

export async function fetchActivity(limit: number = 50): Promise<void> {
  try {
    const response = await fetch(`${API_BASE}/system/activity?limit=${limit}`);
    if (!response.ok) return;
    if (!isJsonResponse(response)) {
      console.error('[SystemStore] Fetch activity returned a non-JSON response');
      return;
    }
    const data = await response.json();
    systemActivity.set(data.activities || []);
  } catch (err) {
    console.error('[SystemStore] Fetch activity error:', err);
  }
}

// ----------------------------------------------------------------------------
// Folder sources
// ----------------------------------------------------------------------------

export async function fetchFolderSourceConfig(): Promise<boolean> {
  try {
    const response = await fetch(`${API_BASE}/folder-source/config`);
    if (!response.ok) return false;
    const data = await response.json();
    folderSourceUrl.set(data.gist_url || '');
    importedFolderSources.set(data.imported_sources || 0);
    return true;
  } catch (err) {
    console.error('[SystemStore] Fetch folder source config error:', err);
    return false;
  }
}

export async function saveFolderSourceConfig(gistUrl: string): Promise<SaveResult> {
  try {
    const response = await fetch(`${API_BASE}/folder-source/config`, {
      method: 'PUT',
      headers: apiHeaders(true),
      body: JSON.stringify({ gist_url: gistUrl }),
    });
    const data = await response.json().catch(() => null);
    if (response.ok && data?.success) {
      folderSourceUrl.set(gistUrl);
      return { success: true, message: data.message };
    }
    return { success: false, message: data?.message || 'Failed to save folder source config' };
  } catch (err) {
    console.error('[SystemStore] Save folder source config error:', err);
    return { success: false, message: 'Network error' };
  }
}

/** POST /api/folder-source/import with pasted folder-list text. */
export async function importFolderSources(sourceText: string): Promise<FolderSourceImportResult> {
  try {
    const response = await fetch(`${API_BASE}/folder-source/import`, {
      method: 'POST',
      headers: apiHeaders(true),
      body: JSON.stringify({ source_text: sourceText }),
    });
    const data = await response.json().catch(() => null);
    const result: FolderSourceImportResult = {
      success: response.ok && data?.success === true,
      imported: data?.imported || 0,
      message: data?.message || 'Folder list could not be imported',
    };
    if (result.success) importedFolderSources.set(result.imported);
    return result;
  } catch (err) {
    console.error('[SystemStore] Import folder sources error:', err);
    return { success: false, imported: 0, message: 'Network error' };
  }
}

// ============================================================================
// Facade (same object shape the pages use: systemStore.fetchX())
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
  fetchProwlarrSettings,
  fetchIntegrationStatus,
  fetchIntegrationConfiguration,
  refreshIntegrationState,
  initIntegrationStatusSync,
  saveProwlarrSettings,
  testProwlarrConnection,
  fetchTorBoxSettings,
  fetchTorBoxProfile,
  saveTorBoxSettings,
  testTorBoxConnection,
  clearTorBoxSettings,
  fetchMetadataProviders,
  testMetadataProvider,
  saveMetadataProvider,
  fetchSystemHealth,
  fetchStorageStatus,
  fetchQBittorrentSettings,
  saveQBittorrentSettings,
  testQBittorrentConnection,
  fetchLogs,
  clearLogs,
  fetchActivity,
  fetchFolderSourceConfig,
  saveFolderSourceConfig,
  importFolderSources,
};
