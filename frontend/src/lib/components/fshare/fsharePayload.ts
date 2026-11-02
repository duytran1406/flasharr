/**
 * Payload helpers for manual Fshare links (Fshare detail modal / "use this
 * link" actions in Smart Search). Pure functions, no runtime imports.
 */
import type { AddDownloadRequest, TmdbMetadata } from '$lib/stores/downloads.svelte';

/**
 * Validate and canonicalise an Fshare file page URL to
 * `https://www.fshare.vn/file/<code>`. Throws an Error with a user-facing
 * message for anything else (non-HTTPS, other host, folder links, ...).
 */
export function normalizeFshareFileUrl(input: string): string {
  const url = new URL(input);
  if (url.protocol !== 'https:' || !['fshare.vn', 'www.fshare.vn'].includes(url.hostname)) {
    throw new Error('Use an HTTPS Fshare file page URL');
  }
  const match = url.pathname.match(/^\/file\/([^/]+)\/?$/i);
  if (!match) throw new Error('Use an HTTPS Fshare file page URL');
  return `https://www.fshare.vn/file/${match[1]}`;
}

/**
 * Build the POST /api/downloads body for a manual Fshare link.
 * `filename` falls back to the release name; throws like normalizeFshareFileUrl.
 */
export function buildFshareDownloadPayload(
  release: { name?: string; size?: number } | null | undefined,
  url: string,
  filename: string,
  category: string,
  tmdb?: TmdbMetadata | null,
): AddDownloadRequest {
  return {
    url: normalizeFshareFileUrl(url),
    filename: filename.trim() || release?.name || undefined,
    category,
    source: 'fshare',
    provider: 'fshare',
    size: release?.size,
    source_filename: release?.name,
    ...(tmdb ? { tmdb } : {}),
  };
}

/** Copy of `tmdb` metadata targeting a specific season/episode. */
export function withEpisodeTarget<T extends object>(
  tmdb: T,
  target: { season?: number | null; episode?: number | null },
): T & { season?: number | null; episode?: number | null } {
  return { ...tmdb, season: target.season, episode: target.episode };
}
