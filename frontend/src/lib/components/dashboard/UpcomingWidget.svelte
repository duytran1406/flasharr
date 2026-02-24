<script lang="ts">
  import { onMount } from "svelte";
  import { fetchCalendar, type CalendarEntry } from "$lib/stores/arr";

  let entries = $state<CalendarEntry[]>([]);
  let loading = $state(true);

  onMount(async () => {
    try {
      // Fetch next 7 days
      const all = await fetchCalendar(7);
      // Filter only future/today items and limit to 5
      const now = new Date();
      now.setHours(0, 0, 0, 0);
      entries = all
        .filter((e) => e.airDateUtc && new Date(e.airDateUtc) >= now)
        .slice(0, 5);
    } catch (e) {
      console.error(e);
    } finally {
      loading = false;
    }
  });

  function formatDate(d?: string) {
    if (!d) return "TBA";
    return new Date(d).toLocaleDateString("en-US", {
      weekday: "short",
      day: "numeric",
    });
  }
</script>

<div class="upcoming-widget">
  {#if loading}
    <div class="loading">Loading Schedule...</div>
  {:else if entries.length === 0}
    <div class="empty">No upcoming episodes</div>
  {:else}
    <div class="entries-list">
      {#each entries as entry}
        <div class="entry-card">
          <div class="date-badge">
            {formatDate(entry.airDateUtc)}
          </div>
          <div class="entry-info">
            <div class="series-title">
              {entry.series?.title || "Unknown Series"}
            </div>
            <div class="ep-title">
              S{entry.seasonNumber}E{entry.episodeNumber} · {entry.title ||
                "TBA"}
            </div>
          </div>
          <div class="status-indicator" class:has-file={entry.hasFile}></div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .upcoming-widget {
    height: 100%;
    overflow-y: auto;
    /* Hide scrollbar for cleaner look */
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.1) transparent;
  }

  .loading,
  .empty {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .entries-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .entry-card {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.5rem;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.02);
    transition: background 0.2s;
  }

  .entry-card:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .date-badge {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 6px;
    font-size: 0.6rem;
    font-weight: 800;
    color: #fff;
    text-align: center;
    line-height: 1.2;
    flex-shrink: 0;
  }

  .entry-info {
    flex: 1;
    min-width: 0;
  }

  .series-title {
    font-size: 0.8rem;
    font-weight: 700;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ep-title {
    font-size: 0.65rem;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .status-indicator {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.1);
    flex-shrink: 0;
  }

  .status-indicator.has-file {
    background: #00ff80;
    box-shadow: 0 0 6px rgba(0, 255, 128, 0.4);
  }
</style>
