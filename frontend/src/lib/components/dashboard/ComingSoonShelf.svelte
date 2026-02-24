<script lang="ts">
  import { onMount } from "svelte";
  import ContentShelf from "./ContentShelf.svelte";
  import { fetchCalendar, getSeriesPoster } from "$lib/stores/arr";

  type CalendarItem = {
    tmdbId: number;
    type: "tv";
    title: string;
    poster: string | null;
    year: number | null;
    status: "available" | "missing";
    airDate: string | undefined;
  };

  let items = $state<CalendarItem[]>([]);
  let loading = $state(true);

  onMount(async () => {
    try {
      const calendar = await fetchCalendar();

      if (!calendar || calendar.length === 0) {
        loading = false;
        return;
      }

      // Group by series and take upcoming items
      const seriesMap = new Map<number, CalendarItem>();

      for (const episode of calendar) {
        // Check if series exists and has tmdbId
        if (!episode.series || !episode.series.tmdbId) continue;

        const tmdbId = episode.series.tmdbId;
        if (!seriesMap.has(tmdbId)) {
          seriesMap.set(tmdbId, {
            tmdbId,
            type: "tv",
            title: episode.series.title || "Unknown Series",
            poster: getSeriesPoster(episode.series),
            year: episode.series.year || null,
            status: episode.hasFile ? "available" : "missing",
            airDate: episode.airDateUtc,
          });
        }
      }

      // Convert to array and sort by air date
      items = Array.from(seriesMap.values())
        .sort((a, b) => {
          const dateA = a.airDate ? new Date(a.airDate).getTime() : 0;
          const dateB = b.airDate ? new Date(b.airDate).getTime() : 0;
          return dateA - dateB;
        })
        .slice(0, 12);
    } catch (e) {
      console.error("Failed to load Coming Soon:", e);
    } finally {
      loading = false;
    }
  });
</script>

<ContentShelf title="Coming Soon" {items} {loading} />
