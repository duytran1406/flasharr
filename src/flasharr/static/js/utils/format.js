/**
 * Flasharr: Formatting Utilities
 * 
 * Common formatters for bytes, dates, durations, and other display values.
 */

/**
 * Formats bytes into human-readable string.
 * 
 * @param {number} bytes - Number of bytes
 * @param {number} decimals - Decimal places (default: 2)
 * @returns {string} - Formatted string (e.g., "1.5 GB")
 */
export function formatBytes(bytes, decimals = 2) {
    if (bytes === 0 || bytes === null || bytes === undefined) return '0 B';
    if (isNaN(bytes)) return '0 B';

    const k = 1024;
    const dm = decimals < 0 ? 0 : decimals;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];

    const i = Math.floor(Math.log(Math.abs(bytes)) / Math.log(k));
    const index = Math.min(i, sizes.length - 1);

    return parseFloat((bytes / Math.pow(k, index)).toFixed(dm)) + ' ' + sizes[index];
}

/**
 * Formats bytes per second into speed string.
 * 
 * @param {number} bytesPerSec - Bytes per second
 * @returns {string} - Formatted string (e.g., "15.3 MB/s")
 */
export function formatSpeed(bytesPerSec) {
    if (!bytesPerSec || bytesPerSec <= 0) return '0 B/s';
    return formatBytes(bytesPerSec, 1) + '/s';
}

/**
 * Formats seconds into human-readable duration.
 * 
 * @param {number} seconds - Duration in seconds
 * @returns {string} - Formatted string (e.g., "2h 15m")
 */
export function formatDuration(seconds) {
    if (!seconds || seconds <= 0 || !isFinite(seconds)) return '--';

    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);

    if (hours > 24) {
        const days = Math.floor(hours / 24);
        const remainingHours = hours % 24;
        return `${days}d ${remainingHours}h`;
    }

    if (hours > 0) {
        return `${hours}h ${minutes}m`;
    }

    if (minutes > 0) {
        return `${minutes}m ${secs}s`;
    }

    return `${secs}s`;
}

/**
 * Formats a timestamp or Date object into locale string.
 * 
 * @param {number|string|Date} timestamp - Unix timestamp (seconds or ms), ISO string, or Date
 * @param {Object} options - Intl.DateTimeFormat options
 * @returns {string} - Formatted date string
 */
export function formatDate(timestamp, options = {}) {
    if (!timestamp) return '--';

    let date;

    if (timestamp instanceof Date) {
        date = timestamp;
    } else if (typeof timestamp === 'string') {
        date = new Date(timestamp);
    } else if (typeof timestamp === 'number') {
        // Detect if it's seconds or milliseconds
        date = new Date(timestamp < 1e12 ? timestamp * 1000 : timestamp);
    } else {
        return '--';
    }

    if (isNaN(date.getTime())) return '--';

    const defaultOptions = {
        dateStyle: 'short',
        timeStyle: 'short',
        ...options
    };

    return new Intl.DateTimeFormat(undefined, defaultOptions).format(date);
}

/**
 * Formats a date as relative time (e.g., "2 hours ago").
 * 
 * @param {number|string|Date} timestamp - The timestamp
 * @returns {string} - Relative time string
 */
export function formatRelativeTime(timestamp) {
    if (!timestamp) return '--';

    let date;
    if (timestamp instanceof Date) {
        date = timestamp;
    } else if (typeof timestamp === 'number') {
        date = new Date(timestamp < 1e12 ? timestamp * 1000 : timestamp);
    } else {
        date = new Date(timestamp);
    }

    if (isNaN(date.getTime())) return '--';

    const now = new Date();
    const diffMs = now - date;
    const diffSecs = Math.floor(diffMs / 1000);
    const diffMins = Math.floor(diffSecs / 60);
    const diffHours = Math.floor(diffMins / 60);
    const diffDays = Math.floor(diffHours / 24);

    if (diffSecs < 60) return 'just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    if (diffDays < 7) return `${diffDays}d ago`;

    return formatDate(date, { dateStyle: 'short' });
}

/**
 * Formats a percentage value.
 * 
 * @param {number} value - The percentage (0-100)
 * @param {number} decimals - Decimal places
 * @returns {string} - Formatted percentage
 */
export function formatPercent(value, decimals = 1) {
    if (value === null || value === undefined || isNaN(value)) return '0%';
    return `${parseFloat(value).toFixed(decimals)}%`;
}

/**
 * Truncates a string to a maximum length with ellipsis.
 * 
 * @param {string} str - The string to truncate
 * @param {number} maxLength - Maximum length
 * @param {string} suffix - Suffix to add (default: '...')
 * @returns {string} - Truncated string
 */
export function truncate(str, maxLength, suffix = '...') {
    if (!str || typeof str !== 'string') return '';
    if (str.length <= maxLength) return str;
    return str.slice(0, maxLength - suffix.length) + suffix;
}

/**
 * Formats a number with thousand separators.
 * 
 * @param {number} num - The number
 * @returns {string} - Formatted number
 */
export function formatNumber(num) {
    if (num === null || num === undefined || isNaN(num)) return '0';
    return new Intl.NumberFormat().format(num);
}

/**
 * Capitalizes the first letter of a string.
 * 
 * @param {string} str - The string
 * @returns {string} - Capitalized string
 */
export function capitalize(str) {
    if (!str || typeof str !== 'string') return '';
    return str.charAt(0).toUpperCase() + str.slice(1);
}
