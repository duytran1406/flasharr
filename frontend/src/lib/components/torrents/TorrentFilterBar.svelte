<script lang="ts">
  import { Badge, Button } from "@media-set/core-ui";
  import { qbitStore } from "$lib/stores/qbit.svelte";
  import { toasts } from "$lib/stores/toasts";

  interface FilterPill {
    label: string;
    clear: () => void;
  }

  const statusOptions = $derived(qbitStore.filterOptions.status);
  const categoryOptions = $derived(qbitStore.filterOptions.category);
  const tagOptions = $derived(qbitStore.filterOptions.tag);
  const trackerOptions = $derived(qbitStore.filterOptions.tracker);

  /** Mobile: the filter grid collapses behind a "Filters" toggle. */
  let filtersOpen = $state(false);

  const activeFilters = $derived(
    [
      qbitStore.search && {
        label: `Search: ${qbitStore.search}`,
        clear: () => (qbitStore.search = ""),
      },
      qbitStore.filters.status && {
        label: `State: ${qbitStore.filters.status}`,
        clear: () => (qbitStore.filters.status = ""),
      },
      qbitStore.filters.category && {
        label: `Category: ${qbitStore.filters.category}`,
        clear: () => (qbitStore.filters.category = ""),
      },
      qbitStore.filters.tag && {
        label: `Tag: ${qbitStore.filters.tag}`,
        clear: () => (qbitStore.filters.tag = ""),
      },
      qbitStore.filters.tracker && {
        label: `Tracker: ${qbitStore.filters.tracker}`,
        clear: () => (qbitStore.filters.tracker = ""),
      },
    ].filter(Boolean) as FilterPill[],
  );

  async function applyFilters() {
    qbitStore.clearSelection();
    await qbitStore.fetchTorrents();
  }

  function applyOrToast() {
    applyFilters().catch((err) => toasts.error(String(err)));
  }

  async function removeFilter(clear: () => void) {
    clear();
    await applyFilters().catch((err) => toasts.error(String(err)));
  }
</script>

<section class="bento-card filter-bar">
  <div class="filter-actions filter-summary">
    <button
      class="filter-toggle"
      aria-expanded={filtersOpen}
      aria-controls="torrent-filters"
      onclick={() => (filtersOpen = !filtersOpen)}
    >
      Filters{activeFilters.length ? ` (${activeFilters.length})` : ""}
    </button>
  </div>

  <div id="torrent-filters" class="filters" class:filters-open={filtersOpen}>
    <div class="search-field">
      <span class="material-icons search-icon" aria-hidden="true">search</span>
      <input
        class="filter-input"
        placeholder="Search torrents…"
        aria-label="Search torrents"
        onkeydown={(e) => e.key === "Enter" && applyOrToast()}
        bind:value={qbitStore.search}
      />
    </div>
    <select
      class="filter-select"
      aria-label="Filter by status"
      onchange={applyOrToast}
      bind:value={qbitStore.filters.status}
    >
      <option value="">All states</option>
      {#each statusOptions as status}
        <option value={status}>{status}</option>
      {/each}
    </select>
    <select
      class="filter-select"
      aria-label="Filter by category"
      onchange={applyOrToast}
      bind:value={qbitStore.filters.category}
    >
      <option value="">All categories</option>
      {#each categoryOptions as category}
        <option value={category}>{category}</option>
      {/each}
    </select>
    <select
      class="filter-select"
      aria-label="Filter by tag"
      onchange={applyOrToast}
      bind:value={qbitStore.filters.tag}
    >
      <option value="">All tags</option>
      {#each tagOptions as tag}
        <option value={tag}>{tag}</option>
      {/each}
    </select>
    <input
      class="filter-input"
      placeholder="Tracker domain"
      aria-label="Filter by tracker"
      list="tracker-options"
      onkeydown={(e) => e.key === "Enter" && applyOrToast()}
      bind:value={qbitStore.filters.tracker}
    />
    <datalist id="tracker-options">
      {#each trackerOptions as tracker}
        <option value={tracker}></option>
      {/each}
    </datalist>
  </div>

  <div class="filter-actions">
    <Button
      variant="ghost"
      size="sm"
      onclick={() => {
        qbitStore.clearFilters();
        applyOrToast();
      }}>Clear</Button
    >
  </div>

  {#if activeFilters.length > 0}
    <div class="filter-pills">
      {#each activeFilters as pill}
        <button
          class="filter-pill"
          aria-label={`Remove filter: ${pill.label}`}
          onclick={() => removeFilter(pill.clear)}
        >
          <Badge text={pill.label} variant="grey" />
          <span class="material-icons pill-remove">close</span>
        </button>
      {/each}
    </div>
  {/if}
</section>

<style>
	.filter-bar {
		min-height: 52px;
		padding: 6px 16px;
		display: flex;
		align-items: center;
		gap: 8px;
		border-top: 1px solid var(--cui-glass-border);
		border-bottom: 1px solid var(--cui-glass-border);
	}

	.filters {
		display: grid;
		grid-template-columns: minmax(300px,500px) repeat(4,minmax(120px,1fr));
		gap: 8px;
		flex: 1;
	}

	.search-field {
		position: relative;
		display: flex;
		align-items: center;
	}

	.search-icon {
		position: absolute;
		left: .65rem;
		color: var(--cui-text-tertiary);
		font-size: 1rem;
		pointer-events: none;
	}

	.search-field .filter-input {
		padding-left: 2.1rem;
	}

	.filter-input,
	.filter-select {
		background: var(--cui-input-bg);
		border: 1px solid var(--cui-glass-border);
		border-radius: 0;
		color: var(--cui-text-primary);
		padding: .45rem .65rem;
		font-size: .72rem;
		width: 100%;
		height: 38px;
		min-height: 0;
	}

	.filter-input:focus-visible,
	.filter-select:focus-visible {
		outline: 2px solid var(--cui-primary);
		outline-offset: 1px;
	}

	.filter-actions {
		display: flex;
		justify-content: flex-end;
	}

	.filter-summary {
		display: none;
	}

	.filter-toggle {
		min-height: 44px;
		padding: 0 .75rem;
		background: transparent;
		border: 1px solid var(--cui-glass-border);
		color: var(--cui-text-primary);
		cursor: pointer;
	}

	.filter-pills {
		display: none;
	}

	.filter-pill {
		display: inline-flex;
		align-items: center;
		gap: .3rem;
		background: none;
		border: 0;
		padding: 0;
		cursor: pointer;
	}

	.filter-pill .pill-remove {
		font-size: .85rem;
		color: var(--cui-text-tertiary);
	}

	.filter-pill:hover .pill-remove {
		color: var(--cui-danger);
	}

	@media (max-width:720px) {
		.filter-bar {
			align-items: flex-start;
			flex-wrap: wrap;
			padding: 6px 10px;
		}

		.filter-summary {
			display: flex;
			order: -1;
		}

		.filters {
			display: none;
			flex-basis: 100%;
			grid-template-columns: 1fr;
		}

		.filters.filters-open {
			display: grid;
		}

		.filter-actions:not(.filter-summary) {
			margin-left: auto;
		}

		.filter-pills {
			display: flex;
			flex-basis: 100%;
			flex-wrap: wrap;
			gap: .4rem;
		}

		.filter-pill {
			min-height: 44px;
		}

		.filter-input,
		.filter-select {
			min-height: 44px;
			height: 44px;
		}
	}

	@media (prefers-reduced-motion:reduce) {
		.filter-pill {
			transition: none;
		}
	}
</style>
