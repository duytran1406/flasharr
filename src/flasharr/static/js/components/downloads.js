/**
 * Flasharr: Downloads Component
 * 
 * Manages the Downloads view including listing, filtering, sorting,
 * pagination, and task actions.
 */

import { downloads as api } from '../core/api.js';
import { state } from '../core/state.js';
import { escapeHtml, escapeAttr, escapeJs } from '../utils/sanitize.js';
import { formatBytes, formatSpeed, formatDuration, formatDate } from '../utils/format.js';
import { $, delegate, debounce } from '../utils/dom.js';

/**
 * Downloads view component.
 */
export class DownloadsView {
    constructor() {
        this.container = null;
        this.page = 1;
        this.limit = 12;
        this.sortColumn = 'added';
        this.sortDirection = 'desc';
        this.filterQuery = '';
        this.pollInterval = null;
        this.mounted = false;
    }

    /**
     * Mount the view to a container.
     * @param {Element} container - Target container
     */
    mount(container) {
        this.container = container;
        this.mounted = true;
        this.render();
        this.startPolling();
    }

    /**
     * Unmount the view.
     */
    unmount() {
        this.stopPolling();
        this.mounted = false;
        this.container = null;
    }

    /**
     * Render the downloads view.
     */
    render() {
        if (!this.container) return;

        this.container.innerHTML = `
            <div style="padding: 1.5rem; height: 100%; box-sizing: border-box; display: flex; flex-direction: column;">
                <div class="glass-panel" style="padding: 0; overflow: hidden; display: flex; flex-direction: column; flex: 1;">
                    <!-- Toolbar -->
                    <div style="padding: 1rem 1.5rem; border-bottom: 1px solid rgba(255,255,255,0.05); display: flex; justify-content: space-between; align-items: center;">
                        <h2 class="glow-text" style="font-size: 1rem; text-transform: uppercase;">Active Downloads</h2>
                        <div style="display: flex; gap: 0.5rem; align-items: center;">
                            <button class="btn-tiny btn-success" data-action="resume-all" title="Start/Resume All">
                                <span class="material-icons">play_arrow</span>
                            </button>
                            <button class="btn-tiny btn-danger" data-action="pause-all" title="Pause/Stop All">
                                <span class="material-icons">pause</span>
                            </button>
                            <div style="width: 1px; height: 16px; background: rgba(255,255,255,0.1); margin: 0 0.25rem;"></div>
                            <button class="btn-tiny" data-action="refresh" title="Refresh">
                                <span class="material-icons">refresh</span>
                            </button>
                        </div>
                    </div>

                    <!-- Table Container -->
                    <div style="flex: 1; overflow: hidden; padding: 0; position: relative;">
                        <div style="position: absolute; top: 0; left: 0; right: 0; bottom: 0; overflow-y: auto;">
                            <table class="data-table" style="width: 100%; border-collapse: collapse; table-layout: fixed;">
                                <thead style="position: sticky; top: 0; background: rgba(15, 23, 42, 0.95); z-index: 10; backdrop-filter: blur(10px);">
                                    <tr id="download-header" style="font-size: 0.62rem; font-weight: 800; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em; border-bottom: 1px solid rgba(255,255,255,0.03);">
                                        ${this.renderHeaderRow()}
                                    </tr>
                                </thead>
                                <tbody id="download-list">
                                    <tr><td colspan="8" style="text-align: center; padding: 4rem;"><div class="loading-container"><div class="loading-spinner"></div></div></td></tr>
                                </tbody>
                            </table>
                        </div>
                    </div>

                    <div style="padding: 0.5rem 1.25rem; display: flex; justify-content: flex-end; border-top: 1px solid rgba(255,255,255,0.03); background: rgba(0,0,0,0.15);">
                        <div id="downloads-pagination" class="pagination-coordinator"></div>
                    </div>
                </div>
            </div>
        `;

        this.bindEvents();
        this.refresh();
    }

