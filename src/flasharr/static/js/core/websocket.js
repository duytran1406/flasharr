/**
 * Flasharr: WebSocket Client
 * 
 * Real-time communication with the backend.
 * Handles connection, reconnection, and message routing.
 */

import { state } from './state.js';

/**
 * WebSocket client with auto-reconnect and event handling.
 */
class WebSocketClient {
    constructor() {
        this.ws = null;
        this.url = null;
        this.listeners = new Map();
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 10;
        this.reconnectDelay = 3000;
        this.connected = false;
    }

    /**
     * Connect to the WebSocket server.
     * 
     * @param {string} url - WebSocket URL (optional, auto-detected if not provided)
     */
    connect(url = null) {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            console.log('WebSocket already connected');
            return;
        }

        if (!url) {
            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            url = `${protocol}//${window.location.host}/ws`;
        }

        this.url = url;
        console.log(`🔌 Connecting to ${url}`);

        try {
            this.ws = new WebSocket(url);
            this._setupHandlers();
        } catch (e) {
            console.error('WebSocket connection failed:', e);
            this._scheduleReconnect();
        }
    }

    /**
     * Set up WebSocket event handlers.
     */
    _setupHandlers() {
        this.ws.onopen = () => {
            console.log('✅ WebSocket connected');
            this.connected = true;
            this.reconnectAttempts = 0;
            this._emit('open');

            // Update UI state
            state.set('wsConnected', true);
        };

        this.ws.onclose = (event) => {
            console.log(`❌ WebSocket closed: ${event.code} ${event.reason || ''}`);
            this.connected = false;
            this._emit('close', event);

            // Update UI state
            state.set('wsConnected', false);

            // Auto-reconnect
            this._scheduleReconnect();
        };

        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
            this._emit('error', error);
        };

        this.ws.onmessage = (event) => {
            try {
                const message = JSON.parse(event.data);
                this._handleMessage(message);
            } catch (e) {
                console.error('Failed to parse WebSocket message:', e, event.data);
            }
        };
    }

    /**
     * Handle incoming message and route to listeners.
     * 
     * @param {Object} message - Parsed message object
     */
    _handleMessage(message) {
        const { type, data } = message;

        // Route to specific listeners
        this._emit(type, data);

        // Also emit to 'message' for generic handling
        this._emit('message', message);

        // Update global state based on message type
        switch (type) {
            case 'engine_stats':
                state.update('engineStats', {
                    active: data.active_downloads || 0,
                    queued: data.queue_size || 0,
                    totalSpeed: data.total_speed || 0,
                });
                break;

            case 'account_status':
                if (data.t) {
                    state.update('quota', {
                        remaining: data.t.remaining || 0,
                        total: data.t.total || 0,
                        percentage: data.t.total > 0
                            ? ((data.t.remaining / data.t.total) * 100)
                            : 0,
                    });
                }
                break;

            case 'download_progress':
                // Could update specific download in state
                break;

            case 'log':
                // Could append to log buffer
                break;
        }
    }

    /**
     * Schedule a reconnection attempt.
     */
    _scheduleReconnect() {
        if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            console.error('Max reconnection attempts reached');
            return;
        }

        this.reconnectAttempts++;
        const delay = this.reconnectDelay * Math.min(this.reconnectAttempts, 5);

        console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);

        setTimeout(() => {
            if (!this.connected) {
                this.connect(this.url);
            }
        }, delay);
    }

    /**
     * Register an event listener.
     * 
     * @param {string} type - Event type
     * @param {Function} callback - Event handler
     * @returns {Function} - Unsubscribe function
     */
    on(type, callback) {
        if (!this.listeners.has(type)) {
            this.listeners.set(type, new Set());
        }
        this.listeners.get(type).add(callback);

        return () => {
            const callbacks = this.listeners.get(type);
            if (callbacks) {
                callbacks.delete(callback);
            }
        };
    }

    /**
     * Emit an event to listeners.
     * 
     * @param {string} type - Event type
     * @param {any} data - Event data
     */
    _emit(type, data = null) {
        const callbacks = this.listeners.get(type);
        if (callbacks) {
            callbacks.forEach(callback => {
                try {
                    callback(data);
                } catch (e) {
                    console.error(`WebSocket listener error for ${type}:`, e);
                }
            });
        }
    }

    /**
     * Send a message to the server.
     * 
     * @param {string} type - Message type
     * @param {Object} data - Message data
     */
    send(type, data = {}) {
        if (!this.connected || !this.ws) {
            console.warn('WebSocket not connected, cannot send message');
            return false;
        }

        try {
            this.ws.send(JSON.stringify({ type, data }));
            return true;
        } catch (e) {
            console.error('Failed to send WebSocket message:', e);
            return false;
        }
    }

    /**
     * Close the WebSocket connection.
     */
    disconnect() {
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
        this.connected = false;
    }

    /**
     * Check if connected.
     * 
     * @returns {boolean}
     */
    isConnected() {
        return this.connected && this.ws && this.ws.readyState === WebSocket.OPEN;
    }
}

// Singleton instance
export const ws = new WebSocketClient();

// For debugging in console
if (typeof window !== 'undefined') {
    window.__ws = ws;
}
