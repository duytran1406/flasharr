<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import {
    getMovieDetails,
    getPosterUrl,
    getBackdropUrl,
    getYear,
    getSimilar,
    getRecommendations,
    type TMDBMovieDetails,
    type TMDBMovie,
  } from "$lib/services/tmdb";
  import { toasts } from "$lib/stores/toasts";
  import { ui } from "$lib/stores/ui.svelte";
  import { MediaCard } from "$lib/components";
  import { Button, Badge } from "@media-set/core-ui";
  import {
    fetchAllMovies,
    formatDiskSize,
    findMovieInList,
    type RadarrMovie,
  } from "$lib/stores/arr";

  const movieId = $derived(page.params.id as string);

  let movie = $state<TMDBMovieDetails | null>(null);
  let similar = $state<TMDBMovie[]>([]);
  let recommended = $state<TMDBMovie[]>([]);
  let loading = $state(true);
  let posterLoaded = $state(false);
  let backdropLoaded = $state(false);
  // Library integration
  let libraryMovie = $state<RadarrMovie | null>(null);
  let inLibrary = $derived(libraryMovie !== null);
  let addingToLibrary = $state(false);

  async function loadData() {
    loading = true;
    try {
      const [details, sim, reco] = await Promise.all([
        getMovieDetails(movieId),
        getSimilar("movie", movieId),
        getRecommendations("movie", movieId),
      ]);

      movie = details;
      similar = sim.slice(0, 4);
      recommended = reco.slice(0, 4);

      // Parallel: fetch Radarr library data
      try {
        const allMovies = await fetchAllMovies();
        const tmdbIdNum = Number(movieId);
        const match = findMovieInList(allMovies, tmdbIdNum);
        if (match) {
          libraryMovie = match;
        }
      } catch {
        // Library lookup is best-effort
      }
    } catch (error) {
      console.error("Failed to load movie data:", error);
      toasts.error("Failed to load movie intelligence brief");
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (movieId) {
      posterLoaded = false;
      backdropLoaded = false;
      loadData();
    }
  });

  function formatCurrency(val: number) {
    if (!val) return "N/A";
    return new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: "USD",
      maximumFractionDigits: 0,
    }).format(val);
  }

  function formatRuntime(mins: number) {
    if (!mins) return "N/A";
    const hrs = Math.floor(mins / 60);
    const m = mins % 60;
    return hrs > 0 ? `${hrs}h ${m}m` : `${m}m`;
  }

  async function handleAddToLibrary() {
    if (!movie || addingToLibrary) return;
    addingToLibrary = true;
    try {
      const resp = await fetch("/api/arr/movies/add", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ tmdb_id: Number(movieId) }),
      });
      if (resp.ok) {
        const data = await resp.json();
        toasts.success(`"${movie.title}" added to Radarr library`);
        // Mark as in-library optimistically so the button disappears
        libraryMovie = {
          id: data.arr_id,
          title: movie.title,
          tmdb_id: Number(movieId),
        } as any;
      } else if (resp.status === 409) {
        toasts.info(`"${movie.title}" is already in your Radarr library`);
      } else {
        const text = await resp.text();
        toasts.error(`Failed to add to library: ${text}`);
      }
    } catch {
      toasts.error("Network error — could not reach server");
    } finally {
      addingToLibrary = false;
    }
  }

  function handleSmartSearch() {
    if (!movie) return;
    ui.openSmartSearch(
      {
        tmdbId: String(movie.id),
        type: "movie",
        title: movie.title,
        year: getYear(movie.release_date) || undefined,
      },
      { mode: "target" },
    );
  }

  // Get specific release dates (Theatrical, Digital, Physical)
  let releaseInfo = $derived.by(() => {
    if (!movie?.release_dates) return null;
    const us = movie.release_dates.results.find((r) => r.iso_3166_1 === "US");
    if (!us) return null;

    const info: Record<string, string> = {};
    us.release_dates.forEach((rd) => {
      const date = new Date(rd.release_date).toLocaleDateString("en-US", {
        month: "long",
        day: "numeric",
        year: "numeric",
      });
      if (rd.type === 3) info.Theatrical = date;
      if (rd.type === 4) info.Digital = date;
      if (rd.type === 5) info.Physical = date;
    });
    return info;
  });

  // Get Keywords
  let tags = $derived(movie?.keywords?.keywords || []);

  // Format Score for UI
  function formatScore(score: number) {
    return Math.round(score * 10);
  }

  function handleCollectionClick() {
    if (movie?.belongs_to_collection) {
      goto(`/collection/${movie.belongs_to_collection.id}`);
    }
  }

  function goBack() {
    window.history.back();
  }
