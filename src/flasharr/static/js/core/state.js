/**
 * Flasharr: State Management
 * 
 * Centralized reactive state management.
 * Components can subscribe to state changes and update automatically.
 */

/**
 * Application state store with reactive subscriptions.
 */
class AppState {
    constructor() {
        // Download state
        this.downloads = [];
        this.downloadSort = { column: 'added', direction: 'desc' };
        this.downloadPage = 1;
        this.downloadFilter = '';

        // Discover state
        this.discover = {
            type: 'movie',
            page: 1,
            sort: 'popularity.desc',
            genre: '',
            year: '',
            dateFrom: '',
            dateTo: '',
            language: '',
            certification: '',
            runtimeMin: 0,
            runtimeMax: 400,
            scoreMin: 0,
            scoreMax: 10,
            voteCountMin: 0,
            loading: false,
            hasMore: true,
            showFilters: false,
            items: [],
            scrollPosition: 0,
        };

        // Account/Quota state
        this.quota = {
            remaining: 0,
            total: 0,
            percentage: 0,
        };

        // Engine stats
        this.engineStats = {
            active: 0,
            queued: 0,
            totalSpeed: 0,
        };

        // UI state
        this.currentView = 'dashboard';
        this.sidebarCollapsed = false;
        this.theme = 'oceanholic';

        // Search state
        this.searchQuery = '';

        // Cache
        this.genreCache = {
            movie: null,
            tv: null,
        };

        // Subscribers
        this._listeners = new Map();
    }

    /**
     * Subscribe to state changes.
     * 
     * @param {string} key - State key to watch (supports dot notation)
     * @param {Function} callback - Callback(newValue, oldValue)
     * @returns {Function} - Unsubscribe function
     * 
     * @example
     * const unsub = state.subscribe('downloads', (downloads) => {
     *     renderDownloadList(downloads);
     * });
     */
    subscribe(key, callback) {
        if (!this._listeners.has(key)) {
            this._listeners.set(key, new Set());
        }
        this._listeners.get(key).add(callback);

        // Return unsubscribe function
        return () => {
            const listeners = this._listeners.get(key);
            if (listeners) {
                listeners.delete(callback);
            }
        };
    }

    /**
     * Notify subscribers of a state change.
     * 
     * @param {string} key - State key that changed
     * @param {any} newValue - New value
     * @param {any} oldValue - Previous value
     */
    _notify(key, newValue, oldValue) {
        const listeners = this._listeners.get(key);
        if (listeners) {
            listeners.forEach(callback => {
                try {
                    callback(newValue, oldValue);
                } catch (e) {
                    console.error(`State listener error for ${key}:`, e);
                }
            });
        }

        // Also notify parent keys
        const parts = key.split('.');
        while (parts.length > 1) {
            parts.pop();
            const parentKey = parts.join('.');
            const parentListeners = this._listeners.get(parentKey);
            if (parentListeners) {
                parentListeners.forEach(callback => {
                    try {
                        callback(this.get(parentKey));
                    } catch (e) {
                        console.error(`State listener error for ${parentKey}:`, e);
                    }
                });
            }
        }
    }

    /**
     * Get a state value by key (supports dot notation).
     * 
     * @param {string} key - State key
     * @returns {any}
     */
    get(key) {
        const parts = key.split('.');
        let value = this;
        for (const part of parts) {
            if (value === undefined || value === null) return undefined;
            value = value[part];
        }
        return value;
    }

    /**
     * Set a state value and notify subscribers.
     * 
     * @param {string} key - State key (supports dot notation)
     * @param {any} value - New value
     */
    set(key, value) {
        const parts = key.split('.');
        const lastPart = parts.pop();

        let target = this;
        for (const part of parts) {
            if (target[part] === undefined) {
                target[part] = {};
            }
            target = target[part];
        }

        const oldValue = target[lastPart];
        target[lastPart] = value;

        this._notify(key, value, oldValue);
    }

    /**
     * Update multiple properties of a nested state object.
     * 
     * @param {string} key - State key
     * @param {Object} updates - Properties to update
     */
    update(key, updates) {
        const current = this.get(key);
        if (current && typeof current === 'object') {
            Object.assign(current, updates);
            this._notify(key, current);
        }
    }

    /**
     * Reset discover state to defaults.
     */
    resetDiscover() {
        const defaults = {
            page: 1,
            genre: '',
            year: '',
            dateFrom: '',
            dateTo: '',
            language: '',
            certification: '',
            runtimeMin: 0,
            runtimeMax: 400,
            scoreMin: 0,
            scoreMax: 10,
            voteCountMin: 0,
            loading: false,
            hasMore: true,
            items: [],
            scrollPosition: 0,
        };
        Object.assign(this.discover, defaults);
        this._notify('discover', this.discover);
    }

    /**
     * Check if discover filters are at default values.
     * 
     * @returns {boolean}
     */
    isDefaultDiscoverState() {
        const s = this.discover;
        return !s.genre && !s.year && !s.dateFrom && !s.dateTo &&
            !s.language && !s.certification &&
            s.runtimeMin === 0 && s.runtimeMax === 400 &&
            s.scoreMin === 0 && s.scoreMax === 10;
    }
}

// Singleton instance
export const state = new AppState();

// For debugging in console
if (typeof window !== 'undefined') {
    window.__state = state;
}
