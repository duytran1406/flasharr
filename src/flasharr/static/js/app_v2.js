/**
 * Flasharr: Project Aurora Core
 * High-performance SINGLE PAGE APPLICATION logic.
 * Incorporates: Invisible Router, WebSocket Client, Modal System.
 */

if (typeof window.Socket === 'undefined') {
    window.Socket = class Socket {
        constructor(url) {
            this.url = url;
            this.events = {};
            this.connect();
        }
        connect() {
            this.ws = new WebSocket(this.url);
            this.ws.onopen = () => {
                if (this.events['open']) this.events['open'].forEach(cb => cb());
            };
            this.ws.onmessage = (e) => {
                const data = JSON.parse(e.data);
                if (this.events[data.type]) this.events[data.type].forEach(cb => cb(data.data));
            };
            this.ws.onclose = () => {
                if (this.events['close']) this.events['close'].forEach(cb => cb());
                setTimeout(() => this.connect(), 3000);
            };
        }
        on(type, cb) {
            if (!this.events[type]) this.events[type] = [];
            this.events[type].push(cb);
        }
    }
}

// Expose updateFilter globally to prevent 'not defined' errors with inline onclicks
window.updateDiscoverFilter = (key, val) => {
    if (window.router) window.router.updateFilter(key, val);
};

class Router {
    constructor() {
        this.ws = null;
        this.stats = {
            up: 0,
            down: 0,
            active: 0,
            total_active: 0
        };
        this.remainingQuotaGb = 0;
        this.omniSearchQuery = '';

        // Discover State
        this.discoverState = {
            type: 'movie',
            page: 1,
            sort: 'popularity.desc', // Jellyseerr default (Standard popularity)
            genre: '', // Comma separated IDs
            year: '',

            // Advanced Filters
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
            showFilters: false, // Hidden by default
            tmdbPage: 1, // TMDb's own pagination
            buffer: [] // Local buffer for partial pages
        };
    }

    get container() {
        return document.getElementById('view-container');
    }

    get navItems() {
        return document.querySelectorAll('.nav-item');
    }

    init() {
        // Global navigation interceptor
        this.navItems.forEach(item => {
            item.addEventListener('click', (e) => {
                e.preventDefault();
                const route = item.getAttribute('data-route');
                // Reset discover state on fresh navigation
                if (route === 'discover') {
                    this.discoverState.page = 1;
                    this.discoverState.hasMore = true;
                }
                this.navigate(route);
            });
        });

        // Global Shortcut: / to focus search
        window.addEventListener('keydown', (e) => {
            if (e.key === '/' && document.activeElement.tagName !== 'INPUT' && document.activeElement.tagName !== 'TEXTAREA') {
                e.preventDefault();
                document.getElementById('spotlight-search')?.focus();
            }
        });

        // Handle browser back/forward
        window.onpopstate = (e) => {
            if (e.state && e.state.view) {
                this.navigate(e.state.view, false);
            }
        };
    }

    connect() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const url = `${protocol}//${window.location.host}/ws`;
        console.log(`🔌 Connecting to ${url}`);

        this.ws = new Socket(url);

        this.ws.on('open', () => {
            const statusEl = document.getElementById('connection-status');
            if (statusEl) {
                statusEl.classList.remove('disconnected');
                statusEl.querySelector('.status-text').innerText = 'Neural Link Active';
            }
        });

        this.ws.on('close', () => {
            const statusEl = document.getElementById('connection-status');
            if (statusEl) {
                statusEl.classList.add('disconnected');
                statusEl.querySelector('.status-text').innerText = 'Neural Link Offline';
            }
        });

        this.ws.on('engine_stats', (stats) => {
            this.stats = stats;
            this.updateDashboardStats();
        });

