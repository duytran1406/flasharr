<script lang="ts">
  import { onMount } from "svelte";
  import { fetchCalendar, type CalendarEntry } from "$lib/stores/arr";
  import { sonarrConfigured as hasSonarrConfig } from "$lib/stores/system";
  import { ErrorState } from "@media-set/core-ui";
  import CalendarToolbar from "$lib/components/calendar/CalendarToolbar.svelte";
  import CalendarGrid from "$lib/components/calendar/CalendarGrid.svelte";

  type ViewMode = "week" | "month";

  let viewMode = $state<ViewMode>("week");
  let entries = $state<CalendarEntry[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let hasSonarr = $derived($hasSonarrConfig);

  onMount(async () => {
    const headerContainer = document.getElementById("header-dynamic-content");
    if (headerContainer) {
      headerContainer.innerHTML = `
        <div style="display: flex; align-items: center; gap: 0.75rem;">
          <span class="material-icons" style="color: var(--cui-secondary); font-size: var(--cui-text-2xl);">calendar_month</span>
          <h1 style="font-size: var(--cui-text-sm); font-weight: 800; letter-spacing: 0.1em; text-transform: uppercase; margin: 0; color: var(--cui-text-primary);">Calendar</h1>
        </div>
      `;
    }

    if (hasSonarr) {
      await loadCalendar();
    } else {
      loading = false;
    }
  });

  function startOfDay(date: Date): Date {
    const d = new Date(date);
    d.setHours(0, 0, 0, 0);
    return d;
  }

  function toDateKey(date: Date): string {
    return date.toISOString().split("T")[0];
  }

  // First day of the visible range (week view: 7 days, month view: 30 days)
  let rangeStart = $state(startOfDay(new Date()));
  const rangeDays = $derived(viewMode === "week" ? 7 : 30);
  const rangeLabel = $derived.by(() => {
    const rangeEnd = new Date(rangeStart);
    rangeEnd.setDate(rangeEnd.getDate() + rangeDays - 1);
    const format: Intl.DateTimeFormatOptions = {
      month: "short",
      day: "numeric",
      year: "numeric",
    };
    return `${rangeStart.toLocaleDateString("en-US", format)} – ${rangeEnd.toLocaleDateString("en-US", format)}`;
  });

  async function loadCalendar() {
    loading = true;
    error = null;
    try {
      entries = await fetchCalendar(rangeDays, rangeStart);
    } catch (e) {
      console.error("Failed to load calendar:", e);
      error = "Could not refresh the calendar.";
    } finally {
      loading = false;
    }
  }

  // Group entries by date string, one bucket per day in the visible range
  let groupedByDate = $derived.by(() => {
    const groups: Record<string, CalendarEntry[]> = {};

    for (let i = 0; i < rangeDays; i++) {
      const d = new Date(rangeStart);
      d.setDate(d.getDate() + i);
      groups[toDateKey(d)] = [];
    }

    for (const entry of entries) {
      if (!entry.airDateUtc) continue;
      const key = entry.airDateUtc.split("T")[0];
      if (groups[key]) groups[key].push(entry);
    }

    return groups;
  });

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

  function switchView(mode: ViewMode) {
    viewMode = mode;
    loadCalendar();
  }

  function shiftRange(direction: -1 | 1) {
    const next = new Date(rangeStart);
    next.setDate(next.getDate() + direction * rangeDays);
    rangeStart = next;
    loadCalendar();
  }

  function goToToday() {
    rangeStart = startOfDay(new Date());
    loadCalendar();
  }
</script>

<svelte:head>
  <title>Calendar - Flasharr</title>
</svelte:head>

<div class="calendar-page">
  <CalendarToolbar
    {viewMode}
    totalEpisodes={totalEpisodes()}
    acquiredCount={acquiredCount()}
    {rangeLabel}
    onSwitchView={switchView}
    onPrevious={() => shiftRange(-1)}
    onToday={goToToday}
    onNext={() => shiftRange(1)}
  />

  {#if loading && entries.length === 0}
    <div class="cal-loading">
      <div class="pulse-ring"></div><span>Loading calendar…</span>
    </div>
  {:else if !hasSonarr}
    <div class="cal-loading">
      <span class="material-icons" style="font-size: 48px; opacity: 0.15;"
        >link_off</span
      ><span>SONARR NOT CONNECTED</span>
    </div>
  {:else if error && entries.length === 0}
    <ErrorState
      title="Calendar unavailable"
      message={error}
      showRetry
      onRetry={loadCalendar}
    />
  {:else}
    {#if error}
      <ErrorState
        title="Calendar refresh failed"
        message={error}
        showRetry
        onRetry={loadCalendar}
      />
    {/if}
    <CalendarGrid
      {viewMode}
      {groupedByDate}
      {formatDayOfWeek}
      {formatDayNum}
      {isToday}
      {isPast}
      {formatEpCode}
    />
  {/if}
</div>

<style>
  .calendar-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  /* Loading */
  .cal-loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--cui-space-md);
    padding: var(--cui-space-xxl);
    color: var(--cui-text-tertiary);
    flex: 1;
  }

  .cal-loading span {
    font-size: var(--cui-text-xs);
    font-weight: 800;
    letter-spacing: 0.15em;
    opacity: 0.5;
  }

  .pulse-ring {
    width: 28px;
    height: 28px;
    border: 2px solid color-mix(in srgb, var(--cui-secondary) 30%, transparent);
    border-top-color: var(--cui-secondary);
    border-radius: var(--cui-radius-full);
    animation: spin var(--cui-duration-slow) linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (max-width: 768px) {
    .calendar-page {
      padding-bottom: calc(
        var(--cui-mobile-bottom-nav-height) + var(--cui-mobile-safe-area-bottom)
      );
      box-sizing: border-box;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .pulse-ring {
      animation: none;
    }
  }
</style>
