import { writable } from 'svelte/store';

export interface SmartGrabData {
  tmdbId: string;
  /** Canonical TV identity (string from route/search data) when known. */
  tvdbId?: string | number | null;
  type: "movie" | "tv";
  title: string;
  year?: string | number;
  /** Seasons data from Smart Search results */
  seasons: any[];
  /** Restrict the plan to this season (Smart Search's current season). */
  targetSeason?: number;
  tmdbSeasonEpisodeCounts?: Record<number, number>;
  /** Episode keys already downloaded (e.g. "S01E01", "S01E02") */
  existingDownloads?: Set<string>;
}

function createSmartGrabStore() {
  const { subscribe, set } = writable<{
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
