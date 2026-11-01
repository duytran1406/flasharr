<script lang="ts">
  import { onMount } from "svelte";
  import { ErrorState } from "@media-set/core-ui";
  import MediaCard from "$lib/components/MediaCard.svelte";
  import LibraryControlShell from "$lib/components/library/LibraryControlShell.svelte";
  import LibraryStatsStrip from "$lib/components/library/LibraryStatsStrip.svelte";
  import LibraryCollectionsGrid, {
    type LibraryCollection,
  } from "$lib/components/library/LibraryCollectionsGrid.svelte";
  import type {
    DiscoverMediaType,
    UnifiedGenre,
  } from "$lib/components/DiscoverFilterBar.svelte";
  import {
    fetchAllSeries,
    fetchAllMovies,
    type SonarrSeries,
    type RadarrMovie,
  } from "$lib/stores/arr";
  import { sonarrConfigured, radarrConfigured } from "$lib/stores/system";

  type LibraryTab = "tv" | "movies" | "collections";

  let activeTab = $state<LibraryTab>("tv");
  const viewMode = "grid";

  // Filter bar state (shared DiscoverFilterBar contract)
  let searchQuery = $state("");
  let sortBy = $state("title");
  let filterStatus = $state("all");
  let selectedGenres = $state<UnifiedGenre[]>([]);
  let fromYear = $state("");
  let toYear = $state("");
  let minRating = $state(0);
  let maxRating = $state(10);
  let resolvedGenreIds = $state<number[]>([]);
  let hasFilter = $state(false);

  // Infinite scroll
  let visibleCount = $state(60);
  const PAGE_SIZE = 60;
  let sentinel = $state<HTMLDivElement | undefined>(undefined);

  let series = $state<SonarrSeries[]>([]);
  let movies = $state<RadarrMovie[]>([]);
  let loading = $state(true);
  let seriesError = $state<string | null>(null);
  let moviesError = $state<string | null>(null);

  const hasSonarr = $derived($sonarrConfigured);
  const hasRadarr = $derived($radarrConfigured);
  const hasLibrary = $derived(
    hasSonarr || hasRadarr || series.length > 0 || movies.length > 0,
  );
  const filterMediaType = $derived<"movie" | "tv">(
    activeTab === "tv" ? "tv" : "movie",
  );

  onMount(async () => {
    if (hasRadarr && !hasSonarr) activeTab = "movies";

    const headerContainer = document.getElementById("header-dynamic-content");
    if (headerContainer) {
      headerContainer.innerHTML = `
        <div style="display: flex; align-items: center; gap: 0.75rem;">
          <span class="material-icons" style="color: #a78bfa; font-size: 1.5rem;">video_library</span>
          <h1 style="font-size: 0.9rem; font-weight: 800; letter-spacing: 0.1em; text-transform: uppercase; margin: 0; color: #fff;">Media Library</h1>
        </div>
      `;
    }

    await loadLibrary();
  });

  async function loadLibrary() {
    loading = true;
    seriesError = null;
    moviesError = null;

    const [seriesResult, moviesResult] = await Promise.allSettled([
      hasSonarr ? fetchAllSeries() : Promise.resolve(series),
      hasRadarr ? fetchAllMovies() : Promise.resolve(movies),
    ]);

    if (seriesResult.status === "fulfilled") {
      series = seriesResult.value;
    } else {
      console.error("Failed to load series:", seriesResult.reason);
      seriesError = "Could not load Sonarr series.";
    }

    if (moviesResult.status === "fulfilled") {
      movies = moviesResult.value;
    } else {
      console.error("Failed to load movies:", moviesResult.reason);
      moviesError = "Could not load Radarr movies.";
    }

    loading = false;
  }

  // Infinite scroll: grow the visible window when the sentinel comes into view
  $effect(() => {
    const target = sentinel;
    if (!target || activeTab === "collections") return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting && visibleCount < currentItems.length) {
          visibleCount = visibleCount + PAGE_SIZE;
        }
      },
      { rootMargin: "200px" },
    );
    observer.observe(target);
    return () => observer.disconnect();
  });

  function episodeCount(s: SonarrSeries): number {
    return s.statistics?.episodeCount || 0;
  }

  function episodeFileCount(s: SonarrSeries): number {
    return s.statistics?.episodeFileCount || 0;
  }

  function seriesSize(s: SonarrSeries): number {
    return s.statistics?.sizeOnDisk || 0;
  }

  function inYearRange(year: number | undefined): boolean {
    const y = year || 0;
    if (fromYear && y < parseInt(fromYear)) return false;
    if (toYear && y > parseInt(toYear)) return false;
    return true;
  }

  const filteredSeries = $derived.by(() => {
    let list = series;
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      list = list.filter((s) => s.title.toLowerCase().includes(q));
    }
    if (filterStatus === "continuing") list = list.filter((s) => s.status === "continuing");
    if (filterStatus === "ended") list = list.filter((s) => s.status === "ended");
    if (filterStatus === "missing") {
      list = list.filter((s) => episodeCount(s) > episodeFileCount(s));
    }
    if (filterStatus === "complete") {
      list = list.filter(
        (s) => episodeCount(s) === episodeFileCount(s) && episodeCount(s) > 0,
      );
    }
    if (fromYear || toYear) list = list.filter((s) => inYearRange(s.year));

    if (sortBy === "title") list = [...list].sort((a, b) => a.title.localeCompare(b.title));
    if (sortBy === "missing") {
      list = [...list].sort(
        (a, b) =>
          episodeCount(b) - episodeFileCount(b) - (episodeCount(a) - episodeFileCount(a)),
      );
    }
    if (sortBy === "size") list = [...list].sort((a, b) => seriesSize(b) - seriesSize(a));
    if (sortBy === "year") list = [...list].sort((a, b) => (b.year || 0) - (a.year || 0));
    return list;
  });

  const filteredMovies = $derived.by(() => {
    let list = movies;
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      list = list.filter((m) => m.title.toLowerCase().includes(q));
    }
    if (filterStatus === "downloaded") list = list.filter((m) => m.hasFile === true);
    if (filterStatus === "missing") list = list.filter((m) => m.hasFile !== true);
    if (filterStatus === "monitored") list = list.filter((m) => m.monitored === true);
    if (fromYear || toYear) list = list.filter((m) => inYearRange(m.year));

    if (sortBy === "title") list = [...list].sort((a, b) => a.title.localeCompare(b.title));
    if (sortBy === "size") {
      list = [...list].sort((a, b) => (b.sizeOnDisk || 0) - (a.sizeOnDisk || 0));
    }
    if (sortBy === "year") list = [...list].sort((a, b) => (b.year || 0) - (a.year || 0));
    return list;
  });

  /** Movies grouped by their Radarr collection. */
  const collections = $derived.by(() => {
    const byId = new Map<number, LibraryCollection>();
    for (const movie of movies) {
      if (!movie.collection?.tmdbId) continue;
      const existing = byId.get(movie.collection.tmdbId);
      if (existing) {
        existing.movies.push(movie);
      } else {
        byId.set(movie.collection.tmdbId, {
          tmdbId: movie.collection.tmdbId,
          title:
            movie.collection.title ||
            movie.title ||
            `Collection ${movie.collection.tmdbId}`,
          movies: [movie],
        });
      }
    }
    return Array.from(byId.values()).sort((a, b) =>
      (a.title || "").localeCompare(b.title || ""),
    );
  });

  const seriesStats = $derived.by(() => {
    const totalEps = series.reduce((sum, s) => sum + episodeCount(s), 0);
    const fileEps = series.reduce((sum, s) => sum + episodeFileCount(s), 0);
    const totalSize = series.reduce((sum, s) => sum + seriesSize(s), 0);
    return {
      count: series.length,
      totalEps,
      fileEps,
      missing: totalEps - fileEps,
      totalSize,
    };
  });

  const movieStats = $derived.by(() => {
    const withFile = movies.filter((m) => m.hasFile === true).length;
    const totalSize = movies.reduce((sum, m) => sum + (m.sizeOnDisk || 0), 0);
    return {
      count: movies.length,
      withFile,
      missing: movies.length - withFile,
      totalSize,
    };
  });

  const currentItems = $derived<(SonarrSeries | RadarrMovie)[]>(
    activeTab === "movies" ? filteredMovies : filteredSeries,
  );
  const visibleItems = $derived(
    activeTab === "collections" ? [] : currentItems.slice(0, visibleCount),
  );
  const hasMoreItems = $derived(
    activeTab === "collections" ? false : visibleCount < currentItems.length,
  );

  // Reset the infinite-scroll window whenever the view or filters change
  $effect(() => {
    activeTab;
    searchQuery;
    filterStatus;
    sortBy;
    selectedGenres;
    fromYear;
    toYear;
    minRating;
    maxRating;
    visibleCount = PAGE_SIZE;
  });

  function handleMediaTypeChange(type: DiscoverMediaType) {
    activeTab = type === "movie" ? "movies" : type === "tv" ? "tv" : "collections";
  }

  function handleSortChange(value: string) {
    if (value === "release_date.desc") sortBy = "year";
    else if (value === "title.asc") sortBy = "title";
    else if (value === "size") sortBy = "size";
    else sortBy = "title";
  }

  let searchTimer: number | undefined;
  function handleSearchInput() {
    // Filtering is derived from searchQuery; the timer is a no-op debounce hook.
    clearTimeout(searchTimer);
    searchTimer = window.setTimeout(() => {}, 300);
  }

  function handleGenreToggle(genre: UnifiedGenre) {
    const index = selectedGenres.findIndex((g) => g.name === genre.name);
    if (index >= 0) {
      selectedGenres = selectedGenres.filter((_, i) => i !== index);
    } else {
      selectedGenres = [...selectedGenres, genre];
    }
  }

  function handleYearChange() {}

  function handleRatingChange() {}

  function handleClearAll() {
    selectedGenres = [];
    fromYear = "";
    toYear = "";
    minRating = 0;
    maxRating = 10;
    searchQuery = "";
    filterStatus = "all";
  }
