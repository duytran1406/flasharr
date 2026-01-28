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

  const movieId = $derived(page.params.id as string);

  let movie = $state<TMDBMovieDetails | null>(null);
  let similar = $state<TMDBMovie[]>([]);
  let recommended = $state<TMDBMovie[]>([]);
  let loading = $state(true);
  let posterLoaded = $state(false);
  let backdropLoaded = $state(false);
  let isUpcoming = $derived(
    !movie ||
      movie.status === "Planned" ||
      movie.status === "In Production" ||
      (movie.release_date && new Date(movie.release_date) > new Date()),
  );

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

  function handleSmartSearch() {
    if (!movie) return;
    ui.openSmartSearch({
      tmdbId: String(movie.id),
      type: "movie",
      title: movie.title,
      year: getYear(movie.release_date) || undefined,
    });
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
</script>

<div class="media-detail-view" data-view="movie">
  <div class="detail-container">
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
          <button
            class="smart-search-btn"
            disabled={loading}
            onclick={handleSmartSearch}
          >
            <span class="material-icons">manage_search</span>
            SMART SEARCH
          </button>
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
    background: var(--bg-main);
    color: var(--text-primary);
  }

  .skeleton-bg {
    position: relative;
    overflow: hidden;
  }

  .skeleton-bg::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(255, 255, 255, 0.05),
      transparent
    );
    animation: shimmer 2s infinite linear;
  }

  .skeleton {
    position: relative;
    overflow: hidden;
    border-radius: 4px;
  }

  .skeleton::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(255, 255, 255, 0.05),
      transparent
    );
    animation: shimmer 2s infinite linear;
  }

  @keyframes shimmer {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(100%);
    }
  }

  .skeleton-tagline {
    width: 200px;
    height: 1rem;
    margin-bottom: 0.5rem;
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
    margin-bottom: 0.75rem;
  }
  .skeleton-small-text {
    width: 80%;
    height: 0.8rem;
    margin-bottom: 0.4rem;
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
    background: linear-gradient(
      to bottom,
      rgba(15, 23, 42, 0.2) 0%,
      rgba(15, 23, 42, 1) 100%
    );
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
    transition: opacity 0.15s ease-in-out;
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
    transition: opacity 0.15s ease-in-out;
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
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.8);
    border: 1px solid rgba(255, 255, 255, 0.1);
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
    font-size: 0.85rem;
    font-weight: 800;
    color: var(--color-primary);
    text-transform: uppercase;
    letter-spacing: 0.25rem;
    margin-bottom: 0.5rem;
    font-family: var(--font-mono);
  }

  .detail-title {
    font-size: 3.5rem;
    font-weight: 800;
    margin: 0 0 1rem;
    line-height: 1.1;
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.5);
  }

  .detail-meta {
    display: flex;
    align-items: center;
    gap: 1rem;
    font-size: 0.9rem;
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
    color: #ffd700;
  }
  .meta-rating .material-icons {
    font-size: 16px;
  }

  /* Content Grid */
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
    letter-spacing: 0.15em;
    font-size: 0.75rem;
    color: var(--color-primary);
    margin-bottom: 1.5rem;
    font-weight: 800;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .overview-text {
    font-size: 1.1rem;
    line-height: 1.8;
    color: var(--text-secondary);
    margin-bottom: 4rem;
  }

  /* Related Sections */
  .related-section {
    margin-bottom: 4rem;
  }

  .related-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 2rem; /* Increased gap for better separation */
  }

  /* Sidebar */
  .detail-sidebar {
    position: relative;
    z-index: 5; /* Lower than hovered card (50) but higher than normal grid items */
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .action-panel {
    padding: 1.5rem;
    background: rgba(0, 243, 255, 0.05);
    border: 1px solid rgba(0, 243, 255, 0.2);
    position: relative;
    overflow: hidden;
  }

  .action-panel::before {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    width: 4px;
    height: 100%;
    background: var(--color-primary);
    box-shadow: 0 0 15px var(--color-primary);
  }

  .smart-search-btn {
    width: 100%;
    padding: 1rem 1.5rem;
    background: rgba(0, 243, 255, 0.08);
    color: var(--color-primary);
    border: 1px solid rgba(0, 243, 255, 0.2);
    font-weight: 800;
    font-size: 0.8rem;
    letter-spacing: 0.1em;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    position: relative;
    text-transform: uppercase;
    font-family: var(--font-mono, monospace);
    border-radius: 12px;
    backdrop-filter: blur(8px);
  }

  .smart-search-btn:hover {
    background: rgba(0, 243, 255, 0.15);
    border-color: rgba(0, 243, 255, 0.4);
    transform: translateY(-2px);
    box-shadow: 0 8px 20px rgba(0, 243, 255, 0.15);
  }

  .smart-search-btn:active {
    transform: translateY(0) scale(0.98);
  }

  .stats-panel {
    padding: 0.5rem 0;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(10, 15, 25, 0.6);
    backdrop-filter: blur(20px);
    overflow: hidden;
    position: relative;
  }

  .stats-panel::after {
    content: "";
    position: absolute;
    bottom: 0;
    right: 0;
    width: 20px;
    height: 20px;
    background: linear-gradient(
      135deg,
      transparent 50%,
      rgba(0, 243, 255, 0.2) 50%
    );
    pointer-events: none;
  }

  .info-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.9rem 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
    transition: all 0.2s;
  }

  .info-row:hover {
    background: rgba(0, 243, 255, 0.05);
    padding-left: 1.75rem;
  }

  .info-row:last-child {
    border-bottom: none;
  }

  .info-row .label {
    font-size: 0.65rem;
    font-weight: 800;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.15em;
    font-family: var(--font-mono, monospace);
  }

  .info-row .value {
    font-size: 0.9rem;
    font-weight: 700;
    color: var(--text-primary);
    font-family: var(--font-mono, monospace);
  }

  .external-links {
    display: flex;
    justify-content: center;
    gap: 1.5rem;
    padding: 1.5rem;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(0, 0, 0, 0.2);
  }

  .link-icon {
    width: 40px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0.5;
    transition: all 0.3s;
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

  /* Collection Banner */
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
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: #000;
    overflow: hidden;
    cursor: pointer;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    text-align: left;
    clip-path: polygon(
      0% 0%,
      calc(100% - 20px) 0%,
      100% 20px,
      100% 100%,
      20px 100%,
      0% calc(100% - 20px)
    );
  }

  .collection-banner:hover {
    border-color: var(--color-primary);
    transform: translateY(-2px);
    box-shadow:
      0 10px 40px rgba(0, 243, 255, 0.15),
      0 0 20px rgba(0, 243, 255, 0.05);
  }

  .banner-bg {
    position: absolute;
    inset: 0;
    background-size: cover;
    background-position: center 30%;
    opacity: 0.65;
    transition: transform 1.2s cubic-bezier(0.23, 1, 0.32, 1);
    filter: saturate(1.2) contrast(1.1);
  }

  .collection-banner:hover .banner-bg {
    transform: scale(1.1);
    opacity: 0.8;
  }

  .banner-overlay {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      rgba(1, 2, 3, 0.98) 0%,
      rgba(1, 2, 3, 0.8) 35%,
      rgba(0, 243, 255, 0.05) 100%
    );
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
    gap: 0.4rem;
  }

  .banner-tag {
    font-size: 0.6rem;
    color: var(--color-primary);
    text-transform: uppercase;
    letter-spacing: 0.25em;
    font-weight: 800;
    font-family: var(--font-mono, monospace);
    opacity: 0.9;
  }

  .collection-title {
    margin: 0;
    font-size: 1.8rem;
    font-weight: 900;
    color: #fff;
    text-transform: uppercase;
    letter-spacing: 0.02em;
    text-shadow: 0 4px 12px rgba(0, 0, 0, 0.8);
  }

  .banner-view-btn {
    background: rgba(0, 0, 0, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.15);
    padding: 0.6rem 1.4rem;
    border-radius: 50px;
    font-size: 0.65rem;
    font-weight: 900;
    letter-spacing: 0.15em;
    color: #fff;
    transition: all 0.3s;
    backdrop-filter: blur(10px);
  }

  .collection-banner:hover .banner-view-btn {
    background: var(--color-primary);
    color: #000;
    border-color: var(--color-primary);
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.4);
  }

  /* Richer Stats Styling */
  .info-section-label {
    padding: 1rem 1.5rem 0.5rem;
    font-size: 0.6rem;
    font-weight: 900;
    color: var(--color-primary);
    text-transform: uppercase;
    letter-spacing: 0.25em;
    background: rgba(0, 243, 255, 0.05);
    border-top: 1px solid rgba(0, 243, 255, 0.1);
  }

  .info-row .sub-label {
    font-weight: 500;
    opacity: 0.8;
    padding-left: 0.5rem;
  }

  /* Keywords Panel */
  .keywords-panel {
    margin-top: 1.5rem;
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    padding: 0.5rem;
  }

  .keyword-tag {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    padding: 0.35rem 0.75rem;
    font-size: 0.65rem;
    font-weight: 600;
    color: var(--text-muted);
    transition: all 0.2s;
    cursor: default;
    clip-path: polygon(
      4px 0%,
      100% 0%,
      100% calc(100% - 4px),
      calc(100% - 4px) 100%,
      0% 100%,
      0% 4px
    );
  }

  .keyword-tag:hover {
    background: rgba(0, 243, 255, 0.1);
    color: var(--color-primary);
    border-color: rgba(0, 243, 255, 0.3);
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
