<script lang="ts">
  import { onMount } from "svelte";
  import {
    fetchLibraryOverview,
    fetchDiskSpace,
    formatDiskSize,
    type LibraryOverview,
    type DiskSpace,
  } from "$lib/stores/arr";
  import { toasts } from "$lib/stores/toasts";

  let library = $state<LibraryOverview | null>(null);
  let storage = $state<DiskSpace[]>([]);
  let loading = $state(true);

  onMount(async () => {
    try {
      const [lib, disk] = await Promise.all([
        fetchLibraryOverview(),
        fetchDiskSpace(),
      ]);
      library = lib;
      storage = disk;
    } catch (e: any) {
      console.error("Failed to load library overview:", e);
      toasts.error(`Library Widget Error: ${e.message}`);
    } finally {
      loading = false;
    }
  });

  function diskPercent(d: DiskSpace): number {
    if (d.totalSpace === 0) return 0;
    return Math.round(((d.totalSpace - d.freeSpace) / d.totalSpace) * 100);
  }

  function diskColor(pct: number): string {
    if (pct > 90) return "#ff4444";
    if (pct > 75) return "#ffcc00";
    return "var(--color-primary)";
  }
</script>

<div class="library-widget">
  {#if loading}
    <div class="lib-loading">
      <div class="pulse-ring"></div>
      <span>SYNCING LIBRARY...</span>
    </div>
  {:else if !library}
    <div class="lib-empty">
      <span class="material-icons">cloud_off</span>
      <p>ARR SERVICES NOT CONNECTED</p>
    </div>
  {:else}
    <!-- Connection Status -->
    <div class="connection-row">
      <div class="conn-badge" class:active={library.sonarr_connected}>
        <span class="dot"></span>
        SONARR
      </div>
      <div class="conn-badge" class:active={library.radarr_connected}>
        <span class="dot"></span>
        RADARR
      </div>
    </div>

    <!-- Stats Grid -->
    <div class="lib-stats-grid">
      <div class="lib-stat">
        <span class="stat-value">{library.series_count}</span>
        <span class="stat-label">SERIES</span>
      </div>
      <div class="lib-stat">
        <span class="stat-value">{library.movie_count}</span>
        <span class="stat-label">MOVIES</span>
      </div>
      <div class="lib-stat">
        <span class="stat-value">{library.episodes_with_files}</span>
        <span class="stat-label">EPISODES</span>
      </div>
      <div class="lib-stat warn">
        <span class="stat-value"
          >{library.episodes_missing + library.movies_missing}</span
        >
        <span class="stat-label">MISSING</span>
      </div>
    </div>

    <!-- Total Size -->
    <div class="size-row">
      <span class="material-icons">storage</span>
      <span class="size-label">LIBRARY SIZE</span>
      <span class="size-value"
        >{formatDiskSize(library.total_size_on_disk)}</span
      >
    </div>

    <!-- Disk Usage -->
    {#if storage.length > 0}
      <div class="disk-list">
        {#each storage as disk}
          {@const pct = diskPercent(disk)}
          <div class="disk-row">
            <div class="disk-header">
              <span class="disk-path" title={disk.path}
                >{disk.label || disk.path}</span
              >
              <span class="disk-pct" style="color: {diskColor(pct)}"
                >{pct}%</span
              >
            </div>
            <div class="disk-bar">
              <div
                class="disk-fill"
                style="width: {pct}%; background: {diskColor(pct)}"
              ></div>
            </div>
            <div class="disk-sizes">
              <span
                >{formatDiskSize(disk.totalSpace - disk.freeSpace)} / {formatDiskSize(
                  disk.totalSpace,
                )}</span
              >
            </div>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .library-widget {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .lib-loading,
  .lib-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    padding: 1.5rem;
    color: var(--text-muted);
  }

  .lib-empty .material-icons {
    font-size: 32px;
    opacity: 0.2;
  }

  .lib-empty p,
  .lib-loading span {
    font-size: 0.6rem;
    font-weight: 800;
    letter-spacing: 0.15em;
    opacity: 0.5;
  }

  .pulse-ring {
    width: 20px;
    height: 20px;
    border: 2px solid rgba(0, 243, 255, 0.3);
    border-top-color: var(--color-primary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Connection badges */
  .connection-row {
    display: flex;
    gap: 0.5rem;
  }

  .conn-badge {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 0.55rem;
    font-weight: 900;
    letter-spacing: 0.1em;
    color: var(--text-muted);
    opacity: 0.4;
    background: rgba(255, 255, 255, 0.02);
    padding: 3px 8px;
    border-radius: 3px;
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .conn-badge.active {
    opacity: 1;
    color: #00ff80;
    border-color: rgba(0, 255, 128, 0.2);
    background: rgba(0, 255, 128, 0.05);
  }

  .conn-badge .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.2);
  }

  .conn-badge.active .dot {
    background: #00ff80;
    box-shadow: 0 0 6px #00ff80;
  }

  /* Stats Grid */
  .lib-stats-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.5rem;
  }

  .lib-stat {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 0.5rem 0.25rem;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.04);
    border-radius: 3px;
  }

  .stat-value {
    font-family: var(--font-mono, monospace);
    font-size: 1.1rem;
    font-weight: 900;
    color: var(--color-primary);
  }

  .lib-stat.warn .stat-value {
    color: #ffcc00;
  }

  .stat-label {
    font-size: 0.5rem;
    font-weight: 800;
    letter-spacing: 0.1em;
    color: var(--text-muted);
    opacity: 0.6;
  }

  /* Size Row */
  .size-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.5rem;
    background: rgba(255, 255, 255, 0.015);
    border: 1px solid rgba(255, 255, 255, 0.03);
    border-radius: 3px;
    font-size: 0.6rem;
    font-weight: 800;
    letter-spacing: 0.1em;
    color: var(--text-muted);
  }

  .size-row .material-icons {
    font-size: 14px;
    opacity: 0.5;
  }

  .size-label {
    flex: 1;
  }

  .size-value {
    color: var(--color-primary);
    font-family: var(--font-mono, monospace);
  }

  /* Disk Usage */
  .disk-list {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .disk-row {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 0.3rem 0.25rem;
  }

  .disk-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .disk-path {
    font-size: 0.55rem;
    font-weight: 700;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 80%;
  }

  .disk-pct {
    font-family: var(--font-mono, monospace);
    font-size: 0.6rem;
    font-weight: 900;
  }

  .disk-bar {
    height: 3px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 2px;
    overflow: hidden;
  }

  .disk-fill {
    height: 100%;
    border-radius: 2px;
    transition: width 0.5s ease;
  }

  .disk-sizes {
    font-size: 0.5rem;
    color: var(--text-muted);
    opacity: 0.5;
  }
</style>
