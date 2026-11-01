<script lang="ts">
  import { goto } from "$app/navigation";
  import { Badge } from "@media-set/core-ui";
  import { downloadStore, onDownloadEvent } from "$lib/stores/downloads.svelte";
  import { fetchLibraryOverview } from "$lib/stores/arr";

  type StatusColor = "blue" | "yellow" | "red" | "green" | "purple" | "gray";

  interface Props {
    id: number;
    title: string;
    posterPath?: string | null;
    backdropPath?: string | null;
    mode?: "poster" | "banner";
    voteAverage?: number;
    releaseDate?: string;
    overview?: string;
    mediaType?: "movie" | "tv";
    badge?: { text: string; variant?: string; color?: string };
    /** Library status colour; drawn as a thin bar across the top of the card. */
    statusType?: StatusColor | string | null;
    /** Library status text shown above the title. */
    statusLabel?: string | null;
    onClick?: () => void;
    hideDetailsButton?: boolean;
  }

  let {
    id,
    title,
    posterPath = null,
    backdropPath = null,
    mode = "poster",
    voteAverage = 0,
    releaseDate = "",
    overview = "",
    mediaType = "movie",
    badge,
    statusType,
    statusLabel,
    onClick,
    hideDetailsButton = true,
  }: Props = $props();

  let loaded = $state(false);

  function getImageUrl(path: string | null): string {
    if (!path) {
      return mode === "poster"
        ? "/images/placeholder-poster.svg"
        : "/images/placeholder-banner.png";
    }
    const size = mode === "poster" ? "w342" : "w780";
    return `https://image.tmdb.org/t/p/${size}${path}`;
  }

  function getYear(date: string): string {
    return date?.substring(0, 4) || "N/A";
  }

  function handleClick(e?: MouseEvent | KeyboardEvent) {
    if (onClick) {
      onClick();
    } else {
      // Use goto for smooth SPA navigation
      goto(`/${mediaType}/${id}`);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      handleClick();
    }
  }

  function statusColor(): string {
    switch (statusType) {
      case "blue":
        return "#3b82f6";
      case "yellow":
        return "#f59e0b";
      case "red":
        return "#ef4444";
      case "green":
        return "#22c55e";
      case "purple":
        return "#a855f7";
      case "gray":
        return "#6b7280";
      default:
        return "transparent";
    }
  }

  function cardTmdbId(): number | null {
    const value = Number(id);
    return Number.isFinite(value) && value > 0 ? value : null;
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

  // Live status: follow download events for this title.
  $effect(() => {
    const tmdbId = cardTmdbId();
    if (!tmdbId) return;

    const offUpdated = onDownloadEvent("TASK_UPDATED", (event) => {
      const task = event.detail;
      const taskId = taskTmdbId(task);
      if (!taskId || taskId !== tmdbId) return;
      const state = String(task?.state || "").toUpperCase();
      statusType = state === "COMPLETED" ? "green" : "purple";
      statusLabel =
        state === "COMPLETED" ? "DOWNLOADED · MONITORED" : "DOWNLOADING";
    });

    const offCompleted = onDownloadEvent("TASK_COMPLETED", (event) => {
      const task = event.detail;
      const taskId = taskTmdbId(task);
      if (!taskId || taskId !== tmdbId) return;
      statusType = "green";
      statusLabel = "DOWNLOADED · MONITORED";
      fetchLibraryOverview().catch(() => {});
      downloadStore.fetchAll().catch(() => {});
    });

    return () => {
      offUpdated();
      offCompleted();
    };
  });
</script>

<div
  class="media-card-v3 mode-{mode}"
  role="button"
  tabindex="0"
  onclick={handleClick}
  onkeydown={handleKeydown}
>
  <div class="card-inner">
    <div class="card-image-wrapper">
      {#if statusType}
        <div
          class="library-status-bar"
          style={`background:${statusColor()}`}
          title={statusLabel || statusType}
        ></div>
      {/if}
      <img
        src={getImageUrl(mode === "poster" ? posterPath : backdropPath)}
        alt={title}
        loading="lazy"
        class:loaded
        onload={() => (loaded = true)}
      />
      <div class="card-shine"></div>
    </div>

    {#if badge}
      <div class="badge-container">
        <Badge
          text={badge.text}
          variant={badge.variant as any}
          color={badge.color}
        />
      </div>
    {/if}

    <div class="card-overlay">
      <div class="overlay-main">
        {#if statusLabel}
          <span class="card-status">{statusLabel}</span>
        {/if}
        <div class="card-title-row">
          <h3 class="card-title">{title}</h3>
          <span class="card-year">{getYear(releaseDate)}</span>
        </div>

        <div class="card-stats">
          <div class="stat-item rating">
            <span class="material-icons">star</span>
            <span>{voteAverage?.toFixed(1) || "0.0"}</span>
          </div>
          {#if mediaType}
            <Badge text={mediaType.toUpperCase()} variant="source" size="xs" noDot />
          {/if}
        </div>
      </div>

      {#if overview}
        <div class="overlay-expand">
          <p class="expand-overview {mode === 'banner' ? 'clamp-2' : ''}">
            {overview}
          </p>
          {#if !hideDetailsButton}
            <div class="expand-actions">
              <div class="btn-more">
                DETAILS
                <span class="material-icons">arrow_forward</span>
              </div>
            </div>
          {/if}
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .media-card-v3 {
    position: relative;
    width: 100%;
    cursor: pointer;
    background: transparent;
    transition: transform .4s cubic-bezier(.175, .885, .32, 1.275);
    z-index: 1;
    background-image: url(/images/placeholder-poster.svg);
    background-size: cover;
    background-position: center;
    border-radius: 16px;
  }
  .media-card-v3.mode-banner {
    background-image: url(/images/placeholder-banner.png);
  }
  .media-card-v3.mode-poster {
    aspect-ratio: 2/3;
  }
  .media-card-v3.mode-banner {
    aspect-ratio: 16/9;
  }
  .card-inner {
    position: relative;
    width: 100%;
    height: 100%;
    border-radius: 16px;
    overflow: visible;
    border: 1px solid rgba(255, 255, 255, .05);
    box-shadow: 0 10px 30px -5px #0000004d;
    transition: all .4s ease;
    isolation: isolate;
  }
  .media-card-v3:hover {
    transform: scale(1.05) translateY(-5px);
    z-index: 50;
  }
  .media-card-v3:hover .card-inner {
    border-color: #00f3ff66;
    box-shadow: 0 20px 50px -10px #00f3ff33, 0 0 20px #00f3ff1a;
  }
  .card-image-wrapper {
    position: absolute;
    inset: 0;
    z-index: 1;
    overflow: hidden;
    border-radius: 16px;
  }
  .library-status-bar {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 4px;
    z-index: 3;
    box-shadow: 0 1px 6px #00000059;
  }
  .card-image-wrapper img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform .6s ease, opacity .15s ease-in-out;
    opacity: 0;
  }
  .card-image-wrapper img.loaded {
    opacity: 1;
  }
  .media-card-v3:hover img {
    transform: scale(1.1);
  }
  .card-shine {
    position: absolute;
    top: 0;
    left: -100%;
    width: 50%;
    height: 100%;
    background:
      linear-gradient(
        to right,
        transparent,
        rgba(255, 255, 255, .1),
        transparent);
    transform: skew(-25deg);
    transition: .75s;
    z-index: 1;
  }
  .media-card-v3:hover .card-shine {
    left: 150%;
  }
  .badge-container {
    position: absolute;
    top: 1rem;
    left: 1rem;
    z-index: 5;
  }
  .card-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    padding: 1.25rem;
    background:
      linear-gradient(
        to top,
        rgba(0, 0, 0, .95) 0%,
        rgba(0, 0, 0, .6) 30%,
        rgba(0, 0, 0, .2) 60%,
        transparent 100%);
    border-radius: 16px;
    z-index: 2;
    transition: all .4s cubic-bezier(.4, 0, .2, 1);
  }
  .card-overlay:before {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: 16px;
    background-image:
      radial-gradient(
        circle,
        rgba(255, 255, 255, .06) 1px,
        transparent 1px);
    background-size: 6px 6px;
    mask-image:
      linear-gradient(
        to top,
        black 0%,
        rgba(0, 0, 0, .4) 40%,
        transparent 70%);
    -webkit-mask-image:
      linear-gradient(
        to top,
        black 0%,
        rgba(0, 0, 0, .4) 40%,
        transparent 70%);
    pointer-events: none;
    z-index: 0;
    transition: opacity .3s;
  }
  .media-card-v3:hover .card-overlay {
    background:
      linear-gradient(
        to top,
        #000000fa,
        #000c,
        #0006);
  }
  .media-card-v3:hover .card-overlay:before {
    background-image:
      radial-gradient(
        circle,
        rgba(0, 243, 255, .12) 1px,
        transparent 1px);
    opacity: 1;
  }
  .card-status {
    display: inline-flex;
    width: fit-content;
    margin-bottom: .55rem;
    padding: .3rem .5rem;
    border: 1px solid rgba(255, 255, 255, .28);
    border-radius: 999px;
    background: #040810e0;
    color: #fff;
    font-size: .7rem;
    font-weight: 800;
    letter-spacing: .04em;
  }
  .overlay-main {
    transform: translateY(0);
    transition: transform .4s ease;
  }
  .media-card-v3:hover .overlay-main {
    transform: translateY(0);
  }
  .card-title-row {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: .75rem;
    margin-bottom: .5rem;
  }
  .card-title {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 800;
    color: #fff;
    line-height: 1.2;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    text-shadow: 0 2px 4px rgba(0, 0, 0, .5);
  }
  .card-year {
    font-size: .8rem;
    font-weight: 700;
    color: var(--color-primary, #00f3ff);
    font-family: var(--font-mono, monospace);
    opacity: .9;
    flex-shrink: 0;
  }
  .card-stats {
    display: flex;
    align-items: center;
    gap: .75rem;
  }
  .stat-item {
    display: flex;
    align-items: center;
    gap: .25rem;
    font-size: .75rem;
    font-weight: 600;
    color: #ffffffb3;
  }
  .stat-item.rating {
    color: gold;
  }
  .stat-item.rating .material-icons {
    font-size: 14px;
  }
  .overlay-expand {
    max-height: 0;
    overflow: hidden;
    opacity: 0;
    transition: all .4s cubic-bezier(.4, 0, .2, 1);
    display: flex;
    flex-direction: column;
  }
  .media-card-v3:hover .overlay-expand {
    max-height: 180px;
    opacity: 1;
    margin-top: .75rem;
  }
  .expand-overview {
    margin: 0;
    font-size: .8rem;
    line-height: 1.5;
    color: #ffffffb3;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .expand-overview.clamp-2 {
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }
  .expand-actions {
    margin-top: .75rem;
    display: flex;
    justify-content: flex-end;
  }
  .btn-more {
    background: #00f3ff0d;
    border: 1px solid rgba(0, 243, 255, .3);
    color: var(--color-primary, #00f3ff);
    padding: .3rem .6rem;
    font-size: .65rem;
    font-weight: 900;
    letter-spacing: .15em;
    display: flex;
    align-items: center;
    gap: .4rem;
    transition: all .3s;
    font-family: var(--font-mono, monospace);
    clip-path: polygon(4px 0%, 100% 0%, 100% calc(100% - 4px), calc(100% - 4px) 100%, 0% 100%, 0% 4px);
  }
  .media-card-v3:hover .btn-more {
    border-color: #00f3ff99;
    background: #00f3ff1a;
  }
  .btn-more:hover {
    background: var(--color-primary, #00f3ff);
    color: #000;
    border-color: var(--color-primary, #00f3ff);
    box-shadow: 0 0 15px #00f3ff66;
  }
  .btn-more .material-icons {
    font-size: 14px;
  }
  @media (hover: none), (pointer: coarse) {
    .media-card-v3:hover {
      transform: none;
    }
    .card-overlay {
      background:
        linear-gradient(
          to top,
          rgba(0, 0, 0, 1) 0%,
          rgba(0, 0, 0, .84) 48%,
          rgba(0, 0, 0, .18) 78%,
          transparent 100%);
    }
    .card-title {
      font-size: 1.1rem;
      text-shadow: 0 2px 6px #000;
    }
    .card-year,
    .stat-item {
      opacity: 1;
    }
  }
  @media (max-width: 768px) {
    .media-card-v3.mode-poster {
      aspect-ratio: 16/10;
      background-image: url(/images/placeholder-banner.png);
    }
    .card-overlay {
      padding: 1rem;
      background:
        linear-gradient(
          to top,
          #03060d 0%,
          rgba(3, 6, 13, .94) 45%,
          rgba(3, 6, 13, .3) 76%,
          transparent 100%);
    }
    .card-title {
      font-size: 1.2rem;
    }
    .card-status {
      color: #fff;
      background: #080d18;
      border-color: #ffffff73;
    }
    .stat-item {
      color: #ffffffe0;
    }
  }
</style>
