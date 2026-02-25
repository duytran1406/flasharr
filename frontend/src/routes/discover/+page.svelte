<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { animeFly } from "$lib/animations";
  import type { TMDBMovie, TMDBTVShow } from "$lib/types/tmdb";
  import { MediaCard, ErrorState } from "$lib/components";

  // ============= CONFIGURATION =============
  const ITEMS_PER_PAGE = 20; // TMDB returns 20 per page
  const POOL_SIZE = 1000; // Pre-create 1000 slots for extensive scrolling
  const ITEMS_PER_ROW = 5; // 5 items per row

  // ============= POOL SLOT TYPE =============
  interface PoolSlot {
    slotId: number; // Fixed ID for keying
    visible: boolean; // Is this slot showing data?
    data: TMDBMovie | TMDBTVShow | null;
  }

  // ============= STATE =============
  let mediaType: "movie" | "tv" = "movie";
  let sortBy = "popularity.desc";
  let searchQuery = "";
  let showFilters = false;
  let fromYear = "";
  let toYear = "";
  let selectedGenres: number[] = [];
  let minRating = 0;
  let maxRating = 10;
  let genres: { id: number; name: string }[] = [];
  let loading = false;
  let error: string | null = null;
  let currentPage = 1;
  let hasMore = true;
  let scrollContainer: HTMLElement;

  // ============= OBJECT POOL =============
  // Create fixed pool of 1000 slots - NEVER changes length
  const pool: PoolSlot[] = Array.from({ length: POOL_SIZE }, (_, i) => ({
    slotId: i,
    visible: false,
    data: null,
  }));

  // Track the next available slot index
  let nextPoolIndex = 0;

  // Request management
  let activeAbortController: AbortController | null = null;
  let lastRequestId = 0;

  const sortOptions = [
    { value: "popular_today", label: "Popular Today" },
    { value: "popularity.desc", label: "Popularity Descending" },
    { value: "popularity.asc", label: "Popularity Ascending" },
    { value: "release_date.desc", label: "Release Date Descending" },
    { value: "release_date.asc", label: "Release Date Ascending" },
    { value: "vote_average.desc", label: "TMDB Rating Descending" },
    { value: "vote_average.asc", label: "TMDB Rating Ascending" },
    { value: "title.asc", label: "Title A-Z" },
    { value: "title.desc", label: "Title Z-A" },
  ];

  // ============= POOL MANAGEMENT =============
  // Add new items to the pool - simple append, no rebuilding
  function addItemsToPool(items: (TMDBMovie | TMDBTVShow)[]) {
    for (const item of items) {
      if (nextPoolIndex < POOL_SIZE) {
        // Update slot in place - no array length change!
        pool[nextPoolIndex].data = item;
        pool[nextPoolIndex].visible = true;
        nextPoolIndex++;
      } else {
        console.warn("[Discover] Pool is full, cannot add more items");
        break;
      }
    }
    console.log(`[Discover] Pool updated: ${nextPoolIndex} visible slots`);
  }

  // Clear pool and release all data references
  function clearPool() {
    for (let i = 0; i < POOL_SIZE; i++) {
      pool[i].visible = false;
      pool[i].data = null; // Release data reference for GC
    }
    nextPoolIndex = 0; // Reset the index
    console.log("[Discover] Pool cleared");
  }

  // ============= FETCH LOGIC =============
  async function fetchGenres() {
    try {
      const endpoint =
        mediaType === "movie"
          ? "/api/tmdb/genres/movie"
          : "/api/tmdb/genres/tv";
      const res = await fetch(endpoint);
      const data = await res.json();
      genres = data.genres || [];
    } catch (err) {
      console.error("Failed to fetch genres:", err);
    }
  }

  async function fetchDiscoverData(reset = false) {
    if (!reset && (loading || !hasMore)) {
      console.log(
        `[Discover] Skipping fetch - loading: ${loading}, hasMore: ${hasMore}`,
      );
      return;
    }

    // Cancel any in-flight request
    if (activeAbortController) {
      console.log("[Discover] Aborting previous request");
      activeAbortController.abort();
    }

    activeAbortController = new AbortController();
    const requestId = ++lastRequestId;
    const timeoutId = setTimeout(() => {
      console.error("[Discover] Request timeout - aborting");
      activeAbortController?.abort();
    }, 10000);

    const requestType = mediaType;
    const requestQuery = searchQuery;
    const requestSort = sortBy;
    const requestPage = reset ? 1 : currentPage;

    console.log(
      `[Discover] Starting fetch #${requestId} - Page ${requestPage}, Type: ${requestType}`,
    );
    loading = true;

    if (reset) {
      currentPage = 1;
      clearPool(); // This resets nextPoolIndex and clears all data
      hasMore = true;
    }

    try {
      let url = "";
      if (requestSort === "popular_today") {
        url = `/api/discovery/popular-today?type=${requestType}&page=${requestPage}&limit=${ITEMS_PER_PAGE}`;
      } else if (requestQuery.trim()) {
        const query = encodeURIComponent(requestQuery);
        url = `/api/tmdb/search?q=${query}&media_type=${requestType}&page=${requestPage}`;
      } else {
        url = `/api/tmdb/discover/${requestType}?page=${requestPage}&sort_by=${requestSort}`;
        if (fromYear) url += `&primary_release_date.gte=${fromYear}-01-01`;
        if (toYear) url += `&primary_release_date.lte=${toYear}-12-31`;
        if (selectedGenres.length > 0)
          url += `&with_genres=${selectedGenres.join(",")}`;
        if (minRating > 0) url += `&vote_average.gte=${minRating}`;
        if (maxRating < 10) url += `&vote_average.lte=${maxRating}`;
      }

      console.log(`[Discover] Fetching: ${url}`);
      const res = await fetch(url, { signal: activeAbortController.signal });

      if (!res.ok) {
        throw new Error(`HTTP ${res.status}: ${res.statusText}`);
      }

      const data = await res.json();
      clearTimeout(timeoutId);

      // Check if request is still relevant
      if (requestId !== lastRequestId) {
        console.warn(`[Discover] Request #${requestId} is stale`);
        return;
      }

      if (
        requestType !== mediaType ||
        requestQuery !== searchQuery ||
        requestSort !== sortBy
      ) {
        console.warn(`[Discover] Discarding stale results`);
        return;
      }

      const newItems = data.results || [];

      if (!Array.isArray(newItems)) {
        throw new Error("Invalid API response");
      }

      // Add new items directly to pool - no caching needed
      addItemsToPool(newItems);

      currentPage = requestPage + 1;
      hasMore = newItems.length === ITEMS_PER_PAGE;
      error = null;
    } catch (err) {
      if (err instanceof Error && err.name === "AbortError") {
        console.warn(`[Discover] Request #${requestId} was aborted`);
      } else {
        console.error(`[Discover] Request #${requestId} failed:`, err);
        error = err instanceof Error ? err.message : "Failed to load content";
        hasMore = false;
      }
    } finally {
      clearTimeout(timeoutId);
      loading = false;
      activeAbortController = null;
      console.log(
        `[Discover] Request #${requestId} complete. Visible slots: ${pool.filter((s) => s.visible).length}`,
      );
    }
  }

  // ============= SCROLL HANDLING =============
  let scrollTimeout: number | undefined;

  function handleScroll() {
    if (!scrollContainer) return;

    if (scrollTimeout) clearTimeout(scrollTimeout);

    scrollTimeout = window.setTimeout(() => {
      const { scrollTop, scrollHeight, clientHeight } = scrollContainer;
      const distanceFromBottom = scrollHeight - scrollTop - clientHeight;

      if (distanceFromBottom < 500 && !loading && hasMore) {
        console.log("[Discover] Triggering fetchDiscoverData from scroll");
        fetchDiscoverData();
      }
    }, 100);
  }

  // ============= UI HELPERS =============
  function toggleGenre(genreId: number) {
    if (selectedGenres.includes(genreId)) {
      selectedGenres = selectedGenres.filter((id) => id !== genreId);
    } else {
      selectedGenres = [...selectedGenres, genreId];
    }
    fetchDiscoverData(true);
  }

  function clearFilters() {
    fromYear = "";
    toYear = "";
    selectedGenres = [];
    minRating = 0;
    maxRating = 10;
    fetchDiscoverData(true);
  }

  function changeMediaType(type: "movie" | "tv") {
    mediaType = type;
    selectedGenres = [];
    fetchGenres();
    fetchDiscoverData(true);
  }

  function changeSort(value: string) {
    sortBy = value;
    fetchDiscoverData(true);
  }

  let searchTimeoutId: number;
  function handleSearch() {
    clearTimeout(searchTimeoutId);
    searchTimeoutId = window.setTimeout(() => {
      fetchDiscoverData(true);
    }, 500);
  }

  function getBadge(
    item: TMDBMovie | TMDBTVShow,
  ): { text: string; variant: string } | null {
    const rating = item.vote_average || 0;
    const year =
      "release_date" in item
        ? parseInt(item.release_date?.substring(0, 4) || "0")
        : parseInt(item.first_air_date?.substring(0, 4) || "0");

    // Use named badge variants so Badge.svelte renders high-contrast text
    // (passing raw color strings was making text = background = invisible)
    if (rating >= 8.5) return { text: "TOP RATED", variant: "warning" };

    const currentYear = new Date().getFullYear();
    if (year && year <= currentYear - 30)
      return { text: "CLASSIC", variant: "primary" };

    return null;
  }

  function getTitle(item: TMDBMovie | TMDBTVShow): string {
    return "title" in item ? item.title : item.name;
  }

  const currentYear = new Date().getFullYear();
  const yearOptions = Array.from({ length: 100 }, (_, i) => currentYear - i);

  // ============= LIFECYCLE =============
  onMount(() => {
    console.log("[Discover] 🚀 Version 2.0.3 - 1000-Slot Pool Build");
    fetchGenres();
    fetchDiscoverData(true);

    return () => {
      if (scrollTimeout) clearTimeout(scrollTimeout);
      if (activeAbortController) activeAbortController.abort();
    };
  });
