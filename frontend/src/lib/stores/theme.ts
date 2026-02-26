import { writable } from 'svelte/store';
import { browser } from '$app/environment';

export type Theme = 'dark';

function createThemeStore() {
  const { subscribe, set } = writable<Theme>('dark');

  return {
    subscribe,
    set: (_value: Theme) => {
      if (browser) {
        document.documentElement.setAttribute('data-theme', 'dark');
      }
      set('dark');
    },
    toggle: () => {
      // Dark mode only — no-op
    },
    init: () => {
      if (browser) {
        document.documentElement.setAttribute('data-theme', 'dark');
        set('dark');
      }
    }
  };
}

export const theme = createThemeStore();
