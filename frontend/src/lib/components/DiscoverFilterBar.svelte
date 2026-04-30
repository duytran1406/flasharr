<script lang="ts">
  /**
   * DiscoverFilterBar — inline horizontal filter bar for the Flasharr Discover page.
   * Adapted from the Jellyflix discover filter bar (jellyflix/src/routes/(app)/discover/+page.svelte).
   *
   * Owns NO data-fetching. All state is passed via $bindable props so the parent
   * (discover page) stays in full control of fetch logic.
   */

  import { X, Palette, Star, Search, ChevronDown, SlidersHorizontal } from "lucide-svelte";

  // ── Unified genre map (same TMDB IDs as Jellyflix, adapted for Flasharr's
  //    movie/tv split — Flasharr never requests "all" simultaneously) ─────────
  export interface UnifiedGenre {
    name: string;
    movieId: number | null;
    tvId: number | null;
  }

  export const UNIFIED_GENRES: UnifiedGenre[] = [
    { name: "Action",      movieId: 28,    tvId: 10759 },
    { name: "Adventure",   movieId: 12,    tvId: 10759 },
    { name: "Animation",   movieId: 16,    tvId: 16    },
    { name: "Comedy",      movieId: 35,    tvId: 35    },
    { name: "Crime",       movieId: 80,    tvId: 80    },
    { name: "Documentary", movieId: 99,    tvId: 99    },
    { name: "Drama",       movieId: 18,    tvId: 18    },
    { name: "Family",      movieId: 10751, tvId: 10751 },
    { name: "Fantasy",     movieId: 14,    tvId: 10765 },
    { name: "History",     movieId: 36,    tvId: null  },
    { name: "Horror",      movieId: 27,    tvId: null  },
    { name: "Kids",        movieId: null,  tvId: 10762 },
    { name: "Music",       movieId: 10402, tvId: null  },
    { name: "Mystery",     movieId: 9648,  tvId: 9648  },
    { name: "Romance",     movieId: 10749, tvId: null  },
    { name: "Sci-Fi",      movieId: 878,   tvId: 10765 },
    { name: "Thriller",    movieId: 53,    tvId: null  },
    { name: "War",         movieId: 10752, tvId: 10768 },
    { name: "Western",     movieId: 37,    tvId: 37    },
  ];

  function getGenreTint(name: string): string {
    const map: Record<string, string> = {
      Action:      "#ef4444",
      Adventure:   "#00ffa3",
      Animation:   "#00f3ff",
      Comedy:      "#ffb700",
      Crime:       "#64748b",
      Documentary: "#78716c",
      Drama:       "#8b5cf6",
      Family:      "#38bdf8",
      Fantasy:     "#c084fc",
      History:     "#d97706",
      Horror:      "#374151",
      Kids:        "#fb923c",
      Music:       "#ec4899",
      Mystery:     "#6366f1",
      Romance:     "#f43f5e",
      "Sci-Fi":    "#0ea5e9",
      Thriller:    "#7000ff",
      War:         "#57534e",
      Western:     "#b45309",
    };
    return map[name] ?? "#334155";
  }

  // ── Props — all $bindable so parent can react to filter changes ───────────
  interface Props {
    mediaType: "movie" | "tv";
    sortBy: string;
    searchQuery: string;
    selectedGenres: UnifiedGenre[];
    fromYear: string;
    toYear: string;
    minRating: number;
    maxRating: number;
    resolvedGenreIds?: number[];
    hasFilter?: boolean;
    onMediaTypeChange: (t: "movie" | "tv") => void;
    onSortChange: (v: string) => void;
    onSearchInput: () => void;
    onGenreToggle: (g: UnifiedGenre) => void;
    onYearChange: () => void;
    onRatingChange: () => void;
    onClearAll: () => void;
  }

  let {
    mediaType = $bindable(),
    sortBy = $bindable(),
    searchQuery = $bindable(),
    selectedGenres = $bindable(),
    fromYear = $bindable(),
    toYear = $bindable(),
    minRating = $bindable(),
    maxRating = $bindable(),
    resolvedGenreIds = $bindable([] as number[]),
    hasFilter = $bindable(false),
    onMediaTypeChange,
    onSortChange,
    onSearchInput,
    onGenreToggle,
    onYearChange,
    onRatingChange,
    onClearAll,
  }: Props = $props();

  const sortOptions = [
    { value: "popular_today",     label: "Popular Today"         },
    { value: "popularity.desc",   label: "Popularity ↓"          },
    { value: "popularity.asc",    label: "Popularity ↑"          },
    { value: "release_date.desc", label: "Release Date ↓"        },
    { value: "release_date.asc",  label: "Release Date ↑"        },
    { value: "vote_average.desc", label: "Rating ↓"              },
    { value: "vote_average.asc",  label: "Rating ↑"              },
    { value: "title.asc",         label: "Title A→Z"             },
    { value: "title.desc",        label: "Title Z→A"             },
  ];

  const currentYear = new Date().getFullYear();

  // Genre dropdown open/close
  let genreDropdownOpen = $state(false);
  let genreDropdownRef = $state<HTMLDivElement | null>(null);

  // Rating panel open/close
  let ratingPanelOpen = $state(false);
  let ratingPanelRef = $state<HTMLDivElement | null>(null);

  // Sync computed values to bindable props so parent can read them
  $effect(() => {
    hasFilter =
      selectedGenres.length > 0 ||
      fromYear !== "" ||
      toYear !== "" ||
      minRating > 0 ||
      maxRating < 10 ||
      searchQuery.trim() !== "";
  });

  $effect(() => {
    resolvedGenreIds = selectedGenres
      .map((g) => (mediaType === "movie" ? g.movieId : g.tvId))
      .filter((id): id is number => id !== null);
  });

  // ── Outside-click handler ────────────────────────────────────────────────
  function onDocClick(e: MouseEvent) {
    if (genreDropdownRef && !genreDropdownRef.contains(e.target as Node)) {
      genreDropdownOpen = false;
    }
    if (ratingPanelRef && !ratingPanelRef.contains(e.target as Node)) {
      ratingPanelOpen = false;
    }
  }

  // Attach/detach document listener via $effect
  $effect(() => {
    document.addEventListener("click", onDocClick);
    return () => document.removeEventListener("click", onDocClick);
  });

  function isGenreSelected(g: UnifiedGenre): boolean {
    return selectedGenres.some((sg) => sg.name === g.name);
  }

  // Genres filtered by current media type availability
  const availableGenres = $derived(
    UNIFIED_GENRES.filter((g) =>
      mediaType === "movie" ? g.movieId !== null : g.tvId !== null
    )
  );

  // Label for rating button
  const ratingLabel = $derived(
    minRating > 0 || maxRating < 10
      ? `${minRating.toFixed(1)}–${maxRating.toFixed(1)}`
      : "Rating"
  );
  const ratingActive = $derived(minRating > 0 || maxRating < 10);
