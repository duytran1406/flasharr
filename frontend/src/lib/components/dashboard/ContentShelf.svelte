<script lang="ts">
  interface Props {
    title: string;
    items: any[];
    loading?: boolean;
  }

  let { title, items, loading = false } = $props();
</script>

<div class="content-shelf">
  <div class="shelf-header">
    <h2>{title}</h2>
    <div class="shelf-count">{items.length} items</div>
  </div>

  {#if loading}
    <div class="shelf-loading">
      <div class="skeleton-grid">
        {#each Array(6) as _}
          <div class="skeleton-card"></div>
        {/each}
      </div>
    </div>
  {:else if items.length > 0}
    <div class="shelf-scroll">
      {#each items as item (item.tmdbId || item.id)}
        <a
          href="/{item.type === 'tv' ? 'tv' : 'movie'}/{item.tmdbId || item.id}"
          class="shelf-card"
        >
          <div class="card-poster">
            {#if item.poster}
              <img src={item.poster} alt={item.title} loading="lazy" />
            {:else}
              <div class="poster-placeholder">
                <span class="material-icons"
                  >{item.type === "tv" ? "live_tv" : "movie"}</span
                >
              </div>
            {/if}

            <!-- Status overlay -->
            {#if item.status === "downloading"}
              <div class="status-overlay downloading">
                <div class="pulse-dot"></div>
                <span>Downloading</span>
              </div>
            {:else if item.status === "available"}
              <div class="status-badge available">
                <span class="material-icons">check_circle</span>
              </div>
            {/if}
          </div>

          <div class="card-info">
            <div class="card-title">{item.title}</div>
            {#if item.year}
              <div class="card-meta">{item.year}</div>
            {/if}
          </div>
        </a>
      {/each}
    </div>
  {:else}
    <div class="shelf-empty">
      <span class="material-icons">inbox</span>
      <p>No items yet</p>
    </div>
  {/if}
</div>

<style>
  .content-shelf {
    margin-bottom: 3rem;
  }

  .shelf-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1rem;
    padding: 0 1.5rem;
  }

  .shelf-header h2 {
    font-size: 1.5rem;
    font-weight: 700;
    color: #f1f5f9;
    margin: 0;
  }

  .shelf-count {
    font-size: 0.75rem;
    color: var(--text-muted);
    opacity: 0.6;
    font-family: var(--font-mono, monospace);
  }

  .shelf-scroll {
    display: flex;
    gap: 1rem;
    overflow-x: auto;
    overflow-y: hidden;
    padding: 0 1.5rem 1rem;
    scroll-behavior: smooth;
    -webkit-overflow-scrolling: touch;
  }

  .shelf-scroll::-webkit-scrollbar {
    height: 6px;
  }

  .shelf-scroll::-webkit-scrollbar-track {
    background: rgba(255, 255, 255, 0.05);
    border-radius: 3px;
  }

  .shelf-scroll::-webkit-scrollbar-thumb {
    background: rgba(167, 139, 250, 0.3);
    border-radius: 3px;
  }

  .shelf-scroll::-webkit-scrollbar-thumb:hover {
    background: rgba(167, 139, 250, 0.5);
  }

  .shelf-card {
    flex: 0 0 180px;
    text-decoration: none;
    color: inherit;
    transition: transform 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .shelf-card:hover {
    transform: translateY(-8px);
  }

  .card-poster {
    position: relative;
    aspect-ratio: 2/3;
    border-radius: 8px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.05);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }

  .card-poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: filter 0.3s;
  }

  .shelf-card:hover .card-poster img {
    filter: brightness(1.1);
  }

  .poster-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    opacity: 0.3;
  }

  .poster-placeholder .material-icons {
    font-size: 48px;
  }

  .status-overlay {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    background: linear-gradient(to top, rgba(0, 0, 0, 0.9), transparent);
    padding: 0.5rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .status-overlay.downloading {
    color: #60a5fa;
  }

  .pulse-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #60a5fa;
    animation: pulse 2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
      transform: scale(1);
    }
    50% {
      opacity: 0.5;
      transform: scale(1.2);
    }
  }

  .status-badge {
    position: absolute;
    top: 8px;
    right: 8px;
    opacity: 0.9;
  }

  .status-badge.available .material-icons {
    font-size: 20px;
    color: #34d399;
    filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.8));
  }

  .card-info {
    margin-top: 0.75rem;
  }

  .card-title {
    font-size: 0.875rem;
    font-weight: 600;
    color: #f1f5f9;
    line-height: 1.3;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .card-meta {
    font-size: 0.75rem;
    color: var(--text-muted);
    opacity: 0.7;
    margin-top: 0.25rem;
  }

  .shelf-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 4rem 1.5rem;
    color: var(--text-muted);
    opacity: 0.5;
  }

  .shelf-empty .material-icons {
    font-size: 48px;
    margin-bottom: 1rem;
  }

  .shelf-empty p {
    font-size: 0.875rem;
    font-weight: 600;
  }

  .skeleton-grid {
    display: flex;
    gap: 1rem;
    padding: 0 1.5rem;
  }

  .skeleton-card {
    flex: 0 0 180px;
    aspect-ratio: 2/3;
    background: linear-gradient(
      90deg,
      rgba(255, 255, 255, 0.05) 0%,
      rgba(255, 255, 255, 0.1) 50%,
      rgba(255, 255, 255, 0.05) 100%
    );
    background-size: 200% 100%;
    animation: shimmer 1.5s infinite;
    border-radius: 8px;
  }

  @keyframes shimmer {
    0% {
      background-position: -200% 0;
    }
    100% {
      background-position: 200% 0;
    }
  }
</style>
