/**
 * Formatting helpers for the /torrents workspace (queue table, detail panel,
 * add modal). Pure functions; "—" is the placeholder for missing values.
 */

/** 1024-based size; 0 decimals for bytes, 1 otherwise. Non-positive → "0 B". */
export function formatTorrentBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** i).toFixed(i ? 1 : 0)} ${units[i]}`;
}

/** "<size>/s", or "—" when idle. */
export function formatTorrentSpeed(bytesPerSecond: number): string {
  return Number.isFinite(bytesPerSecond) && bytesPerSecond > 0 ? `${formatTorrentBytes(bytesPerSecond)}/s` : '—';
}

/** "1h 5m" / "12m"; "—" for 0/unknown or qBittorrent's ∞ (>= 8640000 s). */
export function formatTorrentEta(seconds: number): string {
  if (!seconds || seconds >= 8_640_000) return '—';
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return hours ? `${hours}h ${minutes}m` : `${minutes}m`;
}

/** 0.0–1.0 → "42.0%". */
export function formatTorrentProgress(progress: number): string {
  return `${((progress || 0) * 100).toFixed(1)}%`;
}

/** Unix seconds → locale string; "—" for 0/negative. */
export function formatTorrentDate(unixSeconds: number): string {
  return unixSeconds > 0 ? new Date(unixSeconds * 1000).toLocaleString() : '—';
}

/** First non-empty (not undefined/null/"") value among `keys`, else `fallback`. */
export function pickField(record: any, keys: string[], fallback: any = '—'): any {
  for (const key of keys) {
    if (record?.[key] !== undefined && record[key] !== null && record[key] !== '') return record[key];
  }
  return fallback;
}

/** Peers payload (array | { peers: array | map }) → array of peer objects. */
export function peerList(payload: any): any[] {
  return Array.isArray(payload)
    ? payload
    : Array.isArray(payload?.peers)
      ? payload.peers
      : payload?.peers && typeof payload.peers === 'object'
        ? Object.values(payload.peers)
        : [];
}

/** Like peerList, but map entries keep their key as `endpoint`. */
export function peerEntries(payload: any): any[] {
  return Array.isArray(payload)
    ? payload
    : Array.isArray(payload?.peers)
      ? payload.peers
      : payload?.peers && typeof payload.peers === 'object'
        ? Object.entries(payload.peers).map(([endpoint, peer]) => ({ endpoint, ...(peer as object) }))
        : [];
}

export function peerEndpoint(peer: any): string {
  return String(peer?.endpoint || peer?.ip || peer?.addr || '').trim() || '—';
}

export function peerFlags(peer: any): string {
  return pickField(peer, ['flags_desc', 'flags', 'connection'], '—');
}

/** qBittorrent tracker status code → label. */
export function trackerStatusLabel(status: unknown): string {
  return (
    ({ 0: 'disabled', 1: 'not contacted', 2: 'working', 3: 'updating', 4: 'not working' } as Record<number, string>)[
      Number(status)
    ] || String(status ?? 'unknown')
  );
}

export type TrackerStatusTone = 'ok' | 'warn' | 'error' | 'muted';

export function trackerStatusTone(status: unknown): TrackerStatusTone {
  const label = trackerStatusLabel(status);
  return label === 'working' ? 'ok' : label === 'updating' ? 'warn' : label === 'not working' ? 'error' : 'muted';
}

/** Tracker URL → host; DHT/PeX/LSD pseudo-trackers and invalid URLs are returned as-is. */
export function trackerHost(url: string): string {
  if (!url || url === '** [DHT] **' || url === '** [PeX] **' || url === '** [LSD] **') return url || '—';
  try {
    return new URL(url).host || url;
  } catch {
    return url;
  }
}

/** Downsample piece states to at most ~`maxCells` cells (max per bucket). */
export function downsamplePieces(states: number[], maxCells = 160): number[] {
  if (!states.length) return [];
  const bucket = Math.max(1, Math.ceil(states.length / maxCells));
  const cells: number[] = [];
  for (let i = 0; i < states.length; i += bucket) {
    cells.push(Math.max(...states.slice(i, i + bucket)));
  }
  return cells;
}

/** qBittorrent piece state (0 missing, 1 downloading, 2 have) → CSS modifier. */
export function pieceStateClass(state: number): 'done' | 'partial' | 'empty' {
  return state >= 2 ? 'done' : state === 1 ? 'partial' : 'empty';
}

/** Last path segment (either slash). */
export function fileBaseName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

/** Parent directory of a path, or "" at the root. */
export function parentPath(path: string): string {
  const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return index > 0 ? path.slice(0, index) : '';
}