        this.ws.on('account_status', (status) => {
            if (status.t) this.updateQuota(status.t);
            // Update other account info if needed
        });
    }

    navigate(view, addToHistory = true) {
        console.log(`🚀 Navigating to: ${view}`);

        // Update Title
        document.title = `Flasharr - ${view.charAt(0).toUpperCase() + view.slice(1)}`;

        // Update Sidebar UI
        this.navItems.forEach(item => {
            item.classList.toggle('active', item.dataset.route === view);
        });

        // Close any context menus
        this.closeContextMenu();

        // Push state
        if (addToHistory) {
            window.history.pushState({ view }, '', view === 'dashboard' ? '/' : `/${view}`);
            this.omniSearchQuery = ''; // Clear on manual navigation
        }

        // Set view attribute for CSS targeting
        this.container.setAttribute('data-view', view);

        // Render Dynamic Header
        this.renderDynamicHeader(view);

        // Render View
        if (view.startsWith('media/')) {
            const parts = view.split('/');
            this.loadMediaDetail(parts[1], parts[2]);
        } else {
            switch (view) {
                case 'dashboard':
                    this.loadDashboard();
                    this.updateCaptainInfo(null, true); // Force quota refresh
                    break;
                case 'discover': this.loadDiscover(); break;
                case 'downloads': this.loadDownloads(); break;
                case 'explore': this.loadExplore(); break;
                case 'settings':
                    this.loadSettings();
                    this.updateCaptainInfo(null, true); // Force quota refresh
                    break;
                default: this.loadDashboard(); break;
            }
        }
    }

    renderDynamicHeader(view) {
        const header = document.getElementById('header-dynamic-content');
        if (!header) return;

        let placeholder = "Search data fragments...";
        let icon = "search";
        let mode = "global";
        let extraButton = '';

        if (view === 'downloads') {
            placeholder = "Filter missions by ID or Name...";
            icon = "filter_list";
            mode = "filter";
            extraButton = `<button class="add-btn" onclick="window.router.showPromptAdd()" style="padding: 0.5rem 1rem; border-radius: 8px; font-size: 0.7rem; font-weight: 800; margin-left: 1rem; white-space: nowrap;">
                <span class="material-icons" style="font-size: 14px; margin-right: 4px;">add</span>NEW MISSION
            </button>`;
        } else if (view === 'settings') {
            placeholder = "Locate system parameter...";
            icon = "manage_search";
            mode = "locate";
        } else if (view === 'discover') {
            placeholder = "Search Movies & TV Series directly...";
            icon = "movie_filter";
            mode = "search";
        }

        header.innerHTML = `
            <div style="display: flex; align-items: center; gap: 0.75rem; width: 100%;">
                <div class="search-bar-header" style="flex: 1;">
                    <span class="material-icons">${icon}</span>
                    <input type="text" id="spotlight-search" placeholder="${placeholder}" autocomplete="off">
                </div>
                ${extraButton}
            </div>
        `;

        const input = document.getElementById('spotlight-search');
        if (!input) return;

        input.oninput = (e) => {
            const q = e.target.value.trim().toLowerCase();
            this.omniSearchQuery = q;
            if (mode === 'filter') this.refreshDownloads();
        };

        input.onkeypress = (e) => {
            if (e.key === 'Enter') {
                const q = e.target.value.trim();
                if (!q) return;

                if (mode === 'global') {
                    if (view !== 'explore') {
                        this.navigate('explore');
                        setTimeout(() => this.executeExploreSearch(q), 100);
                    } else {
                        this.executeExploreSearch(q);
                    }
                } else if (mode === 'search' && view === 'discover') {
                    this.executeTMDBSearch(q);
                } else if (mode === 'locate') {
                    this.findSetting(q);
                }
            }
        };
    }

    filterDownloads(query) {
        const rows = document.querySelectorAll('#download-list .download-row');
        rows.forEach(row => {
            const text = row.innerText.toLowerCase();
            row.style.display = text.includes(query) ? '' : 'none';
        });
    }

    findSetting(query) {
        const container = this.container;
        const labels = container.querySelectorAll('label, h2, h3');
        let match = null;

        for (const el of labels) {
            if (el.innerText.toLowerCase().includes(query.toLowerCase())) {
                match = el;
                break;
            }
        }

        if (match) {
            match.scrollIntoView({ behavior: 'smooth', block: 'center' });
            const parent = match.closest('.glass-panel') || match;
            parent.classList.add('highlight-setting');
            setTimeout(() => parent.classList.remove('highlight-setting'), 3000);
        }
    }

    toggleSidebar() {
        document.body.classList.toggle('sidebar-collapsed');
        const icon = document.getElementById('collapse-icon');
        if (icon) {
            icon.textContent = document.body.classList.contains('sidebar-collapsed') ? 'menu' : 'menu_open';
        }
    }

    // --- MODAL SYSTEM ---

    modalShow({ title, body, footer, onConfirm }) {
        const overlay = document.getElementById('modal-overlay');
        const container = document.getElementById('modal-container');

        container.innerHTML = `
            <div class="modal-header">
                <h3>${title}</h3>
                <button class="icon-btn-tiny" onclick="window.router.modalHide()"><span class="material-icons">close</span></button>
            </div>
            <div class="modal-body">${body}</div>
            <div class="modal-footer">${footer || `
                <button class="modal-btn secondary" onclick="window.router.modalHide()">Close</button>
            `}</div>
        `;

        overlay.classList.add('active');
        container.classList.add('active');
    }

    modalHide() {
        document.getElementById('modal-overlay').classList.remove('active');
        document.getElementById('modal-container').classList.remove('active');
    }

    copyToClipboard(text, label = 'URL') {
        navigator.clipboard.writeText(text).then(() => {
            // Optional: Show a subtle toast or notification if we had one
            console.log(`${label} copied to clipboard`);
        });
    }

    showError(msg) {
        this.modalShow({
            title: 'System Error',
            body: `<div style="color: #FF5252; display: flex; align-items: center; gap: 1rem;">
                <span class="material-icons" style="font-size: 48px;">error_outline</span>
                <p>${msg}</p>
            </div>`,
            footer: '<button class="modal-btn primary" onclick="window.router.modalHide()">Acknowledged</button>'
        });
    }

    showPromptAdd() {
        this.modalShow({
            title: 'Initiate Link Extraction',
            body: `
                <p style="margin-bottom: 1rem; color: var(--text-secondary);">Enter Fshare.vn file or folder URL to begin processing.</p>
                <input type="text" id="add-url-input" class="modal-input" placeholder="https://www.fshare.vn/file/..." autofocus>
            `,
            footer: `
                <button class="modal-btn secondary" onclick="window.router.modalHide()">Cancel</button>
                <button class="modal-btn primary" id="confirm-add-btn">Start Download</button>
            `
        });

        const input = document.getElementById('add-url-input');
        input.focus();
        input.addEventListener('keypress', (e) => {
            if (e.key === 'Enter') this.handleTaskAdd(input.value);
        });
        document.getElementById('confirm-add-btn').onclick = () => this.handleTaskAdd(input.value);
    }

    async handleTaskAdd(url) {
        if (!url) return;
        this.modalHide();
        try {
            const res = await fetch('/api/downloads', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ url })
            });
            const data = await res.json();
            if (data.status === 'ok') {
                if (window.location.pathname.includes('downloads')) this.loadDownloads();
                else this.navigate('downloads');
            } else {
                this.showError(data.message || 'Failed to add task');
            }
        } catch (e) {
            this.showError('Network error while adding task');
        }
    }

    showTaskInfo(task) {
        this.modalShow({
            title: 'Task Intelligence',
            body: `
                <div class="info-grid">
                    <div class="info-item">
                        <span class="info-label">Filename</span>
                        <span class="info-value" style="word-break: break-all;">${task.filename}</span>
                    </div>
                    <div class="info-item">
                        <span class="info-label">Status</span>
                        <span class="info-value" style="color: var(--color-primary)">${task.state}</span>
                    </div>
                    <div class="info-item">
                        <span class="info-label">Progress</span>
                        <div style="flex: 1;">
                            <div style="display: flex; justify-content: space-between; font-size: 0.7rem; margin-bottom: 4px;">
                                <span>${task.progress}%</span>
                                <span>${task.size.formatted_total}</span>
                            </div>
                            <div style="height: 6px; background: rgba(255,255,255,0.05); border-radius: 3px; overflow: hidden;">
                                <div style="height: 100%; width: ${task.progress}%; background: var(--color-primary); box-shadow: 0 0 10px var(--color-primary)"></div>
                            </div>
                        </div>
                    </div>
                    <div class="info-item">
                        <span class="info-label">Source Link</span>
                        <span class="info-value"><a href="${task.url}" target="_blank" style="color: var(--color-secondary); text-decoration: none;">View on Fshare</a></span>
                    </div>
                    <div class="info-item">
                        <span class="info-label">ID</span>
                        <span class="info-value" style="font-family: var(--font-mono); font-size: 0.7rem;">${task.id}</span>
                    </div>
                </div>
            `,
            footer: `
                <button class="modal-btn secondary" onclick="window.router.modalHide()">Close</button>
                <button class="modal-btn primary" onclick="window.router.taskAction('${task.id}', 'start')">Resume</button>
            `
        });
    }

    showConnectAccount() {
        this.modalShow({
            title: 'Connect Fshare Account',
            body: `
                <div style="display: flex; flex-direction: column; gap: 1rem;">
                    <div class="form-group">
                        <label>Fshare Email</label>
                        <input type="email" id="acc-email" class="modal-input" placeholder="email@example.com">
                    </div>
                    <div class="form-group">
                        <label>Password</label>
                        <input type="password" id="acc-pass" class="modal-input" placeholder="••••••••">
                    </div>
                </div>
            `,
            footer: `
                <button class="modal-btn secondary" onclick="window.router.modalHide()">Cancel</button>
                <button class="modal-btn primary" id="confirm-connect-btn">Authenticate</button>
            `
        });

        document.getElementById('confirm-connect-btn').onclick = async () => {
            const email = document.getElementById('acc-email').value;
            const password = document.getElementById('acc-pass').value;
            if (!email || !password) return;

            try {
                const res = await fetch('/api/accounts/add', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ email, password })
                });
                const data = await res.json();
                if (data.status === 'ok') {
                    this.modalHide();
                    this.fetchSettingsData();
                } else {
                    alert('Authentication failed: ' + data.message);
                }
            } catch (e) { alert('Network error'); }
        };
    }


    // --- DOWNLOADS MODULE ---

    loadDownloads() {
        this.container.innerHTML = `
            <div class="glass-panel" style="padding: 0; overflow: hidden; display: flex; flex-direction: column; height: 75vh;">
                <!-- Toolbar -->
                <div style="padding: 1rem 1.5rem; border-bottom: 1px solid rgba(255,255,255,0.05); display: flex; justify-content: space-between; align-items: center;">
                    <h2 class="glow-text" style="font-size: 1rem; text-transform: uppercase;">Active Transmissions</h2>
                    <div style="display: flex; gap: 0.5rem;">
                        <button class="icon-btn-tiny" onclick="window.router.loadDownloads()" title="Refresh">
                            <span class="material-icons">refresh</span>
                        </button>
                    </div>
                </div>

                <!-- Table Container with fixed height for ~12 items -->
                <div style="flex: 1; overflow: hidden; padding: 0; position: relative;">
                    <div style="position: absolute; top: 0; left: 0; right: 0; bottom: 0; overflow-y: auto;">
                        <table class="data-table" style="width: 100%; border-collapse: collapse;">
                            <thead style="position: sticky; top: 0; background: rgba(15, 23, 42, 0.95); z-index: 10; backdrop-filter: blur(10px);">
                                <tr style="font-size: 0.65rem; font-weight: 800; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em; border-bottom: 1px solid rgba(255,255,255,0.03);">
                                    <th style="padding: 0.6rem 1.25rem; text-align: left; width: 35%;">Resource Identity</th>
                                <th style="padding: 0.6rem 0.5rem; text-align: left; width: 10%;">Status</th>
                                <th style="padding: 0.6rem 0.5rem; text-align: left; width: 8%;">Size</th>
                                <th style="padding: 0.6rem 0.5rem; text-align: left; width: 15%;">Progress</th>
                                <th style="padding: 0.6rem 0.5rem; text-align: left; width: 10%;">Speed</th>
                                <th style="padding: 0.6rem 0.5rem; text-align: left; width: 8%;">ETA</th>
                                <th style="padding: 0.6rem 0.5rem; text-align: left; width: 10%;">Added</th>
                                <th style="padding: 0.6rem 1.25rem; text-align: right; width: 4%;">...</th>
                            </tr>
                        </thead>
                        <tbody id="download-list">
                            <tr><td colspan="8" style="text-align: center; padding: 4rem;"><div class="loading-spinner"></div></td></tr>
                        </tbody>
                    </table>
                </div>

                <div style="padding: 0.5rem 1.25rem; display: flex; justify-content: flex-end; border-top: 1px solid rgba(255,255,255,0.03); background: rgba(0,0,0,0.15);">
                    <div id="downloads-pagination" class="pagination-coordinator"></div>
                </div>
            </div>
        `;
        this.downloadPage = 1;
        this.downloadLimit = 10;
        this.refreshDownloads();
        this.startDownloadPolling();
    }

    startDownloadPolling() {
        if (this.dlInterval) clearInterval(this.dlInterval);
        this.dlInterval = setInterval(() => {
            if (window.history.state && window.history.state.view === 'downloads') this.refreshDownloads(false);
            else clearInterval(this.dlInterval);
        }, 2000);
    }

    async refreshDownloads(showInitialLoading = true) {
        const listEl = document.getElementById('download-list');
        if (!listEl) return;

        try {
            const res = await fetch('/api/downloads');
            const data = await res.json();
            const tasks = (data.downloads || []).filter(t => {
                if (!this.omniSearchQuery) return true;
                const q = this.omniSearchQuery.toLowerCase();
                return t.filename.toLowerCase().includes(q) || t.id.toLowerCase().includes(q);
            }).sort((a, b) => {
                const dateA = new Date(a.added);
                const dateB = new Date(b.added);
                return dateB - dateA;
            });

            // Client-side pagination
            const total = tasks.length;
            const totalPages = Math.ceil(total / this.downloadLimit) || 1;
            if (this.downloadPage > totalPages) this.downloadPage = totalPages;

            const start = (this.downloadPage - 1) * this.downloadLimit;
            const paginatedTasks = tasks.slice(start, start + this.downloadLimit);

            this.renderDownloadList(paginatedTasks, listEl);
            this.renderDownloadsPagination(this.downloadPage, totalPages, total);
        } catch (e) {
            listEl.innerHTML = `<tr><td colspan="8" style="padding: 2rem; color: #ff5252; text-align: center;">Telemetric link failure: ${e.message}</td></tr>`;
        }
    }

    renderDownloadsPagination(current, total, totalItems) {
        const containers = [document.getElementById('downloads-pagination')];
        containers.forEach(el => {
            if (!el) return;
            el.innerHTML = `
                <div style="display: flex; gap: 1.5rem; align-items: center;">
                    <span class="page-info" style="font-size: 0.7rem; color: var(--text-muted); letter-spacing: 0.1em; font-weight: 700;">REGISTRY: ${totalItems} ENTITIES</span>
                    <div style="display: flex; gap: .5rem; background: rgba(255,255,255,0.03); padding: 4px; border-radius: 8px; border: 1px solid rgba(255,255,255,0.05);">
                        <button class="icon-btn-tiny" onclick="window.router.setDownloadPage(${current - 1})" ${current === 1 ? 'disabled' : ''} style="background: transparent;">
                            <span class="material-icons" style="font-size: 16px;">chevron_left</span>
                        </button>
                        <div style="display: flex; align-items: center; padding: 0 .5rem; font-weight: 800; font-size: 0.7rem; color: var(--color-primary); font-family: var(--font-mono);">${current} / ${total}</div>
                        <button class="icon-btn-tiny" onclick="window.router.setDownloadPage(${current + 1})" ${current === total ? 'disabled' : ''} style="background: transparent;">
                            <span class="material-icons" style="font-size: 16px;">chevron_right</span>
                        </button>
                    </div>
                </div>
            `;
        });
    }

    setDownloadPage(page) {
        this.downloadPage = page;
        this.refreshDownloads();
    }

    renderDownloadList(tasks, body) {
        if (!tasks || tasks.length === 0) {
            body.innerHTML = `<tr><td colspan="8" style="padding: 4rem; text-align: center; color: var(--text-muted);">
                <span class="material-icons" style="font-size: 48px; display: block; margin-bottom: 1rem; opacity: 0.2;">inbox</span>
                No active missions in queue.
            </td></tr>`;
            return;
        }

        body.innerHTML = tasks.map(t => {
            const state = t.state || 'Unknown';
            const displayState = state.charAt(0).toUpperCase() + state.slice(1);
            const isDownloading = state === 'Downloading' || state === 'Extracting' || state === 'Running' || state === 'Starting';
            const isCompleted = state === 'Completed' || state === 'Finished';
            const isError = state === 'Error' || state === 'Failed' || state === 'Offline';
            const isQueued = state === 'Queued' || state === 'Waiting' || state === 'Pending';

            // Color System
            const color = isCompleted ? '#00ffa3' : (isError ? '#FF5252' : (isDownloading ? '#00f3ff' : (isQueued ? '#ffd700' : '#64748b')));
            const icon = isCompleted ? 'check_circle' : (isError ? 'report_problem' : (isDownloading ? 'sync' : (isQueued ? 'hourglass_bottom' : 'pause_circle')));

            let addedDate = t.added || '-';
            if (typeof t.added === 'number' || (!isNaN(t.added) && t.added > 1000000)) {
                const d = new Date(parseInt(t.added) * 1000);
                addedDate = d.toLocaleDateString() + ' ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
            }

            return `
            <tr class="transfer-row" data-id="${t.id}" oncontextmenu="event.preventDefault(); window.router.showContextMenu(event, ${JSON.stringify(t).replace(/"/g, '&quot;')})" style="transition: background 0.2s; cursor: pointer;">
                <td style="padding: 0.5rem 1.25rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600;" title="${t.filename}" onclick="window.router.showTaskInfo(${JSON.stringify(t).replace(/"/g, '&quot;')})">
                    ${t.filename}
                </td>
                <td style="padding: 0.5rem 0.5rem;">
                    <span style="background: ${color}15; color: ${color}; padding: 2px 6px; border-radius: 4px; font-size: 0.55rem; font-weight: 800; border: 1px solid ${color}30; display: inline-flex; align-items: center; gap: 3px; text-transform: uppercase;">
                        <span class="material-icons" style="font-size: 9px;">${icon}</span> ${displayState}
                    </span>
                </td>
                <td style="padding: 0.5rem 0.5rem; color: var(--text-secondary); font-size: 0.7rem;">${t.size.formatted_total}</td>
                <td style="padding: 0.5rem 0.5rem;">
                    <div style="display: flex; justify-content: space-between; font-size: 0.55rem; margin-bottom: 2px; font-weight: 700; opacity: 0.8;">
                        <span>${t.progress}%</span>
                    </div>
                    <div style="height: 3px; background: rgba(255,255,255,0.05); border-radius: 2px; overflow: hidden;">
                        <div style="height: 100%; width: ${t.progress}%; background: ${color}; box-shadow: 0 0 6px ${color}80"></div>
                    </div>
                </td>
                <td style="padding: 0.5rem 0.5rem; color: ${color}; font-family: var(--font-mono); font-size: 0.7rem; font-weight: 700;">${isDownloading ? t.speed.formatted + '/s' : '-'}</td>
                <td style="padding: 0.5rem 0.5rem; color: var(--text-secondary); font-size: 0.7rem;">${isDownloading ? t.eta.formatted : '-'}</td>
                <td style="padding: 0.5rem 0.5rem; color: var(--text-muted); font-size: 0.65rem;">${addedDate}</td>
                <td style="padding: 0.5rem 1.25rem; text-align: right;">
                    <button class="icon-btn-tiny" onclick="event.stopPropagation(); window.router.showContextMenu(event, ${JSON.stringify(t).replace(/"/g, '&quot;')})">
                        <span class="material-icons" style="font-size: 14px;">more_vert</span>
                    </button>
                </td>
            </tr>
            `;
        }).join('');
    }

    async taskAction(id, action) {
        let url = `/api/downloads/${id}`;
        let method = 'POST';

        if (action === 'start') url += '/start';
        else if (action === 'pause') url += '/pause';
        else if (action === 'delete') method = 'DELETE';

        try {
            await fetch(url, { method });
            if (this.dlInterval) this.refreshDownloads();
        } catch (e) { alert('Operation failed'); }
    }


    // --- DISCOVER / JELLYSEERR UI ---

    async loadDiscover(type = 'movie') {
        this.discoverState.type = type;
        this.discoverState.page = 1;
        this.discoverState.tmdbPage = 1;
        this.discoverState.buffer = [];
        this.discoverState.hasMore = true;

        this.container.innerHTML = `
            <div class="discover-layout">
                <main class="discover-main">
                    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 2rem;">
                        <h2 class="glow-text" style="font-size: 1.5rem; font-weight: 800; display: flex; align-items: center; gap: 0.75rem;">
                            <span class="material-icons" style="color: var(--color-primary);">rocket_launch</span> 
                            Discovery
                        </h2>

                        <div style="display: flex; align-items: center;">
                            <div class="tab-container">
                                <button class="tab-btn ${this.discoverState.type === 'movie' ? 'active' : ''}" onclick="window.router.switchDiscoverType('movie')">Movies</button>
                                <button class="tab-btn ${this.discoverState.type === 'tv' ? 'active' : ''}" onclick="window.router.switchDiscoverType('tv')">TV Series</button>
                            </div>

                            <select onchange="window.router.setDiscoverSort(this.value)" class="sort-select-premium" style="margin-right: 1rem;">
                                <option value="popular_today" ${this.discoverState.sort === 'popular_today' ? 'selected' : ''}>Popular Today</option>
                                <option value="popularity.desc" ${this.discoverState.sort === 'popularity.desc' ? 'selected' : ''}>Popularity Descending</option>
                                <option value="popularity.asc" ${this.discoverState.sort === 'popularity.asc' ? 'selected' : ''}>Popularity Ascending</option>
                                <option value="primary_release_date.desc" ${this.discoverState.sort === 'primary_release_date.desc' ? 'selected' : ''}>Release Date Descending</option>
                                <option value="primary_release_date.asc" ${this.discoverState.sort === 'primary_release_date.asc' ? 'selected' : ''}>Release Date Ascending</option>
                                <option value="vote_average.desc" ${this.discoverState.sort === 'vote_average.desc' ? 'selected' : ''}>TMDB Rating Descending</option>
                                <option value="vote_average.asc" ${this.discoverState.sort === 'vote_average.asc' ? 'selected' : ''}>TMDB Rating Ascending</option>
                                <option value="original_title.asc" ${this.discoverState.sort === 'original_title.asc' ? 'selected' : ''}>Title (A-Z)</option>
                                <option value="original_title.desc" ${this.discoverState.sort === 'original_title.desc' ? 'selected' : ''}>Title (Z-A)</option>
                            </select>

                            <button class="filter-toggle-btn ${this.discoverState.showFilters ? 'active' : ''}" onclick="window.router.toggleSidebarFilters()" title="Transmission Filters">
                                <span class="material-icons">filter_list</span>
                            </button>
                        </div>
                    </div>

                     <!-- Scrollable Grid Container -->
                    <div id="discover-scroll-container" style="overflow-y: auto; flex: 1; padding-right: 10px; padding-bottom: 2rem;">
                         <div id="discover-grid" class="discover-grid"></div>
                         <div id="discover-sentinel" style="height: 20px; width: 100%;"></div>
                         <div id="discover-loading-more" style="display: none; justify-content: center; padding: 2rem;">
                             <div class="loading-spinner" style="width: 30px; height: 30px;"></div>
                         </div>
                    </div>
                </main>

                <!-- Right Sidebar (Filters) -->
                <div id="discover-sidebar" class="discover-sidebar custom-scrollbar" style="display: ${this.discoverState.showFilters ? 'block' : 'none'}; width: 320px; flex-shrink: 0; overflow-y: auto;">
                    <!-- Filter content rendered dynamically -->
                </div>

           </div>
        `;

        this.setupInfiniteScroll();
        await this.fetchGenres(type); // Will also trigger render of sidebar genres
        this.fetchDiscoverData(true);
    }

    renderSidebarContent() {
        const s = this.discoverState;
        return `
             <div class="filter-section">
                <span class="filter-label">Media Identity</span>
                <div class="glass-panel" style="display: flex; gap: 4px; padding: 4px; border-radius: 12px; background: rgba(0,0,0,0.2);">
                    <button class="btn-toggle ${s.type === 'movie' ? 'active' : ''}" onclick="window.router.loadDiscover('movie')" style="flex: 1;">Movies</button>
                    <button class="btn-toggle ${s.type === 'tv' ? 'active' : ''}" onclick="window.router.loadDiscover('tv')" style="flex: 1;">TV Series</button>
                </div>
            </div>

            <div class="filter-section">
                <span class="filter-label">Release Date</span>
                <div class="filter-input-group">
                    <input type="date" class="filter-input-date" placeholder="From" value="${s.dateFrom}" onchange="window.router.updateFilter('dateFrom', this.value)">
                    <input type="date" class="filter-input-date" placeholder="To" value="${s.dateTo}" onchange="window.router.updateFilter('dateTo', this.value)">
                </div>
            </div>

            <div class="filter-section">
                <span class="filter-label">Genres</span>
                <div id="genre-list" class="genre-list"></div>
            </div>
            
            <div class="filter-section">
                <span class="filter-label">Runtime</span>
                <div class="filter-range-container">
                    <input type="range" min="0" max="400" value="${s.runtimeMax}" onchange="window.router.updateFilter('runtimeMax', this.value)">
                    <div style="font-size: 0.75rem; color: var(--text-muted); text-align: center;">0 - ${s.runtimeMax} minutes</div>
                </div>
            </div>

             <div class="filter-section">
                <span class="filter-label">TMDB User Score</span>
                <div class="filter-range-container">
                    <input type="range" min="0" max="10" step="0.1" value="${s.scoreMin}" onchange="window.router.updateFilter('scoreMin', this.value)">
                     <div style="font-size: 0.75rem; color: var(--text-muted); text-align: center;">${s.scoreMin} - 10</div>
                </div>
            </div>
            
            <div class="filter-section">
                 <button class="modal-btn secondary" style="width: 100%;" onclick="window.router.clearFilters()">
                    <span class="material-icons" style="font-size: 16px;">close</span> Clear Active Filters
                 </button>
            </div>
       `;
    }

    updateFilter(key, value) {
        this.discoverState[key] = value;
        // Re-render sidebar to reflect state (like active pills)
        const sidebar = document.getElementById('discovery-sidebar');
        if (sidebar) sidebar.innerHTML = this.renderSidebarContent();

        // Re-fetch genres to keep list populated (sidebar innerHTML wipe)
        this.getGenresFromCache(this.discoverState.type);

        // Reset and fetch
        this.resetAndFetch();
    }

    clearFilters() {
        Object.assign(this.discoverState, {
            dateFrom: '', dateTo: '', language: '', certification: '',
            runtimeMin: 0, runtimeMax: 400, scoreMin: 0, scoreMax: 10, voteCountMin: 0, genre: '', year: ''
        });
        this.updateFilter('genre', ''); // Triggers refresh
    }

    // Slight mod to re-use cached genres
    getGenresFromCache(type) {
        // This assumes genres are loaded into this.genres by fetchGenres previously
        // We need to just re-render the genre list div
        this.renderGenres(this.genres || []);
    }

    resetAndFetch() {
        this.discoverState.page = 1;
        this.discoverState.tmdbPage = 1;
        this.discoverState.buffer = []; // Corrected reference
        this.discoverState.hasMore = true;
        this.fetchDiscoverData(true);
    }

    async fetchGenres(type) {
        try {
            const res = await fetch(`/api/tmdb/genres?type=${type}`);
            const data = await res.json();
            this.genres = data.genres || []; // Cache for re-renders
            this.renderGenres(this.genres);
        } catch (e) { console.error(e); }
    }

    renderGenres(genres) {
        const container = document.getElementById('genre-list');
        if (!container) return;

        container.innerHTML = genres.map(g => `
            <div class="genre-chip ${this.discoverState.genre == g.id ? 'active' : ''}" 
                 onclick="window.router.updateFilter('genre', '${this.discoverState.genre == g.id ? '' : g.id}')">
                ${g.name}
            </div>
        `).join('');
    }

    toggleSidebarFilters() {
        this.discoverState.showFilters = !this.discoverState.showFilters;
        const sidebar = document.getElementById('discovery-sidebar');
        const btn = document.querySelector('.filter-toggle-btn');
        if (sidebar) sidebar.classList.toggle('collapsed', !this.discoverState.showFilters);
        if (btn) btn.classList.toggle('active', this.discoverState.showFilters);
    }

    setDiscoverType(type) {
        this.discoverState.type = type;
        this.discoverState.page = 1;
        this.discoverState.hasMore = true;
        this.loadDiscover(type);
    }

    switchDiscoverType(type) {
        if (this.discoverState.type === type) return;
        this.discoverState.type = type;
        this.discoverState.genre = '';
        this.discoverState.page = 1;
        this.discoverState.hasMore = true;
        // Reset sort to Popular Today if needed or keep existing
        this.loadDiscover(type); // Re-render logic with new tabs state
    }

    setDiscoverSort(sort) {
        this.discoverState.sort = sort;
        this.resetAndFetch();
    }

    setDiscoverFilter(key, value) {
        this.discoverState[key] = value;
        this.discoverState.page = 1;
        this.discoverState.hasMore = true;
        this.fetchDiscoverData(true);
    }

    setupInfiniteScroll() {
        const sentinel = document.getElementById('discover-sentinel');
        if (!sentinel) return;

        if (this.discoverObserver) this.discoverObserver.disconnect();

        this.discoverObserver = new IntersectionObserver((entries) => {
            if (entries[0].isIntersecting && !this.discoverState.loading && this.discoverState.hasMore) {
                console.log("👀 Sentinel Intersected: Pulling next data fragment...");
                this.fetchDiscoverData(false);
            }
        }, {
            root: document.getElementById('discover-scroll-container'),
            rootMargin: '400px', // Pre-fetch before user reaches the bottom
            threshold: 0.1
        });

        this.discoverObserver.observe(sentinel);
    }

    async fetchDiscoverData(reset = false) {
        if (this.discoverState.loading) return;
        this.discoverState.loading = true;

        if (reset) {
            document.getElementById('discover-grid').innerHTML = '<div class="loading-spinner"></div>';
            this.discoverState.page = 1;
            this.discoverState.tmdbPage = 1;
            this.discoverState.buffer = [];
            this.discoverState.hasMore = true;
        } else {
            const loader = document.getElementById('discover-loading-more');
            if (loader) loader.style.display = 'block';
        }

        const itemsPerPage = 18;
        const s = this.discoverState;

        try {
            // Fill buffer up to 18 items if we have more results from TMDB
            while (s.buffer.length < itemsPerPage && s.hasMore) {
                const params = new URLSearchParams({
                    page: s.tmdbPage,
                    sort_by: s.sort,
                    genre: s.genre || '',
                    year: s.year || '',
                    date_from: s.dateFrom,
                    date_to: s.dateTo,
                    language: s.language,
                    certification: s.certification,
                    runtime_min: s.runtimeMin,
                    runtime_max: s.runtimeMax,
                    score_min: s.scoreMin,
                    score_max: s.scoreMax,
                    vote_count_min: s.voteCountMin
                });

                let endpoint = `/api/tmdb/discover/${s.type}?${params.toString()}`;
                if (s.sort === 'popular_today') {
                    endpoint = `/api/discovery/popular-today?page=${s.tmdbPage}`;
                }

                const res = await fetch(endpoint);
                const data = await res.json();

                if (data.results && data.results.length > 0) {
                    s.buffer.push(...data.results);
                    s.tmdbPage++;
                    // TMDb returns 20 items per page usually
                    if (data.results.length < 20) {
                        // If we got less than full page, no more items on TMDB
                        // But we might still have some in buffer
                    }
                } else {
                    s.hasMore = false;
                }
            }

            // Slice 18 items from buffer
            const chunk = s.buffer.splice(0, itemsPerPage);
            if (chunk.length > 0) {
                this.renderDiscoverGrid(chunk, s.type, !reset);
                s.page++;
            }

            // If buffer and API are both empty, we're done
            if (s.buffer.length === 0 && !s.hasMore) {
                s.hasMore = false;
            } else {
                s.hasMore = true; // Still have items in buffer or API
            }

        } catch (e) {
            console.error("Discover Telemetry Error", e);
        } finally {
            this.discoverState.loading = false;
            const loader = document.getElementById('discover-loading-more');
            if (loader) loader.style.display = 'none';
        }
    }

    renderDiscoverGrid(items, type, append = false, targetNode = null) {
        const grid = targetNode || document.getElementById('discover-grid');
        if (!grid) return;

        const html = items.map(item => {
            const mediaType = item.media_type || type;
            const title = item.title || item.name;
            const date = item.release_date || item.first_air_date || '';
            const year = date.split('-')[0];
            const poster = item.poster_path ? `https://image.tmdb.org/t/p/w500${item.poster_path}` : '/static/images/placeholder_poster.jpg';

            return `
                 <div class="poster-card" onclick="window.router.navigate('media/${mediaType}/${item.id}')">
                      <div class="poster-image" style="background-image: url('${poster}');">
                           <div class="poster-tags" style="position: absolute; top: 8px; left: 8px; display: flex; flex-direction: column; gap: 4px;">
                                ${this.renderTags(item)}
                           </div>
                      </div>
                      <div class="poster-overlay">
                           <div class="poster-title">${title}</div>
                           <div class="poster-meta">${year} • <span class="material-icons" style="font-size: 10px; color: #ffd700;">star</span> ${(item.vote_average || 0).toFixed(1)}</div>
                      </div>
                 </div>
             `;
        }).join('');

        if (append) {
            grid.insertAdjacentHTML('beforeend', html);
        } else {
            grid.innerHTML = html;
        }
    }

    async executeTMDBSearch(q) {
        const grid = document.getElementById('discover-grid');
        const header = document.querySelector('.discover-grid-container h2') || document.querySelector('h2');
        if (header) header.innerHTML = `<span class="material-icons" style="color: var(--color-primary);">search</span> Search Results: ${q}`;

        if (grid) grid.innerHTML = '<div class="loading-spinner"></div>';

        try {
            const res = await fetch(`/api/search?q=${encodeURIComponent(q)}`);
            const data = await res.json();
            if (data.results) {
                this.renderDiscoverGrid(data.results, 'movie');
            }
        } catch (e) {
            if (grid) grid.innerHTML = '<div style="color: #FF5252; text-align: center; padding: 2rem;">Search Telemetry Fragmented</div>';
        }
    }

    async loadMediaDetail(type, id) {
        this.container.innerHTML = `
            <div style="max-width: 1400px; margin: 0 auto; display: flex; flex-direction: column; gap: 0; min-height: calc(100vh - 80px); overflow: hidden; position: relative;">
                <div id="detail-hero" style="height: 400px; background-size: cover; background-position: center; position: relative; border-radius: 0 0 24px 24px; overflow: hidden; border-bottom: 2px solid rgba(255,255,255,0.05);">
                    <div style="position: absolute; inset: 0; background: linear-gradient(to bottom, rgba(15,23,42,0.1) 0%, rgba(15,23,42,1) 100%);"></div>
                    <div style="position: absolute; bottom: 2rem; left: 2rem; display: flex; gap: 2rem; align-items: flex-end; width: calc(100% - 4rem);">
                        <div class="glass-panel" style="width: 200px; height: 300px; border-radius: 12px; overflow: hidden; box-shadow: 0 20px 50px rgba(0,0,0,0.8); border: 2px solid rgba(255,255,255,0.1); flex-shrink: 0; background: #1e293b;">
                             <img id="detail-poster" src="" style="width: 100%; height: 100%; object-fit: cover;">
                        </div>
                        <div style="flex: 1; min-width: 0; margin-bottom: 1rem;">
                            <div id="detail-tagline" style="font-size: 0.8rem; font-weight: 800; color: var(--color-primary); text-transform: uppercase; letter-spacing: 0.2rem; margin-bottom: 0.5rem; font-family: var(--font-mono);"></div>
                            <h1 id="detail-title" style="font-size: 3rem; font-weight: 800; margin-bottom: 0.5rem; text-shadow: 0 2px 10px rgba(0,0,0,0.5); line-height: 1.1;"></h1>
                            <div style="display: flex; align-items: center; gap: 1rem; color: var(--text-muted); font-size: 0.8rem; margin-bottom: 1.5rem; font-family: var(--font-mono); flex-wrap: wrap;">
                                <span id="detail-year"></span>
                                <span>•</span>
                                <span style="display: flex; align-items: center; gap: 4px; color: #ffd700;"><span class="material-icons" style="font-size: 14px;">star</span> <span id="detail-score"></span></span>
                                <span>•</span>
                                <span id="detail-runtime"></span>
                                <span>•</span>
                                <span id="detail-genres" style="color: var(--text-secondary);"></span>
                                <span>•</span>
                                <span id="detail-type" style="text-transform: uppercase; border: 1px solid rgba(255,255,255,0.1); padding: 2px 8px; border-radius: 4px; font-size: 0.65rem;"></span>
                            </div>
                        </div>
                    </div>
                </div>
                
                <div style="padding: 2.5rem 2rem; display: grid; grid-template-columns: minmax(0, 1fr) 350px; gap: 3rem;">
                    <div style="min-width: 0;">
                        <h3 style="text-transform: uppercase; letter-spacing: 0.1em; font-size: 0.75rem; color: var(--color-primary); margin-bottom: 1rem; font-weight: 800;">Intelligence Brief</h3>
                        <p id="detail-overview" style="line-height: 1.8; color: var(--text-secondary); font-size: 1.1rem; margin-bottom: 3rem;"></p>
                        
                        <div id="tv-seasons-section" style="display: none;">
                            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem;">
                                <h3 style="text-transform: uppercase; letter-spacing: 0.1em; font-size: 0.75rem; color: var(--color-primary); font-weight: 800;">Mission Phases (Seasons)</h3>
                                <select id="season-selector" class="glass-panel" style="padding: 0.5rem 1rem; background: rgba(255,255,255,0.05); color: #fff; border: 1px solid rgba(255,255,255,0.1); border-radius: 8px; font-size: 0.8rem;"></select>
                            </div>
                            <div id="episodes-list" style="display: flex; flex-direction: column; gap: 1rem;"></div>
                        </div>

                        <!-- Similar & Recommended Sections -->
                        <div id="related-media-section" style="margin-top: 4rem;">
                            <div id="similar-section" style="margin-bottom: 3rem;">
                                <h3 style="text-transform: uppercase; letter-spacing: 0.1em; font-size: 0.75rem; color: var(--color-primary); margin-bottom: 1.5rem; font-weight: 800;">Similar Trajectories</h3>
                                <div id="similar-grid" class="discover-grid"></div>
                            </div>
                            <div id="recommended-section">
                                <h3 style="text-transform: uppercase; letter-spacing: 0.1em; font-size: 0.75rem; color: var(--color-primary); margin-bottom: 1.5rem; font-weight: 800;">Neural Recommendations</h3>
                                <div id="recommended-grid" class="discover-grid"></div>
                            </div>
                        </div>
                    </div>
                    
                    <div style="display: flex; flex-direction: column; gap: 1.5rem;">
                        <div id="movie-actions" class="glass-panel" style="padding: 1.5rem; display: none;">
                            <h3 style="font-size: 0.8rem; font-weight: 800; margin-bottom: 1rem; text-transform: uppercase;">Direct Retrieval</h3>
                            <button id="btn-smart-dl-movie" class="add-btn" style="width: 100%; padding: 1rem; display: flex; align-items: center; justify-content: center; gap: 0.75rem;">
                                <span class="material-icons">manage_search</span> SMART SEARCH
                            </button>
                        </div>
                        
                        <div class="glass-panel" style="padding: 1.5rem;">
                            <h3 style="font-size: 0.8rem; font-weight: 800; margin-bottom: 1.5rem; text-transform: uppercase; border-bottom: 1px solid rgba(255,255,255,0.05); padding-bottom: 0.75rem;">Registry Info</h3>
                            <div style="display: flex; flex-direction: column; gap: 1.25rem;">
                                <div>
                                    <div style="font-size: 0.65rem; color: var(--text-muted); text-transform: uppercase; font-weight: 800; margin-bottom: 0.25rem;">Operational Status</div>
                                    <div id="side-status" style="font-weight: 700; color: #00ff88;">-</div>
                                </div>
                                <div>
                                    <div style="font-size: 0.65rem; color: var(--text-muted); text-transform: uppercase; font-weight: 800; margin-bottom: 0.25rem;">Network / Studio</div>
                                    <div id="side-network" style="font-weight: 700;">-</div>
                                </div>
                                <div>
                                    <div style="font-size: 0.65rem; color: var(--text-muted); text-transform: uppercase; font-weight: 800; margin-bottom: 0.25rem;">Content Score</div>
                                    <div style="display: flex; align-items: center; gap: 8px;">
                                        <div id="side-score-avg" style="font-size: 1.25rem; font-weight: 800; color: #ffd700;">0.0</div>
                                        <div id="side-score-count" style="font-size: 0.7rem; color: var(--text-muted);">0 votes</div>
                                    </div>
                                </div>
                                <div>
                                    <div style="font-size: 0.65rem; color: var(--text-muted); text-transform: uppercase; font-weight: 800; margin-bottom: 0.25rem;">Temporal Offset</div>
                                    <div id="side-runtime" style="font-weight: 700;">-</div>
                                </div>
                            </div>
                        </div>

                        <div id="side-keywords" style="display: flex; flex-wrap: wrap; gap: 0.5rem; padding: 0 0.5rem;"></div>
                    </div>
                </div>
            </div>
        `;

        // Initialize Placeholders
        document.getElementById('detail-hero').style.backgroundImage = 'none'; // Or a loading gradient
        document.getElementById('detail-poster').src = '/static/images/placeholder_poster.jpg';

        try {
            const res = await fetch(`/api/tmdb/${type}/${id}`);
            const data = await res.json();

            // Fill basics
            document.getElementById('detail-hero').style.backgroundImage = data.backdrop_path ? `url('https://image.tmdb.org/t/p/original${data.backdrop_path}')` : 'none';
            document.getElementById('detail-poster').src = data.poster_path ? `https://image.tmdb.org/t/p/w500${data.poster_path}` : '/static/images/placeholder_poster.jpg';
            document.getElementById('detail-tagline').innerText = data.tagline || '';
            document.getElementById('detail-title').innerText = data.title || data.name;
            document.getElementById('detail-overview').innerText = data.overview;

            const release = (data.release_date || data.first_air_date || '');
            document.getElementById('detail-year').innerText = release.split('-')[0];
            document.getElementById('detail-score').innerText = (data.vote_average || 0).toFixed(1);

            // Runtime / Seasons
            let runtimeText = '';
            if (type === 'movie' && data.runtime) {
                const hrs = Math.floor(data.runtime / 60);
                const mins = data.runtime % 60;
                runtimeText = hrs > 0 ? `${hrs}h ${mins}m` : `${mins}m`;
            } else if (type === 'tv' && data.episode_run_time && data.episode_run_time.length > 0) {
                runtimeText = `${data.episode_run_time[0]} min/ep`;
            } else if (type === 'tv' && data.number_of_seasons) {
                runtimeText = `${data.number_of_seasons} ${data.number_of_seasons === 1 ? 'Season' : 'Seasons'}`;
            }
            document.getElementById('detail-runtime').innerText = runtimeText;

            // Genres
            const genresHtml = (data.genres || []).slice(0, 3).map(g => `
                <span onclick="event.stopPropagation(); window.router.navigateToGenre('${g.id}', '${type}')" style="cursor: pointer; text-decoration: underline; text-decoration-color: rgba(255,255,255,0.2);">${g.name}</span>
            `).join(' / ');
            document.getElementById('detail-genres').innerHTML = genresHtml;

            document.getElementById('detail-type').innerText = type;

            // Sidebar Info
            document.getElementById('side-status').innerText = data.status || 'Active';
            document.getElementById('side-network').innerText = data.networks?.[0]?.name || data.production_companies?.[0]?.name || 'Unknown Registry';
            document.getElementById('side-score-avg').innerText = (data.vote_average || 0).toFixed(1);
            document.getElementById('side-score-count').innerText = `${data.vote_count?.toLocaleString()} samples`;
            document.getElementById('side-runtime').innerText = runtimeText;

            const keywords = data.keywords?.keywords || data.keywords?.results || [];
            document.getElementById('side-keywords').innerHTML = keywords.slice(0, 10).map(k => `
                <span class="keyword-chip" onclick="event.stopPropagation(); window.router.executeTMDBSearch('${k.name}')" style="font-size: 0.65rem; background: rgba(255,255,255,0.05); padding: 4px 8px; border-radius: 4px; color: var(--text-muted); border: 1px solid rgba(255,255,255,0.05); cursor: pointer; transition: all 0.2s;">${k.name}</span>
            `).join('');

            if (type === 'movie') {
                document.getElementById('movie-actions').style.display = 'block';
                document.getElementById('btn-smart-dl-movie').onclick = () => {
                    const year = (data.release_date || '').split('-')[0];
                    this.triggerSmartSearch(data.title || data.name, null, null, year);
                };
            } else {
                this.renderSeasonsSection(data.seasons || [], id, data.name || data.title);
            }

            this.loadRelatedMedia(type, id);
        } catch (e) {
            console.error(e);
            this.container.innerHTML = `<div style="padding: 4rem; text-align: center; color: #FF5252;">Briefing Link Severed: ${e.message}</div>`;
        }
    }

    async loadRelatedMedia(type, id) {
        try {
            const similarRes = await fetch(`/api/tmdb/${type}/${id}/similar`);
            const similarData = await similarRes.json();
            if (similarData.results) {
                const grid = document.getElementById('similar-grid');
                if (grid) this.renderDiscoverGrid(similarData.results.slice(0, 6), type, false, grid);
            }

            const recoRes = await fetch(`/api/tmdb/${type}/${id}/recommendations`);
            const recoData = await recoRes.json();
            if (recoData.results) {
                const grid = document.getElementById('recommended-grid');
                if (grid) this.renderDiscoverGrid(recoData.results.slice(0, 6), type, false, grid);
            }
        } catch (e) {
            console.error("Related Media Error", e);
        }
    }

    renderSeasonsSection(seasons, tvId, showTitle) {
        const section = document.getElementById('tv-seasons-section');
        const selector = document.getElementById('season-selector');
        if (!section || !selector) return;

        section.style.display = 'block';
        selector.innerHTML = seasons.filter(s => s.season_number > 0).map(s => `
            <option value="${s.season_number}">Season ${s.season_number}</option>
        `).join('');

        selector.onchange = (e) => this.loadEpisodes(tvId, e.target.value, showTitle);

        // Initial load first season
        const firstSeason = seasons.find(s => s.season_number > 0);
        if (firstSeason) this.loadEpisodes(tvId, firstSeason.season_number, showTitle);
    }

    async loadEpisodes(tvId, seasonNumber, showTitle) {
        const grid = document.getElementById('episodes-list');
        if (!grid) return;
        grid.innerHTML = '<div class="loading-spinner"></div>';

        try {
            const res = await fetch(`/api/tmdb/tv/${tvId}/season/${seasonNumber}`);
            const data = await res.json();

            grid.innerHTML = (data.episodes || []).map(ep => {
                const still = ep.still_path ? `https://image.tmdb.org/t/p/w300${ep.still_path}` : '/static/images/placeholder_poster.jpg';
                return `
                    <div class="glass-panel" style="display: flex; gap: 1.5rem; padding: 1rem; border-radius: 12px; transition: transform 0.2s; overflow: hidden;">
                        <div style="width: 180px; height: 100px; flex-shrink: 0; border-radius: 8px; overflow: hidden; position: relative; background: #000;">
                            <img src="${still}" style="width: 100%; height: 100%; object-fit: cover; opacity: 0.8;">
                            <div style="position: absolute; bottom: 0.5rem; left: 0.5rem; background: rgba(0,0,0,0.8); padding: 2px 6px; border-radius: 4px; font-size: 0.6rem; font-weight: 800; font-family: var(--font-mono);">
                                EP ${ep.episode_number}
                            </div>
                        </div>
                        <div style="flex: 1; min-width: 0; display: flex; flex-direction: column; justify-content: center;">
                            <div style="display: flex; justify-content: space-between; align-items: flex-start;">
                                <div>
                                    <div style="font-size: 1.1rem; font-weight: 700; margin-bottom: 0.25rem;">${ep.name}</div>
                                    <div style="font-size: 0.75rem; color: var(--text-muted); font-family: var(--font-mono);">${ep.air_date || 'Unknown Date'}</div>
                                </div>
                                <button class="icon-btn-tiny" onclick="window.router.triggerSmartSearch('${(showTitle || '').replace(/'/g, "\\'")}', ${seasonNumber}, ${ep.episode_number})" title="Smart Search">
                                    <span class="material-icons" style="font-size: 18px;">manage_search</span>
                                </button>
                            </div>
                            <div style="font-size: 0.85rem; color: var(--text-secondary); margin-top: 0.75rem; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; line-height: 1.4;">
                                ${ep.overview || 'No overview available for this transmission.'}
                            </div>
                        </div>
                    </div>
                `;
            }).join('');
        } catch (e) {
            grid.innerHTML = `<div style="color: #FF5252; padding: 1rem;">Episode Synchronicity Lost: ${e.message}</div>`;
        }
    }

    async triggerSmartSearch(title, season, episode, year) {
        let titleDisplay = title;
        if (season) titleDisplay += ` S${season.toString().padStart(2, '0')}`;
        if (episode) titleDisplay += `E${episode.toString().padStart(2, '0')}`;

        this.modalShow({
            title: `Neural Search: ${titleDisplay}`,
            body: `
                <div id="smart-search-loading" style="padding: 3rem; text-align: center;">
                    <div class="loading-spinner" style="margin-bottom: 1.5rem;"></div>
                    <p style="color: var(--color-primary); font-family: var(--font-mono); font-weight: 800;">CALCULATING SEARCH FRAGMENTS...</p>
                    <div id="smart-queries-debug" style="margin-top: 1rem; font-size: 0.65rem; color: var(--text-muted); font-family: var(--font-mono);"></div>
                </div>
                <div id="smart-results-container" style="display: none; max-height: 400px; overflow-y: auto;"></div>
            `,
            footer: '<button class="modal-btn secondary" onclick="window.router.modalHide()">Abort</button>'
        });

        const debug = document.getElementById('smart-queries-debug');
        const resultsEl = document.getElementById('smart-results-container');
        const loading = document.getElementById('smart-search-loading');

        try {
            const res = await fetch('/api/discovery/smart-search', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ title, season, episode, year })
            });
            const data = await res.json();

            if (debug) debug.textContent = `VECTORS: ${data.queries_used.join(', ')}`;

            loading.style.display = 'none';
            resultsEl.style.display = 'block';

            if (data.results && data.results.length > 0) {
                resultsEl.innerHTML = data.results.map(r => `
                    <div style="display: flex; justify-content: space-between; align-items: center; padding: 1rem; border-bottom: 1px solid rgba(255,255,255,0.05); background: rgba(255,255,255,0.01);">
                        <div style="flex: 1; min-width: 0; padding-right: 1.5rem;">
                            <div style="font-weight: 700; font-size: 0.85rem; color: #fff; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;" title="${r.name}">${r.name}</div>
                            <div style="font-size: 0.7rem; color: var(--text-muted); font-family: var(--font-mono); margin-top: 2px;">SIZE: ${((r.size_bytes || 0) / 1024 / 1024 / 1024).toFixed(2)} GB</div>
                        </div>
                        <button class="add-btn" onclick="window.router.handleTaskAdd('${r.url}')" style="width: auto; padding: 0.5rem 1rem; font-size: 0.7rem; border-radius: 6px;">
                            <span class="material-icons" style="font-size: 14px;">download</span>
                        </button>
                    </div>
                `).join('');
            } else {
                resultsEl.innerHTML = '<div style="padding: 3rem; text-align: center; color: var(--text-muted);">No matching data fragments found in the matrix.</div>';
            }

        } catch (e) {
            loading.innerHTML = `<p style="color: #FF5252;">Search Failure: ${e.message}</p>`;
        }
    }

    // --- EXPLORE MODULE ---

    loadExplore() {
        this.container.innerHTML = `
            <div style="height: 75vh; overflow: hidden; display: flex; flex-direction: column;">
                <div style="flex: 1; display: flex; flex-direction: column; gap: 0.75rem; overflow: hidden;">
                    <div style="display: flex; justify-content: flex-end; align-items: center; padding: 0.25rem 0;">
                        <div id="explore-pagination" class="pagination-coordinator"></div>
                    </div>
                    
                    <div id="explore-results" style="flex: 1; overflow-y: auto; display: grid; grid-template-columns: repeat(4, 1fr); auto-rows: min-content; gap: 0.75rem; padding-bottom: 2rem;">
                        <div style="grid-column: 1 / -1; min-height: 300px; display: flex; flex-direction: column; align-items: center; justify-content: center; color: var(--text-muted); opacity: 0.5;">
                            <span class="material-icons" style="font-size: 48px; margin-bottom: 0.75rem;">language</span>
                            Use the HUD search bar to explore...
                        </div>
                    </div>
                </div>
            </div>
        `;

        this.explorePage = 1;
        this.exploreLimit = 12; // 3 rows * 4 columns
        this.exploreData = [];
    }

    async executeExploreSearch(q) {
        if (!q) return;
        const results = document.getElementById('explore-results');
        const internalInput = document.getElementById('explore-input');
        if (internalInput) internalInput.value = q;
        this.omniSearchQuery = q;
        const globalSearch = document.getElementById('spotlight-search');
        if (globalSearch && globalSearch.value !== q) globalSearch.value = q;

        results.innerHTML = '<div style="grid-column: 1 / -1; grid-row: 1 / -1; display: flex; align-items: center; justify-content: center;"><div class="loading-spinner"></div></div>';

        try {
            const res = await fetch(`/api/search?q=${encodeURIComponent(q)}`);
            const data = await res.json();
            this.exploreData = data.results || [];
            this.explorePage = 1;
            this.renderExploreResults();
        } catch (e) {
            results.innerHTML = `<div style="grid-column: 1 / -1; grid-row: 1 / -1; display: flex; align-items: center; justify-content: center; color: #FF5252;"> Retrieval error: ${e.message} </div>`;
        }
    }

    renderExploreResults() {
        const results = document.getElementById('explore-results');
        const pagination = document.getElementById('explore-pagination');
        if (!results) return;

        if (this.exploreData.length === 0) {
            results.innerHTML = '<div style="grid-column: 1 / -1; grid-row: 1 / -1; display: flex; align-items: center; justify-content: center; color: var(--text-muted);"> No data fragments found. </div>';
            if (pagination) pagination.innerHTML = '';
            return;
        }

        const total = this.exploreData.length;
        const totalPages = Math.ceil(total / this.exploreLimit);
        const start = (this.explorePage - 1) * this.exploreLimit;
        const items = this.exploreData.slice(start, start + this.exploreLimit);

        results.innerHTML = items.map(item => `
            <div class="explore-card" id="card-${item.id || Math.random().toString(36).substr(2, 9)}">
                <div class="card-quality">${item.quality || 'Standard'}</div>
                <div class="card-title">${item.name}</div>
                <div style="display: flex; justify-content: space-between; align-items: center; margin-top: auto; padding-top: 0.5rem; border-top: 1px solid rgba(255,255,255,0.03);">
                    <span style="font-family: var(--font-mono); font-size: 0.75rem; color: var(--text-muted); font-weight: 800;">${((item.size || 0) / 1024 / 1024 / 1024).toFixed(2)} GB</span>
                    <div style="display: flex; gap: 0.5rem;">
                        <button class="icon-btn-tiny" onclick="window.router.copyToClipboard('${item.url}')" title="Copy URL">
                            <span class="material-icons" style="font-size: 14px;">content_copy</span>
                        </button>
                        <button class="icon-btn-tiny dl-btn" onclick="window.router.initiateExploreDownload(this, '${item.url}', '${item.name.replace(/'/g, "\\'")}')" style="background: var(--color-primary); color: #000; border-radius: 4px; padding: 4px 8px; width: auto; font-weight: 800; font-size: 0.65rem;">
                            <span class="material-icons" style="font-size: 14px;">add</span> DOWNLOAD
                        </button>
                    </div>
                </div>
            </div>
        `).join('');

        if (pagination) {
            pagination.innerHTML = `
                <div style="display: flex; gap: .5rem; align-items: center;">
                    <button onclick="window.router.setExplorePage(${this.explorePage - 1})" ${this.explorePage === 1 ? 'disabled' : ''}>
                        <span class="material-icons" style="font-size: 18px;">chevron_left</span>
                    </button>
                    <span class="page-info" style="font-size: 0.7rem;">${this.explorePage} / ${totalPages}</span>
                    <button onclick="window.router.setExplorePage(${this.explorePage + 1})" ${this.explorePage === totalPages ? 'disabled' : ''}>
                        <span class="material-icons" style="font-size: 18px;">chevron_right</span>
                    </button>
                </div>
            `;
        }
    }

    async initiateExploreDownload(btn, url, filename) {
        const originalText = btn.innerHTML;

        // PRE-FLIGHT QUOTA CHECK
        let remainingGb = this.remainingQuotaGb || 150; // Default fallback

        // Attempt to find item size
        const item = this.exploreData.find(d => d.url === url);
        if (item) {
            const fileSizeGb = item.size / (1024 * 1024 * 1024);
            if (fileSizeGb > remainingGb) {
                const proceed = confirm(`⚠️ INSUFFICIENT QUOTA\n\nRequired: ${(fileSizeGb || 0).toFixed(2)} GB\nAvailable: ${(remainingGb || 0).toFixed(2)} GB\n\nDownload will likely fail. Proceed?`);
                if (!proceed) return;
            }
        }

        this.updateStatusMonitor(`Grabbing ${filename}...`, 'info');

        const card = btn.closest('.explore-card');
        if (card) card.classList.add('holo-processing');
        btn.innerHTML = '<span class="material-icons rotating" style="font-size: 14px;">sync</span> PROCESSING';
        btn.disabled = true;

        try {
            const res = await fetch('/api/downloads', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ url, name: filename })
            });
            const data = await res.json();

            // Check for actual success (API returns success: true/false)
            if (data.success === true || (data.status === 'ok' && data.nzo_id)) {
                if (card) {
                    card.classList.remove('holo-processing');
                    card.classList.add('item-queued');
                }

                this.updateStatusMonitor(`Got ya! Added ${filename}`, 'success');

                // Replace button with "QUEUED" indicator
                btn.outerHTML = `<div style="background: rgba(255,215,0,0.15); color: #ffd700; padding: 4px 12px; border-radius: 4px; font-size: 0.65rem; font-weight: 800; display: inline-flex; align-items: center; gap: 4px; border: 1px solid rgba(255,215,0,0.3);">
                    <span class="material-icons" style="font-size: 12px;">hourglass_top</span> QUEUED
                </div>`;

                setTimeout(() => this.updateCaptainInfo(null, true), 1500);
            } else {
                if (card) card.classList.remove('holo-processing');
                btn.innerHTML = originalText;
                btn.disabled = false;
                this.showError('Mission failed: ' + (data.message || 'Unknown error'));
                this.updateStatusMonitor(`Failed: ${data.message}`, 'error');
            }
        } catch (e) {
            if (card) card.classList.remove('holo-processing');
            btn.innerHTML = originalText;
            btn.disabled = false;
            this.showError('Transmission error during handshake');
            this.updateStatusMonitor(`Transmission Error`, 'error');
        }
    }

    updateStatusMonitor(msg, type = 'info') {
        let monitor = document.getElementById('terminal-monitor');
        if (!monitor) {
            monitor = document.createElement('div');
            monitor.id = 'terminal-monitor';
            monitor.className = 'status-monitor';
            monitor.innerHTML = `
                <span class="material-icons status-monitor-icon">terminal</span>
                <span class="status-monitor-text">INITIALIZING...</span>
            `;
            document.body.appendChild(monitor);
        }

        const textEl = monitor.querySelector('.status-monitor-text');
        const iconEl = monitor.querySelector('.status-monitor-icon');

        monitor.className = `status-monitor active ${type}`;
        textEl.textContent = msg;
        iconEl.textContent = type === 'success' ? 'check_circle' : (type === 'error' ? 'error' : 'terminal');

        // Auto-hide after 5s
        if (this.monitorTimeout) clearTimeout(this.monitorTimeout);
        this.monitorTimeout = setTimeout(() => {
            monitor.classList.remove('active');
        }, 5000);
    }

    // --- DASHBOARD MODULE ---

    loadDashboard() {
        this.container.innerHTML = `
            <!-- Trending Carousel Mount -->
            <div id="trending-mount"></div>

            <!-- Trending Carousel Mount -->
            <div id="trending-mount"></div>

            <div style="display: grid; grid-template-columns: 6.5fr 3.5fr; gap: 1.5rem; height: calc(100vh - 450px); min-height: 400px; margin-bottom: 0;">
                
                <!-- Left Column: Active Mission Pulse (Queue) -->
                <div class="glass-panel" style="padding: 1.5rem; overflow: hidden; display: flex; flex-direction: column;">
                    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.25rem; flex-shrink: 0;">
                        <h3 style="font-size: 0.95rem; font-weight: 700; display: flex; align-items: center; gap: 0.6rem;">
                            <span class="material-icons" style="font-size: 20px; color: var(--color-secondary);">list_alt</span>
                            Active Mission Pulse
                        </h3>
                        <button class="nav-item" onclick="window.router.navigate('downloads')" style="padding: 4px 12px; border-radius: 6px; font-size: 0.7rem; font-weight: 800; border: 1px solid rgba(255,255,255,0.05); background: transparent; cursor: pointer;">QUEUE</button>
                    </div>
                    <div id="minified-queue" style="display: grid; grid-template-columns: repeat(2, 1fr); gap: 1rem; overflow-y: auto; padding-right: 4px;">
                        <div class="loading-spinner" style="transform: scale(0.5); grid-column: 1/-1; margin: 1rem auto;"></div>
                    </div>
                </div>

                <!-- Right Column: Command Center & Netflow -->
                <div style="display: flex; flex-direction: column; gap: 1.5rem;">
                    
                    <!-- Combined Command Unit -->
                    <div class="glass-panel" style="padding: 1.25rem; border-left: 4px solid var(--color-primary); display: flex; flex-direction: column; gap: 1.25rem;">
                        
                        <!-- Header: Captain Identity -->
                        <div style="display: flex; align-items: center; gap: 1rem;">
                            <div style="width: 48px; height: 48px; border-radius: 50%; background: rgba(0,243,255,0.1); display: flex; align-items: center; justify-content: center; border: 1px solid var(--color-primary); box-shadow: 0 0 10px rgba(0,243,255,0.2);">
                                <span class="material-icons" style="font-size: 24px; color: var(--color-primary);">shield</span>
                            </div>
                            <div style="flex: 1; overflow: hidden;">
                                <h2 class="glow-text" style="font-size: 0.85rem; margin-bottom: 2px; text-transform: uppercase; letter-spacing: 0.05em;">Command Identity</h2>
                                <div id="captain-email" style="font-family: var(--font-mono); font-size: 0.75rem; color: var(--text-secondary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">Syncing...</div>
                                <div style="display: flex; justify-content: space-between; align-items: baseline; margin-top: 4px;">
                                    <span id="captain-rank" style="font-size: 0.6rem; font-weight: 800; color: var(--color-primary);">AUTH...</span>
                                    <span id="captain-expiry" style="font-size: 0.6rem; color: var(--text-muted);">...</span>
                                </div>
                            </div>
                        </div>

                        <!-- Telemetry Stats Row -->
                        <div style="display: grid; grid-template-columns: repeat(3, 1fr); gap: 0.5rem;">
                            <div style="background: rgba(0,255,163,0.02); padding: 0.5rem; border-radius: 8px; border: 1px solid rgba(0,255,163,0.05); text-align: center;">
                                <div style="font-size: 0.5rem; color: var(--text-muted); text-transform: uppercase;">Active</div>
                                <div id="stat-active" style="font-size: 1rem; font-weight: 800; color: var(--color-secondary); font-family: var(--font-mono);">0</div>
                            </div>
                            <div style="background: rgba(255,255,255,0.02); padding: 0.5rem; border-radius: 8px; border: 1px solid rgba(255,255,255,0.05); text-align: center;">
                                <div style="font-size: 0.5rem; color: var(--text-muted); text-transform: uppercase;">Queued</div>
                                <div id="stat-queued" style="font-size: 1rem; font-weight: 800; color: #ffd700; font-family: var(--font-mono);">0</div>
                            </div>
                            <div style="background: rgba(255,255,255,0.02); padding: 0.5rem; border-radius: 8px; border: 1px solid rgba(255,255,255,0.05); text-align: center;">
                                <div style="font-size: 0.5rem; color: var(--text-muted); text-transform: uppercase;">Done</div>
                                <div id="stat-completed" style="font-size: 1rem; font-weight: 800; color: var(--color-primary); font-family: var(--font-mono);">0</div>
                            </div>
                        </div>

                        <!-- Fuel Consumption -->
                        <div>
                            <div style="display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 0.25rem;">
                                <span style="font-size: 0.65rem; font-weight: 700; color: #ffd700; text-transform: uppercase;">Fuel (Quota)</span>
                                <div style="font-family: var(--font-mono); font-size: 0.7rem; font-weight: 800; color: var(--text-primary);">
                                    <span id="quota-percent">0</span>%
                                </div>
                            </div>
                            <div class="fuel-gauge" style="height: 6px; background: rgba(0,0,0,0.3); border-radius: 3px; overflow: hidden; position: relative; border: 1px solid rgba(255,255,255,0.03);">
                                <div id="fuel-bar" style="height: 100%; width: 0%; background: linear-gradient(90deg, #ffd700, #ff8c00); box-shadow: 0 0 8px rgba(255, 215, 0, 0.3); border-radius: 3px; transition: width 1s ease;"></div>
                            </div>
                            <div id="quota-text" style="font-family: var(--font-mono); font-size: 0.65rem; color: var(--text-muted); text-align: right; margin-top: 2px;">0 GB / 0 GB</div>
                        </div>

                    </div>

                    <!-- Netflow -->
                    <div class="glass-panel" style="padding: 1rem; flex: 1; min-height: 180px; display: flex; flex-direction: column;">
                        <h3 style="margin-bottom: 0.5rem; font-size: 0.8rem; font-weight: 700; display: flex; align-items: center; gap: 0.6rem; color: var(--text-primary); text-transform: uppercase; letter-spacing: 0.05em;">
                            <span class="material-icons" style="font-size: 16px; color: var(--color-primary);">insights</span>
                            Netflow
                        </h3>
                        <div style="flex: 1; position: relative;">
                            <canvas id="netFlowChart"></canvas>
                        </div>
                    </div>
                </div>
            </div>
        `;
        this.fetchDashboardSync();
        this.initNetflowChart();
        this.fetchPopularToday();
        this.fetchTrending();
    }

    async fetchPopularToday() {
        try {
            const res = await fetch('/api/discovery/popular-today?type=movie&limit=6');
            const data = await res.json();

            if (data.status === 'ok' && data.results) {
                const section = `
                    <div class="glass-panel popular-today-section" style="padding: 2rem; margin-top: 2rem;">
                        <div class="section-header">
                            <span class="material-icons">local_fire_department</span>
                            <h3 class="glow-text" style="font-size: 1.2rem; font-weight: 800;">Popular Today</h3>
                        </div>
                        <div class="discover-grid" style="grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));">
                            ${data.results.map(item => this.renderPosterCard(item)).join('')}
                        </div>
                    </div>
                `;
                this.container.insertAdjacentHTML('beforeend', section);
            }
        } catch (e) {
            console.error('Failed to fetch Popular Today:', e);
        }
    }

    async fetchTrending() {
        try {
            const res = await fetch('/api/tmdb/trending/movie/week');
            const data = await res.json();

            if (data.results && data.results.length > 0) {
                const section = `
                    <div class="glass-panel trending-section" style="padding: 2rem; margin-bottom: 2rem;">
                        <div class="section-header">
                            <span class="material-icons">trending_up</span>
                            <h3 class="glow-text" style="font-size: 1.2rem; font-weight: 800;">Trending This Week</h3>
                        </div>
                        <div class="carousel-container">
                            <button class="carousel-btn prev" onclick="window.router.carouselPrev()" id="carousel-prev">
                                <span class="material-icons">chevron_left</span>
                            </button>
                            <div class="carousel-track" id="trending-carousel">
                                ${data.results.slice(0, 20).map(item => this.renderPosterCard(item, 'carousel')).join('')}
                            </div>
                            <button class="carousel-btn next" onclick="window.router.carouselNext()" id="carousel-next">
                                <span class="material-icons">chevron_right</span>
                            </button>
                        </div>
                    </div>
                `;
                const mount = document.getElementById('trending-mount');
                if (mount) mount.innerHTML = section;
                this.initCarousel();
            }
        } catch (e) {
            console.error('Failed to fetch Trending:', e);
        }
    }

    renderPosterCard(item, variant = 'normal') {
        const title = item.title || item.name;
        const posterUrl = item.poster_url || '';
        const score = item.score || item.vote_average || 0;
        const year = (item.release_date || '').split('-')[0];
        const mediaType = item.media_type || 'movie';
        const id = item.id;

        const fshareAvailable = item.fshare_available;
        const fshareCount = item.fshare_count || 0;

        const cardWidth = variant === 'carousel' ? 'min-width: 200px; width: 200px;' : '';

        return `
            <div class="poster-card" style="${cardWidth}" onclick="window.router.navigate('media/${mediaType}/${id}')">
                <div class="poster-image" style="${posterUrl ? `background-image: url('${posterUrl}')` : ''}"></div>
                ${fshareAvailable !== undefined ? `
                    <div class="fshare-badge ${fshareAvailable ? '' : 'unavailable'}">
                        ${fshareAvailable ? `✓ ${fshareCount}` : 'N/A'}
                    </div>
                ` : ''}
                <div class="poster-overlay">
                    <div class="poster-content">
                        <div class="poster-title">${title}</div>
                        <div class="poster-meta">
                            <span class="material-icons" style="font-size: 12px;">star</span>
                            ${score.toFixed(1)} • ${year}
                        </div>
                        <div class="poster-desc">${item.overview || 'No overview available.'}</div>
                    </div>
                </div>
            </div>
        `;
    }

    initCarousel() {
        this.carouselIndex = 0;
        const track = document.getElementById('trending-carousel');
        if (!track) return;

        this.carouselMax = track.children.length;
        this.updateCarouselButtons();
    }

    carouselNext() {
        const track = document.getElementById('trending-carousel');
        if (!track) return;

        const cardWidth = 200 + 24; // card width + gap
        const visibleCards = Math.floor(track.parentElement.offsetWidth / cardWidth);

        if (this.carouselIndex < this.carouselMax - visibleCards) {
            this.carouselIndex++;
            this.updateCarousel();
        }
    }

    carouselPrev() {
        if (this.carouselIndex > 0) {
            this.carouselIndex--;
            this.updateCarousel();
        }
    }

    updateCarousel() {
        const track = document.getElementById('trending-carousel');
        if (!track) return;

        const cardWidth = 200 + 24; // card width + gap
        const offset = this.carouselIndex * -cardWidth;
        track.style.transform = `translateX(${offset}px)`;

        this.updateCarouselButtons();
    }

    updateCarouselButtons() {
        const prevBtn = document.getElementById('carousel-prev');
        const nextBtn = document.getElementById('carousel-next');

        if (prevBtn) prevBtn.disabled = this.carouselIndex === 0;

        if (nextBtn) {
            const track = document.getElementById('trending-carousel');
            if (track) {
                const cardWidth = 200 + 24;
                const visibleCards = Math.floor(track.parentElement.offsetWidth / cardWidth);
                nextBtn.disabled = this.carouselIndex >= this.carouselMax - visibleCards;
            }
        }
    }

    async fetchDashboardSync() {
        try {
            const res = await fetch('/api/stats');
            const data = await res.json();
            if (data.status === 'ok' && data.fshare_downloader) {
                const acc = data.fshare_downloader.primary_account;
                this.updateCaptainInfo(acc);
                this.updateDashboardStats(data.fshare_downloader);
                this.updateQuota(acc.traffic_left);
            }

            const dlRes = await fetch('/api/downloads');
            const dlData = await dlRes.json();
            this.renderMinifiedQueue(dlData.downloads || []);
        } catch (e) { console.error('Dashboard telemetry fail', e); }
    }

    updateCaptainInfo(acc, force = false) {
        // Force refresh logic
        if (force) {
            fetch('/api/verify-account', { method: 'POST' })
                .then(r => r.json())
                .then(d => {
                    if (d.status === 'ok' && d.account) {
                        this.updateCaptainInfo(d.account, false);
                    }
                }).catch(console.error);
            if (!acc) return;
        }

        if (!acc) return;
        const emailEl = document.getElementById('captain-email');
        const rankEl = document.getElementById('captain-rank');
        const expiryEl = document.getElementById('captain-expiry');

        if (emailEl) emailEl.textContent = acc.email || 'Anonymous Commander';
        if (rankEl) {
            rankEl.innerHTML = `<span class="material-icons" style="font-size: 12px;">${acc.premium ? 'stars' : 'bolt'}</span> ${acc.premium ? 'Gold Tier Commander' : 'Free Tier Cadet'}`;
            rankEl.style.color = acc.premium ? 'var(--color-primary)' : 'var(--text-muted)';
            rankEl.style.background = acc.premium ? 'rgba(0,243,255,0.1)' : 'rgba(255,255,255,0.05)';
        }
        if (expiryEl) {
            expiryEl.innerHTML = `VALID UNTIL: <span style="color: var(--color-primary);">${acc.expiry || 'UNLIMITED'}</span>`;
        }

        if (acc.traffic_left) this.updateQuota(acc.traffic_left);
    }

    async updateDashboardStats(stats) {
        const activeEl = document.getElementById('stat-active');
        const queuedEl = document.getElementById('stat-queued');
        const completedEl = document.getElementById('stat-completed');
        const totalEl = document.getElementById('stat-total');

        if (activeEl) activeEl.textContent = stats.active || 0;
        if (queuedEl) queuedEl.textContent = stats.queued || 0;
        if (completedEl) completedEl.textContent = stats.completed || 0;
        if (totalEl) totalEl.textContent = stats.total || 0;

        const speedEl = document.getElementById('global-speed');
        const activeGlobalEl = document.getElementById('global-active');
        if (speedEl) speedEl.textContent = stats.speed || '0 B/s';
        if (activeGlobalEl) activeGlobalEl.textContent = stats.active || 0;

        // Push to graph if defined
        if (window.netFlowChartInst) {
            const chart = window.netFlowChartInst;
            chart.data.datasets[0].data.push(stats.speed_bytes / 1024 / 1024); // MB/s
            chart.data.datasets[0].data.shift();
            chart.update('none');
        }
    }

    updateQuota(traffic) {
        const text = document.getElementById('quota-text');
        const bar = document.getElementById('fuel-bar');
        const percentEl = document.getElementById('quota-percent');
        const cell = document.getElementById('fuel-cell');
        const label = cell ? cell.querySelector('h3') : null;

        if (!text || !bar || !percentEl || !traffic) return;

        let usedGb = 0; // Changed from leftGb to usedGb
        let totalGb = 150.0; // Dynamic fallback

        try {
            // Enhanced parsing for "Used / Total GB"
            const parts = traffic.split('/').map(p => parseFloat(p.replace(/[^\d.]/g, '')));
            if (parts.length === 2) {
                usedGb = parts[0];
                totalGb = parts[1];
            } else {
                usedGb = parts[0];
            }
        } catch (e) {
            console.error("Pulse Link: Quota Parse Error", e);
        }

        this.remainingQuotaGb = Math.max(0, totalGb - usedGb); // Updated calculation

        // Calculate Usage (since user wants <60% Green, 85%+ Red)
        const usagePercent = totalGb > 0 ? (usedGb / totalGb) * 100 : 0; // Updated calculation
        const displayPercent = Math.min(100, Math.max(0, usagePercent)).toFixed(1);

        // Update UI
        text.textContent = `${(usedGb || 0).toFixed(1)} GB / ${(totalGb || 0).toFixed(0)} GB`; // Updated to usedGb
        bar.style.width = displayPercent + '%';
        percentEl.textContent = displayPercent;

        // Dynamic Coloring
        // < 60%: Green, 60-85%: Yellow, 85%+: Red
        let color = '#00ff88'; // Green
        let glow = 'rgba(0, 255, 136, 0.3)';

        if (usagePercent >= 85) {
            color = '#ff4d4d'; // Red
            glow = 'rgba(255, 77, 77, 0.3)';
        } else if (usagePercent >= 60) {
            color = '#ffd700'; // Yellow
            glow = 'rgba(255, 215, 0, 0.3)';
        }

        bar.style.background = `linear-gradient(90deg, ${color}, #ffffff)`;
        bar.style.boxShadow = `0 0 10px ${glow}`;
        if (label) label.style.color = color;
        if (cell) cell.style.borderLeftColor = color;
        const percentContainer = percentEl.parentElement;
        if (percentContainer) percentContainer.style.color = color;
    }


    renderMinifiedQueue(tasks) {
        const list = document.getElementById('minified-queue');
        if (!list) return;

        const priorityTasks = tasks.filter(t => t.state === 'Downloading' || t.state === 'Extracting').slice(0, 4);
        if (priorityTasks.length === 0) {
            list.innerHTML = '<div style="color: var(--text-muted); font-size: 0.75rem; text-align: center; padding: 1.5rem; background: rgba(0,0,0,0.1); border-radius: 8px; border: 1px dashed rgba(255,255,255,0.05);">Uplink stable. No active data streams.</div>';
            return;
        }

        list.innerHTML = priorityTasks.map(t => `
            <div style="background: rgba(255,255,255,0.02); padding: 0.85rem; border-radius: 10px; border: 1px solid rgba(255,255,255,0.04); display: flex; flex-direction: column; gap: 0.5rem;">
                <div style="display: flex; justify-content: space-between; align-items: center;">
                    <span style="font-size: 0.8rem; font-weight: 700; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 75%;">${t.filename}</span>
                    <span style="font-family: var(--font-mono); font-size: 0.75rem; color: var(--color-primary); font-weight: 800;">${t.progress}%</span>
                </div>
                <div style="height: 4px; background: rgba(0,0,0,0.2); border-radius: 2px; overflow: hidden;">
                    <div style="height: 100%; width: ${t.progress}%; background: var(--color-primary); box-shadow: 0 0 10px var(--color-primary); border-radius: 2px;"></div>
                </div>
            </div>
        `).join('');
    }

    initNetflowChart() {
        const ctx = document.getElementById('netFlowChart');
        if (!ctx) return;

        if (window.netFlowChartInst) window.netFlowChartInst.destroy();

        window.netFlowChartInst = new Chart(ctx, {
            type: 'line',
            data: {
                labels: Array(30).fill(''),
                datasets: [{
                    label: 'Downlink Velocity',
                    data: Array(30).fill(0),
                    borderColor: '#00f3ff',
                    borderWidth: 2,
                    tension: 0.4,
                    fill: 'start',
                    backgroundColor: (context) => {
                        const canvas = context.chart.canvas;
                        const chartCtx = canvas.getContext('2d');
                        const gradient = chartCtx.createLinearGradient(0, 0, 0, canvas.height);
                        gradient.addColorStop(0, 'rgba(0, 243, 255, 0.15)');
                        gradient.addColorStop(1, 'rgba(0, 243, 255, 0)');
                        return gradient;
                    },
                    pointRadius: 0,
                    borderCapStyle: 'round'
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                animation: { duration: 1000 },
                plugins: { legend: { display: false } },
                scales: {
                    x: { display: false },
                    y: {
                        display: true,
                        grid: { color: 'rgba(255,255,255,0.03)', drawBorder: false },
                        ticks: { color: 'rgba(255,255,255,0.2)', font: { size: 9, family: 'Roboto Mono' }, callback: (v) => v + ' MB/s' }
                    }
                }
            }
        });
    }


    // --- SETTINGS MODULE ---

    loadSettings() {
        const fetchC = async () => {
            try {
                const r = await fetch('/api/config');
                const d = await r.json();
                if (d.status === 'ok') {
                    const f = document.getElementById('config-form');
                    if (f) {
                        f.download_path.value = d.config.download_path;
                        f.concurrent_downloads.value = d.config.concurrent_downloads;
                        document.getElementById('val-concurrency').innerText = d.config.concurrent_downloads;
                        f.worker_threads.value = d.config.worker_threads;
                        document.getElementById('val-threads').innerText = d.config.worker_threads;
                    }
                }
            } catch (e) { }
        };
        setTimeout(fetchC, 100);
        this.container.innerHTML = `
            <div style="max-width: 1000px; margin: 0 auto; display: flex; flex-direction: column; gap: 2rem;">
                
                <div id="settings-board" style="display: flex; flex-direction: column; gap: 1.5rem;">
                    <!-- Accounts Card -->
                    <div class="glass-panel" style="padding: 2rem;">
                        <div id="accounts-section"><div class="loading-spinner"></div></div>
                    </div>

                    <!-- System Config Card -->
                    <div class="glass-panel" style="padding: 2rem; border-radius: 20px;">
                        <h2 style="margin-bottom: 2rem; display: flex; align-items: center; gap: 0.75rem;">
                            <span class="material-icons" style="color: var(--color-primary);">tune</span>
                            Engine Configuration
                        </h2>
                        <form id="config-form" style="display: grid; grid-template-columns: 1fr 1fr; gap: 1.5rem;">
                            <div class="form-group" style="grid-column: 1 / -1;">
                                <label style="display: block; color: var(--text-secondary); margin-bottom: 0.5rem; font-size: 0.9rem; font-weight: 600;">Download Matrix Path</label>
                                <input type="text" name="download_path" value="/downloads" class="modal-input" style="width: 100%;">
                            </div>
                            <div class="form-group">
                                <label style="display: flex; justify-content: space-between; color: var(--text-secondary); margin-bottom: 0.5rem; font-size: 0.9rem; font-weight: 600;">
                                    Max Concurrency
                                    <span id="val-concurrency" style="color: var(--color-primary); font-family: var(--font-mono);">0</span>
                                </label>
                                <input type="range" name="concurrent_downloads" min="1" max="10" value="3" class="sci-fi-slider" oninput="document.getElementById('val-concurrency').innerText = this.value">
                            </div>
                             <div class="form-group">
                                <label style="display: flex; justify-content: space-between; color: var(--text-secondary); margin-bottom: 0.5rem; font-size: 0.9rem; font-weight: 600;">
                                    Neural Worker Threads
                                    <span id="val-threads" style="color: var(--color-secondary); font-family: var(--font-mono);">0</span>
                                </label>
                                <input type="range" name="worker_threads" min="1" max="5" value="4" class="sci-fi-slider" oninput="document.getElementById('val-threads').innerText = this.value">
                            </div>
                            <div style="grid-column: 1 / -1; display: flex; justify-content: flex-end;">
                                <button type="submit" class="add-btn" style="width: auto; padding: 0.75rem 2.5rem; border-radius: 10px; font-weight: 800;">Save Protocol</button>
                            </div>
                        </form>
                    </div>

                    <!-- Logs Card -->
                    <div class="glass-panel" style="padding: 2rem;">
                        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem;">
                            <h2 style="display: flex; align-items: center; gap: 0.75rem;">
                                <span class="material-icons" style="color: var(--color-primary);">terminal</span>
                                Neural System Logs
                            </h2>
                            <button class="icon-btn" onclick="document.getElementById('log-stream').innerHTML = ''"><span class="material-icons">clear_all</span></button>
                        </div>
                        <div id="log-stream" style="height: 400px; background: rgba(0,0,0,0.6); border-radius: 12px; padding: 1.5rem; overflow-y: auto; font-family: var(--font-mono); font-size: 0.8rem; color: #a6accd; border: 1px solid var(--border-glass);">
                            <div style="color: var(--color-primary);">Awaiting telemetry stream...</div>
                        </div>
                    </div>
                </div>
            </div>
        `;

        this.fetchSettingsData();
        document.getElementById('config-form')?.addEventListener('submit', async (e) => {
            e.preventDefault();
            const fd = new FormData(e.target);
            const data = Object.fromEntries(fd.entries());
            try {
                const res = await fetch('/api/config', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(data)
                });
                const d = await res.json();
                if (d.status === 'ok') alert('Configuration Updated');
                else alert('Update Failed: ' + d.message);
            } catch (err) { alert('Network Error'); }
        });
        this.pollLogs();
    }

    async fetchSettingsData() {
        const accSection = document.getElementById('accounts-section');
        try {
            const res = await fetch('/api/accounts');
            const data = await res.json();
            // Data validation for sync success
            if (data && Array.isArray(data.accounts)) {
                this.renderAccountsSection(data.accounts, accSection);
            } else {
                throw new Error('Invalid telemetry packet');
            }
        } catch (e) {
            console.error('Account sync fail:', e);
            accSection.innerHTML = '<div style="color: #FF5252; display: flex; align-items: center; gap: 0.5rem;"><span class="material-icons">sync_problem</span> Account sync failure.</div>';
        }
    }

    renderAccountsSection(accounts, container) {
        container.innerHTML = `
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 2rem;">
                <h2 style="display: flex; align-items: center; gap: 0.75rem; font-size: 1.1rem; text-transform: uppercase; letter-spacing: 0.05em;">
                    <span class="material-icons" style="color: var(--color-primary);">manage_accounts</span>
                    Account Matrix
                </h2>
                <button class="add-btn" onclick="window.router.showConnectAccount()" style="width: auto; padding: .65rem 1.5rem; font-size: 0.75rem; border-radius: 8px;">
                    CONNECT IDENTITY
                </button>
            </div>
            <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 1.25rem;">
                ${accounts.map(acc => `
                <div class="glass-panel" style="padding: 1.5rem; display: flex; flex-direction: column; gap: 1.25rem; background: rgba(255,255,255,0.01);">
                    <div style="display: flex; align-items: center; gap: 1rem;">
                        <div style="width: 48px; height: 48px; border-radius: 50%; background: ${acc.is_primary ? 'rgba(0,243,255,0.1)' : 'rgba(255,255,255,0.05)'}; display: flex; align-items: center; justify-content: center; border: 1px solid ${acc.is_primary ? 'var(--border-glass-active)' : 'transparent'};">
                            <span class="material-icons" style="color: ${acc.is_primary ? 'var(--color-primary)' : '#64748b'}; font-size: 24px;">${acc.is_primary ? 'stars' : 'person'}</span>
                        </div>
                        <div style="flex: 1; overflow: hidden;">
                            <div style="font-weight: 700; font-size: 0.95rem; margin-bottom: 4px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--text-primary);">${acc.email}</div>
                            <div style="display: flex; gap: 0.5rem; align-items: center;">
                                ${acc.premium ? '<span style="font-size: 0.6rem; background: #FFD700; color: #000; padding: 2px 6px; border-radius: 4px; font-weight: 900; letter-spacing: 0.05em;">VIP</span>' : '<span style="font-size: 0.6rem; background: #475569; color: #fff; padding: 2px 6px; border-radius: 4px; font-weight: 900; letter-spacing: 0.05em;">FREE</span>'}
                                <span style="font-family: var(--font-mono); font-size: 0.7rem; color: var(--text-muted); font-weight: 600;">ACTIVE</span>
                            </div>
                        </div>
                        <div style="display: flex; gap: 0.35rem;">
                            <button class="icon-btn-tiny" onclick="window.router.reloginAccount('${acc.email}')" title="Refresh Telemetry">
                                <span class="material-icons" style="font-size: 16px;">refresh</span>
                            </button>
                            ${!acc.is_primary ? `
                                <button class="icon-btn-tiny" onclick="fetch('/api/accounts/${acc.email}/set-primary', {method:'POST'}).then(()=>window.router.fetchSettingsData())" title="Set Primary">
                                    <span class="material-icons" style="font-size: 16px;">grade</span>
                                </button>
                            ` : ''}
                            <button class="icon-btn-tiny" onclick="if(confirm('Sever connection for ${acc.email}?')) fetch('/api/accounts/${acc.email}', {method:'DELETE'}).then(()=>window.router.fetchSettingsData())" style="color: #FF5252;" title="Disconnect">
                                <span class="material-icons" style="font-size: 16px;">link_off</span>
                            </button>
                        </div>
                    </div>
                    <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; padding-top: 1rem; border-top: 1px solid rgba(255,255,255,0.03);">
                        <div>
                            <div style="font-size: 0.6rem; text-transform: uppercase; color: var(--text-muted); font-weight: 800; letter-spacing: 0.05em; margin-bottom: 4px;">Neural Fuel Left</div>
                            <div style="font-family: var(--font-mono); font-size: 0.85rem; color: var(--color-primary); font-weight: 700;">${acc.traffic_left || '0 GB'}</div>
                        </div>
                        <div>
                            <div style="font-size: 0.6rem; text-transform: uppercase; color: var(--text-muted); font-weight: 800; letter-spacing: 0.05em; margin-bottom: 4px;">Valid Until</div>
                            <div style="font-family: var(--font-mono); font-size: 0.85rem; color: var(--text-secondary); font-weight: 700;">${acc.expiry || 'LIFETIME'}</div>
                        </div>
                    </div>
                </div>
                `).join('')}
            </div>
        `;
    }

    async reloginAccount(email) {
        try {
            const res = await fetch(`/api/accounts/${email}/refresh`, { method: 'POST' });
            const data = await res.json();
            if (data.status === 'ok') {
                this.fetchSettingsData();
            } else {
                this.showError('Relogin Failed: ' + data.message);
            }
        } catch (e) {
            this.showError('Relogin Transmission Error');
        }
    }

    pollLogs() {
        const fetchL = async () => {
            const el = document.getElementById('log-stream');
            if (!el) return;
            try {
                const res = await fetch('/api/logs');
                const logs = await res.json();
                if (logs.length > 0) {
                    el.innerHTML = logs.map(l => `<div style="margin-bottom: 6px;"><span style="color: var(--text-muted);">[${l.time}]</span> <span style="color: ${l.level === 'error' ? '#FF5252' : (l.level === 'warning' ? '#FFD700' : '#00f3ff')}; font-weight: 800;">[${l.level.toUpperCase()}]</span> ${l.message}</div>`).join('');
                    el.scrollTop = el.scrollHeight;
                }
            } catch (e) { }
        };
        fetchL();
        if (this.logInterval) clearInterval(this.logInterval);
        this.logInterval = setInterval(() => {
            if (document.getElementById('log-stream')) fetchL();
            else clearInterval(this.logInterval);
        }, 2000);
    }

    closeContextMenu() {
        document.getElementById('actions-menu')?.remove();
    }

    showContextMenu(e, task) {
        this.closeContextMenu();
        const menu = document.createElement('div');
        menu.id = 'actions-menu';
        menu.className = 'glass-panel';
        menu.style.position = 'fixed';
        menu.style.left = `${e.clientX}px`;
        menu.style.top = `${e.clientY}px`;
        menu.style.zIndex = '1000';
        menu.style.minWidth = '180px';
        menu.style.padding = '0.5rem';
        menu.style.borderRadius = '12px';
        menu.style.boxShadow = '0 10px 40px rgba(0,0,0,0.8)';

        const actions = [
            { icon: 'info', text: task.filename, header: true },
            { icon: 'play_arrow', text: 'Resume Mission', action: 'start' },
            { icon: 'pause', text: 'Suspend Mission', action: 'pause' },
            { icon: 'info_outline', text: 'View Details', action: 'info' },
            { icon: 'content_copy', text: 'Copy Fshare Link', action: 'copy' },
            { icon: 'delete', text: 'Abort & Erase', action: 'delete', danger: true },
        ];

        menu.innerHTML = actions.map(a => {
            if (a.header) return `<div style="padding: 0.75rem 1rem; font-size: 0.65rem; font-weight: 800; color: var(--text-muted); text-transform: uppercase; border-bottom: 1px solid rgba(255,255,255,0.05); margin-bottom: 0.5rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">${a.text}</div>`;
            return `
                <div class="menu-item" onclick="window.router.handleContextAction('${task.id}', '${a.action}', ${JSON.stringify(task).replace(/"/g, '&quot;')})" style="display: flex; align-items: center; gap: 0.75rem; padding: 0.6rem 1rem; cursor: pointer; border-radius: 8px; font-size: 0.8rem; transition: background 0.2s; color: ${a.danger ? '#FF5252' : 'inherit'};">
                    <span class="material-icons" style="font-size: 16px;">${a.icon}</span>
                    <span>${a.text}</span>
                </div>
            `;
        }).join('');

        document.body.appendChild(menu);

        // Dynamic positioning to keep menu on screen
        const rect = menu.getBoundingClientRect();
        if (rect.right > window.innerWidth) menu.style.left = `${window.innerWidth - rect.width - 20}px`;
        if (rect.bottom > window.innerHeight) menu.style.top = `${window.innerHeight - rect.height - 20}px`;

        // Click away handler
        const close = (evt) => { if (!menu.contains(evt.target)) { this.closeContextMenu(); document.removeEventListener('mousedown', close); } };
        document.addEventListener('mousedown', close);
    }

    handleContextAction(id, action, taskOrUrl) {
        this.closeContextMenu();
        if (action === 'copy') {
            const url = typeof taskOrUrl === 'object' ? taskOrUrl.url : taskOrUrl;
            navigator.clipboard.writeText(url);
            return;
        }
        if (action === 'info') {
            this.showTaskInfo(taskOrUrl);
            return;
        }
        this.taskAction(id, action);
    }

    renderTags(item) {
        const tags = [];
        if (item.vote_average > 8) tags.push({ text: 'Highly Rated', color: '#ffd700', icon: 'star', type: 'sort', value: 'vote_average.desc' });
        if (item.popularity > 1500) tags.push({ text: 'Trending', color: '#ff4d4d', icon: 'trending_up', type: 'sort', value: 'popularity.desc' });

        // Quality based on some heuristic or data if available
        // For now, let's just use vote_count as a proxy for 'Popular'
        if (item.vote_count > 5000) tags.push({ text: 'Classic', color: '#00ccff', icon: 'history' });

        return tags.map(t => {
            if (t.type === 'sort') {
                return `
                    <div class="media-tag" onclick="event.stopPropagation(); window.router.setDiscoverSort('${t.value}')" style="background: ${t.color}20; color: ${t.color}; border: 1px solid ${t.color}40; display: flex; align-items: center; gap: 4px; padding: 2px 8px; border-radius: 4px; font-size: 0.65rem; font-weight: 800; text-transform: uppercase; cursor: pointer; pointer-events: auto;">
                        <span class="material-icons" style="font-size: 10px;">${t.icon}</span>
                        ${t.text}
                    </div>
                `;
            }
            return `
                <div class="media-tag" style="background: ${t.color}20; color: ${t.color}; border: 1px solid ${t.color}40; display: flex; align-items: center; gap: 4px; padding: 2px 8px; border-radius: 4px; font-size: 0.65rem; font-weight: 800; text-transform: uppercase;">
                    <span class="material-icons" style="font-size: 10px;">${t.icon}</span>
                    ${t.text}
                </div>
            `;
        }).join('');
    }

    navigateToGenre(genreId, type) {
        this.discoverState.genre = genreId;
        this.discoverState.type = type;
        this.discoverState.page = 1;
        this.discoverState.hasMore = true;
        this.navigate('discover');
    }
}


// Initialize
document.addEventListener('DOMContentLoaded', () => {
    window.router = new Router();
    window.router.init();
    window.router.connect();

    const path = window.location.pathname.replace('/', '') || 'dashboard';
    window.router.navigate(path === 'v2' ? 'dashboard' : path, false);
});

