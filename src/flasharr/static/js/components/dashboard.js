/**
 * Flasharr: Dashboard Component
 * 
 * Main dashboard view with trending carousel, active downloads,
 * account overview, and traffic statistics.
 */

import { downloads, tmdb, accounts, discovery } from '../core/api.js';
import { state } from '../core/state.js';
import { ws } from '../core/websocket.js';
import { escapeHtml } from '../utils/sanitize.js';
import { formatBytes, formatSpeed, formatPercent } from '../utils/format.js';
import { $, delegate } from '../utils/dom.js';

/**
 * Dashboard view component.
 */
export class DashboardView {
    constructor() {
        this.container = null;
        this.mounted = false;
        this.trafficChart = null;
        this.trafficData = [];
        this.pollInterval = null;
    }

    /**
     * Mount the view.
     */
    mount(container) {
        this.container = container;
        this.mounted = true;
        this.render();
        this.startPolling();
        this.setupWebSocket();
    }

    /**
     * Unmount the view.
     */
    unmount() {
        this.stopPolling();
        this.mounted = false;
        this.container = null;
        if (this.trafficChart) {
            this.trafficChart.destroy();
            this.trafficChart = null;
        }
    }

    /**
     * Render the dashboard.
     */
    render() {
        if (!this.container) return;

        this.container.innerHTML = `
            <div style="padding: 1.5rem; height: 100%; box-sizing: border-box; display: flex; flex-direction: column; gap: 1rem; overflow: hidden;">
                <!-- Trending Carousel -->
                <div class="box-section" style="margin-bottom: 1rem; border-color: rgba(255, 215, 0, 0.3); min-height: 256px; display: flex; flex-direction: column; justify-content: center;">
                    <div class="box-label" style="color: #ffd700;">
                        <span class="material-icons">trending_up</span>
                        Trending This Week
                    </div>
                    <div class="carousel-container" style="margin-top: 0.5rem; flex: 1;">
                        <button class="carousel-btn prev" id="carousel-prev" disabled>
                            <span class="material-icons">chevron_left</span>
                        </button>
                        <div class="carousel-track" id="trending-carousel">
                            <div class="loading-container" style="width: 100%; height: 100%;">
                                <div class="loading-spinner"></div>
                            </div>
                        </div>
                        <button class="carousel-btn next" id="carousel-next" disabled>
                            <span class="material-icons">chevron_right</span>
                        </button>
                    </div>
                </div>

                <!-- Main Dashboard Grid -->
                <div style="display: grid; grid-template-columns: 6.5fr 3.5fr; gap: 1rem; flex: 1; min-height: 0; margin-bottom: 0;">
                    
                    <!-- Left Column: Active Downloads -->
                    <div class="box-section" style="border-color: rgba(0,243,255,0.15);">
                        <div class="box-label" style="color: var(--color-secondary);">
                            <span class="material-icons" style="font-size: 14px;">list_alt</span>
                            Active Downloads
                        </div>
                        
                        <div style="position: absolute; top: 0.6rem; right: 0.8rem; z-index: 20;">
                            <button class="nav-item expand-btn" style="padding: 2px 8px; border-radius: 4px; font-size: 0.6rem; font-weight: 800; border: 1px solid rgba(255,255,255,0.1); background: rgba(0,0,0,0.3); cursor: pointer; color: var(--text-muted);">EXPAND</button>
                        </div>

                        <div id="minified-queue" style="display: grid; grid-template-columns: repeat(2, 1fr); gap: 0.75rem; overflow-y: auto; padding-right: 4px; padding-top: 0.5rem; height: 100%;">
                            <div class="loading-container" style="grid-column: 1 / -1;">
                                <div class="loading-spinner" style="transform: scale(0.5);"></div>
                            </div>
                        </div>
                    </div>

                    <!-- Right Column -->
                    <div style="display: grid; grid-template-rows: 1.84fr 2.76fr; gap: 1rem;">
                        
                        <!-- Account Overview -->
                        <div class="box-section" style="border-color: rgba(0,243,255,0.2); gap: 1rem; height: 100%;">
                            <div class="box-label" style="color: var(--color-primary);">
                                <span class="material-icons">shield</span>
                                Account Overview
                            </div>
                            
                            <!-- Identity Header -->
                            <div style="display: flex; align-items: center; gap: 0.75rem; margin-top: 0.25rem;">
                                <div style="width: 36px; height: 36px; border-radius: 50%; background: rgba(0,243,255,0.05); display: flex; align-items: center; justify-content: center; border: 1px solid var(--color-primary);">
                                    <span class="material-icons" style="font-size: 18px; color: var(--color-primary);">person</span>
                                </div>
                                <div style="flex: 1; overflow: hidden; line-height: 1.2;">
                                    <div id="user-email" style="font-family: var(--font-mono); font-size: 0.7rem; color: var(--text-primary); font-weight: 700; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">Syncing...</div>
                                    <div style="display: flex; gap: 0.5rem; justify-content: space-between; align-items: center;">
                                        <span id="user-rank" style="font-size: 0.55rem; color: var(--color-secondary); font-weight: 800;">AUTH...</span>
                                        <span id="user-expiry" style="font-size: 0.55rem; color: var(--text-muted);">Checking validity...</span>
                                    </div>
                                </div>
                            </div>

                            <!-- Stats Summary -->
                            <div style="display: grid; grid-template-columns: repeat(3, 1fr); gap: 0.5rem;">
                                <div style="background: rgba(0,255,163,0.02); padding: 0.35rem; border-radius: 4px; border: 1px solid rgba(0,255,163,0.05); text-align: center;">
                                    <div style="font-size: 0.45rem; color: var(--text-muted); text-transform: uppercase;">Active</div>
                                    <div id="stat-active" style="font-size: 0.85rem; font-weight: 800; color: var(--color-secondary); font-family: var(--font-mono);">0</div>
                                </div>
                                <div style="background: rgba(255,255,255,0.02); padding: 0.35rem; border-radius: 4px; border: 1px solid rgba(255,255,255,0.05); text-align: center;">
                                    <div style="font-size: 0.45rem; color: var(--text-muted); text-transform: uppercase;">Queued</div>
                                    <div id="stat-queued" style="font-size: 0.85rem; font-weight: 800; color: #ffd700; font-family: var(--font-mono);">0</div>
                                </div>
                                <div style="background: rgba(255,255,255,0.02); padding: 0.35rem; border-radius: 4px; border: 1px solid rgba(255,255,255,0.05); text-align: center;">
                                    <div style="font-size: 0.45rem; color: var(--text-muted); text-transform: uppercase;">Done</div>
                                    <div id="stat-completed" style="font-size: 0.85rem; font-weight: 800; color: var(--color-primary); font-family: var(--font-mono);">0</div>
                                </div>
                            </div>

                            <!-- Quota Bar -->
                            <div style="margin-top: auto;">
                                <div style="display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 0.2rem;">
                                    <span style="font-size: 0.55rem; font-weight: 700; color: #ffd700; text-transform: uppercase;">Daily Quota</span>
                                    <div style="font-family: var(--font-mono); font-size: 0.6rem; font-weight: 800; color: var(--text-primary);">
                                        <span id="quota-percent">0</span>%
                                    </div>
                                </div>
                                <div id="quota-gauge" class="fuel-gauge" style="height: 4px; background: rgba(0,0,0,0.3); border-radius: 2px; overflow: hidden; position: relative; border: 1px solid rgba(255,255,255,0.03);">
                                    <div id="quota-bar" style="height: 100%; width: 0%; background: linear-gradient(90deg, #ffd700, #ff8c00); box-shadow: 0 0 6px rgba(255, 215, 0, 0.3); border-radius: 2px; transition: width 1s ease;"></div>
                                </div>
                                <div id="quota-text" style="font-family: var(--font-mono); font-size: 0.55rem; color: var(--text-muted); text-align: right; margin-top: 1px;">0 GB / 0 GB</div>
                            </div>
                        </div>

                        <!-- Traffic Chart -->
                        <div class="box-section" style="border-color: rgba(255,255,255,0.1); height: 100%;">
                            <div class="box-label" style="color: var(--text-primary);">
                                <span class="material-icons">insights</span>
                                Netflow Statistic
                            </div>
                            <div id="traffic-chart-container" style="flex: 1; position: relative; padding-top: 0.5rem; min-height: 0;">
                                <canvas id="trafficChart"></canvas>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        `;

        this.bindEvents();
        this.fetchData();
        this.initTrafficChart();
    }

