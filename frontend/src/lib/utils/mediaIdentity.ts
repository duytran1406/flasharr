/**
 * Canonical media identity contract.
 *
 * - Movies are identified by their TMDB id (route `/movie/:tmdbId`).
 * - Series are identified by their TVDB id (route `/tv/:tvdbId`); the TMDB id
 *   is only carried as a legacy "bridge" for TMDB-backed UI data.
 *
 * Inputs are loosely typed on purpose: TMDB results, Sonarr/Radarr rows and
 * download tasks all flow through here with camelCase or snake_case keys.
 */

export type MediaIdentity =
  | { type: 'movie'; tmdbId: number }
  | { type: 'tv'; tvdbId: number; tmdbBridge: number | null };

/** Download request `tmdb` metadata for a TV episode/season (backend TmdbMetadata). */
export interface TvTmdbMetadata {
  tmdb_id: number | null;
  tvdb_id: number | null;
  media_type: 'tv';
  title: string | null;
  year: number | string | null;
  season: number | null;
  episode: number | null;
}

/** A positive finite number, else null. */
function positiveId(value: unknown): number | null {
  const n = Number(value);
  return Number.isFinite(n) && n > 0 ? n : null;
}

/**
 * Resolve the canonical identity of a media-like object.
 * TV items without a TVDB id resolve to null (no canonical identity).
 */
export function resolveMediaIdentity(item: any): MediaIdentity | null {
  const kind = String(item?.mediaType ?? item?.media_type ?? item?.type ?? '').toLowerCase();
  const isTv = kind === 'tv' || kind === 'series' || kind === 'show';
  const tvdbId = positiveId(item?.tvdbId ?? item?.tvdb_id);
  const tmdbId = positiveId(item?.tmdbId ?? item?.tmdb_id ?? item?.id);

  if (isTv) return tvdbId ? { type: 'tv', tvdbId, tmdbBridge: tmdbId } : null;
  return tmdbId ? { type: 'movie', tmdbId } : null;
}

/** Detail-page href for an identity (`/tv/:tvdbId` or `/movie/:tmdbId`), or null. */
export function mediaIdentityHref(identity: MediaIdentity | null | undefined): string | null {
  if (!identity) return null;
  return identity.type === 'tv' ? `/tv/${identity.tvdbId}` : `/movie/${identity.tmdbId}`;
}

/** Build the `tmdb` metadata block for a TV download request. */
export function buildTvTmdbMetadata(input: {
  tmdbId?: unknown;
  tvdbId?: unknown;
  title?: string | null;
  year?: number | string | null;
  season?: number | null;
  episode?: number | null;
}): TvTmdbMetadata {
  return {
    tmdb_id: positiveId(input.tmdbId),
    tvdb_id: positiveId(input.tvdbId),
    media_type: 'tv',
    title: input.title ?? null,
    year: input.year ?? null,
    season: input.season ?? null,
    episode: input.episode ?? null,
  };
}
