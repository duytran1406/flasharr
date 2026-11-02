<script lang="ts">
  import type { ActivityEvent, LogEntry } from "$lib/stores/system";

  type LogCategory = "services" | "system";

  interface Props {
    activeCategory: LogCategory;
    logs: LogEntry[];
    activities: ActivityEvent[];
    onClearLogs?: () => void;
  }

  let { activeCategory = $bindable(), logs, activities, onClearLogs }: Props = $props();

  const activityIcons: Record<string, string> = {
    queued: "cloud_download",
    completed: "check_circle",
    failed: "error",
  };

  const categories: { id: LogCategory; label: string; icon: string }[] = [
    { id: "services", label: "System Log", icon: "terminal" },
    { id: "system", label: "Activity", icon: "history" },
  ];

  function handleTabKeydown(event: KeyboardEvent, index: number) {
    if (!["ArrowLeft", "ArrowRight"].includes(event.key)) return;
    event.preventDefault();
    const next = (index + (event.key === "ArrowRight" ? 1 : categories.length - 1)) % categories.length;
    activeCategory = categories[next].id;
    queueMicrotask(() => document.getElementById(`settings-log-tab-${categories[next].id}`)?.focus());
  }
</script>

<aside class="log-panel">
  <div class="log-panel-header">
    <div class="log-toggle" role="tablist" aria-label="Settings events">
      {#each categories as category, index}
        <button
          id={`settings-log-tab-${category.id}`}
          class="log-tab"
          class:active={activeCategory === category.id}
          role="tab"
          aria-selected={activeCategory === category.id}
          aria-controls="settings-log-panel"
          tabindex={activeCategory === category.id ? 0 : -1}
          onclick={() => (activeCategory = category.id)}
          onkeydown={(event) => handleTabKeydown(event, index)}
        ><span class="material-icons" aria-hidden="true">{category.icon}</span> {category.label}</button>
      {/each}
    </div>
    {#if activeCategory === "services"}
      <button class="btn-clear-log" onclick={onClearLogs} title="Clear local log view" aria-label="Clear local log view"><span class="material-icons">delete_sweep</span></button>
    {/if}
  </div>

  <div id="settings-log-panel" class="log-panel-body" role="tabpanel" aria-labelledby={`settings-log-tab-${activeCategory}`}>
    {#if activeCategory === "services"}
      {#if logs.length === 0}
        <div class="log-empty"><span class="material-icons">hourglass_empty</span> Waiting for system signals...</div>
      {:else}
        {#each logs as log}
          <div class="log-line {log.level.toLowerCase()}"><span class="log-ts">[{log.timestamp}]</span><span class="log-lvl">{log.level}</span><span class="log-msg">{log.message}</span></div>
        {/each}
      {/if}
    {:else}
      <div class="activity-feed">
        {#if activities.length === 0}
          <div class="log-empty"><span class="material-icons">inbox</span> No recent activity</div>
        {:else}
          {#each activities as activity}
            <div class="activity-item" class:failed={activity.kind === "failed"}>
              <div class="activity-icon"><span class="material-icons">{activityIcons[activity.kind]}</span></div>
              <div class="activity-content"><span class="activity-msg">{activity.message}</span><span class="activity-time">{activity.timestamp}</span></div>
            </div>
          {/each}
        {/if}
      </div>
    {/if}
  </div>

  <div class="log-panel-footer"><span class="dot-pulse" aria-hidden="true"></span><span class="log-status">Polling every 3 seconds · {activeCategory === "services" ? logs.length : activities.length} events · Clear affects this browser only</span></div>
</aside>

<style>
  .log-panel {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    max-width: 100%;
    background: var(--cui-surface-0);
    border: 1px solid var(--cui-glass-border);
    clip-path: polygon(8px 0,100% 0,100% calc(100% - 8px),calc(100% - 8px) 100%,0 100%,0 8px);
    overflow: hidden;
  }

  .log-panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: .5rem .75rem;
    border-bottom: 1px solid var(--cui-glass-border);
    flex-shrink: 0;
  }

  .log-toggle {
    display: flex;
    gap: .25rem;
    background: var(--cui-surface-hover);
    border-radius: 8px;
    padding: 2px;
  }

  .log-tab {
    display: flex;
    align-items: center;
    gap: .35rem;
    min-height: 44px;
    padding: .4rem .75rem;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--cui-text-secondary);
    font-size: var(--cui-text-sm);
    font-weight: 700;
    font-family: var(--cui-font-mono,monospace);
    letter-spacing: .06em;
    cursor: pointer;
    transition: color .2s,background .2s;
  }

  .log-tab .material-icons {
    font-size: 1rem;
  }

  .log-tab.active {
    background: color-mix(in srgb,var(--cui-primary) 14%,transparent);
    color: var(--cui-primary);
  }

  .btn-clear-log {
    width: 44px;
    height: 44px;
    border-radius: 6px;
    background: var(--cui-surface-hover);
    border: 1px solid var(--cui-glass-border);
    color: var(--cui-text-secondary);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
  }

  .btn-clear-log:hover {
    color: var(--cui-danger);
    border-color: var(--cui-danger);
  }

  .log-tab:focus-visible,
  .btn-clear-log:focus-visible {
    outline: 2px solid var(--cui-primary);
    outline-offset: 2px;
  }

  .log-panel-body {
    flex: 1;
    min-width: 0;
    max-width: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    padding: .65rem;
    font-family: var(--font-mono,monospace);
    font-size: .68rem;
    line-height: 1.65;
  }

  .log-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: .5rem;
    height: 100%;
    color: #fff3;
    font-size: .7rem;
  }

  .log-line {
    max-width: 100%;
    padding: .15rem .4rem;
    border-radius: 3px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .log-line:hover,
  .activity-item:hover {
    background: #ffffff08;
  }

  .log-ts {
    color: var(--cui-text-tertiary,#82909e);
    margin-right: .4rem;
  }

  .log-lvl {
    font-weight: 800;
    margin-right: .4rem;
    min-width: 36px;
    display: inline-block;
  }

  .log-msg {
    color: var(--cui-text-primary,#e8edf3);
  }

  .log-line.info .log-lvl {
    color: #22d3ee;
  }

  .log-line.warn .log-lvl {
    color: #fbbf24;
  }

  .log-line.error .log-lvl {
    color: #fb7185;
  }

  .log-line.debug .log-lvl {
    color: #aab6c2;
  }

  .activity-feed {
    display: flex;
    flex-direction: column;
    gap: .25rem;
  }

  .activity-item {
    display: flex;
    align-items: flex-start;
    gap: .5rem;
    padding: .5rem;
    border-radius: 8px;
  }

  .activity-icon {
    width: 24px;
    height: 24px;
    border-radius: 6px;
    background: #00f3ff14;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .activity-icon .material-icons {
    font-size: .85rem;
    color: #00f3ff;
  }

  .activity-item.failed .activity-icon {
    background: #fb71851f;
  }

  .activity-item.failed .material-icons {
    color: #fb7185;
  }

  .activity-content {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .activity-msg {
    font-size: .65rem;
    color: #ffffffbf;
    line-height: 1.4;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .activity-time {
    font-size: .55rem;
    color: #ffffff40;
  }

  .log-panel-footer {
    display: flex;
    align-items: center;
    gap: .5rem;
    padding: .4rem .75rem;
    border-top: 1px solid rgba(255,255,255,.04);
    flex-shrink: 0;
  }

  .log-status {
    font-size: .55rem;
    font-weight: 600;
    color: var(--cui-text-secondary,#aab6c2);
    font-family: var(--cui-font-mono,monospace);
    letter-spacing: .05em;
  }

  .dot-pulse {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--cui-success);
    animation: dot-blink 1.5s ease-in-out infinite;
  }

  @keyframes dot-blink {
    50% {
      opacity: .3;
    }
  }

  @media (prefers-reduced-motion:reduce) {
    .log-tab,
    .btn-clear-log {
      transition: none;
    }
    .dot-pulse {
      animation: none;
    }
  }

  @media (max-width:767px) {
    .log-panel-header,
    .log-panel-footer {
      padding-inline: .65rem;
    }
  }

  @media (max-width:480px) {
    .log-panel-header {
      align-items: stretch;
      flex-direction: column;
      gap: .5rem;
    }
    .log-toggle {
      width: 100%;
    }
    .log-tab {
      flex: 1;
      justify-content: center;
      padding-inline: .45rem;
    }
  }

</style>
