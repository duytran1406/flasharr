<script lang="ts">
  import {
    getSeriesPoster,
    getMoviePoster,
    formatDiskSize,
    fetchLibraryOverview,
    type SonarrSeries,
    type RadarrMovie,
  } from "$lib/stores/arr";
  import { downloadStore, onDownloadEvent } from "$lib/stores/downloads.svelte";
  import { mediaIdentityHref, resolveMediaIdentity } from "$lib/utils/mediaIdentity";

  interface Props {
    item: SonarrSeries | RadarrMovie | any;
    type: "series" | "movie";
    viewMode?: "grid" | "list";
    /** Hide the top status bar; series then show their progress inside the scrim. */
    disableTopStatusBar?: boolean;
  }

  let { item, type, viewMode = "grid", disableTopStatusBar = false }: Props =
    $props();

  const isSeries = $derived(type === "series");
  let isDownloading = $state(false);

  const epCount = $derived((isSeries && item.statistics?.episodeCount) || 0);
  const epFileCount = $derived(
    (isSeries && item.statistics?.episodeFileCount) || 0,
  );
  const missing = $derived(
    isSeries ? Math.max(0, epCount - epFileCount) : item.hasFile === false ? 1 : 0,
  );
  const progress = $derived(
    isSeries && epCount > 0 ? Math.round((epFileCount / epCount) * 100) : 0,
  );
  const posterUrl = $derived(
    isSeries ? getSeriesPoster(item) : getMoviePoster(item),
  );
  const linkUrl = $derived(
    mediaIdentityHref(
      resolveMediaIdentity({ ...item, type: isSeries ? "tv" : "movie" }),
    ) ?? "#",
  );

  type MovieStatus =
    | "downloading"
    | "downloaded_monitored"
    | "missing_monitored"
    | "missing_unmonitored";
  type SeriesStatus =
    | "downloading"
    | "continuing_full"
    | "monitored_incomplete"
    | "upcoming"
    | "ended"
    | "continuing"
    | "unknown";

  function itemTmdbId(): number | null {
    const raw = item?.tmdbId ?? item?.tmdb_id;
    const value = Number(raw);
    return Number.isFinite(value) && value > 0 ? value : null;
  }

  function movieStatus(): MovieStatus {
    if (isDownloading) return "downloading";
    const monitored = item.monitored === true;
    const hasFile = item.hasFile === true;
    if (hasFile && monitored) return "downloaded_monitored";
    if (!hasFile && monitored) return "missing_monitored";
    return "missing_unmonitored";
  }

  function seriesStatus(): SeriesStatus {
    if (isDownloading) return "downloading";
    const monitored = item.monitored === true;
    const complete = epCount > 0 && epFileCount >= epCount;
    const status = String(item.status || "").toLowerCase();
    if (monitored && epCount > 0 && epFileCount < epCount) {
      return "monitored_incomplete";
    }
    if (status.includes("continuing") && complete) return "continuing_full";
    if (status.includes("upcoming")) return "upcoming";
    if (status.includes("ended")) return "ended";
    if (status.includes("continuing")) return "continuing";
    return "unknown";
  }

  function movieStatusColor(): string {
    const status = movieStatus();
    if (status === "downloading") return "#a855f7";
    if (status === "downloaded_monitored") return "#22c55e";
    if (status === "missing_monitored") return "#ef4444";
    return "#f59e0b";
  }

  function seriesStatusColor(): string {
    const status = seriesStatus();
    if (status === "downloading") return "#a855f7";
    if (status === "continuing_full") return "#06b6d4";
    if (status === "monitored_incomplete") return "#ef4444";
    if (status === "ended") return "#22c55e";
    return "#f59e0b";
  }

  function statusColor(): string {
    return isSeries ? seriesStatusColor() : movieStatusColor();
  }

  function statusLabel(): string {
    if (!isSeries) {
      const status = movieStatus();
      if (status === "downloading") return "DOWNLOADING";
      if (status === "downloaded_monitored") return "DOWNLOADED · MONITORED";
      if (status === "missing_monitored") return "MISSING · MONITORED";
      return "MISSING · UNMONITORED";
    }
    const status = seriesStatus();
    if (status === "downloading") return "DOWNLOADING";
    if (status === "continuing_full") return "CONTINUING · FULLY CAUGHT UP";
    if (status === "monitored_incomplete") return "MONITORED · INCOMPLETE";
    if (status === "upcoming") return "UPCOMING";
    if (status === "ended") return "ENDED";
    if (status === "continuing") return "CONTINUING";
    return "UNKNOWN";
  }

  function showStatusBar(): boolean {
    return !disableTopStatusBar && !isSeries;
  }

  function showSeriesTopProgress(): boolean {
    return !disableTopStatusBar && isSeries && epCount > 0;
  }

  function showScrimProgress(): boolean {
    return disableTopStatusBar && isSeries && epCount > 0;
  }

  function showFillRate(): boolean {
    return isSeries && progress > 0;
  }

  function seriesProgressTitle(): string {
    return `${statusLabel()} · ${epFileCount}/${epCount} episodes`;
  }

  function seriesProgressLabel(): string {
    return `Series fill rate ${progress}%`;
  }

  function taskTmdbId(task: any): number | null {
    const direct = Number(task?.tmdb_id ?? task?.tmdbId);
    if (Number.isFinite(direct) && direct > 0) return direct;
    const match = String(task?.info_path || task?.infoPath || "").match(
      /\/(movie|tv)\/(\d+)/i,
    );
    if (match?.[2]) {
      const parsed = Number(match[2]);
      if (Number.isFinite(parsed) && parsed > 0) return parsed;
    }
    return null;
  }

  function isActiveTaskFor(task: any, tmdbId: number): boolean {
    const taskId = taskTmdbId(task);
    if (!taskId || taskId !== tmdbId) return false;
    const state = String(task?.state || "").toUpperCase();
    return (
      state !== "COMPLETED" &&
      state !== "FAILED" &&
      state !== "CANCELLED" &&
      state !== "SKIPPED"
    );
  }

  $effect(() => {
    const tmdbId = itemTmdbId();
    if (!tmdbId) return;

    isDownloading = Array.from(downloadStore.downloads.values()).some((task) =>
      isActiveTaskFor(task, tmdbId),
    );
    for (const tasks of downloadStore.batchItems.values()) {
      if (tasks.some((task) => isActiveTaskFor(task, tmdbId))) {
        isDownloading = true;
        break;
      }
    }

    // Store may still be loading: watch it briefly.
    if (!isDownloading) {
      const unsubscribe = downloadStore.subscribe((state) => {
        const inDownloads = Array.from(state.downloads.values()).some((task) =>
          isActiveTaskFor(task, tmdbId),
        );
        const inBatches = Array.from(state.batchItems.values()).some((tasks) =>
          tasks.some((task) => isActiveTaskFor(task, tmdbId)),
        );
        isDownloading = inDownloads || inBatches;
      });
      setTimeout(unsubscribe, 2000);
    }

    const offUpdated = onDownloadEvent("TASK_UPDATED", (event) => {
      const task = event.detail;
      const taskId = taskTmdbId(task);
      if (!taskId || taskId !== tmdbId) return;
      const state = String(task?.state || "").toUpperCase();
      isDownloading = state !== "COMPLETED";
    });

    const offCompleted = onDownloadEvent("TASK_COMPLETED", (event) => {
      const task = event.detail;
      const taskId = taskTmdbId(task);
      if (!taskId || taskId !== tmdbId) return;
      isDownloading = false;
      item.hasFile = true;
      item.monitored = true;
      fetchLibraryOverview().catch(() => {});
      downloadStore.fetchAll().catch(() => {});
    });

    return () => {
      offUpdated();
      offCompleted();
    };
  });
