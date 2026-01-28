<script lang="ts">
  interface Props {
    title: string;
    tagline?: string | null;
    backdropPath?: string | null;
    posterPath?: string | null;
    year?: string;
    rating?: number;
    runtime?: string;
    genres?: string[];
    loading?: boolean;
  }

  let {
    title,
    tagline = null,
    backdropPath = null,
    posterPath = null,
    year = "",
    rating = 0,
    runtime = "",
    genres = [],
    loading = false,
  }: Props = $props();

  let posterLoaded = $state(false);
  let backdropLoaded = $state(false);

  function getBackdropUrl(
    path: string | null,
    size: string = "original",
  ): string {
    if (!path) return "/images/placeholder-banner.png";
    return `https://image.tmdb.org/t/p/${size}${path}`;
  }

  function getPosterUrl(path: string | null, size: string = "w500"): string {
    if (!path) return "/images/placeholder-poster.svg";
    return `https://image.tmdb.org/t/p/${size}${path}`;
  }
</script>

<div
  class="detail-hero"
  class:skeleton-bg={loading}
  style="background-image: url(/images/placeholder-banner.png)"
>
  {#if backdropPath}
    <div
      class="backdrop-real"
      class:loaded={backdropLoaded}
      style={`background-image: url(${getBackdropUrl(backdropPath)})`}
    ></div>
    <img
      src={getBackdropUrl(backdropPath)}
      alt=""
      class="hidden-loader"
      onload={() => (backdropLoaded = true)}
    />
  {/if}
  <div class="hero-overlay"></div>
  <div class="hero-content">
    <div class="poster-container glass-panel" class:skeleton={loading}>
      <img
        src={posterPath
          ? getPosterUrl(posterPath)
          : "/images/placeholder-poster.svg"}
        alt={title}
        class="detail-poster"
        class:loaded={posterLoaded || !posterPath}
        onload={() => (posterLoaded = true)}
      />
    </div>
    <div class="hero-info">
      {#if loading}
        <div class="skeleton skeleton-tagline"></div>
        <div class="skeleton skeleton-title"></div>
        <div class="skeleton skeleton-meta"></div>
      {:else}
        {#if tagline}
          <div class="detail-tagline">{tagline}</div>
        {/if}
        <h1 class="detail-title">{title}</h1>
        <div class="detail-meta">
          {#if year}
            <span class="meta-year">{year}</span>
            <span class="meta-divider">•</span>
          {/if}
          {#if rating > 0}
            <span class="meta-rating">
              <span class="material-icons">star</span>
              {rating.toFixed(1)}
            </span>
            <span class="meta-divider">•</span>
          {/if}
          {#if runtime}
            <span class="meta-runtime">{runtime}</span>
            <span class="meta-divider">•</span>
          {/if}
          {#if genres.length > 0}
            <span class="meta-genres">{genres.slice(0, 3).join(" / ")}</span>
          {/if}
        </div>
      {/if}
    </div>
  </div>
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

  .hero-overlay {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      to bottom,
      rgba(15, 23, 42, 0.2) 0%,
      rgba(15, 23, 42, 1) 100%
    );
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
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.8);
    border: 1px solid rgba(255, 255, 255, 0.1);
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
    transition: opacity 0.15s ease-in-out;
  }

  .detail-poster.loaded {
    opacity: 1;
  }

  .backdrop-real {
    position: absolute;
    inset: 0;
    background-size: cover;
    background-position: center;
    opacity: 0;
    transition: opacity 0.15s ease-in-out;
  }

  .backdrop-real.loaded {
    opacity: 1;
  }

  .hidden-loader {
    display: none;
  }

  .hero-info {
    flex: 1;
    padding-bottom: 1rem;
  }

  .detail-tagline {
    font-size: 0.85rem;
    font-weight: 800;
    color: var(--color-primary);
    text-transform: uppercase;
    letter-spacing: 0.25rem;
    margin-bottom: 0.5rem;
    font-family: var(--font-mono);
  }

  .detail-title {
    font-size: 3.5rem;
    font-weight: 800;
    margin: 0 0 1rem;
    line-height: 1.1;
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.5);
  }

  .detail-meta {
    display: flex;
    align-items: center;
    gap: 1rem;
    font-size: 0.9rem;
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
    color: #ffd700;
  }
  .meta-rating .material-icons {
    font-size: 16px;
  }

  .skeleton-bg {
    position: relative;
    overflow: hidden;
  }

  .skeleton-bg::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(255, 255, 255, 0.05),
      transparent
    );
    animation: shimmer 2s infinite linear;
  }

  .skeleton {
    position: relative;
    overflow: hidden;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.05);
  }

  .skeleton::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(255, 255, 255, 0.05),
      transparent
    );
    animation: shimmer 2s infinite linear;
  }

  @keyframes shimmer {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(100%);
    }
  }

  .skeleton-tagline {
    width: 200px;
    height: 1rem;
    margin-bottom: 0.5rem;
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
</style>
