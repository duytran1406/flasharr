// TMDB API service - proxies through backend
const API_BASE = '/api/tmdb';

export interface TMDBGenre {
  id: number;
  name: string;
}

export interface TMDBMovie extends TMDBMovieDetails {
  media_type?: 'movie';
}

export interface TMDBTVShow extends TMDBTVShowDetails {
  media_type?: 'tv';
}

export interface TMDBKeyword {
  id: number;
  name: string;
}

export interface TMDBMovieDetails {
  id: number;
  title: string;
  poster_path: string | null;
  backdrop_path: string | null;
  vote_average: number;
  vote_count: number;
  release_date: string;
  overview: string;
  tagline: string | null;
  runtime: number;
  genres: TMDBGenre[];
  status: string;
  revenue: number;
  budget: number;
  original_language: string;
  belongs_to_collection?: {
    id: number;
    name: string;
    poster_path: string | null;
    backdrop_path: string | null;
  } | null;
  keywords?: {
    keywords: TMDBKeyword[];
  };
  release_dates?: {
    results: {
      iso_3166_1: string;
      release_dates: {
        certification: string;
        iso_639_1: string;
        release_date: string;
        type: number;
        note: string;
      }[];
    }[];
  };
  external_ids?: {
    imdb_id: string | null;
    tvdb_id: number | null;
    facebook_id: string | null;
    instagram_id: string | null;
    twitter_id: string | null;
  };
}

export interface TMDBTVShowDetails {
  id: number;
  name: string;
  poster_path: string | null;
  backdrop_path: string | null;
  vote_average: number;
  vote_count: number;
  first_air_date: string;
  overview: string;
  tagline: string | null;
  genres: TMDBGenre[];
  status: string;
  original_language: string;
  number_of_seasons: number;
  number_of_episodes: number;
  episode_run_time: number[];
  seasons: {
    id: number;
    name: string;
    overview: string;
    poster_path: string | null;
    season_number: number;
    episode_count: number;
  }[];
  keywords?: {
    results: TMDBKeyword[];
  };
  content_ratings?: {
    results: {
      iso_3166_1: string;
      rating: string;
    }[];
  };
  external_ids?: {
    imdb_id: string | null;
    tvdb_id: number | null;
    facebook_id: string | null;
    instagram_id: string | null;
    twitter_id: string | null;
  };
}

export interface TMDBEpisode {
  id: number;
  name: string;
  overview: string;
  still_path: string | null;
  air_date: string;
  episode_number: number;
  season_number: number;
  vote_average: number;
}

export interface TMDBSeasonDetails {
  id: number;
  name: string;
  overview: string;
  poster_path: string | null;
  season_number: number;
  episodes: TMDBEpisode[];
}

export interface TMDBCollection {
  id: number;
  name: string;
  overview: string;
  poster_path: string | null;
  backdrop_path: string | null;
  parts: TMDBMovie[];
}

export interface TMDBResponse<T> {
  results: T[];
  page: number;
  total_pages: number;
  total_results: number;
}

/**
 * Get trending movies for the week
 */
export async function getTrendingMovies(): Promise<TMDBMovie[]> {
  try {
    const response = await fetch(`${API_BASE}/trending/movie/week`);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    const data: TMDBResponse<TMDBMovie> = await response.json();
    return data.results.slice(0, 8); // Return top 8
  } catch (error) {
    console.error('[TMDB] Failed to fetch trending movies:', error);
    return [];
  }
}

/**
 * Get trending TV shows for the week
 */
export async function getTrendingTV(): Promise<TMDBTVShow[]> {
  try {
    const response = await fetch(`${API_BASE}/trending/tv/week`);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    const data: TMDBResponse<TMDBTVShow> = await response.json();
    return data.results.slice(0, 6); // Return top 6
  } catch (error) {
    console.error('[TMDB] Failed to fetch trending TV:', error);
    return [];
  }
}

/**
 * Alias for getTrendingTV - for semantic clarity
 */
export const getPopularTVShows = getTrendingTV;


/**
 * Get movie details
 */
export async function getMovieDetails(id: string | number): Promise<TMDBMovieDetails | null> {
  try {
    const response = await fetch(`${API_BASE}/movie/${id}?append_to_response=keywords,videos,release_dates,external_ids`);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return await response.json();
  } catch (error) {
    console.error(`[TMDB] Failed to fetch movie details for ${id}:`, error);
    return null;
  }
}

