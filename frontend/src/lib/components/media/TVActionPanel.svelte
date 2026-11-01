<script lang="ts">
  import { Badge, Button } from "@media-set/core-ui";
  import type { TMDBTVShowDetails } from "$lib/services/tmdb";
  import { formatDiskSize, type SonarrSeries } from "$lib/stores/arr";
  import type { LibraryProgress } from "./TVDetailHero.svelte";

  interface Props {
    tv: TMDBTVShowDetails | null;
    loading: boolean;
    inLibrary: boolean;
    librarySeries: SonarrSeries | null;
    libraryProgress: LibraryProgress | null;
    contentRating: string;
    tags: { id?: number; name: string }[];
    addingToLibrary: boolean;
    onSmartSearch: () => void;
    onAddToLibrary: () => void;
    /** Compact layout used by the mobile action panels. */
    compact?: boolean;
  }

  let {
    tv,
    loading,
    inLibrary,
    librarySeries,
    libraryProgress,
    contentRating,
    tags,
    addingToLibrary,
    onSmartSearch,
    onAddToLibrary,
    compact = false,
  }: Props = $props();

  function formatScore(score: number): number {
    return Math.round(score * 10);
  }
</script>

<aside class="detail-sidebar" class:compact>
  <div class="action-panel glass-panel">
    {#if inLibrary}
      <div class="library-control">
        <Badge
          variant="status"
          size="sm"
          noDot
          class="font-mono text-[#34d399] border-[#34d399]/30 bg-[#34d399]/10"
        >
          <span class="material-icons text-[13px]">video_library</span>
          IN LIBRARY
        </Badge>
      </div>
    {/if}
    <Button
      icon="manage_search"
      size="md"
      width="100%"
      disabled={loading}
      onclick={onSmartSearch}>Smart Search</Button
    >
    {#if !inLibrary && !loading}
      <Button
        variant="ghost"
        icon="library_add"
        size="md"
        width="100%"
        disabled={addingToLibrary}
        onclick={onAddToLibrary}
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
            style="width:30%"
          ></div>
        </div>
      {/each}
    {:else if tv}
      <div class="info-row">
        <span class="label">Status</span>
        <span class="value" style="color:var(--color-primary)"
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
        <span class="value" style="color:#ffcc00">{contentRating}</span>
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
            style="color:{librarySeries.monitored ? '#34d399' : '#94a3b8'}"
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

<style>
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
  .skeleton-small-text {
    width: 80%;
    height: .8rem;
    margin-bottom: .4rem;
  }
  .detail-sidebar {
    position: relative;
    z-index: 2;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }
  .detail-sidebar.compact .stats-panel,
  .detail-sidebar.compact .keywords-panel {
    display: none;
  }
  .action-panel {
    padding: 1.5rem;
    background: #00f3ff0d;
    border: 1px solid rgba(0, 243, 255, .2);
    position: relative;
    overflow: hidden;
  }
  .action-panel {
    display: flex;
    flex-direction: column;
    gap: .5rem;
  }
  .library-control {
    width: 100%;
    height: 31.2px;
    display: flex;
    min-width: 0;
  }
  .library-control :global(.badge) {
    display: flex;
    width: 100%;
    height: 31.2px;
    box-sizing: border-box;
    justify-content: center;
    align-items: center;
    white-space: nowrap;
    overflow: visible;
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
    font-family: var(--font-mono,monospace);
  }
  .info-row .value {
    font-size: .9rem;
    font-weight: 700;
    color: var(--text-primary);
    font-family: var(--font-mono,monospace);
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
</style>
