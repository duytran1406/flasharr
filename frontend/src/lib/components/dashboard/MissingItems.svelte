<script lang="ts">
  import { onMount } from "svelte";
  import { toasts } from "$lib/stores/toasts";

  interface MissingItem {
    id: number;
    type: "movie" | "tv";
    title: string;
    year?: number;
    season?: number;
    episode?: number;
    episodeTitle?: string;
    airDate?: string;
    tmdbId: number;
    monitored: boolean;
  }

  let items = $state<MissingItem[]>([]);
  let loading = $state(true);
  let filter = $state<"all" | "movie" | "tv">("all");

  onMount(async () => {
    try {
      const response = await fetch("/api/arr/missing?page=1&page_size=50");

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const data = await response.json();

      // Parse movies from Radarr
      const movies: MissingItem[] = (data.movies?.records || []).map(
        (m: any) => ({
          id: m.id,
          type: "movie" as const,
          title: m.title,
          year: m.year,
          tmdbId: m.tmdbId,
          monitored: m.monitored,
        }),
      );

      // Parse episodes from Sonarr
      const episodes: MissingItem[] = (data.episodes?.records || []).map(
        (e: any) => ({
          id: e.id,
          type: "tv" as const,
          title: e.series?.title || e.seriesTitle || "Unknown Series",
          season: e.seasonNumber,
          episode: e.episodeNumber,
          episodeTitle: e.title,
          airDate: e.airDateUtc,
          tmdbId: e.series?.tvdbId || e.seriesId || 0,
          monitored: e.monitored,
        }),
      );

      items = [...movies, ...episodes].sort((a, b) => {
        // Sort by air date if available, otherwise by title
        if (a.airDate && b.airDate) {
          return new Date(b.airDate).getTime() - new Date(a.airDate).getTime();
        }
        return a.title.localeCompare(b.title);
      });
    } catch (e: any) {
      console.error("Failed to load missing items:", e);
      toasts.error(`Missing Items Error: ${e.message}`);
    } finally {
      loading = false;
    }
  });

  let filteredItems = $derived(
    filter === "all" ? items : items.filter((i) => i.type === filter),
  );

  function formatEpisode(season?: number, episode?: number): string {
    if (!season || !episode) return "";
    return `S${String(season).padStart(2, "0")}E${String(episode).padStart(2, "0")}`;
  }

  function formatDate(dateStr?: string): string {
    if (!dateStr) return "";
    const date = new Date(dateStr);
    const now = new Date();
    const diffDays = Math.floor(
      (now.getTime() - date.getTime()) / (1000 * 60 * 60 * 24),
    );

    if (diffDays === 0) return "Today";
    if (diffDays === 1) return "Yesterday";
    if (diffDays < 7) return `${diffDays}d ago`;
    if (diffDays < 30) return `${Math.floor(diffDays / 7)}w ago`;
    return date.toLocaleDateString("en-US", { month: "short", day: "numeric" });
  }

  async function searchItem(item: MissingItem) {
    try {
      const endpoint =
        item.type === "movie"
          ? `/api/arr/movies/${item.id}/search`
          : `/api/arr/episodes/${item.id}/search`;

      await fetch(endpoint, { method: "POST" });
    } catch (e) {
      console.error("Search failed:", e);
    }
  }
</script>

