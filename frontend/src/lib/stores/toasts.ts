import { writable } from 'svelte/store';

export type ToastType = 'success' | 'error' | 'info' | 'warning';

export interface Toast {
  id: string;
  type: ToastType;
  message: string;
  /** Auto-dismiss delay in ms; `null` keeps the toast until removed/updated. */
  duration: number | null;
  /** HH:MM:SS (24h) at creation time */
  timestamp: string;
}

/** Fields of an existing toast that can be patched with `toasts.update()`. */
export type ToastPatch = Partial<Omit<Toast, 'id'>>;

function createToastStore() {
  const { subscribe, update: updateStore } = writable<Toast[]>([]);
  const timers = new Map<string, ReturnType<typeof setTimeout>>();

  function scheduleRemoval(id: string, duration: number) {
    const existing = timers.get(id);
    if (existing) clearTimeout(existing);
    timers.set(
      id,
      setTimeout(() => remove(id), duration),
    );
  }

  function add(type: ToastType, message: string, duration: number | null = 5000): string {
    const id = Math.random().toString(36).substring(2, 11);
    const timestamp = new Date().toLocaleTimeString('en-US', {
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });

    const toast: Toast = { id, type, message, duration, timestamp };
    updateStore((toasts) => [...toasts, toast]);

    if (duration !== null) scheduleRemoval(id, duration);
    return id;
  }

  /**
   * Patch an existing toast (e.g. turn a sticky "Working..." toast into a
   * success). A new numeric `duration` restarts the auto-dismiss timer;
   * `duration: null` makes it sticky.
   */
  function update(id: string, patch: ToastPatch) {
    let nextDuration: number | null | undefined;
    updateStore((toasts) =>
      toasts.map((toast) => {
        if (toast.id !== id) return toast;
        const next = { ...toast, ...patch };
        nextDuration = next.duration;
        return next;
      }),
    );

    if (nextDuration === null) {
      const timer = timers.get(id);
      if (timer) clearTimeout(timer);
      timers.delete(id);
    } else if (nextDuration !== undefined) {
      scheduleRemoval(id, nextDuration);
    }
  }

  function remove(id: string) {
    const timer = timers.get(id);
    if (timer) clearTimeout(timer);
    timers.delete(id);
    updateStore((toasts) => toasts.filter((t) => t.id !== id));
  }

  function clear() {
    for (const timer of timers.values()) clearTimeout(timer);
    timers.clear();
    updateStore(() => []);
  }

  return {
    subscribe,
    success: (msg: string, duration?: number | null) => add('success', msg, duration),
    error: (msg: string, duration?: number | null) => add('error', msg, duration),
    info: (msg: string, duration?: number | null) => add('info', msg, duration),
    warning: (msg: string, duration?: number | null) => add('warning', msg, duration),
    update,
    remove,
    clear,
  };
}

export const toasts = createToastStore();
