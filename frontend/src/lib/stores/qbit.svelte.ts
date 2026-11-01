/**
 * qBittorrent instances + torrents store (Svelte 5 runes) for the /torrents page.
 *
 * REST: /api/instances[?type=qbittorrent], /api/instances/:id/qbit/*
 * Live: QBIT_SYNC_INIT / QBIT_TORRENT_UPSERT / QBIT_TORRENT_REMOVED /
 *       QBIT_TRANSFER_INFO / QBIT_ERROR websocket messages (initWebSocket()).
 *
 * Torrent rows and selection entries are keyed `"<instance_id>:<hash>"`.
 * Mutating actions toast their result (success → toasts.success, else error).
 */
import { toasts } from './toasts';
import { wsClient } from './websocket';
import {
  applyQbitSyncInit,
  normalizeTorrentRow,
  normalizeTorrentRows,
  pruneTorrentSelection,
  type QbitSyncInitMessage,
  type UnifiedTorrentRow,
} from './qbitReducer';

// ============================================================================
// Types
// ============================================================================

/** GET /api/instances (backend InstanceResponse). */
export interface QbitInstance {
  id: string;
  name: string;
  url: string;
  username: string;
  enabled: boolean;
  default_category: string | null;
  save_path: string | null;
  poll_interval_secs: number;
  sync_health: string;
  last_sync_at: string | null;
  app_version: string | null;
  webapi_version: string | null;
  last_seen_at: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
  has_password: boolean;
}

/**
 * Create (no id) or update (id) an instance. `id` and `has_password` are
 * stripped from the request body; omit `password` to keep the stored one.
 */
export interface QbitInstanceInput {
  id?: string;
  has_password?: boolean;
  name: string;
  url: string;
  username?: string | null;
  password?: string | null;
  enabled?: boolean;
  default_category?: string | null;
  save_path?: string | null;
  poll_interval_secs?: number;
}

/** qBittorrent /transfer/info (extra keys passed through). */
export interface QbitTransferInfo {
  dl_info_speed?: number;
  up_info_speed?: number;
  dl_info_data?: number;
  up_info_data?: number;
  connection_status?: string;
  free_space_on_disk?: number;
  [key: string]: unknown;
}

export interface QbitTransferHealth {
  sync_health: string;
  last_sync_at?: string | null;
}

/** Generic action response (`results` only on bulk hash actions). */
export interface QbitActionResult {
  success: boolean;
  message?: string;
  results?: Array<{ hash: string; success: boolean; message?: string }>;
}

export interface QbitTorrentFilters {
  /** substring of the raw qB state (case-insensitive) */
  status: string;
  category: string;
  tag: string;
  /** substring of the tracker (case-insensitive) */
  tracker: string;
}

export interface QbitFilterOptions {
  status: string[];
  category: string[];
  tag: string[];
  tracker: string[];
}

/** Hash actions posted to /qbit/torrents/:action with `{ hashes, ...extra }`. */
export type QbitTorrentAction =
  | 'pause'
  | 'resume'
  | 'delete'
  | 'recheck'
  | 'reannounce'
  | 'set-category'
  | 'set-tags'
  | 'queue/top'
  | 'queue/up'
  | 'queue/down'
  | 'queue/bottom'
  | (string & {});

/** Per-torrent detail endpoints (GET /qbit/torrents/:hash/:kind). */
export type QbitDetailKind = 'properties' | 'files' | 'trackers' | 'peers' | 'piece-states' | (string & {});

export interface QbitAddTorrentOptions {
  category?: string;
  savepath?: string;
  tags?: string;
  paused?: boolean;
  stopped?: boolean;
  skip_checking?: boolean;
  sequential_download?: boolean;
  first_last_piece_prio?: boolean;
}

export interface QbitCreateTorrentPayload {
  source_path: string;
  save_path?: string;
  torrent_name?: string;
  trackers?: string;
  source?: string;
  private?: boolean;
  start_seeding?: boolean;
}

