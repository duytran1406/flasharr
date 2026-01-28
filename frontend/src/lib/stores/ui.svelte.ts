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

  startIntroSequence() {
    this.showIntro = true;
    this.showTagline = false;
    setTimeout(() => {
      this.showTagline = true;
    }, 600);
  }

  finishIntro() {
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
}

export const ui = new UIState();
