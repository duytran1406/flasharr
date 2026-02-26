<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import {
    getCollectionDetails,
    getPosterUrl,
    getBackdropUrl,
    getYear,
    type TMDBCollection,
    type TMDBMovie,
  } from "$lib/services/tmdb";
  import { toasts } from "$lib/stores/toasts";
  import { ui } from "$lib/stores/ui.svelte";
  import { animeFade, animeFly } from "$lib/animations";
  import {
    fetchAllMovies,
    findMovieInList,
    type RadarrMovie,
  } from "$lib/stores/arr";

  const collectionId = $derived(page.params.id as string);

  let collection = $state<TMDBCollection | null>(null);
  let loading = $state(true);
  let backdropLoaded = $state(false);

  // Library data
  let allMovies = $state<RadarrMovie[]>([]);
  let addingIds = $state<Set<number>>(new Set());

  // Lookup helper: check if a TMDB movie is in Radarr library
  function getLibraryMovie(tmdbId: number): RadarrMovie | null {
    return findMovieInList(allMovies, tmdbId);
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

  // Analytics derived from collection parts
  let stats = $derived.by(() => {
    if (!collection?.parts) return null;
    const parts = collection.parts;
    const ownedCount = parts.filter((p) => getLibraryMovie(p.id)).length;
    return {
      total_parts: parts.length,
      owned: ownedCount,
      avg_score:
        parts.reduce((acc, p) => acc + p.vote_average, 0) / parts.length,
      total_votes: parts.reduce((acc, p) => acc + p.vote_count, 0),
    };
  });

  function handleSmartSearch(movie: TMDBMovie) {
    ui.openSmartSearch({
      tmdbId: String(movie.id),
      type: "movie",
      title: movie.title,
      year: getYear(movie.release_date) || undefined,
    });
  }

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
        // Optimistically add to allMovies
        allMovies = [
          ...allMovies,
          {
            id: data.arr_id,
            title: movie.title,
            tmdbId: movie.id,
          } as any,
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
</script>

<div class="collection-view">
  <div class="view-container">
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
          <div class="hero-stats" in:animeFade>
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
      <div class="main-column">
        <!-- Overview -->
        <section class="overview-section glass-panel">
          <div class="section-badge">BRIEFING</div>
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

        <!-- Parts Grid with Library Status -->
        <section class="parts-section">
          <div class="section-header">
            <h3 class="section-label">Asset Sequence</h3>
            <span class="asset-count"
              >{collection?.parts.length || 0} ITEMS DETECTED</span
            >
          </div>

          <div class="assets-grid">
            {#if loading}
              {#each Array(4) as _}
                <div class="asset-skeleton-card"></div>
              {/each}
            {:else if collection?.parts}
              {#each [...collection.parts].sort((a, b) => new Date(a.release_date || 0).getTime() - new Date(b.release_date || 0).getTime()) as movie}
                {@const libMovie = getLibraryMovie(movie.id)}
                {@const inLibrary = libMovie !== null}
                <div
                  class="asset-card"
                  class:owned={inLibrary}
                  in:animeFly={{ y: 20, delay: 100 }}
                >
                  <!-- Poster -->
                  <a href="/movie/{movie.id}" class="asset-poster">
                    <img
                      src={movie.poster_path
                        ? getPosterUrl(movie.poster_path, "w342")
                        : "/images/placeholder-poster.svg"}
                      alt={movie.title}
                      loading="lazy"
                    />
                    {#if inLibrary}
                      <div class="owned-badge">
                        <span class="material-icons">check_circle</span>
                        OWNED
                      </div>
                    {:else}
                      <div class="missing-badge">
                        <span class="material-icons">cloud_off</span>
                        MISSING
                      </div>
                    {/if}
                  </a>

                  <!-- Info -->
                  <div class="asset-info">
                    <a href="/movie/{movie.id}" class="asset-title"
                      >{movie.title}</a
                    >
                    <div class="asset-meta">
                      {#if movie.release_date}
                        <span>{getYear(movie.release_date)}</span>
                      {/if}
                      {#if movie.vote_average > 0}
                        <span class="asset-score">
                          <span class="material-icons">star</span>
                          {movie.vote_average.toFixed(1)}
                        </span>
                      {/if}
                    </div>
                  </div>

                  <!-- Actions -->
                  <div class="asset-actions">
                    <button
                      class="action-btn search-btn"
                      title="Smart Search"
                      onclick={() => handleSmartSearch(movie)}
                    >
                      <span class="material-icons">manage_search</span>
                    </button>

                    {#if !inLibrary}
                      <button
                        class="action-btn add-btn"
                        title="Add to Radarr"
                        disabled={addingIds.has(movie.id)}
                        onclick={() => handleAddToLibrary(movie)}
                      >
                        {#if addingIds.has(movie.id)}
                          <span class="material-icons spinning">sync</span>
                        {:else}
                          <span class="material-icons">library_add</span>
                        {/if}
                      </button>
                    {/if}
                  </div>
                </div>
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
    padding-bottom: 5rem;
  }

  .view-container {
    max-width: 1440px;
    margin: 0 auto;
  }

  /* Hero Section */
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
    transition: opacity 1s cubic-bezier(0.4, 0, 0.2, 1);
    transform: scale(1.05);
    animation: slowZoom 30s linear infinite alternate;
  }

  .backdrop-real.loaded {
    opacity: 0.7;
  }

  @keyframes slowZoom {
    from {
      transform: scale(1.05);
    }
    to {
      transform: scale(1.15);
    }
  }

  .hero-overlay {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      to bottom,
      rgba(1, 2, 3, 0.4) 0%,
      rgba(1, 2, 3, 0.8) 50%,
      rgba(1, 2, 3, 1) 100%
    );
    z-index: 1;
  }

  .hero-content {
    position: relative;
    z-index: 2;
    width: 100%;
  }

  .intelligence-tag {
    font-family: var(--font-mono, monospace);
    font-size: 0.7rem;
    font-weight: 800;
    color: var(--color-primary, #00f3ff);
    letter-spacing: 0.4em;
    margin-bottom: 1rem;
    opacity: 0.8;
  }

  .collection-title {
    font-size: 4rem;
    font-weight: 900;
    margin: 0;
    text-transform: uppercase;
    letter-spacing: -0.02em;
    line-height: 0.9;
    background: linear-gradient(to bottom, #fff, #94a3b8);
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
    gap: 0.25rem;
  }

  .stat-label {
    font-family: var(--font-mono, monospace);
    font-size: 0.6rem;
    font-weight: 900;
    color: #64748b;
    letter-spacing: 0.2em;
  }

  .stat-value {
    font-size: 1.5rem;
    font-weight: 900;
    color: var(--color-primary, #00f3ff);
    font-family: var(--font-heading, "Outfit", sans-serif);
  }

  .stat-value.owned {
    color: #00ff80;
    text-shadow: 0 0 12px rgba(0, 255, 128, 0.3);
  }

  /* Grid Layout */
  .collection-grid-container {
    display: grid;
    grid-template-columns: 1fr 340px;
    gap: 2rem;
    padding: 0 4rem;
    margin-top: -2rem;
    position: relative;
    z-index: 3;
  }

  .main-column {
    display: flex;
    flex-direction: column;
    gap: 3rem;
  }

  /* Glass Panels */
  .glass-panel {
    background: rgba(8, 10, 15, 0.7);
    border: 1px solid rgba(255, 255, 255, 0.08);
    backdrop-filter: blur(20px);
    padding: 2rem;
    position: relative;
    clip-path: polygon(
      0% 0%,
      calc(100% - 20px) 0%,
      100% 20px,
      100% 100%,
      20px 100%,
      0% calc(100% - 20px)
    );
  }

  .section-badge {
    position: absolute;
    top: 0;
    right: 2rem;
    background: var(--color-primary, #00f3ff);
    color: #000;
    font-family: var(--font-mono, monospace);
    font-size: 0.55rem;
    font-weight: 900;
    padding: 0.2rem 0.6rem;
    letter-spacing: 0.1em;
    clip-path: polygon(
      0% 0%,
      100% 0%,
      100% 100%,
      8px 100%,
      0% calc(100% - 8px)
    );
  }

  .section-label {
    font-family: var(--font-mono, monospace);
    font-size: 0.75rem;
    font-weight: 900;
    color: var(--color-primary, #00f3ff);
    text-transform: uppercase;
    letter-spacing: 0.25em;
    margin-bottom: 1.5rem;
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .section-label::after {
    content: "";
    flex: 1;
    height: 1px;
    background: linear-gradient(to right, rgba(0, 243, 255, 0.2), transparent);
  }

  .overview-text {
    font-size: 1rem;
    line-height: 1.8;
    color: #94a3b8;
    max-width: 900px;
  }

  /* Asset Grid */
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
    font-size: 0.65rem;
    font-weight: 900;
    color: #64748b;
    letter-spacing: 0.1em;
  }

  .assets-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 1.5rem;
  }

  /* Asset Card — each movie in collection */
  .asset-card {
    display: flex;
    flex-direction: column;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 6px;
    overflow: hidden;
    transition:
      transform 0.2s,
      border-color 0.3s,
      box-shadow 0.3s;
  }

  .asset-card:hover {
    transform: translateY(-4px);
    box-shadow:
      0 12px 40px rgba(0, 0, 0, 0.4),
      0 0 0 1px rgba(0, 243, 255, 0.15);
  }

  .asset-card.owned {
    border-color: rgba(0, 255, 128, 0.2);
  }

  .asset-card.owned:hover {
    box-shadow:
      0 12px 40px rgba(0, 0, 0, 0.4),
      0 0 0 1px rgba(0, 255, 128, 0.3);
  }

  /* Poster */
  .asset-poster {
    position: relative;
    aspect-ratio: 2/3;
    overflow: hidden;
    display: block;
  }

  .asset-poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .owned-badge,
  .missing-badge {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    align-items: center;
    gap: 4px;
    font-family: var(--font-mono, monospace);
    font-size: 0.5rem;
    font-weight: 900;
    letter-spacing: 0.08em;
    padding: 3px 8px;
    border-radius: 3px;
    backdrop-filter: blur(8px);
  }

  .owned-badge {
    background: rgba(0, 255, 128, 0.15);
    border: 1px solid rgba(0, 255, 128, 0.4);
    color: #00ff80;
  }

  .owned-badge .material-icons {
    font-size: 0.75rem;
  }

  .missing-badge {
    background: rgba(255, 100, 100, 0.15);
    border: 1px solid rgba(255, 100, 100, 0.3);
    color: #ff8080;
  }

  .missing-badge .material-icons {
    font-size: 0.75rem;
  }

  /* Info */
  .asset-info {
    padding: 0.75rem;
    flex: 1;
  }

  .asset-title {
    font-size: 0.8rem;
    font-weight: 700;
    color: #fff;
    text-decoration: none;
    display: block;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-bottom: 4px;
  }

  .asset-title:hover {
    color: var(--color-primary, #00f3ff);
  }

  .asset-meta {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.65rem;
    color: #64748b;
    font-family: var(--font-mono, monospace);
  }

  .asset-score {
    display: flex;
    align-items: center;
    gap: 2px;
    color: #fbbf24;
  }

  .asset-score .material-icons {
    font-size: 0.7rem;
  }

  /* Action buttons */
  .asset-actions {
    display: flex;
    gap: 4px;
    padding: 0 0.75rem 0.75rem;
  }

  .action-btn {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 0.4rem;
    border-radius: 4px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
    color: #94a3b8;
    font-family: var(--font-mono, monospace);
    font-size: 0.6rem;
    font-weight: 700;
    cursor: pointer;
    transition: all 0.2s;
  }

  .action-btn .material-icons {
    font-size: 1rem;
  }

  .search-btn:hover {
    border-color: rgba(0, 243, 255, 0.4);
    background: rgba(0, 243, 255, 0.08);
    color: var(--color-primary, #00f3ff);
  }

  .add-btn:hover {
    border-color: rgba(0, 255, 128, 0.4);
    background: rgba(0, 255, 128, 0.08);
    color: #00ff80;
  }

  .add-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .spinning {
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Sidebar */
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
    padding-bottom: 0.75rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
  }

  .tel-label {
    font-family: var(--font-mono, monospace);
    font-size: 0.6rem;
    font-weight: 800;
    color: #64748b;
  }

  .tel-value {
    font-family: var(--font-mono, monospace);
    font-size: 0.8rem;
    font-weight: 700;
    color: #fff;
  }

  .terminal-green {
    color: #00ff80;
    text-shadow: 0 0 10px rgba(0, 255, 128, 0.3);
  }

  .terminal-amber {
    color: #fbbf24;
    text-shadow: 0 0 10px rgba(251, 191, 36, 0.3);
  }

  .terminal-red {
    color: #ff6464;
    text-shadow: 0 0 10px rgba(255, 100, 100, 0.3);
  }

  .tactical-btn {
    width: 100%;
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #fff;
    padding: 1rem;
    font-family: var(--font-mono, monospace);
    font-size: 0.7rem;
    font-weight: 800;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    cursor: pointer;
    transition: all 0.3s;
    clip-path: polygon(
      8px 0%,
      100% 0%,
      100% calc(100% - 8px),
      calc(100% - 8px) 100%,
      0% 100%,
      0% 8px
    );
  }

  .tactical-btn:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: var(--color-primary, #00f3ff);
    color: var(--color-primary, #00f3ff);
    transform: translateY(-2px);
  }

  .tactical-btn .material-icons {
    font-size: 1.2rem;
  }

  /* Skeletons */
  .skeleton-title {
    width: 60%;
    height: 4rem;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    animation: shimmer 2s infinite linear;
  }

  .skeleton-text {
    width: 100%;
    height: 1rem;
    background: rgba(255, 255, 255, 0.05);
    margin-bottom: 0.75rem;
    border-radius: 4px;
    animation: shimmer 2s infinite linear;
  }

  .asset-skeleton-card {
    aspect-ratio: 2/3;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 6px;
    animation: shimmer 2s infinite linear;
  }

  @keyframes shimmer {
    0% {
      opacity: 0.5;
    }
    50% {
      opacity: 0.8;
    }
    100% {
      opacity: 0.5;
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
    .hero-stats {
      gap: 1.5rem;
    }

    .stat-value {
      font-size: 1.2rem;
    }

    .assets-grid {
      grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
      gap: 1rem;
    }
  }
</style>
