class UIState {
  addDownloadModalOpen = $state(false);
  smartSearchModalOpen = $state(false);
  smartSearchData = $state<{
    tmdbId: string;
    type: "movie" | "tv";
    title: string;
    year?: string | number;
    season?: number;
    episode?: number;
  } | null>(null);
  
  // Global intro animation states
  showIntro = $state(true);
  showTagline = $state(false);
  private _introFinished = false;

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

  openSmartSearch(data: {
    tmdbId: string;
    type: "movie" | "tv";
    title: string;
    year?: string | number;
    season?: number;
    episode?: number;
  }) {
    this.smartSearchData = data;
    this.smartSearchModalOpen = true;
  }

  closeSmartSearch() {
    this.smartSearchModalOpen = false;
    this.smartSearchData = null;
  }

  // Smart Grab Modal (separate from Smart Search)
  smartGrabModalOpen = $state(false);
  smartGrabData = $state<{
    tmdbId: string;
    type: "movie" | "tv";
    title: string;
    year?: string | number;
    seasons: any[]; // Seasons data from Smart Search results
    // TMDB episode counts per season for uncut detection
    // { 1: 35, 2: 41 } means Season 1 has 35 eps, Season 2 has 41 eps
    tmdbSeasonEpisodeCounts?: Record<number, number>;
  } | null>(null);

  openSmartGrab(data: {
    tmdbId: string;
    type: "movie" | "tv";
    title: string;
    year?: string | number;
    seasons: any[];
    tmdbSeasonEpisodeCounts?: Record<number, number>;
  }) {
    this.smartGrabData = data;
    this.smartGrabModalOpen = true;
  }

  closeSmartGrab() {
    this.smartGrabModalOpen = false;
    this.smartGrabData = null;
  }
}

export const ui = new UIState();
