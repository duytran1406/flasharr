<script lang="ts">
  import { tick } from "svelte";
  import { qbitStore } from "$lib/stores/qbit.svelte";

  interface Props {
    onOpenSettings?: () => void;
    /** Column labels currently shown by TorrentQueueTable. */
    visibleColumns: Set<string>;
    onToggleColumn: (column: string) => void;
  }

  let { onOpenSettings, visibleColumns, onToggleColumn }: Props = $props();

  const COLUMNS = [
    "Media / Torrent",
    "Type",
    "State",
    "Size",
    "Progress",
    "DL",
    "UL",
    "ETA",
    "Ratio",
    "Seeds/Peers",
    "Added on",
  ];

  let columnsOpen = $state(false);
  let columnsButton = $state<HTMLButtonElement | undefined>(undefined);
  let columnsMenu = $state<HTMLDivElement | undefined>(undefined);

  async function toggleColumnsMenu() {
    columnsOpen = !columnsOpen;
    if (columnsOpen) {
      await tick();
      columnsMenu?.querySelector<HTMLElement>("[role=menuitemcheckbox]")?.focus();
    }
  }

  function closeColumnsMenu() {
    columnsOpen = false;
    tick().then(() => columnsButton?.focus());
  }

  function handleMenuKeydown(e: KeyboardEvent) {
    const items = [...(columnsMenu?.querySelectorAll<HTMLElement>("[role=menuitemcheckbox]") || [])];
    const index = items.indexOf(document.activeElement as HTMLElement);
    if (e.key === "Escape") {
      e.preventDefault();
      closeColumnsMenu();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      items[(index + 1) % items.length]?.focus();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      items[(index - 1 + items.length) % items.length]?.focus();
    } else if (e.key === "Home") {
      e.preventDefault();
      items[0]?.focus();
    } else if (e.key === "End") {
      e.preventDefault();
      items.at(-1)?.focus();
    }
  }
</script>

<svelte:window
  onclick={(e) => {
    if (columnsOpen && !(e.target as HTMLElement).closest(".columns-wrap")) columnsOpen = false;
  }}
  onkeydown={(e) => {
    if (columnsOpen && e.key === "Escape") {
      e.preventDefault();
      closeColumnsMenu();
    }
  }}
/>

<div class="toolbar">
  <div aria-hidden="true"></div>
  <div class="actions" aria-label="Torrent controls">
    <button title="Refresh" aria-label="Refresh torrents" onclick={() => qbitStore.fetchTorrents()}>
      <span class="material-icons">refresh</span>
    </button>
    <div class="columns-wrap">
      <button
        bind:this={columnsButton}
        title="Columns"
        aria-label="Torrent columns"
        aria-haspopup="menu"
        aria-expanded={columnsOpen}
        onclick={(e) => {
          e.stopPropagation();
          toggleColumnsMenu();
        }}
      >
        <span class="material-icons">view_column</span>
      </button>
      {#if columnsOpen}
        <div
          bind:this={columnsMenu}
          class="columns-menu"
          role="menu"
          aria-label="Torrent columns"
          tabindex="-1"
          onkeydown={handleMenuKeydown}
          onclick={(e) => e.stopPropagation()}
        >
          {#each COLUMNS as column}
            <button
              role="menuitemcheckbox"
              aria-checked={visibleColumns.has(column)}
              disabled={column === "Media / Torrent"}
              onclick={() => onToggleColumn(column)}
            >
              <span class="material-icons"
                >{visibleColumns.has(column) ? "check_box" : "check_box_outline_blank"}</span
              >
              {column}
            </button>
          {/each}
        </div>
      {/if}
    </div>
    <button title="Settings" aria-label="Torrent settings" onclick={onOpenSettings}>
      <span class="material-icons">settings</span>
    </button>
  </div>
</div>

<style>
	.toolbar {
		height: 77px;
		padding: 0 24px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 24px;
		border-bottom: 1px solid var(--cui-glass-border);
	}

	.actions {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.actions > button,
	.columns-wrap > button {
		width: 40px;
		height: 40px;
		display: grid;
		place-items: center;
		background: var(--cui-surface-2);
		border: 1px solid var(--cui-glass-border);
		color: var(--cui-text-secondary);
		cursor: pointer;
	}

	.actions button:hover,
	.actions button:focus-visible {
		border-color: var(--cui-glass-border-active);
		color: var(--cui-primary);
		outline: none;
	}

	.actions .material-icons {
		font-size: 20px;
	}

	.columns-wrap {
		position: relative;
	}

	.columns-menu {
		position: absolute;
		z-index: 1000;
		right: 0;
		top: 48px;
		width: 190px;
		padding: 6px;
		background: var(--cui-glass-heavy);
		border: 1px solid var(--cui-glass-border);
		box-shadow: var(--cui-shadow-glass);
	}

	.columns-menu button {
		width: 100%;
		height: 36px;
		display: flex;
		align-items: center;
		justify-content: flex-start;
		gap: 9px;
		padding: 0 9px;
		border: 0;
		background: transparent;
		color: var(--cui-text-primary);
		cursor: pointer;
	}

	.columns-menu button:hover,
	.columns-menu button:focus-visible {
		background: var(--cui-surface-active);
		outline: none;
	}

	.columns-menu button:disabled {
		opacity: .75;
		cursor: not-allowed;
	}

	.columns-menu .material-icons {
		font-size: 17px;
		color: var(--cui-primary);
	}
</style>
