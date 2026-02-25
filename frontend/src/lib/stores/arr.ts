/**
 * Arr Suite Store
 * 
 * Manages library data from Sonarr/Radarr proxied through Flasharr's /api/arr endpoints.
 * This powers the "single pane of glass" experience — no need to visit Sonarr/Radarr/Seerr.
 */

import { writable, derived, get } from 'svelte/store';
import { toasts } from './toasts';

// ============================================================================
// Types — field names match backend serde serialization (camelCase)
// ============================================================================

export interface LibraryOverview {
  sonarr_connected: boolean;
  radarr_connected: boolean;
  series_count: number;
  movie_count: number;
  total_episodes: number;
  episodes_with_files: number;
  episodes_missing: number;
  movies_with_files: number;
  movies_missing: number;
  total_size_on_disk: number;
}

export interface SonarrStatistics {
  seasonCount?: number;
  episodeFileCount?: number;
  episodeCount?: number;
  totalEpisodeCount?: number;
  sizeOnDisk?: number;
  percentOfEpisodes?: number;
}

export interface SonarrSeries {
  id: number;
  title: string;
  tvdbId?: number;
  tmdbId?: number;
  path?: string;
  year?: number;
  overview?: string;
  status?: string;
  monitored?: boolean;
  images?: MediaImage[];
  qualityProfileId?: number;
  statistics?: SonarrStatistics;
}

export interface RadarrCollection {
  title: string;
  tmdbId: number;
}

export interface RadarrMovie {
  id: number;
  title: string;
  tmdbId: number;
  path?: string;
  year?: number;
  overview?: string;
  status?: string;
  monitored?: boolean;
  hasFile?: boolean;
  sizeOnDisk?: number;
  images?: MediaImage[];
  qualityProfileId?: number;
  runtime?: number;
  /** Radarr collection grouping (e.g. Marvel Cinematic Universe) */
  collection?: RadarrCollection;
}

export interface MediaImage {
  coverType: string;
  url?: string;
  remoteUrl?: string;
}

export interface CalendarEntry {
  id: number;
  seriesId: number;
  title?: string;
  seasonNumber: number;
  episodeNumber: number;
  airDateUtc?: string;
  hasFile: boolean;
  series?: {
    id: number;
    title: string;
    images?: MediaImage[];
  };
}

export interface DiskSpace {
  path: string;
  label?: string;
  freeSpace: number;
  totalSpace: number;
}

export interface ArrServiceStatus {
  connected: boolean;
  version?: string;
  start_time?: string;
  health_issues: Array<{
    source?: string;
    check_type?: string;
    message?: string;
  }>;
}

// ============================================================================
// Stores
// ============================================================================

export const libraryOverview = writable<LibraryOverview | null>(null);
export const calendarEntries = writable<CalendarEntry[]>([]);
export const diskSpaces = writable<DiskSpace[]>([]);
export const arrStatus = writable<{ sonarr?: ArrServiceStatus; radarr?: ArrServiceStatus } | null>(null);
export const arrLoading = writable(false);
export const arrError = writable<string | null>(null);

// ============================================================================
// API Functions
// ============================================================================

const API_BASE = '/api/arr';
const ARR_TIMEOUT_MS = 5000; // 5 second timeout to prevent UI blocking

/** Fetch with timeout — aborts if Arr APIs don't respond quickly */
function fetchWithTimeout(url: string, timeoutMs: number = ARR_TIMEOUT_MS): Promise<Response> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  return fetch(url, { signal: controller.signal }).finally(() => clearTimeout(timer));
}

export async function fetchLibraryOverview(): Promise<LibraryOverview | null> {
  try {
    arrLoading.set(true);
    const res = await fetchWithTimeout(`${API_BASE}/library`);
    if (!res.ok) {
      if (res.status === 503) {
        arrError.set('Arr services not configured');
        return null;
      }
      throw new Error(`HTTP ${res.status}`);
    }
    const data: LibraryOverview = await res.json();
    libraryOverview.set(data);
    arrError.set(null);
    return data;
  } catch (e: any) {
    arrError.set(e.message);
    toasts.error(`Library Error: ${e.message}`);
    return null;
  } finally {
    arrLoading.set(false);
  }
}

export async function fetchCalendar(days: number = 14): Promise<CalendarEntry[]> {
  try {
    const start = new Date().toISOString().split('T')[0];
    const end = new Date(Date.now() + days * 86400000).toISOString().split('T')[0];
    const res = await fetchWithTimeout(`${API_BASE}/calendar?start=${start}&end=${end}`);
    if (!res.ok) return [];
    const data: CalendarEntry[] = await res.json();
    calendarEntries.set(data);
    return data;
  } catch (e: any) {
    toasts.error(`Calendar Error: ${e.message}`);
    return [];
  }
}

