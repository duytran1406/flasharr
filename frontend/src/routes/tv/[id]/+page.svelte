<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import {
    getTVShowDetails,
    getSeasonDetails,
    getPosterUrl,
    getBackdropUrl,
    getYear,
    getSimilar,
    getRecommendations,
    type TMDBTVShowDetails,
    type TMDBTVShow,
    type TMDBSeasonDetails,
  } from "$lib/services/tmdb";
  import { toasts } from "$lib/stores/toasts";
  import { ui } from "$lib/stores/ui.svelte";
  import { MediaCard } from "$lib/components";
  import Button from "$lib/components/ui/Button.svelte";
  import {
    fetchAllSeries,
    fetchEpisodesBySonarrId,
    formatDiskSize,
    findSeriesInList,
    type SonarrSeries,
    type SonarrEpisode,
  } from "$lib/stores/arr";

  const tvId = $derived(page.params.id as string);

  let tv = $state<TMDBTVShowDetails | null>(null);
  let similar = $state<TMDBTVShow[]>([]);
  let recommended = $state<TMDBTVShow[]>([]);
  let selectedSeason = $state<number | null>(null);
  let seasonDetails = $state<TMDBSeasonDetails | null>(null);
  let loading = $state(true);
  let loadingSeason = $state(false);
  let posterLoaded = $state(false);
  let backdropLoaded = $state(false);
  // Library integration
  let librarySeries = $state<SonarrSeries | null>(null);
  let libraryEpisodes = $state<SonarrEpisode[]>([]);
  let inLibrary = $derived(librarySeries !== null);
  let isUpcoming = $derived(
    !tv ||
      tv.status === "Planned" ||
      tv.status === "In Production" ||
      (tv.first_air_date && new Date(tv.first_air_date) > new Date()),
  );

  async function loadData() {
    loading = true;
    try {
      const details = await getTVShowDetails(tvId);
      if (details) {
        tv = details;
        // Find first real season (usually season 1, not specials)
        const firstSeason =
          details.seasons.find((s) => s.season_number > 0) ||
          details.seasons[0];
        if (firstSeason) {
          selectedSeason = firstSeason.season_number;
          await loadSeason(firstSeason.season_number);
        }

        const [sim, reco] = await Promise.all([
          getSimilar("tv", tvId),
          getRecommendations("tv", tvId),
        ]);
        similar = sim.slice(0, 4);
        recommended = reco.slice(0, 4);
      }

      // Parallel: fetch Sonarr library data
      try {
        const allSeries = await fetchAllSeries();
        const tmdbIdNum = Number(tvId);
        const match = findSeriesInList(allSeries, tmdbIdNum);
        if (match) {
          librarySeries = match;
          libraryEpisodes = await fetchEpisodesBySonarrId(match.id);
        }
      } catch {
        // Library lookup is best-effort
      }
    } catch (error) {
      console.error("Failed to load TV data:", error);
      toasts.error("Failed to load TV intelligence brief");
    } finally {
      loading = false;
    }
  }

  async function loadSeason(num: number) {
    loadingSeason = true;
    try {
      const details = await getSeasonDetails(tvId, num);
      seasonDetails = details;
    } catch (error) {
      console.error("Failed to load season:", error);
      toasts.error(`Failed to load Season ${num}`);
    } finally {
      loadingSeason = false;
    }
  }

  function handleSmartSearch() {
    if (!tv) return;
    ui.openSmartSearch({
      tmdbId: String(tv.id),
      type: "tv",
      title: tv.name,
      year: getYear(tv.first_air_date) ?? undefined,
      season: selectedSeason || 1,
    });
  }

  function handleEpisodeSearch(ep: any) {
    if (!tv) return;
    ui.openSmartSearch({
      tmdbId: String(tv.id),
      type: "tv",
      title: tv.name,
      year: getYear(tv.first_air_date) ?? undefined,
      season: ep.season_number,
      episode: ep.episode_number,
    });
  }

  function isEpisodeAired(ep: any): boolean {
    if (!ep.air_date) return false;
    return new Date(ep.air_date) <= new Date();
  }

  // Get Keywords
  let tags = $derived(tv?.keywords?.results || []);

  // Format Score for UI
  function formatScore(score: number) {
    return Math.round(score * 10);
  }

  // Library helpers
  function getEpisodeAcquisition(
    seasonNum: number,
    epNum: number,
  ): boolean | null {
    if (!inLibrary || libraryEpisodes.length === 0) return null;
    const match = libraryEpisodes.find(
      (e) => e.seasonNumber === seasonNum && e.episodeNumber === epNum,
    );
    return match ? match.hasFile : null;
  }

  let libraryProgress = $derived.by(() => {
    if (!librarySeries?.statistics) return null;
    const stats = librarySeries.statistics;
    if (!stats.episodeCount || stats.episodeCount === 0) return null;
    return {
      acquired: stats.episodeFileCount || 0,
      total: stats.episodeCount,
      pct: Math.round(
        ((stats.episodeFileCount || 0) / stats.episodeCount) * 100,
      ),
      sizeOnDisk: stats.sizeOnDisk || 0,
    };
  });

  // Get Certification/Rating
  let contentRating = $derived.by(() => {
    if (!tv?.content_ratings) return "N/A";
    const us = tv.content_ratings.results.find((r) => r.iso_3166_1 === "US");
    return us ? us.rating : "NR";
  });

  $effect(() => {
    if (tvId) {
      posterLoaded = false;
      backdropLoaded = false;
      loadData();
    }
  });

  $effect(() => {
    if (selectedSeason !== null && tv) {
      loadSeason(selectedSeason);
    }
  });