<div class="missing-widget">
  <div class="widget-header">
    <div class="header-left">
      <span class="material-icons">warning</span>
      <h3>Wanted: Missing</h3>
      <span class="count">{filteredItems.length}</span>
    </div>
    <div class="filter-tabs">
      <button
        class="tab"
        class:active={filter === "all"}
        onclick={() => (filter = "all")}
      >
        All
      </button>
      <button
        class="tab"
        class:active={filter === "movie"}
        onclick={() => (filter = "movie")}
      >
        Movies
      </button>
      <button
        class="tab"
        class:active={filter === "tv"}
        onclick={() => (filter = "tv")}
      >
        TV
      </button>
    </div>
  </div>

  <div class="items-list">
    {#if loading}
      <div class="loading">
        <div class="spinner"></div>
        <span>Loading missing items...</span>
      </div>
    {:else if filteredItems.length === 0}
      <div class="empty">
        <span class="material-icons">check_circle</span>
        <p>No missing items!</p>
      </div>
    {:else}
      {#each filteredItems.slice(0, 20) as item (item.id)}
        <div class="item-row">
          <div class="item-icon">
            <span class="material-icons">
              {item.type === "movie" ? "movie" : "tv"}
            </span>
          </div>
          <div class="item-info">
            <a href="/{item.type}/{item.tmdbId}" class="item-title">
              {item.title}
              {#if item.year}
                <span class="year">({item.year})</span>
              {/if}
            </a>
            {#if item.type === "tv"}
              <div class="item-meta">
                <span class="episode-code"
                  >{formatEpisode(item.season, item.episode)}</span
                >
                {#if item.episodeTitle}
                  <span class="episode-title">{item.episodeTitle}</span>
                {/if}
              </div>
            {/if}
          </div>
          <div class="item-date">
            {formatDate(item.airDate)}
          </div>
          <button class="search-btn" onclick={() => searchItem(item)}>
            <span class="material-icons">search</span>
          </button>
        </div>
      {/each}
      {#if filteredItems.length > 20}
        <div class="view-more">
          <a href="/wanted">View all {filteredItems.length} items →</a>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .missing-widget {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    height: 100%;
  }

  .widget-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .header-left .material-icons {
    color: #ffcc00;
    font-size: 1.2rem;
  }

  .widget-header h3 {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: rgba(255, 255, 255, 0.9);
  }

  .count {
    background: rgba(255, 204, 0, 0.15);
    color: #ffcc00;
    padding: 0.25rem 0.6rem;
    border-radius: 12px;
    font-size: 0.75rem;
    font-weight: 700;
  }

  .filter-tabs {
    display: flex;
    gap: 0.5rem;
  }

  .tab {
    padding: 0.4rem 0.8rem;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    color: rgba(255, 255, 255, 0.5);
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .tab:hover {
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.8);
  }

  .tab.active {
    background: var(--color-primary);
    color: #000;
    border-color: var(--color-primary);
  }

  .items-list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .loading,
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 3rem 1rem;
    gap: 1rem;
    color: rgba(255, 255, 255, 0.4);
  }

  .spinner {
    width: 24px;
    height: 24px;
    border: 2px solid rgba(255, 255, 255, 0.1);
    border-top-color: var(--color-primary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .empty .material-icons {
    font-size: 2.5rem;
    color: rgba(0, 255, 128, 0.3);
  }

  .item-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.04);
    border-radius: 8px;
    transition: all 0.2s;
  }

  .item-row:hover {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.1);
  }

  .item-icon {
    flex-shrink: 0;
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 6px;
  }

  .item-icon .material-icons {
    font-size: 1.1rem;
    color: var(--color-primary);
  }

  .item-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .item-title {
    font-size: 0.85rem;
    font-weight: 600;
    color: white;
    text-decoration: none;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: color 0.2s;
  }

  .item-title:hover {
    color: var(--color-primary);
  }

  .year {
    color: rgba(255, 255, 255, 0.4);
    font-weight: 400;
  }

  .item-meta {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.75rem;
  }

  .episode-code {
    font-family: var(--font-mono, monospace);
    color: var(--color-primary);
    font-weight: 700;
  }

  .episode-title {
    color: rgba(255, 255, 255, 0.5);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .item-date {
    flex-shrink: 0;
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.4);
    font-weight: 500;
    min-width: 60px;
    text-align: right;
  }

  .search-btn {
    flex-shrink: 0;
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .search-btn:hover {
    background: var(--color-primary);
    border-color: var(--color-primary);
  }

  .search-btn:hover .material-icons {
    color: #000;
  }

  .search-btn .material-icons {
    font-size: 1rem;
    color: rgba(255, 255, 255, 0.6);
    transition: color 0.2s;
  }

  .view-more {
    padding: 1rem;
    text-align: center;
  }

  .view-more a {
    color: var(--color-primary);
    text-decoration: none;
    font-size: 0.85rem;
    font-weight: 600;
    transition: opacity 0.2s;
  }

  .view-more a:hover {
    opacity: 0.8;
  }

  /* Scrollbar */
  .items-list::-webkit-scrollbar {
    width: 4px;
  }

  .items-list::-webkit-scrollbar-track {
    background: transparent;
  }

  .items-list::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 2px;
  }
</style>
