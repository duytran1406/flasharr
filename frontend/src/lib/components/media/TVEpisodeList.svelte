<script lang="ts">
  import { Badge } from "@media-set/core-ui";
  import {
    getPosterUrl,
    type TMDBEpisode,
    type TMDBSeasonDetails,
    type TMDBTVShowDetails,
  } from "$lib/services/tmdb";

  interface Props {
    tv: TMDBTVShowDetails | null;
    seasonDetails: TMDBSeasonDetails | null;
    selectedSeason?: number;
    loading: boolean;
    loadingSeason: boolean;
    onSeasonChange: (season: number) => void;
    onEpisodeSearch: (episode: TMDBEpisode) => void;
    /** true = in library, false = missing, null/undefined = unknown */
    getEpisodeAcquisition: (season: number, episode: number) => boolean | null | undefined;
  }

  let {
    tv,
    seasonDetails,
    selectedSeason = $bindable(),
    loading,
    loadingSeason,
    onSeasonChange,
    onEpisodeSearch,
    getEpisodeAcquisition,
  }: Props = $props();

  function isReleased(episode: TMDBEpisode): boolean {
    if (!episode.air_date) return false;
    return new Date(episode.air_date) <= new Date();
  }

  function handleSeasonSelect(e: Event & { currentTarget: HTMLSelectElement }) {
    const season = Number(e.currentTarget.value);
    selectedSeason = season;
    onSeasonChange(season);
  }
</script>

