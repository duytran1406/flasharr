import { writable } from 'svelte/store';

interface SmartGrabData {
  tmdbId: string;
  type: "movie" | "tv";
  title: string;
  year?: string | number;
  seasons: any[];
  tmdbSeasonEpisodeCounts?: Record<number, number>;
  /** Episode keys already downloaded (e.g. "S01E01", "S01E02") */
  existingDownloads?: Set<string>;
}

function createSmartGrabStore() {
  const { subscribe, set, update } = writable<{
    isOpen: boolean;
    data: SmartGrabData | null;
  }>({
    isOpen: false,
    data: null,
  });

  return {
    subscribe,
    open: (data: SmartGrabData) => {
      set({ isOpen: true, data });
    },
    close: () => {
      set({ isOpen: false, data: null });
    },
  };
}

export const smartGrabStore = createSmartGrabStore();
