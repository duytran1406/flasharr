/**
 * Flasharr: API Module
 * 
 * Centralized API client for all backend communication.
 * All fetch calls should go through this module.
 */

const API_BASE = '';  // Same origin

/**
 * Default fetch options
 */
const defaultOptions = {
    headers: {
        'Content-Type': 'application/json',
    },
    credentials: 'same-origin',
};

/**
 * Handles API response, throwing on non-OK status.
 * 
 * @param {Response} response - Fetch response
 * @returns {Promise<any>} - Parsed JSON
 */
async function handleResponse(response) {
    if (!response.ok) {
        const error = new Error(`HTTP ${response.status}: ${response.statusText}`);
        error.status = response.status;
        try {
            error.data = await response.json();
        } catch {
            error.data = null;
        }
        throw error;
    }

    const contentType = response.headers.get('content-type');
    if (contentType && contentType.includes('application/json')) {
        return response.json();
    }
    return response.text();
}

/**
 * Performs a GET request.
 * 
 * @param {string} endpoint - API endpoint
 * @param {Object} params - Query parameters
 * @returns {Promise<any>}
 */
export async function get(endpoint, params = {}) {
    const url = new URL(API_BASE + endpoint, window.location.origin);
    Object.entries(params).forEach(([key, value]) => {
        if (value !== undefined && value !== null && value !== '') {
            url.searchParams.append(key, value);
        }
    });

    const response = await fetch(url.toString(), {
        ...defaultOptions,
        method: 'GET',
    });

    return handleResponse(response);
}

/**
 * Performs a POST request.
 * 
 * @param {string} endpoint - API endpoint
 * @param {Object} data - Request body
 * @returns {Promise<any>}
 */
export async function post(endpoint, data = {}) {
    const response = await fetch(API_BASE + endpoint, {
        ...defaultOptions,
        method: 'POST',
        body: JSON.stringify(data),
    });

    return handleResponse(response);
}

/**
 * Performs a PUT request.
 * 
 * @param {string} endpoint - API endpoint
 * @param {Object} data - Request body
 * @returns {Promise<any>}
 */
export async function put(endpoint, data = {}) {
    const response = await fetch(API_BASE + endpoint, {
        ...defaultOptions,
        method: 'PUT',
        body: JSON.stringify(data),
    });

    return handleResponse(response);
}

/**
 * Performs a DELETE request.
 * 
 * @param {string} endpoint - API endpoint
 * @returns {Promise<any>}
 */
export async function del(endpoint) {
    const response = await fetch(API_BASE + endpoint, {
        ...defaultOptions,
        method: 'DELETE',
    });

    return handleResponse(response);
}

// =============================================================================
// DOMAIN-SPECIFIC API METHODS
// =============================================================================

/**
 * Downloads API
 */
export const downloads = {
    /**
     * List all downloads
     * @returns {Promise<{downloads: Array}>}
     */
    list: () => get('/api/downloads'),

    /**
     * Add a new download
     * @param {string} url - Fshare URL
     * @returns {Promise<{status: string, task_id?: string}>}
     */
    add: (url) => post('/api/downloads', { url }),

    /**
     * Perform action on a download
     * @param {string} id - Task ID
     * @param {string} action - Action (pause, resume, cancel, retry)
     * @returns {Promise<{status: string}>}
     */
    action: (id, action) => post(`/api/downloads/${id}/${action}`),

    /**
     * Batch actions
     * @param {string} action - resume-all, pause-all
     * @returns {Promise<{status: string}>}
     */
    batch: (action) => post(`/api/downloads/${action}`),

    /**
     * Delete a download
     * @param {string} id - Task ID
     * @returns {Promise<{status: string}>}
     */
    delete: (id) => del(`/api/downloads/${id}`),
};

/**
 * TMDB API
 */