    /**
     * Render table header row.
     */
    renderHeaderRow() {
        const columns = [
            { key: 'filename', label: 'Filename', width: '31.5%' },
            { key: 'state', label: 'Status', width: '10%' },
            { key: 'size', label: 'Size', width: '10%' },
            { key: 'progress', label: 'Progress', width: '14%' },
            { key: 'speed', label: 'Speed', width: '4.5%' },
            { key: 'eta', label: 'ETA', width: '10%' },
            { key: 'added', label: 'Added', width: '14%' },
        ];

        const headers = columns.map(col => `
            <th style="padding: 0.6rem ${col.key === 'filename' ? '1.25rem' : '0.5rem'}; text-align: left; width: ${col.width}; cursor: pointer;" data-sort="${col.key}">
                <div style="display: flex; align-items: center; gap: 4px;">${col.label} ${this.getSortIcon(col.key)}</div>
            </th>
        `).join('');

        return headers + '<th style="padding: 0.6rem 1.25rem; text-align: right; width: 4%;">...</th>';
    }

    /**
     * Get sort icon for a column.
     */
    getSortIcon(column) {
        if (this.sortColumn !== column) {
            return '<span class="material-icons" style="font-size: 12px; opacity: 0.3;">sort</span>';
        }
        return this.sortDirection === 'asc'
            ? '<span class="material-icons" style="font-size: 12px; color: var(--color-primary);">expand_less</span>'
            : '<span class="material-icons" style="font-size: 12px; color: var(--color-primary);">expand_more</span>';
    }

    /**
     * Bind event handlers.
     */
    bindEvents() {
        const panel = this.container.querySelector('.glass-panel');
        if (!panel) return;

        // Toolbar buttons
        delegate(panel, 'click', '[data-action]', (e, btn) => {
            const action = btn.dataset.action;
            if (action === 'resume-all') this.batchAction('resume-all');
            else if (action === 'pause-all') this.batchAction('pause-all');
            else if (action === 'refresh') this.refresh();
        });

        // Sort headers
        delegate(panel, 'click', '[data-sort]', (e, th) => {
            this.setSort(th.dataset.sort);
        });

        // Row context menu
        delegate(panel, 'contextmenu', '.transfer-row', (e, row) => {
            e.preventDefault();
            const taskId = row.dataset.id;
            this.showContextMenu(e, taskId);
        });

        // Row click for info
        delegate(panel, 'click', '.transfer-row td:first-child', (e, td) => {
            const row = td.closest('.transfer-row');
            if (row) this.showTaskInfo(row.dataset.id);
        });

        // More button
        delegate(panel, 'click', '.more-btn', (e, btn) => {
            e.stopPropagation();
            const row = btn.closest('.transfer-row');
            if (row) this.showContextMenu(e, row.dataset.id);
        });
    }

    /**
     * Set sort column/direction.
     */
    setSort(column) {
        if (this.sortColumn === column) {
            this.sortDirection = this.sortDirection === 'asc' ? 'desc' : 'asc';
        } else {
            this.sortColumn = column;
            this.sortDirection = 'desc';
        }

        // Re-render header and refresh
        const headerRow = $('download-header');
        if (headerRow) {
            headerRow.innerHTML = this.renderHeaderRow();
        }
        this.refresh();
    }

    /**
     * Set page number.
     */
    setPage(page) {
        this.page = page;
        this.refresh();
    }

    /**
     * Filter downloads by query.
     */
    setFilter(query) {
        this.filterQuery = query.toLowerCase();
        this.page = 1;
        this.refresh();
    }

    /**
     * Refresh download list.
     */
    async refresh() {
        const listEl = $('download-list');
        if (!listEl) return;

        try {
            const data = await api.list();
            let tasks = data.downloads || [];

            // Filter
            if (this.filterQuery) {
                tasks = tasks.filter(t =>
                    t.filename.toLowerCase().includes(this.filterQuery) ||
                    t.id.toLowerCase().includes(this.filterQuery)
                );
            }

            // Sort
            tasks = this.sortTasks(tasks);

            // Paginate
            const total = tasks.length;
            const totalPages = Math.ceil(total / this.limit) || 1;
            if (this.page > totalPages) this.page = totalPages;

            const start = (this.page - 1) * this.limit;
            const paginated = tasks.slice(start, start + this.limit);

            // Render
            this.renderList(paginated, listEl);
            this.renderPagination(this.page, totalPages, total);

            // Update state for other components
            state.set('downloads', tasks);

        } catch (e) {
            listEl.innerHTML = `<tr><td colspan="8" style="padding: 2rem; color: #ff5252; text-align: center;">Connection error: ${escapeHtml(e.message)}</td></tr>`;
        }
    }

