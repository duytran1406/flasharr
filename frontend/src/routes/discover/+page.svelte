<script lang="ts">
  import { onMount } from "svelte";
  import { animeFly } from "$lib/animations";
  import type { TMDBMovie, TMDBTVShow } from "$lib/types/tmdb";
  import { MediaCard, ErrorState } from "$lib/components";
  import DiscoverFilterBar, { type UnifiedGenre } from "$lib/components/DiscoverFilterBar.svelte";

  // ============= CONFIGURATION =============
  const ITEMS_PER_PAGE = 20; // TMDB returns 20 per page
  const POOL_SIZE = 1000;    // Pre-create 1000 slots for extensive scrolling

  // ============= POOL SLOT TYPE =============
  interface PoolSlot {
    slotId: number;
    visible: boolean;
    data: TMDBMovie | TMDBTVShow | null;
  }

  // ============= FILTER STATE =============
  let mediaType: "movie" | "tv" = $state("movie");
  let sortBy = $state("popularity.desc");
  let searchQuery = $state("");
  let fromYear = $state("");
  let toYear = $state("");
  let selectedGenres = $state<UnifiedGenre[]>([]);
  let minRating = $state(0);
  let maxRating = $state(10);

  // Exposed from DiscoverFilterBar (resolved IDs for current mediaType)
  let resolvedGenreIds = $state<number[]>([]);
  let hasFilter = $state(false);

  // ============= OBJECT POOL =============
  let pool = $state<PoolSlot[]>(
    Array.from({ length: POOL_SIZE }, (_, i) => ({
      slotId: i,
      visible: false,
      data: null,
    }))
  );

  let nextPoolIndex = 0;

  // Request management
  let activeAbortController: AbortController | null = null;
  let lastRequestId = 0;
  let loading = $state(false);
  let error = $state<string | null>(null);
  let currentPage = $state(1);
  let hasMore = $state(true);
  let scrollContainer: HTMLElement;

  // ============= POOL MANAGEMENT =============
  function addItemsToPool(items: (TMDBMovie | TMDBTVShow)[]) {
    for (const item of items) {
      if (nextPoolIndex < POOL_SIZE) {
        pool[nextPoolIndex].data = item;
        pool[nextPoolIndex].visible = true;
        nextPoolIndex++;
      } else {
        console.warn("[Discover] Pool is full");
        break;
      }
    }
  }

  function clearPool() {
    for (let i = 0; i < POOL_SIZE; i++) {
      pool[i].visible = false;
      pool[i].data = null;
    }
    nextPoolIndex = 0;
  }

  // ============= FETCH LOGIC =============
  async function fetchDiscoverData(reset = false) {
    if (!reset && (loading || !hasMore)) return;

    if (activeAbortController) activeAbortController.abort();
    activeAbortController = new AbortController();
    const requestId = ++lastRequestId;
    const timeoutId = setTimeout(() => {
      console.error("[Discover] Request timeout");
      activeAbortController?.abort();
    }, 10000);

    const requestType = mediaType;
    const requestQuery = searchQuery;
    const requestSort = sortBy;
    const requestPage = reset ? 1 : currentPage;
    const requestGenres = [...resolvedGenreIds];
    const requestFromYear = fromYear;
    const requestToYear = toYear;
    const requestMinRating = minRating;
    const requestMaxRating = maxRating;

    loading = true;

    if (reset) {
      currentPage = 1;
      clearPool();
      hasMore = true;
    }

    try {
      let url = "";

      if (requestSort === "popular_today") {
        url = `/api/discovery/popular-today?type=${requestType}&page=${requestPage}&limit=${ITEMS_PER_PAGE}`;
      } else if (requestQuery.trim()) {
        const q = encodeURIComponent(requestQuery);
        url = `/api/tmdb/search?q=${q}&media_type=${requestType}&page=${requestPage}`;
      } else {
        url = `/api/tmdb/discover/${requestType}?page=${requestPage}&sort_by=${requestSort}`;

        // Date param differs for movies vs TV
        const dateParam =
          requestType === "movie" ? "primary_release_date" : "first_air_date";

        if (requestFromYear) url += `&${dateParam}.gte=${requestFromYear}-01-01`;
        if (requestToYear)   url += `&${dateParam}.lte=${requestToYear}-12-31`;

        if (requestGenres.length > 0)
          url += `&with_genres=${requestGenres.join(",")}`;

        if (requestMinRating > 0)  url += `&vote_average.gte=${requestMinRating}`;
        if (requestMaxRating < 10) url += `&vote_average.lte=${requestMaxRating}`;
      }

      const res = await fetch(url, { signal: activeAbortController.signal });
      if (!res.ok) throw new Error(`HTTP ${res.status}: ${res.statusText}`);

      const data = await res.json();
      clearTimeout(timeoutId);

      if (requestId !== lastRequestId) {
        console.warn(`[Discover] Request #${requestId} stale`);
        return;
      }

      if (
        requestType !== mediaType ||
        requestQuery !== searchQuery ||
        requestSort !== sortBy
      ) {
        console.warn("[Discover] Discarding stale results");
        return;
      }

      let newItems: (TMDBMovie | TMDBTVShow)[] = data.results || [];
      if (!Array.isArray(newItems)) throw new Error("Invalid API response");

      // Client-side filter pass when querying (TMDB search ignores filter params)
      if (requestQuery.trim()) {
        if (requestMinRating > 0)
          newItems = newItems.filter((i) => (i.vote_average || 0) >= requestMinRating);
        if (requestMaxRating < 10)
          newItems = newItems.filter((i) => (i.vote_average || 0) <= requestMaxRating);
        if (requestGenres.length > 0)
          newItems = newItems.filter((i) =>
            (i as any).genre_ids?.some((id: number) => requestGenres.includes(id))
          );
        if (requestFromYear || requestToYear) {
          newItems = newItems.filter((i) => {
            const dateStr =
              (i as TMDBMovie).release_date || (i as TMDBTVShow).first_air_date;
            if (!dateStr) return !requestFromYear;
            const year = new Date(dateStr).getFullYear();
            if (requestFromYear && year < parseInt(requestFromYear)) return false;
            if (requestToYear && year > parseInt(requestToYear)) return false;
            return true;
          });
        }
      }

      addItemsToPool(newItems);
      currentPage = requestPage + 1;
      hasMore = newItems.length === ITEMS_PER_PAGE;
      error = null;
    } catch (err) {
      if (err instanceof Error && err.name === "AbortError") {
        console.warn(`[Discover] Request #${requestId} aborted`);
      } else {
        console.error(`[Discover] Request #${requestId} failed:`, err);
        error = err instanceof Error ? err.message : "Failed to load content";
        hasMore = false;
      }
    } finally {
      clearTimeout(timeoutId);
      loading = false;
      activeAbortController = null;
    }
  }

  // ============= SCROLL HANDLING =============
  let scrollTimeout: number | undefined;

  function handleScroll() {
    if (!scrollContainer) return;
    if (scrollTimeout) clearTimeout(scrollTimeout);
    scrollTimeout = window.setTimeout(() => {
      const { scrollTop, scrollHeight, clientHeight } = scrollContainer;
      if (scrollHeight - scrollTop - clientHeight < 500 && !loading && hasMore) {
        fetchDiscoverData();
      }
    }, 100);
  }

  // ============= FILTER CALLBACKS (passed to DiscoverFilterBar) =============

  function handleMediaTypeChange(type: "movie" | "tv") {
    mediaType = type;
    // Reset genres that don't apply to the new type — the bar will re-filter availableGenres
    // but we keep the selection; resolvedGenreIds will produce correct IDs automatically
    fetchDiscoverData(true);
  }

  function handleSortChange(value: string) {
    sortBy = value;
    fetchDiscoverData(true);
  }

  let searchDebounceId: number;
  function handleSearchInput() {
    clearTimeout(searchDebounceId);
    searchDebounceId = window.setTimeout(() => fetchDiscoverData(true), 450);
  }

  function handleGenreToggle(genre: UnifiedGenre) {
    const idx = selectedGenres.findIndex((g) => g.name === genre.name);
    if (idx >= 0) {
      selectedGenres = selectedGenres.filter((_, i) => i !== idx);
    } else {
      selectedGenres = [...selectedGenres, genre];
    }
    // resolvedGenreIds derived in the component; wait a tick then fetch
    // Use setTimeout 0 so Svelte 5 flushes binding before fetch reads it
    setTimeout(() => fetchDiscoverData(true), 0);
  }

  function handleYearChange() {
    fetchDiscoverData(true);
  }

  function handleRatingChange() {
    fetchDiscoverData(true);
  }

  function handleClearAll() {
    selectedGenres = [];
    fromYear = "";
    toYear = "";
    minRating = 0;
    maxRating = 10;
    searchQuery = "";
    fetchDiscoverData(true);
  }

  // ============= HELPERS =============
  function getBadge(item: TMDBMovie | TMDBTVShow): { text: string; variant: string } | null {
    const rating = item.vote_average || 0;
    const year =
      "release_date" in item
        ? parseInt(item.release_date?.substring(0, 4) || "0")
        : parseInt(item.first_air_date?.substring(0, 4) || "0");

    if (rating >= 8.5) return { text: "TOP RATED", variant: "warning" };
    const now = new Date().getFullYear();
    if (year && year <= now - 30) return { text: "CLASSIC", variant: "primary" };
    return null;
  }

  function getTitle(item: TMDBMovie | TMDBTVShow): string {
    return "title" in item ? item.title : item.name;
  }

  // ============= LIFECYCLE =============
  onMount(() => {
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

  <!-- ── Inline Filter Bar (replaces drawer sidebar) ─────────────────── -->
  <DiscoverFilterBar
    bind:mediaType
    bind:sortBy
    bind:searchQuery
    bind:selectedGenres
    bind:fromYear
    bind:toYear
    bind:minRating
    bind:maxRating
    bind:resolvedGenreIds
    bind:hasFilter
    onMediaTypeChange={handleMediaTypeChange}
    onSortChange={handleSortChange}
    onSearchInput={handleSearchInput}
    onGenreToggle={handleGenreToggle}
    onYearChange={handleYearChange}
    onRatingChange={handleRatingChange}
    onClearAll={handleClearAll}
  />

  <!-- ── Scrollable Grid ─────────────────────────────────────────────── -->
  <div
    class="discover-scroll-container"
    bind:this={scrollContainer}
    onscroll={handleScroll}
  >
    <div class="discover-grid">
      {#if loading && pool.filter((s) => s.visible).length === 0}
        <div class="initial-loading">
          <div class="spinner"></div>
          <p>Loading content...</p>
        </div>
      {:else if error && pool.filter((s) => s.visible).length === 0}
        <div class="error-container">
          <ErrorState
            title="Failed to load content"
            message={error}
            showRetry={true}
            onRetry={() => fetchDiscoverData(true)}
          />
        </div>
      {:else}
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

    {#if !hasMore && pool.filter((s) => s.visible).length > 0 && !loading}
      <p class="end-label">— end of results —</p>
    {/if}
  </div>
</div>

<style>
  .discover-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: transparent;
    color: #fff;
    overflow: hidden;
    position: relative;
  }

  /* Scrollable Container */
  .discover-scroll-container {
    flex: 1;
    overflow-y: auto;
    padding: 2rem 2.5rem 4rem;
  }

  /* Grid — 5 items per row on wide screens */
  .discover-grid {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 1.5rem;
    padding-bottom: 2rem;
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

  /* Loading */
  .loading-indicator {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 4rem;
    gap: 1.25rem;
    color: var(--text-muted);
  }

  .spinner {
    width: 30px;
    height: 30px;
    border: 2px solid rgba(0, 243, 255, 0.1);
    border-top-color: var(--color-primary, #00f3ff);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .end-label {
    text-align: center;
    padding: 1.5rem 0 3rem;
    color: rgba(255, 255, 255, 0.18);
    font-size: 0.7rem;
    font-family: var(--font-mono, monospace);
    letter-spacing: 0.12em;
  }

  /* Responsive */
  @media (max-width: 1280px) {
    .discover-grid {
      grid-template-columns: repeat(4, 1fr);
      gap: 1.25rem;
    }
  }

  @media (max-width: 1024px) {
    .discover-grid {
      grid-template-columns: repeat(3, 1fr);
      gap: 1rem;
    }
  }

  @media (max-width: 768px) {
    .discover-grid {
      grid-template-columns: repeat(2, 1fr);
      gap: 0.75rem;
    }
    .discover-scroll-container {
      padding: 1rem 0.75rem 2rem;
    }
  }
</style>
