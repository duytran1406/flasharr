<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import {
    getTVShowDetails,
    getSeasonDetails,
    getYear,
    getSimilar,
    getRecommendations,
    resolveTvRoute,
    type TMDBEpisode,
    type TMDBTVShowDetails,
    type TMDBTVShow,
    type TMDBSeasonDetails,
  } from "$lib/services/tmdb";
  import { toasts } from "$lib/stores/toasts";
  import { ui } from "$lib/stores/ui.svelte";
  import {
    fetchAllSeries,
    fetchEpisodesBySonarrId,
    findSeriesInList,
    type SonarrSeries,
    type SonarrEpisode,
  } from "$lib/stores/arr";
  import TVDetailHero from "$lib/components/media/TVDetailHero.svelte";
  import TVActionPanel from "$lib/components/media/TVActionPanel.svelte";
  import TVEpisodeList from "$lib/components/media/TVEpisodeList.svelte";
  import RelatedMediaSection from "$lib/components/media/RelatedMediaSection.svelte";

  // The /tv/:id route is keyed by TVDB id; legacy TMDB ids are resolved and redirected.
  const routeId = $derived(page.params.id as string);

  let tv = $state<TMDBTVShowDetails | null>(null);
  let canonicalTvdbId = $state<number | null>(null);
  let legacyTmdbId = $state<number | null>(null);
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
  let sonarrEpisodes = $state<SonarrEpisode[]>([]);
  let inLibrary = $derived(librarySeries !== null);
  let addingToLibrary = $state(false);

  async function loadData() {
    loading = true;
    try {
      const route = await resolveTvRoute(routeId);
      if (route) {
        canonicalTvdbId = route.canonical_tvdb_id;
        legacyTmdbId = route.legacy_tmdb_id;
        if (!route.is_canonical_tv_route && canonicalTvdbId) {
          goto(`/tv/${canonicalTvdbId}`, { replaceState: true, noScroll: true });
        }
      }

      const tmdbId = route?.legacy_tmdb_id ?? Number(routeId);
      const tvData = await getTVShowDetails(tmdbId);
      if (tvData) {
        tv = tvData;
        const firstSeason =
          tvData.seasons.find((s) => s.season_number > 0) || tvData.seasons[0];
        if (firstSeason) selectedSeason = firstSeason.season_number;

        const [similarData, recommendedData] = await Promise.all([
          getSimilar("tv", tmdbId),
          getRecommendations("tv", tmdbId),
        ]);
        similar = similarData.slice(0, 4);
        recommended = recommendedData.slice(0, 4);
      }

      // Library lookup (non-fatal)
      try {
        const allSeries = await fetchAllSeries();
        const match = findSeriesInList(
          allSeries,
          canonicalTvdbId,
          legacyTmdbId ?? Number(routeId),
        );
        if (match) {
          librarySeries = match;
          sonarrEpisodes = await fetchEpisodesBySonarrId(match.id);
          applySonarrSeasons(sonarrEpisodes);
          if (selectedSeason !== null) applySonarrSeason(selectedSeason);
        }
      } catch {
        // Library is optional
      }
    } catch (error) {
      console.error("Failed to load TV data:", error);
      toasts.error("Failed to load TV intelligence brief");
    } finally {
      loading = false;
    }
  }

  async function loadSeason(seasonNumber: number) {
    loadingSeason = true;
    try {
      const fromSonarr = applySonarrSeason(seasonNumber);
      const tmdbSeason = await getSeasonDetails(
        legacyTmdbId ?? routeId,
        seasonNumber,
      );
      if (fromSonarr) {
        if (tmdbSeason) enrichSeasonFromTmdb(tmdbSeason);
        return;
      }
      seasonDetails = tmdbSeason;
    } catch (error) {
      console.error("Failed to load season:", error);
      toasts.error(`Failed to load Season ${seasonNumber}`);
    } finally {
      loadingSeason = false;
    }
  }

  /** Fill stills/overviews missing from the Sonarr episode list with TMDB data. */
  function enrichSeasonFromTmdb(tmdbSeason: TMDBSeasonDetails) {
    if (!seasonDetails || tmdbSeason.episodes.length === 0) return;
    const byNumber = new Map<number, TMDBEpisode>();
    for (const episode of tmdbSeason.episodes) {
      byNumber.set(episode.episode_number, episode);
    }
    seasonDetails = {
      ...seasonDetails,
      episodes: seasonDetails.episodes.map((episode) => {
        const tmdbEpisode = byNumber.get(episode.episode_number);
        if (!tmdbEpisode) return episode;
        return {
          ...episode,
          still_path: episode.still_path ?? tmdbEpisode.still_path,
          overview:
            episode.overview && episode.overview.trim()
              ? episode.overview
              : tmdbEpisode.overview,
        };
      }),
    };
  }

  /** Sonarr (TVDB ordering) is authoritative for the season list once the series is in the library. */
  async function applySonarrSeasons(episodes: SonarrEpisode[]) {
    if (!tv || episodes.length === 0) return;
    const bySeason = new Map<number, SonarrEpisode[]>();
    for (const episode of episodes) {
      if (episode.seasonNumber > 0) {
        const list = bySeason.get(episode.seasonNumber) ?? [];
        list.push(episode);
        bySeason.set(episode.seasonNumber, list);
      }
    }
    if (bySeason.size === 0) return;

    const seasons = [...bySeason.entries()]
      .sort(([a], [b]) => a - b)
      .map(([seasonNumber, list]) => ({
        id: seasonNumber,
        name: `Season ${seasonNumber}`,
        overview: "",
        poster_path: null,
        season_number: seasonNumber,
        episode_count: list.length,
      }));

    tv = {
      ...tv,
      seasons,
      number_of_seasons: seasons.length,
      number_of_episodes: [...bySeason.values()].reduce(
        (sum, list) => sum + list.length,
        0,
      ),
    } as TMDBTVShowDetails;
  }

  /** Build the season view from Sonarr episodes; returns false when Sonarr has none. */
  function applySonarrSeason(seasonNumber: number): boolean {
    if (sonarrEpisodes.length === 0) return false;
    const episodes = sonarrEpisodes
      .filter((ep) => ep.seasonNumber === seasonNumber)
      .sort((a, b) => a.episodeNumber - b.episodeNumber)
      .map((ep) => ({
        id: ep.id,
        name: ep.title || `Episode ${ep.episodeNumber}`,
        overview: ep.overview || "",
        still_path: null,
        air_date: ep.airDateUtc || "",
        episode_number: ep.episodeNumber,
        season_number: ep.seasonNumber,
        vote_average: 0,
      }));
    if (episodes.length === 0) return false;
    seasonDetails = {
      id: seasonNumber,
      name: `Season ${seasonNumber}`,
      overview: "",
      poster_path: null,
      season_number: seasonNumber,
      episodes,
    } as TMDBSeasonDetails;
    return true;
  }

  async function handleAddToLibrary() {
    if (!tv || addingToLibrary) return;
    addingToLibrary = true;
    try {
      const res = await fetch("/api/arr/series/add", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          tvdb_id: canonicalTvdbId ?? tv?.external_ids?.tvdb_id ?? null,
          tmdb_id: legacyTmdbId ?? tv?.id ?? Number(routeId),
        }),
      });
      if (res.ok) {
        const data = await res.json();
        toasts.success(`"${tv.name}" added to Sonarr library`);
        librarySeries = { id: data.arr_id, title: tv.name } as SonarrSeries;
      } else if (res.status === 409) {
        toasts.info(`"${tv.name}" is already in your Sonarr library`);
      } else {
        toasts.error(`Failed to add to library: ${await res.text()}`);
      }
    } catch {
      toasts.error("Network error — could not reach server");
    } finally {
      addingToLibrary = false;
    }
  }

  function tvdbIdForSearch(show: TMDBTVShowDetails): string | undefined {
    if (canonicalTvdbId) return String(canonicalTvdbId);
    return show.external_ids?.tvdb_id
      ? String(show.external_ids.tvdb_id)
      : undefined;
  }

  function handleSmartSearch() {
    if (!tv) return;
    ui.openSmartSearch(
      {
        tmdbId: String(legacyTmdbId ?? tv.id),
        tvdbId: tvdbIdForSearch(tv),
        type: "tv",
        title: tv.name,
        year: getYear(tv.first_air_date) ?? undefined,
        season: selectedSeason || 1,
      },
      { mode: "season" },
    );
  }

  function handleEpisodeSearch(episode: TMDBEpisode) {
    if (!tv) return;
    ui.openSmartSearch(
      {
        tmdbId: String(legacyTmdbId ?? tv.id),
        tvdbId: tvdbIdForSearch(tv),
        type: "tv",
        title: tv.name,
        year: getYear(tv.first_air_date) ?? undefined,
        season: episode.season_number,
        episode: episode.episode_number,
      },
      { mode: "target" },
    );
  }

  /** true = has file, false = missing, null = unknown / not in library. */
  function getEpisodeAcquisition(
    seasonNum: number,
    episodeNum: number,
  ): boolean | null {
    if (!inLibrary || sonarrEpisodes.length === 0) return null;
    const ep = sonarrEpisodes.find(
      (e) => e.seasonNumber === seasonNum && e.episodeNumber === episodeNum,
    );
    return ep ? ep.hasFile : null;
  }

  let libraryProgress = $derived.by(() => {
    if (!librarySeries?.statistics) return null;
    const stats = librarySeries.statistics;
    if (!stats.episodeCount || stats.episodeCount === 0) return null;
    return {
      acquired: stats.episodeFileCount || 0,
      total: stats.episodeCount,
      pct: Math.round(((stats.episodeFileCount || 0) / stats.episodeCount) * 100),
      sizeOnDisk: stats.sizeOnDisk || 0,
    };
  });

  let contentRating = $derived.by(() => {
    if (!tv?.content_ratings) return "N/A";
    const us = tv.content_ratings.results.find((r) => r.iso_3166_1 === "US");
    return us ? us.rating : "NR";
  });

  let tags = $derived(tv?.keywords?.results || []);

  $effect(() => {
    if (routeId) {
      posterLoaded = false;
      backdropLoaded = false;
      loadData();
    }
  });

  $effect(() => {
    if (!loading && selectedSeason !== null && tv) {
      loadSeason(selectedSeason);
    }
  });

  function goBack() {
    window.history.back();
  }