    /**
     * Sort tasks array.
     */
    sortTasks(tasks) {
        const col = this.sortColumn;
        const dir = this.sortDirection === 'asc' ? 1 : -1;

        return tasks.sort((a, b) => {
            let valA = a[col];
            let valB = b[col];

            // Handle nested fields
            if (col === 'size') {
                valA = a.size?.total || 0;
                valB = b.size?.total || 0;
            } else if (col === 'speed') {
                valA = a.speed?.bytes_per_sec || 0;
                valB = b.speed?.bytes_per_sec || 0;
            } else if (col === 'eta') {
                valA = a.eta?.seconds || 999999;
                valB = b.eta?.seconds || 999999;
            }

            if (typeof valA === 'string') {
                return valA.localeCompare(valB) * dir;
            }
            return (valA - valB) * dir;
        });
    }

    /**
     * Render download list rows.
     */
    renderList(tasks, container) {
        if (!tasks || tasks.length === 0) {
            container.innerHTML = `<tr><td colspan="8" style="padding: 4rem; text-align: center; color: var(--text-muted);">
                <span class="material-icons" style="font-size: 48px; display: block; margin-bottom: 1rem; opacity: 0.2;">inbox</span>
                No active downloads in queue.
            </td></tr>`;
            return;
        }

        container.innerHTML = tasks.map(t => this.renderRow(t)).join('');
    }

    /**
     * Render a single download row.
     */
    renderRow(task) {
        const taskState = task.state || 'Unknown';
        const displayState = taskState.charAt(0).toUpperCase() + taskState.slice(1);

        const isDownloading = ['Downloading', 'Extracting', 'Running', 'Starting'].includes(taskState);
        const isCompleted = ['Completed', 'Finished'].includes(taskState);
        const isError = ['Error', 'Failed', 'Offline'].includes(taskState);
        const isQueued = ['Queued', 'Waiting', 'Pending'].includes(taskState);

        // Color system
        const color = isCompleted ? '#00ffa3' :
            isError ? '#FF5252' :
                isDownloading ? '#00f3ff' :
                    isQueued ? '#ffd700' : '#64748b';

        const icon = isCompleted ? 'check_circle' :
            isError ? 'report_problem' :
                isDownloading ? 'sync' :
                    isQueued ? 'hourglass_bottom' : 'pause_circle';

        // Format added date
        let addedDate = '-';
        if (task.added) {
            const timestamp = typeof task.added === 'number' ? task.added : parseInt(task.added);
            if (timestamp > 1000000) {
                addedDate = formatDate(timestamp);
            }
        }

        const filename = escapeHtml(task.filename);
        const taskId = escapeAttr(task.id);
        const progress = parseFloat(task.progress) || 0;
        const sizeFormatted = task.size?.formatted_total || '-';
        const speedFormatted = isDownloading ? (task.speed?.formatted || '-') + '/s' : '-';
        const etaFormatted = isDownloading ? (task.eta?.formatted || '-') : '-';

        return `
        <tr class="transfer-row" data-id="${taskId}" style="transition: background 0.2s; cursor: pointer;">
            <td style="padding: 0.5rem 1.25rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600;" title="${filename}">
                ${filename}
            </td>
            <td style="padding: 0.5rem 0.5rem;">
                <span style="background: ${color}15; color: ${color}; padding: 2px 6px; border-radius: 4px; font-size: 0.55rem; font-weight: 800; border: 1px solid ${color}30; display: inline-flex; align-items: center; gap: 3px; text-transform: uppercase;">
                    <span class="material-icons" style="font-size: 9px;">${icon}</span> ${displayState}
                </span>
            </td>
            <td style="padding: 0.5rem 0.5rem; color: var(--text-secondary); font-size: 0.7rem;">${sizeFormatted}</td>
            <td style="padding: 0.5rem 0.5rem;">
                <div style="display: flex; justify-content: space-between; font-size: 0.55rem; margin-bottom: 2px; font-weight: 700; opacity: 0.8;">
                    <span>${progress.toFixed(1)}%</span>
                </div>
                <div style="height: 3px; background: rgba(255,255,255,0.05); border-radius: 2px; overflow: hidden;">
                    <div style="height: 100%; width: ${progress}%; background: ${color}; box-shadow: 0 0 6px ${color}80"></div>
                </div>
            </td>
            <td style="padding: 0.5rem 0.5rem; color: ${color}; font-family: var(--font-mono); font-size: 0.7rem; font-weight: 700;">${speedFormatted}</td>
            <td style="padding: 0.5rem 0.5rem; color: var(--text-secondary); font-size: 0.7rem;">${etaFormatted}</td>
            <td style="padding: 0.5rem 0.5rem; color: var(--text-muted); font-size: 0.65rem;">${addedDate}</td>
            <td style="padding: 0.5rem 1.25rem; text-align: right;">
                <button class="icon-btn-tiny more-btn">
                    <span class="material-icons" style="font-size: 14px;">more_vert</span>
                </button>
            </td>
        </tr>
        `;
    }