</script>

<div class="media-detail-view" data-view="tv">
  <div class="detail-container">
    <!-- Hero Section -->
    <div
      class="detail-hero"
      class:skeleton-bg={loading}
      style="background-image: url(/images/placeholder-banner.png)"
    >
      {#if tv?.backdrop_path}
        <div
          class="backdrop-real"
          class:loaded={backdropLoaded}
          style={`background-image: url(${getBackdropUrl(tv.backdrop_path, "original")})`}
        ></div>
        <img
          src={getBackdropUrl(tv.backdrop_path, "original")}
          alt=""
          class="hidden-loader"
          onload={() => (backdropLoaded = true)}
        />
      {/if}
      <div class="hero-overlay"></div>
      <div class="hero-content">
        <div class="poster-container glass-panel" class:skeleton={loading}>
          <img
            src={tv?.poster_path
              ? getPosterUrl(tv.poster_path, "w500")
              : "/images/placeholder-poster.svg"}
            alt={tv?.name || "Unknown Subject"}
            class="detail-poster"
            class:loaded={posterLoaded || !tv?.poster_path}
            onload={() => (posterLoaded = true)}
          />
        </div>
        <div class="hero-info">
          {#if loading}
            <div class="skeleton skeleton-tagline"></div>
            <div class="skeleton skeleton-title"></div>
            <div class="skeleton skeleton-meta"></div>
          {:else if tv}
            {#if tv.tagline}
              <div class="detail-tagline">{tv.tagline}</div>
            {/if}
            <h1 class="detail-title">{tv.name}</h1>
            <div class="detail-meta">
              <span class="meta-year">{getYear(tv.first_air_date)}</span>
              <span class="meta-divider">•</span>
              <span class="meta-rating">
                <span class="material-icons">star</span>
                {tv.vote_average.toFixed(1)}
              </span>
              <span class="meta-divider">•</span>
              <span class="meta-runtime">
                {tv.number_of_seasons}
                {tv.number_of_seasons === 1 ? "Season" : "Seasons"}
              </span>
              <span class="meta-divider">•</span>
              <span class="meta-genres">
                {tv.genres
                  .slice(0, 3)
                  .map((g) => g.name)
                  .join(" / ")}
              </span>
            </div>
          {/if}
        </div>
      </div>
      <!-- Library Progress Bar (overlaid on hero) -->
      {#if libraryProgress}
        <div class="library-progress-strip">
          <div class="lp-bar">
            <div class="lp-fill" style="width: {libraryProgress.pct}%"></div>
          </div>
          <div class="lp-stats">
            <span class="lp-label">IN LIBRARY</span>
            <span class="lp-value"
              >{libraryProgress.acquired}/{libraryProgress.total} episodes</span
            >
            <span class="lp-size"
              >{formatDiskSize(libraryProgress.sizeOnDisk)}</span
            >
          </div>
        </div>
      {/if}
    </div>

    <!-- Content Grid -->
    <div class="detail-grid">
      <!-- Main Content -->
      <div class="main-content">
        <section class="overview-section">
          <h3 class="section-label">Overview</h3>
          {#if loading}
            <div class="skeleton skeleton-text"></div>
            <div class="skeleton skeleton-text"></div>
            <div class="skeleton skeleton-text" style="width: 60%"></div>
          {:else if tv}
            <p class="overview-text">
              {tv.overview || "No overview available."}
            </p>
          {/if}
        </section>

        <!-- Seasons Section -->
        <section class="seasons-section">
          <div class="section-header">
            <h3 class="section-label">Seasons</h3>
            {#if !loading && tv && tv.seasons.length > 1}
              <select
                class="season-selector glass-panel"
                bind:value={selectedSeason}
              >
                {#each tv.seasons.filter((s) => s.season_number > 0) as s}
                  <option value={s.season_number}
                    >Season {s.season_number}</option
                  >
                {/each}
              </select>
            {/if}
          </div>

          <div class="episodes-list">
            {#if loading || loadingSeason}
              {#each Array(3) as _}
                <div class="episode-card glass-panel skeleton-bg">
                  <div class="episode-thumbnail skeleton"></div>
                  <div class="episode-info">
                    <div
                      class="skeleton skeleton-title"
                      style="height: 1.5rem; width: 60%"
                    ></div>
                    <div
                      class="skeleton skeleton-text"
                      style="margin-top: 1rem"
                    ></div>
                    <div class="skeleton skeleton-text"></div>
                  </div>
                </div>
              {/each}
            {:else if seasonDetails}
              {#each seasonDetails.episodes as ep}
                <div
                  class="episode-card glass-panel"
                  class:episode-unreleased={!isEpisodeAired(ep)}
                >
                  <div class="episode-thumbnail">
                    <img
                      src={getPosterUrl(ep.still_path, "w500") ||
                        "/images/placeholder-poster.svg"}
                      alt={ep.name}
                    />
                    <div class="episode-badge">EP {ep.episode_number}</div>
                    {#if !isEpisodeAired(ep)}
                      <div class="coming-soon-overlay">
                        <span class="material-icons">schedule</span>
                      </div>
                    {/if}
                  </div>
                  <div class="episode-info">
                    <div class="episode-header">
                      <div class="episode-title-row">
                        <h4 class="episode-name">{ep.name}</h4>
                        {#if isEpisodeAired(ep)}
                          <span class="episode-date"
                            >{new Date(ep.air_date).toLocaleDateString()}</span
                          >
                        {:else}
                          <span class="coming-soon-tag">
                            <span
                              class="material-icons"
                              style="font-size: 0.7rem;">schedule</span
                            >
                            {ep.air_date
                              ? new Date(ep.air_date).toLocaleDateString()
                              : "TBA"}
                          </span>
                        {/if}
                      </div>
                      <div class="episode-actions">
                        {#if getEpisodeAcquisition(ep.season_number, ep.episode_number) === true}
                          <span class="ep-acquired-badge" title="In library">
                            <span class="material-icons">check_circle</span>
                          </span>
                        {:else if getEpisodeAcquisition(ep.season_number, ep.episode_number) === false}
                          <span class="ep-missing-badge" title="Missing">
                            <span class="material-icons">cancel</span>
                          </span>
                        {/if}
                        {#if isEpisodeAired(ep)}
                          <button
                            class="icon-btn-tiny"
                            title="Search this episode"
                            onclick={() => handleEpisodeSearch(ep)}
                          >
                            <span class="material-icons">manage_search</span>
                          </button>
                        {/if}
                      </div>
                    </div>
                    <p class="episode-overview">
                      {ep.overview || "No overview available."}
                    </p>
                  </div>
                </div>
              {/each}
            {/if}
          </div>
        </section>

        <!-- Similar Titles -->
        <section class="related-section">
          <h3 class="section-label">Similar Titles</h3>
          <div
            class="related-grid"
            role="status"
            aria-label="Loading similar titles"
          >
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
                  title={item.name}
                  posterPath={item.poster_path}
                  voteAverage={item.vote_average}
                  releaseDate={item.first_air_date}
                  overview={item.overview}
                  mediaType="tv"
                />
              {/each}
            {/if}
          </div>
        </section>

        <!-- Recommendations -->
        {#if loading || recommended.length > 0}
          <section class="related-section">
            <h3 class="section-label">Recommended for You</h3>
            <div
              class="related-grid"
              role="status"
              aria-label="Loading recommended titles"
            >
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
                    title={item.name}
                    posterPath={item.poster_path}
                    voteAverage={item.vote_average}
                    releaseDate={item.first_air_date}
                    overview={item.overview}
                    mediaType="tv"
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
            <div class="library-badge">
              <span class="material-icons">video_library</span>
              IN LIBRARY
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
              disabled>Add to Library</Button
            >
          {/if}
        </div>

        <div class="stats-panel glass-panel">
          {#if loading}
            {#each Array(4) as _}
              <div class="info-row">
                <div
                  class="skeleton skeleton-small-text"
                  style="width: 30%"
                ></div>
              </div>
            {/each}
          {:else if tv}
            <div class="info-row">
              <span class="label">Status</span>
              <span class="value" style="color: var(--color-primary)"
                >{tv.status}</span
              >
            </div>

            <div class="info-section-label">Broadcast Intelligence</div>
            <div class="info-row">
              <span class="label">First Air</span>
              <span class="value"
                >{new Date(tv.first_air_date).toLocaleDateString()}</span
              >
            </div>
            <div class="info-row">
              <span class="label">Rating</span>
              <span class="value" style="color: #ffcc00">{contentRating}</span>
            </div>
            <div class="info-row">
              <span class="label">Seasons</span>
              <span class="value">{tv.number_of_seasons}</span>
            </div>
            <div class="info-row">
              <span class="label">Episodes</span>
              <span class="value">{tv.number_of_episodes}</span>
            </div>
            {#if tv.episode_run_time.length > 0}
              <div class="info-row">
                <span class="label">Runtime</span>
                <span class="value">{tv.episode_run_time[0]}m</span>
              </div>
            {/if}

            <div class="info-section-label">Intelligence Scores</div>
            <div class="info-row">
              <span class="label">TMDB SCORE</span>
              <span class="value">{formatScore(tv.vote_average)}%</span>
            </div>
            {#if tv.external_ids?.imdb_id}
              <div class="info-row">
                <span class="label">IMDB SCORE</span>
                <span class="value">{tv.vote_average.toFixed(1)}</span>
              </div>
            {/if}

            <div class="info-row">
              <span class="label">Language</span>
              <span class="value">{tv.original_language.toUpperCase()}</span>
            </div>

            <div class="external-links">
              <a
                href="https://www.themoviedb.org/tv/{tv.id}"
                target="_blank"
                class="link-icon"
                title="TMDB"
              >
                <img
                  src="https://www.themoviedb.org/assets/2/v4/logos/v2/blue_square_2-d537fb228cf3ded904ef09b136fe3fec72548ebc1fea3fbbd1ad9e36364db38b.svg"
                  alt="TMDB"
                />
              </a>
              {#if tv.external_ids?.imdb_id}
                <a
                  href="https://www.imdb.com/title/{tv.external_ids.imdb_id}"
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

            {#if inLibrary && librarySeries}
              <div class="info-section-label">Library Data</div>
              <div class="info-row">
                <span class="label">Monitored</span>
                <span
                  class="value"
                  style="color: {librarySeries.monitored
                    ? '#34d399'
                    : '#94a3b8'}"
                >
                  {librarySeries.monitored ? "Yes" : "No"}
                </span>
              </div>
              {#if libraryProgress}
                <div class="info-row">
                  <span class="label">Acquired</span>
                  <span class="value"
                    >{libraryProgress.acquired}/{libraryProgress.total}</span
                  >
                </div>
                <div class="info-row">
                  <span class="label">On Disk</span>
                  <span class="value"
                    >{formatDiskSize(libraryProgress.sizeOnDisk)}</span
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

  /* Seasons Section */
  .seasons-section {
    margin-bottom: 4rem;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1.5rem;
  }

  .season-selector {
    padding: 0.5rem 1rem;
    background: rgba(255, 255, 255, 0.05);
    color: #fff;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    font-size: 0.85rem;
    outline: none;
    cursor: pointer;
  }

  .episodes-list {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .episode-card {
    display: flex;
    gap: 1.5rem;
    padding: 1rem;
    border-radius: 12px;
    transition: transform 0.2s;
    background: rgba(255, 255, 255, 0.02);
  }

  .episode-card:hover {
    background: rgba(255, 255, 255, 0.04);
    transform: translateX(5px);
  }

  .episode-thumbnail {
    width: 200px;
    aspect-ratio: 16/9;
    flex-shrink: 0;
    border-radius: 8px;
    overflow: hidden;
    background: #000;
    position: relative;
  }

  .episode-thumbnail img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0.7;
  }

  .episode-badge {
    position: absolute;
    bottom: 0.5rem;
    left: 0.5rem;
    background: rgba(0, 0, 0, 0.8);
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 0.7rem;
    font-weight: 800;
    font-family: var(--font-mono);
  }

  .episode-info {
    flex: 1;
    min-width: 0;
  }

  .episode-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 0.75rem;
  }

  .episode-title-row {
    flex: 1;
  }

  .episode-name {
    font-size: 1.1rem;
    font-weight: 700;
    margin: 0 0 0.25rem;
    color: var(--text-primary);
  }

  .episode-date {
    font-size: 0.75rem;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }

  .episode-overview {
    font-size: 0.9rem;
    line-height: 1.5;
    color: var(--text-secondary);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  /* Unreleased Episode Styles */
  .episode-unreleased {
    opacity: 0.5;
    position: relative;
  }

  .episode-unreleased .episode-thumbnail img {
    filter: grayscale(0.7);
  }

  .coming-soon-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.5);
    border-radius: 12px;
  }

  .coming-soon-overlay .material-icons {
    font-size: 2rem;
    color: rgba(255, 255, 255, 0.7);
  }

  .coming-soon-tag {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.7rem;
    font-family: var(--font-mono);
    color: var(--accent-warning, #f59e0b);
    background: rgba(245, 158, 11, 0.1);
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    border: 1px solid rgba(245, 158, 11, 0.2);
  }

  /* Related Sections */
  .related-section {
    margin-top: 4rem;
  }

  .related-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 1.5rem;
  }

  /* Sidebar */
  .detail-sidebar {
    position: relative;
    z-index: 2;
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
    .hero-content {
      gap: 1.5rem;
    }
    .poster-container {
      width: 160px;
    }
    .detail-title {
      font-size: 2.5rem;
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
    .episode-card {
      flex-direction: column;
    }
    .episode-thumbnail {
      width: 100%;
    }
  }

  /* Library Integration Styles */
  .library-progress-strip {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    padding: 0.75rem 2rem;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    gap: 1.5rem;
    z-index: 2;
  }

  .lp-bar {
    flex: 1;
    height: 4px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 2px;
    overflow: hidden;
  }

  .lp-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--color-primary), #34d399);
    border-radius: 2px;
    transition: width 0.6s ease;
  }

  .lp-stats {
    display: flex;
    align-items: center;
    gap: 1rem;
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: 0.7rem;
  }

  .lp-label {
    color: var(--color-primary);
    font-weight: 800;
    letter-spacing: 0.1em;
  }

  .lp-value {
    color: var(--text-secondary);
  }

  .lp-size {
    color: var(--text-muted);
  }

  .library-badge {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 1rem;
    background: rgba(52, 211, 153, 0.1);
    border: 1px solid rgba(52, 211, 153, 0.3);
    border-radius: 8px;
    color: #34d399;
    font-size: 0.75rem;
    font-weight: 800;
    letter-spacing: 0.1em;
    font-family: var(--font-mono);
    margin-bottom: 0.5rem;
  }

  .library-badge .material-icons {
    font-size: 1rem;
  }

  .add-library-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    width: 100%;
    padding: 0.85rem 1.5rem;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.15em;
    cursor: not-allowed;
    opacity: 0.5;
    margin-top: 0.5rem;
  }

  .episode-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .ep-acquired-badge {
    color: #34d399;
    display: flex;
    align-items: center;
  }

  .ep-acquired-badge .material-icons {
    font-size: 1.1rem;
  }

  .ep-missing-badge {
    color: #ef4444;
    display: flex;
    align-items: center;
    opacity: 0.6;
  }

  .ep-missing-badge .material-icons {
    font-size: 1.1rem;
  }
</style>
