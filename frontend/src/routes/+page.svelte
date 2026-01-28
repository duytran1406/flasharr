<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import {
    engineStats,
    activeDownloads,
    queuedDownloads,
    formatBytes,
    type DownloadTask,
  } from "$lib/stores/downloads";
  import {
    getTrendingMovies,
    getPosterUrl,
    getBackdropUrl,
    getBackdropUrlOrPlaceholder,
    getPosterUrlOrPlaceholder,
    getYear,
    type TMDBMovie,
  } from "$lib/services/tmdb";

  import { accountStore } from "$lib/stores/account.svelte";
  import {
    MediaCard,
    SpeedGraph,
    Carousel3D,
    IdentityCard,
  } from "$lib/components";

  // Reactive stats from WebSocket
  let stats = $derived({
    active: $engineStats.active_downloads,
    queued: $engineStats.queued,
    completed: $engineStats.completed,
    failed: $engineStats.failed,
  });

  // Reactive account data from accountStore
  let account = $derived(accountStore.primaryFormatted);

  // Reactive active & queued downloads (top 5 combined)
  let activeDownloadsList = $derived(
    [...$activeDownloads, ...$queuedDownloads]
      .sort(
        (a, b) =>
          b.priority - a.priority ||
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime(),
      )
      .slice(0, 5)
      .map((d: DownloadTask) => ({
        id: d.id,
        filename: d.filename,
        state: d.state.toLowerCase(),
        progress: Math.round(d.progress),
      })),
  );

  let trendingItems = $state<TMDBMovie[]>([]);
  let isLoadingTrending = $state(true);

  // Netflow state
  let speedHistory = $state<number[]>(new Array(30).fill(0));

  // Reactive formatted speed for the FloodUI labels
  function parseSpeed(bytesPerSec: number) {
    if (bytesPerSec === 0) return { val: "0", unit: "B/s" };
    const k = 1024;
    const units = ["B/s", "KB/s", "MB/s", "GB/s"];
    const i = Math.floor(Math.log(bytesPerSec) / Math.log(k));
    return {
      val: (bytesPerSec / Math.pow(k, i)).toFixed(1),
      unit: units[i],
    };
  }

  let dlSpeed = $derived(parseSpeed($engineStats.total_speed || 0));

  // Session Telemetry
  let sessionPeak = $state(0);
  let sessionTotalBytes = $state(0);

  // Carousel state
  let carouselTrack: HTMLElement | undefined = $state();

  onMount(() => {
    // Set Page Header
    const headerContainer = document.getElementById("header-dynamic-content");
    if (headerContainer) {
      headerContainer.innerHTML = `
        <div style="display: flex; align-items: center; gap: 0.75rem;">
          <span class="material-icons" style="color: var(--color-primary); font-size: 1.5rem;">grid_view</span>
          <h1 style="font-size: 0.9rem; font-weight: 800; letter-spacing: 0.1em; text-transform: uppercase; margin: 0; color: #fff;">Operational Dashboard</h1>
        </div>
      `;
    }

    // Measure network speed every second - move out of effect to prevent reset
    const interval = setInterval(() => {
      const stats = get(engineStats);
      const currentSpeed = stats.total_speed || 0;
      const speedMb = currentSpeed / (1024 * 1024);

      // Update Peak - Only if current is higher than the historical peak
      if (speedMb > sessionPeak) {
        sessionPeak = speedMb;
      }

      // Accumulate session total bytes
      sessionTotalBytes += currentSpeed;

      // Rotate history for the real-time chart
      speedHistory = [...speedHistory.slice(1), speedMb];
    }, 1000);

    // Fetch trending movies from TMDB (not fetched in layout)
    (async () => {
      try {
        const movies = await getTrendingMovies();
        trendingItems = movies;
        isLoadingTrending = false;
      } catch (e) {
        console.error("Failed to load trending items", e);
        isLoadingTrending = false;
      }
    })();

    return () => clearInterval(interval);
  });
</script>

<svelte:head>
  <title>Dashboard - Flasharr</title>
</svelte:head>

