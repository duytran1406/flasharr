/**
 * Smart Grab planning: turns Smart Search season groups into downloadable
 * episode sets (quick-grab scenarios, quality / release-group / filename
 * pattern sets with coverage + uncut detection) and builds the per-episode
 * download requests.
 */
import type { AddDownloadRequest } from '$lib/stores/downloads.svelte';
import { buildTvTmdbMetadata } from '$lib/utils/mediaIdentity';
import {
  acquisitionFields,
  downloadUrl,
  normalizeSearchResult,
  type RawSearchResult,
} from '$lib/components/smart-search/resultUtils';

// ============================================================================
// Types
// ============================================================================

/** One chosen file for one episode. */
export interface SmartGrabFile {
  seasonNum: number;
  epNum: number;
  file: RawSearchResult;
  /** Filled from the quick-grab pick because the set had no file for this episode. */
  isFallback?: boolean;
}

/** A grouping of episode files (by quality, release group or filename pattern). */
export interface SmartGrabSet {
  id: string;
  name: string;
  /** quality sets only */
  quality?: string;
  episodeCount: number;
  totalEpisodes: number;
  /** 0–100 */
  coverage: number;
  avgSize: number;
  missingEpisodes: number[];
  files: SmartGrabFile[];
  /** pattern sets only */
  maxEpisodeNumber?: number;
  isUncut?: boolean;
  uncutReason?: string;
}

export type QuickGrabScenarioKind =
  | 'balanced'
  | 'same_source'
  | 'same_group'
  | 'same_quality'
  | 'best_quality'
  | 'highest_coverage';

export interface ScenarioFile extends SmartGrabFile {
  source: string;
  quality: QualityTier;
  group: string;
  /** inspection score / seeders / score */
  health: number;
}

export interface QuickGrabScenario {
  /** `<kind>-<index>` */
  id: string;
  kind: QuickGrabScenarioKind;
  name: string;
  files: ScenarioFile[];
  /** 0–100 against the available episode count */
  coverage: number;
  sourceCount: number;
  acquisitionCount: number;
  warnings: string[];
}

export type QualityTier = '4K' | '1080p' | '720p' | 'SD';

export interface SmartGrabOptions {
  /** Only plan this season. */
  targetSeason?: number | null;
  /** `SxxEyy` keys already downloaded. */
  existingDownloads?: Set<string>;
  /** Skip episodes in existingDownloads. */
  skipDownloaded?: boolean;
}

export interface SmartGrabPlan {
  quickGrabScenarios: QuickGrabScenario[];
  qualitySets: SmartGrabSet[];
  releaseGroupSets: SmartGrabSet[];
  patternSets: SmartGrabSet[];
  /** First file of every available episode. */
  quickGrabInfo: { count: number; avgSize: number; files: SmartGrabFile[] };
  totalAvailableEpisodes: number;
  allExpectedEpisodes: number[];
  /** TMDB episode count of the first regular season (0 = unknown) */
  tmdbEpisodeCount: number;
  airedEpisodes: number;
  downloadedCount: number;
}

/** Media fields of the Smart Grab target used to build download requests. */
export interface SmartGrabTarget {
  tmdbId?: string;
  tvdbId?: string | number | null;
  title: string;
  year?: string | number | null;
}

// ============================================================================
// Filename heuristics
// ============================================================================

/** `S01E02` */
export function episodeKey(season: number, episode: number): string {
  return `S${String(season).padStart(2, '0')}E${String(episode).padStart(2, '0')}`;
}

export function detectQuality(filename: string | undefined | null): QualityTier {
  if (!filename) return 'SD';
  if (/4k|2160p/i.test(filename)) return '4K';
  if (/1080p|fullhd/i.test(filename)) return '1080p';
  if (/720p/i.test(filename)) return '720p';
  return 'SD';
}

/** Release group from `-GROUP.ext`, `[GROUP]` or `.GROUP.`; "Unknown" otherwise. */
export function detectReleaseGroup(filename: string | undefined | null): string {
  if (!filename) return 'Unknown';
  const patterns = [/-([A-Za-z0-9]+)(?:\.[a-z]{2,4})?$/, /\[([A-Za-z0-9]+)\]/, /\.([A-Za-z0-9]{2,10})\./];
  for (const pattern of patterns) {
    const match = filename.match(pattern);
    if (match && match[1] && match[1].length > 1) return match[1];
  }
  return 'Unknown';
}

