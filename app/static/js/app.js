// Fshare-Arr Bridge - Frontend JavaScript
// NEXUS Dashboard v2.0

// Network Graph Class
class NetworkGraph {
    constructor(canvasId) {
        this.dataPoints = [];
        this.maxDataPoints = 60;
        this.maxSpeed = 0;
        this.peakSpeed = 0;

        // Initialize with zeros
        for (let i = 0; i < this.maxDataPoints; i++) {
            this.dataPoints.push(0);
        }

        this.canvas = document.getElementById(canvasId);
        if (!this.canvas) {
            // console.warn('Network graph canvas not found');
            return;
        }

        this.ctx = this.canvas.getContext('2d');
        this.resize();
        window.addEventListener('resize', () => this.resize());
        this.draw();
    }

    resize() {
        if (!this.canvas) return;
        const container = this.canvas.parentElement;
        const dpr = window.devicePixelRatio || 1;

        this.canvas.width = container.clientWidth * dpr;
        this.canvas.height = container.clientHeight * dpr;
        this.canvas.style.width = container.clientWidth + 'px';
        this.canvas.style.height = container.clientHeight + 'px';

        this.ctx.scale(dpr, dpr);
        this.width = container.clientWidth;
        this.height = container.clientHeight;
    }

    addDataPoint(speedBytes) {
        if (!this.canvas) return;

        this.dataPoints.push(speedBytes);
        if (this.dataPoints.length > this.maxDataPoints) {
            this.dataPoints.shift();
        }

        if (speedBytes > this.peakSpeed) {
            this.peakSpeed = speedBytes;
        }

        this.maxSpeed = Math.max(...this.dataPoints, 1024 * 1024);
        this.draw();
    }

    draw() {
        if (!this.ctx) return;

        const ctx = this.ctx;
        const width = this.width;
        const height = this.height;

        ctx.clearRect(0, 0, width, height);

        ctx.strokeStyle = 'rgba(255, 255, 255, 0.05)';
        ctx.lineWidth = 1;

        for (let i = 0; i <= 4; i++) {
            const y = (height / 4) * i;
            ctx.beginPath();
            ctx.moveTo(0, y);
            ctx.lineTo(width, y);
            ctx.stroke();
        }

        if (this.dataPoints.length > 1) {
            const gradient = ctx.createLinearGradient(0, 0, 0, height);
            gradient.addColorStop(0, 'rgba(59, 130, 246, 0.3)');
            gradient.addColorStop(1, 'rgba(59, 130, 246, 0.0)');

            ctx.fillStyle = gradient;
            ctx.beginPath();
            ctx.moveTo(0, height);

            this.dataPoints.forEach((speed, index) => {
                const x = (width / (this.maxDataPoints - 1)) * index;
                const y = height - (speed / this.maxSpeed) * (height * 0.9);
                ctx.lineTo(x, y);
            });

            ctx.lineTo(width, height);
            ctx.closePath();
            ctx.fill();

            ctx.strokeStyle = '#3b82f6';
            ctx.lineWidth = 2;
            ctx.beginPath();

            this.dataPoints.forEach((speed, index) => {
                const x = (width / (this.maxDataPoints - 1)) * index;
                const y = height - (speed / this.maxSpeed) * (height * 0.9);

                if (index === 0) {
                    ctx.moveTo(x, y);
                } else {
                    ctx.lineTo(x, y);
                }
            });

            ctx.stroke();
        }
    }

    formatSpeed(bytes) {
        if (bytes === 0) return '0 B/s';
        const units = ['B/s', 'KB/s', 'MB/s', 'GB/s'];
        let size = bytes;
        let unitIndex = 0;

        while (size >= 1024 && unitIndex < units.length - 1) {
            size /= 1024;
            unitIndex++;
        }

        return `${size.toFixed(2)} ${units[unitIndex]}`;
    }
}

class FshareBridge {
    constructor() {
        this.downloads = [];
        this.sortColumn = null;
        this.sortDirection = 'asc';
        this.networkGraph = null;
        this.init();
    }

