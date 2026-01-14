// Flasharr - Frontend JavaScript
// NEXUS Dashboard v2.0

// Network Graph Class
if (typeof window.NetworkGraph === 'undefined') {
    window.NetworkGraph = class NetworkGraph {
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
}

if (typeof window.FshareBridge === 'undefined') {
    window.FshareBridge = class FshareBridge {
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
            this.downloadsListeners = {};
            this.charts = {}; // Store chart instances
            this.chartData = {
                speed: { labels: [], down: [], up: [] },
                storage: { used: 0, free: 0 }
            };
            this.sortColumn = 'added';
            this.sortDirection = 'desc';
            this.networkGraph = null;
            this.networkGraphActive = false;
            this.selectedDownloads = new Set();
            this.filters = { category: 'all', status: 'all' };
            this.lastSearchResults = [];
            this.lastSearchQuery = '';

            // Persist logs
            this.systemLogs = [];
            try {
                const storedLogs = localStorage.getItem('fshare_system_logs');
                if (storedLogs) this.systemLogs = JSON.parse(storedLogs);
            } catch (e) { }

            this.searchViewMode = localStorage.getItem('search_view_mode') || 'grid';

            // WebSocket Client
            this.ws = new FshareWebSocketClient();
            this.ws_ready = false;
            this.global_listeners_ready = false;

            this.init();
        }

        init() {
            this.setupEventListeners();
            this.initTheme();
            this.initContextMenu();

            // Restore from storage so UI isn't empty on load
            const storedStats = localStorage.getItem('fshare_stats');
            if (storedStats) {
                try { this.stats = JSON.parse(storedStats); } catch (e) { }
            }

            const storedResults = localStorage.getItem('fshare_last_search_results');
            if (storedResults) {
                try { this.lastSearchResults = JSON.parse(storedResults); } catch (e) { }
            }
            this.lastSearchQuery = localStorage.getItem('fshare_last_search_query') || '';

            // Try to load downloads from storage if available to avoid empty flicker
            const storedDownloads = localStorage.getItem('fshare_downloads');
            if (storedDownloads) {
                try {
                    this.downloads = JSON.parse(storedDownloads);
                    console.log('📦 Loaded', this.downloads.length, 'downloads from storage');
                } catch (e) { }
            }

            this.wakeupDashboardChart();
            this.initSidebar();

            // Immediately render from storage
            this.updateDashboard();

            // Initial fetch to populate data in background
            this.initialLoad();

            // Connect WebSocket (ensures only one set of handlers)
            this.setupWebSocket();

            // Re-bind downloads list if visible
            if (document.getElementById('downloads-full-list') || document.getElementById('download-manager-list')) {
                this.notifyDownloadsChanged();
            }

            // Load logs immediately from cache if available
            if (document.getElementById('system-log')) {
                if (this.systemLogs.length > 0) {
                    this.renderSystemLogs();
                }
                this.loadSystemLogs();
                // Pull logs every 5 seconds
                if (window.logInterval) clearInterval(window.logInterval);
                window.logInterval = setInterval(() => this.loadSystemLogs(), 5000);
            }

            // Check for search query param
            const params = new URLSearchParams(window.location.search);
            const q = params.get('q');
            if (q) {
                const input = document.getElementById('search-input');
                if (input) input.value = q;
                this.search(q);
            }

            // Update ETA countdown every second (local UI update, no API)
            // Clear existing interval if any (SPA)
            if (window.etaInterval) clearInterval(window.etaInterval);
            window.etaInterval = setInterval(() => this.updateETACountdown(), 1000);
        }

        setupWebSocket() {
            if (this.ws_ready) return;
            this.ws_ready = true;

            console.log('🔌 setting up WebSocket logic (once)...');

            // Define event handlers
            this.ws.on('connected', () => {
                console.log('✅ WebSocket Connected! Subscribing to events...');
                this.ws.subscribe(['ta', 'tu', 'tr', 'es', 'as', 'st', 'sy']);
                this.updateStatusIndicator('network-status-indicator', true);
            });

            this.ws.on('disconnected', () => {
                console.log('❌ WebSocket Disconnected');
                this.updateStatusIndicator('network-status-indicator', false);
                this.handleConnectionLost();
            });

            // Task Added/Updated
            this.ws.on('task_added', (task) => this.handleTaskUpdate(task));
            this.ws.on('task_update', (task) => this.handleTaskUpdate(task));

            // Task Removed
            this.ws.on('task_removed', (data) => {
                console.log('🗑 Task Removed:', data.taskId);
                this.downloads = this.downloads.filter(d => d.fid !== data.taskId);
                this.notifyDownloadsChanged();
            });

            // Engine Stats (Queue, Speed)
            this.ws.on('engine_stats', (stats) => {
                this.updateStatsFromWS(stats);
            });

            // Account Status
            this.ws.on('account_status', (status) => {
                // console.log('👤 Account Status:', status);
                this.updateAccountStatus(status);
            });

            // Full Sync
            this.ws.on('sync_all', (tasks) => {
                console.log('🔄 Received Full Sync:', tasks.length, 'items');
                this.handleFullSync(tasks);
            });

            // Connect if not already
            if (!this.ws.connected) {
                this.ws.connect();
            }
        }

        async initialLoad() {
            // One-time fetch to get initial state before WS takes over
            await Promise.all([
                this.fetchDownloads(),
                this.fetchStats(),
                this.checkAccountHealth()
            ]);
        }

        async checkAccountHealth() {
            try {
                // Only check if we haven't recently (e.g., last 5 mins) to optimize
                const lastCheck = localStorage.getItem('last_account_check');
                const now = Date.now();
                if (lastCheck && (now - parseInt(lastCheck)) < 300000) return;

                const response = await fetch('/api/verify-account', { method: 'POST' });
                const data = await response.json();

                localStorage.setItem('last_account_check', now);

                if (data.status === 'error' && data.message !== 'No account configured') {
                    this.showAccountErrorModal(data.message);
                    this.updateAccountStatus({ a: false, type: 'Error' });
                } else if (data.status === 'ok') {
                    // Force refresh visuals
                    if (data.account) {
                        this.updateAccountStatus({
                            a: data.account.available,
                            premium: data.account.premium,
                            type: data.account.account_type,
                            traffic: data.account.traffic_left
                        });
                        this.setText('fshare-daily-quota', data.account.traffic_left || '-- / --');

                        // Update stats object to persist this data
                        if (!this.stats.fshare_downloader) this.stats.fshare_downloader = {};
                        if (!this.stats.fshare_downloader.primary_account) this.stats.fshare_downloader.primary_account = {};

                        this.stats.fshare_downloader.primary_account.premium = data.account.premium;
                        this.stats.fshare_downloader.primary_account.traffic_left = data.account.traffic_left;
                        this.stats.fshare_downloader.primary_account.valid = data.account.available;

                        localStorage.setItem('fshare_stats', JSON.stringify(this.stats));
                    }
                }
            } catch (e) {
                console.error('Account check failed:', e);
            }
        }

        showAccountErrorModal(msg) {
            const modalHTML = `
            <div id="account-error-modal" class="modal-overlay" style="position: fixed; top: 0; left: 0; width: 100%; height: 100%; background: rgba(0,0,0,0.85); z-index: 99999; display: flex; justify-content: center; align-items: center; animation: fadeIn 0.3s ease;">
                <div class="service-card" style="width: 100%; max-width: 480px; border-color: #ef4444; box-shadow: 0 0 50px rgba(239, 68, 68, 0.2);">
                    <div class="service-header" style="margin-bottom: 1.5rem;">
                        <div class="service-icon" style="background: rgba(239, 68, 68, 0.1); color: #ef4444;">
                            <span class="material-icons">no_accounts</span>
                        </div>
                        <div class="service-title" style="color: #ef4444;">Account Alert</div>
                    </div>
                    <div style="margin-bottom: 2rem; color: var(--text-secondary); line-height: 1.6;">
                        <h3 style="color: white; margin-bottom: 0.5rem; font-size: 1.1rem;">Fshare Connection Failed</h3>
                        <p>${msg}</p>
                        <p style="margin-top: 1rem; font-size: 0.9rem; background: rgba(255,255,255,0.05); padding: 1rem; border-radius: 8px;">
                            Please check your credentials in <a href="/settings" style="color: #3b82f6; text-decoration: none; font-weight: 600;">Settings</a>.
                        </p>
                    </div>
                    <div style="display: flex; gap: 1rem; justify-content: flex-end;">
                        <button onclick="document.getElementById('account-error-modal').remove()" class="btn-primary" style="background: transparent; border: 1px solid rgba(255,255,255,0.1);">DISMISS</button>
                        <button onclick="window.location.href='/settings'" class="btn-primary" style="background: #3b82f6;">GO TO SETTINGS</button>
                    </div>
                </div>
            </div>`;

            // Remove existing if any
            const existing = document.getElementById('account-error-modal');
            if (existing) existing.remove();

            document.body.insertAdjacentHTML('beforeend', modalHTML);
        }

        // Unified Task Update Handler
        handleTaskUpdate(taskData) {
            const existingIndex = this.downloads.findIndex(d => d.fid === taskData.i);
            const normalized = this.normalizeTaskData(taskData);

            if (existingIndex > -1) {
                this.downloads[existingIndex] = normalized;
            } else {
                this.downloads.push(normalized);
            }

            this.notifyDownloadsChanged();
        }

        handleFullSync(tasks) {
            // Completely replace local state
            console.log('🔄 Applying Full Sync:', tasks.length, 'items');
            this.downloads = tasks.map(t => this.normalizeTaskData(t));
            this.notifyDownloadsChanged();
        }

        notifyDownloadsChanged() {
            // Save to Storage
            localStorage.setItem('fshare_downloads', JSON.stringify(this.downloads));

            // Sort
            this.applySorting();
            if (this.sortColumn) this.updateSortIcons(this.sortColumn);

            // Update Listeners (UI)
            Object.values(this.downloadsListeners).forEach(listener => listener());

            // Check if we need to render specific views if they exist in DOM
            if (document.getElementById('download-manager-list')) {
                this.renderDashboardDownloads(this.downloads.slice(0, 3));
            }
            if (document.getElementById('downloads-full-list')) {
                const filtered = this.getFilteredDownloads();
                this.updatePagination(filtered.length);
                this.renderFullDownloads(this.getPagedDownloads(filtered));
            }
        }

        updateStatsFromWS(stats) {
            const data = {
                fshare_downloader: {
                    active: stats.a,
                    total: stats.q,
                    connected: true,
                    speed_bytes: stats.sp || 0
                }
            };

            const totalSpeedBytes = data.fshare_downloader.speed_bytes || this.downloads.reduce((acc, d) => acc + (d.speed_raw || 0), 0);
            const totalSpeedFormatted = this.formatSpeed(totalSpeedBytes);

            data.fshare_downloader.speed_bytes = totalSpeedBytes;
            data.fshare_downloader.speed = totalSpeedFormatted;

            // Update storage/mem
            this.stats = { ...this.stats, ...data };
            localStorage.setItem('fshare_stats', JSON.stringify(this.stats));

            // Update UI
            Object.values(this.statsListeners).forEach(listener => listener(this.stats));
            this.updateDashboard();
            this.updateNetworkGraph();

            // Update Downloads Footer if exists
            const footerSpeed = document.getElementById('footer-current-speed');
            const footerQueue = document.getElementById('footer-queue-count');
            if (footerSpeed) footerSpeed.textContent = totalSpeedFormatted;
            if (footerQueue) footerQueue.textContent = stats.q || 0;
        }

        updateAccountStatus(status) {
            if (!this.stats) this.stats = {};
            if (!this.stats.fshare_downloader) this.stats.fshare_downloader = {};

            const isPremium = status.premium || status.type === 'Premium' || status.type === 'VIP';
            this.stats.fshare_downloader.primary_account = {
                valid: status.a,
                premium: isPremium,
                type: status.type
            };

            this.updateDashboard();

            // Update Downloads Footer Account if exists
            const dot = document.getElementById('footer-account-dot');
            const text = document.getElementById('footer-account-status');
            if (dot && text) {
                dot.style.background = isPremium ? '#22c55e' : '#ef4444';
                dot.style.boxShadow = isPremium ? '0 0 10px rgba(34, 197, 94, 0.4)' : '0 0 10px rgba(239, 68, 68, 0.4)';
                text.textContent = (isPremium ? 'PREMIUM' : 'FREE') + (status.a ? '' : ' (INVALID)');
            }
        }

        normalizeTaskData(d) {
            const status = (d.state || d.s || d.status || '').toLowerCase();
            const isRunning = status === 'running' || status === 'downloading' || status === 'starting' || status === 'extracting';

            return {
                fid: d.id || d.i,
                name: d.filename || d.n || d.name || 'Unknown',
                status: d.state || d.s || d.status || 'Unknown',
                progress: parseFloat(d.progress || d.p || 0).toFixed(1),
                size: d.size?.formatted_total || d.size_formatted || (d.t ? this.formatSpeed(d.t).replace('/s', '') : '0 B'),
                size_bytes: d.size?.total || d.t || 0,

                speed: isRunning ? (d.speed?.formatted || d.sp_formatted || this.formatSpeed(d.sp || 0)) : '0 B/s',
                speed_raw: isRunning ? (d.speed?.bytes_per_sec || d.sp || 0) : 0,
                eta: isRunning ? (d.eta?.formatted || d.e_formatted || this.formatTime(d.e || 0)) : '-',
                eta_seconds: isRunning ? (d.eta?.seconds || d.e || 0) : 0,

                category: d.category || d.c || 'Uncategorized',
                error_message: d.error_message || d.er || '',
                added: d.added || d.created_at || d.a || null,
                info: d.url || d.u || ""
            };
        }

        formatAddedDate(dateStr) {
            if (!dateStr) return '-';
            try {
                const date = new Date(dateStr);
                const day = String(date.getDate()).padStart(2, '0');
                const month = String(date.getMonth() + 1).padStart(2, '0');
                const year = date.getFullYear();
                const hours = String(date.getHours()).padStart(2, '0');
                const minutes = String(date.getMinutes()).padStart(2, '0');
                return `${day}-${month}-${year} ${hours}:${minutes}`;
            } catch (e) { return '-'; }
        }

        formatTime(seconds) {
            if (seconds === null || seconds === undefined) return '--:--:--';
            const h = Math.floor(seconds / 3600);
            const m = Math.floor((seconds % 3600) / 60);
            const s = Math.floor(seconds % 60);
            return `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
        }

        handleConnectionLost() {
            console.warn('⚠️ Connection lost. Marking active downloads as STOPPED.');
            if (this.downloads) {
                this.downloads = this.downloads.map(d => {
                    const status = (d.status || '').toLowerCase();
                    if (status === 'running' || status === 'downloading') {
                        return { ...d, status: 'STOPPED' };
                    }
                    return d;
                });
                this.notifyDownloadsChanged();
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
                    this.downloads = data.downloads.map(d => this.normalizeTaskData(d));
                    this.notifyDownloadsChanged();
                    return true;
                }
            } catch (e) {
                clearTimeout(timeoutId);
                console.error('Fetch downloads failed:', e);
            }
            return false;
        }

        onDownloads(name, callback) {
            this.downloadsListeners[name] = callback;
            if (this.downloads.length > 0) callback();
        }

        onStats(name, callback) {
            this.statsListeners[name] = callback;
            if (this.stats) callback();
        }

        wakeupDashboardChart() {
            const canvas = document.getElementById('network-graph');
            if (canvas) {
                if (this.networkGraph && (this.networkGraph.canvas !== canvas || !document.body.contains(this.networkGraph.canvas))) {
                    this.networkGraph = null;
                }
                if (!this.networkGraph) {
                    this.networkGraph = new NetworkGraph('network-graph');
                }
                this.networkGraphActive = true;
                this.updateNetworkGraph();
            } else {
                this.networkGraphActive = false;
            }
        }

        initDashboardCharts() {
            // Wait for Chart.js to load if needed
            if (typeof Chart === 'undefined') {
                console.warn('Chart.js not loaded yet');
                return;
            }

            // 1. Network Chart (NetFlow)
            const ctxNet = document.getElementById('networkChart');
            if (ctxNet) {
                // Destroy existing if any
                if (this.charts.network) this.charts.network.destroy();

                this.charts.network = new Chart(ctxNet, {
                    type: 'line',
                    data: {
                        labels: Array(60).fill(''),
                        datasets: [{
                            label: 'Download Speed',
                            data: Array(60).fill(0),
                            borderColor: '#1DE9B6', // Oceanic Teal
                            backgroundColor: (context) => {
                                const ctx = context.chart.ctx;
                                const gradient = ctx.createLinearGradient(0, 0, 0, 200);
                                gradient.addColorStop(0, 'rgba(29, 233, 182, 0.2)');
                                gradient.addColorStop(1, 'rgba(29, 233, 182, 0)');
                                return gradient;
                            },
                            borderWidth: 2,
                            fill: true,
                            tension: 0.4,
                            pointRadius: 0
                        }]
                    },
                    options: {
                        responsive: true,
                        maintainAspectRatio: false,
                        plugins: { legend: { display: false }, tooltip: { enabled: false } },
                        scales: {
                            x: { display: false },
                            y: { display: false, min: 0 }
                        },
                        animation: false,
                        interaction: { intersect: false }
                    }
                });
            }

            // 2. Storage Chart (Ring)
            const ctxStore = document.getElementById('storageChart');
            if (ctxStore) {
                if (this.charts.storage) this.charts.storage.destroy();

                this.charts.storage = new Chart(ctxStore, {
                    type: 'doughnut',
                    data: {
                        labels: ['Used', 'Free'],
                        datasets: [{
                            data: [0, 100], // Start empty
                            backgroundColor: ['#1DE9B6', 'rgba(255, 255, 255, 0.05)'],
                            borderWidth: 0,
                            hoverOffset: 4
                        }]
                    },
                    options: {
                        responsive: true,
                        cutout: '80%',
                        plugins: { legend: { display: false }, tooltip: { enabled: false } }
                    }
                });
            }
        }

        updateDashboard() {
            // Priority: use in-memory stats, fall back to storage
            let fd = this.stats && this.stats.fshare_downloader;

            // Fallback load
            if (!fd) {
                const stored = localStorage.getItem('fshare_stats');
                if (stored) {
                    try {
                        fd = JSON.parse(stored).fshare_downloader;
                    } catch (e) { }
                }
            }

            if (!fd) return;

            // Global Status Bar Updates
            this.setText('global-down-speed', fd.speed || '0 B/s');
            this.setText('global-up-speed', '0 B/s');

            const primary = fd.primary_account || {};
            const isPremium = primary.premium || false;
            const isValid = primary.valid || false;
            const accountType = isPremium ? 'VIP' : (isValid ? 'FREE' : 'N/A');
            this.setText('global-quota', primary.traffic_left || '-- / --');

            // --- Dashboard Specific Updates ---

            // 1. Account Info
            this.setText('dash-account-type', accountType);
            this.setText('dash-expiry', primary.expire_date || 'Never'); // Assume expire_date is available or handle generic

            // Quota
            this.setText('dash-quota-text', primary.traffic_left || '-- / --');
            // Parse quota for progress bar if possible (e.g. "50.5 GB / 100 GB")
            const quotaText = primary.traffic_left || '';
            const match = quotaText.match(/([\d.]+)\s*GB\s*\/\s*([\d.]+)\s*GB/i);
            if (match) {
                const used = parseFloat(match[1]); // This is usually "Left" in Fshare, need to check label logic. 
                // Primary.traffic_left is usually "Left / Total"
                const left = parseFloat(match[1]);
                const total = parseFloat(match[2]);
                const percentLeft = (left / total) * 100;
                // If it's a "User Quota", we might want to show USED or LEFT. 
                // Let's assume bar shows LEFT for now as that's "positive". 
                // Or standard is USED. Let's do % LEFT.
                const bar = document.getElementById('dash-quota-bar');
                if (bar) bar.style.width = `${percentLeft}%`;
            }

            const badge = document.getElementById('dash-account-badge');
            if (badge) {
                badge.style.background = isPremium ? 'rgba(29, 233, 182, 0.1)' : (isValid ? 'rgba(234, 179, 8, 0.1)' : 'rgba(100, 116, 139, 0.1)');
                badge.style.color = isPremium ? 'var(--primary)' : (isValid ? '#eab308' : '#64748b');
            }

            // 2. Queue Status
            this.setText('dash-active-count', fd.active || 0);
            this.setText('dash-queue-count', fd.total || 0);

            // 3. Minified Queue
            // Filter active downloads for the minified list or take top N
            // Assuming this.downloads is populated by `updateDownloads` (which reads from separate API or event)
            // If strictly relying on stats object, we might not have list. 
            // We need to ensure `this.downloads` is current.
            if (document.getElementById('dash-minified-queue')) {
                this.renderMinifiedQueue(this.downloads.slice(0, 5));
            }
        }

        renderMinifiedQueue(items) {
            const container = document.getElementById('dash-minified-queue');
            if (!container) return;

            if (!items || items.length === 0) {
                container.innerHTML = '<div class="empty-placeholder" style="padding: 2rem; text-align: center; color: var(--text-muted); font-size: 0.85rem;">No active downloads</div>';
                return;
            }

            const html = items.map(item => {
                const progress = item.progress || 0;
                const speed = item.speed || '0 B/s';
                const size = item.size || '--';
                // Simple progress bar
                return `
                <div class="minified-item" style="display: grid; grid-template-columns: 2fr 1fr 1fr 1fr; align-items: center; padding: 0.75rem 1rem; border-bottom: 1px solid var(--border-color); font-size: 0.85rem; color: var(--text-color);">
                    <div class="name-col" style="white-space: nowrap; overflow: hidden; text-overflow: ellipsis; padding-right: 1rem;" title="${this.escapeHtml(item.name)}">
                        ${this.escapeHtml(item.name)}
                    </div>
                    <div style="text-align: right; color: var(--text-muted); font-family: 'Roboto Mono', monospace; font-size: 0.75rem;">${size}</div>
                    <div class="progress-col" style="padding: 0 0.5rem;">
                        <div style="background: rgba(255,255,255,0.1); height: 4px; border-radius: 2px; overflow: hidden; width: 100%;">
                            <div style="width: ${progress}%; background: var(--primary); height: 100%;"></div>
                        </div>
                    </div>
                    <div style="text-align: right; font-family: 'Roboto Mono', monospace; font-size: 0.75rem; color: var(--primary);">${speed}</div>
                </div>
                `;
            }).join('');

            container.innerHTML = html;
        }

        updateNetworkGraph() {
            if (!this.networkGraph || !this.networkGraphActive) return;

            const stored = localStorage.getItem('fshare_stats');
            if (!stored) return;

            try {
                const data = JSON.parse(stored);
                if (data.fshare_downloader) {
                    const speedBytes = data.fshare_downloader.speed_bytes || 0;
                    this.networkGraph.addDataPoint(speedBytes);

                    this.setText('current-speed', this.networkGraph.formatSpeed(speedBytes));
                    this.setText('peak-speed', this.networkGraph.formatSpeed(this.networkGraph.peakSpeed));
                }
            } catch (e) { }
        }

        updateETACountdown() {
            const etaElements = document.querySelectorAll('.eta-cell');
            etaElements.forEach(el => {
                const text = el.textContent;
                const match = text.match(/([\d:]+)/) || [null, text];
                if (match[1] && match[1] !== '-') {
                    // Simpler countdown
                    let parts = match[1].split(':').map(Number);
                    let total = parts.reduce((acc, v) => acc * 60 + v, 0);
                    if (total > 0) {
                        total--;
                        const h = Math.floor(total / 3600);
                        const m = Math.floor((total % 3600) / 60);
                        const s = total % 60;
                        const fmt = h > 0 ? `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}` : `${m}:${s.toString().padStart(2, '0')}`;
                        el.textContent = fmt;
                    }
                }
            });
        }

        updateStatusIndicator(id, isOnline) {
            const el = document.getElementById(id);
            if (el) el.className = `widget-status ${isOnline ? 'online' : 'offline'}`;
        }

        updateBadge(id, isSuccess, text) {
            const el = document.getElementById(id);
            if (el) {
                el.className = `status-badge ${isSuccess ? 'success' : 'error'}`;
                el.textContent = text;
            }
        }

        setText(id, value) {
            const el = document.getElementById(id);
            if (el) el.textContent = value;
        }

        async handleAutocomplete(query) {
            if (!query || query.length < 2) { this.hideAutocomplete(); return; }
            try {
                const response = await fetch(`/api/autocomplete?q=${encodeURIComponent(query)}`);
                const data = await response.json();
                if (data.suggestions?.length > 0) this.showAutocomplete(data.suggestions.slice(0, 3));
                else this.hideAutocomplete();
            } catch (e) { this.hideAutocomplete(); }
        }

        showAutocomplete(suggestions) {
            const dropdown = document.getElementById('autocomplete-dropdown');
            if (!dropdown) return;
            dropdown.innerHTML = suggestions.map(s => `<div class="autocomplete-item" onclick="bridge.redirectToSearch('${this.escapeHtml(s)}')">${this.escapeHtml(s)}</div>`).join('');
            dropdown.className = 'autocomplete-dropdown show';
        }

        hideAutocomplete() {
            const dropdown = document.getElementById('autocomplete-dropdown');
            if (dropdown) dropdown.className = 'autocomplete-dropdown';
        }

        redirectToSearch(query) { window.location.href = `/search?q=${encodeURIComponent(query)}`; }

        async loadDownloads() { return await this.fetchDownloads(); }
        async runFullPollCheck() { return await this.fetchDownloads(); }

        async fetchStats() {
            try {
                const response = await fetch('/api/stats');
                const data = await response.json();
                if (data.status === 'ok') {
                    console.log('📊 Initial Stats Loaded:', data);
                    this.stats = data;
                    this.updateDashboard();
                }
            } catch (e) {
                console.error('Fetch stats failed:', e);
            }
        }

        getFilteredDownloads() {
            const query = (document.getElementById('downloads-search-input')?.value || '').toLowerCase();
            return this.downloads.filter(d => {
                const matchesSearch = d.name.toLowerCase().includes(query);

                const cat = (d.category || 'uncategorized').toLowerCase();
                const matchesCategory = this.filters.category === 'all' || cat === this.filters.category;

                const status = (d.status || '').toLowerCase();
                let matchesStatus = this.filters.status === 'all';
                if (!matchesStatus) {
                    if (this.filters.status === 'running') matchesStatus = status.includes('running') || status.includes('downloading') || status.includes('starting') || status.includes('extracting');
                    else if (this.filters.status === 'paused') matchesStatus = status.includes('paused') || status.includes('stopped');
                    else if (this.filters.status === 'completed') matchesStatus = status.includes('finished') || status.includes('completed');
                    else if (this.filters.status === 'error') matchesStatus = status.includes('failed') || status.includes('error');
                    else matchesStatus = status === this.filters.status;
                }

                return matchesSearch && matchesCategory && matchesStatus;
            });
        }

        setFilter(type, value) {
            this.filters[type] = value;
            this.notifyDownloadsChanged();
        }

        // Checkbox temporarily disabled per user request
        toggleSelectAll(checked) { }

        toggleSelection(fid) {
            if (this.selectedDownloads.has(fid)) {
                this.selectedDownloads.delete(fid);
            } else {
                this.selectedDownloads.add(fid);
            }
            // Update "Select All" checkbox state logic could go here, but simple re-render works
            const cb = document.getElementById('select-all-checkbox');
            if (cb) {
                const visible = this.getFilteredDownloads();
                cb.checked = visible.length > 0 && visible.every(d => this.selectedDownloads.has(d.fid));
            }
        }

        getPagedDownloads(downloads) {
            const page = this.currentPage || 1;
            const size = this.itemsPerPage || 6;
            return downloads.slice((page - 1) * size, page * size);
        }

        updatePagination(total) {
            const info = document.getElementById('pagination-info');
            if (!info) return;
            const totalPages = Math.ceil(total / (this.itemsPerPage || 6)) || 1;
            this.currentPage = Math.min(this.currentPage || 1, totalPages);
            const start = total === 0 ? 0 : (this.currentPage - 1) * 6 + 1;
            const end = Math.min(this.currentPage * 6, total);
            info.textContent = `Showing ${start}-${end} of ${total}`;
            const p = document.getElementById('prev-page'), n = document.getElementById('next-page');
            if (p) p.disabled = this.currentPage === 1;
            if (n) n.disabled = this.currentPage === totalPages;
        }

        changePage(delta) { this.currentPage = (this.currentPage || 1) + delta; this.notifyDownloadsChanged(); }

        updateDownloadRow(d, isFullView) {
            const rowId = `row-${d.fid}`;
            let row = document.getElementById(rowId);
            if (!row) return false;

            const s = (d.status || '').toLowerCase();
            let state = 'warning';
            let displayStatus = d.status.toUpperCase();
            let canToggle = true;

            if (s === 'finished' || s === 'completed' || parseFloat(d.progress) >= 100) {
                state = 'completed'; displayStatus = 'COMPLETED'; canToggle = false;
            } else if (s === 'running' || s === 'downloading' || s === 'starting' || s === 'extracting') {
                state = 'running';
            } else if (s === 'paused' || s === 'stopped') {
                state = 'paused';
            } else if (s === 'failed' || s === 'error') {
                state = 'error'; canToggle = false;
            }

            const isRunning = state === 'running';

            // Update row class
            row.className = `main-row row-${state}${row.classList.contains('is-expanded') ? ' is-expanded' : ''}`;

            // Helper for simple text updates
            const updateText = (selector, text) => {
                const el = row.querySelector(selector);
                if (el && el.textContent !== text) el.textContent = text;
            };

            updateText('.download-name', d.name);
            updateText('.cell-size', d.size);
            updateText('.cell-speed', isRunning ? d.speed : '-');
            updateText('.cell-eta', d.eta);
            updateText('.cell-added', this.formatAddedDate(d.added));

            // Category Update
            const catBadge = row.querySelector('.category-badge');
            if (catBadge) {
                catBadge.className = `category-badge cat-${(d.category || '').toLowerCase()}`;
                catBadge.textContent = d.category || 'Uncategorized';
            }

            // Progress Update
            const progressFill = row.querySelector('.progress-fill');
            if (progressFill) progressFill.style.width = `${d.progress}%`;
            updateText('.progress-text', `${d.progress}%`);

            // Status Update
            const statusBadge = row.querySelector('.status-badge');
            if (statusBadge) {
                statusBadge.className = `status-badge ${state}`;
                statusBadge.textContent = displayStatus;
            }

            // Error Message Update
            const errorBtn = row.querySelector('.error-view-btn');
            if (errorBtn) {
                errorBtn.style.display = (state === 'error' && d.error_message) ? 'flex' : 'none';
                if (d.error_message) {
                    errorBtn.onclick = (e) => { e.stopPropagation(); bridge.showError(d.error_message); };
                }
            }

            // Details Row Update (Actions Removed as per request)
            const detailsRow = document.getElementById(`details-${d.fid}`);
            if (detailsRow) {
                // If it exists (legacy or other view), just hide it
                detailsRow.style.display = 'none';
            }

            return true;
        }

        renderDashboardDownloads(downloads) {
            const container = document.getElementById('download-manager-list');
            if (!container) return;

            // If count mismatch or first load, re-render
            const currentRows = container.querySelectorAll('.main-row');
            if (currentRows.length !== downloads.length || downloads.length === 0) {
                container.innerHTML = downloads.length ? downloads.map(d => this.createDashboardDownloadRow(d)).join('') : `<tr><td colspan="8" style="text-align: center; padding: 2rem; color: var(--text-muted);">No active downloads</td></tr>`;
                return;
            }

            // Attempt partial updates
            let needsFullRender = false;
            downloads.forEach((d, index) => {
                const row = currentRows[index * 2];
                if (!row || row.id !== `row-${d.fid}`) {
                    needsFullRender = true;
                    return;
                }
                this.updateDownloadRow(d, false);
            });

            if (needsFullRender) {
                container.innerHTML = downloads.map(d => this.createDashboardDownloadRow(d)).join('');
            }
        }

        renderFullDownloads(downloads) {
            const container = document.getElementById('downloads-full-list');
            if (!container) return;

            if (downloads.length === 0) {
                container.innerHTML = `<tr><td colspan="8" style="text-align: center; padding: 5rem; color: var(--text-muted);">No downloads found</td></tr>`;
                return;
            }

            const currentRows = container.querySelectorAll('tr');

            // Simple heuristic: if length matches and IDs match, update. Else re-render.
            let match = true;
            if (currentRows.length === downloads.length) {
                for (let i = 0; i < downloads.length; i++) {
                    if (currentRows[i].id !== `row-${downloads[i].fid}`) {
                        match = false;
                        break;
                    }
                }
            } else {
                match = false;
            }

            if (match) {
                downloads.forEach(d => this.updateDownloadRow(d, true));
            } else {
                container.innerHTML = downloads.map(d => this.createFullDownloadRow(d)).join('');
            }
        }

        applySorting() {
            if (!this.downloads) return;

            const stateWeights = {
                'starting': 1,
                'downloading': 2,
                'extracting': 3,
                'queued': 4,
                'paused': 5,
                'stopped': 6,
                'finished': 7,
                'completed': 8,
                'failed': 9,
                'error': 10
            };

            this.downloads.sort((a, b) => {
                let aVal, bVal;
                switch (this.sortColumn) {
                    case 'name': aVal = a.name.toLowerCase(); bVal = b.name.toLowerCase(); break;
                    case 'size': aVal = a.size_bytes; bVal = b.size_bytes; break;
                    case 'speed': aVal = a.speed_raw; bVal = b.speed_raw; break;
                    case 'eta': aVal = a.eta_seconds; bVal = b.eta_seconds; break;
                    case 'progress': aVal = parseFloat(a.progress); bVal = parseFloat(b.progress); break;
                    case 'category': aVal = a.category.toLowerCase(); bVal = b.category.toLowerCase(); break;
                    case 'added': aVal = a.added ? new Date(a.added).getTime() : 0; bVal = b.added ? new Date(b.added).getTime() : 0; break;
                    case 'status':
                        aVal = stateWeights[a.status.toLowerCase()] || 99;
                        bVal = stateWeights[b.status.toLowerCase()] || 99;
                        break;
                    default:
                        // Default fallback sort: Active first, then by date
                        const aActive = (a.status.toLowerCase() === 'downloading' || a.status.toLowerCase() === 'starting') ? 0 : 1;
                        const bActive = (b.status.toLowerCase() === 'downloading' || b.status.toLowerCase() === 'starting') ? 0 : 1;
                        if (aActive !== bActive) return aActive - bActive;
                        return new Date(b.added || 0) - new Date(a.added || 0);
                }

                if (aVal === bVal) return 0;
                const result = aVal < bVal ? -1 : 1;
                return this.sortDirection === 'asc' ? result : -result;
            });
        }

        updateSortIcons(column) {
            document.querySelectorAll('.material-icons[id*="sort-icon"]').forEach(icon => {
                icon.textContent = 'swap_vert'; icon.style.opacity = '0.5';
            });
            ['sort-icon-', 'dash-sort-icon-'].forEach(p => {
                const icon = document.getElementById(p + column);
                if (icon) {
                    icon.textContent = this.sortDirection === 'asc' ? 'expand_less' : 'expand_more';
                    icon.style.opacity = '1';
                }
            });
        }

        createDashboardDownloadRow(d) {
            return this.createFullDownloadRow(d, false);
        }

        createFullDownloadRow(d, showActions = true) {
            const s = (d.status || '').toLowerCase();
            let state = 'warning';
            let displayStatus = d.status.toUpperCase();
            let canToggle = true;

            if (s === 'finished' || s === 'completed' || parseFloat(d.progress) >= 100) {
                state = 'completed'; displayStatus = 'COMPLETED'; canToggle = false;
            } else if (s === 'running' || s === 'downloading' || s === 'starting' || s === 'extracting') {
                state = 'running';
            } else if (s === 'paused' || s === 'stopped') {
                state = 'paused';
            } else if (s === 'failed' || s === 'error') {
                state = 'error'; canToggle = false;
            }

            const isRunning = state === 'running';
            const contextMenuAttr = showActions ? `oncontextmenu="bridge.showContextMenu(event, '${d.fid}')"` : '';

            // Note: actionBtns removed from main row as per request.
            // Controls now predominantly via Context Menu.

            return `
            <tr id="row-${d.fid}" class="main-row row-${state}" ${contextMenuAttr}>
                <!-- 1. NAME -->
                <td class="cell-name" style="width: 300px; max-width: 300px; position: relative;">
                    <div class="download-name" title="${this.escapeHtml(d.name)}" style="width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; font-size: 0.9rem;">${this.escapeHtml(d.name)}</div>
                </td>
                <!-- 2. SIZE -->
                <td class="cell-size" style="font-family: 'Roboto Mono', monospace; font-size: 0.76rem; width: 100px;">${d.size}</td>
                <!-- 3. PROGRESS -->
                <td class="cell-progress" style="width: 220px;">
                    <div style="display: flex; align-items: center; gap: 8px; width: 100%;">
                        <div class="progress-bar" style="flex: 1; height: 6px; background: rgba(255,255,255,0.1); border-radius: 3px; overflow: hidden; margin: 0;">
                            <div class="progress-fill" style="width: ${d.progress}%; height: 100%; background: #3b82f6; transition: width 0.3s ease;"></div>
                        </div>
                        <span class="progress-text" style="font-size: 0.68rem; font-weight: 700; color: var(--text-muted); min-width: 40px;">${d.progress}%</span>
                    </div>
                </td>
                <!-- 4. STATUS -->
                <td class="cell-status" style="width: 100px;">
                    <div style="display: flex; flex-direction: column; gap: 4px;">
                        <span class="status-badge ${state}" style="width: fit-content; font-size: 0.68rem;">${displayStatus}</span>
                        <button class="icon-btn error-view-btn" onclick="bridge.showError('${this.escapeHtml(d.error_message)}'); event.stopPropagation();" 
                                style="color: #ef4444; padding: 0; width: fit-content; display: ${state === 'error' && d.error_message ? 'flex' : 'none'}; align-items: center; gap: 4px;">
                            <span class="material-icons" style="font-size: 13px;">error_outline</span> <span style="font-size: 0.63rem;">View Error</span>
                        </button>
                    </div>
                </td>
                <!-- 5. SPEED -->
                <td class="cell-speed" style="font-family: 'Roboto Mono', monospace; font-size: 0.76rem; width: 100px;">${isRunning ? d.speed : '-'}</td>
                <!-- 6. ETA -->
                <td class="cell-eta" style="font-family: 'Roboto Mono', monospace; font-size: 0.76rem; width: 100px;">${d.eta}</td>
                <!-- 7. CATEGORY -->
                <td class="cell-category" style="width: 120px;"><span class="category-badge cat-${(d.category || '').toLowerCase()}" style="font-size: 0.63rem;">${d.category || 'Uncategorized'}</span></td>
                <!-- 8. ADDED -->
                <td class="cell-added" style="font-size: 0.76rem; color: var(--text-muted); width: 150px;">${this.formatAddedDate(d.added)}</td>
            </tr>`;
        }

        toggleRowExpansion(fid) {
            const details = document.getElementById(`details-${fid}`);
            const row = document.getElementById(`row-${fid}`);
            if (details && row) {
                const isExpanded = details.classList.toggle('expanded');
                row.classList.toggle('is-expanded', isExpanded);
                if (isExpanded) {
                    document.querySelectorAll('.details-row.expanded').forEach(el => {
                        if (el.id !== `details-${fid}`) {
                            el.classList.remove('expanded');
                            const mainId = el.id.replace('details-', 'row-');
                            const mainEl = document.getElementById(mainId);
                            if (mainEl) mainEl.classList.remove('is-expanded');
                        }
                    });
                }
            }
        }
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

        async sortDownloads(column) {
            if (this.sortColumn === column) this.sortDirection = this.sortDirection === 'asc' ? 'desc' : 'asc';
            else { this.sortColumn = column; this.sortDirection = 'asc'; }
            this.updateSortIcons(column);
            this.notifyDownloadsChanged();
        }

        showError(msg) {
            const modal = document.getElementById('error-modal'), text = document.getElementById('error-modal-text');
            if (modal && text) { text.textContent = msg; modal.style.display = 'flex'; } else alert(msg);
        }

        closeErrorModal() { const modal = document.getElementById('error-modal'); if (modal) modal.style.display = 'none'; }

        async toggleDownload(fid) {
            try {
                const response = await fetch(`/api/download/toggle/${fid}`, { method: 'POST' });
                if ((await response.json()).success) await this.fetchDownloads();
            } catch (e) { }
        }

        async deleteDownload(fid) {
            const modal = document.getElementById('delete-modal');
            const confirmBtn = document.getElementById('confirm-delete-btn');
            const nameEl = document.getElementById('delete-item-name');

            if (modal && confirmBtn) {
                // Find item name
                const item = this.downloads.find(d => d.fid === fid);
                if (nameEl) nameEl.textContent = item ? item.name : 'this item';

                // Show modal
                modal.style.display = 'flex';

                // Handle confirm click
                confirmBtn.onclick = async () => {
                    confirmBtn.disabled = true;
                    confirmBtn.textContent = 'DELETING...';
                    try {
                        const response = await fetch(`/api/download/delete/${fid}`, { method: 'DELETE' });
                        if ((await response.json()).success) await this.fetchDownloads();
                    } catch (e) {
                        console.error(e);
                    } finally {
                        modal.style.display = 'none';
                        confirmBtn.disabled = false;
                        confirmBtn.textContent = 'DELETE';
                    }
                };
            } else {
                // Fallback
                if (confirm('Delete this download and file?')) {
                    await fetch(`/api/download/delete/${fid}`, { method: 'DELETE' });
                    await this.fetchDownloads();
                }
            }
        }

        async retryDownload(id) {
            try {
                const resp = await fetch(`/api/download/retry/${id}`, { method: 'POST' });
                if ((await resp.json()).success) await this.fetchDownloads();
                else this.showError('Retry failed');
            } catch (e) { }
        }

        async submitManualLink() {
            const input = document.getElementById('manual-link-input'), btn = document.getElementById('submit-link-btn'), url = input.value.trim();
            if (!url) {
                this.showNotification('Please enter a valid URL', 'warning');
                return;
            }
            try {
                btn.disabled = true; btn.textContent = "ADDING...";
                const response = await fetch('/api/download', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ url }) });
                const result = await response.json();

                if (result.success) {
                    this.showNotification('Download added to queue successfully', 'success');
                    this.hideAddModal();
                    input.value = '';
                    await this.fetchDownloads();
                } else {
                    const msg = result.error || 'Unknown error';
                    if (msg.toLowerCase().includes('authorized') || msg.toLowerCase().includes('login') || msg.toLowerCase().includes('session')) {
                        this.showNotification('Session Invalid: Please check your account settings.', 'error');
                    } else {
                        this.showNotification(`Failed to add: ${msg}`, 'error');
                    }
                }
            } catch (e) {
                this.showNotification('Connection Error: Could not reach server', 'error');
            } finally { btn.disabled = false; btn.textContent = "ADD TO QUEUE"; }
        }

        // System Logs
        async loadSystemLogs() {
            const container = document.getElementById('system-log');
            if (!container) return;
            try {
                const response = await fetch('/api/logs');
                const data = await response.json();
                container.innerHTML = (data.logs && data.logs.length > 0) ?
                    data.logs.map(log => `<div class="log-entry ${log.level}"><span class="log-time">${this.escapeHtml(log.time)}</span><span class="log-message">${this.escapeHtml(log.message)}</span></div>`).join('') :
                    `<div class="log-entry info"><span class="log-time">[--:--:--]</span><span class="log-message">No recent logs available</span></div>`;
            } catch (e) { console.error('Load logs error:', e); }
        }

        // Search Operations
        async search(query) {
            if (!query) return;
            this.lastSearchQuery = query;
            localStorage.setItem('fshare_last_search_query', query);

            if (window.location.pathname !== '/search') { this.redirectToSearch(query); return; }

            const resultsContainer = document.getElementById('search-results-grid'); // Fixed ID
            if (!resultsContainer) return;

            // Show loading
            resultsContainer.innerHTML = '<div class="loading-state" style="display: flex;"><span class="material-icons spinning">refresh</span><p>Searching Fshare...</p></div>';

            try {
                const response = await fetch(`/api/search?q=${encodeURIComponent(query)}`);
                const data = await response.json();
                if (data.results && data.results.length > 0) {
                    this.lastSearchResults = data.results;
                    localStorage.setItem('fshare_last_search_results', JSON.stringify(data.results));
                    this.displayResults(data.results);
                } else {
                    this.lastSearchResults = [];
                    localStorage.removeItem('fshare_last_search_results');
                    resultsContainer.innerHTML = '<div class="empty-state" style="display: flex;"><span class="material-icons">search_off</span><p>No results found</p></div>';
                }
            } catch (error) {
                console.error('Search error:', error);
                resultsContainer.innerHTML = '<div class="empty-state" style="display: flex; color: var(--accent-red);"><p>Search failed</p></div>';
            }
        }

        displayResults(results) {
            // This is primarily handled by the search.html script now to respect its pagination/filters
            // but we provide a fallback or trigger if needed.
            const grid = document.getElementById('search-results-grid');
            if (grid && window._searchPageRender) {
                window._searchPageRender(results);
            }
        }

        createResultCard(result) {
            const { name, score, url, size } = result;
            const metadata = result.metadata || this.parseMetadata(name);
            const sizeStr = this.formatSize(size);

            return `
            <div class="widget">
                <div class="widget-header">
                    <div class="widget-title" style="font-size: 0.95rem; text-transform: none;">${this.escapeHtml(name)}</div>
                </div>
                <div class="widget-content">
                    <div style="display: flex; gap: 0.5rem; margin-bottom: 1rem; flex-wrap: wrap;">
                        <span class="status-badge success">Score: ${score}</span>
                        <span class="status-badge info">${sizeStr}</span>
                        ${metadata.resolution ? `<span class="status-badge info">${metadata.resolution}</span>` : ''}
                        ${metadata.viet_sub ? `<span class="status-badge success" style="background: rgba(16, 185, 129, 0.2); color: #10b981; border: 1px solid rgba(16, 185, 129, 0.3);">VIET SUB</span>` : ''}
                        ${metadata.viet_dub ? `<span class="status-badge success" style="background: rgba(245, 158, 11, 0.2); color: #f59e0b; border: 1px solid rgba(245, 158, 11, 0.3);">VIET DUB</span>` : ''}
                    </div>
                    <button class="btn-primary" style="width: 100%; justify-content: center;" onclick="bridge.download(event, '${this.escapeHtml(url)}', '${this.escapeHtml(name)}')">Download</button>
                </div>
            </div>`;
        }

        parseMetadata(filename) {
            const metadata = { resolution: null, viet_sub: false, viet_dub: false };
            if (filename.match(/2160p|4K|UHD/i)) metadata.resolution = '4K';
            else if (filename.match(/1080p/i)) metadata.resolution = '1080p';
            else if (filename.match(/720p/i)) metadata.resolution = '720p';

            if (filename.match(/vietsub|viet\.sub|vie\.sub|phụ đề|phu de/i)) metadata.viet_sub = true;
            if (filename.match(/thuyết minh|thuyet minh|viet\.dub|vie\.dub|lồng tiếng|long tieng|tvp|tmpđ/i)) metadata.viet_dub = true;
            return metadata;
        }

        formatSize(bytes) {
            if (bytes === 0) return '0 B';
            const k = 1024, sizes = ['B', 'KB', 'MB', 'GB', 'TB'], i = Math.floor(Math.log(bytes) / Math.log(k));
            return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
        }

        async download(event, url, name) {
            let btn = event?.currentTarget || event?.target?.closest('button');
            let originalHtml = '';
            if (btn) { btn.disabled = true; originalHtml = btn.innerHTML; btn.innerHTML = `<span class="material-icons spin" style="font-size: 1.2rem;">sync</span> Adding...`; }

            try {
                const response = await fetch('/api/download', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ url, name }) });
                const data = await response.json();
                if (data.success) {
                    if (btn) btn.innerHTML = `✅ Added`;
                    alert(`✅ Added to queue: ${data.normalized}`);
                    await this.runFullPollCheck();
                } else {
                    if (btn) { btn.disabled = false; btn.innerHTML = originalHtml; }
                    alert('❌ Failed to add: ' + (data.error || 'Unknown error'));
                }
            } catch (error) {
                console.error('Download error:', error);
                if (btn) { btn.disabled = false; btn.innerHTML = originalHtml; }
            }
        }

        showAddModal() { const m = document.getElementById('add-link-modal'); if (m) { m.style.display = 'flex'; document.getElementById('manual-link-input').focus(); } }
        hideAddModal() { const m = document.getElementById('add-link-modal'); if (m) { m.style.display = 'none'; document.getElementById('manual-link-input').value = ''; } }

        setupEventListeners() {
            const headerSearch = document.getElementById('header-search-input');
            if (headerSearch) {
                headerSearch.addEventListener('keypress', (e) => { if (e.key === 'Enter') this.redirectToSearch(headerSearch.value); });
            }

            if (!this.global_listeners_ready) {
                this.global_listeners_ready = true;
                document.addEventListener('click', (e) => {
                    if (!e.target.closest('.search-container')) this.hideAutocomplete();
                });
            }

            const sidebarToggle = document.getElementById('sidebar-toggle');
            if (sidebarToggle) sidebarToggle.onclick = () => this.toggleSidebar();

            const sidebarBrand = document.getElementById('sidebar-brand');
            if (sidebarBrand) sidebarBrand.onclick = () => this.toggleSidebar();

            const searchInput = document.getElementById('search-input');
            if (searchInput) searchInput.addEventListener('keypress', (e) => { if (e.key === 'Enter') this.search(searchInput.value); });

            // Theme selection listeners
            const attachThemeListeners = () => {
                document.querySelectorAll('input[name="theme"]').forEach(input => {
                    input.removeEventListener('change', this._themeChangeHandler);
                    this._themeChangeHandler = (e) => this.applyTheme(e.target.value);
                    input.addEventListener('change', this._themeChangeHandler);
                });
            };
            attachThemeListeners();
            // Also expose to window for extreme cases
            window._flasharr_attachThemeListeners = attachThemeListeners;
        }

        initSidebar() {
            const sidebar = document.getElementById('sidebar');
            if (sidebar && localStorage.getItem('sidebar-collapsed') === 'true') sidebar.classList.add('collapsed');
        }

        toggleSidebar() {
            const sidebar = document.getElementById('sidebar');
            if (sidebar) {
                sidebar.classList.toggle('collapsed');
                localStorage.setItem('sidebar-collapsed', sidebar.classList.contains('collapsed'));
            }
        }

        formatSpeed(bytes) {
            if (bytes === 0) return '0.0 B/s';
            const k = 1024, sizes = ['B/s', 'KB/s', 'MB/s', 'GB/s', 'TB/s'], i = Math.floor(Math.log(bytes) / Math.log(k));
            // Ensure 1 digit fraction (e.g., "5.0 MB/s")
            return (bytes / Math.pow(k, i)).toFixed(1) + ' ' + sizes[i];
        }

        async loadSystemLogs() {
            const container = document.getElementById('system-log');
            if (!container) return;
            try {
                const response = await fetch('/api/logs?lines=50');
                const data = await response.json();
                if (data.status === 'ok') {
                    // Update internal state
                    this.systemLogs = data.logs;
                    localStorage.setItem('fshare_system_logs', JSON.stringify(this.systemLogs));
                    this.renderSystemLogs();
                }
            } catch (e) {
                console.error("Failed to load logs:", e);
            }
        }

        renderSystemLogs() {
            const container = document.getElementById('system-log');
            if (!container || !this.systemLogs.length) return;

            container.innerHTML = this.systemLogs.map(line => `<div>${this.escapeHtml(line)}</div>`).join('');
            container.scrollTop = container.scrollHeight;
        }

        escapeHtml(text) {
            const div = document.createElement('div');
            div.textContent = text;
            return div.innerHTML;
        }

        initTheme() {
            // First check localStorage for immediate application (prevent flicker)
            let theme = localStorage.getItem('flasharr_theme') || 'dark';
            this.applyTheme(theme, false); // Don't save back to localStorage during init

            // On settings page, the settings loading will handle the definitive state
        }

        applyTheme(theme, save = true) {
            if (!theme) return;

            document.body.classList.remove('light-mode');

            if (theme === 'light') {
                document.body.classList.add('light-mode');
            } else if (theme === 'system') {
                const isLight = window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches;
                document.body.classList.toggle('light-mode', isLight);

                if (!this.theme_watcher_ready) {
                    this.theme_watcher_ready = true;
                    window.matchMedia('(prefers-color-scheme: light)').addEventListener('change', e => {
                        if (localStorage.getItem('flasharr_theme') === 'system') {
                            document.body.classList.toggle('light-mode', e.matches);
                        }
                    });
                }
            }

            if (save) {
                localStorage.setItem('flasharr_theme', theme);
                // Also update settings radio buttons if they exist
                const radio = document.querySelector(`input[name="theme"][value="${theme}"]`);
                if (radio) radio.checked = true;
            }
        }

        initContextMenu() {
            const menu = document.getElementById('context-menu');
            if (!menu) return;

            // Hide on click anywhere
            document.addEventListener('click', () => menu.style.display = 'none');
            document.addEventListener('scroll', () => menu.style.display = 'none');

            // Prevent context menu on the menu itself
            menu.addEventListener('contextmenu', e => e.preventDefault());

            // Menu actions
            document.getElementById('menu-resume').onclick = () => this.toggleDownload(this.contextFid);
            document.getElementById('menu-pause').onclick = () => this.toggleDownload(this.contextFid);
            document.getElementById('menu-retry').onclick = () => this.retryDownload(this.contextFid);
            document.getElementById('menu-delete').onclick = () => this.deleteDownload(this.contextFid);
            document.getElementById('menu-copy').onclick = async () => {
                const item = this.downloads.find(d => d.fid === this.contextFid);
                if (item && item.info) {
                    try {
                        await navigator.clipboard.writeText(item.info);
                        // Optional: show a mini toast
                    } catch (err) {
                        alert('Failed to copy: ' + err);
                    }
                } else {
                    alert('URL not available for this item');
                }
            };
        }

        showContextMenu(e, fid) {
            e.preventDefault();
            this.contextFid = fid;
            const menu = document.getElementById('context-menu');
            if (!menu) return;

            const item = this.downloads.find(d => d.fid === fid);
            if (!item) return;

            // Update menu based on item state
            const isRunning = ['running', 'downloading', 'starting', 'extracting'].includes(item.status.toLowerCase());
            document.getElementById('menu-resume').style.display = isRunning ? 'none' : 'flex';
            document.getElementById('menu-pause').style.display = isRunning ? 'flex' : 'none';

            // Ensure Retry is visible for error items
            document.getElementById('menu-retry').style.display = (item.status.toLowerCase() === 'error' || item.status.toLowerCase() === 'failed') ? 'flex' : 'none';

            menu.style.display = 'block';

            // Position menu
            let x = e.clientX;
            let y = e.clientY;

            // Boundary checks
            const winW = window.innerWidth;
            const winH = window.innerHeight;
            const menuW = menu.offsetWidth;
            const menuH = menu.offsetHeight;

            if (x + menuW > winW) x = winW - menuW - 10;
            if (y + menuH > winH) y = winH - menuH - 10;

            menu.style.left = x + 'px';
            menu.style.top = y + 'px';
        }

        // Details Modal Logic
        showDetailsModal(fid) {
            // If fid is provided, we can fetch specific details.
            // For now, we use the data we have in this.downloads
            const item = this.downloads.find(d => d.fid === fid);
            if (!item) return;

            this.currentDetailFid = fid;
            document.getElementById('details-modal-title').textContent = item.name;
            document.getElementById('details-modal').style.display = 'flex';

            // Reset Tabs
            this.switchDetailsTab('files');

            // Populate Files (Mock for now or use item.files if available)
            const filesContainer = document.getElementById('details-content-files');
            if (item.files && item.files.length > 0) {
                filesContainer.innerHTML = item.files.map(f => `
                    <div style="padding: 0.5rem; border-bottom: 1px solid var(--border-color); display: flex; justify-content: space-between;">
                        <span style="font-size: 0.85rem;">${this.escapeHtml(f.name || item.name)}</span>
                        <span style="font-size: 0.8rem; font-family: 'Roboto Mono'; color: var(--text-muted);">${f.size || item.size}</span>
                    </div>
                 `).join('');
            } else {
                // Fallback if no specific file list
                filesContainer.innerHTML = `
                    <div style="padding: 0.5rem; border-bottom: 1px solid var(--border-color); display: flex; justify-content: space-between;">
                        <span style="font-size: 0.85rem;">${this.escapeHtml(item.name)}</span>
                        <span style="font-size: 0.8rem; font-family: 'Roboto Mono'; color: var(--text-muted);">${item.size}</span>
                    </div>`;
            }

            // Populate Log (Mock or use error message)
            const logContainer = document.getElementById('details-content-log');
            if (item.error_message) {
                logContainer.innerHTML = `<div style="color: var(--error); font-family: 'Roboto Mono'; font-size: 0.8rem;">${this.escapeHtml(item.error_message)}</div>`;
            } else {
                logContainer.innerHTML = `<div style="color: var(--text-muted); font-style: italic;">No recent errors.</div>`;
            }

            // Peers (Placeholder)
            document.getElementById('details-content-peers').innerHTML = `<div style="color: var(--text-muted);">Direct Download - No peers data via Fshare API.</div>`;
        }

        closeDetailsModal() {
            document.getElementById('details-modal').style.display = 'none';
            this.currentDetailFid = null;
        }

        switchDetailsTab(tabName) {
            // Tabs: files, peers, log
            ['files', 'peers', 'log'].forEach(t => {
                document.getElementById(`details-content-${t}`).style.display = (t === tabName) ? 'block' : 'none';
                const btn = document.getElementById(`tab-btn-${t}`);
                if (btn) {
                    if (t === tabName) {
                        btn.classList.add('active');
                        btn.style.borderBottom = '2px solid var(--primary)';
                        btn.style.color = 'var(--text-color)';
                    } else {
                        btn.classList.remove('active');
                        btn.style.borderBottom = '2px solid transparent';
                        btn.style.color = 'var(--text-muted)';
                    }
                }
            });
        }
    }

    // Initialize
    if (window.fshareBridgeInstance) {
        console.log('Refreshing existing Bridge instance...');
        window.fshareBridgeInstance.init();
    } else {
        document.addEventListener('DOMContentLoaded', () => {
            if (!window.fshareBridgeInstance) new FshareBridge();
        });
        if (document.readyState === 'complete' || document.readyState === 'interactive') {
            if (!window.fshareBridgeInstance) new FshareBridge();
        }
    }
}
