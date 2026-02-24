<script lang="ts">
  import { onMount } from "svelte";
  import ContentShelf from "./ContentShelf.svelte";
  import {
    getTrendingMovies,
    getPopularTVShows,
    getPosterUrl,
  } from "$lib/services/tmdb";

  let items = $state<any[]>([]);
  let loading = $state(true);

  onMount(async () => {
    try {
      // Get both trending movies and popular TV shows
      const [movies, shows] = await Promise.all([
        getTrendingMovies(),
        getPopularTVShows(),
      ]);

      // Mix them together and take top 12
      const mixed = [
        ...movies.slice(0, 6).map((m: any) => ({
          tmdbId: m.id,
          type: "movie",
          title: m.title,
          poster: getPosterUrl(m.poster_path),
          year: m.release_date ? new Date(m.release_date).getFullYear() : null,
          status: "missing", // Default to missing since we don't know if it's in library
        })),
        ...shows.slice(0, 6).map((s: any) => ({
          tmdbId: s.id,
          type: "tv",
          title: s.name,
          poster: getPosterUrl(s.poster_path),
          year: s.first_air_date
            ? new Date(s.first_air_date).getFullYear()
            : null,
          status: "missing",
        })),
      ];

      // Shuffle for variety
      items = mixed.sort(() => Math.random() - 0.5).slice(0, 12);
    } catch (e) {
      console.error("Failed to load Popular:", e);
    } finally {
      loading = false;
    }
  });
</script>

<ContentShelf title="Popular Now" {items} {loading} />