export interface QbitPreferences {
  dl_limit?: number;
  up_limit?: number;
  alt_dl_limit?: number;
  alt_up_limit?: number;
  max_ratio_enabled?: boolean;
  max_ratio?: number;
  max_seeding_time_enabled?: boolean;
  max_seeding_time?: number;
  queueing_enabled?: boolean;
  max_active_downloads?: number;
  max_active_uploads?: number;
  [key: string]: unknown;
}

// ============================================================================
// Store
// ============================================================================

const API_BASE = '/api/instances';
const NO_INSTANCE = 'No qBittorrent instance selected';

function apiHeaders(withJsonBody = false): HeadersInit {
  return withJsonBody ? { 'Content-Type': 'application/json' } : {};
}

function toastResult(result: QbitActionResult, success: string, failure: string) {
  if (result.success) toasts.success(result.message || success);
  else toasts.error(result.message || failure);
}

class QbitStore {
  instances = $state<QbitInstance[]>([]);
  selectedInstanceId = $state('');
  /** All known torrents of all instances, keyed `instance:hash`. */
  torrents = $state<Map<string, UnifiedTorrentRow>>(new Map());
  /** instance id → last transfer info */
  transferInfo = $state<Map<string, QbitTransferInfo>>(new Map());
  /** instance id → sync health from /transfer-info */
  transferHealth = $state<Map<string, QbitTransferHealth>>(new Map());
  /** Selected torrents, keyed `instance:hash`. */
  selectedHashes = $state<Set<string>>(new Set());
  categoryNames = $state<string[]>([]);
  tagNames = $state<string[]>([]);
  filterOptions = $state<QbitFilterOptions>({ status: [], category: [], tag: [], tracker: [] });
  /** `instance:hash:kind` → detail payload */
  detailCache = $state<Map<string, unknown>>(new Map());
  loading = $state(false);
  error = $state<string | null>(null);
  search = $state('');
  filters = $state<QbitTorrentFilters>({ status: '', category: '', tag: '', tracker: '' });

  /** Torrents of the selected instance after search + filters, newest first. */
  torrentList: UnifiedTorrentRow[] = $derived.by(() =>
    Array.from(this.torrents.values())
      .filter((t) => !this.selectedInstanceId || t.instance_id === this.selectedInstanceId)
      .filter(
        (t) => !this.search || `${t.name} ${t.hash} ${t.tracker}`.toLowerCase().includes(this.search.toLowerCase()),
      )
      .filter((t) => !this.filters.status || t.state.toLowerCase().includes(this.filters.status.toLowerCase()))
      .filter((t) => !this.filters.category || t.category === this.filters.category)
      .filter(
        (t) =>
          !this.filters.tag ||
          t.tags
            .split(',')
            .map((tag) => tag.trim())
            .includes(this.filters.tag),
      )
      .filter((t) => !this.filters.tracker || t.tracker.toLowerCase().includes(this.filters.tracker.toLowerCase()))
      .sort((a, b) => b.added_on - a.added_on),
  );

  selectedInstance: QbitInstance | undefined = $derived.by(() =>
    this.instances.find((i) => i.id === this.selectedInstanceId),
  );

  // --------------------------------------------------------------------------
  // Instances
  // --------------------------------------------------------------------------

  /**
   * Load instances, keep the current selection if it is still enabled (else
   * pick the first enabled one), then load torrents + transfer info.
   */
  async fetchInstances(): Promise<void> {
    this.loading = true;
    try {
      const response = await fetch(`${API_BASE}?type=qbittorrent`, { headers: apiHeaders() });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      this.instances = await response.json();
      const current = this.instances.find((i) => i.id === this.selectedInstanceId && i.enabled);
      this.selectedInstanceId = current?.id || this.instances.find((i) => i.enabled)?.id || '';
      await Promise.all([this.fetchTorrents(), this.fetchTransferInfo()]);
    } catch (err) {
      this.error = err instanceof Error ? err.message : 'Failed to load qBittorrent instances';
    } finally {
      this.loading = false;
    }
  }

