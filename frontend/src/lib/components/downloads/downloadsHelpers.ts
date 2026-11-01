/**
 * Pure helpers shared by the /downloads page and its components
 * (DownloadsTable, DownloadsMobileView, DownloadsContextMenus, DownloadsModals).
 *
 * Reconstructed from the 2026-09-02 build, where they are inlined into the
 * downloads page node (nodes/7). The file name is not attested anywhere;
 * it was chosen during the recovery.
 */
import type { BatchSummary, DisplayRowRef, DownloadState, DownloadTask } from '$lib/stores/downloads.svelte';
import type { UnifiedTorrentRow } from '$lib/stores/qbitReducer';
import { aggregateQueueSource } from './mediaQueue';

/** Status priority for urgency-based sorting (lower = more urgent). */
export const STATUS_PRIORITY: Record<string, number> = {
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

/** `ALL` or a lowercase source key (`direct`, `fshare`, `torrent`, `torbox`). */
export type SourceFilter = 'ALL' | 'direct' | 'fshare' | 'torrent' | 'torbox';
/** `ALL` or an upper-case DownloadState. */
export type StatusFilter = 'ALL' | DownloadState | (string & {});
export type SortColumn = 'added' | 'status' | 'filename' | 'size' | 'progress';
export type SortDirection = 'asc' | 'desc';

export interface SourceCounts {
  all: number;
  fshare: number;
  torrent: number;
  torbox: number;
}

/** A batch summary joined with its (lazily loaded) items. */
export interface BatchGroup {
  batchId: string;
  batchName: string;
  downloads: DownloadTask[];
  /** Total count from the API (downloads is lazy-loaded) */
  totalItems: number;
  totalSize: number;
  downloaded: number;
  progress: number;
  speed: number;
  source: string;
  activeCount: number;
  completedCount: number;
  failedCount: number;
  pausedCount: number;
  queuedCount: number;
  createdAt: string;
}

export type DisplayRow =
  | { id: string; kind: 'batch'; group: BatchGroup }
  | { id: string; kind: 'torrent'; torrent: UnifiedTorrentRow }
  | { id: string; kind: 'download'; download: DownloadTask };

export interface BatchStatus {
  state: string;
  color: string;
  icon: string;
}

/** Target of the batch delete confirmation. */
export interface BatchDeleteTarget {
  batchId: string;
  batchName: string;
  itemCount: number;
}

export interface ContextMenuPosition {
  x: number;
  y: number;
}

export function getStateColor(state: string): string {
  if (state === 'COMPLETED') return '#00ffa3';
  if (state === 'FAILED') return '#FF5252';
  if (state === 'DOWNLOADING' || state === 'EXTRACTING' || state === 'STARTING') return '#00f3ff';
  if (state === 'QUEUED' || state === 'WAITING') return '#ffd700';
  return '#64748b'; // paused/other
}

export function getStateIcon(state: string): string {
  if (state === 'COMPLETED') return 'check_circle';
  if (state === 'FAILED') return 'report_problem';
  if (state === 'DOWNLOADING' || state === 'EXTRACTING' || state === 'STARTING') return 'sync';
  if (state === 'QUEUED' || state === 'WAITING') return 'hourglass_bottom';
  return 'pause_circle';
}

export function formatAddedDate(dateStr: string | null | undefined): string {
  if (!dateStr) return '-';
  try {
    const d = new Date(dateStr);
    return `${d.toLocaleDateString()} ${d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`;
  } catch {
    return '-';
  }
}

/** qbittorrent → torrent, flasharr-http → fshare, torbox → debrid. */
export function normalizeSource(source: string | null | undefined): string {
  const s = (source || 'unknown').toLowerCase();
  if (s === 'qbittorrent') return 'torrent';
  if (s === 'flasharr-http') return 'fshare';
  if (s === 'torbox') return 'debrid';
  return s;
}

export function taskSource(task: Pick<DownloadTask, 'download_source' | 'external_provider' | 'host'>): string {
  return normalizeSource(task.download_source || task.external_provider || task.host);
}

export function isTorrentTask(task: DownloadTask): boolean {
  return taskSource(task) === 'torrent';
}

export function sourceSubline(source: string | null | undefined): string {
  const s = normalizeSource(source);
  if (s === 'torrent') return 'Torrent-backed queue item';
  if (s === 'debrid') return 'Debrid queue item';
  if (s === 'fshare') return 'Fshare queue item';
  return 'Flasharr queue item';
}

export function copySourceLabel(task: DownloadTask): string {
  const s = taskSource(task);
  if (s === 'torrent') return 'Copy torrent hash';
  if (s === 'debrid') return 'Copy Debrid source';
  if (s === 'fshare') return 'Copy source link';
  return 'Copy source';
}

export function isActiveState(state: string): boolean {
  return state === 'DOWNLOADING' || state === 'EXTRACTING' || state === 'STARTING';
}

/** `direct` covers the non-torrent Flasharr sources (Fshare and TorBox). */
export function matchesSourceFilter(filter: SourceFilter, source: string | null | undefined): boolean {
  if (filter === 'ALL') return true;
  const s = normalizeSource(source);
  if (filter === 'direct') return s === 'fshare' || s === 'torbox';
  return s === filter;
}

/** Parse a `?source=` value into a SourceFilter (unknown → `ALL`). */
export function parseSourceFilter(value: string | null | undefined): SourceFilter {
  const v = (value || 'all').toLowerCase();
  if (!['all', 'direct', 'fshare', 'torrent', 'torbox'].includes(v)) return 'ALL';
  return v === 'all' ? 'ALL' : (v as SourceFilter);
}

export function torrentKey(torrent: Pick<UnifiedTorrentRow, 'instance_id' | 'hash'>): string {
  return `${torrent.instance_id}:${torrent.hash}`;
}

export function torrentState(torrent: UnifiedTorrentRow): string {
  return torrent.flasharr_state || 'DOWNLOADING';
}

export function countSources(
  downloads: DownloadTask[],
  batches: BatchSummary[],
  torrents: UnifiedTorrentRow[],
): SourceCounts {
  const counts: SourceCounts = { all: 0, fshare: 0, torrent: 0, torbox: 0 };
  for (const task of downloads) {
    const s = taskSource(task);
    counts.all += 1;
    if (s === 'torrent') counts.torrent += 1;
    else if (s === 'torbox') counts.torbox += 1;
    else if (s === 'fshare') counts.fshare += 1;
  }
  for (const batch of batches) {
    const s = normalizeSource(batch.source);
    counts.all += 1;
    if (s === 'torrent') counts.torrent += 1;
    else if (s === 'torbox') counts.torbox += 1;
    else if (s === 'fshare') counts.fshare += 1;
  }
  for (const _torrent of torrents) {
    counts.all += 1;
    counts.torrent += 1;
  }
  return counts;
}

export function filterStandaloneDownloads(
  downloads: DownloadTask[],
  query: string,
  sourceFilter: SourceFilter,
  sortBy: SortColumn,
  sortDir: SortDirection,
): DownloadTask[] {
  let list = downloads;
  if (sourceFilter !== 'ALL') list = list.filter((task) => matchesSourceFilter(sourceFilter, taskSource(task)));
  if (query.trim()) {
    const q = query.toLowerCase();
    list = list.filter((task) => task.filename.toLowerCase().includes(q) || task.id.toLowerCase().includes(q));
  }

  if (sortBy === 'status') {
    return [...list].sort((a, b) => {
      const diff = (STATUS_PRIORITY[a.state] ?? 99) - (STATUS_PRIORITY[b.state] ?? 99);
      if (diff === 0) return b.progress - a.progress;
      return sortDir === 'asc' ? -diff : diff;
    });
  }
  if (sortBy === 'filename') {
    return [...list].sort((a, b) => {
      const result = a.filename.localeCompare(b.filename, undefined, { numeric: true, sensitivity: 'base' });
      return sortDir === 'asc' ? result : -result;
    });
  }
  if (sortBy === 'progress') {
    return [...list].sort((a, b) => (sortDir === 'asc' ? a.progress - b.progress : b.progress - a.progress));
  }
  if (sortBy === 'size') {
    return [...list].sort((a, b) => (sortDir === 'asc' ? a.size - b.size : b.size - a.size));
  }
  if ((sortBy as string) === 'source') {
    return [...list].sort((a, b) => {
      const result = taskSource(a).localeCompare(taskSource(b), undefined, { numeric: true, sensitivity: 'base' });
      return sortDir === 'asc' ? result : -result;
    });
  }
  // 'added': server order
  return list;
}

export function filterTorrentRows(
  torrents: UnifiedTorrentRow[],
  query: string,
  sourceFilter: SourceFilter,
  statusFilter: StatusFilter,
  sortBy: SortColumn,
  sortDir: SortDirection,
): UnifiedTorrentRow[] {
  let list = torrents;
  if (sourceFilter !== 'ALL' && sourceFilter !== 'torrent') return [];
  if (statusFilter !== 'ALL') list = list.filter((t) => torrentState(t) === statusFilter);
  if (query.trim()) {
    const q = query.toLowerCase();
    list = list.filter((t) => `${t.name} ${t.hash} ${t.tracker} ${t.category} ${t.tags}`.toLowerCase().includes(q));
  }
  const dir = sortDir === 'asc' ? 1 : -1;
  return [...list].sort((a, b) => {
    if (sortBy === 'filename') return dir * a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' });
    if (sortBy === 'status') return dir * ((STATUS_PRIORITY[torrentState(a)] ?? 99) - (STATUS_PRIORITY[torrentState(b)] ?? 99));
    if (sortBy === 'progress') return dir * (a.progress - b.progress);
    if (sortBy === 'size') return dir * (a.size - b.size);
    if ((sortBy as string) === 'source') return 0;
    return dir * (a.added_on - b.added_on);
  });
}

/** Join batch summaries with their cached items (`"<batch_id>|<sourceFilter>"` cache keys). */
export function buildBatchGroups(
  batches: BatchSummary[],
  batchItems: Map<string, DownloadTask[]>,
  sourceFilter: SourceFilter,
): { groups: BatchGroup[] } {
  const groups = batches
    .filter((batch) => sourceFilter === 'ALL' || matchesSourceFilter(sourceFilter, batch.source))
    .map((batch): BatchGroup => {
      const cacheKey = `${batch.batch_id}|${sourceFilter === 'ALL' ? '' : sourceFilter}`;
      const items = [...(batchItems.get(cacheKey) || [])].sort((a, b) =>
        a.filename.localeCompare(b.filename, undefined, { numeric: true, sensitivity: 'base' }),
      );
      return {
        batchId: batch.batch_id,
        batchName: batch.batch_name,
        downloads: items,
        totalItems: batch.total_items,
        totalSize: batch.total_size,
        downloaded: batch.downloaded_size,
        progress: batch.progress,
        speed: batch.speed,
        source: aggregateQueueSource(items, batch.source || 'batch'),
        activeCount: batch.downloading_items,
        completedCount: batch.completed_items,
        failedCount: batch.failed_items,
        pausedCount: batch.paused_items,
        queuedCount: batch.queued_items,
        createdAt: batch.created_at,
      };
    });
  return { groups };
}

export function filterBatchGroups(groups: BatchGroup[], query: string, statusFilter: StatusFilter): BatchGroup[] {
  let list = groups;
  if (query.trim()) {
    const q = query.toLowerCase();
    list = list.filter((group) => group.batchName.toLowerCase().includes(q));
  }
  if (statusFilter === 'ALL') return list;
  return list.filter((group) => {
    if (statusFilter === 'DOWNLOADING') return group.activeCount > 0;
    if (statusFilter === 'PAUSED') return group.pausedCount > 0 && group.completedCount < group.totalItems;
    if (statusFilter === 'COMPLETED') return group.completedCount > 0;
    if (statusFilter === 'FAILED') return group.failedCount > 0;
    if (statusFilter === 'QUEUED') return group.queuedCount > 0;
    return false;
  });
}

/**
 * Resolve the authoritative row order from /api/downloads/batches into renderable rows.
 * Without an order (empty `rowOrder`) batches come first, then standalone downloads, then torrents.
 */
export function buildDisplayRows(
  rowOrder: DisplayRowRef[],
  groups: BatchGroup[],
  downloads: DownloadTask[],
  torrents: UnifiedTorrentRow[],
): DisplayRow[] {
  const groupsById = new Map(groups.map((group) => [group.batchId, group]));
  const standaloneById = new Map(downloads.filter((task) => !task.batch_id).map((task) => [task.id, task]));
  const torrentsByKey = new Map(torrents.map((torrent) => [torrentKey(torrent), torrent]));

  const ordered = rowOrder
    .map((ref): DisplayRow | null => {
      if (ref.kind === 'batch') {
        const group = groupsById.get(ref.id);
        return group ? { id: ref.id, kind: 'batch', group } : null;
      }
      if (ref.kind === 'torrent') {
        const torrent = torrentsByKey.get(ref.id);
        return torrent ? { id: ref.id, kind: 'torrent', torrent } : null;
      }
      const download = standaloneById.get(ref.id);
      return download ? { id: ref.id, kind: 'download', download } : null;
    })
    .filter((row): row is DisplayRow => row !== null);

  if (rowOrder.length > 0) return ordered;
  return [
    ...groups.map((group): DisplayRow => ({ id: group.batchId, kind: 'batch', group })),
    ...downloads
      .filter((task) => !task.batch_id)
      .map((task): DisplayRow => ({ id: task.id, kind: 'download', download: task })),
    ...torrents.map((torrent): DisplayRow => ({ id: torrentKey(torrent), kind: 'torrent', torrent })),
  ];
}

type BatchCounts = Pick<
  BatchGroup,
  'activeCount' | 'completedCount' | 'failedCount' | 'pausedCount' | 'queuedCount' | 'totalItems'
>;

/** Aggregate state of a batch; more than one non-empty bucket reports `MIXED`. */
export function getBatchStatus(group: BatchCounts): BatchStatus {
  const bucketCount = [
    group.failedCount > 0 ? 1 : 0,
    group.activeCount > 0 ? 1 : 0,
    group.pausedCount > 0 ? 1 : 0,
    group.queuedCount > 0 ? 1 : 0,
    group.completedCount > 0 ? 1 : 0,
  ].reduce((sum, n) => sum + n, 0);

  if (bucketCount > 1) {
    let primary = 'QUEUED';
    if (group.failedCount > 0) primary = 'FAILED';
    else if (group.activeCount > 0) primary = 'DOWNLOADING';
    else if (group.pausedCount > 0) primary = 'PAUSED';
    return { state: 'MIXED', color: getStateColor(primary), icon: getStateIcon(primary) };
  }

  const single = (state: string): BatchStatus => ({ state, color: getStateColor(state), icon: getStateIcon(state) });
  if (group.failedCount > 0) return single('FAILED');
  if (group.activeCount > 0) return single('DOWNLOADING');
  if (group.pausedCount > 0 && group.completedCount < group.totalItems) return single('PAUSED');
  if (group.queuedCount > 0) return single('QUEUED');
  return single('COMPLETED');
}
