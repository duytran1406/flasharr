// Fshare-Arr Bridge - Frontend JavaScript

class FshareBridge {
    constructor() {
        this.theme = localStorage.getItem('theme') || 'dark';
        this.init();
    }

    init() {
        this.applyTheme();
        this.setupEventListeners();
        this.loadDashboardData();
    }

    // Theme Management
    applyTheme() {
        document.documentElement.className = this.theme;
    }

    toggleTheme() {
        this.theme = this.theme === 'dark' ? 'light' : 'dark';
        localStorage.setItem('theme', this.theme);
        this.applyTheme();
    }

    // Search Functionality
    async search(query) {
        const resultsContainer = document.getElementById('search-results');
        resultsContainer.innerHTML = '<div class="spinner"></div>';

        try {
            const response = await fetch(`/api/search?q=${encodeURIComponent(query)}`);
            const data = await response.json();

            if (data.results && data.results.length > 0) {
                this.displayResults(data.results);
            } else {
                resultsContainer.innerHTML = '<p class="text-secondary">No results found</p>';
            }
        } catch (error) {
            console.error('Search error:', error);
            resultsContainer.innerHTML = '<p class="text-danger">Search failed. Please try again.</p>';
        }
    }

    displayResults(results) {
        const container = document.getElementById('search-results');
        container.innerHTML = results.map(result => this.createResultCard(result)).join('');
    }

    createResultCard(result) {
        const { name, score, url, size } = result;
        const metadata = this.parseMetadata(name);

        return `
            <div class="result-card">
                <div class="result-header">
                    <h3 class="result-title">${this.escapeHtml(name)}</h3>
                    <span class="score-badge">Score: ${score}</span>
                </div>
                <div class="result-meta">
                    ${metadata.resolution ? `<span class="badge resolution">${metadata.resolution}</span>` : ''}
                    ${metadata.year ? `<span class="badge">${metadata.year}</span>` : ''}
                    ${metadata.vietnamese ? `<span class="badge vietnamese">${metadata.vietnamese}</span>` : ''}
                </div>
                <div class="result-actions">
                    <button class="btn btn-primary" onclick="bridge.download('${this.escapeHtml(url)}', '${this.escapeHtml(name)}')">
                        Download
                    </button>
                </div>
            </div>
        `;
    }

    parseMetadata(filename) {
        const metadata = {};

        // Resolution
        if (filename.match(/2160p|4K/i)) metadata.resolution = '4K';
        else if (filename.match(/1080p/i)) metadata.resolution = '1080p';
        else if (filename.match(/720p/i)) metadata.resolution = '720p';

        // Year
        const yearMatch = filename.match(/\b(19[5-9][0-9]|20[0-2][0-9])\b/);
        if (yearMatch) metadata.year = yearMatch[1];

        // Vietnamese
        if (filename.match(/vietsub/i)) metadata.vietnamese = 'Vietsub';
        else if (filename.match(/thuyết minh|thuyet minh/i)) metadata.vietnamese = 'Thuyết Minh';
        else if (filename.match(/lồng tiếng|long tieng/i)) metadata.vietnamese = 'Lồng Tiếng';

        return metadata;
    }

    // Download Management
    async download(url, name) {
        try {
            const response = await fetch('/api/download', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ url, name })
            });

            const data = await response.json();

            if (data.success) {
                this.showNotification('Download added to queue!', 'success');
                this.loadDownloads();
            } else {
                this.showNotification('Failed to add download', 'error');
            }
        } catch (error) {
            console.error('Download error:', error);
            this.showNotification('Download failed', 'error');
        }
    }

    async loadDownloads() {
        try {
            const response = await fetch('/api/downloads');
            const data = await response.json();

            if (data.downloads) {
                this.displayDownloads(data.downloads);
            }
        } catch (error) {
            console.error('Load downloads error:', error);
        }
    }

    displayDownloads(downloads) {
        const container = document.getElementById('downloads-list');
        if (!container) return;

        if (downloads.length === 0) {
            container.innerHTML = '<p class="text-secondary">No active downloads</p>';
            return;
        }

        container.innerHTML = downloads.map(download => this.createDownloadCard(download)).join('');
    }

    createDownloadCard(download) {
        const { name, normalized, progress, status } = download;

        return `
            <div class="download-item">
                <div class="download-info">
                    <p class="filename">${this.escapeHtml(name)}</p>
                    ${normalized ? `<p class="normalized">${this.escapeHtml(normalized)}</p>` : ''}
                </div>
                <div class="progress-bar">
                    <div class="progress-fill" style="width: ${progress}%"></div>
                </div>
                <span class="status">${status} (${progress}%)</span>
            </div>
        `;
    }

    // Dashboard Data
    async loadDashboardData() {
        try {
            const response = await fetch('/api/stats');
            const data = await response.json();

            if (data.stats) {
                this.updateStats(data.stats);
            }
        } catch (error) {
            console.error('Load stats error:', error);
        }
    }

    updateStats(stats) {
        const elements = {
            'total-searches': stats.totalSearches || 0,
            'active-downloads': stats.activeDownloads || 0,
            'success-rate': stats.successRate || 0
        };

        Object.entries(elements).forEach(([id, value]) => {
            const element = document.getElementById(id);
            if (element) element.textContent = value;
        });
    }

    // Autocomplete
    async autocomplete(query) {
        if (query.length < 2) return;

        try {
            const response = await fetch(`/api/autocomplete?q=${encodeURIComponent(query)}`);
            const data = await response.json();

            if (data.suggestions) {
                this.displaySuggestions(data.suggestions);
            }
        } catch (error) {
            console.error('Autocomplete error:', error);
        }
    }

    displaySuggestions(suggestions) {
        const container = document.getElementById('search-suggestions');
        if (!container) return;

        if (suggestions.length === 0) {
            container.style.display = 'none';
            return;
        }

        container.innerHTML = suggestions.map(suggestion => `
            <div class="suggestion-item" onclick="bridge.selectSuggestion('${this.escapeHtml(suggestion)}')">
                ${this.escapeHtml(suggestion)}
            </div>
        `).join('');
        container.style.display = 'block';
    }

    selectSuggestion(suggestion) {
        const input = document.getElementById('search-input');
        if (input) {
            input.value = suggestion;
            this.search(suggestion);
        }
        this.hideSuggestions();
    }

    hideSuggestions() {
        const container = document.getElementById('search-suggestions');
        if (container) container.style.display = 'none';
    }

    // Utilities
    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    showNotification(message, type = 'info') {
        // Simple notification - you can enhance this
        alert(message);
    }

    setupEventListeners() {
        // Search input
        const searchInput = document.getElementById('search-input');
        if (searchInput) {
            let timeout;
            searchInput.addEventListener('input', (e) => {
                clearTimeout(timeout);
                timeout = setTimeout(() => this.autocomplete(e.target.value), 300);
            });

            searchInput.addEventListener('keypress', (e) => {
                if (e.key === 'Enter') {
                    this.search(e.target.value);
                    this.hideSuggestions();
                }
            });
        }

        // Theme toggle
        const themeToggle = document.getElementById('theme-toggle');
        if (themeToggle) {
            themeToggle.addEventListener('click', () => this.toggleTheme());
        }

        // Click outside to hide suggestions
        document.addEventListener('click', (e) => {
            if (!e.target.closest('.search-box')) {
                this.hideSuggestions();
            }
        });
    }
}

// Initialize on page load
let bridge;
document.addEventListener('DOMContentLoaded', () => {
    bridge = new FshareBridge();
});