  /** POST (new) or PUT (existing) an instance; selects it when enabled. Throws on HTTP error. */
  async saveInstance(input: QbitInstanceInput): Promise<QbitInstance> {
    const id = input.id;
    const { id: _id, has_password: _hasPassword, ...body } = input;
    const response = await fetch(id ? `${API_BASE}/${id}` : API_BASE, {
      method: id ? 'PUT' : 'POST',
      headers: apiHeaders(true),
      body: JSON.stringify(body),
    });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const saved: QbitInstance = await response.json();
    await this.fetchInstances();
    if (saved.enabled) this.selectedInstanceId = saved.id;
    return saved;
  }

  /** Throws on HTTP error. */
  async deleteInstance(id: string): Promise<void> {
    const response = await fetch(`${API_BASE}/${id}`, { method: 'DELETE', headers: apiHeaders() });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    await this.fetchInstances();
  }

  async testInstance(id: string): Promise<QbitActionResult> {
    const response = await fetch(`${API_BASE}/${id}/test`, { method: 'POST', headers: apiHeaders() });
    return await response.json();
  }

  // --------------------------------------------------------------------------
  // Torrents
  // --------------------------------------------------------------------------

  /**
   * Load up to 2000 torrents of the selected instance with the current search
   * and filters (server-side), replacing that instance's rows, then rebuild
   * filterOptions from the unfiltered list. Sets `error` and rethrows on failure.
   */
  async fetchTorrents(): Promise<void> {
    if (!this.selectedInstanceId) {
      this.torrents = new Map();
      return;
    }
    this.loading = true;
    this.error = null;
    try {
      const params = new URLSearchParams({ page: '1', limit: '2000', sort: 'added_on', order: 'desc' });
      if (this.search) params.set('search', this.search);
      const serverFilters: Record<string, string[]> = {};
      if (this.filters.status) serverFilters.status = [this.filters.status];
      if (this.filters.category) serverFilters.categories = [this.filters.category];
      if (this.filters.tag) serverFilters.tags = [this.filters.tag];
      if (this.filters.tracker) serverFilters.trackers = [this.filters.tracker];
      if (Object.keys(serverFilters).length) params.set('filters', JSON.stringify(serverFilters));

      const response = await fetch(`${API_BASE}/${this.selectedInstanceId}/qbit/torrents?${params}`, {
        headers: apiHeaders(),
      });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const data = await response.json();
      const rows = normalizeTorrentRows(data.torrents);

      const next = new Map(
        Array.from(this.torrents.entries()).filter(([key]) => !key.startsWith(`${this.selectedInstanceId}:`)),
      );
      for (const row of rows) next.set(`${row.instance_id}:${row.hash}`, row);
      this.torrents = next;
      this.pruneSelection();

      const optionRows =
        Object.keys(serverFilters).length || this.search ? await this.fetchFilterOptionRows() : rows;
      this.filterOptions = {
        status: Array.from(new Set(optionRows.map((r) => r.state).filter(Boolean))).sort(),
        category: Array.from(new Set(optionRows.map((r) => r.category).filter(Boolean))).sort(),
        tag: Array.from(
          new Set(
            optionRows.flatMap((r) =>
              r.tags
                .split(',')
                .map((tag) => tag.trim())
                .filter(Boolean),
            ),
          ),
        ).sort(),
        tracker: Array.from(new Set(optionRows.map((r) => r.tracker).filter(Boolean))).sort(),
      };
    } catch (err) {
      this.error = err instanceof Error ? err.message : 'Failed to load torrents';
      throw err;
    } finally {
      this.loading = false;
    }
  }

  /** Unfiltered torrent list of the selected instance (for filter dropdowns). */
  async fetchFilterOptionRows(): Promise<UnifiedTorrentRow[]> {
    if (!this.selectedInstanceId) return [];
    const params = new URLSearchParams({ page: '1', limit: '2000', sort: 'added_on', order: 'desc' });
    const response = await fetch(`${API_BASE}/${this.selectedInstanceId}/qbit/torrents?${params}`, {
      headers: apiHeaders(),
    });
    return response.ok ? normalizeTorrentRows((await response.json()).torrents) : [];
  }

