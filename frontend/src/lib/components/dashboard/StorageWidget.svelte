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

  interface Props {
    compact?: boolean;
    enabled?: boolean;
  }

  let { compact = false, enabled = true }: Props = $props();

  let library = $state<LibraryOverview | null>(null);
  let storage = $state<DiskSpace[]>([]);

  onMount(async () => {
    if (enabled) {
      try {
        const [lib, disk] = await Promise.all([
          fetchLibraryOverview(),
          fetchDiskSpace(),
        ]);
        library = lib;
        storage = disk;
      } catch (e: any) {
        console.error("Failed to load storage/library data:", e);
        toasts.error(`Dashboard Data Error: ${e.message || "Failed to load"}`);
      }
    }
  });

  function getStrokeDashoffset(pct: number, radius: number) {
    const circumference = 2 * Math.PI * radius;
    return circumference - (pct / 100) * circumference;
  }
</script>

<div class="storage-widget">
  {#if library}
    <div class="stats-row">
      <div class="stat-item">
        <div class="stat-val">{library.series_count}</div>
        <div class="stat-label">SERIES</div>
      </div>
      <div class="stat-item">
        <div class="stat-val">{library.movie_count}</div>
        <div class="stat-label">MOVIES</div>
      </div>
      <div class="stat-item">
        <div class="stat-val">{formatDiskSize(library.total_size_on_disk)}</div>
        <div class="stat-label">SIZE</div>
      </div>
    </div>
  {/if}

  <div class="disks-container">
    {#each storage as disk}
      {@const pct = Math.round(
        ((disk.totalSpace - disk.freeSpace) / disk.totalSpace) * 100,
      )}
      {@const radius = 24}
      {@const circumference = 2 * Math.PI * radius}

      <div class="disk-ring-item">
        <div class="ring-wrapper">
          <svg width="60" height="60" viewBox="0 0 60 60">
            <circle
              cx="30"
              cy="30"
              r={radius}
              stroke="rgba(255,255,255,0.05)"
              stroke-width="4"
              fill="none"
            />
            <circle
              cx="30"
              cy="30"
              r={radius}
              stroke={pct > 90 ? "var(--color-error)" : "var(--color-primary)"}
              stroke-width="4"
              fill="none"
              stroke-dasharray={circumference}
              stroke-dashoffset={getStrokeDashoffset(pct, radius)}
              stroke-linecap="round"
              transform="rotate(-90 30 30)"
            />
          </svg>
          <div class="ring-text">{pct}%</div>
        </div>
        <div class="disk-meta">
          <div class="disk-path" title={disk.path}>{disk.path}</div>
          <div class="disk-free">{formatDiskSize(disk.freeSpace)} FREE</div>
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
  .storage-widget {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    height: 100%;
  }

  .stats-row {
    display: flex;
    justify-content: space-between;
    padding-bottom: 1rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .stat-item {
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .stat-val {
    font-size: 1.1rem;
    font-weight: 900;
    color: var(--text-primary);
  }

  .stat-label {
    font-size: 0.55rem;
    font-weight: 700;
    color: var(--text-muted);
    letter-spacing: 0.05em;
  }

  .disks-container {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    overflow-y: auto;
  }

  .disk-ring-item {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .ring-wrapper {
    position: relative;
    width: 60px;
    height: 60px;
  }

  .ring-text {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    font-size: 0.7rem;
    font-weight: 800;
    color: var(--text-primary);
  }

  .disk-meta {
    flex: 1;
    min-width: 0;
  }

  .disk-path {
    font-size: 0.7rem;
    font-weight: 700;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .disk-free {
    font-size: 0.6rem;
    color: rgba(255, 255, 255, 0.4);
  }
</style>
