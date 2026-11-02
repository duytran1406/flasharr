/** What the Smart Search modal is searching for. IDs are strings (route params). */
export interface SmartSearchData {
  tmdbId: string;
  /** Canonical TV identity when known. */
  tvdbId?: string;
  type: 'movie' | 'tv';
  title: string;
  year?: string | number;
  season?: number;
  episode?: number;
}

/**
 * `season`: browse a whole season (episode is dropped).
 * `target`: a single movie or episode.
 */
export type SmartSearchMode = 'season' | 'target';

/** Where the search was opened from (e.g. the collection page). */
export interface SmartSearchContext {
  source: string;
  tmdbId?: number;
  [key: string]: unknown;
}

/** Payload for SmartSearchOptions.onDownloadResult. */
export interface SmartSearchDownloadResult {
  tmdbId: number;
  success: boolean;
}

export interface SmartSearchOptions {
  /** Defaults to `target` for movies/episodes, `season` otherwise. */
  mode?: SmartSearchMode;
  context?: SmartSearchContext | null;
  /**
   * Download outcome callback. Stored on the store; the 2026-09-02 build sets
   * it (collection page) but no deployed code invokes it.
   */
  onDownloadResult?: ((result: SmartSearchDownloadResult) => void) | null;
}

export interface SmartGrabData {
  tmdbId: string;
  type: 'movie' | 'tv';
  title: string;
  year?: string | number;
  seasons: any[]; // Seasons data from Smart Search results
  /** TMDB episode counts per season for uncut detection, e.g. { 1: 35, 2: 41 } */
  tmdbSeasonEpisodeCounts?: Record<number, number>;
}

class UIState {
  addDownloadModalOpen = $state(false);
  smartSearchModalOpen = $state(false);
  smartSearchData = $state<SmartSearchData | null>(null);
  smartSearchContext = $state<SmartSearchContext | null>(null);
  smartSearchMode = $state<SmartSearchMode>('season');
  onSmartSearchDownloadResult = $state<((result: SmartSearchDownloadResult) => void) | null>(null);

  // Global intro animation states
  showIntro = $state(true);
  showTagline = $state(false);
  _introFinished = false;

  startIntroSequence() {
    // Don't restart the intro if it's already finished
    if (this._introFinished) return;

    this.showIntro = true;
    this.showTagline = false;
    setTimeout(() => {
      this.showTagline = true;
    }, 600);
  }

  finishIntro() {
    this._introFinished = true;
    this.showIntro = false;
  }

  openAddDownload() {
    this.addDownloadModalOpen = true;
  }

  closeAddDownload() {
    this.addDownloadModalOpen = false;
  }

  openSmartSearch(data: SmartSearchData, options?: SmartSearchOptions) {
    const mode: SmartSearchMode =
      options?.mode ?? (data.type === 'movie' || data.episode ? 'target' : 'season');
    this.smartSearchData = mode === 'season' ? { ...data, episode: undefined } : data;
    this.smartSearchContext = options?.context ?? null;
    this.smartSearchMode = mode;
    this.onSmartSearchDownloadResult = options?.onDownloadResult ?? null;
    this.smartSearchModalOpen = true;
  }

  closeSmartSearch() {
    this.smartSearchModalOpen = false;
    this.smartSearchData = null;
    this.smartSearchContext = null;
    this.smartSearchMode = 'season';
    this.onSmartSearchDownloadResult = null;
  }

  // Smart Grab Modal (separate from Smart Search)
  smartGrabModalOpen = $state(false);
  smartGrabData = $state<SmartGrabData | null>(null);

  openSmartGrab(data: SmartGrabData) {
    this.smartGrabData = data;
    this.smartGrabModalOpen = true;
  }

  closeSmartGrab() {
    this.smartGrabModalOpen = false;
    this.smartGrabData = null;
  }
}

export const ui = new UIState();