export const tmdb = {
    /**
     * Search TMDB
     * @param {string} query - Search query
     * @param {string} type - movie, tv, or multi
     * @returns {Promise<{results: Array}>}
     */
    search: (query, type = 'multi') => get(`/api/tmdb/search/${type}`, { q: query }),

    /**
     * Get movie details
     * @param {number} id - TMDB ID
     * @returns {Promise<Object>}
     */
    movie: (id) => get(`/api/tmdb/movie/${id}`),

    /**
     * Get TV show details
     * @param {number} id - TMDB ID
     * @returns {Promise<Object>}
     */
    tv: (id) => get(`/api/tmdb/tv/${id}`),

    /**
     * Get TV season details
     * @param {number} tvId - TMDB TV ID
     * @param {number} seasonNumber - Season number
     * @returns {Promise<Object>}
     */
    season: (tvId, seasonNumber) => get(`/api/tmdb/tv/${tvId}/season/${seasonNumber}`),

    /**
     * Discover movies or TV
     * @param {string} type - movie or tv
     * @param {Object} params - Discovery parameters
     * @returns {Promise<{results: Array, total_pages: number}>}
     */
    discover: (type, params = {}) => get(`/api/tmdb/discover/${type}`, params),

    /**
     * Get trending content
     * @param {string} type - movie, tv, or all
     * @param {string} timeWindow - day or week
     * @returns {Promise<{results: Array}>}
     */
    trending: (type = 'all', timeWindow = 'week') => get(`/api/tmdb/trending/${type}/${timeWindow}`),

    /**
     * Get genre list
     * @param {string} type - movie or tv
     * @returns {Promise<{genres: Array}>}
     */
    genres: (type) => get(`/api/tmdb/genres/${type}`),

    /**
     * Get collection details
     * @param {number} id - Collection ID
     * @returns {Promise<Object>}
     */
    collection: (id) => get(`/api/tmdb/collection/${id}`),
};

/**
 * Smart Search API (Fshare search based on TMDB metadata)
 */
export const smartSearch = {
    /**
     * Search for a movie
     * @param {number} tmdbId - TMDB movie ID
     * @param {string} title - Movie title
     * @param {number} year - Release year
     * @returns {Promise<{results: Array}>}
     */
    movie: (tmdbId, title, year) => get('/api/smart-search/movie', {
        tmdb_id: tmdbId,
        title,
        year
    }),

    /**
     * Search for a TV show/season/episode
     * @param {number} tmdbId - TMDB TV ID
     * @param {string} title - Show title
     * @param {number} year - First air year
     * @param {number} season - Season number (optional)
     * @param {number} episode - Episode number (optional)
     * @returns {Promise<{results: Array}>}
     */
    tv: (tmdbId, title, year, season = null, episode = null) => {
        const params = { tmdb_id: tmdbId, title, year };
        if (season !== null) params.season = season;
        if (episode !== null) params.episode = episode;
        return get('/api/smart-search/tv', params);
    },
};

/**
 * Discovery API (Fshare trending)
 */
export const discovery = {
    /**
     * Get trending Fshare files
     * @returns {Promise<{results: Array}>}
     */
    trending: () => get('/api/discovery/trending'),
};

/**
 * Explore API (Direct Fshare search)
 */
export const explore = {
    /**
     * Search Fshare directly
     * @param {string} query - Search query
     * @returns {Promise<{results: Array}>}
     */
    search: (query) => get('/api/explore/search', { q: query }),
};

/**
 * Account API
 */
export const accounts = {
    /**
     * Get all accounts
     * @returns {Promise<{accounts: Array}>}
     */
    list: () => get('/api/accounts'),

    /**
     * Get account status
     * @returns {Promise<Object>}
     */
    status: () => get('/api/accounts/status'),

    /**
     * Add a new account
     * @param {string} email - Account email
     * @param {string} password - Account password
     * @returns {Promise<{status: string}>}
     */
    add: (email, password) => post('/api/accounts/add', { email, password }),

    /**
     * Re-login an account
     * @param {string} email - Account email
     * @returns {Promise<{status: string}>}
     */
    relogin: (email) => post('/api/accounts/relogin', { email }),
};

/**
 * Settings API
 */
export const settings = {
    /**
     * Get all settings
     * @returns {Promise<Object>}
     */
    get: () => get('/api/settings'),

    /**
     * Update settings
     * @param {Object} data - Settings to update
     * @returns {Promise<{status: string}>}
     */
    update: (data) => post('/api/settings', data),
};

/**
 * System API
 */
export const system = {
    /**
     * Get system status
     * @returns {Promise<Object>}
     */
    status: () => get('/api/system/status'),

    /**
     * Get recent logs
     * @returns {Promise<{logs: Array}>}
     */
    logs: () => get('/api/system/logs'),
};
