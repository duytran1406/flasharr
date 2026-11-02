/**
 * Pure reducers for qBittorrent torrent rows (no runtime imports so the module
 * can be unit-tested with `node --experimental-strip-types`).
 *
 * Rows are keyed `"<instance_id>:<hash>"` everywhere.
 */

/** Arr import bookkeeping attached to managed torrents (backend TorrentImportState). */
export interface TorrentImportState {
  arr_announced: boolean;
  arr_announce_error: string | null;
  arr_series_id: number | null;
  arr_movie_id: number | null;
}

/**
 * A qBittorrent torrent as sent by the backend (`UnifiedTorrentRow` in
 * api/downloads.rs; also the payload of QBIT_* websocket messages).
 * After normalizeTorrentRow() all numeric fields are finite numbers and
 * category/tags/updated_at are strings.
 */
export interface UnifiedTorrentRow {
  instance_id: string;
  hash: string;
  name: string;
  /** raw qB state, e.g. "downloading", "pausedUP" */
  state: string;
  /** mapped Flasharr state: DOWNLOADING | PAUSED | COMPLETED | QUEUED | FAILED | STARTING */
  flasharr_state: string;
  task_state?: string;
  qbit_state?: string;
  sync_health?: string;
  last_sync_at?: string | null;
  transfer_state?: string;
  transfer_active?: boolean;
  /** 0.0–1.0 */
  progress: number;
  size: number;
  downloaded: number;
  uploaded: number;
  dlspeed: number;
  upspeed: number;
  eta: number;
  ratio: number;
  category: string;
  /** comma-separated */
  tags: string;
  /** Not serialized by every backend path; the UI treats it as a string. */
  tracker: string;
  added_on: number;
  completion_on: number;
  amount_left: number;
  availability?: number;
  num_seeds: number;
  num_leechs: number;
  seen_complete?: number;
  last_activity?: number;
  updated_at: string;
  save_path?: string;
  content_path?: string;
  // Managed-torrent media metadata (absent for unmanaged torrents)
  media_key?: string | null;
  media_type?: string | null;
  media_title?: string | null;
  tvdb_id?: number | null;
  tmdb_id?: number | null;
  season?: number | null;
  episode?: number | null;
  source_filename?: string | null;
  payload?: unknown[];
  import_state?: TorrentImportState;
}

/** QBIT_SYNC_INIT websocket message. */
export interface QbitSyncInitMessage {
  instance_id: string;
  torrents: unknown;
  full_update?: boolean;
  /** Drop every existing row of this instance before applying `torrents`. */
  replaces_instance?: boolean;
}

const NUMERIC_FIELDS = [
  'progress',
  'size',
  'downloaded',
  'uploaded',
  'dlspeed',
  'upspeed',
  'eta',
  'ratio',
  'added_on',
  'completion_on',
  'amount_left',
  'availability',
  'num_seeds',
  'num_leechs',
  'seen_complete',
  'last_activity',
] as const;

/** Managed-torrent metadata preserved from the previous row when an update omits it. */
const STICKY_MEDIA_FIELDS = [
  'media_key',
  'media_type',
  'media_title',
  'tvdb_id',
  'tmdb_id',
  'season',
  'episode',
  'source_filename',
  'payload',
  'import_state',
] as const;

/**
 * Validate and normalise one raw row. Returns null unless instance_id, hash,
 * name and state are non-empty strings. Numeric fields default to 0;
 * category/tags/updated_at default to "".
 */
export function normalizeTorrentRow(raw: unknown): UnifiedTorrentRow | null {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return null;
  const record = raw as Record<string, any>;
  for (const field of ['instance_id', 'hash', 'name', 'state']) {
    if (typeof record[field] !== 'string' || !record[field].trim()) return null;
  }
  const row: Record<string, any> = { ...record };
  for (const field of NUMERIC_FIELDS) {
    const value = Number(record[field] ?? 0);
    row[field] = Number.isFinite(value) ? value : 0;
  }
  for (const field of ['category', 'tags', 'updated_at']) {
    row[field] = typeof record[field] === 'string' ? record[field] : '';
  }
  return row as UnifiedTorrentRow;
}

/** Normalise a raw list, dropping invalid rows (non-arrays → []). */
export function normalizeTorrentRows(raw: unknown): UnifiedTorrentRow[] {
  return Array.isArray(raw)
    ? raw.map(normalizeTorrentRow).filter((row): row is UnifiedTorrentRow => row !== null)
    : [];
}

/**
 * Apply a QBIT_SYNC_INIT snapshot. With `replaces_instance` all rows of that
 * instance are dropped first. Incoming rows replace existing ones, but media
 * metadata missing from the incoming row is carried over from the old row.
 */
export function applyQbitSyncInit(
  current: Map<string, UnifiedTorrentRow>,
  message: QbitSyncInitMessage,
): Map<string, UnifiedTorrentRow> {
  const prefix = `${message.instance_id}:`;
  const next = message.replaces_instance
    ? new Map([...current].filter(([key]) => !key.startsWith(prefix)))
    : new Map(current);

  for (const row of normalizeTorrentRows(message.torrents)) {
    const key = `${message.instance_id}:${row.hash}`;
    const previous = current.get(key);
    const carried =
      previous &&
      Object.fromEntries(
        STICKY_MEDIA_FIELDS.filter((field) => row[field] === undefined && previous[field] !== undefined).map(
          (field) => [field, previous[field]],
        ),
      );
    next.set(key, { ...carried, ...row });
  }
  return next;
}

/**
 * Keep only selected keys that still exist (optionally restricted to one
 * instance).
 */
export function pruneTorrentSelection(
  selected: Set<string>,
  torrents: Map<string, UnifiedTorrentRow>,
  instanceId?: string,
): Set<string> {
  const existing = new Set(
    [...torrents.values()]
      .filter((row) => !instanceId || row.instance_id === instanceId)
      .map((row) => `${row.instance_id}:${row.hash}`),
  );
  return new Set([...selected].filter((key) => existing.has(key)));
}
