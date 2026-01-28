<script lang="ts">
  import { goto } from "$app/navigation";
  import Badge from "../ui/Badge.svelte";

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

  function handleClick(e: MouseEvent | KeyboardEvent) {
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
      handleClick(e);
    }
  }
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
            <div class="stat-item type">
              {mediaType.toUpperCase()}
            </div>
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
    transition: transform 0.4s cubic-bezier(0.175, 0.885, 0.32, 1.275);
    z-index: 1;
    /* Ensure child elements (like overlay) can overflow slightly without being cut by PARENT containers */
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
    border: 1px solid rgba(255, 255, 255, 0.05);
    box-shadow: 0 10px 30px -5px rgba(0, 0, 0, 0.3);
    transition: all 0.4s ease;
    /* Create stacking context */
    isolation: isolate;
  }

  .media-card-v3:hover {
    transform: scale(1.05) translateY(-5px);
    z-index: 50; /* Higher z-index on hover to prevent overlap with siblings */
  }

  .media-card-v3:hover .card-inner {
    border-color: rgba(0, 243, 255, 0.4);
    box-shadow:
      0 20px 50px -10px rgba(0, 243, 255, 0.2),
      0 0 20px rgba(0, 243, 255, 0.1);
  }

  .card-image-wrapper {
    position: absolute;
    inset: 0;
    z-index: 1;
    overflow: hidden;
    border-radius: 16px;
  }

  .card-image-wrapper img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition:
      transform 0.6s ease,
      opacity 0.15s ease-in-out;
    opacity: 0;
  }

  .card-image-wrapper img.loaded {
    opacity: 1;
  }

  .media-card-v3:hover img {
    transform: scale(1.1);
  }

  /* Shine Effect */
  .card-shine {
    position: absolute;
    top: 0;
    left: -100%;
    width: 50%;
    height: 100%;
    background: linear-gradient(
      to right,
      transparent,
      rgba(255, 255, 255, 0.1),
      transparent
    );
    transform: skewX(-25deg);
    transition: 0.75s;
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

  /* Overlay System */
  .card-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    padding: 1.25rem;
    background: linear-gradient(
      to top,
      rgba(0, 0, 0, 0.95) 0%,
      rgba(0, 0, 0, 0.6) 30%,
      rgba(0, 0, 0, 0.2) 60%,
      transparent 100%
    );
    border-radius: 16px;
    z-index: 2;
    transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .media-card-v3:hover .card-overlay {
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

  /* Prevent content drift on hover */
  .media-card-v3:hover .overlay-main {
    transform: translateY(0);
  }

  .card-title-row {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 0.75rem;
    margin-bottom: 0.5rem;
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
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
  }

  .card-year {
    font-size: 0.8rem;
    font-weight: 700;
    color: var(--color-primary, #00f3ff);
    font-family: var(--font-mono, monospace);
    opacity: 0.9;
    flex-shrink: 0;
  }

  .card-stats {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .stat-item {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.75rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.7);
  }

  .stat-item.rating {
    color: #ffd700;
  }

  .stat-item.rating .material-icons {
    font-size: 14px;
  }

  .stat-item.type {
    background: rgba(255, 255, 255, 0.1);
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 0.65rem;
    letter-spacing: 0.05em;
  }

  /* Expandable Area (Inline) */
  .overlay-expand {
    max-height: 0;
    overflow: hidden;
    opacity: 0;
    transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
    display: flex;
    flex-direction: column;
  }

  .media-card-v3:hover .overlay-expand {
    max-height: 180px; /* Careful limit to prevent cropping on short cards */
    opacity: 1;
    margin-top: 0.75rem;
  }

  .expand-overview {
    margin: 0;
    font-size: 0.8rem;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.7);
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
    margin-top: 0.75rem;
    display: flex;
    justify-content: flex-end;
  }

  .btn-more {
    background: rgba(0, 243, 255, 0.05);
    border: 1px solid rgba(0, 243, 255, 0.3);
    color: var(--color-primary, #00f3ff);
    padding: 0.3rem 0.6rem;
    font-size: 0.65rem;
    font-weight: 900;
    letter-spacing: 0.15em;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    transition: all 0.3s;
    font-family: var(--font-mono, monospace);
    clip-path: polygon(
      4px 0%,
      100% 0%,
      100% calc(100% - 4px),
      calc(100% - 4px) 100%,
      0% 100%,
      0% 4px
    );
  }

  .media-card-v3:hover .btn-more {
    border-color: rgba(0, 243, 255, 0.6);
    background: rgba(0, 243, 255, 0.1);
  }

  .btn-more:hover {
    background: var(--color-primary, #00f3ff);
    color: #000;
    border-color: var(--color-primary, #00f3ff);
    box-shadow: 0 0 15px rgba(0, 243, 255, 0.4);
  }

  .btn-more .material-icons {
    font-size: 14px;
  }
</style>
