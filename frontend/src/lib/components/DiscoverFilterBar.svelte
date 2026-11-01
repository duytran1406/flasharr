<script lang="ts">
  /**
   * DiscoverFilterBar — inline horizontal filter bar for the Flasharr Discover page.
   * Adapted from the Jellyflix discover filter bar (jellyflix/src/routes/(app)/discover/+page.svelte).
   *
   * Owns NO data-fetching. All state is passed via $bindable props so the parent
   * (discover page) stays in full control of fetch logic.
   *
   * On phones (max-width: 767px) the genre picker opens as a bottom sheet that is
   * portalled to <body> (dialog semantics + focus trap) instead of the inline dropdown.
   */

  import { X, Palette, Star, Search, ChevronDown, SlidersHorizontal } from "lucide-svelte";

  // ── Unified genre map (same TMDB IDs as Jellyflix, adapted for Flasharr's
  //    movie/tv split — Flasharr never requests "all" simultaneously) ─────────
  export interface UnifiedGenre {
    name: string;
    movieId: number | null;
    tvId: number | null;
  }

  export type DiscoverMediaType = "movie" | "tv" | "collections";

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
    mediaType: DiscoverMediaType;
    sortBy: string;
    searchQuery: string;
    selectedGenres: UnifiedGenre[];
    fromYear: string;
    toYear: string;
    minRating: number;
    maxRating: number;
    resolvedGenreIds?: number[];
    hasFilter?: boolean;
    /** Adds a third "Collection" tab to the media toggle. */
    includeCollectionTab?: boolean;
    /** Hides every filter except the media toggle while the collection tab is active. */
    hideNonCollectionFilters?: boolean;
    onMediaTypeChange: (t: DiscoverMediaType) => void;
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
    includeCollectionTab = false,
    hideNonCollectionFilters = false,
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

  // Genre dropdown (desktop) / genre sheet (mobile) open/close
  let genreDropdownOpen = $state(false);
  let genreDropdownRef = $state<HTMLDivElement | null>(null);
  let genreButtonRef = $state<HTMLButtonElement | null>(null);
  let genreSheetRef = $state<HTMLDivElement | null>(null);
  let isMobileSheet = $state(false);

  /** Moves the node to <body> so the sheet escapes the filter bar's stacking context. */
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  function closeGenreSheet(restoreFocus = true) {
    genreDropdownOpen = false;
    if (restoreFocus) queueMicrotask(() => genreButtonRef?.focus());
  }

  // Keep keyboard focus inside the genre sheet while it is open
  function trapGenreSheetFocus(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      closeGenreSheet();
      return;
    }
    if (e.key !== "Tab" || !genreSheetRef) return;
    const focusable = [
      ...genreSheetRef.querySelectorAll<HTMLElement>(
        'button:not([disabled]),[href],input,select,[tabindex]:not([tabindex="-1"])',
      ),
    ];
    if (!focusable.length) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }

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
    const target = e.target as Node;
    if (
      genreDropdownRef &&
      !genreDropdownRef.contains(target) &&
      !genreSheetRef?.contains(target)
    ) {
      genreDropdownOpen = false;
    }
    if (ratingPanelRef && !ratingPanelRef.contains(target)) {
      ratingPanelOpen = false;
    }
  }

  // Attach/detach document listener via $effect
  $effect(() => {
    document.addEventListener("click", onDocClick);
    return () => document.removeEventListener("click", onDocClick);
  });

  // Phones get the portalled genre sheet instead of the inline dropdown
  $effect(() => {
    const query = matchMedia("(max-width: 767px)");
    const update = () => (isMobileSheet = query.matches);
    update();
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  });

  // Move focus into the sheet when it opens
  $effect(() => {
    if (genreDropdownOpen && isMobileSheet) {
      queueMicrotask(() => genreSheetRef?.querySelector("button")?.focus());
    }
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
        {#each (includeCollectionTab
          ? (["movie", "tv", "collections"] as const)
          : (["movie", "tv"] as const)) as m}
          <button
            class="toggle-btn {mediaType === m ? 'active' : ''}"
            onclick={() => onMediaTypeChange(m)}
          >
            {m === "movie" ? "Movies" : m === "tv" ? "TV" : "Collection"}
          </button>
        {/each}
      </div>
    </div>

    {#if !hideNonCollectionFilters || mediaType !== "collections"}
      <!-- GENRE DROPDOWN -->
      <div class="filter-group" bind:this={genreDropdownRef}>
        <button
          bind:this={genreButtonRef}
          class="filter-btn {selectedGenres.length > 0 ? 'active' : ''}"
          aria-haspopup={isMobileSheet ? "dialog" : "menu"}
          aria-expanded={genreDropdownOpen}
          onclick={(e) => { e.stopPropagation(); genreDropdownOpen = !genreDropdownOpen; }}
        >
          <Palette size={15} />
          <span
            >{selectedGenres.length === 0
              ? "Genre"
              : selectedGenres.length === 1
                ? selectedGenres[0].name
                : `${selectedGenres.length} Genres`}</span
          >
          <ChevronDown size={13} class="opacity-50" />
        </button>
        {#if selectedGenres.length > 0}
          <button
            class="clear-x"
            aria-label="Clear genres"
            onclick={(e) => { e.stopPropagation(); selectedGenres = []; onYearChange(); }}
          >
            <X size={13} />
          </button>
        {/if}

        {#if genreDropdownOpen && !isMobileSheet}
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
          <ChevronDown size={13} class="opacity-50" />
        </button>
        {#if ratingActive}
          <button
            class="clear-x"
            aria-label="Clear rating"
            onclick={(e) => {
              e.stopPropagation();
              minRating = 0;
              maxRating = 10;
              onRatingChange();
            }}
          >
            <X size={13} />
          </button>
        {/if}

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
          aria-label="Search titles"
          placeholder="Search title…"
          bind:value={searchQuery}
          oninput={onSearchInput}
        />
        {#if searchQuery}
          <button
            class="clear-x"
            aria-label="Clear title search"
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

<!-- MOBILE GENRE SHEET (portalled to <body>) -->
{#if genreDropdownOpen && isMobileSheet}
  <div
    class="genre-portal"
    role="presentation"
    use:portal
    onclick={(e) => { if (e.target === e.currentTarget) closeGenreSheet(); }}
    onkeydown={(e) => { if (e.key === "Escape") closeGenreSheet(); }}
  >
    <div
      class="genre-sheet"
      bind:this={genreSheetRef}
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="genre-sheet-title"
      onkeydown={trapGenreSheetFocus}
    >
      <header
        ><h2 id="genre-sheet-title">Choose genres</h2><button
          class="genre-close"
          aria-label="Close genre dialog"
          onclick={() => closeGenreSheet()}><X size={20} /></button
        ></header
      >
      <div class="genre-sheet-scroll">
        <div class="genre-dropdown-grid">
          {#each availableGenres as g}
            {@const selected = isGenreSelected(g)}
            {@const tint = getGenreTint(g.name)}
            <button
              class="genre-opt {selected ? 'selected' : ''}"
              style="--tint: {tint}"
              aria-pressed={selected}
              onclick={() => onGenreToggle(g)}
            >
              {g.name}
            </button>
          {/each}
        </div>
      </div>
    </div>
  </div>
{/if}

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
    transition:
      background var(--cui-duration-fast) var(--cui-ease-standard),
      border-color var(--cui-duration-fast) var(--cui-ease-standard),
      box-shadow var(--cui-duration-fast) var(--cui-ease-standard);
    appearance: textfield;
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
  /* ── Mobile genre sheet (portalled to <body>) ── */
  .genre-portal {
    position: fixed;
    inset: 0;
    z-index: 2147483640;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    padding: max(0.75rem, env(safe-area-inset-top)) 0.75rem
      calc(
        var(--cui-mobile-bottom-nav-height, 60px) +
          max(0.75rem, env(safe-area-inset-bottom))
      );
    background: rgba(0, 0, 0, 0.68);
  }

  .genre-sheet {
    display: flex;
    flex-direction: column;
    width: min(100%, 520px);
    max-height: min(76dvh, 620px);
    overflow: hidden;
    background: #080b12;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 16px;
    box-shadow: 0 24px 70px #000;
  }

  .genre-sheet header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex: 0 0 auto;
    padding: 0.65rem 0.75rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  }

  .genre-sheet h2 {
    margin: 0;
    font-size: 1rem;
  }

  .genre-close {
    display: grid;
    place-items: center;
    min-width: 44px;
    min-height: 44px;
    border: 0;
    border-radius: 9px;
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
  }

  .genre-sheet-scroll {
    overflow-y: auto;
    overscroll-behavior: contain;
    touch-action: pan-y;
    padding: 0.75rem;
  }

  /* ── Mobile ── */
  @media (max-width: 767px) {
    .filter-bar-wrap {
      position: sticky;
      top: 0;
      z-index: 2;
      isolation: isolate;
      gap: 0;
    }
    .filter-bar {
      flex-wrap: nowrap;
      overflow-x: auto;
      overflow-y: visible;
      -webkit-overflow-scrolling: touch;
      touch-action: pan-x;
      padding: 0.6rem 0.75rem;
      gap: 0.5rem;
      scrollbar-width: none;
      border-bottom: none;
      border-top: 1px solid rgba(255, 255, 255, 0.08);
      box-shadow:
        0 -8px 32px rgba(0, 0, 0, 0.5),
        inset 0 1px 0 rgba(255, 255, 255, 0.04);
    }
    .filter-bar::-webkit-scrollbar {
      display: none;
    }
    .filter-group {
      flex-shrink: 0;
    }
    .toggle-btn {
      padding: 0.4rem 0.7rem;
      font-size: 0.7rem;
    }
    .filter-btn {
      padding: 0.45rem 0.75rem;
      font-size: 0.75rem;
    }
    .sort-select {
      font-size: 0.75rem;
      padding: 0.4rem 1.6rem 0.4rem 0.6rem;
    }
    .search-group {
      min-width: 100px;
      max-width: 160px;
      padding: 0.45rem 0.75rem;
    }
    .year-input {
      width: 58px;
      font-size: 0.75rem;
      padding: 0.4rem 0.45rem;
    }
    .clear-all-btn {
      padding: 0.45rem 0.75rem;
      font-size: 0.75rem;
    }
    .genre-badges {
      display: none;
    }
    .genre-dropdown {
      position: fixed;
      left: 0.75rem;
      right: 0.75rem;
      width: auto;
      max-width: none;
      top: max(72px, env(safe-area-inset-top));
      bottom: calc(72px + env(safe-area-inset-bottom, 0px));
      max-height: none;
      overflow-y: auto;
      overscroll-behavior: contain;
      z-index: 1200;
      touch-action: pan-y;
    }
    .genre-dropdown-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 0.4rem;
    }
    .genre-opt,
    .filter-btn,
    .toggle-btn,
    .sort-select,
    .year-input,
    .clear-all-btn,
    .clear-x {
      min-height: 44px;
    }
    .rating-panel {
      position: fixed;
      left: 0.75rem;
      right: 0.75rem;
      width: auto;
      max-width: none;
      bottom: calc(72px + env(safe-area-inset-bottom, 0px));
      top: auto;
      z-index: 1200;
      touch-action: pan-y;
    }
  }
</style>
