<script lang="ts">
  import type { TMDBMovie, TMDBTVShow } from "$lib/types/tmdb";
  import { ErrorState } from "@media-set/core-ui";
  import MediaCard from "$lib/components/cards/MediaCard.svelte";
  import type { DiscoverMediaType } from "$lib/components/DiscoverFilterBar.svelte";

  export type DiscoverItem = TMDBMovie | TMDBTVShow;

  /** One pre-allocated slot of the discover object pool. */
  export interface PoolSlot {
    slotId: number;
    visible: boolean;
    data: DiscoverItem | null;
  }

  /** Library state colour: purple = downloading, gray = not in library. */
  export type DiscoverStatusType =
    | "blue"
    | "yellow"
    | "red"
    | "green"
    | "purple"
    | "gray";

  export interface DiscoverBadge {
    text: string;
    variant: string;
  }

  interface Props {
    pool: PoolSlot[];
    loading: boolean;
    error: string | null;
    hasMore: boolean;
    mediaType: DiscoverMediaType;
    getTitle: (item: DiscoverItem) => string;
    getBadge: (item: DiscoverItem) => DiscoverBadge | null;
    getStatusType: (item: DiscoverItem) => DiscoverStatusType;
    getStatusLabel: (item: DiscoverItem) => string;
    onRetry: () => void;
    onClear: () => void;
  }

  let {
    pool,
    loading,
    error,
    hasMore,
    mediaType,
    getTitle,
    getBadge,
    getStatusType,
    getStatusLabel,
    onRetry,
    onClear,
  }: Props = $props();

  let visibleCount = $derived(pool.filter((slot) => slot.visible).length);
</script>

<div class="discover-grid">
  {#if loading && visibleCount === 0}
    <div class="initial-loading">
      <div class="spinner"></div><p>Loading content...</p>
    </div>
  {:else if error && visibleCount === 0}
    <div class="error-container">
      <ErrorState
        title="Failed to load content"
        message={error}
        showRetry={true}
        {onRetry}
      />
    </div>
  {:else if visibleCount === 0}
    <div class="empty-results" role="status">
      <span class="material-icons" aria-hidden="true">search_off</span>
      <h2>No titles found</h2>
      <p>Try different search terms or fewer filters.</p>
      <button type="button" onclick={onClear}>Clear filters</button>
    </div>
  {:else}
    {#each pool as slot (slot.slotId)}
      {#if slot.visible && slot.data}
        <MediaCard
          id={slot.data.id}
          title={getTitle(slot.data)}
          posterPath={slot.data.poster_path}
          voteAverage={slot.data.vote_average}
          releaseDate={"release_date" in slot.data
            ? slot.data.release_date
            : slot.data.first_air_date}
          overview={slot.data.overview}
          mediaType={mediaType as "movie" | "tv"}
          badge={getBadge(slot.data)
            ? {
                text: getBadge(slot.data)!.text,
                variant: getBadge(slot.data)!.variant,
              }
            : undefined}
          statusType={getStatusType(slot.data)}
          statusLabel={getStatusLabel(slot.data)}
        />
      {/if}
    {/each}
  {/if}
</div>

{#if loading && visibleCount > 0}
  <div class="loading-indicator">
    <div class="spinner"></div><p>Loading more...</p>
  </div>
{/if}

{#if !hasMore && visibleCount > 0 && !loading}
  <p class="end-label">— end of results —</p>
{/if}

<style>
  /* Grid — 5 items per row on wide screens */
  .discover-grid {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: var(--cui-space-lg);
    padding-bottom: var(--cui-space-xl);
  }

  .error-container,
  .initial-loading,
  .empty-results {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    min-height: 400px;
    gap: var(--cui-space-md);
  }

  .empty-results {
    text-align: center;
    color: var(--cui-text-secondary);
  }

  .empty-results .material-icons {
    font-size: 2rem;
    color: var(--cui-primary);
  }

  .empty-results h2 {
    margin: 0;
  }

  .empty-results p {
    margin: 0;
  }

  .empty-results button {
    min-height: var(--cui-touch-min);
    padding: 0 var(--cui-space-md);
    border: 1px solid var(--cui-primary);
    background: var(--cui-surface-focus);
    color: var(--cui-primary);
    cursor: pointer;
  }

  .initial-loading p {
    color: var(--cui-text-tertiary);
    font-size: var(--cui-text-base);
    font-family: var(--cui-font-mono);
    letter-spacing: 0.1em;
  }

  /* Loading */
  .loading-indicator {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: var(--cui-space-xxl);
    gap: var(--cui-space-lg);
    color: var(--cui-text-tertiary);
  }

  .spinner {
    width: 30px;
    height: 30px;
    border: 2px solid var(--cui-surface-active);
    border-top-color: var(--cui-primary);
    border-radius: var(--cui-radius-full);
    animation: spin var(--cui-duration-slow) linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .end-label {
    text-align: center;
    padding: var(--cui-space-lg) 0 var(--cui-space-xxl);
    color: var(--cui-text-disabled);
    font-size: var(--cui-text-xs);
    font-family: var(--cui-font-mono);
    letter-spacing: 0.12em;
  }

  /* Responsive */
  @media (max-width: 1280px) {
    .discover-grid {
      grid-template-columns: repeat(4, 1fr);
      gap: var(--cui-space-lg);
    }
  }

  @media (max-width: 1024px) {
    .discover-grid {
      grid-template-columns: repeat(3, 1fr);
      gap: var(--cui-space-md);
    }
  }

  @media (max-width: 768px) {
    .discover-grid {
      grid-template-columns: 1fr;
      gap: var(--cui-space-md);
      max-width: 430px;
      margin-inline: auto;
    }
  }
</style>
