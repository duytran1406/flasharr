<script lang="ts">
  import { tick } from 'svelte';
  import type { SourceFilter, StatusFilter } from './downloadsHelpers';

  interface Props {
    connectionText: string;
    connected: boolean;
    onReconnect?: () => void;
    onResumeAll?: () => void;
    onPauseAll?: () => void;
    onRefresh?: () => void;
    sourceFilter?: SourceFilter;
    statusFilter?: StatusFilter;
    mediaTypeFilter?: string;
    sort?: string;
    loadedCount?: number;
    onSourceFilter?: (source: string) => void;
    onStatusFilter?: (status: string) => void;
    onMediaTypeFilter?: (mediaType: string) => void;
    onSort?: (sort: string) => void;
    onClear?: () => void;
    visibleColumns: Set<string>;
    onToggleColumn: (column: string) => void;
    searchQuery?: string;
  }

  let {
    connectionText,
    connected,
    onReconnect,
    onResumeAll,
    onPauseAll,
    onRefresh,
    visibleColumns,
    onToggleColumn,
    searchQuery = $bindable(),
  }: Props = $props();

  const COLUMNS = ['Filename', 'Source', 'Status', 'Size', 'Progress', 'Speed', 'ETA', 'Date added'];

  let columnsOpen = $state(false);
  let columnsButton = $state<HTMLButtonElement | undefined>(undefined);
  let columnsMenu = $state<HTMLDivElement | undefined>(undefined);

  async function toggleColumnsMenu() {
    columnsOpen = !columnsOpen;
    if (columnsOpen) {
      await tick();
      columnsMenu?.querySelector<HTMLElement>('[role=menuitemcheckbox]')?.focus();
    }
  }

  function toggleColumn(column: string) {
    // Filename is always visible
    if (column !== 'Filename') onToggleColumn(column);
  }

  function closeColumnsMenu() {
    columnsOpen = false;
    tick().then(() => columnsButton?.focus());
  }

  function handleMenuKeydown(e: KeyboardEvent) {
    const items = [...(columnsMenu?.querySelectorAll<HTMLElement>('[role=menuitemcheckbox]') || [])];
    const index = items.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'Escape') {
      e.preventDefault();
      closeColumnsMenu();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      items[(index + 1) % items.length]?.focus();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      items[(index - 1 + items.length) % items.length]?.focus();
    } else if (e.key === 'Home') {
      e.preventDefault();
      items[0]?.focus();
    } else if (e.key === 'End') {
      e.preventDefault();
      items.at(-1)?.focus();
    }
  }
</script>

<svelte:window
  onclick={(e) => {
    if (columnsOpen && !(e.target as HTMLElement).closest('.columns-wrap')) columnsOpen = false;
  }}
  onkeydown={(e) => {
    if (columnsOpen && e.key === 'Escape') {
      e.preventDefault();
      closeColumnsMenu();
    }
  }}
/>

<div class="toolbar">
  <label class="search"
    ><span class="material-icons" aria-hidden="true">search</span><input
      type="search"
      placeholder="Search downloads…"
      aria-label="Search downloads"
      bind:value={searchQuery}
    /></label
  >
  <div class="actions" aria-label="Download controls">
    {#if !connected}
      <button aria-label="Reconnect live updates" title={`Reconnect · ${connectionText}`} onclick={onReconnect}
        ><span class="material-icons">wifi</span></button
      >
    {/if}
    <button title="Resume all" aria-label="Resume all" onclick={onResumeAll}
      ><span class="material-icons resume">play_arrow</span></button
    >
    <button title="Pause all" aria-label="Pause all" onclick={onPauseAll}
      ><span class="material-icons pause">pause</span></button
    >
    <button title="Refresh" aria-label="Refresh downloads" onclick={onRefresh}
      ><span class="material-icons">refresh</span></button
    >
    <div class="columns-wrap">
      <button
        bind:this={columnsButton}
        title="Columns"
        aria-label="Download columns"
        aria-haspopup="menu"
        aria-expanded={columnsOpen}
        onclick={(e) => {
          e.stopPropagation();
          toggleColumnsMenu();
        }}><span class="material-icons">view_column</span></button
      >
      {#if columnsOpen}
        <div
          class="columns-menu"
          role="menu"
          aria-label="Download columns"
          tabindex="-1"
          bind:this={columnsMenu}
          onkeydown={handleMenuKeydown}
          onclick={(e) => e.stopPropagation()}
        >
          {#each COLUMNS as column}
            <button
              role="menuitemcheckbox"
              aria-checked={visibleColumns.has(column)}
              disabled={column === 'Filename'}
              onclick={() => toggleColumn(column)}
              ><span class="material-icons"
                >{visibleColumns.has(column) ? 'check_box' : 'check_box_outline_blank'}</span
              >
              {column}</button
            >
          {/each}
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .toolbar {
    height:77px;
    padding:0 24px;
    display:flex;
    align-items:center;
    justify-content:space-between;
    gap:24px;
    border-bottom:1px solid var(--cui-glass-border);
  }
  .search {
    height:42px;
    width:min(680px,65%);
    display:flex;
    align-items:center;
    gap:10px;
    padding:0 14px;
    background:var(--cui-input-bg);
    border:1px solid var(--cui-glass-border);
    color:var(--cui-text-tertiary);
  }
  .search:focus-within {
    border-color:var(--cui-glass-border-active);
    box-shadow:var(--cui-glow-primary);
  }
  .search .material-icons {
    font-size:19px;
  }
  .search input {
    flex:1;
    min-width:0;
    border:0;
    outline:0;
    background:transparent;
    color:var(--cui-text-primary);
    font:inherit;
  }
  .actions {
    display:flex;
    align-items:center;
    gap:8px;
  }
  .actions button {
    width:44px;
    height:44px;
    display:grid;
    place-items:center;
    background:var(--cui-surface-2);
    border:1px solid var(--cui-glass-border);
    color:var(--cui-text-secondary);
    cursor:pointer;
  }
  .actions button:hover,
  .actions button:focus-visible {
    border-color:var(--cui-glass-border-active);
    color:var(--cui-primary);
    outline:none;
  }
  .actions .material-icons {
    font-size:20px;
  }
  .actions .resume {
    color:var(--cui-secondary);
  }
  .actions .pause {
    color:var(--cui-danger);
  }
  .columns-wrap {
    position:relative;
  }
  .columns-menu {
    position:absolute;
    z-index:1000;
    right:0;
    top:48px;
    width:190px;
    padding:6px;
    background:var(--cui-glass-heavy);
    border:1px solid var(--cui-glass-border);
    box-shadow:var(--cui-shadow-glass);
  }
  .columns-menu button {
    width:100%;
    min-height:44px;
    display:flex;
    grid-template-columns:none;
    justify-content:flex-start;
    gap:9px;
    padding:0 9px;
    border:0;
    background:transparent;
  }
  .columns-menu button:hover,
  .columns-menu button:focus-visible {
    background:var(--cui-surface-active);
  }
  .columns-menu button:disabled {
    opacity:.75;
    cursor:not-allowed;
  }
  .columns-menu .material-icons {
    font-size:17px;
    color:var(--cui-primary);
  }
</style>