export async function fetchDiskSpace(): Promise<DiskSpace[]> {
  try {
    const res = await fetchWithTimeout(`${API_BASE}/storage`);
    if (!res.ok) return [];
    const data: DiskSpace[] = await res.json();
    diskSpaces.set(data);
    return data;
  } catch (e: any) {
    toasts.error(`Storage Error: ${e.message}`);
    return [];
  }
}

export async function fetchArrStatus(): Promise<void> {
  try {
    const res = await fetchWithTimeout(`${API_BASE}/health`);
    if (!res.ok) return;
    const data = await res.json();
    arrStatus.set(data);
  } catch {
    // silently fail
  }
}

export async function fetchMissing(page: number = 1, pageSize: number = 10): Promise<any> {
  try {
    const res = await fetchWithTimeout(`${API_BASE}/missing?page=${page}&page_size=${pageSize}`);
    if (!res.ok) return null;
    return await res.json();
  } catch (e: any) {
    toasts.error(`Missing Items Error: ${e.message}`);
    return null;
  }
}

export async function fetchAllSeries(): Promise<SonarrSeries[]> {
  try {
    const res = await fetchWithTimeout(`${API_BASE}/series`);
    if (!res.ok) return [];
    return await res.json();
  } catch (e: any) {
    toasts.error(`Series Fetch Error: ${e.message}`);
    return [];
  }
}

export async function fetchAllMovies(): Promise<RadarrMovie[]> {
  try {
    const res = await fetchWithTimeout(`${API_BASE}/movies`);
    if (!res.ok) return [];
    return await res.json();
  } catch (e: any) {
    toasts.error(`Movies Fetch Error: ${e.message}`);
    return [];
  }
}

export async function fetchHistory(pageSize: number = 20): Promise<any> {
  try {
    const res = await fetchWithTimeout(`${API_BASE}/history?page_size=${pageSize}`);
    if (!res.ok) return null;
    return await res.json();
  } catch (e: any) {
    toasts.error(`History Fetch Error: ${e.message}`);
    return null;
  }
}

// ============================================================================
// Helpers
// ============================================================================

export function formatDiskSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return (bytes / Math.pow(k, i)).toFixed(1) + ' ' + units[i];
}

export function getSeriesPoster(series: SonarrSeries): string | null {
  const poster = series.images?.find(i => i.coverType === 'poster');
  return poster?.remoteUrl || poster?.url || null;
}

export function getSeriesBanner(series: SonarrSeries): string | null {
  const banner = series.images?.find(i => i.coverType === 'banner' || i.coverType === 'fanart');
  return banner?.remoteUrl || banner?.url || null;
}

export function getMoviePoster(movie: RadarrMovie): string | null {
  const poster = movie.images?.find(i => i.coverType === 'poster');
  return poster?.remoteUrl || poster?.url || null;
}

// ============================================================================
// Cross-reference helpers — bridge TMDB ↔ Sonarr/Radarr
// ============================================================================

const TMDB_IMAGE_BASE = 'https://image.tmdb.org/t/p';

/** Get a TMDB poster URL from a tmdbId (constructs the path via API lookup) */
export function tmdbPosterUrl(posterPath: string | null, size: 'w185' | 'w342' | 'w500' = 'w342'): string | null {
  if (!posterPath) return null;
  return `${TMDB_IMAGE_BASE}/${size}${posterPath}`;
}

/** Find a Sonarr series by TMDB ID from a pre-fetched list */
export function findSeriesInList(allSeries: SonarrSeries[], tmdbId: number): SonarrSeries | null {
  return allSeries.find(s => s.tmdbId === tmdbId) || null;
}

/** Find a Radarr movie by TMDB ID from a pre-fetched list */
export function findMovieInList(allMovies: RadarrMovie[], tmdbId: number): RadarrMovie | null {
  return allMovies.find(m => m.tmdbId === tmdbId) || null;
}

/** Fetch Sonarr episodes for a given Sonarr series ID */
export async function fetchEpisodesBySonarrId(seriesId: number): Promise<SonarrEpisode[]> {
  try {
    const res = await fetchWithTimeout(`${API_BASE}/episodes?series_id=${seriesId}`);
    if (!res.ok) return [];
    return await res.json();
  } catch {
    return [];
  }
}

export interface SonarrEpisode {
  id: number;
  seasonNumber: number;
  episodeNumber: number;
  title?: string;
  airDateUtc?: string;
  hasFile: boolean;
  overview?: string;
}
