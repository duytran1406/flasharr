/**
 * Smart Search result helpers: source classification, ranking, per-season /
 * per-episode release summaries, normalisation into the UI result model,
 * badges, and the extra download-request fields for torrent/debrid picks.
 *
 * Inputs come straight from /api/search/smart (backend SmartSearchResponse,
 * SeasonGroup, TorrentPackage, SmartSearchResult) and may use snake_case or
 * camelCase keys, so they are typed loosely.
 */

// ============================================================================
// Types
// ============================================================================

/** One release as returned by the backend (SmartSearchResult + optional extras). */
export interface RawSearchResult {
  name?: string;
  url?: string;
  size?: number;
  score?: number;
  total_score?: number;
  quality?: string;
  quality_name?: string;
  resolution?: string;
  match_type?: string;
  result_source?: string;
  provider?: string;
  provider_id?: string;
  indexer?: string;
  seeders?: number;
  leechers?: number;
  info_hash?: string;
  download_ref?: string;
  origin_provider?: string;
  origin_indexer?: string;
  cached?: boolean;
  availability?: Availability;
  file_idx?: number;
  sources?: unknown;
  behavior_hints?: { filename?: string; [key: string]: unknown };
  inspection_score?: number;
  vietsub?: boolean;
  vietdub?: boolean;
  hdr?: boolean;
  dolby_vision?: boolean;
  /** Set on synthetic entries built from a multi-episode package. */
  packageSelection?: boolean;
  coveredEpisodes?: number[];
  [key: string]: any;
}

export type Availability = 'cached' | 'partially_cached' | 'unknown' | (string & {});

/** `fshare` | `torrent` | `debrid` | other provider ids. */
export type ResultSource = 'fshare' | 'torrent' | 'debrid' | (string & {});

/** Source tab filter: `all`, `best` or a ResultSource. */
export type SourceFilter = 'all' | 'best' | ResultSource;

export interface ReleaseSummary {
  /** humanised match type ("Exact", "Alias", ...) */
  fit: string;
  quality: string;
  /** "Cached" | "Partial" | "<n> seeders" | "Unknown" */
  health: string;
  size: string;
}

export interface EpisodeReleaseRow extends ReleaseSummary {
  /** `SxxEyy` */
  key: string;
  title: string;
  releaseCount: number;
  downloaded: boolean;
  /** no release and not downloaded */
  missing: boolean;
}

/** UI model of a release (what result cards and download actions consume). */
export interface NormalizedSearchResult {
  providerId: ResultSource;
  source: ResultSource;
  kind: 'movie' | 'episode' | (string & {});
  name: string;
  quality?: string;
  resolution?: string;
  size?: number;
  score?: number;
  downloadRef?: string;
  /** omitted for torrent/debrid results that have a download_ref */
  url?: string;
  provider?: string;
  acquisition?: { method: 'reference' | 'url'; value: string };
  seeders?: number;
  leechers?: number;
  indexer?: string;
  infoHash?: string;
  fileIdx?: number;
  sources?: unknown;
  behaviorHints?: { filename?: string; [key: string]: unknown };
  cached?: boolean;
  availability?: Availability;
  vietsub: boolean;
  vietdub: boolean;
  hdr: boolean;
  dolbyVision: boolean;
}

/** Download-request fields for torrent/debrid picks (merged into AddDownloadRequest). */
export interface AcquisitionFields {
  source?: 'debrid' | 'torrent';
  provider?: 'torbox' | 'qbittorrent';
  seeders?: number;
  leechers?: number;
  info_hash?: string;
  file_idx?: number;
  source_filename?: string;
  download_ref?: string;
  origin_provider?: string;
  origin_indexer?: string;
  /** debrid only */
  strategy?: 'torbox_http' | 'local_qb';
}

export interface ResultBadge {
  text: string;
  variant: 'source' | 'success' | 'warning' | 'default' | 'quality' | 'language' | 'hdr' | 'dv';
  color?: string;
  noDot?: boolean;
}

// ============================================================================
// Source
// ============================================================================

/**
 * Source id of a raw result. TorBox results count as `debrid` when cached,
 * otherwise as `torrent`.
 */
export function resultSource(result: RawSearchResult): ResultSource {
  const source =
    result.result_source ??
    result.resultSource ??
    result.provider_id ??
    result.providerId ??
    result.provider ??
    'unknown';
  if (source === 'torbox') return result.cached === true ? 'debrid' : 'torrent';
  return source;
}

export function matchesSourceFilter(result: RawSearchResult, filter: SourceFilter): boolean {
  return filter === 'all' || filter === 'best' || resultSource(result) === filter;
}

