import { writable } from 'svelte/store';

export type ToastType = 'success' | 'error' | 'info' | 'warning';

export interface Toast {
  id: string;
  type: ToastType;
  message: string;
  duration: number;
  timestamp: string;
}

function createToastStore() {
  const { subscribe, update } = writable<Toast[]>([]);

  function add(type: ToastType, message: string, duration = 5000) {
    const id = Math.random().toString(36).substring(2, 11);
    const timestamp = new Date().toLocaleTimeString('en-US', { 
      hour12: false, 
      hour: '2-digit', 
      minute: '2-digit', 
      second: '2-digit' 
    });
    
    const toast: Toast = { id, type, message, duration, timestamp };

    update((toasts) => [...toasts, toast]);

    // Auto-dismiss after duration
    setTimeout(() => {
      remove(id);
    }, duration);

    return id;
  }

  function remove(id: string) {
    update((toasts) => toasts.filter((t) => t.id !== id));
  }

  function clear() {
    update(() => []);
  }

  return {
    subscribe,
    success: (msg: string, duration?: number) => add('success', msg, duration),
    error: (msg: string, duration?: number) => add('error', msg, duration),
    info: (msg: string, duration?: number) => add('info', msg, duration),
    warning: (msg: string, duration?: number) => add('warning', msg, duration),
    remove,
    clear,
  };
}

export const toasts = createToastStore();
