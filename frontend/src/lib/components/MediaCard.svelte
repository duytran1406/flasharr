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

  let isSeries = $derived(type === "series");

  let epCount = $derived(isSeries ? item.statistics?.episodeCount || 0 : 0);
  let epFileCount = $derived(
    isSeries ? item.statistics?.episodeFileCount || 0 : 0,
  );
  let missing = $derived(
    isSeries ? epCount - epFileCount : item.hasFile === false ? 1 : 0,
  );

  let progress = $derived(
    isSeries && epCount > 0 ? Math.round((epFileCount / epCount) * 100) : 0,
  );

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

<!-- ─── LIST VIEW ─── -->
{#if viewMode === "list"}
  <a href={linkUrl} class="media-card list-view">
    <div class="list-poster-wrap">
      {#if posterUrl}
        <img src={posterUrl} alt={item.title} class="poster" loading="lazy" />
      {:else}
        <div class="poster placeholder">
          <span class="material-icons">{isSeries ? "live_tv" : "movie"}</span>
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
      <div class="card-meta size-meta">
        {formatDiskSize(
          isSeries ? item.statistics?.sizeOnDisk || 0 : item.sizeOnDisk || 0,
        )}
      </div>
    </div>
  </a>

  <!-- ─── GRID / POSTER CARD VIEW ─── -->
{:else}
  <a href={linkUrl} class="media-card poster-card">
    <!-- Poster image -->
    <div class="poster-wrap">
      {#if posterUrl}
        <img src={posterUrl} alt={item.title} class="poster" loading="lazy" />
      {:else}
        <div class="poster placeholder">
          <span class="material-icons">{isSeries ? "live_tv" : "movie"}</span>
        </div>
      {/if}

      <!-- Top-right badges -->
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

      <!-- Missing badge -->
      {#if missing > 0}
        <div class="missing-badge">
          {isSeries ? `${missing} missing` : "missing"}
        </div>
      {/if}

      <!-- Bottom gradient scrim — title lives here, never cropped -->
      <div class="card-scrim">
        {#if isSeries}
          <div class="scrim-progress">
            <div class="scrim-progress-bar">
              <div class="scrim-progress-fill" style="width: {progress}%"></div>
            </div>
            <span class="scrim-ep-count">{epFileCount}/{epCount}</span>
          </div>
        {/if}
        <div class="scrim-title" title={item.title}>{item.title}</div>
        <div class="scrim-meta">
          {#if item.year}<span>{item.year}</span>{/if}
          {#if isSeries && item.statistics?.seasonCount}
            <span>· {item.statistics.seasonCount}S</span>
          {:else if !isSeries && item.runtime}
            <span>· {item.runtime}m</span>
          {/if}
        </div>
      </div>
    </div>
  </a>
{/if}

<style>
  /* ─── Shared base ─── */
  .media-card {
    text-decoration: none;
    color: inherit;
    display: block;
    border-radius: 10px;
    cursor: pointer;
    position: relative;
  }

  /* ════════════════════════════════════════
     GRID / POSTER CARD
     The card IS the poster — no external
     card-info block, so nothing overflows.
  ════════════════════════════════════════ */
  .media-card.poster-card {
    display: block;
  }

  .poster-wrap {
    position: relative;
    aspect-ratio: 2/3;
    border-radius: 10px;
    overflow: hidden; /* clips poster & badges cleanly */
    background: rgba(255, 255, 255, 0.05);
    box-shadow:
      0 4px 12px rgba(0, 0, 0, 0.4),
      0 1px 3px rgba(0, 0, 0, 0.3);
    transition:
      transform 0.22s cubic-bezier(0.16, 1, 0.3, 1),
      box-shadow 0.22s cubic-bezier(0.16, 1, 0.3, 1);
    will-change: transform;
  }

  .media-card.poster-card:hover .poster-wrap {
    transform: translateY(-5px) scale(1.02);
    box-shadow:
      0 16px 32px rgba(0, 0, 0, 0.55),
      0 4px 8px rgba(0, 0, 0, 0.35);
  }

  .media-card.poster-card:hover .poster {
    filter: brightness(1.08);
  }

  .poster {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    transition: filter 0.3s ease;
  }

  .poster.placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    background: rgba(255, 255, 255, 0.04);
    width: 100%;
    height: 100%;
  }

  /* ─── Top badges ─── */
  .status-badge {
    position: absolute;
    top: 6px;
    right: 6px;
    font-size: 0.4rem;
    font-weight: 900;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 2px 5px;
    border-radius: 4px;
    color: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.6);
    z-index: 2;
  }

  .file-badge {
    position: absolute;
    top: 6px;
    right: 6px;
    z-index: 2;
  }

  .file-badge .material-icons {
    font-size: 16px;
    color: #fbbf24;
    text-shadow: 0 1px 3px rgba(0, 0, 0, 0.9);
  }

  .file-badge.has-file .material-icons {
    color: #34d399;
  }

  .missing-badge {
    position: absolute;
    top: 6px;
    left: 6px;
    background: rgba(251, 191, 36, 0.95);
    color: #000;
    font-size: 0.45rem;
    font-weight: 900;
    text-transform: uppercase;
    padding: 2px 5px;
    border-radius: 4px;
    letter-spacing: 0.05em;
    z-index: 2;
  }

  /* ─── Bottom gradient scrim (title overlay) ─── */
  .card-scrim {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    /* tall gradient for legibility */
    background: linear-gradient(
      to top,
      rgba(5, 5, 10, 0.97) 0%,
      rgba(5, 5, 10, 0.75) 45%,
      transparent 100%
    );
    padding: 0.9rem 0.55rem 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 3px;
    z-index: 1;
    /* Transition on hover to subtly brighten */
    transition: background 0.2s;
  }

  .media-card.poster-card:hover .card-scrim {
    background: linear-gradient(
      to top,
      rgba(5, 5, 10, 1) 0%,
      rgba(5, 5, 10, 0.82) 50%,
      transparent 100%
    );
  }

  .scrim-title {
    font-size: 0.72rem;
    font-weight: 700;
    color: #f8fafc;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.2;
    text-shadow: 0 1px 4px rgba(0, 0, 0, 0.8);
  }

  .scrim-meta {
    font-size: 0.6rem;
    color: rgba(255, 255, 255, 0.55);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .scrim-progress {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-bottom: 2px;
  }

  .scrim-progress-bar {
    flex: 1;
    height: 2px;
    background: rgba(255, 255, 255, 0.18);
    border-radius: 1px;
    overflow: hidden;
  }

  .scrim-progress-fill {
    height: 100%;
    background: var(--color-primary, #00f3ff);
    border-radius: 1px;
    transition: width 0.4s ease;
  }

  .scrim-ep-count {
    font-size: 0.5rem;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.65);
    font-family: var(--font-mono, monospace);
    white-space: nowrap;
    flex-shrink: 0;
  }

  /* ════════════════════════════════════════
     LIST VIEW
  ════════════════════════════════════════ */
  .media-card.list-view {
    display: grid;
    grid-template-columns: 50px 1fr;
    gap: 0.75rem;
    align-items: center;
    padding: 0.5rem;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    overflow: hidden;
    transition:
      transform 0.18s ease,
      background 0.18s ease,
      border-color 0.18s ease;
  }

  .media-card.list-view:hover {
    transform: translateX(4px);
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.1);
  }

  .list-poster-wrap {
    width: 50px;
    height: 75px;
    border-radius: 5px;
    overflow: hidden;
    flex-shrink: 0;
    background: rgba(255, 255, 255, 0.05);
  }

  .list-poster-wrap .poster {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .card-info {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
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