    init() {
        this.setupEventListeners();
        this.networkGraph = new NetworkGraph('network-graph');
        this.loadDashboardData();
        this.loadDownloads();
        this.loadDashboardData();
        this.loadDownloads();
        this.loadSystemLogs();
        this.initSidebar();

        // Auto-refresh stats every 1.5s for responsive UI
        setInterval(() => this.loadDashboardData(), 1500);
        setInterval(() => this.loadDownloads(), 5000);
        setInterval(() => this.loadSystemLogs(), 15000);

        // Update network graph every 1000ms
        setInterval(() => this.updateNetworkGraph(), 1000);

        // Update ETA countdown every second
        setInterval(() => this.updateETACountdown(), 1000);
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

        // Widget 1: Network Graph - handled by updateNetworkGraph()
        this.updateStatusIndicator('network-status-indicator', data.pyload.connected);

        // Widget 2: Downloader
        const pyloadConnected = data.pyload.connected;
        this.updateStatusIndicator('pyload-status-indicator', pyloadConnected);

        // Fshare Account Status from pyLoad
        const fshareStatus = data.pyload.fshare_account || {};
        const isPremium = fshareStatus.valid && fshareStatus.premium;
        this.updateBadge('fshare-account-status', isPremium, isPremium ? 'PREMIUM' : 'N/A');

        this.setText('active-downloads-count', String(data.pyload.active).padStart(2, '0'));
        this.setText('queue-count', data.pyload.total);

        // Widget 3: Search Engine
        this.updateStatusIndicator('timfshare-status-indicator', true);
        this.updateBadge('timfshare-status', true, 'ONLINE');
        this.setText('api-health', '100%');
        this.setText('api-ping', '45ms');
    }

    async updateNetworkGraph() {
        if (!this.networkGraph) return;

        try {
            const response = await fetch('/api/stats');
            const data = await response.json();

            if (data && data.pyload) {
                const activeDownloads = data.pyload.active || 0;
                const speedBytes = data.pyload.speed_bytes || 0;

                // Only update graph if there are active downloads
                if (activeDownloads > 0) {
                    this.networkGraph.addDataPoint(speedBytes);

                    // Update speed displays
                    const currentSpeed = this.networkGraph.formatSpeed(speedBytes);
                    const peakSpeed = this.networkGraph.formatSpeed(this.networkGraph.peakSpeed);

                    this.setText('current-speed', currentSpeed);
                    this.setText('peak-speed', peakSpeed);
                }
            }
        } catch (error) {
            console.error('Network graph update error:', error);
        }
    }

