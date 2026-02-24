<script lang="ts">
  import { page } from "$app/stores";
  import { onMount } from "svelte";
  import {
    fetchAllSeries,
    formatDiskSize,
    getSeriesPoster,
    getSeriesBanner,
    type SonarrSeries,
  } from "$lib/stores/arr";

  interface SonarrEpisode {
    id: number;
    seasonNumber: number;
    episodeNumber: number;
    title?: string;
    airDateUtc?: string;
    hasFile: boolean;
    overview?: string;
  }

  let seriesId = $derived(Number($page.params.id));
  let series = $state<SonarrSeries | null>(null);
  let episodes = $state<SonarrEpisode[]>([]);
  let loading = $state(true);
  let activeSeason: number | "all" = $state("all");

  onMount(async () => {
    const headerContainer = document.getElementById("header-dynamic-content");
    if (headerContainer) {
      headerContainer.innerHTML = `
        <div style="display: flex; align-items: center; gap: 0.75rem;">
          <a href="/library" style="color: var(--text-muted); text-decoration: none; display: flex; align-items: center;">
            <span class="material-icons" style="font-size: 1.2rem;">arrow_back</span>
          </a>
          <span class="material-icons" style="color: #a78bfa; font-size: 1.3rem;">tv</span>
          <h1 style="font-size: 0.9rem; font-weight: 800; letter-spacing: 0.1em; text-transform: uppercase; margin: 0; color: #fff;">Series Detail</h1>
        </div>
      `;
    }

    try {
      // Fetch series data
      const allSeries = await fetchAllSeries();
      series = allSeries.find((s) => s.id === seriesId) || null;

      // Fetch episodes
      const res = await fetch(`/api/arr/episodes?series_id=${seriesId}`);
      if (res.ok) {
        episodes = await res.json();
      }
    } catch (e) {
      console.error("Failed to load series detail:", e);
    } finally {
      loading = false;
    }
  });

  // Available seasons
  let seasons = $derived(() => {
    const set = new Set(episodes.map((e) => e.seasonNumber));
    return [...set].sort((a, b) => a - b);
  });

  // Filtered episodes by season
  let filteredEpisodes = $derived(() => {
    if (activeSeason === "all") return episodes;
    return episodes.filter((e) => e.seasonNumber === activeSeason);
  });

  // Season stats
  let seasonStats = $derived(() => {
    const eps = filteredEpisodes();
    const total = eps.length;
    const withFile = eps.filter((e) => e.hasFile).length;
    return { total, withFile, missing: total - withFile };
  });

  // Overall progress
  let overallProgress = $derived(() => {
    if (episodes.length === 0) return 0;
    return Math.round(
      (episodes.filter((e) => e.hasFile).length / episodes.length) * 100,
    );
  });

  function formatDate(dateStr?: string): string {
    if (!dateStr) return "—";
    return new Date(dateStr).toLocaleDateString("en-US", {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  }

  function getStatusLabel(status?: string): string {
    if (status === "continuing") return "CONTINUING";
    if (status === "ended") return "ENDED";
    if (status === "upcoming") return "UPCOMING";
    return status?.toUpperCase() || "UNKNOWN";
  }

  function getStatusColor(status?: string): string {
    if (status === "continuing") return "#34d399";
    if (status === "ended") return "#94a3b8";
    if (status === "upcoming") return "#fbbf24";
    return "#64748b";
  }
</script>

<svelte:head>
  <title>{series?.title || "Series"} - Library - Flasharr</title>
</svelte:head>

<div class="series-detail">
  {#if loading}
    <div class="loading-state">
      <div class="pulse-ring"></div>
      <span>LOADING SERIES DATA...</span>
    </div>
  {:else if !series}
    <div class="loading-state">
      <span class="material-icons" style="font-size: 48px; opacity: 0.2;"
        >error</span
      >
      <span>SERIES NOT FOUND</span>
    </div>
  {:else}
    <!-- Hero Section -->
    <div
      class="hero"
      style="background-image: url('{getSeriesBanner(series) || ''}')"
    >
      <div class="hero-overlay"></div>
      <div class="hero-content">
        <div class="hero-poster">
          {#if getSeriesPoster(series)}
            <img src={getSeriesPoster(series)} alt={series.title} />
          {:else}
            <div class="poster-placeholder">
              <span class="material-icons">live_tv</span>
            </div>
          {/if}
        </div>
        <div class="hero-info">
          <h1>{series.title}</h1>
          <div class="hero-meta">
            <span
              class="status-tag"
              style="background: {getStatusColor(series.status)}"
              >{getStatusLabel(series.status)}</span
            >
            {#if series.year}<span class="meta-item">{series.year}</span>{/if}
            {#if series.statistics?.seasonCount}<span class="meta-item"
                >{series.statistics.seasonCount} Seasons</span
              >{/if}
          </div>
          <div class="hero-stats">
            <div class="h-stat">
              <span class="hl">EPISODES</span>
              <span class="hv"
                >{series.statistics?.episodeFileCount || 0}/{series.statistics
                  ?.episodeCount || 0}</span
              >
            </div>
            <div class="h-stat">
              <span class="hl">SIZE</span>
              <span class="hv"
                >{formatDiskSize(series.statistics?.sizeOnDisk || 0)}</span
              >
            </div>
            <div class="h-stat">
              <span class="hl">PROGRESS</span>
              <span class="hv">{overallProgress()}%</span>
            </div>
          </div>
          <!-- Progress Bar -->
          <div class="hero-progress">
            <div
              class="hero-progress-fill"
              style="width: {overallProgress()}%"
            ></div>
          </div>
          {#if series.overview}
            <p class="hero-overview">{series.overview}</p>
          {/if}
        </div>
      </div>
    </div>

    <!-- Season Tabs -->
    <div class="season-bar">
      <div class="season-tabs">
        <button
          class="season-tab"
          class:active={activeSeason === "all"}
          onclick={() => (activeSeason = "all")}
        >
          ALL
        </button>
        {#each seasons() as sn}
          <button
            class="season-tab"
            class:active={activeSeason === sn}
            onclick={() => (activeSeason = sn)}
          >
            S{String(sn).padStart(2, "0")}
          </button>
        {/each}
      </div>
      <div class="season-stats">
        <span>{seasonStats().withFile}/{seasonStats().total} episodes</span>
        {#if seasonStats().missing > 0}
          <span class="missing-count">{seasonStats().missing} missing</span>
        {/if}
      </div>
    </div>

    <!-- Episode Grid -->
    <div class="episode-list">
      {#each filteredEpisodes() as ep (ep.id)}
        <div
          class="episode-row"
          class:has-file={ep.hasFile}
          class:missing={!ep.hasFile}
        >
          <div class="ep-number">
            <span
              >S{String(ep.seasonNumber).padStart(2, "0")}E{String(
                ep.episodeNumber,
              ).padStart(2, "0")}</span
            >
          </div>
          <div class="ep-info">
            <div class="ep-title">{ep.title || "TBA"}</div>
            <div class="ep-date">{formatDate(ep.airDateUtc)}</div>
          </div>
          <div class="ep-status">
            {#if ep.hasFile}
              <span class="material-icons status-acquired">check_circle</span>
            {:else}
              <span class="material-icons status-missing">cancel</span>
            {/if}
          </div>
        </div>
      {:else}
        <div class="empty-episodes">
          <span class="material-icons">playlist_remove</span>
          <p>No episodes found</p>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .series-detail {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow-y: auto;
  }

  /* Loading */
  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1rem;
    padding: 4rem;
    color: var(--text-muted);
    flex: 1;
  }

  .loading-state span {
    font-size: 0.65rem;
    font-weight: 800;
    letter-spacing: 0.15em;
    opacity: 0.5;
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

  /* Hero */
  .hero {
    position: relative;
    min-height: 280px;
    background-size: cover;
    background-position: center;
    flex-shrink: 0;
  }

  .hero-overlay {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      to right,
      rgba(10, 15, 25, 0.95) 0%,
      rgba(10, 15, 25, 0.8) 40%,
      rgba(10, 15, 25, 0.6) 100%
    );
  }

  .hero-content {
    position: relative;
    z-index: 1;
    display: flex;
    gap: 1.5rem;
    padding: 2rem;
    align-items: flex-start;
  }

  .hero-poster {
    width: 140px;
    flex-shrink: 0;
    border-radius: 4px;
    overflow: hidden;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .hero-poster img {
    width: 100%;
    display: block;
  }

  .poster-placeholder {
    aspect-ratio: 2/3;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.03);
    color: var(--text-muted);
    opacity: 0.3;
  }

  .poster-placeholder .material-icons {
    font-size: 48px;
  }

  .hero-info {
    flex: 1;
    min-width: 0;
  }

  .hero-info h1 {
    font-size: 1.5rem;
    font-weight: 800;
    margin: 0 0 0.5rem;
    color: #fff;
  }

  .hero-meta {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 1rem;
    flex-wrap: wrap;
  }

  .status-tag {
    font-size: 0.5rem;
    font-weight: 900;
    letter-spacing: 0.1em;
    padding: 2px 8px;
    border-radius: 3px;
    color: #fff;
  }

  .meta-item {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .hero-stats {
    display: flex;
    gap: 1.5rem;
    margin-bottom: 0.75rem;
  }

  .h-stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .h-stat .hl {
    font-size: 0.5rem;
    font-weight: 800;
    letter-spacing: 0.12em;
    color: var(--text-muted);
    opacity: 0.6;
  }

  .h-stat .hv {
    font-family: var(--font-mono, monospace);
    font-size: 0.85rem;
    font-weight: 800;
    color: var(--color-primary);
  }

  .hero-progress {
    height: 4px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 2px;
    overflow: hidden;
    margin-bottom: 1rem;
    max-width: 400px;
  }

  .hero-progress-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--color-primary), #a78bfa);
    border-radius: 2px;
    transition: width 0.5s;
  }

  .hero-overview {
    font-size: 0.75rem;
    color: var(--text-muted);
    line-height: 1.5;
    max-width: 600px;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  /* Season Bar */
  .season-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    flex-shrink: 0;
    gap: 1rem;
  }

  .season-tabs {
    display: flex;
    gap: 2px;
    overflow-x: auto;
    flex: 1;
  }

  .season-tab {
    padding: 0.4rem 0.75rem;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    color: var(--text-muted);
    font-size: 0.6rem;
    font-weight: 800;
    letter-spacing: 0.08em;
    cursor: pointer;
    transition: all 0.2s;
    white-space: nowrap;
    font-family: var(--font-mono, monospace);
  }

  .season-tab.active {
    background: rgba(167, 139, 250, 0.1);
    border-color: rgba(167, 139, 250, 0.3);
    color: #a78bfa;
  }

  .season-tab:hover:not(.active) {
    background: rgba(255, 255, 255, 0.04);
    color: #fff;
  }

  .season-stats {
    display: flex;
    gap: 0.75rem;
    font-size: 0.6rem;
    font-weight: 800;
    color: var(--text-muted);
    font-family: var(--font-mono, monospace);
    white-space: nowrap;
  }

  .missing-count {
    color: #fbbf24;
  }

  /* Episode List */
  .episode-list {
    flex: 1;
    overflow-y: auto;
    padding: 0.5rem 1.5rem;
  }

  .episode-row {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
    transition: all 0.2s;
  }

  .episode-row:hover {
    background: rgba(255, 255, 255, 0.02);
  }

  .episode-row.missing {
    opacity: 0.6;
  }

  .episode-row.missing:hover {
    opacity: 1;
  }

  .ep-number {
    font-family: var(--font-mono, monospace);
    font-size: 0.7rem;
    font-weight: 800;
    color: var(--color-primary);
    min-width: 72px;
    opacity: 0.8;
  }

  .episode-row.missing .ep-number {
    color: var(--text-muted);
  }

  .ep-info {
    flex: 1;
    min-width: 0;
  }

  .ep-title {
    font-size: 0.75rem;
    font-weight: 600;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .episode-row.missing .ep-title {
    color: var(--text-muted);
  }

  .ep-date {
    font-size: 0.6rem;
    color: var(--text-muted);
    opacity: 0.5;
    margin-top: 2px;
  }

  .ep-status .material-icons {
    font-size: 18px;
  }

  .status-acquired {
    color: #34d399;
  }
  .status-missing {
    color: #ef4444;
    opacity: 0.5;
  }

  .empty-episodes {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    padding: 3rem;
    color: var(--text-muted);
  }

  .empty-episodes .material-icons {
    font-size: 36px;
    opacity: 0.2;
  }

  .empty-episodes p {
    font-size: 0.6rem;
    font-weight: 800;
    letter-spacing: 0.15em;
    opacity: 0.5;
  }

  /* Scrollbar */
  .episode-list::-webkit-scrollbar {
    width: 4px;
  }
  .episode-list::-webkit-scrollbar-track {
    background: transparent;
  }
  .episode-list::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 2px;
  }

  .series-detail::-webkit-scrollbar {
    width: 4px;
  }
  .series-detail::-webkit-scrollbar-track {
    background: transparent;
  }
  .series-detail::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 2px;
  }

  /* Mobile */
  @media (max-width: 768px) {
    .hero-content {
      flex-direction: column;
      align-items: center;
      text-align: center;
    }

    .hero-poster {
      width: 100px;
    }
    .hero-info h1 {
      font-size: 1.2rem;
    }
    .hero-stats {
      justify-content: center;
    }
    .hero-overview {
      max-width: 100%;
    }
    .hero-progress {
      max-width: 100%;
    }

    .season-bar {
      flex-direction: column;
      align-items: flex-start;
    }
    .season-tabs {
      width: 100%;
    }
  }
</style>
