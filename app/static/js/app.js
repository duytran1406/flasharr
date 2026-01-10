// Fshare-Arr Bridge - Frontend JavaScript
// Homepage Style Dashboard Integration

class FshareBridge {
    constructor() {
        this.init();
    }

    init() {
        this.setupEventListeners();
        this.loadDashboardData();
        this.loadDownloads();

        // Auto-refresh stats every 10s
        setInterval(() => this.loadDashboardData(), 10000);
    }

    // Dashboard Data & Stats
    async loadDashboardData() {
        try {
            const response = await fetch('/api/stats');
            const data = await response.json();

            if (data) {
                this.updateUIValues(data);
            }
        } catch (error) {
            console.error('Load stats error:', error);
        }
    }

    updateUIValues(data) {
        // Top bar stats
        this.setText('top-speedtest', data.system.speedtest);
        this.setText('top-uptime', this.formatUptime(data.system.uptime));
        this.setText('top-downloads', data.pyload.active);

        // pyLoad card
        this.setText('stats-active-count', data.pyload.active);
        this.setText('stats-speed', data.pyload.speed);
        this.setText('stats-total-count', data.pyload.total);
        this.updateBadge('pyload-status-badge', data.pyload.connected);
        this.setText('pyload-active-desc', `${data.pyload.active} active downloads currently`);

        // Bridge card
        this.setText('stats-search-count', data.bridge.searches);
        this.setText('stats-success-rate', data.bridge.success_rate);
    }

    formatUptime(seconds) {
        const s = parseInt(seconds);
        if (s < 60) return `${s}s`;
        if (s < 3600) return `${Math.floor(s / 60)}m`;
        if (s < 86400) return `${Math.floor(s / 3600)}h`;
        return `${Math.floor(s / 86400)}d`;
    }

    setText(id, value) {
        const el = document.getElementById(id);
        if (el) el.textContent = value;
    }

    updateBadge(id, isConnected) {
        const el = document.getElementById(id);
        if (el) {
            el.textContent = isConnected ? 'Connected' : 'Disconnected';
            el.className = `badge ${isConnected ? 'badge-success' : 'badge-danger'}`;
        }
    }

    // Search Operations
    async search(query) {
        if (!query) return;

        // Redirect to search page or handle inline
        if (window.location.pathname !== '/search') {
            window.location.href = `/search?q=${encodeURIComponent(query)}`;
            return;
        }

        const resultsContainer = document.getElementById('search-results');
        if (!resultsContainer) return;

        resultsContainer.innerHTML = '<div style="grid-column: 1/-1; text-align: center; padding: 3rem;"><p>Searching Fshare...</p></div>';

        try {
            const response = await fetch(`/api/search?q=${encodeURIComponent(query)}`);
            const data = await response.json();

            if (data.results && data.results.length > 0) {
                this.displayResults(data.results);
            } else {
                resultsContainer.innerHTML = '<div style="grid-column: 1/-1; text-align: center; padding: 3rem;"><p class="text-secondary">No results found</p></div>';
            }
        } catch (error) {
            console.error('Search error:', error);
            resultsContainer.innerHTML = '<p class="text-danger">Search failed</p>';
        }
    }

    displayResults(results) {
        const container = document.getElementById('search-results');
        if (!container) return;

        container.innerHTML = results.map(result => this.createResultCard(result)).join('');
    }

    createResultCard(result) {
        const { name, score, url, size } = result;
        const metadata = this.parseMetadata(name);
        const sizeStr = this.formatSize(size);

        return `
            <div class="card">
                <div class="card-top">
                    <div class="card-title-group">
                        <div class="card-title">${this.escapeHtml(name)}</div>
                    </div>
                </div>
                <div class="card-content">
                    <div style="display: flex; gap: 0.5rem; margin-bottom: 1rem; flex-wrap: wrap;">
                        <span class="badge badge-success">Score: ${score}</span>
                        <span class="badge badge-secondary">${sizeStr}</span>
                        ${metadata.resolution ? `<span class="badge badge-secondary">${metadata.resolution}</span>` : ''}
                        ${metadata.vietnamese ? `<span class="badge badge-success">${metadata.vietnamese}</span>` : ''}
                    </div>
                </div>
                <button class="btn btn-primary" style="width: 100%; justify-content: center;" 
                        onclick="bridge.download('${this.escapeHtml(url)}', '${this.escapeHtml(name)}')">
                    Download
                </button>
            </div>
        `;
    }

