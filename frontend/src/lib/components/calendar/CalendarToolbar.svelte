<script lang="ts">
  interface Props {
    viewMode: "week" | "month";
    totalEpisodes: number;
    acquiredCount: number;
    rangeLabel: string;
    onSwitchView: (mode: "week" | "month") => void;
    onPrevious: () => void;
    onToday: () => void;
    onNext: () => void;
  }

  let {
    viewMode,
    totalEpisodes,
    acquiredCount,
    rangeLabel,
    onSwitchView,
    onPrevious,
    onToday,
    onNext,
  }: Props = $props();
</script>

<div class="cal-toolbar">
  <div class="cal-toolbar-left">
    <div class="view-toggle">
      <button
        class:active={viewMode === "week"}
        onclick={() => onSwitchView("week")}
      >
        <span class="material-icons">view_week</span>
        <span>Week</span>
      </button>
      <button
        class:active={viewMode === "month"}
        onclick={() => onSwitchView("month")}
      >
        <span class="material-icons">calendar_view_month</span>
        <span>Month</span>
      </button>
    </div>

    <div class="range-controls" aria-label="Calendar range">
      <button onclick={onPrevious} aria-label="Previous range"
        ><span class="material-icons">chevron_left</span><span>Previous</span
        ></button
      >
      <button class="today-button" onclick={onToday}>Today</button>
      <button onclick={onNext} aria-label="Next range"
        ><span>Next</span><span class="material-icons">chevron_right</span
        ></button
      >
    </div>

    <span class="range-label" aria-live="polite">{rangeLabel}</span>
  </div>

  <div class="cal-toolbar-right">
    <div class="cal-stat">
      <span class="csl">EPISODES</span><span class="csv">{totalEpisodes}</span>
    </div>
    <div class="cal-stat">
      <span class="csl">ACQUIRED</span><span class="csv acquired"
        >{acquiredCount}</span
      >
    </div>
    <div class="cal-stat">
      <span class="csl">PENDING</span><span class="csv pending"
        >{totalEpisodes - acquiredCount}</span
      >
    </div>
  </div>
</div>

<style>
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
  .cal-toolbar-right,
  .view-toggle,
  .range-controls {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .view-toggle {
    gap: 2px;
  }

  .view-toggle button,
  .range-controls button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    min-height: 44px;
    padding: 0.5rem 0.75rem;
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

  .view-toggle button .material-icons,
  .range-controls .material-icons {
    font-size: 1rem;
  }

  .view-toggle button.active,
  .today-button {
    background: rgba(52, 211, 153, 0.1) !important;
    border-color: rgba(52, 211, 153, 0.3) !important;
    color: #34d399 !important;
  }

  .view-toggle button:hover:not(.active),
  .range-controls button:hover {
    background: rgba(255, 255, 255, 0.04);
    color: #fff;
  }

  .view-toggle button:focus-visible,
  .range-controls button:focus-visible {
    outline: 2px solid var(--cui-secondary);
    outline-offset: 2px;
  }

  .range-label {
    font-size: 0.7rem;
    color: var(--text-muted);
    font-family: var(--font-mono, monospace);
    white-space: nowrap;
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

  @media (max-width: 768px) {
    .cal-toolbar {
      align-items: stretch;
      padding: 0.75rem;
    }
    .cal-toolbar-left {
      flex-wrap: wrap;
      gap: 0.5rem;
    }
    .range-controls {
      gap: 2px;
    }
    .range-controls button {
      padding: 0.5rem;
    }
    .range-label {
      flex-basis: 100%;
      text-align: center;
    }
    .cal-toolbar-right {
      justify-content: space-around;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .view-toggle button,
    .range-controls button {
      transition: none;
    }
  }
</style>