    /**
     * Bind event handlers.
     */
    bindEvents() {
        // Expand button
        delegate(this.container, 'click', '.expand-btn', () => {
            if (window.router) {
                window.router.navigate('downloads');
            }
        });

        // Carousel navigation
        delegate(this.container, 'click', '#carousel-prev', () => this.carouselPrev());
        delegate(this.container, 'click', '#carousel-next', () => this.carouselNext());
    }

    /**
     * Fetch dashboard data.
     */
    async fetchData() {
        await Promise.all([
            this.fetchTrending(),
            this.fetchActiveDownloads(),
            this.fetchAccountStatus()
        ]);
    }

    /**
     * Fetch trending content.
     */
    async fetchTrending() {
        const carousel = $('trending-carousel');
        if (!carousel) return;

        try {
            const data = await tmdb.trending('all', 'week');
            if (data.results && data.results.length > 0) {
                const items = data.results.slice(0, 12);
                carousel.innerHTML = items.map(item => this.renderPosterCard(item)).join('');
                this.updateCarouselButtons();
            }
        } catch (e) {
            carousel.innerHTML = '<div style="color: var(--text-muted); padding: 2rem;">Failed to load trending</div>';
        }
    }

    /**
     * Render a poster card.
     */
    renderPosterCard(item) {
        const title = escapeHtml(item.title || item.name || 'Unknown');
        const type = item.media_type || (item.title ? 'movie' : 'tv');
        const posterPath = item.poster_path
            ? `https://image.tmdb.org/t/p/w300${item.poster_path}`
            : '/static/images/no-poster.png';
        const rating = item.vote_average ? item.vote_average.toFixed(1) : '-';

        return `
            <div class="poster-card" data-type="${type}" data-id="${item.id}" style="cursor: pointer;">
                <img src="${posterPath}" alt="${title}" loading="lazy" style="width: 100%; aspect-ratio: 2/3; object-fit: cover; border-radius: 8px;">
                <div class="poster-overlay">
                    <div class="poster-title">${title}</div>
                    <div class="poster-rating">⭐ ${rating}</div>
                </div>
            </div>
        `;
    }

