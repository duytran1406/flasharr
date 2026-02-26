<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { animeFade, animeFly, animeSlide, stagger } from "$lib/animations";
  import { toasts } from "$lib/stores/toasts";
  import { downloadStore } from "$lib/stores/downloads";
  import { ui } from "$lib/stores/ui.svelte";
  import { SearchResultCard } from "$lib/components";
  import { queryClient } from "$lib/stores/query";
  import Button from "$lib/components/ui/Button.svelte";

  const API_BASE = "/api";

  // State
  let searchQuery = $state("");
  let isLoading = $state(false);
  let hasSearched = $state(false);
  let showTrending = $state(true);

  // Results State
  let allResults = $state<any[]>([]);
  let visibleResults = $state<any[]>([]);
  let trendingResults = $state<any[]>([]);

  // Infinite scroll config
  const BATCH_SIZE = 30;
  let visibleCount = $state(BATCH_SIZE);
  let totalResults = $state(0);
  let loadingMore = $state(false);
  let scrollObserver: IntersectionObserver | null = null;
  let sentinelEl: HTMLDivElement | undefined = $state();

  // View mode
  let viewMode = $state<"grid" | "list">("grid");

  function setupHeader() {
    const headerContainer = document.getElementById("header-dynamic-content");
    if (headerContainer) {
      headerContainer.innerHTML = `
        <div style="display: flex; align-items: center; gap: 1.5rem; width: 100%;">
          <div class="search-bar-header" style="flex: 1; position: relative;">
            <span class="material-icons" style="position: absolute; left: 1rem; top: 50%; transform: translateY(-50%); color: var(--text-muted); pointer-events: none;">search</span>
            <input type="text" id="spotlight-search" 
              placeholder="Detecting file signatures across Fshare..." 
              style="width: 100%; padding: 0.75rem 1rem 0.75rem 3rem; background: rgba(0,0,0,0.2); border: 1px solid rgba(255,255,255,0.1); border-radius: 12px; color: #fff; outline: none; transition: all 0.3s;"
              autocomplete="off">
          </div>
        </div>
      `;

      const searchInput = document.getElementById(
        "spotlight-search",
      ) as HTMLInputElement;

      if (searchInput) {
        searchInput.addEventListener("keydown", (e) => {
          if ((e as KeyboardEvent).key === "Enter") {
            const val = searchInput.value.trim();
            if (val) {
              searchQuery = val;
              handleSearch(val);
            }
          }
        });

        // Sync initial text if from URL
        if (searchQuery) searchInput.value = searchQuery;
        searchInput.focus();
      }
    }
  }

  onMount(() => {
    const q = new URLSearchParams(window.location.search).get("q");
    setupHeader();

    if (q) {
      searchQuery = q;
      handleSearch(q);
    } else {
      fetchTrending();
    }
  });

  // Fetch Trending (Default View)
  async function fetchTrending() {
    isLoading = true;
    try {
      const data = await queryClient.fetch("trending", async () => {
        const res = await fetch(`${API_BASE}/discovery/trending`);
        if (!res.ok) throw new Error("Failed to fetch trending");
        return res.json();
      });

      trendingResults = mapResults(data.results || []);
      showTrending = true;
      hasSearched = false;
    } catch (err) {
      console.error("Trending error:", err);
    } finally {
      isLoading = false;
    }
  }

  // Enhanced Search (Infinite Scroll)
  async function handleSearch(query: string) {
    if (!query) return;

    // Reset state
    visibleCount = BATCH_SIZE;
    hasSearched = true;
    showTrending = false;
    isLoading = true;

    try {
      const data = await queryClient.fetch(`search:${query}`, async () => {
        const res = await fetch(
          `${API_BASE}/search/enhanced?q=${encodeURIComponent(query)}&enrich=true`,
        );
        if (!res.ok) throw new Error("Search failed");
        return res.json();
      });

      allResults = mapResults(data.results || []);
      totalResults = allResults.length;
      visibleResults = allResults.slice(0, visibleCount);
      setupScrollObserver();
    } catch (err) {
      console.error("Search error:", err);
      toasts.error("Neural link failure: Search connection lost");
      allResults = [];
      visibleResults = [];
    } finally {
      isLoading = false;
    }
  }

  function loadMore() {
    if (loadingMore || visibleCount >= allResults.length) return;
    loadingMore = true;
    // Small delay to show loading indicator
    setTimeout(() => {
      visibleCount = Math.min(visibleCount + BATCH_SIZE, allResults.length);
      visibleResults = allResults.slice(0, visibleCount);
      loadingMore = false;
    }, 150);
  }

  function setupScrollObserver() {
    // Clean up previous observer
    if (scrollObserver) scrollObserver.disconnect();

    // Wait for DOM to render the sentinel
    requestAnimationFrame(() => {
      if (!sentinelEl) return;
      scrollObserver = new IntersectionObserver(
        (entries) => {
          if (entries[0]?.isIntersecting) loadMore();
        },
        { rootMargin: "200px" },
      );
      scrollObserver.observe(sentinelEl);
    });
  }

  // Re-observe when sentinel element changes
  $effect(() => {
    if (sentinelEl && hasSearched && allResults.length > 0) {
      setupScrollObserver();
    }
  });

  onDestroy(() => {
    if (scrollObserver) scrollObserver.disconnect();
  });

  function toggleViewMode(mode: "grid" | "list") {
    if (viewMode === mode) return;
    viewMode = mode;
  }

  // Mapper to normalize Backend V3 response for UI
  function mapResults(rawItems: any[]) {
    return rawItems.map((item: any) => {
      // Fallback title logic
      const displayTitle = item.tmdb_title || item.parsed_name || item.name;

      // Extract quality info if not separated
      let res = item.resolution;
      let src = item.source;
      if (!res && item.quality) {
        if (item.quality.includes("2160") || item.quality.includes("4K"))
          res = "4K";
        else if (item.quality.includes("1080")) res = "1080p";
        else if (item.quality.includes("720")) res = "720p";
      }

      return {
        id: item.tmdb_id,
        title: displayTitle,
        posterPath: item.poster_path, // Prefer path for TMDB image construction
        posterUrl: item.poster_url, // Fallback full URL
        voteAverage: item.vote_average || 0,
        releaseDate:
          item.release_date || item.first_air_date || item.year || "",
        mediaType: item.media_type || "movie",
        fcode: item.id, // Fshare ID is "id" in V3 spec, mapped to fcode for UI
        originalFilename: item.name,
        fileSize: item.size || 0,
        score: 0, // V3 API matching logic handles scoring internally

        // Rich Metadata
        quality: item.quality,
        resolution: res,
        source: src,
        episodeTag: item.episode_tag,
        hasVietsub: item.vietsub || false,
        hasVietdub: item.vietdub || false,
      };
    });
  }

  // Formatters
  function formatSize(bytes: number) {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  function getQualityColor(res?: string) {
    if (!res) return "var(--text-muted)";
    if (res === "4K" || res === "2160p") return "#ffd700"; // Gold
    if (res === "1080p") return "#00ffa3"; // Green
    if (res === "720p") return "#00f3ff"; // Cyan
    return "var(--text-muted)";
  }

  // Actions
  async function handleDownload(item: any) {
    // Build TMDB metadata if available
    const tmdb = item.id
      ? {
          tmdb_id: item.id,
          media_type: item.mediaType,
          title: item.title,
          year: item.releaseDate ? item.releaseDate.substring(0, 4) : undefined,
        }
      : undefined;

    const result = await downloadStore.addDownload({
      url: `https://fshare.vn/file/${item.fcode}`,
      tmdb,
    });
    if (result.success) toasts.success("Initiating data retrieval");
    else
      toasts.error(
        result.error || "Neural link failure: Download initiation failed",
      );
  }

  function handleCopyUrl(item: any) {
    navigator.clipboard.writeText(`https://fshare.vn/file/${item.fcode}`);
    toasts.success("Binary link copied to clipboard");
  }

  function openSmartSearch(item: any) {
    ui.openSmartSearch({
      tmdbId: String(item.id),
      type: item.mediaType,
      title: item.title,
      year: item.releaseDate?.substring(0, 4),
    });
  }
</script>

<svelte:head>
  <title>Search - Flasharr</title>
</svelte:head>

<div class="search-viewport">
  <!-- Loading State -->
  {#if isLoading}
    <div class="loading-container" in:animeFade>
      <div class="loading-spinner"></div>
      <p>Synchronizing with Fshare API...</p>
    </div>

    <!-- Trending / Hero State -->
  {:else if showTrending && !hasSearched}
    <div class="trending-section" in:animeFade={{ duration: 400 }}>
      <div class="hero-header">
        <div class="icon-ring-small">
          <span class="material-icons">local_fire_department</span>
        </div>
        <div>
          <h2>Trending on Fshare</h2>
          <p class="subtitle">
            Popular files discovered across the network today
          </p>
        </div>
      </div>

      <div class="search-results-grid">
        {#each trendingResults as item (item.fcode)}
          <div in:animeSlide={{ y: 20, duration: 400 }}>
            <SearchResultCard
              {...item}
              onDownload={() => handleDownload(item)}
            />
          </div>
        {/each}
      </div>
    </div>

    <!-- Search Results State -->
  {:else if hasSearched}
    <div class="search-header-tools">
      <div class="results-meta">
        <span class="count">{totalResults}</span>
        <span class="label">ASSETS DETECTED</span>
      </div>

      <div class="view-toggle glass-panel">
        <button
          class="toggle-btn"
          class:active={viewMode === "grid"}
          onclick={() => toggleViewMode("grid")}
          title="Tactical Grid"
        >
          <span class="material-icons">grid_view</span>
        </button>
        <button
          class="toggle-btn"
          class:active={viewMode === "list"}
          onclick={() => toggleViewMode("list")}
          title="Data List"
        >
          <span class="material-icons">view_list</span>
        </button>
      </div>
    </div>

    <!-- Results Grid/List -->
    <div class="results-container">
      {#if visibleResults.length === 0}
        <div class="empty-state">
          <span class="material-icons">search_off</span>
          <h3>NO MATCHES FOUND</h3>
          <p>Broaden your search parameters or check title spelling.</p>
        </div>
      {:else if viewMode === "grid"}
        <div class="search-results-grid">
          {#each visibleResults as item (item.fcode)}
            <div>
              <SearchResultCard
                {...item}
                onDownload={() => handleDownload(item)}
              />
            </div>
          {/each}
        </div>
      {:else}
        <div class="search-results-list">
          {#each visibleResults as item (item.fcode)}
            <div class="result-list-item glass-panel">
              <div class="item-visual">
                {#if item.posterPath}
                  <img
                    src="https://image.tmdb.org/t/p/w92{item.posterPath}"
                    alt=""
                    loading="lazy"
                  />
                {:else if item.posterUrl}
                  <img src={item.posterUrl} alt="" loading="lazy" />
                {:else}
                  <div class="placeholder">
                    <span class="material-icons">movie</span>
                  </div>
                {/if}
              </div>

              <div class="item-main">
                <div class="item-title-row">
                  <h3 class="title">{item.title}</h3>
                  <span class="year">{item.releaseDate?.substring(0, 4)}</span>
                </div>
                <div class="filename" title={item.originalFilename}>
                  {item.originalFilename}
                </div>

                <div class="item-meta">
                  {#if item.resolution}
                    <span
                      class="badge res"
                      style="color: {getQualityColor(item.resolution)}"
                      >{item.resolution}</span
                    >
                  {/if}
                  {#if item.source}
                    <span class="badge src">{item.source}</span>
                  {/if}
                  {#if item.episodeTag}
                    <span class="badge episode">{item.episodeTag}</span>
                  {/if}
                  <span class="size">{formatSize(item.fileSize)}</span>
                  {#if item.hasVietsub}
                    <span class="badge sub">VIETSUB</span>
                  {/if}
                  {#if item.hasVietdub}
                    <span class="badge dub">VIETDUB</span>
                  {/if}
                </div>
              </div>

              <div class="item-actions">
                {#if item.id}
                  <Button
                    variant="ghost"
                    size="sm"
                    icon="psychology"
                    onclick={() => openSmartSearch(item)}
                    title="Smart Search"
                  ></Button>
                {/if}
                <Button
                  variant="ghost"
                  size="sm"
                  icon="link"
                  onclick={() => handleCopyUrl(item)}
                  title="Copy Link"
                ></Button>
                <Button
                  size="sm"
                  icon="download"
                  onclick={() => handleDownload(item)}>Get</Button
                >
              </div>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Infinite scroll sentinel -->
      {#if visibleCount < allResults.length}
        <div class="scroll-sentinel" bind:this={sentinelEl}>
          {#if loadingMore}
            <div class="loading-more">
              <div class="loading-spinner small"></div>
              <span>Loading more results...</span>
            </div>
          {/if}
        </div>
      {:else if visibleResults.length > 0}
        <div class="end-of-results">
          <span class="material-icons">check_circle</span>
          <span>All {totalResults} results loaded</span>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .search-viewport {
    padding: 2.5rem 2rem;
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .loading-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 50vh;
    color: var(--text-muted);
  }

  /* Trending Section */
  .trending-section {
    max-width: 1600px;
    margin: 0 auto;
  }
  .hero-header {
    display: flex;
    align-items: center;
    gap: 1.5rem;
    margin-bottom: 2.5rem;
    padding-bottom: 2rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }
  .icon-ring-small {
    width: 60px;
    height: 60px;
    border-radius: 50%;
    background: rgba(255, 107, 107, 0.1); /* Red-ish for trending */
    border: 1px solid rgba(255, 107, 107, 0.2);
    display: flex;
    align-items: center;
    justify-content: center;
    color: #ff6b6b;
  }
  .subtitle {
    color: var(--text-muted);
    margin-top: 0.25rem;
  }

  /* Shared header tools */
  .search-header-tools {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 2rem;
  }

  .results-meta .count {
    font-family: var(--font-mono);
    font-size: 1.25rem;
    font-weight: 800;
    color: var(--color-primary);
    margin-right: 0.5rem;
  }
  .results-meta .label {
    font-size: 0.65rem;
    font-weight: 900;
    letter-spacing: 0.1em;
    color: var(--text-muted);
  }

  /* Toggles */
  .view-toggle {
    display: flex;
    padding: 0.25rem;
    gap: 0.25rem;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.2);
  }
  .toggle-btn {
    width: 36px;
    height: 36px;
    border-radius: 8px;
    border: none;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s;
  }
  .toggle-btn.active {
    background: var(--color-primary);
    color: #000;
  }

  /* Grid Layout — 10 per row */
  .search-results-grid {
    display: grid;
    grid-template-columns: repeat(10, 1fr);
    gap: 0.75rem;
  }
  @media (max-width: 1600px) {
    .search-results-grid {
      grid-template-columns: repeat(8, 1fr);
    }
  }
  @media (max-width: 1200px) {
    .search-results-grid {
      grid-template-columns: repeat(6, 1fr);
    }
  }
  @media (max-width: 800px) {
    .search-results-grid {
      grid-template-columns: repeat(3, 1fr);
    }
  }

  /* List Layout */
  .search-results-list {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .result-list-item {
    display: flex;
    align-items: center;
    gap: 1.5rem;
    padding: 1rem;
    border-radius: 16px;
    border: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(255, 255, 255, 0.02);
    transition:
      transform 0.2s,
      border-color 0.2s;
  }
  .result-list-item:hover {
    border-color: var(--color-primary);
    background: rgba(255, 255, 255, 0.04);
    transform: translateX(5px);
  }

  .item-visual {
    width: 60px;
    height: 90px;
    border-radius: 8px;
    overflow: hidden;
    flex-shrink: 0;
    background: #000;
  }
  .item-visual img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .item-visual .placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
  }

  .item-main {
    flex: 1;
    min-width: 0;
  }

  .item-title-row {
    display: flex;
    align-items: baseline;
    gap: 0.75rem;
    margin-bottom: 0.25rem;
  }
  .item-title-row .title {
    font-size: 1rem;
    font-weight: 700;
    color: #fff;
  }
  .item-title-row .year {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    color: var(--color-primary);
  }

  .filename {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-bottom: 0.5rem;
    opacity: 0.7;
  }

  .item-meta {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  /* Badges */
  .badge {
    font-size: 0.65rem;
    font-weight: 800;
    padding: 0.15rem 0.4rem;
    border-radius: 4px;
    letter-spacing: 0.05em;
  }
  .badge.res {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid currentColor;
  }
  .badge.src {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }
  .badge.episode {
    background: rgba(138, 43, 226, 0.2);
    color: #c084fc;
    border: 1px solid rgba(138, 43, 226, 0.4);
  }
  .badge.sub {
    background: rgba(255, 107, 107, 0.2);
    color: #ff6b6b;
  }
  .badge.dub {
    background: rgba(255, 165, 0, 0.2);
    color: #ffa500;
  }
  .size {
    font-family: var(--font-mono);
    font-size: 0.7rem;
    color: var(--text-muted);
    margin-left: auto; /* Push size to the right if needed, or remove */
  }

  /* Actions */
  .item-actions {
    display: flex;
    gap: 0.5rem;
    flex-shrink: 0;
  }
  .action-btn-icon {
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: var(--text-muted);
    cursor: pointer;
    transition: all 0.2s;
  }
  .action-btn-icon:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
    border-color: rgba(255, 255, 255, 0.3);
  }
  .dl-btn-premium {
    height: 36px;
    padding: 0 1.25rem;
    background: var(--color-primary);
    color: #000;
    font-weight: 800;
    border-radius: 8px;
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.75rem;
    transition: all 0.2s;
  }
  .dl-btn-premium:hover {
    box-shadow: 0 0 15px rgba(0, 243, 255, 0.4);
    transform: translateY(-2px);
  }

  /* Infinite scroll sentinel & indicators */
  .scroll-sentinel {
    height: 60px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .loading-more {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    color: var(--text-muted);
    font-size: 0.8rem;
  }
  .loading-spinner.small {
    width: 20px;
    height: 20px;
    border-width: 2px;
  }
  .end-of-results {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding: 1.5rem 0 2rem;
    color: var(--text-muted);
    font-size: 0.75rem;
    opacity: 0.6;
  }
  .end-of-results .material-icons {
    font-size: 16px;
    color: #00ff80;
  }

  .empty-state {
    text-align: center;
    padding: 4rem 0;
    color: var(--text-muted);
  }
  .empty-state .material-icons {
    font-size: 64px;
    opacity: 0.3;
    margin-bottom: 1rem;
  }
</style>
