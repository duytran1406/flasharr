<script lang="ts">
  import { onMount } from "svelte";
  import {
    fetchAllSeries,
    fetchAllMovies,
    formatDiskSize,
    getSeriesPoster,
    getMoviePoster,
    type SonarrSeries,
    type RadarrMovie,
  } from "$lib/stores/arr";
  import { integrations } from "$lib/stores/settings";

  import MediaCard from "$lib/components/MediaCard.svelte";

  // State
  let activeTab: "series" | "movies" | "collections" = $state("series");
  let viewMode: "grid" | "list" = $state("grid");
  let searchQuery = $state("");
  let sortBy = $state("title");
  let filterStatus = $state("all");

  // Infinite scroll: reveal items progressively, 60 at a time
  let displayCount = $state(60);
  const ITEMS_STEP = 60;
  let sentinel: HTMLElement | undefined = $state();

  // Data
  let series = $state<SonarrSeries[]>([]);
  let movies = $state<RadarrMovie[]>([]);
  let loading = $state(true);

  let hasSonarr = $derived($integrations.sonarr_enabled);
  let hasRadarr = $derived($integrations.radarr_enabled);
  let hasAnyArr = $derived(hasSonarr || hasRadarr);

  onMount(async () => {
    // Default tab based on whats enabled
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

    if (!hasAnyArr) {
      loading = false;
      return;
    }

    try {
      const fetchSeries = hasSonarr ? fetchAllSeries() : Promise.resolve([]);
      const fetchMovies = hasRadarr ? fetchAllMovies() : Promise.resolve([]);

      const [s, m] = await Promise.all([fetchSeries, fetchMovies]);
      series = s;
      movies = m;
    } catch (e) {
      console.error("Failed to load library:", e);
    } finally {
      loading = false;
    }
  });

  // Infinite scroll sentinel — must be top-level $effect, NOT inside onMount
  // (Svelte 5 throws effect_orphan if $effect is called inside a callback)
  $effect(() => {
    const el = sentinel;
    if (!el) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting) {
          const total =
            activeTab === "series"
              ? filteredSeries().length
              : activeTab === "movies"
                ? filteredMovies().length
                : 0; // collections tab doesn't paginate
          if (displayCount < total) displayCount += ITEMS_STEP;
        }
      },
      { rootMargin: "200px" },
    );
    observer.observe(el);
    return () => observer.disconnect();
  });

  // Helpers to read stats from nested statistics
  function getEpCount(s: SonarrSeries): number {
    return s.statistics?.episodeCount || 0;
  }
  function getEpFileCount(s: SonarrSeries): number {
    return s.statistics?.episodeFileCount || 0;
  }
  function getSeriesSize(s: SonarrSeries): number {
    return s.statistics?.sizeOnDisk || 0;
  }
  function getSeasonCount(s: SonarrSeries): number {
    return s.statistics?.seasonCount || 0;
  }

  // Filtered & sorted series
  let filteredSeries = $derived(() => {
    let result = series;

    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      result = result.filter((s) => s.title.toLowerCase().includes(q));
    }

    if (filterStatus === "continuing")
      result = result.filter((s) => s.status === "continuing");
    if (filterStatus === "ended")
      result = result.filter((s) => s.status === "ended");
    if (filterStatus === "missing")
      result = result.filter((s) => getEpCount(s) > getEpFileCount(s));
    if (filterStatus === "complete")
      result = result.filter(
        (s) => getEpCount(s) === getEpFileCount(s) && getEpCount(s) > 0,
      );

    if (sortBy === "title")
      result = [...result].sort((a, b) => a.title.localeCompare(b.title));
    if (sortBy === "missing")
      result = [...result].sort(
        (a, b) =>
          getEpCount(b) -
          getEpFileCount(b) -
          (getEpCount(a) - getEpFileCount(a)),
      );
    if (sortBy === "size")
      result = [...result].sort((a, b) => getSeriesSize(b) - getSeriesSize(a));
    if (sortBy === "year")
      result = [...result].sort((a, b) => (b.year || 0) - (a.year || 0));

    return result;
  });

  // Filtered & sorted movies
  let filteredMovies = $derived(() => {
    let result = movies;

    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      result = result.filter((m) => m.title.toLowerCase().includes(q));
    }

    if (filterStatus === "downloaded")
      result = result.filter((m) => m.hasFile === true);
    if (filterStatus === "missing")
      result = result.filter((m) => m.hasFile !== true);
    if (filterStatus === "monitored")
      result = result.filter((m) => m.monitored === true);

    if (sortBy === "title")
      result = [...result].sort((a, b) => a.title.localeCompare(b.title));
    if (sortBy === "size")
      result = [...result].sort(
        (a, b) => (b.sizeOnDisk || 0) - (a.sizeOnDisk || 0),
      );
    if (sortBy === "year")
      result = [...result].sort((a, b) => (b.year || 0) - (a.year || 0));

    return result;
  });

  // Stats
  let seriesStats = $derived(() => {
    const totalEps = series.reduce((sum, s) => sum + getEpCount(s), 0);
    const fileEps = series.reduce((sum, s) => sum + getEpFileCount(s), 0);
    const totalSize = series.reduce((sum, s) => sum + getSeriesSize(s), 0);
    return {
      count: series.length,
      totalEps,
      fileEps,
      missing: totalEps - fileEps,
      totalSize,
    };
  });

  let movieStats = $derived(() => {
    const withFile = movies.filter((m) => m.hasFile === true).length;
    const totalSize = movies.reduce((sum, m) => sum + (m.sizeOnDisk || 0), 0);
    return {
      count: movies.length,
      withFile,
      missing: movies.length - withFile,
      totalSize,
    };
  });

  function getEpisodeProgress(s: SonarrSeries): number {
    const total = getEpCount(s);
    if (total === 0) return 0;
    return Math.round((getEpFileCount(s) / total) * 100);
  }

  function getStatusColor(status?: string): string {
    if (status === "continuing") return "#34d399";
    if (status === "ended") return "#94a3b8";
    if (status === "upcoming") return "#fbbf24";
    return "#64748b";
  }
  // Collections: group movies by collection.tmdbId (derived, no extra fetch)
  interface LibraryCollection {
    tmdbId: number;
    title: string;
    movies: RadarrMovie[];
  }
  let collections = $derived(() => {
    const map = new Map<number, LibraryCollection>();
    for (const m of movies) {
      if (!m.collection?.tmdbId) continue;
      const ex = map.get(m.collection.tmdbId);
      if (ex) {
        ex.movies.push(m);
      } else {
        map.set(m.collection.tmdbId, {
          tmdbId: m.collection.tmdbId,
          title: m.collection.title,
          movies: [m],
        });
      }
    }
    return Array.from(map.values()).sort((a, b) =>
      a.title.localeCompare(b.title),
    );
  });

  // Progressively-visible items (infinite scroll) — only for series/movies tabs
  let visibleItems = $derived(() => {
    const list = activeTab === "series" ? filteredSeries() : filteredMovies();
    return list.slice(0, displayCount);
  });

  let hasMoreItems = $derived(() => {
    if (activeTab === "collections") return false;
    const list = activeTab === "series" ? filteredSeries() : filteredMovies();
    return displayCount < list.length;
  });

  // Reset display count on filter change
  $effect(() => {
    activeTab;
    searchQuery;
    filterStatus;
    sortBy;
    displayCount = ITEMS_STEP;
  });