</script>

<svelte:head>
  <title>Library - Flasharr</title>
</svelte:head>

<div class="library-page">
  <LibraryControlShell
    {activeTab}
    {filterMediaType}
    {collections}
    onMediaTypeChange={handleMediaTypeChange}
    onSortChange={handleSortChange}
    onSearchInput={handleSearchInput}
    onGenreToggle={handleGenreToggle}
    onYearChange={handleYearChange}
    onRatingChange={handleRatingChange}
    onClearAll={handleClearAll}
    bind:sortBy
    bind:searchQuery
    bind:selectedGenres
    bind:fromYear
    bind:toYear
    bind:minRating
    bind:maxRating
    bind:resolvedGenreIds
    bind:hasFilter
    bind:filterStatus
  />

  {#if !loading}
    <LibraryStatsStrip
      {activeTab}
      {seriesStats}
      {movieStats}
      collectionsCount={collections.length}
      collectionMovieCount={collections.reduce((sum, c) => sum + c.movies.length, 0)}
    />
  {/if}

  {#if loading}
    <div class="loading-state">
      <div class="pulse-ring"></div><span>SYNCING LIBRARY DATA...</span>
    </div>
  {:else if !hasLibrary}
    <div class="loading-state">
      <span class="material-icons" style="font-size: 48px; opacity: 0.15;">link_off</span><span
        >SONARR/RADARR NOT CONNECTED</span
      >
    </div>
  {:else if (activeTab === "tv" && seriesError) || ((activeTab === "movies" || activeTab === "collections") && moviesError)}
    <ErrorState
      title="Library data unavailable"
      message={(activeTab === "tv" ? seriesError : moviesError) ?? undefined}
      showRetry
      onRetry={loadLibrary}
    />
  {:else}
    <div class="content-wrapper">
      {#if activeTab === "collections"}
        <LibraryCollectionsGrid {collections} />
      {:else}
        <div class="media-grid">
          {#each visibleItems as item (item.id)}
            <MediaCard
              {item}
              type={activeTab === "tv" ? "series" : "movie"}
              {viewMode}
              disableTopStatusBar={activeTab === "tv"}
            />
          {:else}
            <div class="empty-state">
              <span class="material-icons">search_off</span><p>No items found</p>
            </div>
          {/each}
        </div>
      {/if}

      {#if hasMoreItems}
        <div class="scroll-sentinel" bind:this={sentinel}>
          <div class="pulse-ring"></div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .library-page {
    display: flex;
    flex-direction: column;
    min-height: 100%;
    overflow-y: auto;
    overflow-x: clip;
    width: 100%;
    max-width: 100%;
  }
  .library-page * {
    min-width: 0;
  }
  .scroll-sentinel {
    display: flex;
    justify-content: center;
    padding: var(--cui-space-lg);
  }
  .content-wrapper {
    padding: var(--cui-space-sm) var(--cui-space-md) var(--cui-space-lg);
  }
  .media-grid {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: var(--cui-space-md);
  }
  .loading-state,
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--cui-space-sm);
    min-height: 40vh;
    color: var(--cui-text-tertiary);
    text-align: center;
  }
  .pulse-ring {
    width: 24px;
    height: 24px;
    border-radius: var(--cui-radius-full);
    border: 2px solid color-mix(in srgb, var(--cui-score-elite) 18%, transparent);
    border-top-color: var(--cui-score-elite);
    animation: spin var(--cui-duration-slow) linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (max-width: 1024px) {
    .media-grid {
      grid-template-columns: repeat(4, minmax(0, 1fr));
    }
  }
  @media (max-width: 768px) {
    .library-page {
      padding-bottom: calc(var(--cui-mobile-bottom-nav-height) + var(--cui-mobile-safe-area-bottom));
    }
    .media-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: var(--cui-space-sm);
    }
    .content-wrapper {
      padding: var(--cui-space-sm);
    }
  }
</style>