    parseMetadata(filename) {
        const metadata = {};
        if (filename.match(/2160p|4K/i)) metadata.resolution = '4K';
        else if (filename.match(/1080p/i)) metadata.resolution = '1080p';
        if (filename.match(/vietsub/i)) metadata.vietnamese = 'Vietsub';
        else if (filename.match(/thuyết minh|thuyet minh/i)) metadata.vietnamese = 'Thuyết Minh';
        return metadata;
    }

    formatSize(bytes) {
        if (bytes === 0) return '0 B';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    }

    // Download Operations
    async download(url, name) {
        try {
            const response = await fetch('/api/download', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ url, name })
            });

            const data = await response.json();

            if (data.success) {
                alert(`Added to queue: ${data.normalized}`);
                this.loadDownloads();
            } else {
                alert('Failed to add download');
            }
        } catch (error) {
            console.error('Download error:', error);
        }
    }

    async loadDownloads() {
        const container = document.getElementById('downloads-list');
        if (!container) return;

        try {
            const response = await fetch('/api/downloads');
            const data = await response.json();

            if (data.downloads && data.downloads.length > 0) {
                container.innerHTML = data.downloads.map(d => this.createDownloadRow(d)).join('');
            } else {
                container.innerHTML = '<p class="text-secondary" style="font-size: 0.85rem;">No active activity in queue</p>';
            }
        } catch (error) {
            console.error('Load downloads error:', error);
        }
    }

    createDownloadRow(d) {
        return `
            <div style="display: flex; justify-content: space-between; align-items: center; padding: 0.5rem; border-bottom: 1px solid var(--card-border);">
                <div style="font-size: 0.85rem; max-width: 70%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                    ${this.escapeHtml(d.name)}
                </div>
                <div class="badge badge-secondary">${d.progress}%</div>
            </div>
        `;
    }

    // Autocomplete handling
    async handleAutocomplete() {
        const input = document.getElementById('search-input');
        const query = input.value.trim();

        if (query.length < 2) {
            this.hideSuggestions();
            return;
        }

        try {
            const response = await fetch(`/api/autocomplete?q=${encodeURIComponent(query)}`);
            const data = await response.json();
            if (data.suggestions) {
                this.displaySuggestions(data.suggestions);
            }
        } catch (e) { console.error(e); }
    }

    displaySuggestions(suggestions) {
        const container = document.getElementById('search-suggestions');
        if (!container) return;

        if (suggestions.length === 0) {
            container.style.display = 'none';
            return;
        }

        container.innerHTML = suggestions.map(s => `
            <div style="padding: 0.75rem 1rem; cursor: pointer; border-bottom: 1px solid rgba(255,255,255,0.05);" 
                 onclick="bridge.selectSuggestion('${this.escapeHtml(s)}')">
                ${this.escapeHtml(s)}
            </div>
        `).join('');
        container.style.display = 'block';
        container.style.background = 'rgba(15, 23, 42, 0.95)';
        container.style.borderRadius = '0.75rem';
        container.style.marginTop = '0.5rem';
        container.style.zIndex = '1000';
    }

    selectSuggestion(s) {
        document.getElementById('search-input').value = s;
        this.hideSuggestions();
        this.search(s);
    }

    hideSuggestions() {
        const el = document.getElementById('search-suggestions');
        if (el) el.style.display = 'none';
    }

    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    setupEventListeners() {
        const searchInput = document.getElementById('search-input');
        if (searchInput) {
            let timeout;
            searchInput.addEventListener('input', () => {
                clearTimeout(timeout);
                timeout = setTimeout(() => this.handleAutocomplete(), 300);
            });
            searchInput.addEventListener('keypress', (e) => {
                if (e.key === 'Enter') this.search(searchInput.value);
            });
        }

        document.addEventListener('click', (e) => {
            if (!e.target.closest('.search-input-group')) this.hideSuggestions();
        });
    }
}

// Initialize
let bridge;
document.addEventListener('DOMContentLoaded', () => {
    bridge = new FshareBridge();

    // Check for query param
    const params = new URLSearchParams(window.location.search);
    const q = params.get('q');
    if (q) {
        const input = document.getElementById('search-input');
        if (input) input.value = q;
        bridge.search(q);
    }
});
