/**
 * Media queue model for the Downloads page.
 *
 * Turns the download store's caches (standalone tasks, batch summaries,
 * expanded batch items, unmanaged qBittorrent torrents and the backend's
 * authoritative row order) into media-keyed groups and a
 * Movie / TV Show → Season → Episode hierarchy, plus the batch-item cache
 * reducers used by the store.
 *
 * Pure functions only (no runtime imports) so the module can be unit-tested
 * with `node --experimental-strip-types`.
 */
import type {
  BatchSummary,
  DisplayRowRef,
  DownloadTask,
} from '$lib/stores/downloads.svelte';
import type { UnifiedTorrentRow } from '$lib/stores/qbitReducer';

// ============================================================================
// Types
// ============================================================================

/**
 * Anything that can appear as a queue row: a download task, a batch
 * placeholder (`id: "batch:<batch_id>"`) or an unmanaged torrent
 * (`id: "torrent:<instance>:<hash>"`).
 */
export type MediaQueueItem = Omit<Partial<DownloadTask>, 'id' | 'state' | 'download_source'> & {
  id: string;
  state?: string;
  download_source?: string | null;
  /** Optional episode title (TV leaves). */
  episode_title?: string | null;
  /** Marks the current/active attempt among duplicates of the same media key. */
  is_current?: boolean;
};

/** Normalised source of a queue item (`unknown` when nothing is known). */
export type QueueSource = 'fshare' | 'torrent' | 'debrid' | 'torbox' | 'mixed' | 'unknown' | (string & {});

export type MediaQueueMediaType = 'movie' | 'tv' | 'unknown';

/** Items sharing one canonical media key (see mediaQueueKey). */
export interface MediaQueueGroup {
  key: string;
  title: string;
  mediaType: MediaQueueMediaType;
  items: MediaQueueItem[];
}

export interface MediaQueueFilter {
  query: string;
  /** `all` or a QueueSource */
  source: string;
  /** `all` or a lower-cased queueStatusLabel() value */
  status: string;
  mediaType: 'all' | MediaQueueMediaType;
  /** `title`, `status`, anything else = by created_at. Falsy keeps backend order. Always descending. */
  sort?: 'title' | 'status' | 'added' | string | null;
}

export interface MediaQueueEpisode {
  key: string;
  title: string;
  item: MediaQueueItem;
}

export interface MediaQueueSeason {
  /** -1 when the season is unknown */
  season: number;
  episodes: MediaQueueEpisode[];
}

export type MediaQueueNode =
  | { kind: 'movie' | 'unknown'; key: string; title: string; item: MediaQueueItem }
  | { kind: 'series'; key: string; title: string; source: QueueSource; seasons: MediaQueueSeason[] };

export interface EpisodeCoordinates {
  season: number | undefined;
  episode: number | undefined;
}

/** Display status used by filters and badges. */
export type QueueStatusLabel =
  | 'Disconnected'
  | 'Sync stale'
  | 'Waiting for metadata'
  | 'Seeding stalled'
  | 'Waiting for peers'
  | 'Paused'
  | 'Checking'
  | 'Seeding'
  | 'Queued'
  | 'Starting'
  | 'Downloading'
  | 'Importing'
  | 'Complete'
  | 'Failed'
  | 'Cancelled';

/** batch_id → items, keyed `"<batch_id>|<sourceFilter>"` in the store. */
export type BatchItemCache = Map<string, DownloadTask[]>;

// ============================================================================
// Identity
// ============================================================================

const SXXEYY = /(?:^|[^a-z0-9])s(\d{1,2})e(\d{1,4})(?=$|[^a-z0-9])/i;

/**
 * Season/episode of an item. TMDB fields win, except that a `0` TMDB value is
 * overridden by a non-zero `SxxEyy` parsed from the filename.
 */
