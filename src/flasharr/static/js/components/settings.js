/**
 * Flasharr: Settings Component
 * 
 * Settings page with account management, engine configuration, and system logs.
 */

import { accounts, settings } from '../core/api.js';
import { ws } from '../core/websocket.js';
import { escapeHtml } from '../utils/sanitize.js';
import { formatDate } from '../utils/format.js';
import { $, delegate } from '../utils/dom.js';

/**
 * Settings view component.
 */
export class SettingsView {
    constructor() {
        this.container = null;
        this.mounted = false;
        this.logPollInterval = null;
    }

    /**
     * Mount the view.
     */
    mount(container) {
        this.container = container;
        this.mounted = true;
        this.render();
        this.startLogPolling();
    }

    /**
     * Unmount the view.
     */
    unmount() {
        this.stopLogPolling();
        this.mounted = false;
        this.container = null;
    }

    /**
     * Render the settings view.
     */
    render() {
        if (!this.container) return;

        this.container.innerHTML = `
            <div style="max-width: 1000px; margin: 0 auto; display: flex; flex-direction: column; gap: 2rem; padding: 1.5rem;">
                
                <div id="settings-board" style="display: flex; flex-direction: column; gap: 1.5rem;">
                    <!-- Accounts Card -->
                    <div class="glass-panel" style="padding: 2rem;">
                        <div id="accounts-section">
                            <div class="loading-container"><div class="loading-spinner"></div></div>
                        </div>
                    </div>

                    <!-- System Config Card -->
                    <div class="glass-panel" style="padding: 2rem; border-radius: 20px;">
                        <h2 style="margin-bottom: 2rem; display: flex; align-items: center; gap: 0.75rem;">
                            <span class="material-icons" style="color: var(--color-primary);">tune</span>
                            Engine Configuration
                        </h2>
                        <form id="config-form" style="display: grid; grid-template-columns: 1fr 1fr; gap: 1.5rem;">
                            <div class="form-group" style="grid-column: 1 / -1;">
                                <label style="display: block; color: var(--text-secondary); margin-bottom: 0.5rem; font-size: 0.9rem; font-weight: 600;">Download Path</label>
                                <input type="text" name="download_path" value="/downloads" class="modal-input" style="width: 100%;">
                            </div>
                            <div class="form-group">
                                <label style="display: flex; justify-content: space-between; color: var(--text-secondary); margin-bottom: 0.5rem; font-size: 0.9rem; font-weight: 600;">
                                    Max Concurrency
                                    <span id="val-concurrency" style="color: var(--color-primary); font-family: var(--font-mono);">0</span>
                                </label>
                                <input type="range" name="concurrent_downloads" min="1" max="10" value="3" class="standard-slider" id="slider-concurrency">
                            </div>
                            <div class="form-group">
                                <label style="display: flex; justify-content: space-between; color: var(--text-secondary); margin-bottom: 0.5rem; font-size: 0.9rem; font-weight: 600;">
                                    Worker Threads
                                    <span id="val-threads" style="color: var(--color-secondary); font-family: var(--font-mono);">0</span>
                                </label>
                                <input type="range" name="worker_threads" min="1" max="5" value="4" class="standard-slider" id="slider-threads">
                            </div>
                            <button type="submit" class="add-btn" style="width: auto; padding: 0.75rem 2.5rem; border-radius: 10px; font-weight: 800;">Save Settings</button>
                        </form>
                    </div>

                    <!-- Logs Card -->
                    <div class="glass-panel" style="padding: 2rem;">
                        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem;">
                            <h2 style="display: flex; align-items: center; gap: 0.75rem;">
                                <span class="material-icons" style="color: var(--color-primary);">terminal</span>
                                System Logs
                            </h2>
                            <button class="icon-btn" id="clear-logs-btn">
                                <span class="material-icons">clear_all</span>
                            </button>
                        </div>
                        <div id="log-stream" style="height: 400px; background: rgba(0,0,0,0.6); border-radius: 12px; padding: 1.5rem; overflow-y: auto; font-family: var(--font-mono); font-size: 0.8rem; color: #a6accd; border: 1px solid var(--border-glass);">
                            <div style="color: var(--color-primary); font-family: var(--font-mono); font-size: 0.7rem;">Connecting to log stream...</div>
                        </div>
                    </div>
                </div>
            </div>
        `;

        this.bindEvents();
        this.fetchData();
    }

