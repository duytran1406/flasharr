<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { animeFade } from "$lib/animations";
  import {
    fetchHistory,
    fetchAllSeries,
    fetchAllMovies,
    type SonarrSeries,
    type RadarrMovie,
  } from "$lib/stores/arr";
  import MediaCard from "../MediaCard.svelte";
  import { toasts } from "$lib/stores/toasts";

  interface Props {
    enabled?: boolean;
  }

  let { enabled = true }: Props = $props();

  interface RecentItem {
    id: string; // unique key
    raw: any;
    type: "movie" | "series";
  }

  let items = $state<RecentItem[]>([]);
  let loading = $state(true);

  $effect(() => {
    if (enabled) {
      untrack(async () => {
        try {
          const [history, series, movies] = await Promise.all([
            fetchHistory(100), // Fetch last 100 events
            fetchAllSeries(),
            fetchAllMovies(),
          ]);

          const mapped: RecentItem[] = [];
          const seenIds = new Set<string>();

          // API returns { sonarr: { records: [] }, radarr: { records: [] } }
          // Merge both into a single flat sorted list of import events
          const sonarrRecords: any[] = history?.sonarr?.records ?? [];
          const radarrRecords: any[] = history?.radarr?.records ?? [];
          const allRecords = [...sonarrRecords, ...radarrRecords]
            .filter((r: any) => r.eventType === "downloadFolderImported")
            .sort(
              (a: any, b: any) =>
                new Date(b.date).getTime() - new Date(a.date).getTime(),
            );

          for (const record of allRecords) {
            let item: RecentItem | null = null;

            if (record.seriesId) {
              if (seenIds.has(`tv-${record.seriesId}`)) continue;
              const s = series.find(
                (s: SonarrSeries) => s.id === record.seriesId,
              );
              if (s) {
                item = { id: `tv-${s.id}`, raw: s, type: "series" };
                seenIds.add(`tv-${record.seriesId}`);
              }
            } else if (record.movieId) {
              if (seenIds.has(`movie-${record.movieId}`)) continue;
              const m = movies.find(
                (m: RadarrMovie) => m.id === record.movieId,
              );
              if (m) {
                item = { id: `movie-${m.id}`, raw: m, type: "movie" };
                seenIds.add(`movie-${record.movieId}`);
              }
            }

            if (item) mapped.push(item);
            if (mapped.length >= 12) break;
          }

          // Fallback: if no import history, show newest library items directly
          if (mapped.length === 0 && (series.length > 0 || movies.length > 0)) {
            const fallback: RecentItem[] = [
              ...movies.slice(0, 6).map((m: RadarrMovie) => ({
                id: `movie-${m.id}`,
                raw: m,
                type: "movie" as const,
              })),
              ...series.slice(0, 6).map((s: SonarrSeries) => ({
                id: `tv-${s.id}`,
                raw: s,
                type: "series" as const,
              })),
            ];
            items = fallback;
          } else {
            items = mapped;
          }
        } catch (e: any) {
          console.error("Failed to load Recently Added:", e);
          toasts.error(`Recently Added Error: ${e.message}`);
        } finally {
          loading = false;
        }
      });
    } else {
      loading = false;
    }
  });
</script>

{#if !loading && items.length > 0}
  <div class="recently-added premium-card" in:animeFade>
    <div class="card-header-premium">
      <span class="material-icons">history</span>
      <span class="label-text">RECENTLY ADDED</span>
      <a href="/library" class="view-link-premium">
        <span class="material-icons">arrow_forward</span>
      </a>
    </div>

    <div class="scroller">
      {#each items as { raw, type, id } (id)}
        <MediaCard item={raw} {type} viewMode="grid" />
      {/each}
    </div>
  </div>
{/if}

<style>
  .recently-added {
    width: 100%;
    margin-top: 1.5rem;
    /* Reset premium-card padding for the scroller to bleed to edges */
    padding-left: 0 !important;
    padding-right: 0 !important;
    overflow: visible !important;
  }

  /* Keep header aligned with card content padding */
  :global(.recently-added .card-header-premium) {
    padding-left: 1.25rem !important;
    padding-right: 1.25rem !important;
    margin-bottom: 0.75rem;
  }

  .scroller {
    display: flex;
    gap: 0.875rem;
    overflow-x: auto;
    overflow-y: hidden;
    padding: 0.25rem 1.25rem 1rem 1.25rem;
    scroll-behavior: smooth;
    scrollbar-width: none;
  }

  .scroller::-webkit-scrollbar {
    display: none;
  }

  /* Each card slot: fixed width, no overflow needed (title is in scrim) */
  :global(.recently-added .scroller .media-card) {
    flex: 0 0 130px;
    width: 130px;
  }

  :global(.recently-added .scroller .poster-wrap) {
    height: 195px;
    width: 130px;
  }

  @media (max-width: 768px) {
    :global(.recently-added .scroller .media-card),
    :global(.recently-added .scroller .poster-wrap) {
      flex: 0 0 108px;
      width: 108px;
    }
    :global(.recently-added .scroller .poster-wrap) {
      height: 162px;
    }
  }
</style>