    /**
     * Render pagination.
     */
    renderPagination(current, total, totalItems) {
        const el = $('downloads-pagination');
        if (!el) return;

        const start = totalItems === 0 ? 0 : (current - 1) * this.limit + 1;
        const end = Math.min(current * this.limit, totalItems);

        el.innerHTML = `
            <div style="display: flex; gap: 1.5rem; align-items: center;">
                <span class="page-info" style="font-size: 0.7rem; color: var(--text-muted); letter-spacing: 0.05em; font-weight: 700;">Showing ${start}-${end} of ${totalItems} items</span>
                <div style="display: flex; gap: .5rem; background: rgba(255,255,255,0.03); padding: 4px; border-radius: 8px; border: 1px solid rgba(255,255,255,0.05);">
                    <button class="icon-btn-tiny" data-page="${current - 1}" ${current === 1 ? 'disabled' : ''} style="background: transparent;">
                        <span class="material-icons" style="font-size: 16px;">chevron_left</span>
                    </button>
                    <div style="display: flex; align-items: center; padding: 0 .5rem; font-weight: 800; font-size: 0.7rem; color: var(--color-primary); font-family: var(--font-mono);">${current} / ${total}</div>
                    <button class="icon-btn-tiny" data-page="${current + 1}" ${current === total ? 'disabled' : ''} style="background: transparent;">
                        <span class="material-icons" style="font-size: 16px;">chevron_right</span>
                    </button>
                </div>
            </div>
        `;

        // Bind pagination events
        delegate(el, 'click', '[data-page]', (e, btn) => {
            if (!btn.disabled) {
                this.setPage(parseInt(btn.dataset.page));
            }
        });
    }

    /**
     * Perform batch action.
     */
    async batchAction(action) {
        try {
            await api.batch(action);
            setTimeout(() => this.refresh(), 500);
        } catch (e) {
            console.error('Batch action failed:', e);
        }
    }

    /**
     * Perform task action.
     */
    async taskAction(taskId, action) {
        try {
            await api.action(taskId, action);
            this.refresh();
        } catch (e) {
            console.error('Task action failed:', e);
        }
    }

    /**
     * Show context menu for a task.
     */
    showContextMenu(event, taskId) {
        // For now, delegate to global router
        // In future, implement standalone context menu
        if (window.router && window.router.showContextMenu) {
            const task = state.get('downloads')?.find(t => t.id === taskId);
            if (task) {
                window.router.showContextMenu(event, task);
            }
        }
    }

    /**
     * Show task info modal.
     */
    showTaskInfo(taskId) {
        if (window.router && window.router.showTaskInfo) {
            const task = state.get('downloads')?.find(t => t.id === taskId);
            if (task) {
                window.router.showTaskInfo(task);
            }
        }
    }

    /**
     * Start polling for updates.
     */
    startPolling() {
        this.stopPolling();
        this.pollInterval = setInterval(() => {
            if (this.mounted) {
                this.refresh();
            } else {
                this.stopPolling();
            }
        }, 2000);
    }

    /**
     * Stop polling.
     */
    stopPolling() {
        if (this.pollInterval) {
            clearInterval(this.pollInterval);
            this.pollInterval = null;
        }
    }
}

// Export singleton instance
export const downloadsView = new DownloadsView();
