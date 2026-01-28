<script lang="ts">
  import MediaCard from "../cards/MediaCard.svelte";
  import SkeletonCard from "../cards/SkeletonCard.svelte";

  interface MediaItem {
    id: number;
    title?: string;
    name?: string;
    poster_path?: string | null;
    vote_average?: number;
    release_date?: string;
    first_air_date?: string;
    overview?: string;
  }

  interface Props {
    title: string;
    items: MediaItem[];
    mediaType: "movie" | "tv";
    loading?: boolean;
    columns?: number;
  }

  let {
    title,
    items,
    mediaType,
    loading = false,
    columns = 4,
  }: Props = $props();

  // Safety: Ensure columns is always a valid positive integer
  const safeColumns = $derived(Math.max(0, Math.floor(columns || 4)));

  function getTitle(item: MediaItem): string {
    return item.title || item.name || "Unknown";
  }

  function getReleaseDate(item: MediaItem): string {
    return item.release_date || item.first_air_date || "";
  }
</script>

<section class="related-section">
  <h3 class="section-label">{title}</h3>
  <div class="related-grid" style="--columns: {columns}">
    {#if loading}
      {#each Array(safeColumns) as _}
        <SkeletonCard mode="poster" />
      {/each}
    {:else if items.length > 0}
      {#each items as item}
        <MediaCard
          id={item.id}
          title={getTitle(item)}
          posterPath={item.poster_path}
          voteAverage={item.vote_average}
          releaseDate={getReleaseDate(item)}
          overview={item.overview}
          {mediaType}
        />
      {/each}
    {:else}
      <div class="empty-state">
        <span class="material-icons">movie_filter</span>
        <p>No related content found</p>
      </div>
    {/if}
  </div>
</section>

<style>
  .related-section {
    margin-bottom: 4rem;
  }

  .section-label {
    text-transform: uppercase;
    letter-spacing: 0.15em;
    font-size: 0.75rem;
    color: var(--color-primary);
    margin-bottom: 1.5rem;
    font-weight: 800;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .related-grid {
    display: grid;
    grid-template-columns: repeat(var(--columns, 4), 1fr);
    gap: 2rem;
  }

  .empty-state {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 3rem;
    color: var(--text-muted);
    opacity: 0.5;
  }

  .empty-state .material-icons {
    font-size: 48px;
    margin-bottom: 1rem;
    opacity: 0.3;
  }

  .empty-state p {
    font-size: 0.8rem;
    font-family: var(--font-mono, monospace);
  }

  @media (max-width: 900px) {
    .related-grid {
      grid-template-columns: repeat(3, 1fr);
    }
  }

  @media (max-width: 600px) {
    .related-grid {
      grid-template-columns: repeat(2, 1fr);
    }
  }
</style>
