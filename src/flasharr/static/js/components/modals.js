/**
 * Flasharr: Modal System Component
 * 
 * Centralized modal management for dialogs, confirmations, and info panels.
 */

import { escapeHtml } from '../utils/sanitize.js';
import { $ } from '../utils/dom.js';

/**
 * Modal manager singleton.
 */
class ModalManager {
    constructor() {
        this.overlay = null;
        this.container = null;
        this.onConfirmCallback = null;
    }

    /**
     * Initialize modal elements.
     */
    init() {
        this.overlay = $('modal-overlay');
        this.container = $('modal-container');

        // Close on overlay click
        if (this.overlay) {
            this.overlay.addEventListener('click', (e) => {
                if (e.target === this.overlay) {
                    this.hide();
                }
            });
        }

        // Close on Escape key
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Escape' && this.isOpen()) {
                this.hide();
            }
        });
    }

    /**
     * Check if modal is currently open.
     */
    isOpen() {
        return this.overlay?.classList.contains('active');
    }

    /**
     * Show a modal.
     * 
     * @param {Object} options - Modal options
     * @param {string} options.title - Modal title
     * @param {string} options.body - Modal body HTML
     * @param {string} options.footer - Modal footer HTML (optional)
     * @param {Function} options.onConfirm - Confirm callback (optional)
     */
    show({ title, body, footer, onConfirm }) {
        if (!this.overlay || !this.container) {
            this.init();
        }

        if (!this.container) {
            console.error('Modal container not found');
            return;
        }

        this.onConfirmCallback = onConfirm;

        const defaultFooter = `
            <button class="modal-btn secondary" data-action="close">Close</button>
        `;

        this.container.innerHTML = `
            <div class="modal-header">
                <h3>${escapeHtml(title)}</h3>
                <button class="icon-btn-tiny" data-action="close">
                    <span class="material-icons">close</span>
                </button>
            </div>
            <div class="modal-body">${body}</div>
            <div class="modal-footer">${footer || defaultFooter}</div>
        `;

        // Bind close buttons
        this.container.querySelectorAll('[data-action="close"]').forEach(btn => {
            btn.addEventListener('click', () => this.hide());
        });

        // Bind confirm buttons
        this.container.querySelectorAll('[data-action="confirm"]').forEach(btn => {
            btn.addEventListener('click', () => {
                if (this.onConfirmCallback) {
                    this.onConfirmCallback();
                }
            });
        });

        this.overlay.classList.add('active');
        this.container.classList.add('active');

        // Focus first input if present
        const firstInput = this.container.querySelector('input, textarea, select');
        if (firstInput) {
            setTimeout(() => firstInput.focus(), 100);
        }
    }

    /**
     * Hide the modal.
     */
    hide() {
        if (this.overlay) {
            this.overlay.classList.remove('active');
        }
        if (this.container) {
            this.container.classList.remove('active');
        }
        this.onConfirmCallback = null;
    }

    /**
     * Show an error message modal.
     * 
     * @param {string} message - Error message
     */
    showError(message) {
        this.show({
            title: 'System Error',
            body: `
                <div style="color: #FF5252; display: flex; align-items: center; gap: 1rem;">
                    <span class="material-icons" style="font-size: 48px;">error_outline</span>
                    <p>${escapeHtml(message)}</p>
                </div>
            `,
            footer: '<button class="modal-btn primary" data-action="close">Acknowledged</button>'
        });
    }

    /**
     * Show a confirmation dialog.
     * 
     * @param {Object} options - Confirmation options
     * @param {string} options.title - Dialog title
     * @param {string} options.message - Confirmation message
     * @param {Function} options.onConfirm - Confirm callback
     * @param {string} options.confirmText - Confirm button text (default: 'Confirm')
     * @param {string} options.cancelText - Cancel button text (default: 'Cancel')
     */
    confirm({ title, message, onConfirm, confirmText = 'Confirm', cancelText = 'Cancel' }) {
        this.show({
            title,
            body: `<p style="color: var(--text-secondary);">${escapeHtml(message)}</p>`,
            footer: `
                <button class="modal-btn secondary" data-action="close">${escapeHtml(cancelText)}</button>
                <button class="modal-btn primary" data-action="confirm">${escapeHtml(confirmText)}</button>
            `,
            onConfirm: () => {
                this.hide();
                if (onConfirm) onConfirm();
            }
        });
    }

    /**
     * Show prompt for adding a new download.
     * 
     * @param {Function} onSubmit - Callback with URL
     */
    showAddDownload(onSubmit) {
        this.show({
            title: 'Initiate Link Extraction',
            body: `
                <p style="margin-bottom: 1rem; color: var(--text-secondary);">Enter Fshare.vn file or folder URL to begin processing.</p>
                <input type="text" id="add-url-input" class="modal-input" placeholder="https://www.fshare.vn/file/..." autofocus>
            `,
            footer: `
                <button class="modal-btn secondary" data-action="close">Cancel</button>
                <button class="modal-btn primary" id="confirm-add-btn">Start Download</button>
            `
        });

        const input = $('add-url-input');
        const confirmBtn = $('confirm-add-btn');

        if (input) {
            input.addEventListener('keypress', (e) => {
                if (e.key === 'Enter' && input.value.trim()) {
                    this.hide();
                    onSubmit(input.value.trim());
                }
            });
        }

        if (confirmBtn) {
            confirmBtn.addEventListener('click', () => {
                if (input && input.value.trim()) {
                    this.hide();
                    onSubmit(input.value.trim());
                }
            });
        }
    }

    /**
     * Show task info modal.
     * 
     * @param {Object} task - Download task object
     */
    showTaskInfo(task) {
        const progress = parseFloat(task.progress) || 0;

        this.show({
            title: 'Task Intelligence',
            body: `
                <div class="info-grid">
                    <div class="info-item">
                        <span class="info-label">Filename</span>
                        <span class="info-value" style="word-break: break-all;">${escapeHtml(task.filename)}</span>
                    </div>
                    <div class="info-item">
                        <span class="info-label">Status</span>
                        <span class="info-value" style="color: var(--color-primary)">${escapeHtml(task.state)}</span>
                    </div>
                    <div class="info-item">
                        <span class="info-label">Progress</span>
                        <div style="flex: 1;">
                            <div style="display: flex; justify-content: space-between; font-size: 0.7rem; margin-bottom: 4px;">
                                <span>${progress.toFixed(1)}%</span>
                                <span>${task.size?.formatted_total || '-'}</span>
                            </div>
                            <div style="height: 6px; background: rgba(255,255,255,0.05); border-radius: 3px; overflow: hidden;">
                                <div style="height: 100%; width: ${progress}%; background: var(--color-primary); box-shadow: 0 0 10px var(--color-primary)"></div>
                            </div>
                        </div>
                    </div>
                    <div class="info-item">
                        <span class="info-label">Source Link</span>
                        <span class="info-value"><a href="${escapeHtml(task.url)}" target="_blank" style="color: var(--color-secondary); text-decoration: none;">View on Fshare</a></span>
                    </div>
                    <div class="info-item">
                        <span class="info-label">ID</span>
                        <span class="info-value" style="font-family: var(--font-mono); font-size: 0.7rem;">${escapeHtml(task.id)}</span>
                    </div>
                </div>
            `,
            footer: `
                <button class="modal-btn secondary" data-action="close">Close</button>
                <button class="modal-btn primary" id="resume-task-btn">Resume</button>
            `
        });

        const resumeBtn = $('resume-task-btn');
        if (resumeBtn) {
            resumeBtn.addEventListener('click', () => {
                this.hide();
                if (window.router) {
                    window.router.taskAction(task.id, 'start');
                }
            });
        }
    }

    /**
     * Show connect account modal.
     * 
     * @param {Function} onSubmit - Callback with { email, password }
     */
    showConnectAccount(onSubmit) {
        this.show({
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
                <button class="modal-btn secondary" data-action="close">Cancel</button>
                <button class="modal-btn primary" id="confirm-connect-btn">Authenticate</button>
            `
        });

        const confirmBtn = $('confirm-connect-btn');
        if (confirmBtn) {
            confirmBtn.addEventListener('click', () => {
                const email = $('acc-email')?.value;
                const password = $('acc-pass')?.value;

                if (email && password) {
                    this.hide();
                    onSubmit({ email, password });
                }
            });
        }
    }
}

// Export singleton
export const modal = new ModalManager();

// Initialize when DOM is ready
if (typeof document !== 'undefined') {
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', () => modal.init());
    } else {
        modal.init();
    }
}
