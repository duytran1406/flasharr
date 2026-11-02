<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import DiscoverFilterBar, {
    type DiscoverMediaType,
    type UnifiedGenre,
  } from "$lib/components/DiscoverFilterBar.svelte";
  import DiscoverResultsGrid, {
    type DiscoverBadge,
    type DiscoverItem,
    type DiscoverStatusType,
    type PoolSlot,
  } from "$lib/components/discover/DiscoverResultsGrid.svelte";
  import type { TMDBMovie, TMDBTVShow } from "$lib/types/tmdb";
  import { fetchAllMovies, type RadarrMovie } from "$lib/stores/arr";
  import { onDownloadEvent } from "$lib/stores/downloads.svelte";

  // ============= CONFIGURATION =============
  const ITEMS_PER_PAGE = 20; // TMDB returns 20 per page
  const POOL_SIZE = 1000;    // Pre-create 1000 slots for extensive scrolling

  // ============= LIBRARY STATE (status badges on cards) =============
  let libraryMovies = $state<RadarrMovie[]>([]);
  let downloadingTmdbIds = $state(new Set<number>());

  function libraryByTmdbId(): Map<number, RadarrMovie> {
    return new Map(libraryMovies.map((movie) => [Number(movie.tmdbId), movie]));
  }

  async function loadLibrary() {
    try {
      libraryMovies = await fetchAllMovies();
    } catch {
      // Status badges are optional; keep the previous library snapshot
    }
  }

  // ============= FILTER STATE =============
  let mediaType = $state<DiscoverMediaType>("movie");
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

  // Resolve genre IDs synchronously so a fetch right after a toggle sees the new selection
  function resolveGenreIds(genres: UnifiedGenre[], type: DiscoverMediaType): number[] {
    return genres
      .map((g) => (type === "movie" ? g.movieId : g.tvId))
      .filter((id): id is number => id !== null);
  }

  function hasActiveFilters(): boolean {
    return (
      selectedGenres.length > 0 ||
      fromYear !== "" ||
      toYear !== "" ||
      minRating > 0 ||
      maxRating < 10
    );
  }

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
  let scrollTimeout: number | undefined;

  // ============= POOL MANAGEMENT =============
  function addItemsToPool(items: DiscoverItem[]) {
    for (const item of items) {
      if (nextPoolIndex >= POOL_SIZE) break;
      pool[nextPoolIndex].data = item;
      pool[nextPoolIndex].visible = true;
      nextPoolIndex++;
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
    const timeoutId = setTimeout(() => activeAbortController?.abort(), 10000);

    const requestType = mediaType;
    const requestQuery = searchQuery;
    const requestSort = sortBy;
    const requestPage = reset ? 1 : currentPage;
    const requestGenres = resolveGenreIds(selectedGenres, requestType);
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

      if (requestSort === "popular_today" && !hasActiveFilters()) {
        url = `/api/discovery/popular-today?type=${requestType}&page=${requestPage}&limit=${ITEMS_PER_PAGE}`;
      } else if (requestQuery.trim()) {
        const q = encodeURIComponent(requestQuery);
        url = `/api/tmdb/search?q=${q}&media_type=${requestType}&page=${requestPage}`;
      } else {
        // "Popular Today" has no discover equivalent once filters are set
        const discoverSort =
          requestSort === "popular_today" ? "popularity.desc" : requestSort;
        // Date param differs for movies vs TV
        const dateParam =
          requestType === "movie" ? "primary_release_date" : "first_air_date";

        url = `/api/tmdb/discover/${requestType}?page=${requestPage}&sort_by=${discoverSort}`;

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

      // Drop stale responses (a newer request or a filter change happened meanwhile)
      if (
        requestId !== lastRequestId ||
        requestType !== mediaType ||
        requestQuery !== searchQuery ||
        requestSort !== sortBy
      ) {
        return;
      }

      let newItems: DiscoverItem[] = Array.isArray(data.results) ? data.results : [];

      // Client-side filter pass (search and popular-today ignore filter params)
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

      addItemsToPool(newItems);
      currentPage = requestPage + 1;
      hasMore = newItems.length === ITEMS_PER_PAGE;
      error = null;
    } catch (err) {
      if (!(err instanceof Error && err.name === "AbortError")) {
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
  function handleMediaTypeChange(type: DiscoverMediaType) {
    // The discover page has no collection tab
    if (type === "collections") return;
    mediaType = type;
    fetchDiscoverData(true);
  }

  function handleSortChange(value: string) {
    sortBy = value;
    fetchDiscoverData(true);
  }

  let searchDebounceId: number | undefined;
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
    fetchDiscoverData(true);
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
  function getBadge(item: DiscoverItem): DiscoverBadge | null {
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

  function getTitle(item: DiscoverItem): string {
    return "title" in item ? item.title : item.name;
  }

  function getStatusType(item: DiscoverItem): DiscoverStatusType {
    const tmdbId = Number(item.id);
    if (downloadingTmdbIds.has(tmdbId)) return "purple";

    const libraryItem = libraryByTmdbId().get(tmdbId);
    if (!libraryItem) return "gray";

    const releaseDate = "release_date" in item ? item.release_date : item.first_air_date;
    const unreleased = releaseDate ? new Date(releaseDate).getTime() > Date.now() : false;
    const monitored = libraryItem.monitored === true;
    const hasFile = libraryItem.hasFile === true;

    if (monitored && unreleased) return "blue";
    if (!hasFile && !monitored) return "yellow";
    if (!hasFile && monitored) return "red";
    if (hasFile && monitored) return "green";
    return "yellow";
  }

  function getStatusLabel(item: DiscoverItem): string {
    const status = getStatusType(item);
    if (status === "blue") return "MONITORED · UNRELEASED";
    if (status === "yellow") return "MISSING · UNMONITORED";
    if (status === "red") return "MISSING · MONITORED";
    if (status === "green") return "DOWNLOADED · MONITORED";
    if (status === "purple") return "DOWNLOADING";
    return "NOT IN LIBRARY";
  }

  // /discover?search focuses the title search field
  $effect(() => {
    if ($page.url.searchParams.has("search")) {
      requestAnimationFrame(() => document.getElementById("filter-search")?.focus());
    }
  });

  // ============= LIFECYCLE =============
  onMount(() => {
    loadLibrary();

    const stopTaskUpdated = onDownloadEvent("TASK_UPDATED", (event) => {
      const task = event.detail as any;
      const tmdbId = Number(task?.tmdb_id ?? task?.tmdbId);
      if (!tmdbId) return;
      if (String(task?.state || "").toUpperCase() === "COMPLETED") {
        downloadingTmdbIds = new Set([...downloadingTmdbIds].filter((id) => id !== tmdbId));
        loadLibrary();
      } else {
        downloadingTmdbIds = new Set([...downloadingTmdbIds, tmdbId]);
      }
    });

    const stopTaskCompleted = onDownloadEvent("TASK_COMPLETED", (event) => {
      const task = event.detail as any;
      const tmdbId = Number(task?.tmdb_id ?? task?.tmdbId);
      if (!tmdbId) return;
      downloadingTmdbIds = new Set([...downloadingTmdbIds].filter((id) => id !== tmdbId));
      loadLibrary();
    });

    fetchDiscoverData(true);

    return () => {
      if (scrollTimeout) clearTimeout(scrollTimeout);
      if (activeAbortController) activeAbortController.abort();
      stopTaskUpdated();
      stopTaskCompleted();
    };
  });
</script>

<svelte:head>
  <title>Discover - Flasharr</title>
</svelte:head>

<div class="discover-page">

  <!-- ── Inline Filter Bar (replaces drawer sidebar) ─────────────────── -->
  <DiscoverFilterBar
    onMediaTypeChange={handleMediaTypeChange}
    onSortChange={handleSortChange}
    onSearchInput={handleSearchInput}
    onGenreToggle={handleGenreToggle}
    onYearChange={handleYearChange}
    onRatingChange={handleRatingChange}
    onClearAll={handleClearAll}
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
  />

  <!-- ── Scrollable Grid ─────────────────────────────────────────────── -->
  <div
    class="discover-scroll-container"
    bind:this={scrollContainer}
    onscroll={handleScroll}
  >
    <DiscoverResultsGrid
      {pool}
      {loading}
      {error}
      {hasMore}
      {mediaType}
      {getTitle}
      {getBadge}
      {getStatusType}
      {getStatusLabel}
      onRetry={() => fetchDiscoverData(true)}
      onClear={handleClearAll}
    />
  </div>
</div>

<style>
  .discover-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: transparent;
    color: var(--cui-text-primary);
    overflow: hidden;
    position: relative;
  }

  /* Scrollable Container */
  .discover-scroll-container {
    position: relative;
    z-index: 1;
    flex: 1;
    overflow-y: auto;
    padding: var(--cui-space-xl) calc(var(--cui-space-xl) + var(--cui-space-xs))
      var(--cui-space-xxl);
  }

  @media (max-width: 768px) {
    .discover-scroll-container {
      padding: var(--cui-space-md) var(--cui-space-xs)
        calc(var(--cui-mobile-bottom-nav-height) + var(--cui-space-xl));
    }
  }

  @media (max-width: 767px) {
    .discover-page {
      padding-bottom: calc(
        var(--cui-mobile-bottom-nav-height) + var(--cui-mobile-safe-area-bottom)
      );
    }

    .discover-scroll-container {
      padding-bottom: calc(
        var(--cui-mobile-bottom-nav-height) + var(--cui-space-xxl) +
          var(--cui-mobile-safe-area-bottom)
      );
      scroll-padding-bottom: calc(
        var(--cui-mobile-bottom-nav-height) + var(--cui-space-xxl) +
          var(--cui-mobile-safe-area-bottom)
      );
    }
  }
</style>
