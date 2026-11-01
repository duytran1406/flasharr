<script module lang="ts">
  import type { RadarrMovie } from "$lib/stores/arr";

  /** A movie collection assembled from the Radarr library. */
  export interface LibraryCollection {
    tmdbId: number;
    title: string;
    movies: RadarrMovie[];
  }
</script>

<script lang="ts">
  import { getMoviePoster } from "$lib/stores/arr";

  interface Props {
    collections: LibraryCollection[];
  }

  let { collections }: Props = $props();
</script>

<div class="col-lib-grid">
  {#each collections as collection (collection.tmdbId)}
    <a href="/collection/{collection.tmdbId}" class="col-lib-card">
      <div class="col-lib-mosaic">
        {#each collection.movies.slice(0, 4) as movie}
          <div class="col-lib-tile">
            {#if getMoviePoster(movie)}
              <img src={getMoviePoster(movie)} alt={movie.title} loading="lazy" />
            {:else}
              <div class="col-lib-tile-blank">
                <span class="material-icons">movie</span>
              </div>
            {/if}
          </div>
        {/each}
        {#if collection.movies.length < 4}
          {#each Array(4 - collection.movies.length) as _}
            <div class="col-lib-tile col-lib-tile-blank">
              <span class="material-icons">add</span>
            </div>
          {/each}
        {/if}
      </div>
      <div class="col-lib-info">
        <span class="col-lib-title">{collection.title}</span>
        <span class="col-lib-count">{collection.movies.length} in library</span>
      </div>
    </a>
  {:else}
    <div class="empty-state">
      <span class="material-icons">collections_bookmark</span>
      <p>No collections found</p>
    </div>
  {/each}
</div>

<style>
  .col-lib-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 1rem;
  }
  .col-lib-card {
    text-decoration: none;
    color: inherit;
    display: flex;
    flex-direction: column;
    gap: .75rem;
  }
  .col-lib-mosaic {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: .2rem;
    aspect-ratio: 1.35;
    overflow: hidden;
    border-radius: 14px;
    background: #ffffff08;
  }
  .col-lib-tile,
  .col-lib-tile-blank {
    position: relative;
    overflow: hidden;
    background: #ffffff0a;
  }
  .col-lib-tile img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .col-lib-tile-blank {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
  }
  .col-lib-info {
    display: flex;
    flex-direction: column;
    gap: .2rem;
  }
  .col-lib-title {
    font-size: .86rem;
    font-weight: 700;
    color: #fff;
  }
  .col-lib-count {
    font-size: .72rem;
    color: var(--text-muted);
  }
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: .75rem;
    min-height: 40vh;
    color: var(--text-muted);
    text-align: center;
  }
  @media (max-width: 1024px) {
    .col-lib-grid {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }
  @media (max-width: 768px) {
    .col-lib-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: .85rem;
    }
  }
</style>