export function episodeCoordinates(item: MediaQueueItem): EpisodeCoordinates {
  const match = item.filename?.match(SXXEYY);
  const parsedSeason = match ? Number(match[1]) : undefined;
  const parsedEpisode = match ? Number(match[2]) : undefined;

  const season =
    parsedSeason != null && item.tmdb_season === 0 && parsedSeason !== 0
      ? parsedSeason
      : item.tmdb_season != null
        ? Number(item.tmdb_season)
        : parsedSeason;
  const episode =
    parsedEpisode != null && item.tmdb_episode === 0 && parsedEpisode !== 0
      ? parsedEpisode
      : item.tmdb_episode != null
        ? Number(item.tmdb_episode)
        : parsedEpisode;

  return { season, episode };
}

/**
 * Canonical media key:
 * `tv:<tvdb>:sNN:eNN` | `movie:<tmdb>` | `unresolved:<id>`.
 */
export function mediaQueueKey(item: MediaQueueItem): string {
  const mediaType = item.media_type?.toLowerCase();
  const { season, episode } = episodeCoordinates(item);
  if (mediaType === 'tv' && item.tvdb_id != null && season != null && episode != null) {
    return `tv:${Number(item.tvdb_id)}:s${String(season).padStart(2, '0')}:e${String(episode).padStart(2, '0')}`;
  }
  if (mediaType === 'movie' && item.tmdb_id) return `movie:${item.tmdb_id}`;
  return `unresolved:${item.id}`;
}

// ============================================================================
// Source
// ============================================================================

/** Normalised source: qbittorrent → torrent, flasharr-http → fshare. */
export function queueItemSource(item: Pick<MediaQueueItem, 'download_source' | 'external_provider' | 'host'>): QueueSource {
  const raw = (item.download_source || item.external_provider || item.host || 'unknown').toLowerCase();
  return raw === 'qbittorrent' ? 'torrent' : raw === 'flasharr-http' ? 'fshare' : raw;
}

/**
 * Aggregate source of several items: the single known source, `mixed` when
 * they differ, or the normalised `fallback` when there are no items/none known.
 */
export function aggregateQueueSource(items: MediaQueueItem[], fallback = 'unknown'): QueueSource {
  if (!items.length) return queueItemSource({ download_source: fallback });
  const sources = new Set(items.map(queueItemSource).filter((s) => s !== 'unknown'));
  if (sources.size > 1) return 'mixed';
  return sources.values().next().value || queueItemSource({ download_source: fallback });
}

// ============================================================================
// Rows
// ============================================================================

/** Present an unmanaged qBittorrent torrent as a queue item. */
export function torrentRowToQueueItem(row: UnifiedTorrentRow): MediaQueueItem {
  return {
    id: `torrent:${row.instance_id}:${row.hash}`,
    filename: row.name,
    state: row.flasharr_state || row.state,
    task_state: row.flasharr_state || row.state,
    qbit_state: row.state,
    download_source: 'torrent',
    progress: row.progress * 100,
    size: row.size,
    speed: row.dlspeed,
    created_at: row.added_on > 0 ? new Date(row.added_on * 1000).toISOString() : row.updated_at,
  };
}

/**
 * Flatten the store caches into queue items.
 *
 * Batches whose items are loaded are replaced by those items; unloaded batches
 * become a `batch:<id>` placeholder. With `displayRows` (the backend's
 * authoritative order) the result follows that order and drops anything not
 * listed; without it the order is downloads, batch items, placeholders, torrents.
 */
export function mergeQueueRows(
  downloads: DownloadTask[],
  batches: BatchSummary[],
  batchItems: BatchItemCache,
  torrents: UnifiedTorrentRow[] = [],
  displayRows: DisplayRowRef[] = [],
): MediaQueueItem[] {
  const loadedItems = Array.from(batchItems.values()).flat();
  const loadedBatchIds = new Set(Array.from(batchItems.keys(), (key) => key.split('|', 1)[0]));
  const batchPlaceholders: MediaQueueItem[] = batches
    .filter((batch) => !loadedBatchIds.has(batch.batch_id))
    .map((batch) => ({
      id: `batch:${batch.batch_id}`,
      batch_name: batch.batch_name,
      filename: batch.batch_name,
      state: batch.state,
      download_source: batch.source,
      created_at: batch.created_at,
    }));

  if (!displayRows.length) {
    return [...downloads, ...loadedItems, ...batchPlaceholders, ...torrents.map(torrentRowToQueueItem)];
  }

  const downloadsById = new Map<string, MediaQueueItem>(downloads.map((d) => [d.id, d]));
  const placeholdersByBatch = new Map(batchPlaceholders.map((p) => [p.id.slice('batch:'.length), p]));
  const itemsByBatch = new Map<string, MediaQueueItem[]>();
  for (const [key, items] of batchItems) itemsByBatch.set(key.split('|', 1)[0], items);
  const torrentsById = new Map(torrents.map((t) => [`${t.instance_id}:${t.hash}`, torrentRowToQueueItem(t)]));

  return displayRows.flatMap((row): MediaQueueItem | MediaQueueItem[] =>
    row.kind === 'download'
      ? downloadsById.get(row.id) || []
      : row.kind === 'torrent'
        ? torrentsById.get(row.id) || []
        : itemsByBatch.get(row.id) || placeholdersByBatch.get(row.id) || [],
  );
}