<div class="dashboard-viewport">
  <!-- Trending Carousel Section -->
  <section class="box-section trending-section">
    <div class="box-label" style="color: #ffcc00;">
      <span class="material-icons">trending_up</span>
      TRENDING THIS WEEK
    </div>
    {#if isLoadingTrending}
      <div class="carousel-loading">
        <div class="loading-spinner"></div>
      </div>
    {:else if trendingItems.length > 0}
      <div
        class="trending-carousel-wrapper"
        role="region"
        aria-label="Trending items slider"
      >
        <Carousel3D
          items={trendingItems}
          width={270 * 1.1}
          height={152 * 1.1}
          space={360 * 1.1}
          display={20}
        >
          {#snippet children(item)}
            <MediaCard
              id={item.id}
              title={item.title || item.name}
              backdropPath={item.backdrop_path}
              voteAverage={item.vote_average}
              releaseDate={item.release_date || item.first_air_date}
              overview={item.overview}
              mode="banner"
              mediaType={item.media_type || "movie"}
              hideDetailsButton={true}
            />
          {/snippet}
        </Carousel3D>
      </div>
    {:else}
      <div class="carousel-empty">
        <span class="material-icons">movie_filter</span>
        <p>No trending movies available</p>
      </div>
    {/if}
  </section>

  <!-- Main Grid -->
  <div class="dashboard-grid">
    <!-- Left: Active Downloads -->
    <section class="box-section downloads-section">
      <div class="box-label" style="color: var(--color-secondary);">
        <span class="material-icons">list_alt</span>
        ACTIVE & QUEUED TASKS
      </div>
      <div class="toolbar-actions-dock">
        <div class="header-stats-row">
          <div class="h-stat">
            <span class="l">ACTIVE</span>
            <span class="v color-secondary">{stats.active}</span>
          </div>
          <div class="h-stat">
            <span class="l">QUEUED</span>
            <span class="v color-warning">{stats.queued}</span>
          </div>
          <div class="h-stat">
            <span class="l">COMPLETED</span>
            <span class="v color-success">{stats.completed}</span>
          </div>
        </div>
        <a href="/downloads" class="expand-btn">EXPAND LIST</a>
      </div>

      <div class="mini-queue">
        {#each activeDownloadsList as dl}
          <div class="data-shard-card {dl.state}">
            <div class="shard-side-accent"></div>
            <div class="shard-main">
              <div class="shard-top">
                <span class="shard-filename" title={dl.filename}
                  >{dl.filename}</span
                >
                <div class="shard-badge {dl.state}">
                  <span class="dot"></span>
                  {dl.state}
                </div>
              </div>

              <div class="shard-progress-block">
                <div class="shard-track">
                  <div class="shard-fill" style="width: {dl.progress}%">
                    <div class="fill-glow"></div>
                  </div>
                </div>
                <div class="shard-meta">
                  <span class="pct">{dl.progress}%</span>
                  <span class="shard-id">NODE-{dl.id.substring(0, 6)}/A</span>
                </div>
              </div>
            </div>
          </div>
        {:else}
          <div class="empty-shard-placeholder">
            <span class="material-icons">radar</span>
            <p>NO ACTIVE DATA STREAMS DETECTED</p>
          </div>
        {/each}
      </div>
    </section>

    <!-- Right: Account & Stats -->
    <div class="side-column">
      <!-- Account Card -->
      <section class="box-section account-section">
        <div class="box-label" style="color: var(--color-primary);">
          <span class="material-icons">shield</span>
          ACCOUNT OVERVIEW
        </div>

        <IdentityCard
          email={account.email}
          rank={account.rank}
          expiry={account.expiry}
          quotaUsed={account.quotaUsed}
          quotaTotal={account.quotaTotal}
          quotaPercent={account.quotaPercent}
          compact={true}
        />
      </section>

      <!-- Netflow -->
      <section class="box-section netflow-section">
        <div class="box-label">
          <span class="material-icons">insights</span>
          NETFLOW STATISTIC
        </div>

        <div class="netflow-telemetry">
          <div class="telemetry-main">
            <div class="telemetry-core">
              <span class="material-icons dl-pulse">settings_input_antenna</span
              >
              <div class="core-text">
                <span class="v">{dlSpeed.val}</span>
                <span class="u">{dlSpeed.unit}</span>
              </div>
            </div>
            <div class="telemetry-label">INBOUND BITRATE</div>
          </div>

          <div class="telemetry-grid">
            <div class="tele-item">
              <span class="l">SESSION PEAK</span>
              <span class="v">{sessionPeak.toFixed(1)} MB/s</span>
            </div>
            <div class="tele-item">
              <span class="l">SESSION DATA</span>
              <span class="v">{formatBytes(sessionTotalBytes)}</span>
            </div>
          </div>
        </div>

        <div class="chart-wrapper">
          <SpeedGraph data={speedHistory} labels={new Array(30).fill("")} />
        </div>
      </section>
    </div>
  </div>
</div>

<style>
  .dashboard-viewport {
    padding: 1.5rem;
    height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    height: 100%;
    overflow: hidden; /* Primary fix for zero-scroll */
  }

  /* Fixed height zones */
  .trending-section {
    flex-shrink: 0;
  }

  .dashboard-grid {
    flex: 1;
    display: grid;
    grid-template-columns: 1fr 380px;
    gap: 1.5rem;
    min-height: 0; /* Important for flex child to allow inner scroll */
  }

  .box-section {
    background: rgba(10, 15, 25, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 0;
    position: relative;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .box-section::before {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    width: 2px;
    height: 100%;
    background: var(--color-primary);
    opacity: 0.5;
  }

  .box-label {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    font-size: 0.65rem;
    font-weight: 900;
    letter-spacing: 0.2rem;
    text-transform: uppercase;
    margin-bottom: 0.75rem;
    color: var(--text-muted);
    font-family: var(--font-mono, monospace);
    padding: 0 0.5rem;
    flex-shrink: 0;
  }

  .box-label .material-icons {
    font-size: 1.1rem;
    opacity: 0.8;
  }

  /* Trending Carousel */
  .trending-section {
    height: 250px;
    padding: 0.75rem 0;
  }

  .trending-carousel-wrapper {
    width: 100%;
    margin-top: 1rem;
    position: relative;
  }

  .carousel-loading,
  .carousel-empty {
    height: 200px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-direction: column;
    gap: 1rem;
    color: var(--text-muted);
  }

  .carousel-empty .material-icons {
    font-size: 48px;
    opacity: 0.3;
  }

  .carousel-empty p {
    font-size: 0.8rem;
    opacity: 0.5;
  }

  /* Main Grid */
  @media (max-width: 1100px) {
    .dashboard-grid {
      grid-template-columns: 1fr;
      overflow-y: auto; /* Allow scroll on smaller screens only */
    }
  }

  /* Mini Queue */
  .downloads-section {
    display: flex;
    flex-direction: column;
    height: 100%;
    position: relative;
  }

  .toolbar-actions-dock {
    position: absolute;
    top: 1rem;
    right: 1.5rem;
  }

  .expand-btn {
    font-size: 0.6rem;
    font-weight: 800;
    color: var(--text-muted);
    text-decoration: none;
    border: 1px solid rgba(255, 255, 255, 0.1);
    padding: 3px 8px;
    border-radius: 4px;
    transition: all 0.2s;
  }

  .expand-btn:hover {
    color: var(--color-primary);
    border-color: var(--color-primary);
    background: rgba(0, 243, 255, 0.05);
  }

  .mini-queue {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    margin-top: 0.5rem;
    flex: 1;
    overflow-y: auto;
    padding-right: 0.5rem;
    min-height: 0;
  }

  /* Custom Scrollbar for mini-queue */
  .mini-queue::-webkit-scrollbar {
    width: 4px;
  }
  .mini-queue::-webkit-scrollbar-track {
    background: transparent;
  }
  .mini-queue::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 2px;
  }
  .mini-queue::-webkit-scrollbar-thumb:hover {
    background: var(--color-primary);
  }

  @media (max-width: 800px) {
    .mini-queue {
      grid-template-columns: 1fr;
    }
  }

  .mini-dl-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    padding: 0.75rem 1rem;
    display: flex;
    gap: 1rem;
    align-items: center;
    position: relative;
    clip-path: polygon(
      0% 0%,
      100% 0%,
      100% calc(100% - 8px),
      calc(100% - 8px) 100%,
      0% 100%
    );
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .mini-dl-card:hover {
    background: rgba(0, 243, 255, 0.03);
    border-color: rgba(0, 243, 255, 0.2);
    transform: translateX(4px);
  }

  .dl-icon {
    width: 32px;
    height: 32px;
    border-radius: 6px;
    background: rgba(0, 243, 255, 0.05);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-primary);
  }

  .dl-body {
    flex: 1;
    min-width: 0;
  }

  .dl-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .dl-header .filename {
    font-size: 0.7rem;
    font-weight: 700;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    margin-right: 0.75rem;
  }

  .status-tag {
    font-size: 0.5rem;
    font-weight: 900;
    text-transform: uppercase;
    padding: 1px 5px;
    border-radius: 3px;
  }

  .status-tag.running {
    background: rgba(0, 255, 128, 0.1);
    color: #00ff80;
  }
  .status-tag.queued {
    background: rgba(255, 204, 0, 0.1);
    color: #ffcc00;
  }
  .status-tag.completed {
    background: rgba(0, 243, 255, 0.1);
    color: var(--color-primary);
  }

  .dl-progress-container {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .dl-progress-bar {
    flex: 1;
    height: 3px;
    background: rgba(0, 0, 0, 0.3);
    border-radius: 2px;
    overflow: hidden;
  }

  .dl-progress-bar .fill {
    height: 100%;
    background: var(--color-primary);
    border-radius: 2px;
  }

  .dl-progress-container .percent {
    font-family: var(--font-mono);
    font-size: 0.6rem;
    color: var(--text-muted);
  }

  /* Side Column */
  .side-column {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    min-height: 0;
    height: 100%;
  }

  .netflow-section {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .chart-wrapper {
    flex: 1;
    margin-top: 0.5rem;
    position: relative;
    min-height: 120px; /* Ensure visibility */
    width: 100%;
  }

  .account-pill-v2 {
    display: flex;
    gap: 1rem;
    align-items: center;
    margin-top: 0.5rem;
    margin-bottom: 1.5rem;
  }

  .stats-mini-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.75rem;
    margin-bottom: 0.75rem;
  }

  .stat-tile {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    padding: 1rem 0.5rem;
    backdrop-filter: blur(8px);
    transition: all 0.3s ease;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    position: relative;
  }

  .stat-tile:hover {
    background: rgba(255, 255, 255, 0.07);
    transform: translateY(-2px);
    border-color: rgba(255, 255, 255, 0.2);
  }

  .stat-tile::after {
    content: "";
    position: absolute;
    bottom: 0;
    left: 50%;
    transform: translateX(-50%);
    width: 12px;
    height: 3px;
    border-radius: 3px 3px 0 0;
    opacity: 0.6;
  }

  .stat-tile:nth-child(1)::after {
    background: var(--color-secondary);
  }
  .stat-tile:nth-child(2)::after {
    background: #ffcc00;
  }
  .stat-tile:nth-child(3)::after {
    background: var(--color-primary);
  }

  .stat-tile .label {
    font-size: 0.6rem;
    font-weight: 800;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.4);
    letter-spacing: 0.05em;
    margin-bottom: 0;
  }

  .stat-tile .value {
    font-family: var(--font-mono);
    font-size: 1.4rem;
    font-weight: 900;
  }
  .value.secondary {
    color: var(--color-secondary);
  }
  .value.warning {
    color: #ffcc00;
  }
  .value.success {
    color: var(--color-primary);
  }

  .quota-mini-tracker {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .tracker-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
  }
  .chart-wrapper {
    flex: 1;
    width: 100%;
    min-height: 0;
  }

  :global(.netflow-section .glass-card) {
    background: transparent !important;
    border: none !important;
    padding: 0 !important;
    box-shadow: none !important;
  }

  /* Premium Shard Cards */
  .data-shard-card {
    background: linear-gradient(
      135deg,
      rgba(255, 255, 255, 0.02) 0%,
      rgba(255, 255, 255, 0.04) 100%
    );
    border: 1px solid rgba(255, 255, 255, 0.03);
    border-radius: 4px;
    display: flex;
    position: relative;
    overflow: hidden;
    transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .data-shard-card:hover {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(0, 243, 255, 0.2);
    transform: translateY(-2px);
  }

  .shard-side-accent {
    width: 3px;
    background: var(--color-primary);
    opacity: 0.3;
    transition: opacity 0.3s;
  }

  .data-shard-card:hover .shard-side-accent {
    opacity: 1;
  }

  .shard-main {
    flex: 1;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .shard-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .shard-filename {
    font-size: 0.75rem;
    font-weight: 700;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 70%;
  }

  .shard-badge {
    font-size: 0.55rem;
    font-weight: 900;
    text-transform: uppercase;
    background: rgba(255, 255, 255, 0.05);
    padding: 2px 8px;
    border-radius: 20px;
    display: flex;
    align-items: center;
    gap: 4px;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .shard-badge .dot {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: #fff;
  }

  .shard-badge.downloading {
    color: var(--color-primary);
    border-color: rgba(0, 243, 255, 0.3);
  }
  .shard-badge.downloading .dot {
    background: var(--color-primary);
    box-shadow: 0 0 5px var(--color-primary);
  }

  .shard-badge.queued {
    color: #ffcc00;
    border-color: rgba(255, 204, 0, 0.3);
  }
  .shard-badge.queued .dot {
    background: #ffcc00;
  }

  .shard-progress-block {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .shard-track {
    height: 2px;
    background: rgba(0, 0, 0, 0.4);
    border-radius: 1px;
    width: 100%;
    position: relative;
  }

  .shard-fill {
    height: 100%;
    background: var(--color-primary);
    position: relative;
    border-radius: 1px;
    transition: width 0.3s ease;
  }

  .fill-glow {
    position: absolute;
    top: 0;
    right: 0;
    width: 20px;
    height: 100%;
    background: linear-gradient(90deg, transparent, #fff);
    opacity: 0.3;
  }

  .shard-meta {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-family: var(--font-mono);
    font-size: 0.6rem;
    color: var(--text-muted);
  }

  .empty-shard-placeholder {
    grid-column: 1 / -1;
    height: 120px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1rem;
    color: var(--text-muted);
    opacity: 0.4;
    border: 1px dashed rgba(255, 255, 255, 0.1);
  }

  /* Netflow Telemetry Upgrade */
  .netflow-telemetry {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    margin-bottom: 2rem;
    padding: 0 0.5rem;
  }

  .telemetry-main {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .telemetry-core {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    color: var(--color-primary);
  }

  .dl-pulse {
    font-size: 2rem !important;
    animation: tele-pulse 2s infinite ease-in-out;
  }

  @keyframes tele-pulse {
    0% {
      transform: scale(1);
      opacity: 0.8;
    }
    50% {
      transform: scale(1.1);
      opacity: 1;
      text-shadow: 0 0 10px var(--color-primary);
    }
    100% {
      transform: scale(1);
      opacity: 0.8;
    }
  }

  .core-text {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
  }

  .core-text .v {
    font-size: 2.2rem;
    font-weight: 800;
    font-family: var(--font-mono);
    letter-spacing: -0.05em;
  }

  .core-text .u {
    font-size: 1rem;
    font-weight: 700;
    opacity: 0.7;
  }

  .telemetry-label {
    font-size: 0.6rem;
    font-weight: 900;
    letter-spacing: 0.1em;
    color: var(--text-muted);
    text-transform: uppercase;
  }

  .telemetry-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1.5rem;
    text-align: right;
  }

  .tele-item {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .tele-item .l {
    font-size: 0.55rem;
    font-weight: 800;
    color: var(--text-muted);
  }

  .tele-item .v {
    font-size: 0.8rem;
    font-weight: 700;
    font-family: var(--font-mono);
    color: #fff;
  }

  /* Compact Header Stats */
  .header-stats-row {
    display: flex;
    gap: 1.5rem;
    align-items: center;
    margin-right: 1.5rem;
  }

  .h-stat {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    font-family: var(--font-mono);
  }

  .h-stat .l {
    font-size: 0.55rem;
    font-weight: 800;
    color: var(--text-muted);
    letter-spacing: 0.05em;
  }

  .h-stat .v {
    font-size: 0.9rem;
    font-weight: 900;
  }

  .color-secondary {
    color: var(--color-secondary);
  }
  .color-warning {
    color: #ffcc00;
  }
  .color-success {
    color: var(--color-primary);
  }

  .toolbar-actions-dock {
    position: absolute;
    top: 1.25rem;
    right: 1.25rem;
    display: flex;
    align-items: center;
  }
</style>