</script>

<div class="media-detail-view" data-view="movie">
  <div class="detail-container">
    <button class="detail-back" type="button" onclick={goBack}>
      <span class="material-icons" aria-hidden="true">arrow_back</span>
      Back
    </button>

    <!-- Hero Section -->
    <div
      class="detail-hero"
      class:skeleton-bg={loading}
      style="background-image: url(/images/placeholder-banner.png)"
    >
      {#if movie?.backdrop_path}
        <div
          class="backdrop-real"
          class:loaded={backdropLoaded}
          style={`background-image: url(${getBackdropUrl(movie.backdrop_path, "original")})`}
        ></div>
        <img
          src={getBackdropUrl(movie.backdrop_path, "original")}
          alt=""
          class="hidden-loader"
          onload={() => (backdropLoaded = true)}
        />
      {/if}
      <div class="hero-overlay"></div>
      <div class="hero-content">
        <div class="poster-container glass-panel" class:skeleton={loading}>
          <img
            src={movie?.poster_path
              ? getPosterUrl(movie.poster_path, "w500")
              : "/images/placeholder-poster.svg"}
            alt={movie?.title || "Unknown Subject"}
            class="detail-poster"
            class:loaded={posterLoaded || !movie?.poster_path}
            onload={() => (posterLoaded = true)}
          />
        </div>
        <div class="hero-info">
          {#if loading}
            <div class="skeleton skeleton-tagline"></div>
            <div class="skeleton skeleton-title"></div>
            <div class="skeleton skeleton-meta"></div>
          {:else}
            {#if movie?.tagline}
              <div class="detail-tagline">{movie.tagline}</div>
            {:else if !movie}
              <div class="detail-tagline">
                SUBJECT_CLASSIFIED: PENDING_RELEASE
              </div>
            {/if}
            <h1 class="detail-title">{movie?.title || "Unknown Subject"}</h1>
            <div class="detail-meta">
              {#if movie}
                <span class="meta-year">{getYear(movie.release_date)}</span>
                <span class="meta-divider">•</span>
                <span class="meta-rating">
                  <span class="material-icons">star</span>
                  {movie.vote_average.toFixed(1)}
                </span>
                <span class="meta-divider">•</span>
                <span class="meta-runtime">{formatRuntime(movie.runtime)}</span>
                <span class="meta-divider">•</span>
                <span class="meta-genres">
                  {movie.genres
                    .slice(0, 3)
                    .map((g) => g.name)
                    .join(" / ")}
                </span>
              {:else}
                <span class="meta-year">YYYY-MM-DD</span>
                <span class="meta-divider">•</span>
                <span class="meta-rating">SECURE_LEVEL: OMEGA</span>
              {/if}
            </div>
          {/if}
        </div>
      </div>
    </div>

    <!-- Content Grid -->
    <div class="detail-grid">
      <!-- Main Content -->
      <div class="main-content">
        <!-- Collection Banner (Inlined into main content) -->
        <section class="mobile-action-panel mobile-action-panel-first">
          {#if inLibrary}
            <Badge variant="success" size="sm">IN LIBRARY</Badge>
          {/if}
          <Button
            icon="manage_search"
            size="md"
            width="100%"
            disabled={loading}
            onclick={handleSmartSearch}>Smart Search</Button
          >
          {#if !inLibrary && !loading}
            <Button
              variant="ghost"
              icon="library_add"
              size="md"
              width="100%"
              disabled={addingToLibrary}
              onclick={handleAddToLibrary}
              >{addingToLibrary ? "Adding..." : "Add to Library"}</Button
            >
          {/if}
        </section>

        {#if movie?.belongs_to_collection}
          <div class="collection-intro">
            <button
              class="collection-banner glass-panel"
              onclick={handleCollectionClick}
            >
              <div
                class="banner-bg"
                style={`background-image: url(${getBackdropUrl(movie.belongs_to_collection.backdrop_path, "w1280")})`}
              ></div>
              <div class="banner-overlay"></div>
              <div class="banner-content">
                <div class="banner-text">
                  <span class="banner-tag">PART OF THE COLLECTION</span>
                  <h4 class="collection-title">
                    {movie.belongs_to_collection.name}
                  </h4>
                </div>
                <div class="banner-view-btn">
                  <span>VIEW</span>
                </div>
              </div>
            </button>
          </div>
        {/if}

        <section class="overview-section">
          <h3 class="section-label">Overview</h3>
          {#if loading}
            <div class="skeleton skeleton-text"></div>
            <div class="skeleton skeleton-text"></div>
            <div class="skeleton skeleton-text" style="width: 60%"></div>
          {:else}
            <p class="overview-text">
              {movie?.overview ||
                "Intelligence report pending. Analysis of data fragments suggests a high-priority unreleased asset."}
            </p>
          {/if}
        </section>

        <section class="mobile-action-panel mobile-action-panel-late">
          {#if inLibrary}
            <div class="mb-2 w-full">
              <Badge variant="success" size="sm" class="w-full justify-center rounded-lg">
                <span class="material-icons" style="font-size: 13px;">video_library</span>
                IN LIBRARY
              </Badge>
            </div>
          {/if}
          <Button
            icon="manage_search"
            size="md"
            width="100%"
            disabled={loading}
            onclick={handleSmartSearch}>Smart Search</Button
          >
          {#if !inLibrary && !loading}
            <Button
              variant="ghost"
              icon="library_add"
              size="md"
              width="100%"
              disabled={addingToLibrary}
              onclick={handleAddToLibrary}
              >{addingToLibrary ? "Adding..." : "Add to Library"}</Button
            >
          {/if}
        </section>

        <!-- Similar Titles -->
        <section class="related-section">
          <h3 class="section-label">Similar Titles</h3>
          <div class="related-grid">
            {#if loading}
              {#each Array(4) as _}
                <div class="related-card">
                  <div class="related-poster skeleton"></div>
                  <div class="related-info">
                    <div class="skeleton skeleton-small-text"></div>
                    <div
                      class="skeleton skeleton-small-text"
                      style="width: 40%"
                    ></div>
                  </div>
                </div>
              {/each}
            {:else if similar.length > 0}
              {#each similar as item}
                <MediaCard
                  id={item.id}
                  title={item.title}
                  posterPath={item.poster_path}
                  voteAverage={item.vote_average}
                  releaseDate={item.release_date}
                  overview={item.overview}
                  mediaType="movie"
                />
              {/each}
            {/if}
          </div>
        </section>

        <!-- Recommendations -->
        {#if loading || recommended.length > 0}
          <section class="related-section">
            <h3 class="section-label">Recommended for You</h3>
            <div class="related-grid">
              {#if loading}
                {#each Array(4) as _}
                  <div class="related-card">
                    <div class="related-poster skeleton"></div>
                    <div class="related-info">
                      <div class="skeleton skeleton-small-text"></div>
                      <div
                        class="skeleton skeleton-small-text"
                        style="width: 40%"
                      ></div>
                    </div>
                  </div>
                {/each}
              {:else}
                {#each recommended as item}
                  <MediaCard
                    id={item.id}
                    title={item.title}
                    posterPath={item.poster_path}
                    voteAverage={item.vote_average}
                    releaseDate={item.release_date}
                    overview={item.overview}
                    mediaType="movie"
                  />
                {/each}
              {/if}
            </div>
          </section>
        {/if}
      </div>

      <!-- Sidebar -->
      <aside class="detail-sidebar">
        <div class="action-panel glass-panel">
          {#if inLibrary}
            <div class="mb-2 w-full">
              <Badge variant="success" size="sm" class="w-full justify-center rounded-lg">
                <span class="material-icons" style="font-size: 13px;">video_library</span>
                IN LIBRARY
              </Badge>
            </div>
          {/if}
          <Button
            icon="manage_search"
            size="md"
            width="100%"
            disabled={loading}
            onclick={handleSmartSearch}>Smart Search</Button
          >
          {#if !inLibrary && !loading}
            <Button
              variant="ghost"
              icon="library_add"
              size="md"
              width="100%"
              disabled={addingToLibrary}
              onclick={handleAddToLibrary}
              >{addingToLibrary ? "Adding..." : "Add to Library"}</Button
            >
          {/if}
        </div>

        <div class="stats-panel glass-panel">
          {#if loading}
            {#each Array(4) as _}
              <div class="info-row">
                <div
                  class="skeleton skeleton-small-text"
                  style="width: 40%"
                ></div>
                <div
                  class="skeleton skeleton-small-text"
                  style="width: 30%"
                ></div>
              </div>
            {/each}
          {:else if movie}
            <div class="info-row">
              <span class="label">Status</span>
              <span class="value" style="color: var(--color-primary)"
                >{movie.status}</span
              >
            </div>

            {#if releaseInfo}
              <div class="info-section-label">Release Dates</div>
              {#each Object.entries(releaseInfo) as [type, date]}
                <div class="info-row">
                  <span class="label sub-label">{type}</span>
                  <span class="value">{date}</span>
                </div>
              {/each}
            {:else}
              <div class="info-row">
                <span class="label">Release Date</span>
                <span class="value"
                  >{new Date(movie.release_date).toLocaleDateString()}</span
                >
              </div>
            {/if}

            <div class="info-section-label">Intelligence Scores</div>
            <div class="info-row">
              <span class="label">TMDB SCORE</span>
              <span class="value">{formatScore(movie.vote_average)}%</span>
            </div>
            {#if movie.external_ids?.imdb_id}
              <div class="info-row">
                <span class="label">IMDB SCORE</span>
                <span class="value">{movie.vote_average.toFixed(1)}</span>
              </div>
            {/if}

            <div class="info-section-label">Financials</div>
            {#if movie.budget > 0}
              <div class="info-row">
                <span class="label">Budget</span>
                <span class="value">{formatCurrency(movie.budget)}</span>
              </div>
            {/if}
            {#if movie.revenue > 0}
              <div class="info-row">
                <span class="label">Revenue</span>
                <span class="value">{formatCurrency(movie.revenue)}</span>
              </div>
            {/if}

            <div class="info-row">
              <span class="label">Language</span>
              <span class="value">{movie.original_language.toUpperCase()}</span>
            </div>

            <div class="external-links">
              <a
                href="https://www.themoviedb.org/movie/{movie.id}"
                target="_blank"
                class="link-icon"
                title="TMDB"
              >
                <img
                  src="https://www.themoviedb.org/assets/2/v4/logos/v2/blue_square_2-d537fb228cf3ded904ef09b136fe3fec72548ebc1fea3fbbd1ad9e36364db38b.svg"
                  alt="TMDB"
                />
              </a>
              {#if movie.external_ids?.imdb_id}
                <a
                  href="https://www.imdb.com/title/{movie.external_ids.imdb_id}"
                  target="_blank"
                  class="link-icon"
                  title="IMDb"
                >
                  <img
                    src="https://upload.wikimedia.org/wikipedia/commons/6/69/IMDB_Logo_2016.svg"
                    alt="IMDb"
                  />
                </a>
              {/if}
            </div>

            {#if inLibrary && libraryMovie}
              <div class="info-section-label">Library Data</div>
              <div class="info-row">
                <span class="label">Monitored</span>
                <span
                  class="value"
                  style="color: {libraryMovie.monitored
                    ? '#34d399'
                    : '#94a3b8'}"
                >
                  {libraryMovie.monitored ? "Yes" : "No"}
                </span>
              </div>
              <div class="info-row">
                <span class="label">File Status</span>
                <span
                  class="value"
                  style="color: {libraryMovie.hasFile ? '#34d399' : '#ef4444'}"
                >
                  {libraryMovie.hasFile ? "Acquired" : "Missing"}
                </span>
              </div>
              {#if libraryMovie.sizeOnDisk && libraryMovie.sizeOnDisk > 0}
                <div class="info-row">
                  <span class="label">On Disk</span>
                  <span class="value"
                    >{formatDiskSize(libraryMovie.sizeOnDisk)}</span
                  >
                </div>
              {/if}
            {/if}
          {/if}
        </div>

        {#if tags.length > 0}
          <div class="keywords-panel">
            {#each tags.slice(0, 10) as tag}
              <span class="keyword-tag">{tag.name}</span>
            {/each}
          </div>
        {/if}
      </aside>
    </div>
  </div>
</div>

<style>
  .media-detail-view {
    width: 100%;
    min-height: calc(100vh - 80px);
    padding-bottom: max(2rem, env(safe-area-inset-bottom));
    background: var(--bg-main);
    color: var(--text-primary);
  }
  .detail-back {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: .4rem;
    min-height: 44px;
    margin: .75rem 2rem 0;
    padding: .5rem .75rem;
    border: 1px solid rgba(255, 255, 255, .16);
    border-radius: 8px;
    background: #0a0f19b8;
    color: var(--text-primary);
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
  .skeleton-bg {
    position: relative;
    overflow: hidden;
  }
  .skeleton-bg:after {
    content: "";
    position: absolute;
    inset: 0;
    background:
      linear-gradient(
        90deg,
        transparent,
        rgba(255, 255, 255, .05),
        transparent);
    animation: shimmer 2s infinite linear;
  }
  .skeleton {
    position: relative;
    overflow: hidden;
    border-radius: 4px;
  }
  .skeleton:after {
    content: "";
    position: absolute;
    inset: 0;
    background:
      linear-gradient(
        90deg,
        transparent,
        rgba(255, 255, 255, .05),
        transparent);
    animation: shimmer 2s infinite linear;
  }
  @keyframes shimmer {
    0% {
      transform: translate(-100%);
    }
    to {
      transform: translate(100%);
    }
  }
  .skeleton-tagline {
    width: 200px;
    height: 1rem;
    margin-bottom: .5rem;
  }
  .skeleton-title {
    width: 400px;
    height: 3.5rem;
    margin-bottom: 1rem;
  }
  .skeleton-meta {
    width: 300px;
    height: 1.2rem;
  }
  .skeleton-text {
    width: 100%;
    height: 1rem;
    margin-bottom: .75rem;
  }
  .skeleton-small-text {
    width: 80%;
    height: .8rem;
    margin-bottom: .4rem;
  }
  .detail-container {
    display: flex;
    flex-direction: column;
  }
  .detail-hero {
    height: 450px;
    background-size: cover;
    background-position: center;
    position: relative;
    display: flex;
    align-items: flex-end;
    padding: 0 2rem 3rem;
  }
  .hero-overlay {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(
        to bottom,
        #0f172a33,
        #0f172a);
  }
  .hero-content {
    position: relative;
    z-index: 1;
    display: flex;
    gap: 2.5rem;
    align-items: flex-end;
    width: 100%;
    max-width: 1400px;
    margin: 0 auto;
  }
  .detail-poster {
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity .15s ease-in-out;
  }
  .detail-poster.loaded {
    opacity: 1;
  }
  .backdrop-real {
    position: absolute;
    inset: 0;
    background-size: cover;
    background-position: center;
    opacity: 0;
    transition: opacity .15s ease-in-out;
  }
  .backdrop-real.loaded {
    opacity: 1;
  }
  .hidden-loader {
    display: none;
  }
  .poster-container {
    width: 220px;
    aspect-ratio: 2/3;
    border-radius: 12px;
    overflow: hidden;
    box-shadow: 0 20px 50px #000c;
    border: 1px solid rgba(255, 255, 255, .1);
    flex-shrink: 0;
    background-image: url(/images/placeholder-poster.svg);
    background-size: cover;
    background-position: center;
  }
  .hero-info {
    flex: 1;
    padding-bottom: 1rem;
  }
  .detail-tagline {
    font-size: .85rem;
    font-weight: 800;
    color: var(--color-primary);
    text-transform: uppercase;
    letter-spacing: .25rem;
    margin-bottom: .5rem;
    font-family: var(--font-mono);
  }
  .detail-title {
    font-size: 3.5rem;
    font-weight: 800;
    margin: 0 0 1rem;
    line-height: 1.1;
    text-shadow: 0 2px 10px rgba(0, 0, 0, .5);
  }
  .detail-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: .4rem 1rem;
    font-size: .9rem;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }
  .meta-year {
    color: var(--text-primary);
  }
  .meta-rating {
    display: flex;
    align-items: center;
    gap: 4px;
    color: gold;
  }
  .meta-rating .material-icons {
    font-size: 16px;
  }
  .detail-grid {
    display: grid;
    grid-template-columns: 1fr 350px;
    gap: 3rem;
    padding: 3rem 2rem;
    max-width: 1400px;
    margin: 0 auto;
    width: 100%;
  }
  .section-label {
    text-transform: uppercase;
    letter-spacing: .15em;
    font-size: .75rem;
    color: var(--color-primary);
    margin-bottom: 1.5rem;
    font-weight: 800;
    display: flex;
    align-items: center;
    gap: .5rem;
  }
  .overview-text {
    font-size: 1.1rem;
    line-height: 1.8;
    color: var(--text-secondary);
    margin-bottom: 4rem;
  }
  .related-section {
    margin-bottom: 4rem;
  }
  .related-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 1.1rem;
    width: 100%;
    max-width: 100%;
  }
  .detail-sidebar {
    position: relative;
    z-index: 5;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }
  .mobile-action-panel {
    display: none;
  }
  @media (max-width: 768px) {
    .detail-grid {
      grid-template-columns: 1fr;
      gap: 1rem;
    }
    .detail-sidebar {
      display: none;
    }
    .mobile-action-panel {
      display: flex;
      flex-direction: column;
      gap: .6rem;
      margin: 0 0 1.25rem;
      padding: .9rem;
      border: 1px solid rgba(0, 243, 255, .2);
      background: #00f3ff0d;
      border-radius: 10px;
    }
    .mobile-action-panel-late {
      display: none;
    }
    .related-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: .9rem;
    }
    .overview-text {
      font-size: .95rem;
      line-height: 1.6;
      margin-bottom: 1.5rem;
    }
  }
  .action-panel {
    padding: 1.5rem;
    background: #00f3ff0d;
    border: 1px solid rgba(0, 243, 255, .2);
    position: relative;
    overflow: hidden;
  }
  .action-panel:before {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    width: 4px;
    height: 100%;
    background: var(--color-primary);
    box-shadow: 0 0 15px var(--color-primary);
  }
  .stats-panel {
    padding: .5rem 0;
    border: 1px solid rgba(255, 255, 255, .08);
    background: #0a0f1999;
    -webkit-backdrop-filter: blur(20px);
    backdrop-filter: blur(20px);
    overflow: hidden;
    position: relative;
  }
  .stats-panel:after {
    content: "";
    position: absolute;
    bottom: 0;
    right: 0;
    width: 20px;
    height: 20px;
    background:
      linear-gradient(
        135deg,
        transparent 50%,
        rgba(0, 243, 255, .2) 50%);
    pointer-events: none;
  }
  .info-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: .9rem 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, .03);
    transition: all .2s;
  }
  .info-row:hover {
    background: #00f3ff0d;
    padding-left: 1.75rem;
  }
  .info-row:last-child {
    border-bottom: none;
  }
  .info-row .label {
    font-size: .65rem;
    font-weight: 800;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: .15em;
    font-family: var(--font-mono, monospace);
  }
  .info-row .value {
    font-size: .9rem;
    font-weight: 700;
    color: var(--text-primary);
    font-family: var(--font-mono, monospace);
  }
  .external-links {
    display: flex;
    justify-content: center;
    gap: 1.5rem;
    padding: 1.5rem;
    border-top: 1px solid rgba(255, 255, 255, .08);
    background: #0003;
  }
  .link-icon {
    width: 40px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: .5;
    transition: all .3s;
    filter: grayscale(1);
  }
  .link-icon:hover {
    opacity: 1;
    filter: grayscale(0);
    transform: scale(1.1);
  }
  .link-icon img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
  .collection-intro {
    margin-bottom: 2.5rem;
  }
  .collection-banner {
    width: 100%;
    height: 120px;
    position: relative;
    display: flex;
    align-items: center;
    padding: 0 2.5rem;
    border: 1px solid rgba(255, 255, 255, .1);
    background: #000;
    overflow: hidden;
    cursor: pointer;
    transition: all .3s cubic-bezier(.4, 0, .2, 1);
    text-align: left;
    clip-path: polygon(0% 0%, calc(100% - 20px) 0%, 100% 20px, 100% 100%, 20px 100%, 0% calc(100% - 20px));
  }
  .collection-banner:hover {
    border-color: var(--color-primary);
    transform: translateY(-2px);
    box-shadow: 0 10px 40px #00f3ff26, 0 0 20px #00f3ff0d;
  }
  .banner-bg {
    position: absolute;
    inset: 0;
    background-size: cover;
    background-position: center 30%;
    opacity: .65;
    transition: transform 1.2s cubic-bezier(.23, 1, .32, 1);
    filter: saturate(1.2) contrast(1.1);
  }
  .collection-banner:hover .banner-bg {
    transform: scale(1.1);
    opacity: .8;
  }
  .banner-overlay {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(
        90deg,
        #010203fa,
        #010203cc 35%,
        #00f3ff0d);
  }
  .banner-content {
    position: relative;
    z-index: 1;
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .banner-text {
    display: flex;
    flex-direction: column;
    gap: .4rem;
  }
  .banner-tag {
    font-size: .6rem;
    color: var(--color-primary);
    text-transform: uppercase;
    letter-spacing: .25em;
    font-weight: 800;
    font-family: var(--font-mono, monospace);
    opacity: .9;
  }
  .collection-title {
    margin: 0;
    font-size: 1.8rem;
    font-weight: 900;
    color: #fff;
    text-transform: uppercase;
    letter-spacing: .02em;
    text-shadow: 0 4px 12px rgba(0, 0, 0, .8);
  }
  .banner-view-btn {
    background: #0009;
    border: 1px solid rgba(255, 255, 255, .15);
    padding: .6rem 1.4rem;
    border-radius: 50px;
    font-size: .65rem;
    font-weight: 900;
    letter-spacing: .15em;
    color: #fff;
    transition: all .3s;
    -webkit-backdrop-filter: blur(10px);
    backdrop-filter: blur(10px);
  }
  .collection-banner:hover .banner-view-btn {
    background: var(--color-primary);
    color: #000;
    border-color: var(--color-primary);
    box-shadow: 0 0 20px #00f3ff66;
  }
  .info-section-label {
    padding: 1rem 1.5rem .5rem;
    font-size: .6rem;
    font-weight: 900;
    color: var(--color-primary);
    text-transform: uppercase;
    letter-spacing: .25em;
    background: #00f3ff0d;
    border-top: 1px solid rgba(0, 243, 255, .1);
  }
  .info-row .sub-label {
    font-weight: 500;
    opacity: .8;
    padding-left: .5rem;
  }
  .keywords-panel {
    margin-top: 1.5rem;
    display: flex;
    flex-wrap: wrap;
    gap: .5rem;
    padding: .5rem;
  }
  .keyword-tag {
    background: #ffffff0d;
    border: 1px solid rgba(255, 255, 255, .1);
    padding: .35rem .75rem;
    font-size: .65rem;
    font-weight: 600;
    color: var(--text-muted);
    transition: all .2s;
    cursor: default;
    clip-path: polygon(4px 0%, 100% 0%, 100% calc(100% - 4px), calc(100% - 4px) 100%, 0% 100%, 0% 4px);
  }
  .keyword-tag:hover {
    background: #00f3ff1a;
    color: var(--color-primary);
    border-color: #00f3ff4d;
  }
  @media (max-width: 1024px) {
    .detail-grid {
      grid-template-columns: 1fr;
    }
    .detail-hero {
      height: 350px;
    }
    .detail-title {
      font-size: 2.5rem;
    }
    .collection-banner {
      height: 80px;
      padding: 0 1.5rem;
    }
    .collection-title {
      font-size: 1rem;
    }
  }
  @media (max-width: 768px) {
    .detail-back {
      margin: .5rem 1rem 0;
    }
    .detail-meta {
      gap: .35rem .65rem;
    }
    .meta-divider {
      display: none;
    }
    .detail-hero {
      height: 350px;
      padding: 0 1rem 2rem;
    }
    .poster-container {
      display: none;
    }
    .detail-title {
      font-size: 2rem;
    }
    .detail-grid {
      padding: 2rem 1rem;
    }
  }
</style>