/** Filename with episode markers replaced by {EP}/{SE}, used to group a release series. */
export function filenamePattern(filename: string | undefined | null): string {
  if (!filename) return 'unknown';
  return filename
    .replace(/\.(mkv|mp4|avi|mov|webm)$/i, '')
    .replace(/[._-]?[Ee]p?\.?\d{1,4}[._-]?/g, '{EP}')
    .replace(/[._-]?Tap\.?\d{1,4}[._-]?/gi, '{EP}')
    .replace(/S\d{1,2}E\d{1,4}/gi, '{SE}')
    .replace(/\d{1,2}x\d{1,4}/gi, '{SE}')
    .replace(/[._-](\d{1,3})[._-]/g, '.{EP}.')
    .replace(/[._-]+/g, '.')
    .toLowerCase()
    .substring(0, 80);
}

/** CSS gradient for a coverage bar. */
export function coverageGradient(coverage: number): string {
  if (coverage >= 100) return 'linear-gradient(90deg, #10b981, #059669)';
  if (coverage >= 80) return 'linear-gradient(90deg, #22c55e, #16a34a)';
  if (coverage >= 50) return 'linear-gradient(90deg, #f59e0b, #d97706)';
  return 'linear-gradient(90deg, #ef4444, #dc2626)';
}

// ============================================================================
// Quick-grab scenarios
// ============================================================================

const SOURCE_RANK: Record<string, number> = { fshare: 0, debrid: 1, torrent: 2 };
const QUALITY_RANK: Record<string, number> = { '4K': 4, '1080p': 3, '720p': 2, SD: 1 };

const scenarioSource = (file: RawSearchResult): string =>
  file.result_source ?? file.resultSource ?? (file.provider === 'torbox' ? 'debrid' : file.provider) ?? 'torrent';

const scenarioQuality = (name: string): QualityTier =>
  /4k|2160p/i.test(name) ? '4K' : /1080p|fullhd/i.test(name) ? '1080p' : /720p/i.test(name) ? '720p' : 'SD';

const scenarioGroup = (name: string): string =>
  name.match(/-([A-Za-z0-9]+)(?:\.[a-z]{2,4})?$/)?.[1] ?? name.match(/\[([A-Za-z0-9]+)\]/)?.[1] ?? 'Unknown';

const scenarioHealth = (file: RawSearchResult): number =>
  Number(file.inspection_score ?? file.inspectionScore ?? file.seeders ?? file.score ?? 0);

const scenarioSignature = (files: ScenarioFile[]): string =>
  files
    .map((f) => `${f.seasonNum}:${f.epNum}:${f.file.name}`)
    .sort()
    .join('|');

function toScenarioFile(seasonNum: number, epNum: number, file: RawSearchResult): ScenarioFile {
  return {
    seasonNum,
    epNum,
    file,
    source: scenarioSource(file),
    quality: scenarioQuality(file.name ?? ''),
    group: scenarioGroup(file.name ?? ''),
    health: scenarioHealth(file),
  };
}

/**
 * Build distinct one-file-per-episode scenarios: Balanced, Same Source · X,
 * Same Group · X, Same Quality · X, Best Quality, Highest Coverage.
 * Candidates are episode files plus per-episode files of season packages with
 * unambiguous file indices. `downloaded` holds `"<season>:<episode>"` keys to skip.
 * Scenarios with identical file sets are emitted once.
 */