<section class="seasons-section">
  <div class="section-header">
    <h3 class="section-label">Seasons</h3>
    {#if !loading && tv && tv.seasons.length > 1}
      <select
        class="season-selector glass-panel"
        value={selectedSeason}
        onchange={handleSeasonSelect}
      >
        {#each tv.seasons.filter((s) => s.season_number > 0) as season}
          <option value={season.season_number}>Season {season.season_number}</option>
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
            <div class="skeleton skeleton-title" style="height:1.5rem;width:60%"></div>
            <div class="skeleton skeleton-text" style="margin-top:1rem"></div>
            <div class="skeleton skeleton-text"></div>
          </div>
        </div>
      {/each}
    {:else if seasonDetails}
      {#each seasonDetails.episodes as episode}
        <div
          class="episode-card glass-panel"
          class:episode-unreleased={!isReleased(episode)}
        >
          <div class="episode-thumbnail">
            <img
              src={getPosterUrl(episode.still_path, "w500") ||
                "/images/placeholder-poster.svg"}
              alt={episode.name}
            />
            <div class="absolute top-2 left-2 z-[2] pointer-events-none">
              <Badge variant="episode" size="xs" noDot>EP {episode.episode_number}</Badge>
            </div>
            {#if !isReleased(episode)}
              <div class="coming-soon-overlay">
                <span class="material-icons">schedule</span>
              </div>
            {/if}
          </div>
          <div class="episode-info">
            <div class="episode-header">
              <div class="episode-title-row">
                <h4 class="episode-name">{episode.name}</h4>
                {#if isReleased(episode)}
                  <span class="episode-date"
                    >{new Date(episode.air_date).toLocaleDateString()}</span
                  >
                {:else}
                  <span class="coming-soon-tag">
                    <span class="material-icons" style="font-size:.7rem">schedule</span>
                    {episode.air_date
                      ? new Date(episode.air_date).toLocaleDateString()
                      : "TBA"}
                  </span>
                {/if}
              </div>
              <div class="episode-actions">
                {#if getEpisodeAcquisition(episode.season_number, episode.episode_number) === true}
                  <div title="In library">
                    <Badge
                      variant="status"
                      size="xs"
                      noDot
                      class="text-[#34d399] border-[#34d399]/30 bg-[#34d399]/10"
                    >
                      <span class="material-icons text-[12px]">check_circle</span>
                    </Badge>
                  </div>
                {:else if getEpisodeAcquisition(episode.season_number, episode.episode_number) === false}
                  <div title="Missing">
                    <Badge
                      variant="status"
                      size="xs"
                      noDot
                      class="text-[#ef4444] border-[#ef4444]/30 bg-[#ef4444]/10 opacity-70"
                    >
                      <span class="material-icons text-[12px]">cancel</span>
                    </Badge>
                  </div>
                {/if}
                {#if isReleased(episode)}
                  <button
                    class="icon-btn-tiny"
                    onclick={() => onEpisodeSearch(episode)}
                    title="Search this episode"
                  >
                    <span class="material-icons">manage_search</span>
                  </button>
                {/if}
              </div>
            </div>
            <p class="episode-overview">
              {episode.overview || "No overview available."}
            </p>
          </div>
        </div>
      {/each}
    {/if}
  </div>
</section>

<style>
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
  .skeleton-title {
    margin-bottom: .5rem;
  }
  .skeleton-text {
    width: 100%;
    height: 1rem;
    margin-bottom: .75rem;
  }
  .seasons-section {
    margin-bottom: 4rem;
  }
  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1.5rem;
  }
  .section-label {
    text-transform: uppercase;
    letter-spacing: .15em;
    font-size: .75rem;
    color: var(--color-primary);
    margin-bottom: 0;
    font-weight: 800;
    display: flex;
    align-items: center;
    gap: .5rem;
  }
  .season-selector {
    padding: .5rem 1rem;
    background: #ffffff0d;
    color: #fff;
    border: 1px solid rgba(255, 255, 255, .1);
    border-radius: 8px;
    font-size: .85rem;
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
    transition: transform .2s;
    background: #ffffff05;
  }
  .episode-card:hover {
    background: #ffffff0a;
    transform: translate(5px);
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
    opacity: .7;
  }
  .episode-unreleased {
    opacity: .5;
    position: relative;
  }
  .episode-unreleased .episode-thumbnail img {
    filter: grayscale(.7);
  }
  .coming-soon-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #00000080;
    border-radius: 12px;
  }
  .coming-soon-overlay .material-icons {
    font-size: 2rem;
    color: #ffffffb3;
  }
  .coming-soon-tag {
    display: inline-flex;
    align-items: center;
    gap: .25rem;
    font-size: .7rem;
    font-family: var(--font-mono);
    color: var(--accent-warning,#f59e0b);
    background: #f59e0b1a;
    padding: .15rem .5rem;
    border-radius: 4px;
    border: 1px solid rgba(245, 158, 11, .2);
  }
  .episode-info {
    flex: 1;
    min-width: 0;
  }
  .episode-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: .75rem;
  }
  .episode-title-row {
    flex: 1;
  }
  .episode-name {
    font-size: 1.1rem;
    font-weight: 700;
    margin: 0 0 .25rem;
    color: var(--text-primary);
  }
  .episode-date {
    font-size: .75rem;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }
  .episode-overview {
    font-size: .9rem;
    line-height: 1.5;
    color: var(--text-secondary);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .episode-actions {
    display: flex;
    align-items: center;
    gap: .5rem;
    flex-shrink: 0;
  }
  .icon-btn-tiny {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text-muted);
    padding: .25rem;
    border-radius: 4px;
    display: flex;
    align-items: center;
    transition: color .15s;
  }
  .icon-btn-tiny:hover {
    color: var(--color-primary);
  }
  .icon-btn-tiny .material-icons {
    font-size: 18px;
  }
  @media (max-width: 768px) {
    .episode-card {
      flex-direction: column;
      width: 100%;
      box-sizing: border-box;
      overflow: hidden;
    }
    .episode-thumbnail {
      width: 100%;
      max-width: 100%;
    }
    .episode-info {
      width: 100%;
      min-width: 0;
      box-sizing: border-box;
    }
    .episode-header {
      flex-wrap: wrap;
      gap: .4rem;
    }
    .episode-title-row {
      min-width: 0;
      flex: 1 1 0;
    }
    .episode-name {
      white-space: normal;
      word-break: break-word;
    }
    .episode-actions {
      flex-shrink: 1;
      flex-wrap: wrap;
    }
  }
  @media (max-width: 480px) {
    .episodes-list {
      gap: .75rem;
    }
    .episode-card {
      padding: .75rem;
      gap: .75rem;
    }
    .episode-name {
      font-size: .95rem;
    }
    .episode-overview {
      font-size: .82rem;
    }
    .episode-actions {
      gap: .35rem;
    }
  }
</style>
