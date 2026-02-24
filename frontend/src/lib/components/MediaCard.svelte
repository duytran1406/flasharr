<script lang="ts">
  import {
    getSeriesPoster,
    getMoviePoster,
    formatDiskSize,
    type SonarrSeries,
    type RadarrMovie,
  } from "$lib/stores/arr";

  interface Props {
    item: SonarrSeries | RadarrMovie | any;
    type: "series" | "movie";
    viewMode?: "grid" | "list";
  }

  let { item, type, viewMode = "grid" } = $props();

  // Derived helpers
  let isSeries = $derived(type === "series");

  // Series helpers
  let epCount = $derived(isSeries ? item.statistics?.episodeCount || 0 : 0);
  let epFileCount = $derived(
    isSeries ? item.statistics?.episodeFileCount || 0 : 0,
  );
  let missing = $derived(
    isSeries ? epCount - epFileCount : item.hasFile === false ? 1 : 0,
  ); // For movies, just logic check

  let progress = $derived(
    isSeries && epCount > 0 ? Math.round((epFileCount / epCount) * 100) : 0,
  );

  // Poster logic
  let posterUrl = $derived(
    isSeries ? getSeriesPoster(item) : getMoviePoster(item),
  );
  let linkUrl = $derived(
    isSeries
      ? item.tmdbId
        ? `/tv/${item.tmdbId}`
        : `/library/series/${item.id}`
      : `/movie/${item.tmdbId}`,
  );

  function getStatusColor(status?: string): string {
    if (status === "continuing") return "#34d399";
    if (status === "ended") return "#94a3b8";
    if (status === "upcoming") return "#fbbf24";
    return "#64748b";
  }

  let statusTooltip = $derived(() => {
    if (!isSeries) {
      return item.hasFile ? "In Library" : "Wanted / Missing from Library";
    }
    const s = item.status?.toLowerCase();
    if (s === "continuing") return "Currently Airing / New episodes expected";
    if (s === "ended") return "Show has finished its run";
    if (s === "upcoming") return "New season or series announced";
    return "Status unknown";
  });
</script>

<a
  href={linkUrl}
  class="media-card {viewMode === 'list' ? 'list-view' : ''}"
  class:has-missing={missing > 0}
>
  <div class="poster-wrap">
    {#if posterUrl}
      <img src={posterUrl} alt={item.title} class="poster" loading="lazy" />
    {:else}
      <div class="poster placeholder">
        <span class="material-icons">{isSeries ? "live_tv" : "movie"}</span>
      </div>
    {/if}

    <!-- Status / Quality Badges -->
    {#if isSeries && item.status && item.status.toLowerCase() !== "unknown"}
      <div
        class="status-badge"
        style="background: {getStatusColor(item.status)}"
        title={statusTooltip()}
      >
        {item.status}
      </div>
    {:else if !isSeries}
      <div
        class="file-badge"
        class:has-file={item.hasFile === true}
        title={statusTooltip()}
      >
        <span class="material-icons"
          >{item.hasFile ? "check_circle" : "cloud_download"}</span
        >
      </div>
    {/if}

    <!-- Progress Overlay for Series -->
    {#if isSeries}
      <div class="progress-overlay">
        <div class="progress-bar">
          <div class="progress-fill" style="width: {progress}%"></div>
        </div>
        <div class="progress-text">
          {epFileCount}/{epCount}
        </div>
      </div>
    {/if}

    <!-- Missing Badge -->
    {#if missing > 0}
      <div class="missing-badge">
        {isSeries ? `${missing} missing` : "missing"}
      </div>
    {/if}
  </div>

  <div class="card-info">
    <div class="card-title" title={item.title}>{item.title}</div>
    <div class="card-meta">
      {#if item.year}<span>{item.year}</span>{/if}
      {#if isSeries}
        {#if item.statistics?.seasonCount}<span
            >· {item.statistics.seasonCount} seasons</span
          >{/if}
      {:else if item.runtime}<span>· {item.runtime} min</span>{/if}
    </div>
    {#if viewMode === "list"}
      <div class="card-meta size-meta">
        {formatDiskSize(
          isSeries ? item.statistics?.sizeOnDisk || 0 : item.sizeOnDisk || 0,
        )}
      </div>
    {/if}
  </div>
</a>

<style>
  .media-card {
    text-decoration: none;
    color: inherit;
    display: flex;
    flex-direction: column;
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
    border-radius: 8px;
    overflow: hidden;
    position: relative;
    background: transparent;
  }

  .media-card:hover {
    transform: translateY(-4px);
  }

  .media-card:hover .poster {
    filter: brightness(1.1);
  }

  /* List View Overrides */
  .media-card.list-view {
    display: grid;
    grid-template-columns: 50px 1fr;
    gap: 1rem;
    align-items: center;
    padding: 0.5rem;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .media-card.list-view:hover {
    transform: translateX(4px) translateY(0);
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.1);
  }

  .media-card.list-view .poster-wrap {
    width: 50px;
    height: 75px;
    aspect-ratio: auto;
  }

  .poster-wrap {
    position: relative;
    aspect-ratio: 2/3;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 6px;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.2);
  }

  .poster {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: filter 0.3s;
  }

  .poster.placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
  }

  /* Badges */
  .status-badge {
    position: absolute;
    top: 4px;
    right: 4px;
    font-size: 0.4rem;
    font-weight: 900;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    padding: 2px 4px;
    border-radius: 3px;
    color: #fff;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.5);
  }

  .file-badge {
    position: absolute;
    top: 4px;
    right: 4px;
    /* backdrop-filter: blur(4px); */
    /* border-radius: 50%; */
  }

  .file-badge .material-icons {
    font-size: 16px;
    color: #fbbf24; /* Amber for Wanted/Missing */
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.8);
  }
  .file-badge.has-file .material-icons {
    color: #34d399;
  }

  .missing-badge {
    position: absolute;
    bottom: 24px; /* above progress bar if series */
    left: 0;
    right: 0;
    text-align: center;
    background: rgba(251, 191, 36, 0.95);
    color: #000;
    font-size: 0.5rem;
    font-weight: 800;
    text-transform: uppercase;
    padding: 1px 0;
    letter-spacing: 0.05em;
  }

  /* Adjust missing badge for movies (no progress bar) */
  :global(.media-card:not(.has-progress)) .missing-badge {
    bottom: 0;
  }

  /* Progress Bar */
  .progress-overlay {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    background: rgba(0, 0, 0, 0.8);
    padding: 3px 4px;
    display: flex;
    align-items: center;
    gap: 4px;
    backdrop-filter: blur(2px);
  }

  .progress-bar {
    flex: 1;
    height: 3px;
    background: rgba(255, 255, 255, 0.2);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--color-primary);
    border-radius: 2px;
  }

  .progress-text {
    font-family: var(--font-mono, monospace);
    font-size: 0.5rem;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.8);
    white-space: nowrap;
  }

  /* Card Text info */
  .card-info {
    padding: 0.5rem 0.25rem;
    min-width: 0; /* for truncation */
  }

  .card-title {
    font-size: 0.8rem;
    font-weight: 600;
    color: #f1f5f9;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.2;
  }

  .card-meta {
    font-size: 0.65rem;
    color: var(--text-muted);
    opacity: 0.7;
    margin-top: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .size-meta {
    font-family: var(--font-mono);
    color: var(--color-primary);
    opacity: 0.8;
  }
</style>