export function buildQuickGrabScenarios(
  seasons: any[],
  availableEpisodes: number,
  downloaded: Set<string>,
): QuickGrabScenario[] {
  const candidates = new Map<string, ScenarioFile[]>();

  for (const season of seasons) {
    for (const group of season.episodes_grouped ?? []) {
      const key = `${season.season}:${group.episode_number}`;
      if (!group.episode_number || downloaded.has(key)) continue;
      candidates.set(
        key,
        (group.files ?? []).map((file: RawSearchResult) => toScenarioFile(season.season, group.episode_number, file)),
      );
    }

    for (const pkg of season.packages ?? []) {
      const covered: number[] = pkg.coveredEpisodes ?? [];
      const indices = covered.map((episode) => pkg.fileIndices?.[episode]);
      const ambiguous =
        covered.length > 1 && (indices.some((index) => index === undefined) || new Set(indices).size !== indices.length);
      if (ambiguous) continue;

      for (const episode of covered) {
        const key = `${season.season}:${episode}`;
        const fileIdx = pkg.fileIndices?.[episode];
        if (!episode || fileIdx === undefined || downloaded.has(key)) continue;
        const file: RawSearchResult = {
          ...(pkg.episodeFiles?.[episode] ?? pkg.result),
          file_idx: fileIdx,
          fileIdx,
          packageSelection: true,
        };
        candidates.set(key, [...(candidates.get(key) ?? []), toScenarioFile(season.season, episode, file)]);
      }
    }
  }

  const episodeKeys = [...candidates.keys()];
  const pickPerEpisode = (
    accept: (f: ScenarioFile) => boolean,
    compare: (a: ScenarioFile, b: ScenarioFile) => number,
  ): ScenarioFile[] =>
    episodeKeys.flatMap((key) => {
      const best = [...(candidates.get(key) ?? [])].filter(accept).sort(compare)[0];
      return best ? [best] : [];
    });

  const balanced = (a: ScenarioFile, b: ScenarioFile) =>
    (SOURCE_RANK[a.source] ?? 9) - (SOURCE_RANK[b.source] ?? 9) ||
    (QUALITY_RANK[b.quality] ?? 0) - (QUALITY_RANK[a.quality] ?? 0) ||
    b.health - a.health ||
    (a.file.name ?? '').localeCompare(b.file.name ?? '');
  const healthiest = (a: ScenarioFile, b: ScenarioFile) => b.health - a.health || balanced(a, b);

  const all = [...candidates.values()].flat();
  const sources = [...new Set(all.map((f) => f.source))];
  const groups = [...new Set(all.map((f) => f.group).filter((g) => g !== 'Unknown'))];
  const qualities = [...new Set(all.map((f) => f.quality))];

  const drafts: Array<[QuickGrabScenarioKind, string, ScenarioFile[]]> = [
    ['balanced', 'Balanced', pickPerEpisode(() => true, balanced)],
    ...sources.map(
      (source): [QuickGrabScenarioKind, string, ScenarioFile[]] => [
        'same_source',
        `Same Source · ${source}`,
        pickPerEpisode((f) => f.source === source, healthiest),
      ],
    ),
    ...groups.map(
      (group): [QuickGrabScenarioKind, string, ScenarioFile[]] => [
        'same_group',
        `Same Group · ${group}`,
        pickPerEpisode((f) => f.group === group, balanced),
      ],
    ),
    ...qualities.map(
      (quality): [QuickGrabScenarioKind, string, ScenarioFile[]] => [
        'same_quality',
        `Same Quality · ${quality}`,
        pickPerEpisode((f) => f.quality === quality, balanced),
      ],
    ),
    [
      'best_quality',
      'Best Quality',
      pickPerEpisode(() => true, (a, b) => (QUALITY_RANK[b.quality] ?? 0) - (QUALITY_RANK[a.quality] ?? 0) || balanced(a, b)),
    ],
    ['highest_coverage', 'Highest Coverage', pickPerEpisode(() => true, healthiest)],
  ];

  const seen = new Set<string>();
  return drafts.flatMap(([kind, name, files], index) => {
    const signature = `${kind}:${scenarioSignature(files)}`;
    if (!files.length || seen.has(signature)) return [];
    seen.add(signature);
    const coverage = availableEpisodes ? (files.length / availableEpisodes) * 100 : 0;
    return [
      {
        id: `${kind}-${index}`,
        kind,
        name,
        files,
        coverage,
        sourceCount: new Set(files.map((f) => scenarioSource(f.file))).size,
        acquisitionCount: new Set(files.map((f) => f.file.info_hash ?? f.file.infoHash ?? f.file.url ?? f.file.name)).size,
        warnings: coverage < 100 ? [`${availableEpisodes - files.length} episodes missing`] : [],
      },
    ];
  });
}

// ============================================================================
// Plan
// ============================================================================

