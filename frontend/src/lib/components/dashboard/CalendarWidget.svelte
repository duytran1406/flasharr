<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { fetchCalendar, type CalendarEntry } from "$lib/stores/arr";
  import { toasts } from "$lib/stores/toasts";

  interface Props {
    compact?: boolean;
    enabled?: boolean;
  }

  let { compact = false, enabled = true }: Props = $props();

  let entries = $state<CalendarEntry[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  $effect(() => {
    if (enabled) {
      untrack(async () => {
        try {
          const calendar = await fetchCalendar(14);
          entries = calendar.sort(
            (a, b) =>
              new Date(a.airDateUtc || "").getTime() -
              new Date(b.airDateUtc || "").getTime(),
          );
        } catch (e: any) {
          console.error("Failed to load Calendar:", e);
          toasts.error(`Calendar Error: ${e.message}`);
        } finally {
          loading = false;
        }
      });
    } else {
      loading = false;
    }
  });

  // Group entries by date
  let groupedByDate = $derived(() => {
    const groups: Record<string, CalendarEntry[]> = {};
    for (const entry of entries) {
      const date = entry.airDateUtc
        ? new Date(entry.airDateUtc).toLocaleDateString("en-US", {
            weekday: "short",
            month: "short",
            day: "numeric",
          })
        : "Unknown";
      if (!groups[date]) groups[date] = [];
      groups[date].push(entry);
    }
    return groups;
  });

  function formatEpCode(s: number, e: number): string {
    return `S${String(s).padStart(2, "0")}E${String(e).padStart(2, "0")}`;
  }

  function isToday(dateStr?: string): boolean {
    if (!dateStr) return false;
    const d = new Date(dateStr);
    const today = new Date();
    return d.toDateString() === today.toDateString();
  }

  function isTomorrow(dateStr?: string): boolean {
    if (!dateStr) return false;
    const d = new Date(dateStr);
    const tomorrow = new Date();
    tomorrow.setDate(tomorrow.getDate() + 1);
    return d.toDateString() === tomorrow.toDateString();
  }

  function relativeLabel(dateStr: string): string {
    // Extract the first entry date for comparison
    const firstEntry = entries.find((e) => {
      const d = e.airDateUtc
        ? new Date(e.airDateUtc).toLocaleDateString("en-US", {
            weekday: "short",
            month: "short",
            day: "numeric",
          })
        : "";
      return d === dateStr;
    });
    if (firstEntry?.airDateUtc) {
      if (isToday(firstEntry.airDateUtc)) return "⚡ TODAY";
      if (isTomorrow(firstEntry.airDateUtc)) return "→ TOMORROW";
    }
    return dateStr.toUpperCase();
  }
</script>

<div class="calendar-widget">
  {#if loading}
    <div class="calendar-loading">
      <div class="pulse-ring"></div>
      <span>SCANNING SCHEDULE...</span>
    </div>
  {:else if entries.length === 0 && !enabled}
    <div class="calendar-empty">
      <span class="material-icons">link_off</span>
      <p>SONARR NOT CONNECTED</p>
    </div>
  {:else if entries.length === 0}
    <div class="calendar-empty">
      <span class="material-icons">event_available</span>
      <p>NO UPCOMING EPISODES</p>
    </div>
  {:else}
    <div class="calendar-timeline">
      {#each Object.entries(groupedByDate()) as [date, items]}
        <div class="timeline-group">
          <div
            class="timeline-date"
            class:today={items[0]?.airDateUtc && isToday(items[0].airDateUtc)}
          >
            <div class="date-marker"></div>
            <span>{relativeLabel(date)}</span>
          </div>
          {#each items as entry}
            <div class="timeline-entry" class:has-file={entry.hasFile}>
              <div class="entry-accent"></div>
              <div class="entry-body">
                <div class="entry-series">
                  {entry.series?.title || "Unknown Series"}
                </div>
                <div class="entry-episode">
                  <span class="ep-code"
                    >{formatEpCode(
                      entry.seasonNumber,
                      entry.episodeNumber,
                    )}</span
                  >
                  <span class="ep-title">{entry.title || ""}</span>
                </div>
              </div>
              <div class="entry-status">
                {#if entry.hasFile}
                  <span class="material-icons status-icon acquired"
                    >check_circle</span
                  >
                {:else}
                  <span class="material-icons status-icon pending"
                    >schedule</span
                  >
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .calendar-widget {
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .calendar-loading,
  .calendar-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    padding: 2rem;
    color: var(--text-muted);
    flex: 1;
  }

  .calendar-empty .material-icons {
    font-size: 36px;
    opacity: 0.25;
  }

  .calendar-empty p,
  .calendar-loading span {
    font-size: 0.6rem;
    font-weight: 800;
    letter-spacing: 0.15em;
    opacity: 0.5;
  }

  .pulse-ring {
    width: 24px;
    height: 24px;
    border: 2px solid rgba(0, 243, 255, 0.3);
    border-top-color: var(--color-primary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .calendar-timeline {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .timeline-group {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .timeline-date {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.55rem;
    font-weight: 900;
    letter-spacing: 0.15em;
    color: var(--text-muted);
    padding: 4px 0;
    margin-top: 4px;
  }

  .timeline-date.today {
    color: var(--color-primary);
  }

  .date-marker {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.15);
    flex-shrink: 0;
  }

  .timeline-date.today .date-marker {
    background: var(--color-primary);
    box-shadow: 0 0 8px var(--color-primary);
  }

  .timeline-entry {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.75rem;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.03);
    border-radius: 3px;
    margin-left: 12px;
    transition: all 0.2s;
  }

  .timeline-entry:hover {
    background: rgba(0, 243, 255, 0.03);
    border-color: rgba(0, 243, 255, 0.15);
  }

  .timeline-entry.has-file {
    border-left: 2px solid rgba(0, 255, 128, 0.4);
  }

  .entry-accent {
    width: 2px;
    height: 24px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 2px;
    flex-shrink: 0;
  }

  .timeline-entry.has-file .entry-accent {
    background: rgba(0, 255, 128, 0.4);
  }

  .entry-body {
    flex: 1;
    min-width: 0;
  }

  .entry-series {
    font-size: 0.7rem;
    font-weight: 700;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .entry-episode {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    margin-top: 2px;
  }

  .ep-code {
    font-family: var(--font-mono, monospace);
    font-size: 0.6rem;
    font-weight: 800;
    color: var(--color-primary);
    opacity: 0.8;
  }

  .ep-title {
    font-size: 0.6rem;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .entry-status {
    flex-shrink: 0;
  }

  .status-icon {
    font-size: 16px;
  }

  .status-icon.acquired {
    color: #00ff80;
    opacity: 0.7;
  }

  .status-icon.pending {
    color: var(--text-muted);
    opacity: 0.4;
  }

  /* Scrollbar */
  .calendar-widget::-webkit-scrollbar {
    width: 3px;
  }
  .calendar-widget::-webkit-scrollbar-track {
    background: transparent;
  }
  .calendar-widget::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 2px;
  }
</style>