// ============================================================================
// Hierarchy
// ============================================================================

/**
 * Build the Movie / TV Show → Season → Episode tree. Movies and unresolved
 * items are leaves; TV episodes are grouped under `series:<tvdb>`, seasons
 * sorted ascending, and each series gets an aggregated source.
 */
export function buildMediaQueueTree(items: MediaQueueItem[]): MediaQueueNode[] {
  const nodes: MediaQueueNode[] = [];
  const seriesByKey = new Map<string, Extract<MediaQueueNode, { kind: 'series' }>>();

  for (const item of items) {
    const key = mediaQueueKey(item);
    if (!key.startsWith('tv:')) {
      nodes.push({
        kind: key.startsWith('movie:') ? 'movie' : 'unknown',
        key,
        title: item.batch_name || item.tmdb_title || item.filename || 'Untitled',
        item,
      });
      continue;
    }

    const seriesKey = `series:${Number(item.tvdb_id)}`;
    let series = seriesByKey.get(seriesKey);
    if (!series) {
      series = {
        kind: 'series',
        key: seriesKey,
        title: item.batch_name || item.tmdb_title || item.filename || 'Untitled',
        source: 'unknown',
        seasons: [],
      };
      seriesByKey.set(seriesKey, series);
      nodes.push(series);
    }

    const coords = episodeCoordinates(item);
    const seasonNumber = coords.season ?? -1;
    let season = series.seasons.find((s) => s.season === seasonNumber);
    if (!season) {
      season = { season: seasonNumber, episodes: [] };
      series.seasons.push(season);
    }
    season.episodes.push({
      key,
      title:
        item.episode_title ||
        (coords.episode != null ? `Episode ${coords.episode}` : item.filename || 'Unknown episode'),
      item,
    });
  }

  for (const series of seriesByKey.values()) {
    series.source = aggregateQueueSource(series.seasons.flatMap((s) => s.episodes.map((e) => e.item)));
    series.seasons.sort((a, b) => a.season - b.season);
  }
  return nodes;
}

/** Group items by mediaQueueKey, preserving first-seen order. */
export function groupQueueItemsByMedia(items: MediaQueueItem[]): MediaQueueGroup[] {
  const groups = new Map<string, MediaQueueGroup>();
  for (const item of items) {
    const key = mediaQueueKey(item);
    const mediaType: MediaQueueMediaType = key.startsWith('movie:')
      ? 'movie'
      : key.startsWith('tv:')
        ? 'tv'
        : 'unknown';
    const group = groups.get(key);
    if (group) group.items.push(item);
    else groups.set(key, { key, title: item.batch_name || item.filename || 'Untitled', mediaType, items: [item] });
  }
  return [...groups.values()];
}

// ============================================================================
// Status
// ============================================================================

const TASK_STATE_LABELS: Record<string, QueueStatusLabel> = {
  QUEUED: 'Queued',
  STARTING: 'Starting',
  DOWNLOADING: 'Downloading',
  WAITING: 'Queued',
  EXTRACTING: 'Importing',
  IMPORTING: 'Importing',
  COMPLETED: 'Complete',
  FAILED: 'Failed',
  CANCELLED: 'Cancelled',
  SKIPPED: 'Cancelled',
};

