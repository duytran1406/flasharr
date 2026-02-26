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
  import Button from "$lib/components/ui/Button.svelte";
  import { fetchAllMovies, type RadarrMovie } from "$lib/stores/arr";

  const collectionId = $derived(page.params.id as string);

  let collection = $state<TMDBCollection | null>(null);
  let loading = $state(true);
  let backdropLoaded = $state(false);

  // Library data
  let allMovies = $state<RadarrMovie[]>([]);
  let addingIds = $state<Set<number>>(new Set());

  // Derived reactive Set — Svelte tracks this properly so UI updates instantly
  let libraryTmdbIds = $derived(new Set(allMovies.map((m) => m.tmdbId)));

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
                <div
                  class="col-card"
                  class:owned={libraryTmdbIds.has(movie.id)}
                  in:animeFly={{ y: 20, delay: 100 }}
                  role="button"
                  tabindex="0"
                  onclick={() => goto(`/movie/${movie.id}`)}
                  onkeydown={(e) =>
                    e.key === "Enter" && goto(`/movie/${movie.id}`)}
                >
                  <div class="col-card-inner">
                    <img
                      src={movie.poster_path
                        ? getPosterUrl(movie.poster_path, "w342")
                        : "/images/placeholder-poster.svg"}
                      alt={movie.title}
                      loading="lazy"
                    />
                    <div class="card-shine"></div>

                    <!-- Library Status Badge (Top Left) -->
                    <div class="status-tags">
                      {#if libraryTmdbIds.has(movie.id)}
                        <span class="status-badge owned-badge">
                          <span class="material-icons">check_circle</span>
                          OWNED
                        </span>
                      {:else}
                        <span class="status-badge missing-badge">
                          <span class="material-icons">cloud_off</span>
                          MISSING
                        </span>
                      {/if}
                    </div>

                    <!-- Year Badge (Top Right) -->
                    {#if movie.release_date}
                      <div class="year-badge">
                        {getYear(movie.release_date)}
                      </div>
                    {/if}

                    <!-- Card Overlay -->
                    <div class="card-overlay">
                      <div class="overlay-top">
                        <h3 class="card-title">{movie.title}</h3>
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
                          {#if libraryTmdbIds.has(movie.id)}
                            <span class="meta-lib-status meta-owned"
                              >IN LIBRARY</span
                            >
                          {:else}
                            <span class="meta-lib-status meta-missing"
                              >NOT OWNED</span
                            >
                          {/if}
                        </div>
                      </div>

                      <div class="overlay-bottom">
                        <div class="card-actions">
                          <Button
                            size="sm"
                            icon="manage_search"
                            onclick={(e) => {
                              e.stopPropagation();
                              handleSmartSearch(movie);
                            }}>Search</Button
                          >
                          {#if !libraryTmdbIds.has(movie.id)}
                            <Button
                              size="sm"
                              variant="ghost"
                              icon={addingIds.has(movie.id)
                                ? "sync"
                                : "library_add"}
                              disabled={addingIds.has(movie.id)}
                              onclick={(e) => {
                                e.stopPropagation();
                                handleAddToLibrary(movie);
                              }}
                              title="Add to Radarr"
                            ></Button>
                          {/if}
                        </div>
                      </div>
                    </div>
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

  /* Hero */
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

  /* Layout */
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

  /* Glass */
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

  /* Parts Section */
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

  /* ================================================================
     Collection Cards Grid — mirrors SearchResultCard layout
     ================================================================ */
  .collection-cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 1.5rem;
  }
  @media (min-width: 1400px) {
    .collection-cards-grid {
      grid-template-columns: repeat(4, 1fr);
    }
  }

  /* Card */
  .col-card {
    position: relative;
    aspect-ratio: 2/3;
    width: 100%;
    cursor: pointer;
    transition: transform 0.4s cubic-bezier(0.175, 0.885, 0.32, 1.275);
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
    border: 1px solid rgba(255, 255, 255, 0.05);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
    transition: all 0.4s ease;
  }
  .col-card:hover .col-card-inner {
    border-color: rgba(0, 243, 255, 0.4);
    box-shadow: 0 20px 50px -10px rgba(0, 243, 255, 0.25);
  }
  .col-card.owned .col-card-inner {
    border-color: rgba(0, 255, 128, 0.15);
  }
  .col-card.owned:hover .col-card-inner {
    border-color: rgba(0, 255, 128, 0.4);
    box-shadow: 0 20px 50px -10px rgba(0, 255, 128, 0.2);
  }

  .col-card-inner img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.6s ease;
  }
  .col-card:hover img {
    transform: scale(1.1);
  }

  /* Shine */
  .card-shine {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      135deg,
      transparent 0%,
      rgba(255, 255, 255, 0.05) 50%,
      transparent 100%
    );
    transform: translateX(-100%);
    transition: transform 0.6s ease;
    z-index: 1;
  }
  .col-card:hover .card-shine {
    transform: translateX(100%);
  }

  /* Status Badges (Top Left) */
  .status-tags {
    position: absolute;
    top: 0.75rem;
    left: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    z-index: 5;
  }
  .status-badge {
    display: flex;
    align-items: center;
    gap: 3px;
    font-family: var(--font-mono, monospace);
    font-size: 0.5rem;
    font-weight: 900;
    letter-spacing: 0.08em;
    padding: 3px 8px;
    border-radius: 4px;
    backdrop-filter: blur(8px);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  }
  .status-badge .material-icons {
    font-size: 0.7rem;
  }

  .owned-badge {
    background: rgba(0, 255, 128, 0.15);
    border: 1px solid rgba(0, 255, 128, 0.4);
    color: #00ff80;
  }
  .missing-badge {
    background: rgba(255, 100, 100, 0.15);
    border: 1px solid rgba(255, 100, 100, 0.3);
    color: #ff8080;
  }

  /* Year Badge (Top Right) */
  .year-badge {
    position: absolute;
    top: 0.75rem;
    right: 0.75rem;
    background: rgba(0, 0, 0, 0.6);
    color: var(--color-primary, #00f3ff);
    font-family: var(--font-mono, monospace);
    font-size: 0.7rem;
    font-weight: 800;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    backdrop-filter: blur(8px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    z-index: 5;
  }

  /* Card Overlay — same gradient style as SearchResultCard */
  .card-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    padding: 1.25rem;
    background: linear-gradient(
      to top,
      rgba(10, 15, 30, 0.98) 0%,
      rgba(10, 15, 30, 0.7) 40%,
      transparent 100%
    );
    z-index: 2;
    transition: all 0.3s ease;
  }
  .col-card:hover .card-overlay {
    background: linear-gradient(
      to top,
      rgba(10, 15, 30, 1) 0%,
      rgba(10, 15, 30, 0.8) 60%
    );
  }

  .overlay-top {
    transform: translateY(0);
    transition: transform 0.4s ease;
  }
  .col-card:hover .overlay-top {
    transform: translateY(-8px);
  }

  .card-title {
    margin: 0 0 0.4rem;
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

  .card-meta {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    font-size: 0.8rem;
    color: rgba(255, 255, 255, 0.7);
    margin-bottom: 0.5rem;
  }
  .meta-rating {
    display: flex;
    align-items: center;
    gap: 3px;
  }

  .meta-lib-status {
    font-family: var(--font-mono, monospace);
    font-size: 0.6rem;
    font-weight: 900;
    letter-spacing: 0.08em;
  }
  .meta-owned {
    color: #00ff80;
  }
  .meta-missing {
    color: #ff8080;
  }

  /* Bottom actions — hidden until hover, slides up */
  .overlay-bottom {
    max-height: 0;
    opacity: 0;
    overflow: hidden;
    transition: all 0.4s ease;
  }
  .col-card:hover .overlay-bottom {
    max-height: 120px;
    opacity: 1;
    margin-top: 0.5rem;
  }

  .card-actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }
  /* Search button fills available space */
  .card-actions :global(.flasharr-btn:first-child) {
    flex: 1;
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
  .card-skeleton {
    aspect-ratio: 2/3;
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.03);
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
    .collection-cards-grid {
      grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
      gap: 1rem;
    }
  }
</style>