/**
 * Get TV show details
 */
export async function getTVShowDetails(id: string | number): Promise<TMDBTVShowDetails | null> {
  try {
    const response = await fetch(`${API_BASE}/tv/${id}?append_to_response=keywords,videos,content_ratings,external_ids`);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return await response.json();
  } catch (error) {
    console.error(`[TMDB] Failed to fetch TV details for ${id}:`, error);
    return null;
  }
}

/**
 * Get season details
 */
export async function getSeasonDetails(tvId: string | number, seasonNumber: number): Promise<TMDBSeasonDetails | null> {
  try {
    const response = await fetch(`${API_BASE}/tv/${tvId}/season/${seasonNumber}`);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return await response.json();
  } catch (error) {
    console.error(`[TMDB] Failed to fetch season ${seasonNumber} for TV ${tvId}:`, error);
    return null;
  }
}

/**
 * Get collection details
 */
export async function getCollectionDetails(id: string | number): Promise<TMDBCollection | null> {
  try {
    const response = await fetch(`${API_BASE}/collection/${id}`);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return await response.json();
  } catch (error) {
    console.error(`[TMDB] Failed to fetch collection details for ${id}:`, error);
    return null;
  }
}

/**
 * Get poster URL from TMDB path
 */
export function getPosterUrl(path: string | null, size: 'w185' | 'w342' | 'w500' = 'w342'): string | null {
  if (!path) return null;
  const cleanPath = path.startsWith('/') ? path.substring(1) : path;
  return `/api/tmdb/image/${size}/${cleanPath}`;
}

/**
 * Get backdrop URL from TMDB path
 */
export function getBackdropUrl(path: string | null, size: 'w780' | 'w1280' | 'original' = 'w1280'): string | null {
  if (!path) return null;
  const cleanPath = path.startsWith('/') ? path.substring(1) : path;
  return `/api/tmdb/image/${size}/${cleanPath}`;
}

/**
 * Get year from date string
 */
export function getYear(dateString: string | undefined | null): number | null {
  if (!dateString) return null;
  return new Date(dateString).getFullYear();
}

/**
 * Get placeholder poster image
 */
export function getPlaceholderPoster(): string {
  return '/images/placeholder-poster.svg';
}

/**
 * Get placeholder banner/backdrop image
 */
export function getPlaceholderBanner(): string {
  return '/images/placeholder-banner.png';
}

/**
 * Get poster URL with fallback to placeholder
 */
export function getPosterUrlOrPlaceholder(path: string | null, size: 'w185' | 'w342' | 'w500' = 'w342'): string {
  return getPosterUrl(path, size) || getPlaceholderPoster();
}

/**
 * Get backdrop URL with fallback to placeholder
 */
export function getBackdropUrlOrPlaceholder(path: string | null, size: 'w780' | 'w1280' | 'original' = 'w1280'): string {
  return getBackdropUrl(path, size) || getPlaceholderBanner();
}

/**
 * Get search results
 */
export async function searchTMDB(query: string, type: 'movie' | 'tv' | 'multi' = 'multi'): Promise<TMDBMovie[] | TMDBTVShow[]> {
  try {
    const response = await fetch(`${API_BASE}/search/${type}?query=${encodeURIComponent(query)}`);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const data: TMDBResponse<any> = await response.json();
    return data.results;
  } catch (error) {
    console.error(`[TMDB] Failed to search for ${query}:`, error);
    return [];
  }
}

/**
 * Get similar titles
 */
export async function getSimilar(type: 'movie' | 'tv', id: string | number): Promise<any[]> {
  try {
    const response = await fetch(`${API_BASE}/${type}/${id}/similar`);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const data: TMDBResponse<any> = await response.json();
    return data.results;
  } catch (error) {
    console.error(`[TMDB] Failed to fetch similar for ${id}:`, error);
    return [];
  }
}

/**
 * Get recommendations
 */
export async function getRecommendations(type: 'movie' | 'tv', id: string | number): Promise<any[]> {
  try {
    const response = await fetch(`${API_BASE}/${type}/${id}/recommendations`);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const data: TMDBResponse<any> = await response.json();
    return data.results;
  } catch (error) {
    console.error(`[TMDB] Failed to fetch recommendations for ${id}:`, error);
    return [];
  }
}
