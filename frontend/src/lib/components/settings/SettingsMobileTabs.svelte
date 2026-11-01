<script lang="ts">
  import { SETTINGS_TABS, nextSettingsTab, type SettingsTabId } from "./settingsTabs";

  interface Props {
    active: SettingsTabId;
    onSelect?: (tab: SettingsTabId) => void;
  }

  let { active, onSelect }: Props = $props();

  function handleKeydown(event: KeyboardEvent, current: SettingsTabId) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const next = nextSettingsTab(current, event.key);
    onSelect?.(next);
    queueMicrotask(() => document.getElementById(`settings-mobile-tab-${next}`)?.focus());
  }
</script>

<div class="mobile-tab-bar" role="tablist" aria-label="Settings sections">
  {#each SETTINGS_TABS as tab}
    <button
      id={`settings-mobile-tab-${tab.id}`}
      type="button"
      class="mobile-tab"
      class:active={active === tab.id}
      role="tab"
      aria-selected={active === tab.id}
      aria-controls={`settings-tabpanel-${tab.id}`}
      tabindex={active === tab.id ? 0 : -1}
      onclick={() => onSelect?.(tab.id)}
      onkeydown={(event) => handleKeydown(event, tab.id)}
    >
      <span class="material-icons" aria-hidden="true">{tab.icon}</span>
      <span>{tab.label}</span>
    </button>
  {/each}
</div>

<style>
  .mobile-tab-bar {
    display: none;
  }

  @media (max-width:767px) {
    .mobile-tab-bar {
      position: sticky;
      top: 0;
      z-index: 2;
      display: flex;
      gap: .25rem;
      overflow-x: auto;
      scrollbar-width: none;
      background: #080a0ff5;
      border: 1px solid rgba(255,255,255,.07);
      border-radius: var(--cui-radius-md);
      padding: .35rem .5rem;
      flex-shrink: 0;
    }
    .mobile-tab-bar::-webkit-scrollbar {
      display: none;
    }
    .mobile-tab {
      display: flex;
      min-width: max-content;
      min-height: 44px;
      align-items: center;
      justify-content: center;
      gap: .35rem;
      padding: .4rem .65rem;
      background: transparent;
      border: none;
      border-radius: var(--cui-radius-sm);
      color: #ffffff80;
      font-size: .6rem;
      font-weight: 700;
      font-family: var(--font-mono,monospace);
      letter-spacing: .06em;
      cursor: pointer;
      transition: color .2s,background .2s;
    }
    .mobile-tab .material-icons {
      font-size: 1.05rem;
    }
    .mobile-tab.active {
      background: #00f3ff1a;
      color: #00f3ff;
    }
    .mobile-tab:focus-visible {
      outline: 2px solid var(--cui-primary,#00f3ff);
      outline-offset: 2px;
    }
  }

  @media (prefers-reduced-motion:reduce) {
    .mobile-tab {
      transition: none;
    }
  }

</style>