    updateETACountdown() {
        // Find all ETA elements in the downloads table
        const etaElements = document.querySelectorAll('.download-table .download-name + div');

        etaElements.forEach(el => {
            const text = el.textContent;
            const match = text.match(/ETA:\s*([\d:]+)/);

            if (match && match[1] !== '-') {
                const parts = match[1].split(':').map(Number);
                let totalSeconds = 0;

                if (parts.length === 3) {
                    totalSeconds = parts[0] * 3600 + parts[1] * 60 + parts[2];
                } else if (parts.length === 2) {
                    totalSeconds = parts[0] * 60 + parts[1];
                } else if (parts.length === 1) {
                    totalSeconds = parts[0];
                }

                if (totalSeconds > 0) {
                    totalSeconds--;
                    const hours = Math.floor(totalSeconds / 3600);
                    const minutes = Math.floor((totalSeconds % 3600) / 60);
                    const seconds = totalSeconds % 60;

                    const newETA = hours > 0
                        ? `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
                        : `${minutes}:${String(seconds).padStart(2, '0')}`;

                    el.innerHTML = el.innerHTML.replace(/ETA:\s*[\d:]+/, `ETA: ${newETA}`);
                }
            }
        });
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

    // Autocomplete for header search
    async handleAutocomplete(query) {
        if (!query || query.length < 2) {
            this.hideAutocomplete();
            return;
        }

        try {
            const response = await fetch(`/api/autocomplete?q=${encodeURIComponent(query)}`);
            const data = await response.json();

            if (data.suggestions && data.suggestions.length > 0) {
                this.showAutocomplete(data.suggestions.slice(0, 3)); // Top 3 only
            } else {
                this.hideAutocomplete();
            }
        } catch (error) {
            console.error('Autocomplete error:', error);
            this.hideAutocomplete();
        }
    }

    showAutocomplete(suggestions) {
        const dropdown = document.getElementById('autocomplete-dropdown');
        if (!dropdown) return;

        dropdown.innerHTML = suggestions.map(s => `
            <div class="autocomplete-item" data-query="${this.escapeHtml(s)}">
                ${this.escapeHtml(s)}
            </div>
        `).join('');

        dropdown.className = 'autocomplete-dropdown show';

        // Add click handlers
        dropdown.querySelectorAll('.autocomplete-item').forEach(item => {
            item.addEventListener('click', () => {
                const query = item.getAttribute('data-query');
                this.redirectToSearch(query);
            });
        });
    }

    hideAutocomplete() {
        const dropdown = document.getElementById('autocomplete-dropdown');
        if (dropdown) {
            dropdown.className = 'autocomplete-dropdown';
        }
    }

    redirectToSearch(query) {
        window.location.href = `/search?q=${encodeURIComponent(query)}`;
    }

    // Download Manager
    async loadDownloads() {
        try {
            const response = await fetch('/api/downloads');
            const data = await response.json();

            if (data.downloads) {
                this.downloads = data.downloads;

                // Re-apply sort if active
                if (this.sortColumn) {
                    const col = this.sortColumn;
                    const dir = this.sortDirection;
                    // Directly apply the sort without toggling
                    this.downloads.sort((a, b) => {
                        let aVal, bVal;
                        switch (col) {
                            case 'name': aVal = a.name.toLowerCase(); bVal = b.name.toLowerCase(); break;
                            case 'category': aVal = (a.category || '').toLowerCase(); bVal = (b.category || '').toLowerCase(); break;
                            case 'size': aVal = a.size_bytes || 0; bVal = b.size_bytes || 0; break;
                            case 'speed': aVal = a.speed_raw || 0; bVal = b.speed_raw || 0; break;
                            case 'eta': aVal = a.eta_seconds || 0; bVal = b.eta_seconds || 0; break;
                            case 'status': aVal = a.status.toLowerCase(); bVal = b.status.toLowerCase(); break;
                            case 'progress': aVal = a.progress || 0; bVal = b.progress || 0; break;
                            default: return 0;
                        }
                        if (aVal < bVal) return dir === 'asc' ? -1 : 1;
                        if (aVal > bVal) return dir === 'asc' ? 1 : -1;
                        return 0;
                    });
                    this.updateSortIcons(col);
                }

                // If we are on the dashboard, only show top 5
                const dashboardContainer = document.getElementById('download-manager-list');
                if (dashboardContainer) {
                    this.renderDashboardDownloads(this.downloads.slice(0, 3));
                }

                // If we are on the downloads page, show all and handle search/pagination
                const downloadsPageContainer = document.getElementById('downloads-full-list');
                if (downloadsPageContainer) {
                    const filtered = this.getFilteredDownloads();
                    this.updatePagination(filtered.length);
                    this.renderFullDownloads(this.getPagedDownloads(filtered));
                }
            }
        } catch (error) {
            console.error('Load downloads error:', error);
        }
    }

    getFilteredDownloads() {
        const query = (document.getElementById('downloads-search-input')?.value || '').toLowerCase();
        if (!query) return this.downloads;
        return this.downloads.filter(d => d.name.toLowerCase().includes(query));
    }

    getPagedDownloads(downloads) {
        if (!this.currentPage) this.currentPage = 1;
        if (!this.itemsPerPage) this.itemsPerPage = 6;

        const start = (this.currentPage - 1) * this.itemsPerPage;
        const end = start + this.itemsPerPage;
        return downloads.slice(start, end);
    }

    updatePagination(totalItems) {
        const info = document.getElementById('pagination-info');
        if (!info) return;

        const totalPages = Math.ceil(totalItems / this.itemsPerPage) || 1;
        if (this.currentPage > totalPages) this.currentPage = totalPages;

        const start = totalItems === 0 ? 0 : (this.currentPage - 1) * this.itemsPerPage + 1;
        const end = Math.min(this.currentPage * this.itemsPerPage, totalItems);

        info.textContent = `Showing ${start}-${end} of ${totalItems}`;

        const prevBtn = document.getElementById('prev-page');
        const nextBtn = document.getElementById('next-page');
        if (prevBtn) prevBtn.disabled = this.currentPage === 1;
        if (nextBtn) nextBtn.disabled = this.currentPage === totalPages;
    }

    changePage(delta) {
        this.currentPage += delta;
        this.loadDownloads();
    }

    renderDashboardDownloads(downloads) {
        const container = document.getElementById('download-manager-list');
        if (!container) return;

        if (downloads.length === 0) {
            container.innerHTML = `<tr><td colspan="6" style="text-align: center; padding: 2rem; color: var(--text-muted);">No active downloads</td></tr>`;
            return;
        }

        container.innerHTML = downloads.map(d => this.createDashboardDownloadRow(d)).join('');
    }

    renderFullDownloads(downloads) {
        const container = document.getElementById('downloads-full-list');
        if (!container) return;

        if (downloads.length === 0) {
            container.innerHTML = `<tr><td colspan="8" style="text-align: center; padding: 5rem; color: var(--text-muted);">No downloads found</td></tr>`;
            return;
        }

        container.innerHTML = downloads.map(d => this.createFullDownloadRow(d)).join('');
    }

    sortDownloads(column) {
        if (!this.downloads || this.downloads.length === 0) return;

        // Toggle sort direction
        if (this.sortColumn === column) {
            this.sortDirection = this.sortDirection === 'asc' ? 'desc' : 'asc';
        } else {
            this.sortColumn = column;
            this.sortDirection = 'asc';
        }

        // Apply sort
        this.downloads.sort((a, b) => {
            let aVal, bVal;
            switch (column) {
                case 'name': aVal = a.name.toLowerCase(); bVal = b.name.toLowerCase(); break;
                case 'category': aVal = (a.category || '').toLowerCase(); bVal = (b.category || '').toLowerCase(); break;
                case 'size': aVal = a.size_bytes || 0; bVal = b.size_bytes || 0; break;
                case 'speed': aVal = a.speed_raw || 0; bVal = b.speed_raw || 0; break;
                case 'eta': aVal = a.eta_seconds || 0; bVal = b.eta_seconds || 0; break;
                case 'status': aVal = a.status.toLowerCase(); bVal = b.status.toLowerCase(); break;
                case 'progress': aVal = a.progress || 0; bVal = b.progress || 0; break;
                default: return 0;
            }

            if (aVal < bVal) return this.sortDirection === 'asc' ? -1 : 1;
            if (aVal > bVal) return this.sortDirection === 'asc' ? 1 : -1;
            return 0;
        });

        this.updateSortIcons(column);

        // Immediate re-render
        if (document.getElementById('downloads-full-list')) {
            const filtered = this.getFilteredDownloads();
            this.updatePagination(filtered.length);
            this.renderFullDownloads(this.getPagedDownloads(filtered));
        }
        if (document.getElementById('download-manager-list')) {
            this.renderDashboardDownloads(this.downloads.slice(0, 3));
        }
    }

    updateSortIcons(column) {
        // Reset all icons
        document.querySelectorAll('.material-icons[id*="sort-icon"]').forEach(icon => {
            icon.textContent = 'swap_vert';
            icon.style.opacity = '0.5';
            icon.style.color = 'inherit';
        });

        // Set active icon
        const prefixes = ['sort-icon-', 'dash-sort-icon-'];
        prefixes.forEach(prefix => {
            const icon = document.getElementById(prefix + column);
            if (icon) {
                icon.textContent = this.sortDirection === 'asc' ? 'expand_less' : 'expand_more';
                icon.style.opacity = '1';
                icon.style.color = 'rgb(var(--accent-500))';
            }
        });
    }

    createDashboardDownloadRow(d) {
        const statusClass = d.status === 'Running' ? 'info' :
            d.status === 'Finished' ? 'success' :
                d.status === 'Stop' ? 'error' : 'warning';

        return `
            <tr>
                <td><div class="download-name">${this.escapeHtml(d.name)}</div></td>
                <td class="download-size">${d.size}</td>
                <td><span class="status-badge ${statusClass}">${d.status.toUpperCase()}</span></td>
                <td>${d.speed}</td>
                <td>
                    <div class="progress-bar" style="width: 80px;">
                        <div class="progress-fill" style="width: ${d.progress}%"></div>
                    </div>
                </td>
                <td style="text-align: right; padding-right: 1rem;">
                    <div class="download-controls" style="justify-content: flex-end;">
                        <button class="icon-btn" onclick="bridge.toggleDownload(${d.fid})">${d.status === 'Running' ? '<span class="material-icons" style="font-size: 18px">pause</span>' : '<span class="material-icons" style="font-size: 18px">play_arrow</span>'}</button>
                    </div>
                </td>
            </tr>
        `;
    }

    createFullDownloadRow(d) {
        const statusClass = d.status === 'Running' ? 'running' :
            d.status === 'Finished' ? 'success' :
                d.status === 'Stop' ? 'error' : 'warning';
        const catClass = (d.category || 'Uncategorized').toLowerCase();
        const catLabel = d.category || 'Uncategorized';

        return `
             <tr>
                <td><div class="download-name" title="${this.escapeHtml(d.name)}">${this.escapeHtml(d.name)}</div></td>
                <td><span class="category-badge cat-${catClass}">${catLabel}</span></td>
                <td>${d.size}</td>
                <td>
                    <div style="display: flex; align-items: center; gap: 0.75rem;">
                        <div class="progress-bar" style="flex: 1; min-width: 100px;">
                            <div class="progress-fill" style="width: ${d.progress}%"></div>
                        </div>
                        <span style="font-size: 0.8rem; font-weight: 600; min-width: 40px;">${d.progress}%</span>
                    </div>
                </td>
                <td><span class="status-badge ${statusClass}">${d.status.toUpperCase()}</span></td>
                <td>${d.speed}</td>
                <td>${d.eta}</td>
                <td style="text-align: right; padding-right: 1.5rem;">
                    <div class="download-controls" style="justify-content: flex-end;">
                        <button class="icon-btn" title="Toggle" onclick="bridge.toggleDownload(${d.fid})">
                            <span class="material-icons" style="font-size: 20px">${d.status === 'Running' ? 'pause' : 'play_arrow'}</span>
                        </button>
                        <button class="icon-btn delete-btn" title="Delete" onclick="bridge.deleteDownload(${d.fid})">
                            <span class="material-icons" style="font-size: 20px">delete_outline</span>
                        </button>
                    </div>
                </td>
            </tr>
        `;
    }

    // Global Action Logic
    async startAllDownloads() {
        if (confirm('Resume all paused downloads?')) {
            const resp = await fetch('/api/downloads/start_all', { method: 'POST' });
            if ((await resp.json()).success) this.loadDownloads();
        }
    }

    async pauseAllDownloads() {
        if (confirm('Pause all active downloads?')) {
            const resp = await fetch('/api/downloads/pause_all', { method: 'POST' });
            if ((await resp.json()).success) this.loadDownloads();
        }
    }

    async stopAllDownloads() {
        if (confirm('Stop all active downloads?')) {
            const resp = await fetch('/api/downloads/stop_all', { method: 'POST' });
            if ((await resp.json()).success) this.loadDownloads();
        }
    }

    createDownloadRow(d) {
        const statusClass = d.status === 'Running' ? 'info' :
            d.status === 'Finished' ? 'success' :
                d.status === 'Stop' ? 'error' : 'warning';

        const progressColor = d.status === 'Running' ? 'var(--accent-blue)' :
            d.status === 'Finished' ? 'var(--accent-green)' :
                'var(--text-muted)';

        const controlBtn = d.status === 'Running' ? '⏸' : '▶';
        const controlTitle = d.status === 'Running' ? 'Pause' : 'Resume';

        // Extract speed and ETA from info (format: "HH:MM:SS @speed")
        const infoMatch = d.info.match(/^([\d:]+)\s*@(.+)$/);
        const eta = infoMatch ? infoMatch[1] : '-';
        const speed = infoMatch ? infoMatch[2] : '-';

        return `
            <tr>
                <td>
                    <div class="download-name">${this.escapeHtml(d.name)}</div>
                    <div style="font-size: 0.7rem; color: var(--text-muted);">ETA: ${eta}</div>
                </td>
                <td class="download-size">${d.size}</td>
                <td><span class="status-badge ${statusClass}">${d.status.toUpperCase()}</span></td>
                <td style="color: var(--text-secondary); font-size: 0.875rem;">${speed}</td>
                <td>
                    <div class="download-progress">
                        <div class="progress-bar" style="width: 120px;">
                            <div class="progress-fill" style="width: ${d.progress}%; background: ${progressColor};"></div>
                        </div>
                        <span class="progress-text">${d.progress}%</span>
                    </div>
                </td>
                <td>
                    <div class="download-controls">
                        <button class="icon-btn" title="${controlTitle}" onclick="bridge.toggleDownload(${d.fid})">${controlBtn}</button>
                        <button class="icon-btn delete-btn" title="Delete" onclick="bridge.deleteDownload(${d.fid})">🗑</button>
                    </div>
                </td>
            </tr>
        `;
    }

    async toggleDownload(fid) {
        try {
            const response = await fetch(`/api/download/toggle/${fid}`, {
                method: 'POST'
            });

            const data = await response.json();
            if (data.success) {
                console.log('Download toggled:', fid);
                this.loadDownloads();
            } else {
                console.error('Failed to toggle download:', data.error);
            }
        } catch (error) {
            console.error('Toggle download error:', error);
        }
    }

    async deleteDownload(fid) {
        if (!confirm('Are you sure you want to delete this download?')) return;

        try {
            const response = await fetch(`/api/download/delete/${fid}`, {
                method: 'DELETE'
            });

            const data = await response.json();
            if (data.success) {
                console.log('Download deleted:', fid);
                this.loadDownloads();
            } else {
                console.error('Failed to delete download:', data.error);
            }
        } catch (error) {
            console.error('Delete download error:', error);
        }
    }

    // System Logs
    async loadSystemLogs() {
        const container = document.getElementById('system-log');
        if (!container) return;

        try {
            const response = await fetch('/api/logs');
            const data = await response.json();

            if (data.logs && data.logs.length > 0) {
                container.innerHTML = data.logs.map(log => `
                    <div class="log-entry ${log.level}">
                        <span class="log-time">${this.escapeHtml(log.time)}</span>
                        <span class="log-message">${this.escapeHtml(log.message)}</span>
                    </div>
                `).join('');
            } else {
                container.innerHTML = `
                    <div class="log-entry info">
                        <span class="log-time">[--:--:--]</span>
                        <span class="log-message">No recent logs available</span>
                    </div>
                `;
            }
        } catch (error) {
            console.error('Load logs error:', error);
        }
    }

    // Search Operations
    async search(query) {
        if (!query) return;

        if (window.location.pathname !== '/search') {
            this.redirectToSearch(query);
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
        // Header search with autocomplete
        const headerSearch = document.getElementById('header-search-input');
        if (headerSearch) {
            let autocompleteTimeout;

            headerSearch.addEventListener('input', (e) => {
                clearTimeout(autocompleteTimeout);
                autocompleteTimeout = setTimeout(() => {
                    this.handleAutocomplete(e.target.value);
                }, 300);
            });

            headerSearch.addEventListener('keypress', (e) => {
                if (e.key === 'Enter') {
                    this.hideAutocomplete();
                    this.redirectToSearch(e.target.value);
                }
            });

            // Hide autocomplete when clicking outside
            document.addEventListener('click', (e) => {
                if (!e.target.closest('.search-container')) {
                    this.hideAutocomplete();
                }
            });
        }

        // Downloads page local search
        const downloadsSearch = document.getElementById('downloads-search-input');
        if (downloadsSearch) {
            // Re-render on input is already handled in HTML with oninput="bridge.loadDownloads()"
        }

        // Sidebar Toggle via Button
        const sidebarToggle = document.getElementById('sidebar-toggle');
        if (sidebarToggle) {
            sidebarToggle.addEventListener('click', (e) => {
                e.stopPropagation(); // Prevent bubbling if needed
                this.toggleSidebar();
            });
        }

        // Sidebar Toggle via Brand (Logo)
        const sidebarBrand = document.getElementById('sidebar-brand');
        if (sidebarBrand) {
            sidebarBrand.addEventListener('click', () => this.toggleSidebar());
        }

        // Search page search input
        const searchInput = document.getElementById('search-input');
        if (searchInput) {
            searchInput.addEventListener('keypress', (e) => {
                if (e.key === 'Enter') this.search(searchInput.value);
            });
        }
    }

    initSidebar() {
        const sidebar = document.getElementById('sidebar');
        const isCollapsed = localStorage.getItem('sidebar-collapsed') === 'true';
        if (sidebar && isCollapsed) {
            sidebar.classList.add('collapsed');
        }
    }

    toggleSidebar() {
        const sidebar = document.getElementById('sidebar');
        if (sidebar) {
            sidebar.classList.toggle('collapsed');
            const isCollapsed = sidebar.classList.contains('collapsed');
            localStorage.setItem('sidebar-collapsed', isCollapsed);
        }
    }
    // Modal Handling
    showAddModal() {
        const modal = document.getElementById('add-link-modal');
        if (modal) {
            modal.style.display = 'flex';
            document.getElementById('manual-link-input').focus();
        }
    }

    hideAddModal() {
        const modal = document.getElementById('add-link-modal');
        if (modal) {
            modal.style.display = 'none';
            document.getElementById('manual-link-input').value = '';
            document.getElementById('modal-error-msg').style.display = 'none';
        }
    }

    async submitManualLink() {
        const input = document.getElementById('manual-link-input');
        const errorMsg = document.getElementById('modal-error-msg');
        const errorText = document.getElementById('error-text');
        const btn = document.getElementById('submit-link-btn');
        const url = input.value.trim();

        if (!url) return;

        // Validate Fshare file or folder link
        const isFshareFile = url.includes('fshare.vn/file/');
        const isFshareFolder = url.includes('fshare.vn/folder/');

        if (!isFshareFile && !isFshareFolder) {
            errorText.textContent = "Please enter a valid Fshare file or folder link.";
            errorMsg.style.display = 'block';
            return;
        }

        try {
            btn.disabled = true;
            btn.textContent = "ADDING...";

            const response = await fetch('/api/download', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ url: url, name: "Manual Upload" })
            });

            const result = await response.json();
            if (result.success) {
                this.hideAddModal();
                this.loadDownloads();
            } else {
                errorText.textContent = result.error || "Failed to add download.";
                errorMsg.style.display = 'block';
            }
        } catch (e) {
            errorText.textContent = "Network error. Try again.";
            errorMsg.style.display = 'block';
        } finally {
            btn.disabled = false;
            btn.textContent = "ADD TO QUEUE";
        }
    }

    // Global Action Wrappers
    async startAllDownloads() {
        await fetch('/api/downloads/start_all', { method: 'POST' });
        this.loadDownloads();
    }

    async pauseAllDownloads() {
        await fetch('/api/downloads/pause_all', { method: 'POST' });
        this.loadDownloads();
    }

    async stopAllDownloads() {
        if (confirm("Are you sure you want to stop all active downloads?")) {
            await fetch('/api/downloads/stop_all', { method: 'POST' });
            this.loadDownloads();
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
