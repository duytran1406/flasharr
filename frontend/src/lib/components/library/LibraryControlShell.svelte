<script lang="ts">
  import DiscoverFilterBar, {
    type DiscoverMediaType,
    type UnifiedGenre,
  } from "$lib/components/DiscoverFilterBar.svelte";
  import type { LibraryCollection } from "./LibraryCollectionsGrid.svelte";

  interface Props {
    activeTab: "tv" | "movies" | "collections";
    filterMediaType: "movie" | "tv";
    collections: LibraryCollection[];
    sortBy: string;
    searchQuery: string;
    selectedGenres: UnifiedGenre[];
    fromYear: string;
    toYear: string;
    minRating: number;
    maxRating: number;
    resolvedGenreIds: number[];
    hasFilter: boolean;
    filterStatus: string;
    onMediaTypeChange: (type: DiscoverMediaType) => void;
    onSortChange: (value: string) => void;
    onSearchInput: () => void;
    onGenreToggle: (genre: UnifiedGenre) => void;
    onYearChange: () => void;
    onRatingChange: () => void;
    onClearAll: () => void;
  }

  let {
    activeTab,
    filterMediaType,
    collections,
    sortBy = $bindable(),
    searchQuery = $bindable(),
    selectedGenres = $bindable(),
    fromYear = $bindable(),
    toYear = $bindable(),
    minRating = $bindable(),
    maxRating = $bindable(),
    resolvedGenreIds = $bindable(),
    hasFilter = $bindable(),
    filterStatus = $bindable(),
    onMediaTypeChange,
    onSortChange,
    onSearchInput,
    onGenreToggle,
    onYearChange,
    onRatingChange,
    onClearAll,
  }: Props = $props();

  const collectionMovieCount = $derived(
    collections.reduce((sum, collection) => sum + collection.movies.length, 0),
  );
</script>

<div class="library-control-shell">
  <DiscoverFilterBar
    mediaType={activeTab === "collections" ? "collections" : filterMediaType}
    includeCollectionTab
    hideNonCollectionFilters
    {onMediaTypeChange}
    {onSortChange}
    {onSearchInput}
    {onGenreToggle}
    {onYearChange}
    {onRatingChange}
    {onClearAll}
    bind:sortBy
    bind:searchQuery
    bind:selectedGenres
    bind:fromYear
    bind:toYear
    bind:minRating
    bind:maxRating
    bind:resolvedGenreIds
    bind:hasFilter
  />

  {#if activeTab === "collections"}
    <div class="collection-mode-bar glass-panel">
      <div>
        <p class="collection-mode-kicker">Collection</p>
        <h3>Grouped movie collections</h3>
        <p>{collectionMovieCount} movies across {collections.length} collections</p>
      </div>
    </div>
  {/if}

  {#if activeTab !== "collections"}
    <div class="library-toolbar">
      <select bind:value={filterStatus} class="sort-select">
        <option value="all">All</option>{#if activeTab === "tv"}
          <option value="continuing">Continuing</option>
          <option value="ended">Ended</option>
          <option value="missing">Has Missing</option>
          <option value="complete">Complete</option>
        {:else}
          <option value="downloaded">Downloaded</option>
          <option value="missing">Missing File</option>
          <option value="monitored">Monitored</option>
        {/if}
      </select>
    </div>
  {/if}
</div>

<style>
  .library-control-shell {
    display: flex;
    flex-direction: column;
    gap: .75rem;
    padding: .75rem 1rem 0;
    flex-shrink: 0;
  }
  .library-toolbar {
    display: flex;
    align-items: center;
    gap: .5rem;
    justify-content: flex-end;
  }
  .collection-mode-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    padding: .9rem 1rem;
  }
  .collection-mode-kicker {
    margin: 0 0 .2rem;
    font-size: .68rem;
    font-weight: 700;
    letter-spacing: .12em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .collection-mode-bar h3 {
    margin: 0;
    font-size: 1rem;
    font-weight: 800;
    color: #fff;
  }
  .collection-mode-bar p {
    margin: .2rem 0 0;
    color: var(--text-muted);
    font-size: .78rem;
  }
  .sort-select {
    background: #ffffff08;
    border: 1px solid rgba(255, 255, 255, .06);
    color: #fff;
    padding: .55rem .8rem;
    border-radius: 8px;
  }
  .sort-select option {
    background: #111827;
  }
  @media (max-width: 768px) {
    .library-control-shell {
      position: sticky;
      top: 0;
      z-index: 20;
      background: #0a0f18f0;
      -webkit-backdrop-filter: blur(14px);
      backdrop-filter: blur(14px);
      padding: .65rem .75rem .75rem;
      border-bottom: 1px solid rgba(255, 255, 255, .05);
    }
    .library-toolbar {
      justify-content: space-between;
      gap: .4rem;
    }
    .sort-select {
      min-height: 44px;
    }
    .collection-mode-bar {
      flex-direction: column;
      align-items: flex-start;
      padding: .8rem;
    }
  }
</style>
