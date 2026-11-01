<script lang="ts">
  import {
    getBackdropUrl,
    getPosterUrl,
    getYear,
    type TMDBTVShowDetails,
  } from "$lib/services/tmdb";
  import { formatDiskSize } from "$lib/stores/arr";

  export interface LibraryProgress {
    pct: number;
    acquired: number;
    total: number;
    sizeOnDisk: number;
  }

  interface Props {
    tv: TMDBTVShowDetails | null;
    loading: boolean;
    posterLoaded?: boolean;
    backdropLoaded?: boolean;
    libraryProgress?: LibraryProgress | null;
  }

  let {
    tv,
    loading,
    posterLoaded = $bindable(),
    backdropLoaded = $bindable(),
    libraryProgress,
  }: Props = $props();
</script>

<div
  class="detail-hero"
  class:skeleton-bg={loading}
  style="background-image: url(/images/placeholder-banner.png)"
>
  {#if tv?.backdrop_path}
    <div
      class="backdrop-real"
      class:loaded={backdropLoaded}
      style={`background-image: url(${getBackdropUrl(tv.backdrop_path, "original")})`}
    ></div>
    <img
      src={getBackdropUrl(tv.backdrop_path, "original")}
      alt=""
      class="hidden-loader"
      onload={() => (backdropLoaded = true)}
    />
  {/if}
  <div class="hero-overlay"></div>

  <div class="hero-content">
    <div class="poster-container glass-panel" class:skeleton={loading}>
      <img
        src={tv?.poster_path
          ? getPosterUrl(tv.poster_path, "w500")
          : "/images/placeholder-poster.svg"}
        alt={tv?.name || "Unknown Subject"}
        class="detail-poster"
        class:loaded={posterLoaded || !tv?.poster_path}
        onload={() => (posterLoaded = true)}
      />
    </div>

    <div class="hero-info">
      {#if loading}
        <div class="skeleton skeleton-tagline"></div>
        <div class="skeleton skeleton-title"></div>
        <div class="skeleton skeleton-meta"></div>
      {:else if tv}
        {#if tv.tagline}
          <div class="detail-tagline">{tv.tagline}</div>
        {/if}
        <h1 class="detail-title">{tv.name}</h1>
        <div class="detail-meta">
          <span class="meta-year">{getYear(tv.first_air_date)}</span>
          <span class="meta-divider">•</span>
          <span class="meta-rating">
            <span class="material-icons">star</span>
            {tv.vote_average.toFixed(1)}
          </span>
          <span class="meta-divider">•</span>
          <span class="meta-runtime"
            >{tv.number_of_seasons}
            {tv.number_of_seasons === 1 ? "Season" : "Seasons"}</span
          >
          <span class="meta-divider">•</span>
          <span class="meta-genres"
            >{tv.genres
              .slice(0, 3)
              .map((g) => g.name)
              .join(" / ")}</span
          >
        </div>
      {/if}
    </div>
  </div>

  {#if libraryProgress}
    <div class="library-progress-strip">
      <div class="lp-bar">
        <div class="lp-fill" style="width: {libraryProgress.pct}%"></div>
      </div>
      <div class="lp-stats">
        <span class="lp-label">IN LIBRARY</span>
        <span class="lp-value"
          >{libraryProgress.acquired}/{libraryProgress.total} episodes</span
        >
        <span class="lp-size">{formatDiskSize(libraryProgress.sizeOnDisk)}</span>
      </div>
    </div>
  {/if}
</div>

<style>
  .detail-hero {
    height: 450px;
    background-size: cover;
    background-position: center;
    position: relative;
    display: flex;
    align-items: flex-end;
    padding: 0 2rem 3rem;
  }
  .skeleton-bg {
    position: relative;
    overflow: hidden;
  }
  .skeleton-bg:after {
    content: "";
    position: absolute;
    inset: 0;
    background:
      linear-gradient(
        90deg,
        transparent,
        rgba(255, 255, 255, .05),
        transparent);
    animation: shimmer 2s infinite linear;
  }
  .skeleton {
    position: relative;
    overflow: hidden;
    border-radius: 4px;
  }
  .skeleton:after {
    content: "";
    position: absolute;
    inset: 0;
    background:
      linear-gradient(
        90deg,
        transparent,
        rgba(255, 255, 255, .05),
        transparent);
    animation: shimmer 2s infinite linear;
  }
  @keyframes shimmer {
    0% {
      transform: translate(-100%);
    }
    to {
      transform: translate(100%);
    }
  }
  .skeleton-tagline {
    width: 200px;
    height: 1rem;
    margin-bottom: .5rem;
  }
  .skeleton-title {
    width: 400px;
    height: 3.5rem;
    margin-bottom: 1rem;
  }
  .skeleton-meta {
    width: 300px;
    height: 1.2rem;
  }
  .backdrop-real {
    position: absolute;
    inset: 0;
    background-size: cover;
    background-position: center;
    opacity: 0;
    transition: opacity .15s ease-in-out;
  }
  .backdrop-real.loaded {
    opacity: 1;
  }
  .hidden-loader {
    display: none;
  }
  .hero-overlay {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(
        to bottom,
        #0f172a33,
        #0f172a);
  }
  .hero-content {
    position: relative;
    z-index: 1;
    display: flex;
    gap: 2.5rem;
    align-items: flex-end;
    width: 100%;
    max-width: 1400px;
    margin: 0 auto;
  }
  .poster-container {
    width: 220px;
    aspect-ratio: 2/3;
    border-radius: 12px;
    overflow: hidden;
    box-shadow: 0 20px 50px #000c;
    border: 1px solid rgba(255, 255, 255, .1);
    flex-shrink: 0;
    background-image: url(/images/placeholder-poster.svg);
    background-size: cover;
    background-position: center;
  }
  .detail-poster {
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity .15s ease-in-out;
  }
  .detail-poster.loaded {
    opacity: 1;
  }
  .hero-info {
    flex: 1;
    padding-bottom: 1rem;
  }
  .detail-tagline {
    font-size: .85rem;
    font-weight: 800;
    color: var(--color-primary);
    text-transform: uppercase;
    letter-spacing: .25rem;
    margin-bottom: .5rem;
    font-family: var(--font-mono);
  }
  .detail-title {
    font-size: 3.5rem;
    font-weight: 800;
    margin: 0 0 1rem;
    line-height: 1.1;
    text-shadow: 0 2px 10px rgba(0, 0, 0, .5);
  }
  .detail-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: .4rem 1rem;
    font-size: .9rem;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }
  .meta-year {
    color: var(--text-primary);
  }
  .meta-rating {
    display: flex;
    align-items: center;
    gap: 4px;
    color: gold;
  }
  .meta-rating .material-icons {
    font-size: 16px;
  }
  .library-progress-strip {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    padding: .75rem 2rem;
    background: #0009;
    -webkit-backdrop-filter: blur(8px);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    gap: 1.5rem;
    z-index: 2;
  }
  .lp-bar {
    flex: 1;
    height: 4px;
    background: #ffffff1a;
    border-radius: 2px;
    overflow: hidden;
  }
  .lp-fill {
    height: 100%;
    background:
      linear-gradient(
        90deg,
        var(--color-primary),
        #34d399);
    border-radius: 2px;
    transition: width .6s ease;
  }
  .lp-stats {
    display: flex;
    align-items: center;
    gap: 1rem;
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: .7rem;
  }
  .lp-label {
    color: var(--color-primary);
    font-weight: 800;
    letter-spacing: .1em;
  }
  .lp-value {
    color: var(--text-secondary);
  }
  .lp-size {
    color: var(--text-muted);
  }
  @media (max-width: 1024px) {
    .hero-content {
      gap: 1.5rem;
    }
    .poster-container {
      width: 160px;
    }
    .detail-title {
      font-size: 2.5rem;
    }
  }
  @media (max-width: 768px) {
    .detail-hero {
      height: auto;
      min-height: 320px;
      padding: 5rem 1rem 2rem;
    }
    .poster-container {
      display: none;
    }
    .detail-title {
      font-size: clamp(1.75rem, 9vw, 2.25rem);
      overflow-wrap: anywhere;
    }
    .detail-meta {
      gap: .35rem .65rem;
    }
    .meta-divider {
      display: none;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .skeleton:after,
    .detail-poster,
    .backdrop-real,
    .lp-fill {
      animation: none;
      transition: none;
    }
  }
</style>
