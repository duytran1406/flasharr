<script lang="ts">
  import { page } from "$app/state";
  import {
    getCollectionDetails,
    getPosterUrl,
    getBackdropUrl,
    getYear,
    type TMDBCollection,
    type TMDBMovie,
  } from "$lib/services/tmdb";
  import { toasts } from "$lib/stores/toasts";
  import { ui, type SmartSearchDownloadResult } from "$lib/stores/ui.svelte";
  import { animeFly } from "$lib/animations";
  import { Button, Badge } from "@media-set/core-ui";
  import { fetchAllMovies, type RadarrMovie } from "$lib/stores/arr";
  import { onDownloadEvent } from "$lib/stores/downloads.svelte";

  type LibraryStatus = "blue" | "yellow" | "red" | "green" | "purple" | "gray";

  const collectionId = $derived(page.params.id as string);

  let collection = $state<TMDBCollection | null>(null);
  let loading = $state(true);
  let backdropLoaded = $state(false);

  // Library data
  let allMovies = $state<RadarrMovie[]>([]);
  let addingIds = $state<Set<number>>(new Set());
  /** Parts with an active (not yet completed) download. */
  let downloadingIds = $state<Set<number>>(new Set());
  /** Parts whose last smart-search download attempt failed. */
  let failedIds = $state<Set<number>>(new Set());

  /** Re-check whether a part has an active download task. */
  async function refreshDownloadState(tmdbId: number) {
    try {
      const res = await fetch(`/api/media/${tmdbId}/downloads`);
      if (!res.ok) return;
      const data = await res.json();
      const groups = data?.episodes ?? {};
      const active = Object.values(groups).some((tasks) =>
        Array.isArray(tasks)
          ? tasks.some((task) => {
              const state = String(task?.state || "").toUpperCase();
              return state && state !== "COMPLETED";
            })
          : false,
      );
      if (active) {
        downloadingIds = new Set([...downloadingIds, tmdbId]);
      } else {
        downloadingIds = new Set([...downloadingIds].filter((id) => id !== tmdbId));
      }
    } catch {
      // Best-effort
    }
  }

  async function refreshAllDownloadStates() {
    if (!collection?.parts?.length) return;
    await Promise.all(collection.parts.map((part) => refreshDownloadState(part.id)));
  }

  function markDownloading(tmdbId: number) {
    downloadingIds = new Set([...downloadingIds, tmdbId]);
    failedIds = new Set([...failedIds].filter((id) => id !== tmdbId));
  }

  function markFailed(tmdbId: number) {
    failedIds = new Set([...failedIds, tmdbId]);
    downloadingIds = new Set([...downloadingIds].filter((id) => id !== tmdbId));
  }

  function clearFailed(tmdbId: number) {
    failedIds = new Set([...failedIds].filter((id) => id !== tmdbId));
  }

  function handleDownloadResult(result: SmartSearchDownloadResult) {
    if (!result?.tmdbId) return;
    if (result.success) {
      markDownloading(result.tmdbId);
      refreshDownloadState(result.tmdbId);
    } else {
      markFailed(result.tmdbId);
    }
  }

  function openSmartSearch(movie: TMDBMovie) {
    ui.openSmartSearch(
      {
        tmdbId: String(movie.id),
        type: "movie",
        title: movie.title,
        year: getYear(movie.release_date) || undefined,
      },
      {
        context: { source: "collection", tmdbId: movie.id },
        onDownloadResult: handleDownloadResult,
      },
    );
  }

  function isDownloading(tmdbId: number): boolean {
    return downloadingIds.has(tmdbId);
  }

  function isFailed(tmdbId: number): boolean {
    return failedIds.has(tmdbId);
  }

  function isNotInLibrary(tmdbId: number): boolean {
    return libraryMovie(tmdbId) === undefined;
  }

  function libraryStatus(tmdbId: number, releaseDate?: string): LibraryStatus {
    if (isDownloading(tmdbId)) return "purple";
    if (isFailed(tmdbId)) return "red";
    const movie = libraryMovie(tmdbId);
    if (!movie) return "gray";
    const unreleased = releaseDate
      ? new Date(releaseDate).getTime() > Date.now()
      : false;
    const monitored = movie.monitored === true;
    const hasFile = movie.hasFile === true;
    if (monitored && unreleased) return "blue";
    if (!hasFile && !monitored) return "yellow";
    if (!hasFile && monitored) return "red";
    if (hasFile && monitored) return "green";
    return "yellow";
  }

  function hasKnownStatus(tmdbId: number): boolean {
    return (
      isDownloading(tmdbId) ||
      isFailed(tmdbId) ||
      isNotInLibrary(tmdbId) ||
      libraryMovie(tmdbId) !== undefined
    );
  }

  function statusColor(status: LibraryStatus): string {
    switch (status) {
      case "blue":
        return "#3b82f6";
      case "yellow":
        return "#f59e0b";
      case "red":
        return "#ef4444";
      case "green":
        return "#22c55e";
      case "purple":
        return "#a855f7";
      default:
        return "#6b7280";
    }
  }

  function statusLabel(status: LibraryStatus): string {
    switch (status) {
      case "blue":
        return "MONITORED · UNRELEASED";
      case "yellow":
        return "MISSING · UNMONITORED";
      case "red":
        return "MISSING · MONITORED";
      case "green":
        return "DOWNLOADED · MONITORED";
      case "purple":
        return "DOWNLOADING";
      default:
        return "NOT IN LIBRARY";
    }
  }

  function libraryStatusColor(tmdbId: number, releaseDate?: string): string {
    return statusColor(libraryStatus(tmdbId, releaseDate));
  }

  function libraryStatusLabel(tmdbId: number, releaseDate?: string): string {
    return statusLabel(libraryStatus(tmdbId, releaseDate));
  }

  function showStatusBar(tmdbId: number): boolean {
    return hasKnownStatus(tmdbId);
  }

  // Derived reactive Set — Svelte tracks this properly so UI updates instantly
  let libraryTmdbIds = $derived(new Set(allMovies.map((m) => m.tmdbId)));
  let libraryByTmdbId = $derived(new Map(allMovies.map((m) => [m.tmdbId, m])));

  function libraryMovie(tmdbId: number): RadarrMovie | undefined {
    return libraryByTmdbId.get(tmdbId);
  }

  function isInLibrary(tmdbId: number): boolean {
    return libraryTmdbIds.has(tmdbId);
  }

  async function loadData() {
    loading = true;
    try {
      const [details, movies] = await Promise.all([
        getCollectionDetails(collectionId),
        fetchAllMovies(),
      ]);
      collection = details;
      allMovies = movies;
      await refreshAllDownloadStates();
    } catch (error) {
      console.error("Failed to load collection data:", error);
      toasts.error("Failed to load collection intelligence brief");
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (collectionId) {
      backdropLoaded = false;
      loadData();
    }
  });

  // Live download state for the parts of this collection
  $effect(() => {
    const offUpdated = onDownloadEvent("TASK_UPDATED", (event) => {
      const task: any = event.detail;
      const tmdbId = Number(task?.tmdb_id ?? task?.tmdbId);
      if (!tmdbId || !collection?.parts?.some((p) => p.id === tmdbId)) return;
      const state = String(task?.state || "").toUpperCase();
      if (state && state !== "COMPLETED") markDownloading(tmdbId);
      refreshDownloadState(tmdbId);
    });
    const offCompleted = onDownloadEvent("TASK_COMPLETED", (event) => {
      const task: any = event.detail;
      const tmdbId = Number(task?.tmdb_id ?? task?.tmdbId);
      if (!tmdbId || !collection?.parts?.some((p) => p.id === tmdbId)) return;
      downloadingIds = new Set([...downloadingIds].filter((id) => id !== tmdbId));
      clearFailed(tmdbId);
      refreshDownloadState(tmdbId);
      loadData();
    });
    return () => {
      offUpdated();
      offCompleted();
    };
  });

  let stats = $derived.by(() => {
    if (!collection?.parts) return null;
    const parts = collection.parts;
    const ownedCount = parts.filter((p) => libraryTmdbIds.has(p.id)).length;
    return {
      total_parts: parts.length,
      owned: ownedCount,
      avg_score:
        parts.reduce((acc, p) => acc + p.vote_average, 0) / parts.length,
      total_votes: parts.reduce((acc, p) => acc + p.vote_count, 0),
    };
  });

  async function handleAddToLibrary(movie: TMDBMovie) {
    if (addingIds.has(movie.id)) return;
    addingIds = new Set([...addingIds, movie.id]);
    try {
      const resp = await fetch("/api/arr/movies/add", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ tmdb_id: movie.id }),
      });
      if (resp.ok) {
        const data = await resp.json();
        toasts.success(`"${movie.title}" added to Radarr library`);
        allMovies = [
          ...allMovies,
          {
            id: data.arr_id,
            title: movie.title,
            tmdbId: movie.id,
          } as RadarrMovie,
        ];
      } else if (resp.status === 409) {
        toasts.info(`"${movie.title}" is already in your Radarr library`);
      } else {
        const text = await resp.text();
        toasts.error(`Failed to add: ${text}`);
      }
    } catch {
      toasts.error("Network error — could not reach server");
    } finally {
      addingIds = new Set([...addingIds].filter((id) => id !== movie.id));
    }
  }

  function handleSearchClick(event: MouseEvent, movie: TMDBMovie) {
    event.preventDefault();
    event.stopPropagation();
    // Defer so the click does not reach the freshly opened modal
    queueMicrotask(() => {
      clearFailed(movie.id);
      openSmartSearch(movie);
    });
  }

  function handleAddClick(event: MouseEvent, movie: TMDBMovie) {
    event.preventDefault();
    event.stopPropagation();
    handleAddToLibrary(movie);
  }

  function movieHref(tmdbId: number): string {
    return `/movie/${tmdbId}`;
  }
