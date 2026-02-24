<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import Badge from "$lib/components/ui/Badge.svelte";

  interface TrendingItem {
    id: number;
    title: string;
    backdrop_path: string | null;
    poster_path: string | null;
    vote_average: number;
    release_date: string;
    overview: string;
    media_type: "movie" | "tv";
  }

  let items = $state<TrendingItem[]>([]);
  let currentIndex = $state(0);
  let loading = $state(true);
  let timer: ReturnType<typeof setInterval>;

  onMount(async () => {
    try {
      const res = await fetch(
        "/api/discovery/popular-today?type=movie&limit=5",
      );
      if (res.ok) {
        const data = await res.json();
        items = (data.results || []).slice(0, 5).map((item: any) => {
          // Extract backdrop_path from backdrop_url if needed
          let backdropPath = item.backdrop_path;
          if (!backdropPath && item.backdrop_url) {
            const match = item.backdrop_url.match(/\/t\/p\/\w+(\/.+)$/);
            backdropPath = match ? match[1] : null;
          }
          let posterPath = item.poster_path;
          if (!posterPath && item.poster_url) {
            const match = item.poster_url.match(/\/t\/p\/\w+(\/.+)$/);
            posterPath = match ? match[1] : null;
          }

          return {
            id: item.id,
            title: item.title || item.name,
            backdrop_path: backdropPath,
            poster_path: posterPath,
            vote_average: item.vote_average || 0,
            release_date: item.release_date || item.first_air_date || "",
            overview: item.overview || item.description || "",
            media_type: item.media_type === "tv" ? "tv" : "movie",
          };
        });
      }
    } catch (e) {
      console.error("Failed to load hero items:", e);
    } finally {
      loading = false;
    }

    // Auto-rotate
    timer = setInterval(() => {
      if (items.length > 0) {
        currentIndex = (currentIndex + 1) % items.length;
      }
    }, 6000);
  });

  onDestroy(() => {
    if (timer) clearInterval(timer);
  });

  function getBackdropUrl(path: string | null): string {
    if (!path) return "/images/placeholder-banner.png";
    return `https://image.tmdb.org/t/p/w1280${path}`;
  }

  function getYear(date: string): string {
    return date?.substring(0, 4) || "";
  }

  function goTo(index: number) {
    currentIndex = index;
  }

  let current = $derived(items[currentIndex]);
</script>

<div class="hero-banner">
  {#if loading}
    <div class="hero-skeleton">
      <div class="shimmer"></div>
    </div>
  {:else if items.length > 0 && current}
    <div
      class="hero-backdrop"
      style="background-image: url({getBackdropUrl(current.backdrop_path)});"
    >
      <div class="hero-gradient"></div>

      <div class="hero-content">
        <div class="hero-info">
          <Badge text="TRENDING" variant="danger" size="sm" />
          <h1 class="hero-title">{current.title}</h1>
          <div class="hero-meta">
            {#if getYear(current.release_date)}
              <span class="year">{getYear(current.release_date)}</span>
            {/if}
            <span class="rating">
              <span class="material-icons">star</span>
              {current.vote_average?.toFixed(1) || "N/A"}
            </span>
            <span class="type">{current.media_type.toUpperCase()}</span>
          </div>
          {#if current.overview}
            <p class="hero-overview">{current.overview}</p>
          {/if}
          <a href="/{current.media_type}/{current.id}" class="hero-button">
            <span class="material-icons">play_arrow</span>
            View Details
          </a>
        </div>
      </div>

      <!-- Dot indicators -->
      <div class="hero-dots">
        {#each items as _, i}
          <button
            class="dot"
            class:active={i === currentIndex}
            onclick={() => goTo(i)}
            aria-label="Go to slide {i + 1}"
          ></button>
        {/each}
      </div>
    </div>
  {:else}
    <div class="hero-empty">
      <span class="material-icons">movie_filter</span>
      <p>No trending content</p>
    </div>
  {/if}
</div>

<style>
  .hero-banner {
    width: 100%;
    height: 100%;
    min-height: 200px;
    border-radius: 16px;
    overflow: hidden;
    position: relative;
  }

  .hero-skeleton {
    width: 100%;
    height: 100%;
    background: rgba(255, 255, 255, 0.05);
    position: relative;
    overflow: hidden;
  }

  .shimmer {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(255, 255, 255, 0.05),
      transparent
    );
    animation: shimmer 1.5s infinite;
  }

  @keyframes shimmer {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(100%);
    }
  }

  .hero-backdrop {
    width: 100%;
    height: 100%;
    background-size: cover;
    background-position: center;
    position: relative;
    transition: background-image 0.5s ease;
  }

  .hero-gradient {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      to right,
      rgba(0, 0, 0, 0.95) 0%,
      rgba(0, 0, 0, 0.7) 40%,
      rgba(0, 0, 0, 0.3) 70%,
      transparent 100%
    );
  }

  .hero-content {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    padding: 2rem;
  }

  .hero-info {
    max-width: 50%;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .hero-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    background: rgba(255, 100, 0, 0.9);
    color: white;
    padding: 0.3rem 0.75rem;
    border-radius: 4px;
    font-size: 0.65rem;
    font-weight: 800;
    letter-spacing: 0.1em;
    width: fit-content;
  }

  .hero-badge .material-icons {
    font-size: 14px;
  }

  .hero-title {
    font-size: 2rem;
    font-weight: 900;
    color: white;
    margin: 0;
    line-height: 1.1;
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.5);
  }

  .hero-meta {
    display: flex;
    align-items: center;
    gap: 1rem;
    font-size: 0.85rem;
  }

  .hero-meta .year {
    color: var(--color-primary, #00f3ff);
    font-weight: 700;
    font-family: var(--font-mono);
  }

  .hero-meta .rating {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    color: #ffd700;
    font-weight: 600;
  }

  .hero-meta .rating .material-icons {
    font-size: 16px;
  }

  .hero-meta .type {
    background: rgba(255, 255, 255, 0.1);
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    font-size: 0.7rem;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.7);
  }

  .hero-overview {
    font-size: 0.85rem;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.7);
    margin: 0;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .hero-button {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    background: var(--color-primary, #00f3ff);
    color: #000;
    padding: 0.75rem 1.5rem;
    border-radius: 8px;
    text-decoration: none;
    font-weight: 800;
    font-size: 0.85rem;
    transition: all 0.2s;
    width: fit-content;
    margin-top: 0.5rem;
  }

  .hero-button:hover {
    transform: scale(1.05);
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.4);
  }

  .hero-button .material-icons {
    font-size: 20px;
  }

  .hero-dots {
    position: absolute;
    bottom: 1.5rem;
    right: 2rem;
    display: flex;
    gap: 0.5rem;
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.3);
    border: none;
    cursor: pointer;
    transition: all 0.2s;
    padding: 0;
  }

  .dot:hover {
    background: rgba(255, 255, 255, 0.5);
  }

  .dot.active {
    background: var(--color-primary, #00f3ff);
    width: 24px;
    border-radius: 5px;
  }

  .hero-empty {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.02);
    color: var(--text-muted);
  }

  .hero-empty .material-icons {
    font-size: 48px;
    opacity: 0.3;
    margin-bottom: 0.5rem;
  }

  @media (max-width: 768px) {
    .hero-info {
      max-width: 100%;
    }
    .hero-title {
      font-size: 1.5rem;
    }
    .hero-overview {
      display: none;
    }
  }
</style>