/** Every release of a response: season packages + episode files, or quality-group files. */
export function collectResultFiles(response: any): RawSearchResult[] {
  if ((response?.seasons?.length ?? 0) > 0) {
    return response.seasons.flatMap((season: any) => [
      ...(season.packages ?? season.seasonPackages ?? []).flatMap((pkg: any) => (pkg.result ? [pkg.result] : [])),
      ...(season.episodes_grouped ?? season.episodesGrouped ?? []).flatMap((group: any) => group.files ?? []),
    ]);
  }
  if ((response?.groups?.length ?? 0) > 0) {
    return response.groups.flatMap((group: any) => group.files ?? []);
  }
  return [];
}

/** True when the response has any result, release or season plan. */
export function hasSmartSearchResults(response: any): boolean {
  return (
    Number(response?.total_found ?? response?.totalFound ?? 0) > 0 ||
    collectResultFiles(response).length > 0 ||
    !!(response?.seasonPlan ?? response?.season_plan)
  );
}

// ============================================================================
// Ranking
// ============================================================================

function firstNumber(record: any, ...keys: string[]): number | null {
  const value = keys.map((key) => record?.[key]).find((v) => typeof v === 'number' && Number.isFinite(v));
  return typeof value === 'number' ? value : null;
}

/** Descending compare with nulls last. */
function compareNullableDesc(a: number | null, b: number | null): number {
  if (a === null && b === null) return 0;
  if (a === null) return 1;
  if (b === null) return -1;
  return b - a;
}

function matchTypeRank(result: RawSearchResult): number {
  switch (result?.match_type ?? result?.matchType) {
    case 'alias':
      return 0;
    case 'exact':
      return 1;
    case 'fuzzy':
    case 'keyword_overlap':
    case 'tv_match':
      return 2;
    default:
      return 3;
  }
}

/**
 * Stable ranking: inspection score ↓, match type (alias, exact, fuzzy…),
 * seeders ↓, total score ↓, score ↓, original order.
 */
export function rankReleases<T extends RawSearchResult>(files: T[]): T[] {
  return files
    .map((file, index) => ({ file, index }))
    .sort((a, b) => {
      const inspection =
        (firstNumber(b.file, 'inspection_score', 'inspectionScore') ?? 0) -
        (firstNumber(a.file, 'inspection_score', 'inspectionScore') ?? 0);
      if (inspection) return inspection;
      const match = matchTypeRank(a.file) - matchTypeRank(b.file);
      if (match) return match;
      const seeders = (firstNumber(b.file, 'seeders') ?? 0) - (firstNumber(a.file, 'seeders') ?? 0);
      if (seeders) return seeders;
      const total = compareNullableDesc(
        firstNumber(a.file, 'total_score', 'totalScore'),
        firstNumber(b.file, 'total_score', 'totalScore'),
      );
      return total || compareNullableDesc(firstNumber(a.file, 'score'), firstNumber(b.file, 'score')) || a.index - b.index;
    })
    .map(({ file }) => file);
}

// ============================================================================
// Summaries
// ============================================================================