    /**
     * Carousel navigation.
     */
    carouselNext() {
        const track = $('trending-carousel');
        if (track) {
            track.scrollBy({ left: 200, behavior: 'smooth' });
            setTimeout(() => this.updateCarouselButtons(), 300);
        }
    }

    carouselPrev() {
        const track = $('trending-carousel');
        if (track) {
            track.scrollBy({ left: -200, behavior: 'smooth' });
            setTimeout(() => this.updateCarouselButtons(), 300);
        }
    }

    updateCarouselButtons() {
        const track = $('trending-carousel');
        const prevBtn = $('carousel-prev');
        const nextBtn = $('carousel-next');

        if (track && prevBtn && nextBtn) {
            prevBtn.disabled = track.scrollLeft <= 0;
            nextBtn.disabled = track.scrollLeft >= track.scrollWidth - track.clientWidth - 10;
        }
    }

    /**
     * Fetch active downloads.
     */
    async fetchActiveDownloads() {
        const queue = $('minified-queue');
        if (!queue) return;

        try {
            const data = await downloads.list();
            const tasks = (data.downloads || [])
                .filter(t => ['Downloading', 'Queued', 'Starting', 'Extracting'].includes(t.state))
                .slice(0, 6);

            if (tasks.length === 0) {
                queue.innerHTML = '<div style="grid-column: 1 / -1; text-align: center; color: var(--text-muted); padding: 2rem;">No active downloads</div>';
            } else {
                queue.innerHTML = tasks.map(t => this.renderMiniTask(t)).join('');
            }
        } catch (e) {
            queue.innerHTML = '<div style="grid-column: 1 / -1; color: var(--text-muted);">Failed to load</div>';
        }
    }

    /**
     * Render mini task card.
     */
    renderMiniTask(task) {
        const progress = parseFloat(task.progress) || 0;
        const isActive = task.state === 'Downloading';
        const color = isActive ? 'var(--color-primary)' : '#ffd700';

        return `
            <div class="mini-task" style="background: rgba(255,255,255,0.02); padding: 0.5rem; border-radius: 8px; border: 1px solid rgba(255,255,255,0.05);">
                <div style="font-size: 0.65rem; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin-bottom: 0.25rem;">${escapeHtml(task.filename)}</div>
                <div style="height: 3px; background: rgba(255,255,255,0.1); border-radius: 2px; overflow: hidden;">
                    <div style="height: 100%; width: ${progress}%; background: ${color};"></div>
                </div>
                <div style="display: flex; justify-content: space-between; margin-top: 0.25rem; font-size: 0.55rem; color: var(--text-muted);">
                    <span>${progress.toFixed(1)}%</span>
                    <span>${isActive ? (task.speed?.formatted || '-') + '/s' : task.state}</span>
                </div>
            </div>
        `;
    }

