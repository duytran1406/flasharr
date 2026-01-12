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
        // Singleton check
        if (window.fshareBridgeInstance) {
            return window.fshareBridgeInstance;
        }
        window.fshareBridgeInstance = this;
        window.bridge = this; // Explicit global access for SPA hooks

        this.downloads = [];
        this.stats = null;
        this.statsListeners = {};
        this.downloadsListeners = {}; // New listener group for download list
        this.isPolling = false;       // Track polling state
        this.sortColumn = null;
        this.sortDirection = 'asc';
        this.networkGraph = null;
        this.networkGraphActive = false;
        this.init();
    }

    init() {
        this.setupEventListeners();

        // Restore from storage so UI isn't empty on load
        const storedStats = localStorage.getItem('fshare_stats');
        if (storedStats) {
            try { this.stats = JSON.parse(storedStats); } catch (e) { }
        }

        const storedDownloads = localStorage.getItem('fshare_downloads');
        if (storedDownloads) {
            try { this.downloads = JSON.parse(storedDownloads); } catch (e) { }
        }

        this.wakeupDashboardChart();
        this.initSidebar();

        // Immediately render from storage
        this.updateDashboard();

        // Initial fetch to populate data in background
        this.runFullPollCheck();

        // Update ETA countdown every second (local UI update, no API)
        setInterval(() => this.updateETACountdown(), 1000);
    }

    // Called when navigating BACK to dashboard via SPA
    wakeupDashboard() {
        console.log('Wakeup Dashboard (SPA)');
        // Refresh from storage immediately
        this.updateDashboard();
        this.applySorting();
        this.wakeupDashboardChart();

        // Ensure polling logic is checked
        this.runFullPollCheck();
    }

    // New centralized method to start polling
    startConditionalPolling() {
        if (this.isPolling) return;

        console.log('🚀 Starting conditional polling loop...');
        this.isPolling = true;
        this.poll();
    }

    async poll() {
        if (!this.isPolling) return;

        try {
            // Ping-Pong: Wait for results before next request
            // 1. Fetch Stats - REMOVED (Calculated from downloads)

            // 2. Fetch Downloads
            const hasActiveDownloads = await this.fetchDownloads();

            // 3. Conditional Loop Logic: Ping again if active
            if (hasActiveDownloads && this.isPolling) {
                // Short delay to avoid CPU spike, then continue ping-pong
                setTimeout(() => this.poll(), 1000);
            } else {
                console.log('💤 No active downloads or polling stopped. Polling paused.');
                this.isPolling = false;
            }
        } catch (error) {
            console.error('Polling error (Connection Lost?):', error);

            // Mark items as STOPPED if connection is lost
            this.handleConnectionLost();

            // Retry on error after long delay
            setTimeout(() => this.poll(), 5000);
        }
    }

    handleConnectionLost() {
        console.warn('⚠️ Connection lost to backend. Marking active downloads as STOPPED.');
        if (this.downloads) {
            this.downloads = this.downloads.map(d => {
                const status = d.status.toLowerCase();
                if (status === 'running' || status === 'downloading') {
                    return { ...d, status: 'STOPPED' };
                }
                return d;
            });
            // Update UI
            Object.values(this.downloadsListeners).forEach(listener => listener());
        }
    }

    // Helper: Single run wrapper for manual actions/init
    async runFullPollCheck() {
        try {
            // await this.fetchStats(); // REMOVED
            const hasActive = await this.fetchDownloads();

            // If we found active downloads during this manual check, ensure polling is running
            if (hasActive) {
                this.startConditionalPolling();
            }
        } catch (e) {
            console.error('Initial check failed:', e);
        }
    }

    async fetchStats() {
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 5000);

        try {
            const response = await fetch('/api/stats', { signal: controller.signal });
            clearTimeout(timeoutId);
            const data = await response.json();

            if (data) {
                // Save to LocalStorage
                localStorage.setItem('fshare_stats', JSON.stringify(data));
                this.stats = data; // Keep memory copy active for simple access

                // Notify listeners
                Object.values(this.statsListeners).forEach(listener => listener());

                // Update internal dashboard & graph
                this.updateDashboard();
                this.updateNetworkGraph();
            }
        } catch (e) {
            clearTimeout(timeoutId);
            console.error('Fetch stats failed:', e.name === 'AbortError' ? 'Timeout (5s)' : e);
            throw e;
        }
    }

    async fetchDownloads() {
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 5000);

        try {
            const response = await fetch('/api/downloads', { signal: controller.signal });
            clearTimeout(timeoutId);
            const data = await response.json();

            if (data.downloads) {
                // Save to LocalStorage
                localStorage.setItem('fshare_downloads', JSON.stringify(data.downloads));

                // Track previous download count and statuses for change detection
                const prevDownloads = this.downloads || [];
                const prevCount = prevDownloads.length;
                const prevStatuses = new Set(prevDownloads.map(d => `${d.fid}:${d.status}`));

                // DATA NORMALIZATION: Map API structure to UI flat structure
                this.downloads = data.downloads.map(d => ({
                    fid: d.id,
                    name: d.filename,
                    status: d.state,
                    progress: parseFloat(d.progress || 0).toFixed(1),
                    size: d.size.formatted_total,
                    size_bytes: d.size.total,
                    speed: d.speed.formatted,
                    speed_raw: d.speed.bytes_per_sec,
                    eta: d.eta.formatted,
                    eta_seconds: d.eta.seconds,
                    category: d.category || 'Uncategorized',
                    error_message: d.error_message || '',
                    info: ""
                }));


                // Calculate total speed from items for graph
                const totalSpeedBytes = this.downloads.reduce((acc, d) => acc + (d.speed_raw || 0), 0);
                const totalSpeedFormatted = this.formatSpeed(totalSpeedBytes);

                // Notify stats listeners with computed data (avoids extra API call and inconsistencies)
                const computedStats = {
                    fshare_downloader: {
                        speed_bytes: totalSpeedBytes,
                        speed: totalSpeedFormatted
                    }
                };

                Object.values(this.statsListeners).forEach(listener => listener(computedStats));

                // Process data (Sort)
                this.applySorting();

                // Notify UI
                Object.values(this.downloadsListeners).forEach(listener => listener());

                // Check for active status to continue polling
                return this.downloads.some(d => {
                    const status = (d.status || '').toLowerCase();
                    return status === 'running' || status === 'downloading' || status === 'starting';
                });
            }
        } catch (e) {
            clearTimeout(timeoutId);
            console.error('Fetch downloads failed:', e.name === 'AbortError' ? 'Timeout (5s)' : e);
            throw e;
        }
        return false;
    }

    // Listener registration
    onDownloads(name, callback) {
        this.downloadsListeners[name] = callback;
        // If we have data, trigger immediately
        const stored = localStorage.getItem('fshare_downloads');
        if (stored) callback();
    }

    // Subscribe to stats updates
    onStats(name, callback) {
        this.statsListeners[name] = callback;
        // If we have data, trigger immediately
        const stored = localStorage.getItem('fshare_stats');
        if (stored) callback();
    }

    wakeupDashboardChart() {
        const canvas = document.getElementById('network-graph');
        if (canvas) {
            // Detection for SPA navigation: if the instance exists but points to a different/detached canvas, recreate it
            if (this.networkGraph && (this.networkGraph.canvas !== canvas || !document.body.contains(this.networkGraph.canvas))) {
                console.log('Re-initializing Network Graph (Canvas changed)');
                this.networkGraph = null;
            }

            if (!this.networkGraph) {
                this.networkGraph = new NetworkGraph('network-graph');
            }
            this.networkGraphActive = true;

            // Immediately attempt update from storage
            if (localStorage.getItem('fshare_stats')) {
                this.updateNetworkGraph();
            }
        } else {
            this.networkGraphActive = false;
        }
    }

    // Dashboard Data & Stats
    updateDashboard() {
        const stored = localStorage.getItem('fshare_stats');
        if (!stored) return;

        try {
            const data = JSON.parse(stored);
            const fd = data.fshare_downloader || {};

            // Header stats - removed system.speedtest and system.uptime dependencies
            this.setText('header-active', fd.active || 0);

            // Widget 1: Network Graph - handled by updateNetworkGraph()
            this.updateStatusIndicator('network-status-indicator', fd.connected);

            // Widget 2: Downloader
            this.updateStatusIndicator('pyload-status-indicator', fd.connected);

            // Fshare Account Status
            const primary = fd.primary_account || {};
            const isPremium = primary.valid && primary.premium;
            this.updateBadge('fshare-account-status', isPremium, isPremium ? 'PREMIUM' : 'N/A');

            this.setText('active-downloads-count', String(fd.active || 0).padStart(2, '0'));
            this.setText('queue-count', fd.total || 0);

            // Widget 3: Search Engine
            this.updateStatusIndicator('timfshare-status-indicator', true);
            this.updateBadge('timfshare-status', true, 'ONLINE');
            this.setText('api-health', '100%');
            this.setText('api-ping', '45ms');
        } catch (e) {
            console.error('Error parsing dashboard stats from storage', e);
        }
    }

    updateNetworkGraph() {
        // Try to wake up chart if not active
        if (!this.networkGraphActive) {
            this.wakeupDashboardChart();
        }

        if (!this.networkGraph || !this.networkGraphActive) return;

        const stored = localStorage.getItem('fshare_stats');
        if (!stored) return;

        try {
            const data = JSON.parse(stored);
            if (data.fshare_downloader) {
                const speedBytes = data.fshare_downloader.speed_bytes || 0;

                // Always update graph to show current state (even if 0)
                this.networkGraph.addDataPoint(speedBytes);

                // Update speed displays
                const currentSpeed = this.networkGraph.formatSpeed(speedBytes);
                // Only update peak if > 0 or keep previous
                const peakSpeed = this.networkGraph.formatSpeed(this.networkGraph.peakSpeed);

                this.setText('current-speed', currentSpeed);
                this.setText('peak-speed', peakSpeed);
            }
        } catch (e) {
            console.error('Error parsing network graph stats from storage', e);
        }
    }

    updateETACountdown() {
        // Find all ETA cells using the specific class
        const etaElements = document.querySelectorAll('.eta-cell');

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

    // Download Manager - Legacy loadDownloads method replaced by fetchDownloads
    async loadDownloads() {
        // Wrapper for compatibility with old calls, but routed through new system
        await this.runFullPollCheck();
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

    // Sorting helper extracted from old loadDownloads
    applySorting() {
        if (!this.downloads) return;

        this.downloads.sort((a, b) => {
            const aIncomplete = a.progress < 100;
            const bIncomplete = b.progress < 100;

            if (aIncomplete && !bIncomplete) return -1;
            if (!aIncomplete && bIncomplete) return 1;

            if (aIncomplete && bIncomplete) {
                return a.progress - b.progress;
            }

            if (this.sortColumn) {
                let aVal, bVal;
                switch (this.sortColumn) {
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
            }
            return 0;
        });
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
        // Normalizes status casing just in case
        const status = (d.status || '').toLowerCase();
        const statusClass = status === 'running' || status === 'downloading' ? 'info' :
            status === 'finished' || status === 'completed' ? 'success' :
                status === 'stop' || status === 'paused' || status === 'failed' ? 'error' : 'warning';

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
                        <button class="icon-btn" onclick="bridge.toggleDownload('${d.fid}')">${(d.status === 'Running' || d.status === 'Downloading') ? '<span class="material-icons" style="font-size: 18px">pause</span>' : '<span class="material-icons" style="font-size: 18px">play_arrow</span>'}</button>
                    </div>
                </td>
            </tr>
        `;
    }

    createFullDownloadRow(d) {
        const statusClass = (d.status === 'Running' || d.status === 'Downloading') ? 'running' :
            (d.status === 'Finished' || d.status === 'Completed') ? 'success' :
                (d.status === 'Stop' || d.status === 'Paused' || d.status === 'Failed') ? 'error' : 'warning';
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
                <td>
                    <div style="display: flex; align-items: center; gap: 0.5rem;">
                        <span class="status-badge ${statusClass}">${d.status.toUpperCase()}</span>
                        ${d.status.toLowerCase() === 'failed' && d.error_message ? `
                        <button class="icon-btn" onclick="bridge.showError('${this.escapeHtml(d.error_message)}')" title="View Error" style="color: #ef4444;">
                            <span class="material-icons" style="font-size: 18px;">error</span>
                        </button>` : ''}
                    </div>
                </td>
                <td>${d.speed}</td>
                <td class="eta-cell">${d.eta}</td>
                <td style="text-align: right; padding-right: 1.5rem;">
                    <div class="download-controls" style="justify-content: flex-end;">
                        <button class="icon-btn" title="Toggle" onclick="bridge.toggleDownload('${d.fid}')">
                            <span class="material-icons" style="font-size: 20px">${(d.status === 'Running' || d.status === 'Downloading') ? 'pause' : 'play_arrow'}</span>
                        </button>
                        <button class="icon-btn delete-btn" title="Delete" onclick="bridge.deleteDownload('${d.fid}')">
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
            if ((await resp.json()).success) await this.runFullPollCheck();
        }
    }

    async pauseAllDownloads() {
        if (confirm('Pause all active downloads?')) {
            const resp = await fetch('/api/downloads/pause_all', { method: 'POST' });
            if ((await resp.json()).success) await this.runFullPollCheck();
        }
    }

    async stopAllDownloads() {
        if (confirm('Stop all active downloads?')) {
            const resp = await fetch('/api/downloads/stop_all', { method: 'POST' });
            if ((await resp.json()).success) await this.runFullPollCheck();
        }
    }

    showError(msg) {
        const modal = document.getElementById('error-modal');
        const text = document.getElementById('error-modal-text');
        if (modal && text) {
            text.textContent = msg;
            modal.style.display = 'flex';
        } else {
            alert(msg);
        }
    }

    closeErrorModal() {
        const modal = document.getElementById('error-modal');
        if (modal) modal.style.display = 'none';
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
                await this.runFullPollCheck();
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
                await this.runFullPollCheck();
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
        const metadata = result.metadata || this.parseMetadata(name);
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
                        ${metadata.viet_sub ? `<span class="status-badge success" style="background: rgba(16, 185, 129, 0.2); color: #10b981; border: 1px solid rgba(16, 185, 129, 0.3);">VIET SUB</span>` : ''}
                        ${metadata.viet_dub ? `<span class="status-badge success" style="background: rgba(245, 158, 11, 0.2); color: #f59e0b; border: 1px solid rgba(245, 158, 11, 0.3);">VIET DUB</span>` : ''}
                    </div>
                    <button class="btn-primary" style="width: 100%; justify-content: center;" 
                            onclick="bridge.download(event, '${this.escapeHtml(url)}', '${this.escapeHtml(name)}')">
                        Download
                    </button>
                </div>
            </div>
        `;
    }

    parseMetadata(filename) {
        // Fallback local parser for UI-only elements if needed
        const metadata = { resolution: null, viet_sub: false, viet_dub: false };
        if (filename.match(/2160p|4K|UHD/i)) metadata.resolution = '4K';
        else if (filename.match(/1080p/i)) metadata.resolution = '1080p';
        else if (filename.match(/720p/i)) metadata.resolution = '720p';

        const vietSubMarkers = /vietsub|viet\.sub|vie\.sub|phụ đề|phu de/i;
        const vietDubMarkers = /thuyết minh|thuyet minh|viet\.dub|vie\.dub|lồng tiếng|long tieng|tvp|tmpđ/i;

        if (filename.match(vietSubMarkers)) metadata.viet_sub = true;
        if (filename.match(vietDubMarkers)) metadata.viet_dub = true;

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
    async download(event, url, name) {
        let btn = null;
        let originalHtml = '';

        if (event && event.currentTarget) {
            btn = event.currentTarget;
        } else if (event && event.target) {
            btn = event.target.closest('button');
        }

        if (btn) {
            btn.disabled = true;
            originalHtml = btn.innerHTML;
            btn.innerHTML = `<span class="material-icons spin" style="font-size: 1.2rem;">sync</span> Adding...`;
        }

        console.log(`[FshareBridge] Initiating download for: ${name}`);
        console.log(`[FshareBridge] Fshare URL: ${url}`);

        try {
            const response = await fetch('/api/download', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ url, name })
            });

            const data = await response.json();

            if (data.success) {
                // Keep button disabled if successful to prevent double clicks, but update text
                if (btn) btn.innerHTML = `✅ Added`;
                alert(`✅ Added to queue: ${data.normalized}`);
                await this.runFullPollCheck();
            } else {
                if (btn) {
                    btn.disabled = false;
                    btn.innerHTML = originalHtml;
                }
                alert('❌ Failed to add download: ' + (data.error || 'Unknown error'));
            }
        } catch (error) {
            console.error('Download error:', error);
            if (btn) {
                btn.disabled = false;
                btn.innerHTML = originalHtml;
            }
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
    // Helper: Format Bytes/Speed
    formatSpeed(bytes) {
        if (bytes === 0) return '0 B/s';
        const k = 1024;
        const sizes = ['B/s', 'KB/s', 'MB/s', 'GB/s', 'TB/s'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
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
        await this.runFullPollCheck();
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
    // Prevent re-initialization
    if (window.fshareBridgeInstance) {
        bridge = window.fshareBridgeInstance;
        console.log('Bridge already initialized, skipping recreation.');
        return;
    }

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
