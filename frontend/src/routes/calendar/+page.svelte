<script lang="ts">
  import { onMount } from "svelte";
  import { fetchCalendar, type CalendarEntry } from "$lib/stores/arr";
  import { integrations } from "$lib/stores/settings";

  let viewMode: "week" | "month" = $state("week");
  let entries = $state<CalendarEntry[]>([]);
  let loading = $state(true);
  let currentDate = $state(new Date());

  let hasSonarr = $derived($integrations.sonarr_enabled);

  onMount(async () => {
    const headerContainer = document.getElementById("header-dynamic-content");
    if (headerContainer) {
      headerContainer.innerHTML = `
        <div style="display: flex; align-items: center; gap: 0.75rem;">
          <span class="material-icons" style="color: #34d399; font-size: 1.5rem;">calendar_month</span>
          <h1 style="font-size: 0.9rem; font-weight: 800; letter-spacing: 0.1em; text-transform: uppercase; margin: 0; color: #fff;">Calendar</h1>
        </div>
      `;
    }

    if (hasSonarr) {
      await loadCalendar();
    } else {
      loading = false;
    }
  });

  async function loadCalendar() {
    loading = true;
    try {
      entries = await fetchCalendar(viewMode === "week" ? 7 : 30);
    } finally {
      loading = false;
    }
  }

  // Group entries by date string
  let groupedByDate = $derived(() => {
    const groups: Record<string, CalendarEntry[]> = {};

    // Build date range
    const days = viewMode === "week" ? 7 : 30;
    for (let i = 0; i < days; i++) {
      const d = new Date();
      d.setDate(d.getDate() + i);
      const key = d.toISOString().split("T")[0];
      groups[key] = [];
    }

    // Fill with entries
    for (const entry of entries) {
      if (entry.airDateUtc) {
        const key = entry.airDateUtc.split("T")[0];
        if (groups[key]) {
          groups[key].push(entry);
        }
      }
    }

    return groups;
  });

  function formatDateLabel(dateStr: string): string {
    const d = new Date(dateStr + "T00:00:00");
    const today = new Date();
    const tomorrow = new Date();
    tomorrow.setDate(tomorrow.getDate() + 1);

    if (d.toDateString() === today.toDateString()) return "⚡ Today";
    if (d.toDateString() === tomorrow.toDateString()) return "→ Tomorrow";
    return d.toLocaleDateString("en-US", {
      weekday: "long",
      month: "short",
      day: "numeric",
    });
  }

  function formatDayOfWeek(dateStr: string): string {
    return new Date(dateStr + "T00:00:00").toLocaleDateString("en-US", {
      weekday: "short",
    });
  }

  function formatDayNum(dateStr: string): string {
    return new Date(dateStr + "T00:00:00").getDate().toString();
  }

  function isToday(dateStr: string): boolean {
    return (
      new Date(dateStr + "T00:00:00").toDateString() ===
      new Date().toDateString()
    );
  }

  function isPast(dateStr: string): boolean {
    const d = new Date(dateStr + "T00:00:00");
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    return d < today;
  }

  function formatEpCode(s: number, e: number): string {
    return `S${String(s).padStart(2, "0")}E${String(e).padStart(2, "0")}`;
  }

  function totalEpisodes(): number {
    return entries.length;
  }

  function acquiredCount(): number {
    return entries.filter((e) => e.hasFile).length;
  }

  function switchView(mode: "week" | "month") {
    viewMode = mode;
    loadCalendar();
  }
</script>

<svelte:head>
  <title>Calendar - Flasharr</title>
</svelte:head>

