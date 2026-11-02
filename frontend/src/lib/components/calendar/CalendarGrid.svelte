<script lang="ts">
  import type { CalendarEntry } from "$lib/stores/arr";
  import { Badge } from "@media-set/core-ui";

  interface Props {
    viewMode: "week" | "month";
    /** Entries keyed by local `YYYY-MM-DD`, one key per day in the visible range. */
    groupedByDate: Record<string, CalendarEntry[]>;
    formatDayOfWeek: (dateStr: string) => string;
    formatDayNum: (dateStr: string) => string;
    isToday: (dateStr: string) => boolean;
    isPast: (dateStr: string) => boolean;
    formatEpCode: (season: number, episode: number) => string;
  }

  let {
    viewMode,
    groupedByDate,
    formatDayOfWeek,
    formatDayNum,
    isToday,
    isPast,
    formatEpCode,
  }: Props = $props();
</script>

<div class="cal-grid" class:month-view={viewMode === "month"}>
  {#each Object.entries(groupedByDate) as [dateStr, dayEntries]}
    <div
      class="cal-day"
      class:today={isToday(dateStr)}
      class:past={isPast(dateStr)}
      class:has-episodes={dayEntries.length > 0}
    >
      <!-- Day Header -->
      <div class="day-header">
        <div class="day-num" class:today={isToday(dateStr)}>
          {formatDayNum(dateStr)}
        </div>
        <div class="day-label">
          <span class="day-name">{formatDayOfWeek(dateStr)}</span>
          {#if isToday(dateStr)}
            <Badge variant="success" size="xs" noDot>TODAY</Badge>
          {/if}
        </div>
        {#if dayEntries.length > 0}
          <span class="day-count">{dayEntries.length}</span>
        {/if}
      </div>

      <!-- Episodes -->
      <div class="day-episodes">
        {#each dayEntries as entry}
          <div class="cal-episode" class:acquired={entry.hasFile}>
            <div class="ep-accent" class:acquired={entry.hasFile}></div>
            <div class="ep-content">
              <div class="ep-series">
                {entry.series?.title || "Unknown"}
              </div>
              <div class="ep-meta">
                <span class="ep-code"
                  >{formatEpCode(entry.seasonNumber, entry.episodeNumber)}</span
                >
                {#if entry.title}
                  <span class="ep-name">{entry.title}</span>
                {/if}
              </div>
            </div>
            <div class="ep-icon">
              {#if entry.hasFile}
                <span class="material-icons acquired">check_circle</span>
              {:else if isPast(dateStr)}
                <span class="material-icons overdue">warning</span>
              {:else}
                <span class="material-icons pending">schedule</span>
              {/if}
            </div>
          </div>
        {:else}
          <div class="no-episodes">
            <span>No episodes</span>
          </div>
        {/each}
      </div>
    </div>
  {/each}
</div>

<style>
  /* Calendar Grid */
  .cal-grid {
    flex: 1;
    min-width: 0;
    min-height: 0;
    max-height: 100%;
    box-sizing: border-box;
    overflow: auto;
    padding: 1rem 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .cal-grid.month-view {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 0.5rem;
    align-content: flex-start;
  }

  /* Day Card */
  .cal-day {
    min-width: 0;
    flex-shrink: 0;
    background: rgba(255, 255, 255, 0.015);
    border: 1px solid rgba(255, 255, 255, 0.03);
    border-radius: 4px;
    overflow: hidden;
    transition: all 0.2s;
    position: relative;
  }

  .cal-day.today {
    border-color: rgba(52, 211, 153, 0.2);
    border-left: 1px solid rgba(52, 211, 153, 0.5);
    background: linear-gradient(
      110deg,
      rgba(52, 211, 153, 0.03) 0%,
      rgba(255, 255, 255, 0.015) 35%
    );
  }

  /* Novu bold left-glow bleed on today */
  .cal-day.today::after {
    content: "";
    position: absolute;
    top: 5%;
    bottom: 5%;
    left: -20px;
    width: 80px;
    background: radial-gradient(
      ellipse at 0% 50%,
      rgba(52, 211, 153, 0.5) 0%,
      transparent 65%
    );
    opacity: 0.18;
    pointer-events: none;
    z-index: 0;
  }

  .cal-day.past:not(.today) {
    opacity: 0.5;
  }

  .cal-day.has-episodes {
    border-color: rgba(255, 255, 255, 0.06);
  }

  /* Day Header */
  .day-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
    min-height: 38px;
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
  }

  .day-num {
    font-family: var(--font-mono, monospace);
    font-size: 1.1rem;
    font-weight: 900;
    color: var(--text-muted);
    min-width: 28px;
  }

  .day-num.today {
    color: #34d399;
  }

  .day-label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex: 1;
  }

  .day-name {
    font-size: 0.6rem;
    font-weight: 800;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-muted);
    opacity: 0.6;
  }

  .day-count {
    font-family: var(--font-mono, monospace);
    font-size: 0.55rem;
    font-weight: 800;
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-muted);
    padding: 1px 5px;
    border-radius: 3px;
  }

  /* Episodes */
  .day-episodes {
    min-width: 0;
    padding: 0.4rem;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .cal-episode {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
    min-height: 42px;
    box-sizing: border-box;
    padding: 0.5rem 0.6rem;
    background: rgba(255, 255, 255, 0.02);
    border-radius: 3px;
    transition: all 0.2s;
  }

  .cal-episode:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .cal-episode.acquired {
    border-left: 2px solid rgba(52, 211, 153, 0.4);
  }

  .ep-accent {
    width: 2px;
    height: 20px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 2px;
    flex-shrink: 0;
  }

  .ep-accent.acquired {
    background: rgba(52, 211, 153, 0.5);
  }

  .ep-content {
    flex: 1;
    min-width: 0;
  }

  .ep-series {
    font-size: 0.7rem;
    font-weight: 700;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ep-meta {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    margin-top: 2px;
  }

  .ep-code {
    font-family: var(--font-mono, monospace);
    font-size: 0.55rem;
    font-weight: 800;
    color: #34d399;
    opacity: 0.7;
  }

  .ep-name {
    font-size: 0.55rem;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ep-icon .material-icons {
    font-size: 16px;
  }

  .ep-icon .acquired {
    color: #34d399;
  }
  .ep-icon .overdue {
    color: #ef4444;
  }
  .ep-icon .pending {
    color: var(--text-muted);
    opacity: 0.4;
  }

  .no-episodes {
    padding: 0.5rem;
    text-align: center;
  }

  .no-episodes span {
    font-size: 0.55rem;
    color: var(--text-muted);
    opacity: 0.3;
  }

  /* Scrollbar */
  .cal-grid::-webkit-scrollbar {
    width: 4px;
  }
  .cal-grid::-webkit-scrollbar-track {
    background: transparent;
  }
  .cal-grid::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 2px;
  }

  /* Month view adaptations */
  .cal-grid.month-view .day-header {
    padding: 0.3rem 0.5rem;
  }

  .cal-grid.month-view .day-num {
    font-size: 0.85rem;
    min-width: 20px;
  }

  .cal-grid.month-view .ep-series {
    font-size: 0.6rem;
  }

  .cal-grid.month-view .ep-meta {
    display: none;
  }

  /* Responsive */
  @media (max-width: 768px) {
    .cal-grid.month-view {
      grid-template-columns: 1fr;
    }
  }
</style>