</script>

<svelte:head>
  <title>Discover - Flasharr</title>
</svelte:head>

<div class="discover-page">
  <!-- Control Bar -->
  <div class="control-bar glass-panel">
    <div class="search-section">
      <span class="material-icons search-icon">search</span>
      <input
        type="text"
        class="search-input"
        placeholder="Search {mediaType === 'movie' ? 'Movies' : 'TV Series'}..."
        bind:value={searchQuery}
        onclick={() => (searchQuery = searchQuery)}
        oninput={handleSearch}
      />
    </div>

    <div class="controls-right">
      <div class="media-toggle">
        <button
          class="toggle-btn"
          class:active={mediaType === "movie"}
          onclick={() => changeMediaType("movie")}
        >
          MOVIES
        </button>
        <button
          class="toggle-btn"
          class:active={mediaType === "tv"}
          onclick={() => changeMediaType("tv")}
        >
          TV
        </button>
      </div>

      <div class="select-wrapper">
        <select
          class="sort-select"
          bind:value={sortBy}
          onchange={() => changeSort(sortBy)}
        >
          {#each sortOptions as option}
            <option value={option.value}>{option.label}</option>
          {/each}
        </select>
        <span class="material-icons select-chevron">expand_more</span>
      </div>

      <button
        class="filter-toggle-btn"
        class:active={showFilters}
        onclick={() => (showFilters = !showFilters)}
      >
        <span class="material-icons">tune</span>
      </button>
    </div>
  </div>

  <!-- Main Content with Filters -->
  <div class="content-wrapper">
    <!-- Scrollable Grid -->
    <div
      class="discover-scroll-container"
      bind:this={scrollContainer}
      onscroll={handleScroll}
    >
      <div class="discover-grid">
        {#if loading && pool.filter((s) => s.visible).length === 0}
          <!-- Show simple loading message -->
          <div class="initial-loading">
            <div class="spinner"></div>
            <p>Loading content...</p>
          </div>
        {:else if error && pool.filter((s) => s.visible).length === 0}
          <!-- Show error state -->
          <div class="error-container">
            <ErrorState
              title="Failed to load content"
              message={error}
              showRetry={true}
              onRetry={() => fetchDiscoverData(true)}
            />
          </div>
        {:else}
          <!-- Object Pooling: Fixed array of 60 slots, only render visible ones -->
          {#each pool as slot (slot.slotId)}
            {#if slot.visible && slot.data}
              <MediaCard
                id={slot.data.id}
                title={getTitle(slot.data)}
                posterPath={slot.data.poster_path}
                voteAverage={slot.data.vote_average}
                releaseDate={"release_date" in slot.data
                  ? slot.data.release_date
                  : slot.data.first_air_date}
                overview={slot.data.overview}
                {mediaType}
                badge={getBadge(slot.data)
                  ? {
                      text: getBadge(slot.data)!.text,
                      variant: getBadge(slot.data)!.variant,
                    }
                  : undefined}
              />
            {/if}
          {/each}
        {/if}
      </div>

      {#if loading && pool.filter((s) => s.visible).length > 0}
        <div class="loading-indicator">
          <div class="spinner"></div>
          <p>Loading more...</p>
        </div>
      {/if}
    </div>

    <!-- Filter Sidebar (Drawer) -->
    {#if showFilters}
      <div
        class="filter-sidebar"
        transition:animeFly={{ x: 300, duration: 300 }}
      >
        <div class="filter-header">
          <h3>Intelligence Filters</h3>
          <button class="clear-btn" onclick={clearFilters}>Reset</button>
        </div>

        <div class="filter-section">
          <label for="from-year">Temporal Range (Year)</label>
          <div class="year-inputs">
            <div class="select-wrapper">
              <select
                id="from-year"
                bind:value={fromYear}
                onchange={() => fetchDiscoverData(true)}
              >
                <option value="">Starting Year</option>
                {#each yearOptions as year}
                  <option value={year}>{year}</option>
                {/each}
              </select>
            </div>
            <div class="select-wrapper">
              <select
                id="to-year"
                bind:value={toYear}
                onchange={() => fetchDiscoverData(true)}
                aria-label="To Year"
              >
                <option value="">Ending Year</option>
                {#each yearOptions as year}
                  <option value={year}>{year}</option>
                {/each}
              </select>
            </div>
          </div>
        </div>

        <div class="filter-section">
          <div role="group" aria-label="Genres">
            <span class="filter-label-text">Genres</span>
            <div class="genre-tags">
              {#each genres as genre}
                <button
                  class="genre-tag"
                  class:active={selectedGenres.includes(genre.id)}
                  onclick={() => toggleGenre(genre.id)}
                >
                  {genre.name}
                </button>
              {/each}
            </div>
          </div>
        </div>

        <div class="filter-section">
          <span class="filter-label-text"
            >Intelligence Grade ({minRating} - {maxRating})</span
          >
          <div class="rating-slider-v2">
            <div class="rating-row">
              <span>Min: {minRating}</span>
              <input
                id="min-rating"
                type="range"
                min="0"
                max="10"
                step="0.5"
                bind:value={minRating}
                onchange={() => fetchDiscoverData(true)}
              />
            </div>
            <div class="rating-row">
              <span>Max: {maxRating}</span>
              <input
                id="max-rating"
                type="range"
                min="0"
                max="10"
                step="0.5"
                bind:value={maxRating}
                onchange={() => fetchDiscoverData(true)}
                aria-label="Max Rating"
              />
            </div>
          </div>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .discover-page {
    display: flex;
    flex-direction: column;
    height: 100%; /* Fill the container, do not overflow */
    background: transparent;
    color: #fff;
    overflow: hidden;
    position: relative;
  }

  /* Control Bar */
  .control-bar {
    display: flex;
    gap: 1.5rem;
    padding: 1.25rem 2rem;
    background: rgba(5, 7, 10, 0.6);
    backdrop-filter: blur(12px);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    flex-shrink: 0;
    align-items: center;
    z-index: 100;
    box-shadow: 0 4px 30px rgba(0, 0, 0, 0.2);
  }

  .search-section {
    flex: 1;
    max-width: 500px;
    position: relative;
    display: flex;
    align-items: center;
  }

  .search-icon {
    position: absolute;
    left: 1rem;
    color: var(--text-muted);
    font-size: 1.2rem;
    pointer-events: none;
  }

  .search-input {
    width: 100%;
    padding: 0.75rem 1rem 0.75rem 3rem;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #fff;
    font-size: 0.9rem;
    transition: all 0.3s;
    font-family: var(--font-mono, monospace);
    letter-spacing: 0.05em;
    clip-path: polygon(
      0% 0%,
      calc(100% - 10px) 0%,
      100% 10px,
      100% 100%,
      10px 100%,
      0% calc(100% - 10px)
    );
  }

  .search-input:focus {
    outline: none;
    border-color: var(--color-primary);
    background: rgba(0, 0, 0, 0.6);
    box-shadow:
      0 0 0 1px var(--color-primary),
      0 0 20px rgba(0, 243, 255, 0.1);
  }

  .controls-right {
    display: flex;
    gap: 1.25rem;
    align-items: center;
    margin-left: auto;
  }

  .media-toggle {
    display: flex;
    gap: 0.25rem;
    background: rgba(0, 0, 0, 0.6);
    padding: 0.25rem;
    border: 1px solid rgba(255, 255, 255, 0.1);
    position: relative;
    clip-path: polygon(
      4px 0%,
      calc(100% - 4px) 0%,
      100% 4px,
      100% calc(100% - 4px),
      calc(100% - 4px) 100%,
      4px 100%,
      0% calc(100% - 4px),
      0% 4px
    );
  }

  .toggle-btn {
    padding: 0.5rem 1.25rem;
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-weight: 800;
    font-size: 0.7rem;
    letter-spacing: 0.15em;
    cursor: pointer;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    clip-path: polygon(
      4px 0%,
      100% 0%,
      100% calc(100% - 4px),
      calc(100% - 4px) 100%,
      0% 100%,
      0% 4px
    );
  }

  .toggle-btn.active {
    background: linear-gradient(
      135deg,
      rgba(0, 243, 255, 0.2),
      rgba(0, 243, 255, 0.05)
    );
    color: var(--color-primary);
    box-shadow: 0 0 15px rgba(0, 243, 255, 0.2);
    font-weight: 900;
  }

  .select-wrapper {
    position: relative;
    display: flex;
    align-items: center;
  }

  .sort-select {
    padding: 0.65rem 2.5rem 0.65rem 1rem;
    background: rgba(0, 0, 0, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: #fff;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    appearance: none;
    transition: all 0.3s;
    font-family: var(--font-mono, monospace);
    clip-path: polygon(
      0% 0%,
      calc(100% - 8px) 0%,
      100% 8px,
      100% 100%,
      8px 100%,
      0% calc(100% - 8px)
    );
  }

  .filter-toggle-btn {
    padding: 0.65rem;
    background: rgba(0, 0, 0, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: #fff;
    cursor: pointer;
    transition: all 0.3s;
    display: flex;
    align-items: center;
    position: relative;
    overflow: hidden;
    clip-path: polygon(
      4px 0%,
      calc(100% - 4px) 0%,
      100% 4px,
      100% calc(100% - 4px),
      calc(100% - 4px) 100%,
      4px 100%,
      0% calc(100% - 4px),
      0% 4px
    );
  }
  .filter-toggle-btn:hover {
    border-color: var(--color-primary);
    color: var(--color-primary);
    box-shadow: 0 0 15px rgba(0, 243, 255, 0.15);
  }

  .filter-toggle-btn.active {
    background: var(--color-primary);
    color: #000;
    border-color: var(--color-primary);
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.4);
  }

  /* Content Wrapper */
  .content-wrapper {
    display: flex;
    flex: 1;
    overflow: hidden;
    position: relative;
    height: 100%; /* Ensure full height */
  }

  /* Scrollable Container */
  .discover-scroll-container {
    flex: 1;
    overflow-y: auto;
    padding: 2.5rem;
    transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .content-wrapper:has(.filter-sidebar) .discover-scroll-container {
    padding-right: 400px; /* Instead of margin, use padding to keep layout stable */
    margin-right: 0;
  }

  /* Grid - 5 items per row */
  .discover-grid {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 1.5rem;
    padding-bottom: 4rem;
  }

  .error-container,
  .initial-loading {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    min-height: 400px;
    gap: 1rem;
  }

  .initial-loading p {
    color: var(--text-muted);
    font-size: 1rem;
    font-family: var(--font-mono, monospace);
    letter-spacing: 0.1em;
  }

  /* Filter Sidebar - Cyber Upgrade */
  .filter-sidebar {
    position: absolute;
    right: 0;
    top: 0;
    bottom: 0;
    width: 380px;
    background: rgba(8, 10, 15, 0.85); /* Increased opacity for readability */
    backdrop-filter: blur(40px);
    -webkit-backdrop-filter: blur(40px);
    border-left: 1px solid var(--color-primary); /* Always neon when present */
    padding: 2rem 1.5rem;
    overflow-y: auto;
    z-index: 1000;
    box-shadow: -10px 0 50px rgba(0, 0, 0, 0.8);
  }

  .filter-sidebar::before {
    /* Top cyber accent */
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 2px;
    background: linear-gradient(
      90deg,
      transparent,
      var(--color-primary),
      transparent
    );
    opacity: 0.8;
  }

  .filter-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 2.5rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  }

  .filter-header h3 {
    font-size: 1.1rem;
    font-weight: 900;
    text-transform: uppercase;
    letter-spacing: 0.15em;
    color: #fff;
    margin: 0;
    text-shadow: 0 0 10px rgba(0, 243, 255, 0.3);
  }

  .clear-btn {
    padding: 0.4rem 0.8rem;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    color: var(--text-muted);
    font-size: 0.65rem;
    font-weight: 800;
    cursor: pointer;
    transition: all 0.2s;
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .clear-btn:hover {
    background: rgba(255, 0, 0, 0.1);
    color: #ff4d4d;
    border-color: rgba(255, 0, 0, 0.2);
  }

  .filter-section {
    margin-bottom: 1.5rem;
  }

  .filter-section label,
  .filter-label-text {
    display: block;
    font-weight: 800;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.15em;
    margin-bottom: 0.6rem;
    color: var(--text-muted);
  }

  .year-inputs {
    display: flex;
    gap: 1rem;
  }

  .year-inputs select {
    width: 100%;
  }

  .genre-tags {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.4rem;
  }

  .genre-tag {
    padding: 0.35rem 0.25rem;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: var(--text-secondary);
    font-size: 0.6rem;
    font-weight: 700;
    cursor: pointer;
    transition: all 0.2s;
    font-family: var(--font-mono, monospace);
    display: flex;
    align-items: center;
    justify-content: center;
    text-align: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    clip-path: polygon(
      4px 0%,
      100% 0%,
      100% calc(100% - 4px),
      calc(100% - 4px) 100%,
      0% 100%,
      0% 4px
    );
  }

  .genre-tag:hover {
    border-color: rgba(0, 243, 255, 0.4);
    color: var(--color-primary);
    background: rgba(0, 243, 255, 0.05);
  }

  .genre-tag.active {
    background: linear-gradient(
      135deg,
      rgba(0, 243, 255, 0.2),
      rgba(0, 243, 255, 0.1)
    );
    border-color: var(--color-primary);
    color: var(--color-primary);
    box-shadow: 0 0 15px rgba(0, 243, 255, 0.2);
  }

  .rating-slider-v2 {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .rating-row {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .rating-row span {
    font-size: 0.75rem;
    color: var(--color-primary);
    font-family: var(--font-mono);
  }

  .rating-slider-v2 input[type="range"] {
    width: 100%;
    -webkit-appearance: none;
    appearance: none;
    background: rgba(255, 255, 255, 0.05);
    height: 4px;
    border-radius: 2px;
  }

  .rating-slider-v2 input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 18px;
    height: 18px;
    background: #ffd700;
    border-radius: 50%;
    cursor: pointer;
    box-shadow: 0 0 10px rgba(255, 215, 0, 0.3);
  }

  /* Loading */
  .loading-indicator {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 5rem;
    gap: 1.5rem;
    color: var(--text-muted);
  }

  .spinner {
    width: 32px;
    height: 32px;
    border: 2px solid rgba(0, 243, 255, 0.1);
    border-top-color: var(--color-primary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Responsive */
  @media (max-width: 1024px) {
    .discover-grid {
      grid-template-columns: repeat(4, 1fr);
      gap: 1.25rem;
    }
  }

  @media (max-width: 768px) {
    .discover-grid {
      grid-template-columns: repeat(2, 1fr);
      gap: 0.75rem;
    }

    .control-bar {
      padding: 0.75rem;
      gap: 0.5rem;
      flex-wrap: wrap;
    }

    .search-section {
      width: 100%;
      max-width: none;
      order: 1;
    }

    .search-input {
      font-size: 14px;
      padding: 0.6rem 0.75rem 0.6rem 2.5rem;
    }

    .search-icon {
      font-size: 1rem;
      left: 0.75rem;
    }

    .controls-right {
      width: 100%;
      justify-content: space-between;
      order: 2;
      gap: 0.5rem;
    }

    .toggle-btn {
      padding: 0.4rem 0.75rem;
      font-size: 0.6rem;
    }

    .sort-select {
      padding: 0.5rem 2rem 0.5rem 0.75rem;
      font-size: 0.75rem;
    }

    .filter-toggle-btn {
      padding: 0.5rem;
    }

    .discover-scroll-container {
      padding: 0.75rem;
    }

    .filter-sidebar {
      width: 100%;
    }

    .content-wrapper:has(.filter-sidebar) .discover-scroll-container {
      margin-right: 0;
      padding-right: 0.75rem;
    }
  }
</style>
