<script lang="ts">
  import { onMount, untrack } from "svelte";
  import {
    anime,
    stagger,
    countUp,
    animateRing,
    revealOnScroll,
  } from "$lib/animations";
  import { get } from "svelte/store";
  import Carousel3D from "$lib/components/Carousel3D.svelte";
  import { MediaCard } from "$lib/components";
  import CalendarWidget from "$lib/components/dashboard/CalendarWidget.svelte";
  import StorageWidget from "$lib/components/dashboard/StorageWidget.svelte";
  import LibraryCoverage from "$lib/components/dashboard/LibraryCoverage.svelte";
  import RecentlyAdded from "$lib/components/dashboard/RecentlyAdded.svelte";
  import NetflowStats from "$lib/components/dashboard/NetflowStats.svelte";
  import {
    engineStats,
    formatSpeed,
    downloads,
    downloadStore,
  } from "$lib/stores/downloads";
  import {
    fetchLibraryOverview,
    libraryOverview,
    fetchArrStatus,
    arrStatus,
  } from "$lib/stores/arr";
  import { integrations } from "$lib/stores/settings";
  import { accountStore } from "$lib/stores/account.svelte";
  import Badge from "$lib/components/ui/Badge.svelte";

  // Trending carousel state
  let trendingItems = $state<any[]>([]);
  let trendingLoading = $state(true);

  // Speed history for graph (last 30 readings)
  let speedHistory = $state<number[]>(Array.from({ length: 30 }, () => 0));
  let sessionPeak = $state(0);
  let sessionTotalBytes = $state(0);

  // Engine stats from WebSocket
  let stats = $derived($engineStats);
  let activeCount = $derived(stats.active_downloads);
  let queuedCount = $derived(stats.queued);

  let speedValueParts = $derived.by(() => {
    const bytes = stats.total_speed || 0;
    if (bytes === 0) return { v: "0.0", u: "MB/s" };
    const k = 1024;
    const sizes = ["B/s", "KB/s", "MB/s", "GB/s", "TB/s"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    const val = parseFloat(
      (bytes / Math.pow(k, i)).toFixed(i >= 2 ? 1 : i === 1 ? 0 : 0),
    );
    return { v: val.toString(), u: sizes[i] };
  });

  // Library stats - reactive from store
  let libraryStats = $derived($libraryOverview);

  // Real-time connection status from /api/arr/health endpoint
  // Provides genuinely accurate ONLINE/OFFLINE state (not just "is configured")
  let sonarrConnected = $derived(
    $arrStatus?.sonarr?.connected ?? libraryStats?.sonarr_connected ?? false,
  );
  let radarrConnected = $derived(
    $arrStatus?.radarr?.connected ?? libraryStats?.radarr_connected ?? false,
  );

  // Calculate coverage for library summary
  let movieCoverage = $derived(
    libraryStats && libraryStats.movie_count > 0
      ? Math.round(
          ((libraryStats.movie_count - libraryStats.movies_missing) /
            libraryStats.movie_count) *
            100,
        )
      : 0,
  );
  let episodeCoverage = $derived(
    libraryStats &&
      libraryStats.episodes_with_files + libraryStats.episodes_missing > 0
      ? Math.round(
          (libraryStats.episodes_with_files /
            (libraryStats.episodes_with_files +
              libraryStats.episodes_missing)) *
            100,
        )
      : 0,
  );

  let hasArrIntegration = $derived(
    $integrations.sonarr_enabled || $integrations.radarr_enabled,
  );

  // Active downloads for queue — must read both standalone downloads AND batch items
  // because TASK_BATCH_UPDATE only inserts batch tasks into batchItems, not downloads.
  let activeDownloads = $derived.by(() => {
    const state = $downloadStore;
    const ACTIVE_STATES = new Set([
      "DOWNLOADING",
      "QUEUED",
      "STARTING",
      "WAITING",
    ]);

    // Collect from standalone downloads
    const standalone = Array.from(state.downloads.values()).filter((d) =>
      ACTIVE_STATES.has(d.state),
    );

    // Collect from all expanded batch items
    const fromBatches: any[] = [];
    for (const items of state.batchItems.values()) {
      for (const item of items) {
        if (ACTIVE_STATES.has(item.state)) fromBatches.push(item);
      }
    }

    // Merge + deduplicate by task id, sort DOWNLOADING first
    const seen = new Set<string>();
    const all = [...standalone, ...fromBatches].filter((d) => {
      if (seen.has(d.id)) return false;
      seen.add(d.id);
      return true;
    });

    return all
      .sort((a, b) => {
        const priority = (s: string) =>
          s === "DOWNLOADING"
            ? 0
            : s === "STARTING"
              ? 1
              : s === "WAITING"
                ? 2
                : 3;
        return priority(a.state) - priority(b.state);
      })
      .slice(0, 5);
  });

  // Account details for dashboard
  let account = $derived(accountStore.primaryFormatted);
  let isAccountVip = $derived(accountStore.isVip);

  // Update speed history when stats change
  $effect(() => {
    const speed = stats.total_speed;
    const speedMBps = speed / (1024 * 1024);

    untrack(() => {
      speedHistory = [...speedHistory.slice(1), speedMBps];
      if (speedMBps > sessionPeak) {
        sessionPeak = speedMBps;
      }
      sessionTotalBytes += speed;
    });
  });

  onMount(async () => {
    // Set dynamic header
    const headerContainer = document.getElementById("header-dynamic-content");
    if (headerContainer) {
      headerContainer.innerHTML = `
        <div style="display: flex; align-items: center; gap: 0.75rem;">
          <span class="material-icons" style="color: var(--color-primary); font-size: 1.5rem;">dashboard</span>
          <h1 style="font-size: 0.9rem; font-weight: 800; letter-spacing: 0.1em; text-transform: uppercase; margin: 0; color: #fff;">Dashboard</h1>
        </div>
      `;
    }

    // Reactively fetch library stats + real connection status when integrations load
    $effect(() => {
      if (hasArrIntegration) {
        untrack(() => {
          fetchLibraryOverview();
          fetchArrStatus(); // fetch real ONLINE/OFFLINE status per service
        });
      }
    });

    // If integrations were already loaded before this $effect ran, kick off immediately
    if (hasArrIntegration) {
      fetchLibraryOverview();
      fetchArrStatus();
    }

    // Fetch trending content for hero carousel from TMDB
    try {
      const res = await fetch(
        "/api/discovery/popular-today?type=movie&limit=10",
      );
      if (res.ok) {
        const data = await res.json();
        untrack(() => {
          trendingItems = (data.results || []).map((item: any) => {
            // The API returns poster_url, we need to build backdrop_path
            // TMDB trending API should have backdrop_path directly
            let backdropPath = item.backdrop_path;

            // If no backdrop_path, try to extract from poster_url format or use poster
            if (!backdropPath && item.poster_url) {
              // Convert poster to backdrop by using the same ID with backdrop path
              // poster_url format: https://image.tmdb.org/t/p/w500/xxxxx.jpg
              const match = item.poster_url.match(/\/([^/]+\.jpg)$/);
              if (match) {
                backdropPath = "/" + match[1]; // Use same image as fallback
              }
            }

            return {
              id: item.id,
              title: item.title || item.name,
              backdrop_path: backdropPath,
              vote_average: item.vote_average || item.score,
              release_date: item.release_date || item.first_air_date || "",
              overview: item.overview || item.description || "",
              media_type: item.media_type || "movie",
            };
          });
        });
      }
    } catch (e) {
      console.error("Failed to load trending:", e);
    } finally {
      trendingLoading = false;
    }
  });

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }
</script>

