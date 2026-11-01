<script lang="ts">
  import { tick } from "svelte";
  import {
    availableSourceTabs,
    collectResultFiles,
    resultSource,
    type SourceFilter,
  } from "./resultUtils";

  interface Props {
    results?: any;
    active?: SourceFilter;
    onSelect?: (source: SourceFilter) => void;
  }

  let { results = null, active = "all", onSelect }: Props = $props();

  let open = $state(false);
  let trigger: HTMLButtonElement | undefined;
  let menu = $state<HTMLDivElement | undefined>();

  const counts = $derived.by(() => {
    const files = collectResultFiles(results);
    const tally: Record<string, number> = { all: files.length };
    for (const file of files) tally[resultSource(file)] = (tally[resultSource(file)] ?? 0) + 1;
    return tally;
  });

  const tabs = $derived(availableSourceTabs({ sources: counts }));
  const activeLabel = $derived(tabs.find(([key]) => key === active)?.[1] ?? "All");

  /** Per-provider health lines shown under "Source diagnostics". */
  const diagnostics = $derived(
    ["fshare", "torrent", "stremio", "torbox"].flatMap((key) => {
      const status = results?.sources?.[key];
      if (!status) return [];
      const label =
        key === "fshare"
          ? "Fshare"
          : key === "torrent"
            ? "Prowlarr"
            : key === "stremio"
              ? "Stremio addons"
              : "TorBox";
      const telemetry = status.telemetry;
      const byHealth = telemetry?.byHealth ?? telemetry?.by_health ?? {};
      const byFailure = telemetry?.byFailureClass ?? telemetry?.by_failure_class ?? {};
      const details = [
        status.message || status.state,
        ...Object.entries(byHealth).map(([name, count]) => `${count} ${name}`),
        ...Object.entries(byFailure).map(([name, count]) => `${count} ${name}`),
        telemetry?.truncated ? "details limited" : "",
      ]
        .filter(Boolean)
        .join(" · ");
      const addons: string[] = (status.addons ?? []).slice(0, 64).map((addon: any) =>
        [
          addon.name || addon.id || "Addon",
          addon.health,
          `${addon.resultCount ?? addon.result_count ?? 0} results`,
          `${addon.elapsedMs ?? addon.elapsed_ms ?? 0} ms`,
          addon.failureClass ?? addon.failure_class,
        ]
          .filter(Boolean)
          .join(" · "),
      );
      return [{ key, label, details, addons }];
    }),
  );

  function options(): HTMLElement[] {
    return [...(menu?.querySelectorAll<HTMLElement>('[role="option"]') ?? [])];
  }

  async function toggleMenu() {
    open = !open;
    if (open) {
      await tick();
      (options().find((el) => el.getAttribute("aria-selected") === "true") ?? options()[0])?.focus();
    }
  }

  function closeMenu(restoreFocus = true) {
    open = false;
    if (restoreFocus) tick().then(() => trigger?.focus());
  }

  function handleMenuKeydown(event: KeyboardEvent) {
    const items = options();
    const index = items.indexOf(document.activeElement as HTMLElement);
    if (event.key === "Escape") {
      event.preventDefault();
      closeMenu();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      items[(index + 1) % items.length]?.focus();
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      items[(index - 1 + items.length) % items.length]?.focus();
    } else if (event.key === "Home") {
      event.preventDefault();
      items[0]?.focus();
    } else if (event.key === "End") {
      event.preventDefault();
      items.at(-1)?.focus();
    }
  }

  function select(source: SourceFilter) {
    onSelect?.(source);
    closeMenu();
  }
</script>

<svelte:window
  onclick={(event) => {
    if (open && !(event.target as HTMLElement).closest(".source-filter")) closeMenu(false);
  }}
/>

<div class="source-filter">
  <button
    bind:this={trigger}
    class="trigger"
    type="button"
    aria-label="Filter by source"
    aria-haspopup="listbox"
    aria-expanded={open}
    onclick={toggleMenu}
  >
    <span>{activeLabel} <strong>{counts[active] ?? 0}</strong></span>
    <span class="material-icons" aria-hidden="true">{open ? "expand_less" : "expand_more"}</span>
  </button>
  {#if open}
    <div
      bind:this={menu}
      class="menu"
      role="listbox"
      tabindex="-1"
      aria-label="Filter search results by source"
      onkeydown={handleMenuKeydown}
    >
      {#each tabs as [key, label]}
        <button
          type="button"
          role="option"
          aria-selected={active === key}
          class:active={active === key}
          onclick={() => select(key)}><span>{label}</span><span>{counts[key] ?? 0}</span></button
        >
      {/each}
    </div>
  {/if}
</div>
{#if diagnostics.length > 0}
  <details class="source-status">
    <summary>Source diagnostics</summary>
    <ul>
      {#each diagnostics as item}
        <li>
          <strong>{item.label}</strong>: {item.details}
          {#if item.addons.length > 0}
            <ul class="addon-statuses">
              {#each item.addons as addon}
                <li>{addon}</li>
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
  </details>
{/if}

<style>
  .source-filter {
    position: relative;
    display: flex;
    align-items: center;
    margin: .35rem 0 .75rem;
    width: max-content;
  }

  .trigger {
    min-height: var(--cui-touch-min);
    min-width: 9rem;
    padding: .35rem .55rem .35rem .75rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: .75rem;
    border: 1px solid var(--cui-glass-border);
    border-radius: var(--cui-radius-md);
    background: var(--cui-surface-hover);
    color: var(--cui-text-secondary);
    cursor: pointer;
    font-size: var(--cui-text-xs);
    font-weight: 800;
    text-transform: uppercase;
  }

  .trigger strong {
    color: var(--cui-primary);
    margin-left: .25rem;
  }

  .trigger .material-icons {
    font-size: 1.1rem;
  }

  .trigger:focus-visible,
  .menu button:focus-visible {
    outline: 2px solid var(--cui-primary);
    outline-offset: 2px;
  }

  .menu {
    position: absolute;
    z-index: 20;
    top: calc(100% + .25rem);
    left: 0;
    min-width: 11rem;
    padding: .3rem;
    border: 1px solid var(--cui-glass-border-active);
    border-radius: var(--cui-radius-md);
    background: var(--cui-surface-3,#171a20);
    box-shadow: 0 .75rem 2rem #00000073;
  }

  .menu button {
    width: 100%;
    min-height: var(--cui-touch-min);
    padding: .5rem .65rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border: 0;
    border-radius: .4rem;
    background: transparent;
    color: var(--cui-text-secondary);
    cursor: pointer;
    font-size: var(--cui-text-xs);
    font-weight: 700;
    text-transform: uppercase;
  }

  .menu button:hover,
  .menu button.active {
    background: var(--cui-surface-active);
    color: var(--cui-primary);
  }

  .source-status {
    max-width: 42rem;
    margin: .45rem 0 0;
    color: #ffb84dd9;
    font-size: .72rem;
  }

  .source-status summary {
    cursor: pointer;
  }

  .source-status > ul {
    max-height: 8rem;
    margin: .35rem 0 0;
    padding-left: 1.1rem;
    overflow: auto;
  }

  .source-status li {
    overflow-wrap: anywhere;
  }

  .addon-statuses {
    max-height: 6rem;
    margin: .25rem 0;
    padding-left: 1rem;
    overflow: auto;
    color: var(--cui-text-secondary);
  }
</style>