  async fetchTransferInfo(): Promise<void> {
    if (!this.selectedInstanceId) return;
    const response = await fetch(`${API_BASE}/${this.selectedInstanceId}/qbit/transfer-info`, {
      headers: apiHeaders(),
    });
    if (!response.ok) return;
    const data = await response.json();

    const info = new Map(this.transferInfo);
    info.set(this.selectedInstanceId, data.transfer || {});
    this.transferInfo = info;

    const health = new Map(this.transferHealth);
    health.set(this.selectedInstanceId, {
      sync_health: data.sync_health || 'unknown',
      last_sync_at: data.last_sync_at,
    });
    this.transferHealth = health;
  }

  // --------------------------------------------------------------------------
  // Mutations (selected instance)
  // --------------------------------------------------------------------------

  private requireInstance(): string {
    if (!this.selectedInstanceId) throw new Error(NO_INSTANCE);
    return this.selectedInstanceId;
  }

  private async post(path: string, body: unknown): Promise<QbitActionResult> {
    const response = await fetch(`${API_BASE}/${this.requireInstance()}/qbit/${path}`, {
      method: 'POST',
      headers: apiHeaders(true),
      body: JSON.stringify(body),
    });
    return await response.json();
  }

  async createTorrent(payload: QbitCreateTorrentPayload): Promise<QbitActionResult> {
    this.requireInstance();
    const result = await this.post('torrents/create', payload);
    toastResult(result, 'Torrent created', 'Failed to create torrent');
    return result;
  }

  /** Add a magnet/URL. */
  async addTorrent(url: string, options: QbitAddTorrentOptions = {}): Promise<QbitActionResult> {
    this.requireInstance();
    const result = await this.post('torrents/add', { url, ...options });
    if (result.success) toasts.success('Torrent added');
    else toasts.error(result.message || 'Failed to add torrent');
    return result;
  }

  /** Upload a .torrent file (multipart; empty/null options are skipped). */
  async addTorrentFile(file: File | Blob, options: QbitAddTorrentOptions = {}): Promise<QbitActionResult> {
    const instanceId = this.requireInstance();
    const form = new FormData();
    form.append('torrent', file);
    for (const [key, value] of Object.entries(options)) {
      if (value == null || value === '') continue;
      form.append(key, typeof value === 'boolean' ? String(value) : `${value}`);
    }
    const response = await fetch(`${API_BASE}/${instanceId}/qbit/torrents/add-file`, {
      method: 'POST',
      headers: apiHeaders(),
      body: form,
    });
    const result: QbitActionResult = await response.json();
    if (result.success) toasts.success('Torrent file added');
    else toasts.error(result.message || 'Failed to add torrent file');
    return result;
  }

  /**
   * Run a hash action (default: the current selection). No-op (undefined)
   * without an instance or hashes.
   */
  async action(
    action: QbitTorrentAction,
    hashes: string[] = this.selectedForCurrentInstance(),
    extra: Record<string, unknown> = {},
  ): Promise<QbitActionResult | undefined> {
    if (!this.selectedInstanceId || hashes.length === 0) return;
    const result = await this.post(`torrents/${action}`, { hashes, ...extra });
    toastResult(result, 'qBittorrent action sent', 'qBittorrent action failed');
    return result;
  }

  /** Load category and tag names of the selected instance. */
  async fetchLabels(): Promise<void> {
    if (!this.selectedInstanceId) return;
    const [categories, tags] = await Promise.all([
      fetch(`${API_BASE}/${this.selectedInstanceId}/qbit/categories`, { headers: apiHeaders() }),
      fetch(`${API_BASE}/${this.selectedInstanceId}/qbit/tags`, { headers: apiHeaders() }),
    ]);
    this.categoryNames = categories.ok ? Object.keys(await categories.json()) : [];
    this.tagNames = tags.ok ? await tags.json() : [];
  }