    /**
     * Fetch account status.
     */
    async fetchAccountStatus() {
        try {
            const data = await accounts.status();
            this.updateAccountUI(data);
        } catch (e) {
            console.error('Failed to fetch account status:', e);
        }
    }

    /**
     * Update account UI elements.
     */
    updateAccountUI(data) {
        const emailEl = $('user-email');
        const rankEl = $('user-rank');
        const expiryEl = $('user-expiry');

        if (data.email && emailEl) emailEl.textContent = data.email;
        if (data.account_type && rankEl) rankEl.textContent = data.account_type.toUpperCase();
        if (data.expire_date && expiryEl) {
            const expDate = new Date(data.expire_date);
            expiryEl.textContent = `Expires: ${expDate.toLocaleDateString()}`;
        }

        // Update quota
        if (data.t) {
            this.updateQuota(data.t);
        }
    }

    /**
     * Update quota display.
     */
    updateQuota(traffic) {
        const remaining = traffic.remaining || 0;
        const total = traffic.total || 0;
        const percent = total > 0 ? (remaining / total) * 100 : 0;

        const percentEl = $('quota-percent');
        const barEl = $('quota-bar');
        const textEl = $('quota-text');

        if (percentEl) percentEl.textContent = percent.toFixed(1);
        if (barEl) barEl.style.width = `${percent}%`;
        if (textEl) {
            textEl.textContent = `${formatBytes(remaining * 1024 * 1024 * 1024)} / ${formatBytes(total * 1024 * 1024 * 1024)}`;
        }

        state.update('quota', { remaining, total, percentage: percent });
    }

    /**
     * Update stats from WebSocket.
     */
    updateStats(stats) {
        const activeEl = $('stat-active');
        const queuedEl = $('stat-queued');
        const completedEl = $('stat-completed');

        if (activeEl) activeEl.textContent = stats.active_downloads || 0;
        if (queuedEl) queuedEl.textContent = stats.queue_size || 0;
        if (completedEl) completedEl.textContent = stats.completed || 0;
    }

    /**
     * Setup WebSocket listeners.
     */
    setupWebSocket() {
        ws.on('engine_stats', (stats) => {
            if (this.mounted) {
                this.updateStats(stats);
                this.addTrafficDataPoint(stats.total_speed || 0);
            }
        });

        ws.on('account_status', (data) => {
            if (this.mounted && data.t) {
                this.updateQuota(data.t);
            }
        });
    }

    /**
     * Initialize traffic chart.
     */
    initTrafficChart() {
        const canvas = $('trafficChart');
        if (!canvas || typeof Chart === 'undefined') return;

        const ctx = canvas.getContext('2d');

        // Initialize with empty data
        this.trafficData = new Array(30).fill(0);

        this.trafficChart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: this.trafficData.map(() => ''),
                datasets: [{
                    label: 'Speed',
                    data: this.trafficData,
                    borderColor: '#00f3ff',
                    backgroundColor: 'rgba(0, 243, 255, 0.1)',
                    borderWidth: 2,
                    fill: true,
                    tension: 0.4,
                    pointRadius: 0
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: { display: false }
                },
                scales: {
                    x: { display: false },
                    y: {
                        display: true,
                        grid: { color: 'rgba(255,255,255,0.05)' },
                        ticks: {
                            callback: (v) => formatBytes(v) + '/s',
                            color: 'rgba(255,255,255,0.5)',
                            font: { size: 10 }
                        }
                    }
                }
            }
        });
    }

    /**
     * Add data point to traffic chart.
     */
    addTrafficDataPoint(speed) {
        if (!this.trafficChart) return;

        this.trafficData.push(speed);
        if (this.trafficData.length > 30) {
            this.trafficData.shift();
        }

        this.trafficChart.data.datasets[0].data = this.trafficData;
        this.trafficChart.update('none');
    }

    /**
     * Start polling.
     */
    startPolling() {
        this.pollInterval = setInterval(() => {
            if (this.mounted) {
                this.fetchActiveDownloads();
            }
        }, 5000);
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

// Export singleton
export const dashboardView = new DashboardView();