</script>

<div class="media-detail-view" data-view="tv">
  <div class="detail-container">
    <button class="detail-back" type="button" onclick={goBack}>
      <span class="material-icons" aria-hidden="true">arrow_back</span>
      Back
    </button>

    <TVDetailHero
      {tv}
      {loading}
      {libraryProgress}
      bind:posterLoaded
      bind:backdropLoaded
    />

    <div class="detail-grid">
      <div class="main-content">
        <div class="mobile-action-panel mobile-action-panel-first">
          <TVActionPanel
            {loading}
            {inLibrary}
            {addingToLibrary}
            {librarySeries}
            {libraryProgress}
            {contentRating}
            {tags}
            {tv}
            compact
            onSmartSearch={handleSmartSearch}
            onAddToLibrary={handleAddToLibrary}
          />
        </div>

        <section class="overview-section">
          <h3 class="section-label">Overview</h3>
          {#if loading}
            <div class="skeleton skeleton-text"></div>
            <div class="skeleton skeleton-text"></div>
            <div class="skeleton skeleton-text" style="width:60%"></div>
          {:else if tv}
            <p class="overview-text">
              {tv.overview || "No overview available."}
            </p>
          {/if}
        </section>

        <TVEpisodeList
          {tv}
          {loading}
          {loadingSeason}
          {seasonDetails}
          onSeasonChange={loadSeason}
          onEpisodeSearch={handleEpisodeSearch}
          {getEpisodeAcquisition}
          bind:selectedSeason
        />

        <div class="mobile-action-panel mobile-action-panel-late">
          <TVActionPanel
            {loading}
            {inLibrary}
            {addingToLibrary}
            {librarySeries}
            {libraryProgress}
            {contentRating}
            {tags}
            {tv}
            compact
            onSmartSearch={handleSmartSearch}
            onAddToLibrary={handleAddToLibrary}
          />
        </div>

        <RelatedMediaSection
          title="Similar Titles"
          items={similar.map((s) => ({
            ...s,
            title: s.name,
            release_date: s.first_air_date,
          }))}
          mediaType="tv"
          {loading}
        />

        {#if loading || recommended.length > 0}
          <RelatedMediaSection
            title="Recommended for You"
            items={recommended.map((r) => ({
              ...r,
              title: r.name,
              release_date: r.first_air_date,
            }))}
            mediaType="tv"
            {loading}
          />
        {/if}
      </div>

      <TVActionPanel
        {loading}
        {inLibrary}
        {addingToLibrary}
        {librarySeries}
        {libraryProgress}
        {contentRating}
        {tags}
        {tv}
        onSmartSearch={handleSmartSearch}
        onAddToLibrary={handleAddToLibrary}
      />
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
  .detail-container {
    display: flex;
    flex-direction: column;
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
  .overview-section {
    margin-bottom: 0;
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
  .skeleton-text {
    width: 100%;
    height: 1rem;
    margin-bottom: .75rem;
  }
  .mobile-action-panel {
    display: none;
  }
  @media (max-width: 1024px) {
    .detail-grid {
      grid-template-columns: 1fr;
    }
  }
  @media (max-width: 768px) {
    .detail-back {
      margin: .5rem 1rem 0;
    }
    .detail-grid {
      grid-template-columns: 1fr;
      gap: 1rem;
      padding: 2rem 1rem;
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
  }
  @media (max-width: 480px) {
    .detail-grid {
      padding: 1.5rem .75rem;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .skeleton:after {
      animation: none;
    }
  }
</style>