function humanizeLabel(value: unknown): string {
  if (typeof value !== 'string' || !value.trim()) return 'Unknown';
  const text = value.trim().replaceAll('_', ' ');
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/** Fit / quality / health / size of the best release (null → all "Unknown"). */
export function releaseSummary(result: RawSearchResult | null): ReleaseSummary {
  const availability = result?.availability;
  const health =
    availability === 'cached' || result?.cached === true
      ? 'Cached'
      : availability === 'partially_cached'
        ? 'Partial'
        : typeof result?.seeders === 'number'
          ? `${result.seeders} seeders`
          : 'Unknown';
  const size = typeof result?.size === 'number' && result.size > 0 ? formatResultSize(result.size) : 'Size unavailable';
  return {
    fit: humanizeLabel(result?.match_type ?? result?.matchType),
    quality: humanizeLabel(result?.quality_name ?? result?.qualityName ?? result?.quality ?? result?.resolution),
    health,
    size,
  };
}

/**
 * Releases from a season's torrent packages that pass `filter`.
 * With `episode`: only packages covering it, resolved to the per-episode file
 * (or the package result with that file index). Without: single-episode
 * packages resolve to that episode, multi-episode ones become package
 * selections (`packageSelection: true`, `coveredEpisodes`).
 */
export function seasonPackageReleases(season: any, filter: SourceFilter, episode?: number): RawSearchResult[] {
  return (season?.packages ?? [])
    .map((pkg: any): RawSearchResult | null => {
      const covered: number[] = pkg.coveredEpisodes ?? pkg.covered_episodes ?? [];
      const target = episode ?? (covered.length === 1 ? covered[0] : undefined);
      const episodeFiles = pkg.episodeFiles ?? pkg.episode_files ?? {};
      const fileIndices = pkg.fileIndices ?? pkg.file_indices ?? {};
      if (episode !== undefined && !covered.includes(episode)) return null;
      if (target !== undefined && episodeFiles[target]) return episodeFiles[target];
      return {
        ...pkg.result,
        packageSelection: target === undefined,
        coveredEpisodes: covered,
        ...(target !== undefined && fileIndices[target] !== undefined ? { file_idx: fileIndices[target] } : {}),
      };
    })
    .filter((release: RawSearchResult | null): release is RawSearchResult => release !== null && matchesSourceFilter(release, filter));
}

/** Any season has a package release or a (non-zero) episode file passing `filter`. */
export function seasonsHaveReleases(seasons: any[], filter: SourceFilter): boolean {
  return seasons.some(
    (season) =>
      seasonPackageReleases(season, filter).length > 0 ||
      (season?.episodes_grouped ?? []).some(
        (group: any) =>
          group.episode_number !== 0 && (group.files ?? []).some((file: RawSearchResult) => matchesSourceFilter(file, filter)),
      ),
  );
}

/** False for whole-package selections. */
export function isSingleFileRelease(release: RawSearchResult | null | undefined): boolean {
  return release?.packageSelection !== true;
}

/** Summary row for one episode group of a season. `downloaded` holds `SxxEyy` keys. */
export function episodeReleaseRow(
  seasonNumber: number,
  episodeGroup: any,
  filter: SourceFilter,
  downloaded: Set<string>,
): EpisodeReleaseRow {
  const episodeNumber = Number(episodeGroup?.episode_number ?? episodeGroup?.episodeNumber ?? 0);
  const key = `S${String(seasonNumber).padStart(2, '0')}E${String(episodeNumber).padStart(2, '0')}`;
  const releases = rankReleases(
    (episodeGroup?.files ?? []).filter((file: RawSearchResult) => matchesSourceFilter(file, filter)),
  );
  const summary = releases[0] ? releaseSummary(releases[0]) : releaseSummary(null);
  const isDownloaded = downloaded.has(key);
  return {
    key,
    title: episodeGroup?.name || `Episode ${episodeNumber}`,
    releaseCount: releases.length,
    ...summary,
    downloaded: isDownloaded,
    missing: releases.length === 0 && !isDownloaded,
  };
}

// ============================================================================
// Normalisation + download fields
// ============================================================================

const VIE_TAG = /\.vie[. [\-_]/i;
const HDR_TAG = /\.hdr[. [\-_]/i;
const DV_TAG = /\.dv[. [\-_]/i;

/** Convert a raw release into the UI result model (flags inferred from the name when absent). */
export function normalizeSearchResult(
  raw: RawSearchResult,
  kind: NormalizedSearchResult['kind'] = 'movie',
): NormalizedSearchResult {
  const source = resultSource(raw);
  const name = raw.name ?? '';
  const lower = name.toLowerCase();
  const downloadRef = raw.download_ref ?? raw.downloadRef;
  return {
    providerId: source,
    source,
    kind,
    name,
    quality: raw.quality,
    resolution: raw.resolution,
    size: raw.size,
    score: raw.score,
    downloadRef,
    url: (source === 'torrent' || source === 'debrid') && downloadRef ? undefined : raw.url,
    provider:
      source === 'debrid'
        ? (raw.origin_provider ?? raw.originProvider ?? raw.provider ?? 'torbox')
        : source === 'torrent'
          ? (raw.provider ?? 'qbittorrent')
          : raw.provider,
    acquisition: downloadRef
      ? { method: 'reference', value: downloadRef }
      : raw.url
        ? { method: 'url', value: raw.url }
        : undefined,
    seeders: raw.seeders,
    leechers: raw.leechers,
    indexer: raw.origin_indexer ?? raw.originIndexer ?? raw.indexer,
    infoHash: raw.info_hash ?? raw.infoHash,
    fileIdx: raw.file_idx ?? raw.fileIdx,
    sources: raw.sources,
    behaviorHints: raw.behavior_hints ?? raw.behaviorHints,
    cached: raw.cached,
    availability: raw.availability ?? (raw.cached === true ? 'cached' : undefined),
    vietsub: raw.vietsub ?? (lower.includes('vietsub') || lower.includes('.sub.')),
    vietdub: raw.vietdub ?? (lower.includes('.vie.') || lower.includes('[vie]') || VIE_TAG.test(name)),
    hdr: raw.hdr ?? (lower.includes('.hdr.') || lower.includes('[hdr]') || HDR_TAG.test(name)),
    dolbyVision:
      raw.dolby_vision ?? raw.dolbyVision ?? (lower.includes('.dv.') || lower.includes('[dv]') || DV_TAG.test(name)),
  };
}

/** Cached debrid items download over HTTP from TorBox; others go through local qBittorrent. */
export function debridStrategy(result: NormalizedSearchResult): 'torbox_http' | 'local_qb' {
  return result.availability === 'cached' ? 'torbox_http' : 'local_qb';
}

/** Extra AddDownloadRequest fields for torrent/debrid results ({} for Fshare). */
export function acquisitionFields(result: NormalizedSearchResult): AcquisitionFields {
  if (result.source === 'debrid') {
    return {
      source: 'debrid',
      provider: 'torbox',
      seeders: result.seeders,
      leechers: result.leechers,
      info_hash: result.infoHash,
      file_idx: result.fileIdx,
      source_filename: result.behaviorHints?.filename ?? result.name,
      download_ref: result.downloadRef,
      origin_provider: result.provider,
      origin_indexer: result.indexer,
      strategy: debridStrategy(result),
    };
  }
  if (result.source === 'torrent') {
    return {
      source: 'torrent',
      provider: 'qbittorrent',
      seeders: result.seeders,
      leechers: result.leechers,
      info_hash: result.infoHash,
      file_idx: result.fileIdx,
      source_filename: result.behaviorHints?.filename ?? result.name,
      download_ref: result.downloadRef,
    };
  }
  return {};
}

/** URL to send with the download request (none when a download_ref is used). */
export function downloadUrl(result: NormalizedSearchResult): string | undefined {
  return result.downloadRef ? undefined : result.url;
}

function sourceBadge(result: NormalizedSearchResult): ResultBadge {
  if (result.source === 'debrid') return { text: 'TORBOX', variant: 'source' };
  if (result.source === 'torrent') return { text: 'TORRENT', variant: 'source' };
  return { text: 'FSHARE', variant: 'source' };
}

/** One-line provenance, e.g. "torbox · cached · indexer" or "torrent · 12 seed · 3 leech". */
export function resultSourceMeta(result: NormalizedSearchResult): string {
  if (result.source === 'debrid') {
    const parts = ['torbox'];
    if (result.cached) parts.push('cached');
    if (result.provider && result.provider !== 'torbox') parts.push(result.provider);
    if (result.indexer) parts.push(result.indexer);
    return parts.join(' · ');
  }
  if (result.source === 'torrent') {
    const parts = [result.provider === 'stremio' ? 'Stremio' : 'torrent'];
    if (result.seeders != null) parts.push(`${result.seeders} seed`);
    if (result.leechers != null) parts.push(`${result.leechers} leech`);
    if (result.indexer) parts.push(result.indexer);
    return parts.join(' · ');
  }
  const parts = ['fshare'];
  if (result.provider && result.provider !== 'fshare') parts.push(result.provider);
  return parts.join(' · ');
}

/** Badge list for a result card (source, availability, seed/leech, language, HDR/DV, quality). */
export function resultBadges(result: NormalizedSearchResult): ResultBadge[] {
  const badges: ResultBadge[] = [sourceBadge(result)];
  if (result.availability === 'cached' || result.cached) badges.push({ text: 'CACHED', variant: 'success' });
  if (result.availability === 'partially_cached') badges.push({ text: 'PARTIAL', variant: 'warning' });
  if (result.availability === 'unknown') badges.push({ text: 'UNKNOWN', variant: 'default' });
  if (result.source === 'torrent' || result.source === 'debrid') {
    badges.push({
      text: `Seed ${result.seeders ?? 0}`,
      variant: 'quality',
      color: (result.seeders ?? 0) > 0 ? '#22c55e' : '#ef4444',
    });
    badges.push({ text: `Leech ${result.leechers ?? 0}`, variant: 'source', color: '#f59e0b' });
  }
  if (result.vietdub) badges.push({ text: 'VIE', variant: 'language' });
  if (result.vietsub) badges.push({ text: 'SUB', variant: 'success' });
  if (result.hdr) badges.push({ text: 'HDR', variant: 'hdr', noDot: true });
  if (result.dolbyVision) badges.push({ text: 'DV', variant: 'dv', noDot: true });
  if (result.quality) badges.push({ text: result.quality, variant: 'quality' });
  return badges;
}

/** 1024-based size with up to 2 decimals, trailing zeros trimmed ("1.5 GB"). */
export function formatResultSize(bytes: number): string {
  if (!bytes) return '0 B';
  const k = 1024;
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + units[i];
}

/**
 * Source tabs to show for a response: All plus every source with results
 * (`response.sources` = per-source counts).
 */
export function availableSourceTabs(response: any): Array<[SourceFilter, string]> {
  const counts = response.sources ?? {};
  const tabs: Array<[SourceFilter, string]> = [
    ['all', 'All'],
    ['fshare', 'Fshare'],
    ['torrent', 'Torrent'],
    ['debrid', 'TorBox'],
  ];
  return tabs.filter(([id]) => id === 'best' || id === 'all' || (counts[id] ?? 0) > 0);
}
