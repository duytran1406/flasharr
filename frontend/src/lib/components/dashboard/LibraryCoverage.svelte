<script lang="ts">
  import { onMount } from "svelte";
  import { fetchLibraryOverview, type LibraryOverview } from "$lib/stores/arr";

  let library = $state<LibraryOverview | null>(null);
  let loading = $state(true);

  onMount(async () => {
    try {
      library = await fetchLibraryOverview();
    } catch (e) {
      console.error("Failed to load library coverage:", e);
    } finally {
      loading = false;
    }
  });

  let movieCoverage = $derived(
    library
      ? Math.round(
          ((library.movie_count - library.movies_missing) /
            library.movie_count) *
            100,
        ) || 0
      : 0,
  );

  let episodeCoverage = $derived(
    library
      ? Math.round(
          (library.episodes_with_files /
            (library.episodes_with_files + library.episodes_missing)) *
            100,
        ) || 0
      : 0,
  );
</script>

<div class="library-coverage">
  {#if loading}
    <div class="coverage-skeleton"></div>
  {:else if library}
    <div class="coverage-grid">
      <div class="coverage-item">
        <div class="item-main">
          <span class="value">{library.movie_count}</span>
          <span class="label">Movies</span>
        </div>
        <div class="item-progress">
          <div class="progress-bar">
            <div class="progress-fill" style="width: {movieCoverage}%"></div>
          </div>
          <span class="percentage">{movieCoverage}% Owned</span>
        </div>
      </div>

      <div class="coverage-item">
        <div class="item-main">
          <span class="value">{library.series_count}</span>
          <span class="label">Series</span>
        </div>
        <div class="item-progress">
          <div class="progress-bar">
            <div class="progress-fill" style="width: {episodeCoverage}%"></div>
          </div>
          <span class="percentage">{episodeCoverage}% Complete</span>
        </div>
      </div>

      <div class="coverage-item highlight">
        <div class="item-main">
          <span class="value"
            >{library.episodes_missing + library.movies_missing}</span
          >
          <span class="label">Missing</span>
        </div>
        <div class="item-action">
          <span class="hint">Items awaiting download</span>
        </div>
      </div>
    </div>
  {:else}
    <div class="coverage-error">
      <span>Connect services to see library coverage</span>
    </div>
  {/if}
</div>

<style>
  .library-coverage {
    width: 100%;
    margin: 1rem 0;
  }

  .coverage-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1.5rem;
  }

  .coverage-item {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 16px;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    backdrop-filter: blur(10px);
    transition: all 0.3s ease;
  }

  .coverage-item:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.1);
    transform: translateY(-2px);
  }

  .item-main {
    display: flex;
    flex-direction: column;
  }

  .value {
    font-size: 2rem;
    font-weight: 800;
    color: white;
    line-height: 1;
    letter-spacing: -0.02em;
  }

  .label {
    font-size: 0.8rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.4);
    text-transform: uppercase;
    letter-spacing: 0.1em;
    margin-top: 0.25rem;
  }

  .item-progress {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .progress-bar {
    height: 6px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 3px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(to right, var(--color-primary), #00ff80);
    border-radius: 3px;
    transition: width 1s cubic-bezier(0.23, 1, 0.32, 1);
  }

  .percentage {
    font-size: 0.75rem;
    font-weight: 700;
    color: var(--color-primary);
  }

  .coverage-item.highlight {
    background: rgba(255, 204, 0, 0.05);
    border-color: rgba(255, 204, 0, 0.1);
  }

  .coverage-item.highlight .value {
    color: #ffcc00;
  }

  .hint {
    font-size: 0.75rem;
    color: rgba(255, 204, 0, 0.6);
    font-style: italic;
  }

  .coverage-skeleton {
    height: 120px;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 16px;
    animation: pulse 2s infinite;
  }

  @keyframes pulse {
    0% {
      opacity: 0.5;
    }
    50% {
      opacity: 1;
    }
    100% {
      opacity: 0.5;
    }
  }

  .coverage-error {
    padding: 2rem;
    text-align: center;
    background: rgba(255, 255, 255, 0.02);
    border-radius: 16px;
    color: rgba(255, 255, 255, 0.3);
    font-size: 0.9rem;
    border: 1px dashed rgba(255, 255, 255, 0.1);
  }

  @media (max-width: 768px) {
    .coverage-grid {
      grid-template-columns: 1fr;
      gap: 1rem;
    }
  }
</style>