function addToSet(
  sets: Map<string, SmartGrabSet>,
  episodesBySet: Map<string, Set<number>>,
  setKey: string,
  seasonNum: number,
  epNum: number,
  file: RawSearchResult,
  id: string,
  name: string,
) {
  if (!sets.has(setKey)) {
    sets.set(setKey, {
      id,
      name,
      quality: id.startsWith('quality-') ? name : undefined,
      episodeCount: 0,
      totalEpisodes: 0,
      coverage: 0,
      avgSize: 0,
      missingEpisodes: [],
      files: [],
    });
    episodesBySet.set(setKey, new Set());
  }
  const set = sets.get(setKey)!;
  const episodes = episodesBySet.get(setKey)!;
  if (!episodes.has(epNum)) {
    episodes.add(epNum);
    set.files.push({ seasonNum, epNum, file });
  }
}

/**
 * Compute coverage, average size and missing episodes for each set, fill
 * gaps with the quick-grab pick (`isFallback`), flag pattern sets whose max
 * episode exceeds the official count as uncut, then sort: non-uncut first,
 * by coverage desc.
 */
function finalizeSets(
  sets: Map<string, SmartGrabSet>,
  episodesBySet: Map<string, Set<number>>,
  expectedEpisodes: number[],
  quickGrabFiles: SmartGrabFile[],
  totalEpisodes: number,
  uncut?: { officialEpisodeCount: number; patternMaxEpisode: Map<string, number> },
): SmartGrabSet[] {
  const result: SmartGrabSet[] = [];
  for (const [key, set] of sets) {
    const episodes = episodesBySet.get(key) || new Set<number>();
    set.episodeCount = episodes.size;
    set.totalEpisodes = totalEpisodes;
    set.coverage = totalEpisodes > 0 ? (episodes.size / totalEpisodes) * 100 : 0;
    set.avgSize = set.files.length > 0 ? set.files.reduce((sum, f) => sum + (f.file.size || 0), 0) / set.files.length : 0;
    set.missingEpisodes = expectedEpisodes.filter((episode) => !episodes.has(episode));

    for (const episode of set.missingEpisodes) {
      const fallback = quickGrabFiles.find((f) => f.epNum === episode);
      if (fallback) set.files.push({ ...fallback, isFallback: true });
    }
    set.files.sort((a, b) => a.epNum - b.epNum);

    if (uncut?.officialEpisodeCount) {
      const maxEpisode = uncut.patternMaxEpisode.get(key) || 0;
      set.maxEpisodeNumber = maxEpisode;
      if (maxEpisode > uncut.officialEpisodeCount) {
        set.isUncut = true;
        set.uncutReason = `${maxEpisode} eps detected (official: ${uncut.officialEpisodeCount})`;
      }
    }
    result.push(set);
  }
  return result.sort((a, b) => {
    if (a.isUncut && !b.isUncut) return 1;
    if (!a.isUncut && b.isUncut) return -1;
    return b.coverage - a.coverage;
  });
}

/**
 * Build the Smart Grab plan from Smart Search `seasons` (SeasonGroup[]).
 * Specials (season 0) and episodes beyond the TMDB episode count are ignored.
 */
