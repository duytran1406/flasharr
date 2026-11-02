<script lang="ts">
  import { formatDiskSize } from "$lib/stores/arr";

  export interface SeriesLibraryStats {
    count: number;
    totalEps: number;
    fileEps: number;
    missing: number;
    totalSize: number;
  }

  export interface MovieLibraryStats {
    count: number;
    withFile: number;
    missing: number;
    totalSize: number;
  }

  interface Props {
    activeTab: "tv" | "movies" | "collections";
    seriesStats: SeriesLibraryStats;
    movieStats: MovieLibraryStats;
    collectionsCount: number;
    collectionMovieCount: number;
  }

  let {
    activeTab,
    seriesStats,
    movieStats,
    collectionsCount,
    collectionMovieCount,
  }: Props = $props();
</script>

<div class="stats-strip">
  {#if activeTab === "tv"}
    <div class="stat-chip">
      <span class="sl">SERIES</span><span class="sv">{seriesStats.count}</span>
    </div>
    <div class="stat-chip">
      <span class="sl">EPISODES</span><span class="sv"
        >{seriesStats.fileEps}/{seriesStats.totalEps}</span
      >
    </div>
    <div class="stat-chip warn">
      <span class="sl">MISSING</span><span class="sv">{seriesStats.missing}</span>
    </div>
    <div class="stat-chip">
      <span class="sl">SIZE</span><span class="sv"
        >{formatDiskSize(seriesStats.totalSize)}</span
      >
    </div>
  {:else if activeTab === "movies"}
    <div class="stat-chip">
      <span class="sl">MOVIES</span><span class="sv">{movieStats.count}</span>
    </div>
    <div class="stat-chip">
      <span class="sl">DOWNLOADED</span><span class="sv">{movieStats.withFile}</span>
    </div>
    <div class="stat-chip warn">
      <span class="sl">MISSING</span><span class="sv">{movieStats.missing}</span>
    </div>
    <div class="stat-chip">
      <span class="sl">SIZE</span><span class="sv"
        >{formatDiskSize(movieStats.totalSize)}</span
      >
    </div>
  {:else}
    <div class="stat-chip">
      <span class="sl">COLLECTIONS</span><span class="sv">{collectionsCount}</span>
    </div>
    <div class="stat-chip">
      <span class="sl">MOVIES</span><span class="sv">{collectionMovieCount}</span>
    </div>
  {/if}
</div>

<style>
  .stats-strip {
    display: flex;
    gap: .75rem;
    padding: .9rem 1rem;
    flex-wrap: wrap;
  }
  .stat-chip {
    display: inline-flex;
    align-items: center;
    gap: .45rem;
    padding: .55rem .8rem;
    background: #ffffff08;
    border: 1px solid rgba(255, 255, 255, .05);
    border-radius: 10px;
  }
  .stat-chip.warn {
    border-color: #f59e0b40;
  }
  .sl {
    font-size: .64rem;
    font-weight: 800;
    letter-spacing: .1em;
    color: var(--text-muted);
  }
  .sv {
    font-size: .8rem;
    font-weight: 800;
    color: #fff;
    font-family: var(--font-mono,monospace);
  }
  @media (max-width: 768px) {
    .stats-strip {
      display: none;
    }
  }
</style>