<div class="calendar-page">
  <!-- Toolbar -->
  <div class="cal-toolbar">
    <div class="cal-toolbar-left">
      <div class="view-toggle">
        <button
          class:active={viewMode === "week"}
          onclick={() => switchView("week")}
        >
          <span class="material-icons">view_week</span>
          <span>Week</span>
        </button>
        <button
          class:active={viewMode === "month"}
          onclick={() => switchView("month")}
        >
          <span class="material-icons">calendar_view_month</span>
          <span>Month</span>
        </button>
      </div>
    </div>

    <div class="cal-toolbar-right">
      <div class="cal-stat">
        <span class="csl">EPISODES</span><span class="csv"
          >{totalEpisodes()}</span
        >
      </div>
      <div class="cal-stat">
        <span class="csl">ACQUIRED</span><span class="csv acquired"
          >{acquiredCount()}</span
        >
      </div>
      <div class="cal-stat">
        <span class="csl">PENDING</span><span class="csv pending"
          >{totalEpisodes() - acquiredCount()}</span
        >
      </div>
    </div>
  </div>

  <!-- Calendar Content -->
  {#if loading}
    <div class="cal-loading">
      <div class="pulse-ring"></div>
      <span>SCANNING SCHEDULE DATABASE...</span>
    </div>
  {:else if !hasSonarr}
    <div class="cal-loading">
      <span class="material-icons" style="font-size: 48px; opacity: 0.15;"
        >link_off</span
      >
      <span>SONARR NOT CONNECTED</span>
    </div>
  {:else}
    <div class="cal-grid" class:month-view={viewMode === "month"}>
      {#each Object.entries(groupedByDate()) as [dateStr, dayEntries]}
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
                <span class="today-badge">TODAY</span>
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
                      >{formatEpCode(
                        entry.seasonNumber,
                        entry.episodeNumber,
                      )}</span
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
  {/if}
</div>

<style>
  .calendar-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  /* Toolbar */
  .cal-toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    flex-shrink: 0;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .cal-toolbar-left,
  .cal-toolbar-right {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .view-toggle {
    display: flex;
    gap: 2px;
  }

  .view-toggle button {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.5rem 1rem;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    color: var(--text-muted);
    font-size: 0.65rem;
    font-weight: 800;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    cursor: pointer;
    transition: all 0.2s;
    font-family: var(--font-mono, monospace);
  }

  .view-toggle button .material-icons {
    font-size: 1rem;
  }

  .view-toggle button.active {
    background: rgba(52, 211, 153, 0.1);
    border-color: rgba(52, 211, 153, 0.3);
    color: #34d399;
  }

  .view-toggle button:hover:not(.active) {
    background: rgba(255, 255, 255, 0.04);
    color: #fff;
  }

  .cal-stat {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.6rem;
    font-weight: 800;
    letter-spacing: 0.08em;
  }

  .csl {
    color: var(--text-muted);
    opacity: 0.5;
  }
  .csv {
    color: var(--color-primary);
    font-family: var(--font-mono, monospace);
  }
  .csv.acquired {
    color: #34d399;
  }
  .csv.pending {
    color: #fbbf24;
  }

  /* Loading */
  .cal-loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1rem;
    padding: 4rem;
    color: var(--text-muted);
    flex: 1;
  }

  .cal-loading span {
    font-size: 0.65rem;
    font-weight: 800;
    letter-spacing: 0.15em;
    opacity: 0.5;
  }

  .pulse-ring {
    width: 28px;
    height: 28px;
    border: 2px solid rgba(52, 211, 153, 0.3);
    border-top-color: #34d399;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Calendar Grid */
  .cal-grid {
    flex: 1;
    overflow-y: auto;
    padding: 1rem 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .cal-grid.month-view {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 0.5rem;
    align-content: flex-start;
  }

  /* Day Card */
  .cal-day {
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

  .today-badge {
    font-size: 0.45rem;
    font-weight: 900;
    letter-spacing: 0.12em;
    background: rgba(52, 211, 153, 0.2);
    color: #34d399;
    padding: 1px 5px;
    border-radius: 2px;
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
    padding: 0.4rem;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .cal-episode {
    display: flex;
    align-items: center;
    gap: 0.5rem;
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
  .cal-grid.month-view .cal-day {
    min-height: 120px;
  }

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
    .cal-toolbar {
      flex-direction: column;
    }
    .cal-grid.month-view {
      grid-template-columns: 1fr;
    }
  }
</style>
