// Fshare-Arr Bridge - Frontend JavaScript
// NEXUS Dashboard v2.0

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
        setInterval(() => this.loadDownloads(), 5000);
    }

    // Dashboard Data & Stats
    async loadDashboardData() {
        try {
            const response = await fetch('/api/stats');
            const data = await response.json();

            if (data) {
                this.updateDashboard(data);
            }
        } catch (error) {
            console.error('Load stats error:', error);
        }
    }

    updateDashboard(data) {
        // Header stats
        this.setText('header-speed', data.system.speedtest);
        this.setText('header-active', data.pyload.active);
        this.setText('header-uptime', this.formatUptime(data.system.uptime));

        // Widget 1: System Status
        this.updateBadge('indexer-status', true, 'STABLE');
        this.updateBadge('sabnzbd-status', true, 'READY');

        // Widget 2: Downloader
        const pyloadConnected = data.pyload.connected;
        this.updateStatusIndicator('pyload-status-indicator', pyloadConnected);
        this.updateBadge('fshare-account-status', pyloadConnected, pyloadConnected ? 'PREMIUM PLUS' : 'OFFLINE');
        this.setText('active-downloads-count', String(data.pyload.active).padStart(2, '0'));
        this.setText('queue-count', data.pyload.total);

        // Widget 3: Search Engine
        this.updateStatusIndicator('timfshare-status-indicator', true);
        this.updateBadge('timfshare-status', true, 'ONLINE');
        this.setText('api-health', '100%');
        this.setText('api-ping', '45ms');
    }

    updateStatusIndicator(id, isOnline) {
        const el = document.getElementById(id);
        if (el) {
            el.className = `widget-status ${isOnline ? 'online' : 'offline'}`;
        }
    }

    updateBadge(id, isSuccess, text) {
        const el = document.getElementById(id);
        if (el) {
            el.className = `status-badge ${isSuccess ? 'success' : 'error'}`;
            el.textContent = text;
        }
    }

    formatUptime(seconds) {
        const s = parseInt(seconds);
        if (s < 60) return `${s}s`;
        if (s < 3600) return `${Math.floor(s / 60)}m`;
        if (s < 86400) return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
        return `${Math.floor(s / 86400)}d ${Math.floor((s % 86400) / 3600)}h`;
    }

    setText(id, value) {
        const el = document.getElementById(id);
        if (el) el.textContent = value;
    }

    // Download Manager
    async loadDownloads() {
        const container = document.getElementById('download-manager-list');
        if (!container) return;

        try {
            const response = await fetch('/api/downloads');
            const data = await response.json();

            if (data.downloads && data.downloads.length > 0) {
                // Show top 5 downloads
                const topDownloads = data.downloads.slice(0, 5);
                container.innerHTML = topDownloads.map(d => this.createDownloadRow(d)).join('');
            } else {
                container.innerHTML = `
                    <tr>
                        <td colspan="5" style="text-align: center; padding: 3rem; color: var(--text-muted);">
                            No active downloads in queue
                        </td>
                    </tr>
                `;
            }
        } catch (error) {
            console.error('Load downloads error:', error);
        }
    }

    createDownloadRow(d) {
        const statusClass = d.status === 'Running' ? 'info' :
            d.status === 'Finished' ? 'success' :
                d.status === 'Stop' ? 'error' : 'warning';

        const progressColor = d.status === 'Running' ? 'var(--accent-blue)' :
            d.status === 'Finished' ? 'var(--accent-green)' :
                'var(--text-muted)';

        return `
            <tr>
                <td>
                    <div class="download-name">${this.escapeHtml(d.name)}</div>
                    <div style="font-size: 0.7rem; color: var(--text-muted);">${d.status.toUpperCase()}</div>
                </td>
                <td class="download-size">${d.size}</td>
                <td><span class="status-badge ${statusClass}">${d.status.toUpperCase()}</span></td>
                <td>
                    <div class="download-progress">
                        <div class="progress-bar" style="width: 120px;">
                            <div class="progress-fill" style="width: ${d.progress}%; background: ${progressColor};"></div>
                        </div>
                        <span class="progress-text">${d.progress}%</span>
                        <span style="font-size: 0.7rem; color: var(--text-muted);">${d.info}</span>
                    </div>
                </td>
                <td>
                    <div class="download-controls">
                        <button class="icon-btn" title="Pause">⏸</button>
                        <button class="icon-btn" title="Delete">🗑</button>
                    </div>
                </td>
            </tr>
        `;
    }

    // Search Operations
    async search(query) {
        if (!query) return;

        if (window.location.pathname !== '/search') {
            window.location.href = `/search?q=${encodeURIComponent(query)}`;
            return;
        }

        const resultsContainer = document.getElementById('search-results');
        if (!resultsContainer) return;

        resultsContainer.innerHTML = '<div style="text-align: center; padding: 3rem;"><p>Searching Fshare...</p></div>';

        try {
            const response = await fetch(`/api/search?q=${encodeURIComponent(query)}`);
            const data = await response.json();

            if (data.results && data.results.length > 0) {
                this.displayResults(data.results);
            } else {
                resultsContainer.innerHTML = '<div style="text-align: center; padding: 3rem;"><p class="text-secondary">No results found</p></div>';
            }
        } catch (error) {
            console.error('Search error:', error);
            resultsContainer.innerHTML = '<p style="color: var(--accent-red);">Search failed</p>';
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
            <div class="widget">
                <div class="widget-header">
                    <div class="widget-title" style="font-size: 0.95rem; text-transform: none;">
                        ${this.escapeHtml(name)}
                    </div>
                </div>
                <div class="widget-content">
                    <div style="display: flex; gap: 0.5rem; margin-bottom: 1rem; flex-wrap: wrap;">
                        <span class="status-badge success">Score: ${score}</span>
                        <span class="status-badge info">${sizeStr}</span>
                        ${metadata.resolution ? `<span class="status-badge info">${metadata.resolution}</span>` : ''}
                        ${metadata.vietnamese ? `<span class="status-badge success">${metadata.vietnamese}</span>` : ''}
                    </div>
                    <button class="btn-primary" style="width: 100%; justify-content: center;" 
                            onclick="bridge.download('${this.escapeHtml(url)}', '${this.escapeHtml(name)}')">
                        Download
                    </button>
                </div>
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
                alert(`✅ Added to queue: ${data.normalized}`);
                this.loadDownloads();
            } else {
                alert('❌ Failed to add download');
            }
        } catch (error) {
            console.error('Download error:', error);
        }
    }

    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    setupEventListeners() {
        const searchInput = document.getElementById('search-input');
        if (searchInput) {
            searchInput.addEventListener('keypress', (e) => {
                if (e.key === 'Enter') this.search(searchInput.value);
            });
        }
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
