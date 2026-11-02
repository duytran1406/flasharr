import { writable } from "svelte/store";

export type ToastType = "success" | "error" | "info" | "warning";

export interface Toast {
  id: string;
  type: ToastType;
  message: string;
  /** Auto-dismiss delay in ms */
  duration: number;
  /** HH:MM:SS (24h) at creation time */
  timestamp: string;
}

export function createToastStore() {
  const { subscribe, update } = writable<Toast[]>([]);

  function add(type: ToastType, message: string, duration = 5000): string {
    const id = Math.random().toString(36).substring(2, 11);
    const timestamp = new Date().toLocaleTimeString("en-US", {
      hour12: false,
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
    const toast: Toast = { id, type, message, duration, timestamp };

    update((all) => [...all, toast]);
    setTimeout(() => {
      remove(id);
    }, duration);

    return id;
  }

  function remove(id: string) {
    update((all) => all.filter((t) => t.id !== id));
  }

  function clear() {
    update(() => []);
  }

  return {
    subscribe,
    success: (message: string, duration?: number) => add("success", message, duration),
    error: (message: string, duration?: number) => add("error", message, duration),
    info: (message: string, duration?: number) => add("info", message, duration),
    warning: (message: string, duration?: number) => add("warning", message, duration),
    remove,
    clear,
  };
}

/** Shared app-wide toast queue. */
export const toasts = createToastStore();