</script>

<svelte:head>
  <title>Library - Flasharr</title>
</svelte:head>

<div class="library-page">
  <!-- Tab Bar -->
  <div class="tab-bar">
    <div class="tabs">
      {#if hasSonarr}
        <button
          class="tab"
          class:active={activeTab === "series"}
          onclick={() => {
            activeTab = "series";
          }}
        >
          <span class="material-icons">live_tv</span>
          <span>Series</span>
          {#if !loading}<span class="tab-count">{series.length}</span>{/if}
        </button>
      {/if}
      {#if hasRadarr}
        <button
          class="tab"
          class:active={activeTab === "movies"}
          onclick={() => {
            activeTab = "movies";
          }}
        >
          <span class="material-icons">movie</span>
          <span>Movies</span>
          {#if !loading}<span class="tab-count">{movies.length}</span>{/if}
        </button>
      {/if}
      {#if hasRadarr && !loading && collections().length > 0}
        <button
          class="tab"
          class:active={activeTab === "collections"}
          onclick={() => {
            activeTab = "collections";
          }}
        >
          <span class="material-icons">collections_bookmark</span>
          <span>Collections</span>
          <span class="tab-count">{collections().length}</span>
        </button>
      {/if}
    </div>

    {#if activeTab !== "collections"}
      <div class="toolbar">
        <div class="search-box">
          <span class="material-icons">search</span>
          <input
            type="text"
            bind:value={searchQuery}
            placeholder="Search library..."
          />
        </div>

        <select class="sort-select" bind:value={sortBy}>
          <option value="title">Name</option>
          <option value="year">Year</option>
          <option value="size">Size</option>
          {#if activeTab === "series"}<option value="missing">Missing</option
            >{/if}
        </select>

        <select class="sort-select" bind:value={filterStatus}>
          <option value="all">All</option>
          {#if activeTab === "series"}
            <option value="continuing">Continuing</option>
            <option value="ended">Ended</option>
            <option value="missing">Has Missing</option>
            <option value="complete">Complete</option>
          {:else}
            <option value="downloaded">Downloaded</option>
            <option value="missing">Missing File</option>
            <option value="monitored">Monitored</option>
          {/if}
        </select>

        <div class="view-toggle">
          <button
            class:active={viewMode === "grid"}
            onclick={() => (viewMode = "grid")}
            title="Grid view"
          >
            <span class="material-icons">grid_view</span>
          </button>
          <button
            class:active={viewMode === "list"}
            onclick={() => (viewMode = "list")}
            title="List view"
          >
            <span class="material-icons">view_list</span>
          </button>
        </div>
      </div>
    {/if}
  </div>

  <!-- Stats Strip -->
  {#if !loading}
    <div class="stats-strip">
      {#if activeTab === "series"}
        <div class="stat-chip">
          <span class="sl">SERIES</span><span class="sv"
            >{seriesStats().count}</span
          >
        </div>
        <div class="stat-chip">
          <span class="sl">EPISODES</span><span class="sv"
            >{seriesStats().fileEps}/{seriesStats().totalEps}</span
          >
        </div>
        <div class="stat-chip warn">
          <span class="sl">MISSING</span><span class="sv"
            >{seriesStats().missing}</span
          >
        </div>
        <div class="stat-chip">
          <span class="sl">SIZE</span><span class="sv"
            >{formatDiskSize(seriesStats().totalSize)}</span
          >
        </div>
      {:else}
        <div class="stat-chip">
          <span class="sl">MOVIES</span><span class="sv"
            >{movieStats().count}</span
          >
        </div>
        <div class="stat-chip">
          <span class="sl">DOWNLOADED</span><span class="sv"
            >{movieStats().withFile}</span
          >
        </div>
        <div class="stat-chip warn">
          <span class="sl">MISSING</span><span class="sv"
            >{movieStats().missing}</span
          >
        </div>
        <div class="stat-chip">
          <span class="sl">SIZE</span><span class="sv"
            >{formatDiskSize(movieStats().totalSize)}</span
          >
        </div>
      {/if}
    </div>
  {/if}

  <!-- Content -->
  {#if loading}
    <div class="loading-state">
      <div class="pulse-ring"></div>
      <span>SYNCING LIBRARY DATA...</span>
    </div>
  {:else if !hasAnyArr}
    <div class="loading-state">
      <span class="material-icons" style="font-size: 48px; opacity: 0.15;"
        >link_off</span
      >
      <span>SONARR/RADARR NOT CONNECTED</span>
    </div>
  {:else}
    <div class="content-wrapper">
      {#if activeTab === "collections"}
        <!-- Collections Grid -->
        <div class="col-lib-grid">
          {#each collections() as col (col.tmdbId)}
            <a class="col-lib-card" href="/collection/{col.tmdbId}">
              <div class="col-lib-mosaic">
                {#each col.movies.slice(0, 4) as m}
                  <div class="col-lib-tile">
                    {#if getMoviePoster(m)}
                      <img
                        src={getMoviePoster(m)}
                        alt={m.title}
                        loading="lazy"
                      />
                    {:else}
                      <div class="col-lib-tile-blank">
                        <span class="material-icons">movie</span>
                      </div>
                    {/if}
                  </div>
                {/each}
                {#if col.movies.length < 4}
                  {#each Array(4 - col.movies.length) as _}
                    <div class="col-lib-tile col-lib-tile-blank">
                      <span class="material-icons">add</span>
                    </div>
                  {/each}
                {/if}
              </div>
              <div class="col-lib-info">
                <span class="col-lib-title">{col.title}</span>
                <span class="col-lib-count">{col.movies.length} in library</span
                >
              </div>
            </a>
          {:else}
            <div class="empty-state">
              <span class="material-icons">collections_bookmark</span>
              <p>No collections found</p>
            </div>
          {/each}
        </div>
      {:else}
        <!-- Series / Movies Grid -->
        <div class="media-grid" class:list-view={viewMode === "list"}>
          {#each visibleItems() as item (item.id)}
            <MediaCard
              {item}
              type={activeTab === "series" ? "series" : "movie"}
              {viewMode}
            />
          {:else}
            <div class="empty-state">
              <span class="material-icons">search_off</span>
              <p>No items found</p>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Infinite scroll sentinel (only for series/movies) -->
      {#if hasMoreItems()}
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
  }

  .tab-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    flex-shrink: 0;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .tabs {
    display: flex;
    gap: 2px;
  }

  .scroll-sentinel {
    display: flex;
    justify-content: center;
    padding: 1.5rem;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 1.25rem;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    color: var(--text-muted);
    font-size: 0.7rem;
    font-weight: 800;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    cursor: pointer;
    transition: all 0.2s;
    font-family: var(--font-mono, monospace);
  }

  .tab .material-icons {
    font-size: 1rem;
  }

  .tab.active {
    background: rgba(167, 139, 250, 0.1);
    border-color: rgba(167, 139, 250, 0.3);
    color: #a78bfa;
  }

  .tab:hover:not(.active) {
    background: rgba(255, 255, 255, 0.04);
    color: #fff;
  }

  .tab-count {
    background: rgba(255, 255, 255, 0.08);
    padding: 1px 6px;
    border-radius: 3px;
    font-size: 0.6rem;
  }

  .tab.active .tab-count {
    background: rgba(167, 139, 250, 0.2);
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .search-box {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: linear-gradient(
      0deg,
      rgba(168, 85, 247, 0.02) 0%,
      rgba(255, 255, 255, 0.03) 40%
    );
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-bottom: 1px solid rgba(168, 85, 247, 0.4);
    padding: 0.4rem 0.75rem;
    border-radius: 3px;
    position: relative;
    overflow: hidden;
  }

  /* Novu bold bottom-glow bleed from search box */
  .search-box::after {
    content: "";
    position: absolute;
    bottom: -20px;
    left: 5%;
    right: 5%;
    height: 60px;
    background: radial-gradient(
      ellipse at 50% 100%,
      rgba(168, 85, 247, 0.45) 0%,
      transparent 65%
    );
    opacity: 0.15;
    pointer-events: none;
    z-index: 0;
  }

  .search-box .material-icons {
    font-size: 1rem;
    color: var(--text-muted);
    opacity: 0.5;
  }

  .search-box input {
    background: transparent;
    border: none;
    color: #fff;
    font-size: 0.7rem;
    font-family: var(--font-mono, monospace);
    outline: none;
    width: 150px;
  }

  .search-box input::placeholder {
    color: var(--text-muted);
    opacity: 0.5;
  }

  .sort-select {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    color: var(--text-muted);
    font-size: 0.65rem;
    font-family: var(--font-mono, monospace);
    padding: 0.45rem 0.5rem;
    border-radius: 3px;
    cursor: pointer;
    outline: none;
  }

  .sort-select option {
    background: #0a0f18;
    color: #fff;
  }

  .view-toggle {
    display: flex;
    gap: 2px;
  }

  .view-toggle button {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    color: var(--text-muted);
    padding: 0.35rem;
    cursor: pointer;
    transition: all 0.2s;
  }

  .view-toggle button.active {
    color: var(--color-primary);
    border-color: rgba(0, 243, 255, 0.3);
    background: rgba(0, 243, 255, 0.05);
  }

  .view-toggle button .material-icons {
    font-size: 1rem;
  }

  .stats-strip {
    display: flex;
    gap: 0.75rem;
    padding: 0.5rem 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
    flex-shrink: 0;
  }

  .stat-chip {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.6rem;
    font-weight: 800;
    letter-spacing: 0.08em;
  }

  .stat-chip .sl {
    color: var(--text-muted);
    opacity: 0.5;
  }
  .stat-chip .sv {
    color: var(--color-primary);
    font-family: var(--font-mono, monospace);
  }
  .stat-chip.warn .sv {
    color: #fbbf24;
  }

  .media-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 1.25rem;
    padding: 1.5rem;
    overflow-y: auto;
    flex: 1;
  }

  .media-grid.list-view {
    grid-template-columns: 1fr;
    gap: 0.5rem;
  }

  .loading-state,
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1rem;
    padding: 4rem;
    color: var(--text-muted);
    flex: 1;
  }

  .loading-state span,
  .empty-state p {
    font-size: 0.65rem;
    font-weight: 800;
    letter-spacing: 0.15em;
    opacity: 0.5;
  }

  .empty-state .material-icons {
    font-size: 48px;
    opacity: 0.2;
  }

  .pulse-ring {
    width: 28px;
    height: 28px;
    border: 2px solid rgba(167, 139, 250, 0.3);
    border-top-color: #a78bfa;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .media-grid::-webkit-scrollbar {
    width: 4px;
  }
  .media-grid::-webkit-scrollbar-track {
    background: transparent;
  }
  .media-grid::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 2px;
  }

  @media (max-width: 768px) {
    .tab-bar {
      flex-direction: column;
      gap: 0.75rem;
    }
    .toolbar {
      flex-wrap: wrap;
      width: 100%;
    }
    .search-box {
      flex: 1;
    }
    .search-box input {
      width: 100%;
    }
    .media-grid {
      grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
      gap: 0.75rem;
      padding: 1rem;
    }
  }
  /* Collections grid in Library */
  .col-lib-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 1.25rem;
    padding: 1.5rem;
  }

  .col-lib-card {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    cursor: pointer;
    transition: transform 0.2s;
    text-decoration: none;
    color: inherit;
  }

  .col-lib-card:hover {
    transform: translateY(-3px);
  }

  .col-lib-mosaic {
    display: grid;
    grid-template-columns: 1fr 1fr;
    grid-template-rows: 1fr 1fr;
    aspect-ratio: 1;
    border-radius: 6px;
    overflow: hidden;
    gap: 2px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .col-lib-tile {
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.03);
  }

  .col-lib-tile img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .col-lib-tile-blank {
    color: rgba(255, 255, 255, 0.1);
  }

  .col-lib-tile-blank .material-icons {
    font-size: 1.5rem;
  }

  .col-lib-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0 2px;
  }

  .col-lib-title {
    font-size: 0.72rem;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.85);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .col-lib-count {
    font-size: 0.6rem;
    font-weight: 600;
    color: var(--color-primary);
    font-family: var(--font-mono, monospace);
    letter-spacing: 0.05em;
    opacity: 0.7;
  }

  .scroll-sentinel {
    display: flex;
    justify-content: center;
    padding: 2rem;
  }
</style>
