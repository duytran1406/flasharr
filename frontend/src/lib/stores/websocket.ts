import { writable, derived, type Writable } from 'svelte/store';

/**
 * WebSocket Connection Status
 */
export type WSStatus = 'disconnected' | 'connecting' | 'connected' | 'error';

/**
 * WebSocket Message Types (from contract-fe-bridge.md)
 */
export type WSMessageType = 
  | 'SYNC_ALL'
  | 'TASK_ADDED'
  | 'TASK_UPDATED'
  | 'TASK_BATCH_UPDATE'
  | 'TASK_REMOVED'
  | 'ENGINE_STATS';

/**
 * Base WebSocket Message
 */
export interface WSMessage {
  type: WSMessageType;
  [key: string]: any;
}

/**Set
 * Message Handler Function
 */
type MessageHandler = (data: any) => void;

/**
 * WebSocket Client Configuration
 */
interface WSConfig {
  url: string;
  reconnectDelay: number;
  maxReconnectAttempts: number;
  debug: boolean;
}

/**
 * WebSocket Client Class
 * Handles connection, reconnection, and message routing
 */
class WebSocketClient {
  private ws: WebSocket | null = null;
  private config: WSConfig;
  private handlers: Map<WSMessageType, MessageHandler[]> = new Map();
  private reconnectTimer: number | null = null;
  private reconnectAttempts = 0;
  private intentionalDisconnect = false;

  // Svelte stores
  public status: Writable<WSStatus>;
  public lastMessage: Writable<WSMessage | null>;
  public error: Writable<string | null>;

  constructor(config: Partial<WSConfig> = {}) {
    // Use window.location.host (includes port) for proper Docker/production support
    const defaultUrl = (typeof window !== 'undefined') 
      ? `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}/api/ws`
      : 'ws://localhost:8484/api/ws';

    this.config = {
      url: config.url || defaultUrl,
      reconnectDelay: config.reconnectDelay || 5000,
      maxReconnectAttempts: config.maxReconnectAttempts || 10,
      debug: config.debug ?? true,
    };

    this.status = writable<WSStatus>('disconnected');
    this.lastMessage = writable<WSMessage | null>(null);
    this.error = writable<string | null>(null);
  }

  /**
   * Connect to WebSocket server
   */
  connect(): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.log('Already connected');
      return;
    }

    this.intentionalDisconnect = false;
    this.status.set('connecting');
    this.error.set(null);

    try {
      this.log(`Connecting to ${this.config.url}...`);
      this.ws = new WebSocket(this.config.url);

      this.ws.onopen = this.handleOpen.bind(this);
      this.ws.onmessage = this.handleMessage.bind(this);
      this.ws.onerror = this.handleError.bind(this);
      this.ws.onclose = this.handleClose.bind(this);
    } catch (err) {
      this.log('Connection error:', err);
      this.status.set('error');
      this.error.set(err instanceof Error ? err.message : 'Connection failed');
      this.scheduleReconnect();
    }
  }

  /**
   * Disconnect from WebSocket server
   */
  disconnect(): void {
    this.log('Disconnecting...');
    this.intentionalDisconnect = true;
    this.clearReconnectTimer();
    
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    
    this.status.set('disconnected');
  }

  /**
   * Register a message handler for a specific message type
   */
  on(type: WSMessageType, handler: MessageHandler): void {
    if (!this.handlers.has(type)) {
      this.handlers.set(type, []);
    }
    this.handlers.get(type)!.push(handler);
    this.log(`Registered handler for ${type}`);
  }

  /**
   * Unregister a message handler
   */
  off(type: WSMessageType, handler: MessageHandler): void {
    const handlers = this.handlers.get(type);
    if (handlers) {
      const index = handlers.indexOf(handler);
      if (index > -1) {
        handlers.splice(index, 1);
        this.log(`Unregistered handler for ${type}`);
      }
    }
  }

  /**
   * Send a message to the server (future use)
   */
  send(message: any): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
      this.log('Sent message:', message);
    } else {
      this.log('Cannot send message: not connected');
    }
  }

  /**
   * Handle WebSocket open event
   */
  private handleOpen(): void {
    this.log('✅ Connected to WebSocket');
    this.status.set('connected');
    this.error.set(null);
    this.reconnectAttempts = 0;
  }

  /**
   * Handle incoming WebSocket messages
   */
  private handleMessage(event: MessageEvent): void {
    try {
      const message: WSMessage = JSON.parse(event.data);
      this.log('📨 Received:', message.type, message);

      // Update last message store
      this.lastMessage.set(message);

      // Route to registered handlers
      const handlers = this.handlers.get(message.type);
      if (handlers && handlers.length > 0) {
        handlers.forEach(handler => {
          try {
            handler(message);
          } catch (err) {
            this.log(`Error in handler for ${message.type}:`, err);
          }
        });
      } else {
        this.log(`No handlers registered for ${message.type}`);
      }
    } catch (err) {
      this.log('Failed to parse message:', err);
      this.error.set('Invalid message received');
    }
  }

  /**
   * Handle WebSocket error event
   */
  private handleError(event: Event): void {
    this.log('❌ WebSocket error:', event);
    this.status.set('error');
    this.error.set('WebSocket connection error');
  }

  /**
   * Handle WebSocket close event
   */
  private handleClose(event: CloseEvent): void {
    this.log(`🔌 Disconnected (code: ${event.code}, reason: ${event.reason})`);
    this.status.set('disconnected');
    this.ws = null;

    // Reconnect if not intentional disconnect
    if (!this.intentionalDisconnect) {
      this.scheduleReconnect();
    }
  }

  /**
   * Schedule reconnection attempt
   */
  private scheduleReconnect(): void {
    if (this.reconnectAttempts >= this.config.maxReconnectAttempts) {
      this.log(`Max reconnect attempts (${this.config.maxReconnectAttempts}) reached`);
      this.error.set('Failed to reconnect after multiple attempts');
      return;
    }

    this.clearReconnectTimer();
    
    const delay = this.config.reconnectDelay * Math.pow(1.5, this.reconnectAttempts);
    this.reconnectAttempts++;
    
    this.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})...`);
    
    this.reconnectTimer = window.setTimeout(() => {
      this.connect();
    }, delay);
  }

  /**
   * Clear reconnection timer
   */
  private clearReconnectTimer(): void {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  /**
   * Log debug messages
   */
  private log(...args: any[]): void {
    if (this.config.debug) {
      console.log('[WebSocket]', ...args);
    }
  }

  /**
   * Get current connection status
   */
  get isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }
}

/**
 * Global WebSocket client instance
 */
export const wsClient = new WebSocketClient();

/**
 * Derived store for connection status
 */
export const isConnected = derived(
  wsClient.status,
  $status => $status === 'connected'
);

/**
 * Derived store for connection indicator
 */
export const connectionIndicator = derived(
  wsClient.status,
  $status => {
    switch ($status) {
      case 'connected':
        return { text: 'Connected', color: 'green', icon: '🟢' };
      case 'connecting':
        return { text: 'Connecting...', color: 'yellow', icon: '🟡' };
      case 'disconnected':
        return { text: 'Disconnected', color: 'gray', icon: '⚪' };
      case 'error':
        return { text: 'Error', color: 'red', icon: '🔴' };
      default:
        return { text: 'Unknown', color: 'gray', icon: '⚪' };
    }
  }
);