    /**
     * Bind event handlers.
     */
    bindEvents() {
        // Slider value display
        const concurrencySlider = $('slider-concurrency');
        const threadsSlider = $('slider-threads');

        if (concurrencySlider) {
            concurrencySlider.addEventListener('input', (e) => {
                const valEl = $('val-concurrency');
                if (valEl) valEl.textContent = e.target.value;
            });
        }

        if (threadsSlider) {
            threadsSlider.addEventListener('input', (e) => {
                const valEl = $('val-threads');
                if (valEl) valEl.textContent = e.target.value;
            });
        }

        // Form submission
        const form = $('config-form');
        if (form) {
            form.addEventListener('submit', (e) => this.handleConfigSubmit(e));
        }

        // Clear logs
        const clearBtn = $('clear-logs-btn');
        if (clearBtn) {
            clearBtn.addEventListener('click', () => {
                const logStream = $('log-stream');
                if (logStream) logStream.innerHTML = '';
            });
        }
    }

    /**
     * Fetch settings data.
     */
    async fetchData() {
        await Promise.all([
            this.fetchConfig(),
            this.fetchAccounts()
        ]);
    }

    /**
     * Fetch configuration.
     */
    async fetchConfig() {
        try {
            const data = await settings.get();
            if (data.status === 'ok' && data.config) {
                const form = $('config-form');
                if (form) {
                    form.download_path.value = data.config.download_path || '/downloads';
                    form.concurrent_downloads.value = data.config.concurrent_downloads || 3;
                    form.worker_threads.value = data.config.worker_threads || 4;

                    // Update display values
                    const valConcurrency = $('val-concurrency');
                    const valThreads = $('val-threads');
                    if (valConcurrency) valConcurrency.textContent = data.config.concurrent_downloads;
                    if (valThreads) valThreads.textContent = data.config.worker_threads;
                }
            }
        } catch (e) {
            console.error('Failed to fetch config:', e);
        }
    }

    /**
     * Fetch accounts.
     */
    async fetchAccounts() {
        const section = $('accounts-section');
        if (!section) return;

        try {
            const data = await accounts.list();
            this.renderAccounts(data.accounts || [], section);
        } catch (e) {
            section.innerHTML = '<div style="color: #ff5252;">Failed to load accounts</div>';
        }
    }

    /**
     * Render accounts section.
     */
    renderAccounts(accountsList, container) {
        if (accountsList.length === 0) {
            container.innerHTML = `
                <div style="text-align: center; padding: 2rem;">
                    <span class="material-icons" style="font-size: 48px; color: var(--text-muted); opacity: 0.3;">account_circle</span>
                    <p style="color: var(--text-muted); margin-top: 1rem;">No accounts connected</p>
                    <button class="add-btn" id="add-account-btn" style="margin-top: 1rem;">Connect Account</button>
                </div>
            `;
        } else {
            container.innerHTML = `
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem;">
                    <h2 style="display: flex; align-items: center; gap: 0.75rem;">
                        <span class="material-icons" style="color: var(--color-secondary);">account_circle</span>
                        Connected Accounts
                    </h2>
                    <button class="add-btn" id="add-account-btn" style="padding: 0.5rem 1rem; font-size: 0.8rem;">
                        <span class="material-icons" style="font-size: 16px;">add</span> Add
                    </button>
                </div>
                <div class="accounts-list" style="display: flex; flex-direction: column; gap: 0.75rem;">
                    ${accountsList.map(acc => this.renderAccountRow(acc)).join('')}
                </div>
            `;
        }

        // Bind add account button
        const addBtn = $('add-account-btn');
        if (addBtn) {
            addBtn.addEventListener('click', () => this.showAddAccountModal());
        }
    }