</script>

<!-- ─── LIST VIEW ─── -->
{#if viewMode === "list"}
  <a href={linkUrl} class="media-card list-view">
    {#if showStatusBar()}
      <div
        class="library-status-bar"
        style={`background:${statusColor()}`}
        title={statusLabel()}
      ></div>
    {:else if showSeriesTopProgress()}
      <div
        class="series-top-progress"
        title={seriesProgressTitle()}
        aria-label={seriesProgressLabel()}
      >
        <div class="series-top-progress-track">
          <div
            class="series-top-progress-fill"
            style={`width:${progress}%; background:${statusColor()}`}
          ></div>
        </div>
        <span
          class="series-top-progress-count"
          style={`color:${statusColor()}`}
          title={`${epFileCount} of ${epCount} episodes acquired`}
          >{epFileCount}/{epCount}</span
        >
      </div>
    {/if}
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
      <div class="card-title-row">
        <div class="card-title" title={item.title}>{item.title}</div>
      </div>
      <div class="card-meta-row">
        {#if item.year}<span class="meta-item">{item.year}</span>{/if}
        {#if isSeries}
          {#if item.statistics?.seasonCount}<span class="meta-item">·</span><span
              class="meta-item">{item.statistics.seasonCount}S</span
            >{/if}
          {#if item.statistics?.episodeCount}<span class="meta-item">·</span><span
              class="meta-item">{epFileCount}/{epCount}</span
            >{/if}
        {:else if item.runtime}<span class="meta-item">·</span><span
            class="meta-item">{item.runtime}m</span
          >{/if}
        <span class="meta-item">·</span>
        <span class="meta-item size-meta"
          >{formatDiskSize(
            isSeries ? item.statistics?.sizeOnDisk || 0 : item.sizeOnDisk || 0,
          )}</span
        >
      </div>
      <div class="card-indicators">
        {#if isSeries}
          {#if showFillRate()}
            <div class="progress-bar-mini">
              <div
                class="progress-fill"
                style={`width:${progress}%; background:${statusColor()}`}
              ></div>
            </div>
          {/if}
          {#if missing > 0}
            <span class="indicator-badge missing">{missing} missing</span>
          {/if}
          {#if item.monitored}
            <span class="indicator-badge monitored">Monitored</span>
          {/if}
        {:else if item.monitored}
          <span class="indicator-badge monitored">Monitored</span>
        {/if}
      </div>
    </div>
  </a>

  <!-- ─── GRID / POSTER CARD VIEW ─── -->
{:else}
  <a href={linkUrl} class="media-card poster-card">
    <div class="poster-wrap">
      {#if posterUrl}
        <img src={posterUrl} alt={item.title} class="poster" loading="lazy" />
      {:else}
        <div class="poster placeholder">
          <span class="material-icons">{isSeries ? "live_tv" : "movie"}</span>
        </div>
      {/if}
      {#if showStatusBar()}
        <div
          class="library-status-bar"
          style={`background:${statusColor()}`}
          title={statusLabel()}
        ></div>
      {:else if showSeriesTopProgress()}
        <div
          class="series-top-progress"
          title={seriesProgressTitle()}
          aria-label={seriesProgressLabel()}
        >
          <div class="series-top-progress-track">
            <div
              class="series-top-progress-fill"
              style={`width:${progress}%; background:${statusColor()}`}
            ></div>
          </div>
          <span
            class="series-top-progress-count"
            style={`color:${statusColor()}`}
            title={`${epFileCount} of ${epCount} episodes acquired`}
            >{epFileCount}/{epCount}</span
          >
        </div>
      {/if}

      <!-- Bottom gradient scrim — title lives here, never cropped -->
      <div class="card-scrim">
        {#if showScrimProgress()}
          <div class="scrim-progress">
            <div class="scrim-progress-bar">
              <div
                class="scrim-progress-fill"
                style={`width:${progress}%; background:${statusColor()}`}
              ></div>
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
  .media-card {
    text-decoration: none;
    color: inherit;
    display: block;
    border-radius: 10px;
    cursor: pointer;
    position: relative;
  }
  .media-card.poster-card {
    display: block;
  }
  .poster-wrap {
    position: relative;
    aspect-ratio: 2/3;
    border-radius: 10px;
    overflow: hidden;
    background: #ffffff0d;
    box-shadow: 0 4px 12px #0006, 0 1px 3px #0000004d;
    transition: transform .22s cubic-bezier(.16, 1, .3, 1), box-shadow .22s cubic-bezier(.16, 1, .3, 1);
    will-change: transform;
  }
  .media-card.poster-card:hover .poster-wrap {
    transform: translateY(-5px) scale(1.02);
    box-shadow: 0 16px 32px #0000008c, 0 4px 8px #00000059;
  }
  .media-card.poster-card:hover .poster {
    filter: brightness(1.08);
  }
  .library-status-bar {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 4px;
    z-index: 3;
    overflow: visible;
    box-shadow: 0 1px 6px #00000059;
  }
  .library-status-bar:before {
    content: "";
    position: absolute;
    inset: 0;
    background: inherit;
  }
  .library-status-bar:after {
    content: "";
    position: absolute;
    inset: -2px 0 0;
    background: inherit;
    opacity: .65;
    filter: blur(8px);
    z-index: -1;
  }
  .media-card:hover .library-status-bar:after {
    opacity: .85;
  }
  .media-card:hover .library-status-bar:before {
    filter: brightness(1.08);
  }
  .library-status-bar[style*="#3b82f6"] {
    background:
      linear-gradient(
        135deg,
        #3b82f6f2,
        #60a5fae6,
        #93c5fdd9) !important;
  }
  .library-status-bar[style*="#f59e0b"] {
    background:
      linear-gradient(
        135deg,
        #f59e0bf2,
        #fbbf24e6,
        #fde047d9) !important;
  }
  .library-status-bar[style*="#ef4444"] {
    background:
      linear-gradient(
        135deg,
        #ef4444f2,
        #f87171e6,
        #fca5a5d9) !important;
  }
  .library-status-bar[style*="#22c55e"] {
    background:
      linear-gradient(
        135deg,
        #22c55ef2,
        #4ade80e6,
        #86efacd9) !important;
  }
  .library-status-bar[style*="#6b7280"] {
    background:
      linear-gradient(
        135deg,
        #6b7280f2,
        #9ca3afe6,
        #d1d5dbd9) !important;
  }
  .poster {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    transition: filter .3s ease;
  }
  .poster.placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    background: #ffffff0a;
    width: 100%;
    height: 100%;
  }
  .card-scrim {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    background:
      linear-gradient(
        to top,
        rgba(5, 5, 10, .97) 0%,
        rgba(5, 5, 10, .75) 45%,
        transparent 100%);
    padding: .9rem .55rem .5rem;
    display: flex;
    flex-direction: column;
    gap: 3px;
    z-index: 1;
    transition: background .2s;
  }
  .media-card.poster-card:hover .card-scrim {
    background:
      linear-gradient(
        to top,
        rgba(5, 5, 10, 1) 0%,
        rgba(5, 5, 10, .82) 50%,
        transparent 100%);
  }
  .scrim-title {
    font-size: .72rem;
    font-weight: 700;
    color: #f8fafc;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.2;
    text-shadow: 0 1px 4px rgba(0, 0, 0, .8);
  }
  .scrim-meta {
    font-size: .6rem;
    color: #ffffff8c;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .series-top-progress {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 3;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 6px 0;
  }
  .series-top-progress-track,
  .scrim-progress-bar {
    flex: 1;
    height: 4px;
    background: #0f172ab8;
    border-radius: 999px;
    overflow: hidden;
    box-shadow: inset 0 0 0 1px #ffffff0d;
  }
  .series-top-progress-fill,
  .scrim-progress-fill {
    height: 100%;
    border-radius: inherit;
    transition: width .4s ease;
  }
  .series-top-progress-count {
    font-size: .58rem;
    font-weight: 800;
    color: #ffffffd9;
    font-family: var(--font-mono, monospace);
    white-space: nowrap;
    line-height: 1;
    text-shadow: 0 1px 3px rgba(0, 0, 0, .7);
    flex-shrink: 0;
  }
  .scrim-progress {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-bottom: 2px;
  }
  .scrim-ep-count {
    font-size: .5rem;
    font-weight: 700;
    color: #ffffffa6;
    font-family: var(--font-mono, monospace);
    white-space: nowrap;
    flex-shrink: 0;
  }
  .media-card.list-view {
    display: grid;
    grid-template-columns: 56px minmax(0, 1fr);
    gap: .65rem;
    align-items: center;
    padding: .55rem;
    width: 100%;
    max-width: 100%;
    min-width: 0;
    box-sizing: border-box;
    background: #ffffff05;
    border: 1px solid rgba(255, 255, 255, .05);
    border-radius: 8px;
    overflow: hidden;
    transition:
      transform .18s ease,
      background .18s ease,
      border-color .18s ease;
  }
  .media-card.list-view:hover {
    transform: translate(4px);
    background: #ffffff0a;
    border-color: #ffffff1a;
  }
  .list-poster-wrap {
    width: 56px;
    height: 84px;
    border-radius: 6px;
    overflow: hidden;
    flex-shrink: 0;
    background: #ffffff0d;
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
    gap: .2rem;
    width: 100%;
    overflow: hidden;
  }
  .card-title-row {
    display: flex;
    align-items: center;
    gap: .4rem;
    min-width: 0;
  }
  .card-title {
    font-size: .78rem;
    font-weight: 700;
    color: #f1f5f9;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.25;
    flex: 1;
    min-width: 0;
  }
  .card-meta-row {
    display: flex;
    align-items: center;
    gap: .25rem;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
  }
  .meta-item {
    font-size: .62rem;
    color: var(--text-muted);
    opacity: .78;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .size-meta {
    font-family: var(--font-mono);
    color: var(--color-primary);
    opacity: .85;
  }
  .card-indicators {
    display: flex;
    align-items: center;
    gap: .35rem;
    min-width: 0;
    flex-wrap: wrap;
  }
  .indicator-badge {
    font-size: .52rem;
    font-weight: 700;
    line-height: 1;
    padding: 3px 6px;
    border-radius: 999px;
    border: 1px solid transparent;
    white-space: nowrap;
  }
  .indicator-badge.missing {
    color: #fbbf24;
    background: #fbbf241f;
    border-color: #fbbf2459;
  }
  .indicator-badge.monitored {
    color: #93c5fd;
    background: #3b82f61f;
    border-color: #3b82f659;
  }
  .progress-bar-mini {
    width: 72px;
    max-width: 100%;
    height: 3px;
    border-radius: 999px;
    background: #ffffff24;
    overflow: hidden;
    flex-shrink: 0;
  }
  .progress-fill {
    height: 100%;
    border-radius: 999px;
  }
  @media (max-width: 420px) {
    .media-card.list-view {
      grid-template-columns: 48px minmax(0, 1fr);
      gap: .55rem;
      padding: .45rem;
    }
    .list-poster-wrap {
      width: 48px;
      height: 72px;
    }
    .card-title {
      font-size: .72rem;
    }
    .indicator-badge,
    .meta-item {
      font-size: .5rem;
    }
    .progress-bar-mini {
      width: 56px;
    }
    .series-top-progress {
      gap: 4px;
      padding: 4px 5px 0;
    }
    .series-top-progress-count {
      font-size: .52rem;
    }
  }
</style>