export function buildSmartGrabPlan(seasons: any[], options: SmartGrabOptions): SmartGrabPlan {
  const scoped = seasons.filter(
    (season) => options.targetSeason == null || Number(season.season) === options.targetSeason,
  );
  const existing = options.existingDownloads ?? new Set<string>();

  const qualitySets = new Map<string, SmartGrabSet>();
  const groupSets = new Map<string, SmartGrabSet>();
  const patternSets = new Map<string, SmartGrabSet>();
  const quickGrabFiles: SmartGrabFile[] = [];
  const expected: number[] = [];
  const episodesBySet = {
    quality: new Map<string, Set<number>>(),
    releaseGroup: new Map<string, Set<number>>(),
    pattern: new Map<string, Set<number>>(),
  };
  const patternMaxEpisode = new Map<string, number>();

  let quickGrabSize = 0;
  let downloadedCount = 0;
  let tmdbEpisodeCount = 0;

  for (const season of scoped) {
    if (season.season !== 0 && season.episode_count) {
      tmdbEpisodeCount = season.episode_count;
      break;
    }
  }

  const airedEpisodes = scoped
    .filter((season) => season.season !== 0)
    .reduce((sum, season) => sum + (season.aired_episode_count || 0), 0);

  for (const season of scoped) {
    if (season.season === 0) continue;

    for (const pkg of season.packages ?? []) {
      for (const episode of pkg.coveredEpisodes ?? []) {
        if (!episode || (tmdbEpisodeCount > 0 && episode > tmdbEpisodeCount) || expected.includes(episode)) continue;
        expected.push(episode);
      }
    }

    for (const group of season.episodes_grouped || []) {
      if (!group.files || group.files.length === 0) continue;
      const episode: number = group.episode_number;
      if (episode === 0 || (tmdbEpisodeCount > 0 && episode > tmdbEpisodeCount)) continue;
      if (options.skipDownloaded && existing.has(episodeKey(season.season, episode))) {
        downloadedCount++;
        continue;
      }

      expected.push(episode);
      const first = group.files[0];
      quickGrabFiles.push({ seasonNum: season.season, epNum: episode, file: first });
      quickGrabSize += first.size || 0;

      for (const file of group.files as RawSearchResult[]) {
        const quality = detectQuality(file.name);
        const releaseGroup = detectReleaseGroup(file.name);
        const pattern = filenamePattern(file.name);
        const name = file.name ?? '';
        addToSet(qualitySets, episodesBySet.quality, quality, season.season, episode, file, `quality-${quality}`, quality);
        addToSet(groupSets, episodesBySet.releaseGroup, releaseGroup, season.season, episode, file, `rg-${releaseGroup}`, releaseGroup);
        addToSet(
          patternSets,
          episodesBySet.pattern,
          pattern,
          season.season,
          episode,
          file,
          `pattern-${pattern}`,
          name.substring(0, 50) + (name.length > 50 ? '...' : ''),
        );
        patternMaxEpisode.set(pattern, Math.max(patternMaxEpisode.get(pattern) || 0, episode));
      }
    }
  }

  const expectedEpisodes = [...new Set(expected)].sort((a, b) => a - b);
  const totalEpisodes = airedEpisodes > 0 ? airedEpisodes : expectedEpisodes.length;
  const availableEpisodes = Math.max(0, totalEpisodes - downloadedCount);
  const downloadedKeys = new Set(
    [...existing].map((key) => {
      const match = key.match(/^S(\d+)E(\d+)$/);
      return match ? `${Number(match[1])}:${Number(match[2])}` : key;
    }),
  );

  return {
    quickGrabScenarios: buildQuickGrabScenarios(scoped, availableEpisodes, downloadedKeys),
    qualitySets: finalizeSets(qualitySets, episodesBySet.quality, expectedEpisodes, quickGrabFiles, availableEpisodes),
    releaseGroupSets: finalizeSets(groupSets, episodesBySet.releaseGroup, expectedEpisodes, quickGrabFiles, availableEpisodes),
    patternSets: finalizeSets(patternSets, episodesBySet.pattern, expectedEpisodes, quickGrabFiles, availableEpisodes, {
      officialEpisodeCount: tmdbEpisodeCount,
      patternMaxEpisode,
    }),
    quickGrabInfo: {
      count: quickGrabFiles.length,
      avgSize: quickGrabFiles.length > 0 ? quickGrabSize / quickGrabFiles.length : 0,
      files: quickGrabFiles,
    },
    totalAvailableEpisodes: availableEpisodes,
    allExpectedEpisodes: expectedEpisodes,
    tmdbEpisodeCount,
    airedEpisodes,
    downloadedCount,
  };
}

// ============================================================================
// Download requests
// ============================================================================

/**
 * POST /api/downloads body for one planned episode (category `tv`).
 * Package selections drop `source_filename` so the backend picks by file index.
 */
export function buildEpisodeDownloadRequest(
  entry: SmartGrabFile,
  target: SmartGrabTarget,
  priority: number,
  batchId: string,
  batchName: string,
): AddDownloadRequest {
  const tmdbId = parseInt(target.tmdbId || '');
  const year = target.year ?? null;
  const tmdb = buildTvTmdbMetadata({
    tmdbId: Number.isFinite(tmdbId) ? tmdbId : null,
    tvdbId: target.tvdbId,
    title: target.title,
    year: year != null ? (typeof year === 'string' ? parseInt(year) : year) : null,
    season: entry.seasonNum,
    episode: entry.epNum,
  });
  const result = normalizeSearchResult(entry.file, 'episode');
  const extra = acquisitionFields(result);
  if (entry.file.packageSelection) delete extra.source_filename;

  return {
    url: downloadUrl(result),
    filename: entry.file.name,
    category: 'tv',
    priority: priority.toString(),
    tmdb,
    batch_id: batchId,
    batch_name: batchName,
    size: entry.file.size,
    ...extra,
  };
}
