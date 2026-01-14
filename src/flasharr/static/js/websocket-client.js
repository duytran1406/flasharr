/**
 * Optimized WebSocket Client for Dashboard
 * 
 * Features:
 * - Automatic reconnection
 * - Selective subscriptions
 * - Delta updates (only changed data)
 * - Minimal bandwidth usage
 * - Event batching support
 */

if (typeof window.FshareWebSocketClient === 'undefined') {
    window.FshareWebSocketClient = class {
        constructor(url = null) {
            // Singleton check
            if (window.fshareWSInstance) {
                console.log('Using existing WebSocket client instance');
                return window.fshareWSInstance;
            }

            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            this.url = url || `${protocol}//${window.location.host}/ws`;
            this.ws = null;
            this.clientId = null;
            this.reconnectDelay = 1000;
            this.maxReconnectDelay = 30000;
            this.reconnectAttempts = 0;
            this.subscriptions = [];
            this.eventHandlers = {};
            this.connected = false;

            // Task cache for delta updates
            this.taskCache = {};

            // Stats cache
            this.statsCache = {};

            window.fshareWSInstance = this;
        }

        /**
         * Connect to WebSocket server
         */
        connect() {
            try {
                this.ws = new WebSocket(this.url);

                this.ws.onopen = () => this.onOpen();
                this.ws.onmessage = (event) => this.onMessage(event);
                this.ws.onclose = () => this.onClose();
                this.ws.onerror = (error) => this.onError(error);

            } catch (error) {
                console.error('WebSocket connection error:', error);
                this.scheduleReconnect();
            }
        }

        /**
         * Disconnect from server
         */
        disconnect() {
            if (this.ws) {
                this.ws.close();
                this.ws = null;
            }
            this.connected = false;
        }

        /**
         * Subscribe to specific event types
         * @param {Array<string>} events - Event types to subscribe to
         */
        subscribe(events) {
            this.subscriptions = events;

            if (this.connected) {
                this.send({
                    t: 'sb',
                    d: { subscribe: events }
                });
            }
        }

        /**
         * Register event handler
         * @param {string} eventType - Event type (e.g., 'tu' for task update)
         * @param {Function} handler - Handler function
         */
        on(eventType, handler) {
            if (!this.eventHandlers[eventType]) {
                this.eventHandlers[eventType] = [];
            }
            this.eventHandlers[eventType].push(handler);
        }

        /**
         * Send message to server
         */
        send(data) {
            if (this.ws && this.ws.readyState === WebSocket.OPEN) {
                this.ws.send(JSON.stringify(data));
            }
        }

        /**
         * Handle connection open
         */
        onOpen() {
            console.log('WebSocket connected');
            this.connected = true;
            this.reconnectAttempts = 0;
            this.reconnectDelay = 1000;

            // Subscribe to events
            if (this.subscriptions.length > 0) {
                this.subscribe(this.subscriptions);
            }

            // Trigger connected event
            this.trigger('connected', { clientId: this.clientId });
        }

        /**
         * Handle incoming message
         */
        onMessage(event) {
            try {
                if (!event.data || event.data === 'null') return;
                const message = JSON.parse(event.data);
                if (!message) return;
                const eventType = message.t;
                const data = message.d;

                // Handle specific event types
                switch (eventType) {
                    case 'cn': // Connected
                        this.clientId = data.id;
                        break;

                    case 'tu': // Task updated
                        this.handleTaskUpdate(data);
                        break;

                    case 'ta': // Task added
                        this.handleTaskAdded(data);
                        break;

                    case 'tr': // Task removed
                        this.handleTaskRemoved(data);
                        break;

                    case 'es': // Engine stats
                        this.handleEngineStats(data);
                        break;

                    case 'as': // Account status
                        this.handleAccountStatus(data);
                        break;

                    case 'sa': // Sync all
                        this.handleSyncAll(data);
                        break;

                    case 'hb': // Heartbeat
                        // Respond to heartbeat
                        this.send({ t: 'hb' });
                        break;

                    case 'batch': // Batched messages
                        if (Array.isArray(data)) {
                            data.forEach(item => {
                                if (item && item.t) {
                                    // Re-route through dispatcher logic but without re-parsing
                                    this.dispatch(item.t, item.d);
                                }
                            });
                        }
                        break;
                }

                // Trigger registered handlers
                this.trigger(eventType, data);

            } catch (error) {
                console.error('Error parsing WebSocket message:', error);
            }
        }

        /**
         * Handle task update (delta)
         */
        handleTaskUpdate(delta) {
            const taskId = delta.i;

            // Get or create task in cache
            if (!this.taskCache[taskId]) {
                this.taskCache[taskId] = { i: taskId };
            }

            // Apply delta to cached task
            Object.assign(this.taskCache[taskId], delta);

            // Trigger update with full task data
            this.trigger('task_update', this.taskCache[taskId]);
        }

        /**
         * Handle task added
         */
        handleTaskAdded(data) {
            const taskId = data.i;
            this.taskCache[taskId] = data;
            this.trigger('task_added', data);
        }

        /**
         * Handle task removed
         */
        handleTaskRemoved(data) {
            const taskId = data.i;
            delete this.taskCache[taskId];
            this.trigger('task_removed', { taskId });
        }

        /**
         * Handle engine stats
         */
        handleEngineStats(data) {
            // Update stats cache
            Object.assign(this.statsCache, data);
            this.trigger('engine_stats', this.statsCache);
        }

        /**
         * Handle account status
         */
        handleAccountStatus(data) {
            this.trigger('account_status', data);
        }

        /**
         * Handle full state sync
         */
        handleSyncAll(tasks) {
            this.taskCache = {};
            if (Array.isArray(tasks)) {
                tasks.forEach(t => {
                    if (t && t.i) this.taskCache[t.i] = t;
                });
            }
            this.trigger('sync_all', tasks);
        }

        /**
         * Handle connection close
         */
        onClose() {
            console.log('WebSocket disconnected');
            this.connected = false;
            this.trigger('disconnected');
            this.scheduleReconnect();
        }

        /**
         * Handle error
         */
        onError(error) {
            console.error('WebSocket error:', error);
            this.trigger('error', error);
        }

        /**
         * Schedule reconnection
         */
        scheduleReconnect() {
            this.reconnectAttempts++;

            // Exponential backoff
            const delay = Math.min(
                this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1),
                this.maxReconnectDelay
            );

            console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})...`);

            setTimeout(() => {
                this.connect();
            }, delay);
        }

        /**
         * Trigger event handlers
         */
        trigger(eventType, data) {
            const handlers = this.eventHandlers[eventType] || [];
            handlers.forEach(handler => {
                try {
                    handler(data);
                } catch (error) {
                    console.error(`Error in ${eventType} handler:`, error);
                }
            });
        }

        /**
         * Get task from cache
         */
        getTask(taskId) {
            return this.taskCache[taskId];
        }

        /**
         * Get all tasks from cache
         */
        getAllTasks() {
            return Object.values(this.taskCache);
        }

        /**
         * Get current stats
         */
        getStats() {
            return this.statsCache;
        }

        /**
         * Clear cache
         */
        clearCache() {
            this.taskCache = {};
            this.statsCache = {};
        }

        /**
         * Central dispatcher for events
         */
        dispatch(eventType, data) {
            // Handle specific internal logic
            switch (eventType) {
                case 'cn': this.clientId = data.id; break;
                case 'tu': this.handleTaskUpdate(data); break;
                case 'ta': this.handleTaskAdded(data); break;
                case 'tr': this.handleTaskRemoved(data); break;
                case 'es': this.handleEngineStats(data); break;
                case 'as': this.handleAccountStatus(data); break;
                case 'hb': this.send({ t: 'hb' }); break;
            }

            // Trigger user-defined handlers
            this.trigger(eventType, data);
        }
    }
}

// Helper functions for decoding compact data

/**
 * Decode state abbreviation
 */
function decodeState(s) {
    const states = {
        'Queued': 'Queued',
        'Starting': 'Starting',
        'Downloading': 'Downloading',
        'Paused': 'Paused',
        'Completed': 'Completed',
        'Failed': 'Failed',
        'Cancelled': 'Cancelled',
        'Waiting': 'Waiting',
        'Skipped': 'Skipped',
        'TempOffline': 'Temp Offline',
        'Extracting': 'Extracting',
        'Finished': 'Finished',
        'Offline': 'Offline'
    };
    return states[s] || s;
}

/**
 * Decode priority abbreviation
 */
function decodePriority(pr) {
    const priorities = {
        'L': 'Low',
        'N': 'Normal',
        'H': 'High',
        'U': 'Urgent'
    };
    return priorities[pr] || 'Normal';
}

/**
 * Format bytes to human readable
 */
function formatBytes(bytes) {
    if (!bytes || bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
}

/**
 * Format speed
 */
function formatSpeed(bytesPerSec) {
    if (!bytesPerSec || bytesPerSec === 0) return '0 B/s';
    return formatBytes(bytesPerSec) + '/s';
}

/**
 * Format ETA
 */
function formatETA(seconds) {
    if (!seconds || seconds === 0) return '-';
    if (seconds < 60) return `${Math.round(seconds)}s`;
    if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
    return `${Math.round(seconds / 3600)}h`;
}

// Export for use in other scripts
if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
        FshareWebSocketClient,
        decodeState,
        decodePriority,
        formatBytes,
        formatSpeed,
        formatETA
    };
}