    /**
     * Render account row.
     */
    renderAccountRow(account) {
        const isActive = account.status === 'active';
        const statusColor = isActive ? 'var(--color-secondary)' : '#ff5252';
        const statusIcon = isActive ? 'check_circle' : 'error';

        return `
            <div class="account-row" style="display: flex; align-items: center; justify-content: space-between; padding: 1rem; background: rgba(255,255,255,0.02); border-radius: 12px; border: 1px solid rgba(255,255,255,0.05);">
                <div style="display: flex; align-items: center; gap: 1rem;">
                    <div style="width: 40px; height: 40px; border-radius: 50%; background: rgba(0,243,255,0.05); display: flex; align-items: center; justify-content: center; border: 1px solid ${statusColor};">
                        <span class="material-icons" style="font-size: 20px; color: ${statusColor};">${statusIcon}</span>
                    </div>
                    <div>
                        <div style="font-weight: 600; color: var(--text-primary);">${escapeHtml(account.email)}</div>
                        <div style="font-size: 0.75rem; color: var(--text-muted);">
                            ${account.account_type ? account.account_type.toUpperCase() : 'Unknown'} 
                            ${account.expire_date ? `• Expires: ${formatDate(account.expire_date)}` : ''}
                        </div>
                    </div>
                </div>
                <div style="display: flex; gap: 0.5rem;">
                    <button class="icon-btn relogin-btn" data-email="${escapeHtml(account.email)}" title="Re-login">
                        <span class="material-icons">refresh</span>
                    </button>
                </div>
            </div>
        `;
    }

    /**
     * Show add account modal.
     */
    showAddAccountModal() {
        if (window.router && window.router.showConnectAccount) {
            window.router.showConnectAccount();
        }
    }

    /**
     * Handle config form submission.
     */
    async handleConfigSubmit(e) {
        e.preventDefault();

        const form = e.target;
        const formData = new FormData(form);
        const data = Object.fromEntries(formData.entries());

        try {
            const result = await settings.update(data);
            if (result.status === 'ok') {
                this.showNotification('Configuration saved', 'success');
            } else {
                this.showNotification('Failed to save: ' + (result.message || 'Unknown error'), 'error');
            }
        } catch (e) {
            this.showNotification('Network error', 'error');
        }
    }

    /**
     * Show notification.
     */
    showNotification(message, type = 'info') {
        // Simple alert for now, could be replaced with toast
        alert(message);
    }

    /**
     * Start log polling.
     */
    startLogPolling() {
        // Setup WebSocket log listener
        ws.on('log', (data) => {
            if (this.mounted) {
                this.appendLog(data);
            }
        });

        // Also poll periodically
        this.logPollInterval = setInterval(() => {
            if (this.mounted) {
                this.fetchLogs();
            }
        }, 5000);
    }

    /**
     * Stop log polling.
     */
    stopLogPolling() {
        if (this.logPollInterval) {
            clearInterval(this.logPollInterval);
            this.logPollInterval = null;
        }
    }

    /**
     * Fetch logs.
     */
    async fetchLogs() {
        try {
            const response = await fetch('/api/logs?limit=50');
            const data = await response.json();

            if (data.logs && data.logs.length > 0) {
                const logStream = $('log-stream');
                if (logStream) {
                    logStream.innerHTML = data.logs.map(log => this.formatLogEntry(log)).join('');
                    logStream.scrollTop = logStream.scrollHeight;
                }
            }
        } catch (e) {
            // Silently fail
        }
    }

    /**
     * Append a log entry.
     */
    appendLog(logEntry) {
        const logStream = $('log-stream');
        if (!logStream) return;

        const entry = document.createElement('div');
        entry.innerHTML = this.formatLogEntry(logEntry);
        logStream.appendChild(entry.firstChild);

        // Keep only last 200 entries
        while (logStream.children.length > 200) {
            logStream.removeChild(logStream.firstChild);
        }

        logStream.scrollTop = logStream.scrollHeight;
    }

    /**
     * Format log entry.
     */
    formatLogEntry(log) {
        const level = log.level || 'INFO';
        const message = log.message || log.msg || log;
        const timestamp = log.timestamp ? new Date(log.timestamp).toLocaleTimeString() : '';

        const levelColors = {
            'ERROR': '#ff5252',
            'WARN': '#ffd700',
            'WARNING': '#ffd700',
            'INFO': 'var(--color-primary)',
            'DEBUG': 'var(--text-muted)'
        };

        const color = levelColors[level.toUpperCase()] || 'var(--text-secondary)';

        return `
            <div style="margin-bottom: 0.25rem; line-height: 1.4;">
                <span style="color: var(--text-muted); font-size: 0.7rem;">${timestamp}</span>
                <span style="color: ${color}; font-weight: 600; margin-left: 0.5rem;">[${level}]</span>
                <span style="margin-left: 0.5rem;">${escapeHtml(String(message))}</span>
            </div>
        `;
    }
}

// Export singleton
export const settingsView = new SettingsView();