  /** Cached GET /qbit/torrents/:hash/:kind (null without an instance; throws on HTTP error). */
  async detail<T = any>(hash: string, kind: QbitDetailKind): Promise<T | null> {
    if (!this.selectedInstanceId) return null;
    const cacheKey = `${this.selectedInstanceId}:${hash}:${kind}`;
    if (this.detailCache.has(cacheKey)) return this.detailCache.get(cacheKey) as T;
    const response = await fetch(`${API_BASE}/${this.selectedInstanceId}/qbit/torrents/${hash}/${kind}`, {
      headers: apiHeaders(),
    });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const data = await response.json();
    const next = new Map(this.detailCache);
    next.set(cacheKey, data);
    this.detailCache = next;
    return data;
  }

  async setFilePriority(hash: string, fileId: number, priority: number): Promise<QbitActionResult> {
    this.requireInstance();
    const result = await this.post(`torrents/${hash}/files/priority`, { id: fileId, priority });
    toastResult(result, 'File priority updated', 'File priority failed');
    return result;
  }

  async preferences(): Promise<QbitPreferences> {
    const instanceId = this.requireInstance();
    const response = await fetch(`${API_BASE}/${instanceId}/qbit/preferences`, { headers: apiHeaders() });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return await response.json();
  }

  async savePreferences(preferences: QbitPreferences): Promise<QbitActionResult> {
    this.requireInstance();
    const result = await this.post('preferences', preferences);
    toastResult(result, 'Preferences saved', 'Preferences save failed');
    return result;
  }

  /** Limits in bytes/s (null/undefined = unlimited/unchanged per backend). */
  async setSpeedLimits(
    hashes: string[],
    downloadLimit: number | null | undefined,
    uploadLimit: number | null | undefined,
  ): Promise<QbitActionResult> {
    return await this.postTorrentBody('set-speed-limits', {
      hashes,
      download_limit: downloadLimit,
      upload_limit: uploadLimit,
    });
  }

  async setShareLimits(
    hashes: string[],
    ratioLimit: number | null | undefined,
    seedingTimeLimit: number | null | undefined,
  ): Promise<QbitActionResult> {
    return await this.postTorrentBody('set-share-limits', {
      hashes,
      ratio_limit: ratioLimit,
      seeding_time_limit: seedingTimeLimit,
    });
  }

  /** POST /qbit/torrents/:action with an explicit body; toasts the result. */
  async postTorrentBody(action: string, body: Record<string, unknown>): Promise<QbitActionResult> {
    this.requireInstance();
    const result = await this.post(`torrents/${action}`, body);
    toastResult(result, 'qBittorrent action sent', 'qBittorrent action failed');
    return result;
  }

  async renameTorrent(hash: string, name: string): Promise<QbitActionResult> {
    this.requireInstance();
    const result = await this.post(`torrents/${hash}/rename`, { name });
    toastResult(result, 'Torrent renamed', 'Torrent rename failed');
    return result;
  }

  async renameFile(hash: string, oldPath: string, newName: string): Promise<QbitActionResult> {
    this.requireInstance();
    const result = await this.post(`torrents/${hash}/rename-file`, { old_path: oldPath, new_name: newName });
    toastResult(result, 'File renamed', 'File rename failed');
    return result;
  }

  async renameFolder(hash: string, oldPath: string, newName: string): Promise<QbitActionResult> {
    this.requireInstance();
    const result = await this.post(`torrents/${hash}/rename-folder`, { old_path: oldPath, new_name: newName });
    toastResult(result, 'Folder renamed', 'Folder rename failed');
    return result;
  }

  async setLocation(hashes: string[], location: string): Promise<QbitActionResult> {
    return await this.postTorrentBody('set-location', { hashes, location });
  }

  /** `peers`: newline/pipe separated `host:port` list, as qBittorrent expects. */
  async addPeers(hash: string, peers: string): Promise<QbitActionResult> {
    this.requireInstance();
    const result = await this.post(`torrents/${hash}/peers/add`, { peers });
    toastResult(result, 'Peer(s) added', 'Failed to add peers');
    if (result.success) this.invalidateDetail(hash, 'peers');
    return result;
  }