</script>

<!-- ═══════════════════════════════════════════════════════════════
     FILTER BAR
     ═══════════════════════════════════════════════════════════════ -->
<div class="filter-bar-wrap">
  <div class="filter-bar">

    <!-- MEDIA TYPE TOGGLE -->
    <div class="filter-group">
      <div class="media-toggle">
        {#each (["movie", "tv"] as const) as m}
          <button
            class="toggle-btn {mediaType === m ? 'active' : ''}"
            onclick={() => onMediaTypeChange(m)}
          >
            {m === "movie" ? "Movies" : "TV"}
          </button>
        {/each}
      </div>
    </div>

    <!-- GENRE DROPDOWN -->
    <div class="filter-group" bind:this={genreDropdownRef}>
      <button
        class="filter-btn {selectedGenres.length > 0 ? 'active' : ''}"
        onclick={(e) => { e.stopPropagation(); genreDropdownOpen = !genreDropdownOpen; }}
      >
        <Palette size={15} />
        <span>
          {#if selectedGenres.length === 0}
            Genre
          {:else if selectedGenres.length === 1}
            {selectedGenres[0].name}
          {:else}
            {selectedGenres.length} Genres
          {/if}
        </span>
        {#if selectedGenres.length > 0}
          <span
            role="button"
            tabindex="0"
            class="clear-x"
            onclick={(e) => { e.stopPropagation(); selectedGenres = []; onYearChange(); }}
            onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.stopPropagation(); selectedGenres = []; onYearChange(); } }}
          >
            <X size={13} />
          </span>
        {:else}
          <ChevronDown size={13} class="opacity-50" />
        {/if}
      </button>

      {#if genreDropdownOpen}
        <div class="genre-dropdown">
          <div class="genre-dropdown-grid">
            {#each availableGenres as g}
              {@const selected = isGenreSelected(g)}
              {@const tint = getGenreTint(g.name)}
              <button
                class="genre-opt {selected ? 'selected' : ''}"
                style="--tint: {tint}"
                onclick={(e) => { e.stopPropagation(); onGenreToggle(g); }}
              >
                {g.name}
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </div>

    <!-- YEAR RANGE -->
    <div class="filter-group">
      <div class="year-pair">
        <input
          id="filter-year-from"
          type="number"
          class="year-input {fromYear ? 'active' : ''}"
          placeholder="From"
          min="1900"
          max={currentYear}
          bind:value={fromYear}
          onchange={onYearChange}
        />
        <span class="year-sep">–</span>
        <input
          id="filter-year-to"
          type="number"
          class="year-input {toYear ? 'active' : ''}"
          placeholder="To"
          min="1900"
          max={currentYear + 2}
          bind:value={toYear}
          onchange={onYearChange}
        />
      </div>
    </div>

    <!-- RATING RANGE (flyout panel) -->
    <div class="filter-group" bind:this={ratingPanelRef}>
      <button
        class="filter-btn {ratingActive ? 'active' : ''}"
        onclick={(e) => { e.stopPropagation(); ratingPanelOpen = !ratingPanelOpen; }}
      >
        <Star size={15} color={ratingActive ? "#ffb700" : "currentColor"} />
        <span>{ratingLabel}</span>
        {#if ratingActive}
          <span
            role="button"
            tabindex="0"
            class="clear-x"
            onclick={(e) => {
              e.stopPropagation();
              minRating = 0;
              maxRating = 10;
              onRatingChange();
            }}
            onkeydown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.stopPropagation();
                minRating = 0;
                maxRating = 10;
                onRatingChange();
              }
            }}
          >
            <X size={13} />
          </span>
        {:else}
          <ChevronDown size={13} class="opacity-50" />
        {/if}
      </button>

      {#if ratingPanelOpen}
        <div class="rating-panel">
          <div class="rating-row-item">
            <span class="rating-row-label">Min: <strong>{minRating.toFixed(1)}</strong></span>
            <input
              id="filter-min-rating"
              type="range"
              class="score-slider"
              min="0"
              max="10"
              step="0.5"
              bind:value={minRating}
              style="--val: {minRating}"
              onchange={onRatingChange}
            />
          </div>
          <div class="rating-row-item">
            <span class="rating-row-label">Max: <strong>{maxRating.toFixed(1)}</strong></span>
            <input
              id="filter-max-rating"
              type="range"
              class="score-slider"
              min="0"
              max="10"
              step="0.5"
              bind:value={maxRating}
              style="--val: {maxRating}"
              onchange={onRatingChange}
            />
          </div>
        </div>
      {/if}
    </div>

    <!-- SORT -->
    <div class="filter-group">
      <div class="select-wrapper">
        <SlidersHorizontal size={15} class="select-icon" />
        <select
          class="sort-select"
          bind:value={sortBy}
          onchange={() => onSortChange(sortBy)}
        >
          {#each sortOptions as opt}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
      </div>
    </div>

    <!-- SEARCH -->
    <div class="filter-group search-group">
      <Search size={15} class="opacity-40 shrink-0" />
      <input
        id="filter-search"
        type="text"
        class="search-input"
        placeholder="Search title…"
        bind:value={searchQuery}
        oninput={onSearchInput}
      />
      {#if searchQuery}
        <button
          class="clear-x"
          onclick={() => { searchQuery = ""; onSearchInput(); }}
        >
          <X size={13} />
        </button>
      {/if}
    </div>

    <!-- CLEAR ALL -->
    {#if hasFilter}
      <button class="clear-all-btn" onclick={onClearAll}>
        <X size={14} />
        Clear
      </button>
    {/if}

  </div>

  <!-- SELECTED GENRE BADGES (shown when 2+ genres selected) -->
  {#if selectedGenres.length > 1}
    <div class="genre-badges">
      {#each selectedGenres as g}
        {@const tint = getGenreTint(g.name)}
        <div class="picked-badge" style="--tint: {tint}">
          <span>{g.name}</span>
          <button
            class="badge-x"
            onclick={() => onGenreToggle(g)}
          >
            <X size={11} />
          </button>
        </div>
      {/each}
      <button
        class="clear-genres-btn"
        onclick={() => { selectedGenres = []; onYearChange(); }}
      >
        Clear genres
      </button>
    </div>
  {/if}
</div>

<style>
  /* ── Wrapper ────────────────────────────────────────────────────────── */
  .filter-bar-wrap {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    flex-shrink: 0;
    z-index: 100;
  }

  /* ── Bar ────────────────────────────────────────────────────────────── */
  .filter-bar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.7rem;
    padding: 0.75rem 1.25rem;
    background: linear-gradient(
      135deg,
      rgba(18, 20, 28, 0.8) 0%,
      rgba(8, 10, 15, 0.9) 100%
    );
    backdrop-filter: blur(30px) saturate(180%);
    -webkit-backdrop-filter: blur(30px) saturate(180%);
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    border-top: 1px solid rgba(255, 255, 255, 0.04);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4),
                inset 0 1px 0 rgba(255, 255, 255, 0.04);
  }

  /* ── Group ──────────────────────────────────────────────────────────── */
  .filter-group {
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  /* ── Media type toggle ──────────────────────────────────────────────── */
  .media-toggle {
    display: flex;
    gap: 0.15rem;
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 10px;
    padding: 0.2rem;
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.5);
  }

  .toggle-btn {
    padding: 0.45rem 1rem;
    border-radius: 8px;
    font-size: 0.78rem;
    font-weight: 800;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.38);
    background: transparent;
    border: none;
    cursor: pointer;
    transition: all 220ms cubic-bezier(0.2, 0.8, 0.2, 1);
    font-family: var(--font-mono, monospace);
  }
  .toggle-btn:hover:not(.active) {
    color: rgba(255, 255, 255, 0.75);
  }
  .toggle-btn.active {
    background: rgba(0, 243, 255, 0.14);
    color: var(--color-primary, #00f3ff);
    box-shadow: 0 0 12px rgba(0, 243, 255, 0.18),
                inset 0 1px 0 rgba(0, 243, 255, 0.15);
    text-shadow: 0 0 8px rgba(0, 243, 255, 0.5);
  }

  /* ── Generic filter button (Genre, Rating) ──────────────────────────── */
  .filter-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.55rem 0.9rem;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    color: rgba(255, 255, 255, 0.7);
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 220ms cubic-bezier(0.2, 0.8, 0.2, 1);
    white-space: nowrap;
    font-family: var(--font-body, "Inter", sans-serif);
  }
  .filter-btn:hover {
    background: rgba(255, 255, 255, 0.07);
    color: #fff;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.25);
  }
  .filter-btn.active {
    background: linear-gradient(
      to bottom,
      rgba(0, 243, 255, 0.14),
      rgba(0, 243, 255, 0.06)
    );
    border-color: rgba(0, 243, 255, 0.3);
    color: var(--color-primary, #00f3ff);
    box-shadow: inset 0 1px 0 rgba(0, 243, 255, 0.18),
                0 0 10px rgba(0, 243, 255, 0.12);
    text-shadow: 0 0 10px rgba(0, 243, 255, 0.4);
  }

  /* ── Genre dropdown ─────────────────────────────────────────────────── */
  .genre-dropdown {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    z-index: 300;
    background: rgba(6, 8, 14, 0.97);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    padding: 0.75rem;
    width: 320px;
    max-width: 90vw;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.85),
                0 0 0 1px rgba(0, 243, 255, 0.06);
    backdrop-filter: blur(20px);
  }

  .genre-dropdown-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.3rem;
  }

  .genre-opt {
    padding: 0.38rem 0.5rem;
    border-radius: 7px;
    font-size: 0.7rem;
    font-weight: 700;
    text-align: center;
    cursor: pointer;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.6);
    transition: all 140ms ease;
    letter-spacing: 0.02em;
    font-family: var(--font-body, "Inter", sans-serif);
  }
  .genre-opt:hover {
    background: color-mix(in srgb, var(--tint) 18%, rgba(255, 255, 255, 0.04));
    border-color: color-mix(in srgb, var(--tint) 50%, transparent);
    color: #fff;
  }
  .genre-opt.selected {
    background: color-mix(in srgb, var(--tint) 22%, rgba(0, 0, 0, 0.3));
    border-color: var(--tint);
    color: #fff;
    box-shadow: 0 0 10px color-mix(in srgb, var(--tint) 35%, transparent);
  }

  /* ── Year inputs ────────────────────────────────────────────────────── */
  .year-pair {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }
  .year-input {
    width: 72px;
    padding: 0.52rem 0.65rem;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 9px;
    color: rgba(255, 255, 255, 0.85);
    font-size: 0.85rem;
    font-family: var(--font-mono, monospace);
    text-align: center;
    transition: all 180ms;
    -moz-appearance: textfield;
  }
  .year-input::-webkit-outer-spin-button,
  .year-input::-webkit-inner-spin-button {
    -webkit-appearance: none;
  }
  .year-input:focus,
  .year-input.active {
    outline: none;
    border-color: rgba(0, 243, 255, 0.45);
    background: rgba(0, 0, 0, 0.4);
    box-shadow: 0 0 0 2px rgba(0, 243, 255, 0.12);
  }
  .year-sep {
    color: rgba(255, 255, 255, 0.25);
    font-size: 0.9rem;
    pointer-events: none;
  }

  /* ── Rating panel ───────────────────────────────────────────────────── */
  .rating-panel {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    z-index: 300;
    background: rgba(6, 8, 14, 0.97);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    padding: 1rem 1.25rem;
    width: 240px;
    box-shadow: 0 24px 50px rgba(0, 0, 0, 0.8),
                0 0 0 1px rgba(0, 243, 255, 0.06);
    backdrop-filter: blur(20px);
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .rating-row-item {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .rating-row-label {
    font-size: 0.72rem;
    font-family: var(--font-mono, monospace);
    color: rgba(255, 255, 255, 0.45);
    letter-spacing: 0.05em;
  }

  .rating-row-label strong {
    color: #ffb700;
  }

  .score-slider {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 5px;
    border-radius: 3px;
    cursor: pointer;
    outline: none;
    background: linear-gradient(
      to right,
      #ffb700 0%,
      #ffb700 calc(var(--val, 0) * 10%),
      rgba(255, 255, 255, 0.1) calc(var(--val, 0) * 10%)
    );
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.5);
  }
  .score-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 17px;
    height: 17px;
    border-radius: 50%;
    background: #ffb700;
    box-shadow: 0 0 6px rgba(255, 183, 0, 0.5),
                inset 0 1px 1px rgba(255, 255, 255, 0.5);
    cursor: pointer;
    transition: transform 140ms, box-shadow 140ms;
  }
  .score-slider::-webkit-slider-thumb:hover {
    transform: scale(1.2);
    box-shadow: 0 0 14px rgba(255, 183, 0, 0.75),
                inset 0 1px 1px rgba(255, 255, 255, 0.8);
  }

  /* ── Sort select ────────────────────────────────────────────────────── */
  .select-wrapper {
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  :global(.select-wrapper .select-icon) {
    color: rgba(255, 255, 255, 0.4);
    pointer-events: none;
    flex-shrink: 0;
  }

  .sort-select {
    padding: 0.52rem 2rem 0.52rem 0.75rem;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    color: rgba(255, 255, 255, 0.75);
    font-size: 0.82rem;
    font-weight: 600;
    font-family: var(--font-mono, monospace);
    cursor: pointer;
    appearance: none;
    transition: all 200ms;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='rgba(255,255,255,0.4)' stroke-width='2'%3E%3Cpolyline points='6 9 12 15 18 9'%3E%3C/polyline%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 0.6rem center;
  }
  .sort-select:focus {
    outline: none;
    border-color: rgba(0, 243, 255, 0.4);
    box-shadow: 0 0 0 2px rgba(0, 243, 255, 0.1);
    color: #fff;
  }
  .sort-select option {
    background: #0a0c14;
    color: #fff;
  }

  /* ── Search ─────────────────────────────────────────────────────────── */
  .search-group {
    flex: 1;
    min-width: 160px;
    max-width: 300px;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    padding: 0.52rem 0.9rem;
    transition: all 220ms;
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.15);
  }
  .search-group:focus-within {
    background: rgba(0, 0, 0, 0.4);
    border-color: rgba(0, 243, 255, 0.45);
    box-shadow: 0 0 0 2px rgba(0, 243, 255, 0.12),
                inset 0 2px 4px rgba(0, 0, 0, 0.25);
  }

  .search-input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: rgba(255, 255, 255, 0.9);
    font-size: 0.88rem;
    font-family: var(--font-body, "Inter", sans-serif);
    min-width: 0;
    width: 100%;
  }
  .search-input::placeholder {
    color: rgba(255, 255, 255, 0.25);
  }

  /* ── Clear X ────────────────────────────────────────────────────────── */
  .clear-x {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 2px;
    border-radius: 4px;
    color: rgba(255, 255, 255, 0.38);
    background: rgba(255, 255, 255, 0.05);
    border: none;
    cursor: pointer;
    transition: color 140ms, background 140ms;
  }
  .clear-x:hover {
    color: #fff;
    background: rgba(255, 255, 255, 0.15);
  }

  /* ── Clear all button ───────────────────────────────────────────────── */
  .clear-all-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.52rem 0.9rem;
    background: rgba(239, 68, 68, 0.05);
    border: 1px solid rgba(239, 68, 68, 0.2);
    border-radius: 10px;
    color: rgba(255, 90, 90, 0.8);
    font-size: 0.8rem;
    font-weight: 700;
    cursor: pointer;
    transition: all 220ms cubic-bezier(0.2, 0.8, 0.2, 1);
    white-space: nowrap;
    font-family: var(--font-body, "Inter", sans-serif);
  }
  .clear-all-btn:hover {
    background: rgba(239, 68, 68, 0.14);
    border-color: rgba(239, 68, 68, 0.4);
    color: #fff;
    box-shadow: 0 4px 12px rgba(239, 68, 68, 0.15);
    transform: translateY(-1px);
  }

  /* ── Genre badges row ───────────────────────────────────────────────── */
  .genre-badges {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    padding: 0 1.25rem 0.6rem;
  }

  .picked-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.28rem 0.6rem;
    background: color-mix(in srgb, var(--tint) 15%, rgba(0, 0, 0, 0.4));
    border: 1px solid color-mix(in srgb, var(--tint) 40%, transparent);
    border-radius: 20px;
    color: #fff;
    font-size: 0.72rem;
    font-weight: 600;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
    animation: badgeIn 260ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }

  @keyframes badgeIn {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.95);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .badge-x {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: rgba(255, 255, 255, 0.55);
    cursor: pointer;
    transition: all 130ms;
    border-radius: 50%;
    padding: 1px;
    background: transparent;
    border: none;
  }
  .badge-x:hover {
    background: rgba(255, 255, 255, 0.18);
    color: #fff;
  }

  .clear-genres-btn {
    font-size: 0.72rem;
    font-weight: 700;
    color: rgba(255, 90, 90, 0.7);
    background: none;
    border: none;
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    border-radius: 6px;
    transition: all 150ms;
  }
  .clear-genres-btn:hover {
    color: rgba(255, 90, 90, 1);
    background: rgba(239, 68, 68, 0.08);
  }

  /* ── Responsive ─────────────────────────────────────────────────────── */
  @media (max-width: 767px) {
    /* Single-row horizontal scrollable chip bar */
    .filter-bar {
      flex-wrap: nowrap;
      overflow-x: auto;
      overflow-y: hidden;
      -webkit-overflow-scrolling: touch;
      padding: 0.5rem 0.75rem;
      gap: 0.5rem;
      scrollbar-width: none; /* Firefox */
    }
    .filter-bar::-webkit-scrollbar {
      display: none; /* Chrome/Safari */
    }
    /* Prevent groups from shrinking — preserve their natural width */
    .filter-group {
      flex-shrink: 0;
    }
    /* Compact toggle buttons */
    .toggle-btn {
      padding: 0.38rem 0.65rem;
      font-size: 0.72rem;
    }
    /* Compact sort select */
    .sort-select {
      font-size: 0.75rem;
      padding: 0.38rem 1.6rem 0.38rem 0.6rem;
    }
    /* Make search narrower on mobile */
    .search-group {
      min-width: 100px;
      max-width: 160px;
    }
    /* Year inputs */
    .year-input {
      width: 58px;
      font-size: 0.78rem;
      padding: 0.38rem 0.45rem;
    }
    /* Genre badge row: also scrollable */
    .genre-badges {
      flex-wrap: nowrap;
      overflow-x: auto;
      -webkit-overflow-scrolling: touch;
      padding-bottom: 0.5rem;
      scrollbar-width: none;
    }
    .genre-badges::-webkit-scrollbar {
      display: none;
    }
  }
</style>