</script>

<div class="collection-view">
  <div class="view-container">
    <button class="detail-back" type="button" onclick={() => window.history.back()}>
      <span class="material-icons" aria-hidden="true">arrow_back</span>
      Back
    </button>

    <!-- Hero Section -->
    <header
      class="collection-hero"
      class:loading
      style={`background-image: url(/images/placeholder-banner.png)`}
    >
      {#if collection?.backdrop_path}
        <div
          class="backdrop-real"
          class:loaded={backdropLoaded}
          style={`background-image: url(${getBackdropUrl(collection.backdrop_path, "original")})`}
        ></div>
        <img
          src={getBackdropUrl(collection.backdrop_path, "original")}
          alt=""
          class="hidden-loader"
          onload={() => (backdropLoaded = true)}
        />
      {/if}

      <div class="hero-overlay"></div>

      <div class="hero-content">
        <div class="intelligence-tag">COLLECTION_INTEL</div>
        <h1 class="collection-title">
          {#if loading}
            <div class="skeleton-title"></div>
          {:else}
            {collection?.name || "Unknown Sequence"}
          {/if}
        </h1>

        {#if stats}
          <div class="hero-stats">
            <div class="hero-stat">
              <span class="stat-label">TOTAL_ITEMS</span>
              <span class="stat-value">{stats.total_parts}</span>
            </div>
            <div class="hero-stat">
              <span class="stat-label">IN_LIBRARY</span>
              <span class="stat-value owned"
                >{stats.owned}/{stats.total_parts}</span
              >
            </div>
            <div class="hero-stat">
              <span class="stat-label">AVG_SCORE</span>
              <span class="stat-value">{stats.avg_score.toFixed(1)}</span>
            </div>
          </div>
        {/if}
      </div>
    </header>

    <!-- Content Area -->
    <div class="collection-grid-container">
      <section class="mobile-telemetry" aria-label="Collection telemetry">
        <span>{collection?.parts.length || 0} items</span>
        {#if stats}
          <span>{stats.owned}/{stats.total_parts} in library</span>
        {/if}
      </section>

      <div class="main-column">
        <!-- Overview -->
        <section class="overview-section glass-panel">
          <div class="section-badge-wrapper"><Badge variant="success">BRIEFING</Badge></div>
          <h3 class="section-label">Operational Overview</h3>
          {#if loading}
            <div class="skeleton-text"></div>
            <div class="skeleton-text"></div>
            <div class="skeleton-text" style="width: 60%"></div>
          {:else}
            <p class="overview-text">
              {collection?.overview ||
                "No intelligence report found for this collection fragment."}
            </p>
          {/if}
        </section>

        <!-- Parts Grid — Search Card Layout -->
        <section class="parts-section">
          <div class="section-header">
            <h3 class="section-label">Asset Sequence</h3>
            <span class="asset-count"
              >{collection?.parts.length || 0} ITEMS DETECTED</span
            >
          </div>

          <div class="collection-cards-grid">
            {#if loading}
              {#each Array(6) as _}
                <div class="card-skeleton"></div>
              {/each}
            {:else if collection?.parts}
              {#each [...collection.parts].sort((a, b) => new Date(a.release_date || 0).getTime() - new Date(b.release_date || 0).getTime()) as movie}
                <article
                  class="col-card"
                  class:owned={isInLibrary(movie.id)}
                  in:animeFly={{ y: 20, delay: 100 }}
                >
                  <div class="col-card-inner">
                    {#if showStatusBar(movie.id)}
                      <div
                        class="library-status-bar"
                        style={`background:${libraryStatusColor(movie.id, movie.release_date)}`}
                        title={libraryStatusLabel(movie.id, movie.release_date)}
                      ></div>
                    {/if}
                    <img
                      src={movie.poster_path
                        ? getPosterUrl(movie.poster_path, "w342")
                        : "/images/placeholder-poster.svg"}
                      alt={movie.title}
                      loading="lazy"
                    />
                    <div class="card-shine"></div>

                    <!-- Year Badge (Top Right) -->
                    {#if movie.release_date}
                      <div class="year-badge-wrapper">
                        <Badge text={getYear(movie.release_date)?.toString()} variant="default" size="xs" noDot />
                      </div>
                    {/if}

                    <!-- Card Overlay -->
                    <div class="card-overlay">
                      <div class="overlay-top">
                        <h3 class="card-title">
                          <a href={movieHref(movie.id)}>{movie.title}</a>
                        </h3>
                        <div class="card-meta">
                          {#if movie.vote_average > 0}
                            <span class="meta-rating">
                              <span
                                class="material-icons"
                                style="font-size:0.75rem;vertical-align:middle;color:#f59e0b"
                                >star</span
                              >
                              {movie.vote_average.toFixed(1)}
                            </span>
                          {/if}
                        </div>
                      </div>

                      <div class="overlay-bottom">
                        <div class="card-actions">
                          <button
                            type="button"
                            class="collection-search-btn"
                            onclick={(e) => handleSearchClick(e, movie)}
                          >
                            <span class="material-icons" style="font-size: 14px;">manage_search</span>
                            Search
                          </button>
                          {#if !libraryTmdbIds.has(movie.id)}
                            <Button
                              size="sm"
                              variant="ghost"
                              icon={addingIds.has(movie.id)
                                ? "sync"
                                : "library_add"}
                              disabled={addingIds.has(movie.id)}
                              onclick={(e: MouseEvent) => handleAddClick(e, movie)}
                              title="Add to Radarr"
                            ></Button>
                          {/if}
                        </div>
                      </div>
                    </div>
                  </div>
                </article>
              {/each}
            {/if}
          </div>
        </section>
      </div>

      <aside class="sidebar-column">
        <div class="telemetry-panel glass-panel">
          <div class="section-badge">TELEMETRY</div>
          <div class="telemetry-rows">
            <div class="telemetry-row">
              <span class="tel-label">OBJECT_ID</span>
              <span class="tel-value">#{collectionId}</span>
            </div>
            <div class="telemetry-row">
              <span class="tel-label">ACCESS_LEVEL</span>
              <span class="tel-value terminal-green">COMMANDER</span>
            </div>
            {#if stats}
              <div class="telemetry-row">
                <span class="tel-label">LIBRARY_STATUS</span>
                <span
                  class="tel-value"
                  class:terminal-green={stats.owned === stats.total_parts}
                  class:terminal-amber={stats.owned > 0 &&
                    stats.owned < stats.total_parts}
                  class:terminal-red={stats.owned === 0}
                >
                  {stats.owned === stats.total_parts
                    ? "COMPLETE"
                    : `${stats.owned}/${stats.total_parts}`}
                </span>
              </div>
              <div class="telemetry-row">
                <span class="tel-label">TOTAL_VOTES</span>
                <span class="tel-value"
                  >{stats.total_votes.toLocaleString()}</span
                >
              </div>
            {/if}
          </div>

          <div class="holographic-wave"></div>
        </div>

        <div class="action-panel glass-panel">
          <button class="tactical-btn" onclick={() => window.history.back()}>
            <span class="material-icons">arrow_back</span>
            RETREAT_TO_PREVIOUS
          </button>
        </div>
      </aside>
    </div>
  </div>
</div>

<style>
  .collection-view {
    width: 100%;
    min-height: 100vh;
    background: #010203;
    color: #e2e8f0;
    padding-bottom: max(5rem, env(safe-area-inset-bottom));
  }
  .detail-back {
    position: relative;
    z-index: 4;
    display: inline-flex;
    align-items: center;
    gap: .4rem;
    min-height: 44px;
    margin: .75rem 4rem -3.75rem;
    padding: .5rem .75rem;
    border: 1px solid rgba(255, 255, 255, .16);
    border-radius: 8px;
    background: #0a0f19b8;
    color: #fff;
    font: 700 .75rem var(--font-mono,monospace);
    letter-spacing: .08em;
    text-transform: uppercase;
    cursor: pointer;
  }
  .detail-back:hover,
  .detail-back:focus-visible {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }
  .detail-back .material-icons {
    font-size: 1rem;
  }
  .view-container {
    max-width: 1440px;
    margin: 0 auto;
  }
  .collection-hero {
    height: 500px;
    position: relative;
    background-size: cover;
    background-position: center 20%;
    display: flex;
    align-items: flex-end;
    padding: 3rem 4rem;
    overflow: hidden;
  }
  .backdrop-real {
    position: absolute;
    inset: 0;
    background-size: cover;
    background-position: center 25%;
    opacity: 0;
    transition: opacity 1s cubic-bezier(.4, 0, .2, 1);
    transform: scale(1.05);
    animation: slowZoom 30s linear infinite alternate;
  }
  .backdrop-real.loaded {
    opacity: .7;
  }
  @keyframes slowZoom {
    0% {
      transform: scale(1.05);
    }
    to {
      transform: scale(1.15);
    }
  }
  .hero-overlay {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(
        to bottom,
        #01020366,
        #010203cc,
        #010203);
    z-index: 1;
  }
  .hero-content {
    position: relative;
    z-index: 2;
    width: 100%;
  }
  .intelligence-tag {
    font-family: var(--font-mono, monospace);
    font-size: .7rem;
    font-weight: 800;
    color: var(--color-primary, #00f3ff);
    letter-spacing: .4em;
    margin-bottom: 1rem;
    opacity: .8;
  }
  .collection-title {
    font-size: clamp(1.4rem, 4.5vw, 3rem);
    font-weight: 900;
    margin: 0;
    text-transform: uppercase;
    letter-spacing: -.02em;
    line-height: .95;
    background:
      linear-gradient(
        to bottom,
        #fff,
        #94a3b8);
    -webkit-background-clip: text;
    background-clip: text;
    -webkit-text-fill-color: transparent;
  }
  .hero-stats {
    display: flex;
    gap: 3rem;
    margin-top: 2rem;
  }
  .hero-stat {
    display: flex;
    flex-direction: column;
    gap: .25rem;
  }
  .stat-label {
    font-family: var(--font-mono, monospace);
    font-size: .6rem;
    font-weight: 900;
    color: #64748b;
    letter-spacing: .2em;
  }
  .stat-value {
    font-size: 1.5rem;
    font-weight: 900;
    color: var(--color-primary, #00f3ff);
    font-family: var(--font-heading, "Outfit", sans-serif);
  }
  .stat-value.owned {
    color: #00ff80;
    text-shadow: 0 0 12px rgba(0, 255, 128, .3);
  }
  .collection-grid-container {
    display: grid;
    grid-template-columns: 1fr 340px;
    gap: 2rem;
    padding: 0 4rem;
    margin-top: -2rem;
    position: relative;
    z-index: 3;
  }
  .mobile-telemetry {
    display: none;
  }
  .main-column {
    display: flex;
    flex-direction: column;
    gap: 3rem;
  }
  .glass-panel {
    background: #080a0fb3;
    border: 1px solid rgba(255, 255, 255, .08);
    -webkit-backdrop-filter: blur(20px);
    backdrop-filter: blur(20px);
    padding: 2rem;
    position: relative;
    clip-path: polygon(0% 0%, calc(100% - 20px) 0%, 100% 20px, 100% 100%, 20px 100%, 0% calc(100% - 20px));
  }
  .section-badge-wrapper {
    position: absolute;
    top: -12px;
    right: 2rem;
  }
  .section-label {
    font-family: var(--font-mono, monospace);
    font-size: .75rem;
    font-weight: 900;
    color: var(--color-primary, #00f3ff);
    text-transform: uppercase;
    letter-spacing: .25em;
    margin-bottom: 1.5rem;
    display: flex;
    align-items: center;
    gap: 1rem;
  }
  .section-label:after {
    content: "";
    flex: 1;
    height: 1px;
    background:
      linear-gradient(
        to right,
        rgba(0, 243, 255, .2),
        transparent);
  }
  .overview-text {
    font-size: 1rem;
    line-height: 1.8;
    color: #94a3b8;
    max-width: 900px;
  }
  .parts-section {
    margin-bottom: 2rem;
  }
  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 2rem;
  }
  .asset-count {
    font-family: var(--font-mono, monospace);
    font-size: .65rem;
    font-weight: 900;
    color: #64748b;
    letter-spacing: .1em;
  }
  .collection-cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 1rem;
    width: 100%;
    max-width: 100%;
  }
  @media (min-width: 1400px) {
    .collection-cards-grid {
      grid-template-columns: repeat(4, 1fr);
    }
  }
  .col-card {
    position: relative;
    aspect-ratio: 2/3;
    width: 100%;
    transition: transform .4s cubic-bezier(.175, .885, .32, 1.275);
    z-index: 1;
  }
  .col-card:hover {
    transform: scale(1.05) translateY(-5px);
    z-index: 10;
  }
  .col-card-inner {
    position: relative;
    width: 100%;
    height: 100%;
    border-radius: 16px;
    overflow: hidden;
    background: #0a0f1e;
    border: 1px solid rgba(255, 255, 255, .05);
    box-shadow: 0 10px 30px #0000004d;
    transition: all .4s ease;
  }
  .col-card:hover .col-card-inner {
    border-color: #00f3ff66;
    box-shadow: 0 20px 50px -10px #00f3ff40;
  }
  .col-card.owned .col-card-inner {
    border-color: #00ff8026;
  }
  .col-card.owned:hover .col-card-inner {
    border-color: #00ff8066;
    box-shadow: 0 20px 50px -10px #00ff8033;
  }
  .col-card-inner img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform .6s ease;
  }
  .col-card:hover img {
    transform: scale(1.1);
  }
  .library-status-bar {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 4px;
    z-index: 3;
    overflow: visible;
    box-shadow: 0 1px 6px #00000059;
  }
  .library-status-bar:before {
    content: "";
    position: absolute;
    inset: 0;
    background: inherit;
  }
  .library-status-bar:after {
    content: "";
    position: absolute;
    inset: -2px 0 0;
    background: inherit;
    opacity: .65;
    filter: blur(8px);
    z-index: -1;
  }
  .col-card:hover .library-status-bar:after {
    opacity: .85;
  }
  .col-card:hover .library-status-bar:before {
    filter: brightness(1.08);
  }
  .library-status-bar[style*="#3b82f6"] {
    background:
      linear-gradient(
        135deg,
        #3b82f6f2,
        #60a5fae6,
        #93c5fdd9) !important;
  }
  .library-status-bar[style*="#f59e0b"] {
    background:
      linear-gradient(
        135deg,
        #f59e0bf2,
        #fbbf24e6,
        #fde047d9) !important;
  }
  .library-status-bar[style*="#ef4444"] {
    background:
      linear-gradient(
        135deg,
        #ef4444f2,
        #f87171e6,
        #fca5a5d9) !important;
  }
  .library-status-bar[style*="#22c55e"] {
    background:
      linear-gradient(
        135deg,
        #22c55ef2,
        #4ade80e6,
        #86efacd9) !important;
  }
  .library-status-bar[style*="#a855f7"] {
    background:
      linear-gradient(
        135deg,
        #a855f7f2,
        #c084fce6,
        #d8b4fed9) !important;
  }
  .library-status-bar[style*="#6b7280"] {
    background:
      linear-gradient(
        135deg,
        #6b7280f2,
        #9ca3afe6,
        #d1d5dbd9) !important;
  }
  .library-status-bar[style*=transparent]:after {
    display: none;
  }
  .library-status-bar[style*=transparent]:before {
    display: none;
  }
  .card-shine {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(
        135deg,
        transparent 0%,
        rgba(255, 255, 255, .05) 50%,
        transparent 100%);
    transform: translate(-100%);
    transition: transform .6s ease;
    z-index: 1;
  }
  .col-card:hover .card-shine {
    transform: translate(100%);
  }
  .year-badge-wrapper {
    position: absolute;
    top: .75rem;
    right: .75rem;
    z-index: 5;
  }
  .card-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    padding: 1.25rem;
    background:
      linear-gradient(
        to top,
        rgba(10, 15, 30, .98) 0%,
        rgba(10, 15, 30, .7) 40%,
        transparent 100%);
    z-index: 2;
    transition: all .3s ease;
  }
  .col-card:hover .card-overlay {
    background:
      linear-gradient(
        to top,
        #0a0f1e,
        #0a0f1ecc 60%);
  }
  .overlay-top {
    transform: translateY(0);
    transition: transform .4s ease;
  }
  .col-card:hover .overlay-top {
    transform: translateY(-8px);
  }
  .card-title {
    margin: 0 0 .4rem;
    font-size: 1rem;
    font-weight: 800;
    color: #fff;
    line-height: 1.3;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .card-title a {
    color: inherit;
    text-decoration: none;
  }
  .card-title a:focus-visible {
    outline: 2px solid var(--color-primary);
    outline-offset: 3px;
  }
  .card-meta {
    display: flex;
    align-items: center;
    gap: .75rem;
    font-size: .8rem;
    color: #ffffffb3;
    margin-bottom: .5rem;
  }
  .meta-rating {
    display: flex;
    align-items: center;
    gap: 3px;
  }
  .overlay-bottom {
    max-height: 120px;
    opacity: 1;
    overflow: visible;
    margin-top: .5rem;
    transition: all .4s ease;
  }
  .col-card:hover .overlay-bottom {
    max-height: 120px;
    opacity: 1;
    margin-top: .5rem;
  }
  @media (max-width: 768px) {
    .overlay-bottom {
      max-height: 120px;
      opacity: 1;
    }
    .collection-grid-container {
      grid-template-columns: 1fr;
      gap: 1rem;
    }
    .sidebar-column,
    .telemetry-panel {
      display: none;
    }
    .collection-cards-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: .85rem;
    }
  }
  .card-actions {
    display: flex;
    gap: .5rem;
    align-items: center;
    pointer-events: auto;
  }
  .collection-search-btn {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: .35rem;
    padding: .45rem .8rem;
    font-family: var(--font-mono, monospace);
    font-size: .65rem;
    font-weight: 800;
    letter-spacing: .08em;
    text-transform: uppercase;
    color: #00f3ff;
    background: #0a1018b8;
    border: 1px solid rgba(0, 243, 255, .45);
    border-radius: 8px;
    cursor: pointer;
    pointer-events: auto;
    transition: all .2s ease;
  }
  .collection-search-btn:hover {
    color: #fff;
    border-color: #fff;
    box-shadow: 0 0 16px #00f3ff59;
  }
  .collection-search-btn:active {
    transform: translateY(1px);
  }
  .card-actions :global(.flasharr-btn) {
    pointer-events: auto !important;
  }
  .sidebar-column {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }
  .telemetry-rows {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .telemetry-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-bottom: .75rem;
    border-bottom: 1px solid rgba(255, 255, 255, .03);
  }
  .tel-label {
    font-family: var(--font-mono, monospace);
    font-size: .6rem;
    font-weight: 800;
    color: #64748b;
  }
  .tel-value {
    font-family: var(--font-mono, monospace);
    font-size: .8rem;
    font-weight: 700;
    color: #fff;
  }
  .terminal-green {
    color: #00ff80;
    text-shadow: 0 0 10px rgba(0, 255, 128, .3);
  }
  .terminal-amber {
    color: #fbbf24;
    text-shadow: 0 0 10px rgba(251, 191, 36, .3);
  }
  .terminal-red {
    color: #ff6464;
    text-shadow: 0 0 10px rgba(255, 100, 100, .3);
  }
  .tactical-btn {
    width: 100%;
    background: transparent;
    border: 1px solid rgba(255, 255, 255, .1);
    color: #fff;
    padding: 1rem;
    font-family: var(--font-mono, monospace);
    font-size: .7rem;
    font-weight: 800;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: .75rem;
    cursor: pointer;
    transition: all .3s;
    clip-path: polygon(8px 0%, 100% 0%, 100% calc(100% - 8px), calc(100% - 8px) 100%, 0% 100%, 0% 8px);
  }
  .tactical-btn:hover {
    background: #ffffff0d;
    border-color: var(--color-primary, #00f3ff);
    color: var(--color-primary, #00f3ff);
    transform: translateY(-2px);
  }
  .tactical-btn .material-icons {
    font-size: 1.2rem;
  }
  .skeleton-title {
    width: 60%;
    height: 4rem;
    background: #ffffff0d;
    border-radius: 8px;
    animation: shimmer 2s infinite linear;
  }
  .skeleton-text {
    width: 100%;
    height: 1rem;
    background: #ffffff0d;
    margin-bottom: .75rem;
    border-radius: 4px;
    animation: shimmer 2s infinite linear;
  }
  .card-skeleton {
    aspect-ratio: 2/3;
    border-radius: 16px;
    background: #ffffff08;
    animation: shimmer 2s infinite linear;
  }
  @keyframes shimmer {
    0% {
      opacity: .5;
    }
    50% {
      opacity: .8;
    }
    to {
      opacity: .5;
    }
  }
  .hidden-loader {
    display: none;
  }
  @media (max-width: 1024px) {
    .collection-grid-container {
      grid-template-columns: 1fr;
      padding: 0 2rem;
    }
    .collection-hero {
      padding: 3rem 2rem;
    }
    .collection-title {
      font-size: 3rem;
    }
  }
  @media (max-width: 768px) {
    .detail-back {
      margin: .5rem 1rem -2.75rem;
    }
    .collection-hero {
      height: auto;
      min-height: 300px;
      padding: 5rem 1rem 2rem;
    }
    .collection-grid-container {
      padding: 0 1rem;
      margin-top: -1rem;
      gap: 1rem;
    }
    .sidebar-column {
      display: none;
    }
    .mobile-telemetry {
      display: flex;
      grid-column: 1;
      gap: .75rem;
      flex-wrap: wrap;
      padding: .7rem .8rem;
      border: 1px solid rgba(255, 255, 255, .1);
      background: #080a0fb8;
      font: 700 .7rem var(--font-mono,monospace);
      color: #94a3b8;
      text-transform: uppercase;
      letter-spacing: .06em;
    }
    .glass-panel {
      padding: 1.25rem;
      clip-path: none;
    }
    .section-header {
      align-items: flex-start;
      gap: .75rem;
      flex-wrap: wrap;
      margin-bottom: 1rem;
    }
    .asset-count {
      white-space: normal;
    }
    .card-actions {
      gap: .35rem;
    }
    .card-actions :global(.flasharr-btn) {
      min-height: 44px;
    }
    .hero-stats {
      gap: 1.5rem;
    }
    .stat-value {
      font-size: 1.2rem;
    }
    .collection-cards-grid {
      grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
      gap: 1rem;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    *,
    *::before,
    *::after {
      animation-duration: .01ms !important;
      animation-iteration-count: 1 !important;
      transition-duration: .01ms !important;
      scroll-behavior: auto !important;
    }
  }
</style>