  async banPeers(peers: string): Promise<QbitActionResult> {
    this.requireInstance();
    const result = await this.post('torrents/peers/ban', { peers });
    toastResult(result, 'Peer(s) banned', 'Failed to ban peers');
    return result;
  }

  /** Add the same torrent to another instance. */
  async crossSeed(hash: string, targetInstanceId: string, savePath?: string): Promise<QbitActionResult> {
    this.requireInstance();
    const result = await this.post(`torrents/${hash}/cross-seed`, {
      target_instance_id: targetInstanceId,
      save_path: savePath,
    });
    toastResult(result, 'Cross-seeded', 'Cross-seed failed');
    return result;
  }

  // --------------------------------------------------------------------------
  // Local state
  // --------------------------------------------------------------------------

  clearFilters() {
    this.search = '';
    this.filters = { status: '', category: '', tag: '', tracker: '' };
  }

  /** Drop cached details of a torrent (one kind, or all kinds). */
  invalidateDetail(hash: string, kind?: QbitDetailKind) {
    if (!this.selectedInstanceId) return;
    const prefix = `${this.selectedInstanceId}:${hash}:`;
    const exact = kind ? `${prefix}${kind}` : '';
    const next = new Map(this.detailCache);
    for (const key of next.keys()) {
      if (kind ? key === exact : key.startsWith(prefix)) next.delete(key);
    }
    this.detailCache = next;
  }

  pruneSelection() {
    const pruned = pruneTorrentSelection(this.selectedHashes, this.torrents, this.selectedInstanceId);
    if (pruned.size !== this.selectedHashes.size) this.selectedHashes = pruned;
  }

  initWebSocket() {
    wsClient.on('QBIT_SYNC_INIT', (message) => {
      this.torrents = applyQbitSyncInit(this.torrents, message as unknown as QbitSyncInitMessage);
      this.pruneSelection();
    });
    wsClient.on('QBIT_TORRENT_UPSERT', (message) => {
      const row = normalizeTorrentRow(message.torrent);
      if (!row) return;
      const next = new Map(this.torrents);
      next.set(`${row.instance_id}:${row.hash}`, row);
      this.torrents = next;
    });
    wsClient.on('QBIT_TORRENT_REMOVED', (message) => {
      const key = `${message.instance_id}:${message.hash}`;
      const next = new Map(this.torrents);
      next.delete(key);
      this.torrents = next;
      const selected = new Set(this.selectedHashes);
      selected.delete(key);
      this.selectedHashes = selected;
    });
    wsClient.on('QBIT_TRANSFER_INFO', (message) => {
      const next = new Map(this.transferInfo);
      next.set(message.instance_id, message.info);
      this.transferInfo = next;
    });
    wsClient.on('QBIT_ERROR', (message) => {
      if (message.instance_id === this.selectedInstanceId) this.error = message.message;
    });
  }

  /** Toggle selection of a hash of the selected instance. */
  toggle(hash: string) {
    if (!this.selectedInstanceId) return;
    const key = `${this.selectedInstanceId}:${hash}`;
    const next = new Set(this.selectedHashes);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    this.selectedHashes = next;
  }

  /** Replace the selection with these hashes of the selected instance. */
  selectOnly(hashes: string[]) {
    const prefix = this.selectedInstanceId ? `${this.selectedInstanceId}:` : '';
    this.selectedHashes = new Set(hashes.map((hash) => `${prefix}${hash}`));
  }

  /** Bare hashes selected on the current instance. */
  selectedForCurrentInstance(): string[] {
    const prefix = `${this.selectedInstanceId}:`;
    return Array.from(this.selectedHashes)
      .filter((key) => key.startsWith(prefix))
      .map((key) => key.slice(prefix.length));
  }

  clearSelection() {
    this.selectedHashes = new Set();
  }
}

export const qbitStore = new QbitStore();