<svelte:head>
  <title>Dashboard - Flasharr</title>
</svelte:head>

<div class="dashboard-page">
  <!-- COLUMN A (70%) -->
  <div class="column-a">
    <!-- Row 1: Hero Banner with 3D Carousel (50%) -->
    <section class="hero-section">
      <div class="hero-header">
        <span class="material-icons">local_fire_department</span>
        <h3>TRENDING</h3>
        <a href="/discover" class="view-all">
          Explore More
          <span class="material-icons">arrow_forward</span>
        </a>
      </div>

      <div class="carousel-viewport">
        {#if trendingLoading}
          <div class="hero-skeleton">
            <div class="shimmer"></div>
          </div>
        {:else if trendingItems.length > 0}
          <Carousel3D
            items={trendingItems}
            height={180}
            width={600}
            space={320}
          >
            {#snippet children(item)}
              <a
                href="/{item.media_type}/{item.id}"
                class="banner-card"
                style="background-image: url('https://image.tmdb.org/t/p/w780{item.backdrop_path}')"
              >
                <!-- Gradient overlay -->
                <div class="banner-overlay">
                  <!-- Title row (always visible) -->
                  <div class="overlay-main">
                    <div class="title-row">
                      <h3 class="banner-title">{item.title}</h3>
                      <span class="banner-year"
                        >{item.release_date?.substring(0, 4) || ""}</span
                      >
                    </div>
                    <div class="banner-stats">
                      {#if item.vote_average}
                        <div class="stat-rating">
                          <span class="material-icons">star</span>
                          <span>{item.vote_average.toFixed(1)}</span>
                        </div>
                      {/if}
                      <div class="stat-type">
                        {item.media_type?.toUpperCase() || "MOVIE"}
                      </div>
                    </div>
                  </div>
                  <!-- Expandable overview (on hover) -->
                  {#if item.overview}
                    <div class="overlay-expand">
                      <p class="expand-overview">{item.overview}</p>
                    </div>
                  {/if}
                </div>
              </a>
            {/snippet}
          </Carousel3D>
        {:else}
          <div class="hero-empty">
            <span class="material-icons">movie_filter</span>
            <p>No trending content</p>
          </div>
        {/if}
      </div>
    </section>

    <!-- Row 2: Status Cards (15%) -->
    <section class="status-section">
      <!-- Library Summary -->
      <a
        href="/library"
        class="status-card-v5 card-library"
        use:revealOnScroll={{ y: 20, delay: 0 }}
      >
        <div class="sc-head">
          <span class="material-icons sc-icon">video_library</span>
          <span class="sc-label">LIBRARY SUMMARY</span>
        </div>
        <div class="sc-banner purple-banner">
          <div class="banner-dots"></div>
          {#if libraryStats}
            <div class="library-stats-row">
              <div class="library-stat">
                <span class="stat-value" use:countUp={libraryStats.movie_count}
                  >{libraryStats.movie_count}</span
                >
                <span class="stat-label">MOVIES</span>
              </div>
              <div class="library-stat">
                <span
                  class="stat-value"
                  use:countUp={libraryStats.total_episodes}
                  >{libraryStats.total_episodes}</span
                >
                <span class="stat-label">EPISODES</span>
              </div>
              <div class="coverage-ring">
                <svg viewBox="0 0 48 48" class="ring-svg">
                  <circle cx="24" cy="24" r="20" class="ring-track" />
                  <circle
                    cx="24"
                    cy="24"
                    r="20"
                    class="ring-fill"
                    use:animateRing={{
                      progress: Math.round(
                        (movieCoverage + episodeCoverage) / 2,
                      ),
                    }}
                  />
                </svg>
                <div class="ring-text">
                  <span class="ring-value"
                    >{Math.round((movieCoverage + episodeCoverage) / 2)}</span
                  >
                  <span class="ring-pct">%</span>
                </div>
              </div>
            </div>
          {:else}
            <div class="sc-disconnected">
              <span class="material-icons">cloud_off</span>
              <span>DISCONNECTED</span>
            </div>
          {/if}
        </div>
      </a>

      <!-- Account Status -->
      <a
        href="/settings"
        class="status-card-v5 card-account"
        class:card-warn={!isAccountVip}
        use:revealOnScroll={{ y: 20, delay: 80 }}
      >
        <div class="sc-head">
          <span class="material-icons sc-icon"
            >{isAccountVip ? "verified_user" : "warning_amber"}</span
          >
          <span class="sc-label">ACCOUNT STATUS</span>
        </div>
        <div class="sc-banner red-banner">
          <div class="banner-dots"></div>
          <div class="account-info-v5">
            <div class="acc-email-v5">{account.email}</div>
            <div class="acc-badges">
              <Badge
                text={account.rank}
                variant={isAccountVip ? "vip" : "danger"}
                size="sm"
              />
              {#if !isAccountVip}
                <Badge text="No VIP" variant="warning" size="xs" />
              {/if}
              <span class="acc-expiry-v5">{account.expiry}</span>
            </div>
          </div>
        </div>
      </a>

      <!-- System Health -->
      <div
        class="status-card-v5 card-system"
        use:revealOnScroll={{ y: 20, delay: 160 }}
      >
        <div class="sc-banner green-banner">
          <div class="banner-dots"></div>
          <div class="sc-head">
            <span class="material-icons sc-icon">dns</span>
            <span class="sc-label">SYSTEM HEALTH</span>
          </div>
          <div class="health-nodes">
            <div
              class="health-node"
              class:online={radarrConnected}
              title="Radarr Status"
            >
              <div class="node-icon-wrap">
                <img
                  src="https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/radarr.png"
                  alt="Radarr"
                />
                <span class="pulse-dot"></span>
              </div>
              <span class="node-label"
                >{radarrConnected ? "ONLINE" : "OFFLINE"}</span
              >
            </div>
            <div
              class="health-node"
              class:online={sonarrConnected}
              title="Sonarr Status"
            >
              <div class="node-icon-wrap">
                <img
                  src="https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/sonarr.png"
                  alt="Sonarr"
                />
                <span class="pulse-dot"></span>
              </div>
              <span class="node-label"
                >{sonarrConnected ? "ONLINE" : "OFFLINE"}</span
              >
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Row 3: Recent Library (35%) -->
    <section class="library-section">
      <RecentlyAdded enabled={sonarrConnected || radarrConnected} />
    </section>
  </div>

  <!-- COLUMN B (30%) - Card Drawer -->
  <div class="column-b">
    <!-- Mini Calendar -->
    <div class="drawer-card premium-card">
      <div class="card-header-premium">
        <span class="material-icons">calendar_month</span>
        <span class="label-text">UPCOMING SCHEDULE</span>
        <a href="/calendar" class="view-link-premium">
          <span class="material-icons">arrow_forward</span>
        </a>
      </div>
      <div class="card-body-premium">
        <CalendarWidget compact enabled={sonarrConnected || radarrConnected} />
      </div>
    </div>

    <!-- Netflow Widget -->
    <NetflowStats
      {speedHistory}
      {sessionPeak}
      {sessionTotalBytes}
      currentSpeedValue={speedValueParts.v}
      currentSpeedUnit={speedValueParts.u}
    />

    <!-- Download Queue -->
    <div class="drawer-card premium-card">
      <div class="card-header-premium">
        <span class="material-icons">download</span>
        <span class="label-text">ACTIVE QUEUE</span>
        <a href="/downloads" class="view-link-premium">
          <span class="material-icons">arrow_forward</span>
        </a>
      </div>
      <div class="card-body-premium">
        {#if activeDownloads.length > 0}
          {#each activeDownloads as dl}
            <div class="queue-item">
              <div class="queue-info">
                <span class="queue-name" title={dl.filename}>{dl.filename}</span
                >
                <div class="queue-progress-bar">
                  <div
                    class="progress-fill"
                    style="width: {dl.progress || 0}%"
                  ></div>
                </div>
              </div>
              <span class="queue-percent">{(dl.progress || 0).toFixed(0)}%</span
              >
            </div>
          {/each}
        {:else}
          <div class="queue-empty">
            <span class="material-icons">check_circle</span>
            <span>No active downloads</span>
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .dashboard-page {
    display: grid;
    grid-template-columns: 70% 30%;
    /* Fill whatever .view-container gives us — don't recompute 100vh here.
       The parent already subtracts the header; we just need to fill it. */
    height: 100%;
    gap: 1rem;
    padding: 1rem;
    overflow: hidden;
    box-sizing: border-box;
  }

  /* COLUMN A — scrolls its own content */
  .column-a {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .hero-section {
    flex: 0 0 auto;
    min-height: 320px;
    display: flex;
    flex-direction: column;
    margin-bottom: 0.15rem;
    position: relative;
    overflow: hidden;
  }

  .carousel-viewport {
    flex: 1;
    overflow: hidden;
    position: relative;
    padding-top: 3.5%;
    mask-image: linear-gradient(
      to right,
      transparent,
      black 5%,
      black 95%,
      transparent
    );
  }

  .hero-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
    padding: 0 0.5rem;
  }

  .hero-header .material-icons {
    color: #ff6b00;
    font-size: 1.25rem;
  }

  .hero-header h3 {
    margin: 0;
    font-size: 0.8rem;
    font-weight: 800;
    letter-spacing: 0.1em;
    color: var(--text-primary);
  }

  .hero-header .view-all {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.7rem;
    font-weight: 600;
    color: var(--text-muted);
    text-decoration: none;
    transition: color 0.2s;
  }

  .hero-header .view-all:hover {
    color: var(--color-primary);
  }

  .hero-header .view-all .material-icons {
    font-size: 14px;
    color: inherit;
  }

  .hero-skeleton {
    flex: 1;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    position: relative;
    overflow: hidden;
  }

  .hero-skeleton .shimmer {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(255, 255, 255, 0.05),
      transparent
    );
    animation: shimmer 1.5s infinite;
  }

  @keyframes shimmer {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(100%);
    }
  }

  .hero-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    opacity: 0.5;
  }

  .hero-empty .material-icons {
    font-size: 48px;
    margin-bottom: 0.5rem;
  }

  /* Banner Card Styles */
  .banner-card {
    display: block;
    width: 100%;
    height: 100%;
    border-radius: 12px;
    background-size: cover;
    background-position: center;
    position: relative;
    overflow: hidden;
    text-decoration: none;
    transition:
      transform 0.3s,
      box-shadow 0.3s;
  }

  .banner-card:hover {
    transform: scale(1.02);
    box-shadow: 0 8px 32px rgba(0, 243, 255, 0.3);
  }

  /* Banner overlay (gradient background) */
  .banner-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    padding: 1rem;
    background: linear-gradient(
      to top,
      rgba(0, 0, 0, 0.95) 0%,
      rgba(0, 0, 0, 0.6) 30%,
      rgba(0, 0, 0, 0.2) 60%,
      transparent 100%
    );
    border-radius: 12px;
    transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .banner-card:hover .banner-overlay {
    background: linear-gradient(
      to top,
      rgba(0, 0, 0, 0.98) 0%,
      rgba(0, 0, 0, 0.8) 50%,
      rgba(0, 0, 0, 0.4) 100%
    );
  }

  .overlay-main {
    transform: translateY(0);
    transition: transform 0.4s ease;
  }

  .title-row {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 0.75rem;
    margin-bottom: 0.5rem;
  }

  .banner-title {
    margin: 0;
    font-size: 1rem;
    font-weight: 800;
    color: #fff;
    line-height: 1.2;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
  }

  .banner-year {
    font-size: 0.8rem;
    font-weight: 700;
    color: var(--color-primary, #00f3ff);
    font-family: var(--font-mono, monospace);
    opacity: 0.9;
    flex-shrink: 0;
  }

  .banner-stats {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .stat-rating {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.75rem;
    font-weight: 600;
    color: #ffd700;
  }

  .stat-rating .material-icons {
    font-size: 14px;
  }

  .stat-type {
    background: rgba(255, 255, 255, 0.1);
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 0.65rem;
    font-weight: 600;
    letter-spacing: 0.05em;
    color: rgba(255, 255, 255, 0.7);
  }

  /* Expandable overview (hidden by default, expands on hover) */
  .overlay-expand {
    max-height: 0;
    overflow: hidden;
    opacity: 0;
    transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .banner-card:hover .overlay-expand {
    max-height: 100px;
    opacity: 1;
    margin-top: 0.75rem;
  }

  .expand-overview {
    margin: 0;
    font-size: 0.75rem;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.7);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .status-section {
    flex: 0 0 15%;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1rem;
    min-height: 0;
  }

  .library-section {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  /* ══════════════════════════════════════════════
     STATUS CARDS V5 — Enhanced Glassmorphism
     Design System: MASTER.md compliant
     ══════════════════════════════════════════════ */

  .status-card-v5 {
    position: relative;
    display: flex;
    flex-direction: column;
    background: var(--bg-glass, rgba(8, 10, 15, 0.8));
    backdrop-filter: blur(24px);
    -webkit-backdrop-filter: blur(24px);
    border: 1px solid
      color-mix(
        in srgb,
        var(--card-accent, rgba(255, 255, 255, 0.08)) 30%,
        transparent
      );
    border-radius: 12px;
    padding: 0;
    overflow: hidden;
    text-decoration: none;
    color: inherit;
    transition:
      transform var(--duration-normal, 250ms) cubic-bezier(0.16, 1, 0.3, 1),
      border-color var(--duration-normal, 250ms),
      box-shadow var(--duration-slow, 350ms);
    box-shadow: var(--shadow-glass, 0 4px 24px rgba(0, 0, 0, 0.4));
  }

  .status-card-v5:hover {
    transform: translateY(-3px);
    border-color: color-mix(
      in srgb,
      var(--card-accent, rgba(255, 255, 255, 0.15)) 80%,
      transparent
    );
    box-shadow:
      0 0 0 1px
        color-mix(in srgb, var(--card-accent, transparent) 40%, transparent),
      0 8px 32px rgba(0, 0, 0, 0.5),
      0 0 28px
        color-mix(in srgb, var(--card-accent, transparent) 25%, transparent);
  }

  /* ── Status Card Banner (full-card) ── */
  .sc-banner {
    position: relative;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    align-items: stretch;
    flex: 1; /* fills the whole card */
    padding: 0.85rem 1rem;
    gap: 0.75rem;
    border-radius: 11px; /* same as card */
  }
  .sc-banner .banner-dots {
    position: absolute;
    inset: 0;
    background-image: radial-gradient(rgba(0, 0, 0, 0.25) 1px, transparent 1px);
    background-size: 6px 6px;
    pointer-events: none;
    border-radius: inherit;
  }

  /* Card accent variants */
  .card-library {
    --card-accent: #a78bfa;
  } /* purple */
  .card-account {
    --card-accent: #ef4444;
  } /* red    */
  .card-system {
    --card-accent: #22c55e;
  } /* green  */

  /* Card head overlays the top of banner */
  .sc-head {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    padding: 0.55rem 0.85rem;
    flex-shrink: 0;
    position: relative;
    z-index: 2; /* sits above the banner layer */
    /* subtle separator at bottom */
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(255, 255, 255, 0.02);
  }
  .sc-icon {
    font-size: 1rem;
    color: var(--card-accent, #00f3ff);
    filter: drop-shadow(0 0 6px var(--card-accent, #00f3ff));
  }
  .sc-label {
    font-size: 0.58rem;
    font-weight: 900;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.5);
    font-family: var(--font-mono, monospace);
  }

  /* ── Banner: fills remaining card space, contains data ── */
  .sc-banner {
    position: absolute;
    inset: 0; /* fills entire card */
    display: flex;
    flex-direction: column;
    justify-content: flex-end; /* content sits at bottom half */
    padding: 0.75rem 0.9rem;
    gap: 0.5rem;
    z-index: 1; /* behind sc-head which is z-index:2 */
  }
  .sc-banner .banner-dots {
    position: absolute;
    inset: 0;
    background-image: radial-gradient(rgba(0, 0, 0, 0.28) 1px, transparent 1px);
    background-size: 6px 6px;
    pointer-events: none;
  }
  /* Top separator line in banner accent color */
  .sc-banner::before {
    content: "";
    position: absolute;
    top: 0;
    left: 8%;
    right: 8%;
    height: 1px;
    background: var(--card-accent, #00f3ff);
    opacity: 0.25;
  }

  /* Banner color variants */
  .sc-banner.purple-banner {
    background: linear-gradient(
      160deg,
      rgba(167, 139, 250, 0.22) 0%,
      rgba(120, 80, 220, 0.08) 55%,
      transparent 100%
    );
  }
  .sc-banner.red-banner {
    background: linear-gradient(
      160deg,
      rgba(239, 68, 68, 0.2) 0%,
      rgba(180, 30, 30, 0.07) 55%,
      transparent 100%
    );
  }
  .sc-banner.green-banner {
    background: linear-gradient(
      160deg,
      rgba(34, 197, 94, 0.2) 0%,
      rgba(20, 140, 60, 0.07) 55%,
      transparent 100%
    );
  }

  .sc-disconnected {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--text-muted, #64748b);
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    font-family: var(--font-mono, monospace);
    opacity: 0.5;
  }

  .sc-disconnected .material-icons {
    font-size: 16px;
  }

  /* ── LIBRARY CARD ──────────────────────────── */

  .library-stats-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .library-stat {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .stat-value {
    font-size: 1.35rem;
    font-weight: 800;
    color: var(--text-primary, #e2e8f0);
    font-family: var(--font-mono, monospace);
    letter-spacing: 0.05em;
    line-height: 1;
  }

  .stat-label {
    font-size: 0.5rem;
    font-weight: 700;
    color: var(--text-muted, #64748b);
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  /* Coverage Ring */
  .coverage-ring {
    position: relative;
    width: 48px;
    height: 48px;
    margin-left: auto;
    flex-shrink: 0;
  }

  .ring-svg {
    width: 100%;
    height: 100%;
    transform: rotate(-90deg);
  }

  .ring-track {
    fill: none;
    stroke: rgba(255, 255, 255, 0.06);
    stroke-width: 3;
  }

  .ring-fill {
    fill: none;
    stroke: var(--card-accent, #a78bfa);
    stroke-width: 3;
    stroke-linecap: round;
    stroke-dasharray: 125.6;
    transition: stroke-dashoffset 0.6s cubic-bezier(0.16, 1, 0.3, 1);
    filter: drop-shadow(0 0 4px var(--card-accent, #a78bfa));
  }

  .ring-text {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0;
  }

  .ring-value {
    font-size: 0.85rem;
    font-weight: 900;
    color: var(--card-accent, #a78bfa);
    font-family: var(--font-mono, monospace);
    text-shadow: 0 0 10px var(--card-accent, rgba(167, 139, 250, 0.4));
    line-height: 1;
  }

  .ring-pct {
    font-size: 0.45rem;
    font-weight: 700;
    color: var(--card-accent, #a78bfa);
    opacity: 0.7;
    line-height: 1;
    margin-top: 0.1rem;
  }

  /* ── ACCOUNT CARD ─────────────────────────── */

  .account-info-v5 {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .acc-email-v5 {
    font-size: 0.8rem;
    color: var(--text-primary, #e2e8f0);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .acc-badges {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  /* Card warn: amber accent on the account card */
  .card-account.card-warn {
    --card-accent: #ffaa00;
    border-color: rgba(255, 170, 0, 0.2);
    box-shadow:
      var(--shadow-glass, 0 4px 24px rgba(0, 0, 0, 0.4)),
      0 0 0 1px rgba(255, 170, 0, 0.08) inset;
  }

  .acc-expiry-v5 {
    font-size: 0.65rem;
    color: var(--text-muted, #64748b);
    font-family: var(--font-mono, monospace);
  }

  /* ── SYSTEM HEALTH CARD ───────────────────── */

  .health-nodes {
    display: flex;
    justify-content: space-around;
    align-items: center;
    gap: 1rem;
    padding: 0.25rem 0;
  }

  .health-node {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.4rem;
    transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .health-node:not(.online) {
    filter: grayscale(1);
    opacity: 0.2;
  }

  .node-icon-wrap {
    position: relative;
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .node-icon-wrap img {
    width: 32px;
    height: 32px;
    object-fit: contain;
    transition: transform var(--duration-normal, 250ms);
  }

  .health-node.online .node-icon-wrap img {
    filter: drop-shadow(0 0 8px var(--color-primary, rgba(0, 243, 255, 0.4)));
  }

  .health-node:hover .node-icon-wrap img {
    transform: scale(1.1);
  }

  /* Pulsing status dot */
  .pulse-dot {
    position: absolute;
    bottom: -2px;
    right: -2px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #ef4444; /* red = offline by default */
    border: 2px solid var(--bg-glass, rgba(8, 10, 15, 0.8));
    box-shadow: 0 0 5px rgba(239, 68, 68, 0.4);
  }

  .health-node.online .pulse-dot {
    background: #22c55e;
    box-shadow: 0 0 8px rgba(34, 197, 94, 0.6);
    animation: pulse-glow 2s ease-in-out infinite;
  }

  @keyframes pulse-glow {
    0%,
    100% {
      box-shadow: 0 0 4px rgba(34, 197, 94, 0.3);
    }
    50% {
      box-shadow: 0 0 14px rgba(34, 197, 94, 0.8);
    }
  }

  .node-label {
    font-size: 0.55rem;
    font-weight: 900;
    letter-spacing: 0.1em;
    color: #ef4444; /* red = offline */
    font-family: var(--font-mono, monospace);
  }

  .health-node.online .node-label {
    color: #22c55e; /* green = online */
  }

  /* ══════════════════════════════════════════════
     COLUMN B / QUEUE / PREMIUM-CARD (unchanged)
     ══════════════════════════════════════════════ */

  /* COLUMN B — fills its grid cell, no independent 100vh math */
  .column-b {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    box-sizing: border-box;
  }

  /* Every direct child (drawer-card + netflow section) gets equal share */
  .column-b > :global(*) {
    flex: 1 1 0 !important; /* 0 basis → equal thirds regardless of content */
    min-height: 0;
    overflow: hidden;
  }

  .drawer-card {
    display: flex;
    flex-direction: column;
    border-radius: 12px;
    overflow: hidden;
    min-height: 0;
  }

  /* Queue Styles */
  .queue-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem;
    background: rgba(255, 255, 255, 0.02);
    border-radius: 8px;
  }

  .queue-info {
    flex: 1;
    min-width: 0;
  }

  .queue-name {
    display: block;
    font-size: 0.7rem;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-bottom: 0.25rem;
  }

  .queue-progress-bar {
    height: 4px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--color-primary);
    transition: width 0.3s;
  }

  .queue-percent {
    font-size: 0.7rem;
    font-weight: 800;
    font-family: var(--font-mono);
    color: var(--color-primary);
  }

  .queue-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding: 2rem;
    color: var(--text-muted);
    opacity: 0.5;
  }

  .queue-empty .material-icons {
    font-size: 32px;
  }

  .queue-empty span:not(.material-icons) {
    font-size: 0.75rem;
  }

  /* Premium Unified Card Style (Column B cards) */
  :global(.premium-card) {
    background: var(--bg-glass, rgba(8, 10, 15, 0.8));
    backdrop-filter: blur(24px);
    -webkit-backdrop-filter: blur(24px);
    border: 1px solid var(--border-glass, rgba(255, 255, 255, 0.08));
    border-left: 1px solid var(--card-accent, var(--color-primary));
    border-radius: 12px;
    position: relative;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    min-height: 0;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    overflow: hidden;
    text-decoration: none;
    color: inherit;
    box-shadow: var(--shadow-glass, 0 4px 24px rgba(0, 0, 0, 0.4));
  }

  :global(.premium-card::before) {
    content: "";
    position: absolute;
    top: 5%;
    bottom: 5%;
    left: 0;
    width: 1px;
    background: linear-gradient(
      180deg,
      transparent 0%,
      var(--card-accent, var(--color-primary)) 25%,
      var(--card-accent, var(--color-primary)) 75%,
      transparent 100%
    );
    filter: blur(0.5px);
    z-index: 2;
  }

  /* Novu halftone dissolution glow from left border */
  :global(.premium-card::after) {
    content: "";
    position: absolute;
    top: -20px;
    bottom: -20px;
    left: -60px;
    width: 300px;
    background-image: radial-gradient(
      circle,
      var(--card-accent, var(--color-primary)) 1.2px,
      transparent 1.2px
    );
    background-size: 8px 8px;
    opacity: 0.2;
    pointer-events: none;
    z-index: 0;
    transition: opacity 0.3s;
    mask-image: radial-gradient(
      ellipse at 0% 50%,
      black 0%,
      rgba(0, 0, 0, 0.5) 8%,
      rgba(0, 0, 0, 0.15) 25%,
      rgba(0, 0, 0, 0.03) 45%,
      transparent 60%
    );
    -webkit-mask-image: radial-gradient(
      ellipse at 0% 50%,
      black 0%,
      rgba(0, 0, 0, 0.5) 8%,
      rgba(0, 0, 0, 0.15) 25%,
      rgba(0, 0, 0, 0.03) 45%,
      transparent 60%
    );
  }

  :global(.premium-card:hover:not(.no-hover-lift)) {
    transform: translateY(-4px);
    border-color: var(--border-glass, rgba(255, 255, 255, 0.08));
    border-left-color: var(--card-accent, var(--color-primary));
    box-shadow:
      0 20px 50px rgba(0, 0, 0, 0.5),
      0 0 25px
        color-mix(
          in srgb,
          var(--card-accent, var(--color-primary)) 25%,
          transparent
        );
  }

  :global(.premium-card:hover::before) {
    opacity: 1;
  }

  :global(.premium-card:hover::after) {
    opacity: 0.35;
  }

  :global(.card-header-premium) {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 1.25rem;
    padding: 0 0.5rem;
    flex-shrink: 0;
  }

  :global(.card-header-premium .material-icons) {
    font-size: 1.1rem;
    color: var(--text-muted);
    opacity: 0.8;
  }

  :global(.card-header-premium .label-text) {
    font-size: 0.65rem;
    font-weight: 900;
    letter-spacing: 0.2rem;
    text-transform: uppercase;
    color: var(--text-muted);
    font-family: var(--font-mono, monospace);
  }

  :global(.view-link-premium) {
    margin-left: auto;
    color: var(--text-muted);
    text-decoration: none;
    transition: color 0.2s;
    display: flex;
    align-items: center;
  }

  :global(.view-link-premium:hover) {
    color: var(--color-primary);
  }

  :global(.card-body-premium) {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  :global(.card-header-premium.no-margin) {
    margin-bottom: 0.5rem;
  }

  /* Mobile Responsive */
  @media (max-width: 1024px) {
    .dashboard-page {
      grid-template-columns: 1fr;
      grid-template-rows: auto auto auto;
      overflow-y: auto;
    }

    .column-a {
      order: 1;
    }

    .column-b {
      order: 2;
      flex-direction: row;
      flex-wrap: wrap;
    }

    .drawer-card {
      flex: 1 1 calc(50% - 0.5rem);
      min-width: 200px;
    }

    .hero-section {
      flex: 0 0 300px;
    }

    .status-section {
      flex: 0 0 auto;
    }

    .library-section {
      flex: 0 0 auto;
    }
  }

  @media (max-width: 768px) {
    .status-section {
      grid-template-columns: 1fr;
    }

    .column-b {
      flex-direction: column;
    }

    .drawer-card {
      flex: 0 0 auto;
      min-height: 200px;
    }
  }
</style>