/**
 * Human status of a queue item. Torrent sync/transfer details (qB state,
 * transfer_state, sync_health) take precedence over the task state.
 */
export function queueStatusLabel(item: MediaQueueItem): QueueStatusLabel {
  if (item.sync_health === 'disconnected') return 'Disconnected';
  if (item.sync_health === 'stale') return 'Sync stale';

  const transfer = (item.transfer_state || '').toLowerCase();
  const qbit = (item.qbit_state || '').toLowerCase();
  const task = (item.task_state || item.state || 'QUEUED').toUpperCase();

  if (transfer.includes('metadata') || qbit.includes('meta')) return 'Waiting for metadata';
  if (qbit.includes('stalledup')) return 'Seeding stalled';
  if (transfer.includes('peer') || qbit.includes('stalled')) return 'Waiting for peers';
  if (transfer.includes('paused') || qbit.includes('pause') || task === 'PAUSED') return 'Paused';
  if (transfer.includes('check') || qbit.includes('check')) return 'Checking';
  if (qbit.includes('upload') || qbit.endsWith('up')) return 'Seeding';
  return TASK_STATE_LABELS[task] || 'Queued';
}

/**
 * The representative item of a group: `is_current` first, then newest
 * `created_at`, then id (descending).
 */
export function pickCurrentQueueItem(items: MediaQueueItem[]): MediaQueueItem {
  return [...items].sort(
    (a, b) =>
      +(b.is_current === true) - +(a.is_current === true) ||
      (Date.parse(b.created_at || '') || 0) - (Date.parse(a.created_at || '') || 0) ||
      b.id.localeCompare(a.id),
  )[0];
}

/**
 * Filter items inside each group (query over title/filename/id, source,
 * status label, media type), dropping empty groups, then optionally sort
 * groups descending by title, status label or created_at of the current item.
 */
export function filterMediaQueueGroups(groups: MediaQueueGroup[], filter: MediaQueueFilter): MediaQueueGroup[] {
  const query = filter.query.trim().toLowerCase();
  const filtered = groups.flatMap((group) => {
    const items = group.items.filter(
      (item) =>
        (!query || `${group.title} ${item.filename || ''} ${item.id}`.toLowerCase().includes(query)) &&
        (filter.source === 'all' || queueItemSource(item) === filter.source) &&
        (filter.status === 'all' || queueStatusLabel(item).toLowerCase() === filter.status.toLowerCase()) &&
        (filter.mediaType === 'all' || group.mediaType === filter.mediaType),
    );
    return items.length ? [{ ...group, items }] : [];
  });

  if (!filter.sort) return filtered;
  const direction = -1;
  return filtered.sort((a, b) =>
    filter.sort === 'title'
      ? direction * a.title.localeCompare(b.title, undefined, { numeric: true })
      : filter.sort === 'status'
        ? direction * queueStatusLabel(pickCurrentQueueItem(a.items)).localeCompare(queueStatusLabel(pickCurrentQueueItem(b.items)))
        : direction *
          ((Date.parse(pickCurrentQueueItem(a.items).created_at || '') || 0) -
            (Date.parse(pickCurrentQueueItem(b.items).created_at || '') || 0)),
  );
}

// ============================================================================
// Batch item cache reducers (used by the download store)
// ============================================================================

/**
 * Merge `task` into every cached item list of its batch (any source filter),
 * updating an existing entry or appending a new one. Returns a new Map.
 */
export function upsertBatchItemCache(cache: BatchItemCache, task: DownloadTask): BatchItemCache {
  if (!task.batch_id) return cache;
  const next = new Map(cache);
  for (const [key, items] of cache) {
    if (key.split('|', 1)[0] !== task.batch_id) continue;
    next.set(
      key,
      items.some((item) => item.id === task.id)
        ? items.map((item) => (item.id === task.id ? { ...item, ...task } : item))
        : [...items, task],
    );
  }
  return next;
}

/** Remove a task id from every cached batch item list. Returns a new Map. */
export function removeFromBatchItemCache(cache: BatchItemCache, taskId: string): BatchItemCache {
  return new Map([...cache].map(([key, items]) => [key, items.filter((item) => item.id !== taskId)]));
}
