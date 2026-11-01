<script lang="ts">
  import { tick } from "svelte";
  import { Badge, type BadgeVariant } from "@media-set/core-ui";
  import { qbitStore } from "$lib/stores/qbit.svelte";
  import type { UnifiedTorrentRow } from "$lib/stores/qbitReducer";
  import {
    formatTorrentBytes,
    formatTorrentEta,
    formatTorrentProgress,
    formatTorrentSpeed,
  } from "./torrentFormat";
  import type { TorrentIntent } from "./torrentIntent";

  type SortKey = "name" | "state" | "size" | "progress" | "dlspeed" | "upspeed" | "eta" | "ratio" | "peers" | "added";

  /** Episode/subtitle file of a managed season-pack torrent. */
  interface PayloadItem {
    kind?: string;
    season?: number | string | null;
    episode?: number | string | null;
    title?: string | null;
    filename?: string | null;
  }

  interface Props {
    /** Torrent currently shown in the details modal (row highlight). */
    detailsTorrent: UnifiedTorrentRow | null;
    onOpenDetails: (torrent: UnifiedTorrentRow) => void;
    onOpenIntent: (intent: TorrentIntent) => void;
    visibleColumns: Set<string>;
  }

  let { detailsTorrent, onOpenDetails, onOpenIntent, visibleColumns }: Props = $props();

  /** [sort key, column label]; labels double as visibility keys. */
  const COLUMNS: [SortKey, string][] = [
    ["name", "Media / Torrent"],
    ["name", "Type"],
    ["state", "State"],
    ["size", "Size"],
    ["progress", "Progress"],
    ["dlspeed", "DL"],
    ["upspeed", "UL"],
    ["eta", "ETA"],
    ["ratio", "Ratio"],
    ["peers", "Seeds/Peers"],
    ["added", "Added on"],
  ];

  let menuTorrent = $state<UnifiedTorrentRow | null>(null);
  let menuPosition = $state({ x: 0, y: 0 });
  let menuEl = $state<HTMLDivElement | null>(null);
  /** Element that opened the context menu; focus returns there on close. */
  let menuTrigger: HTMLElement | null = null;
  let expanded = $state(new Set<string>());
  let scrolledToEnd = $state(false);
  let sortKey = $state<SortKey | null>(null);
  let sortDirection = $state(1);

  const sortedTorrents = $derived(
    sortKey
      ? [...qbitStore.torrentList].sort((a, b) => sortDirection * compareTorrents(a, b, sortKey!))
      : qbitStore.torrentList,
  );

  function compareTorrents(a: UnifiedTorrentRow, b: UnifiedTorrentRow, key: SortKey): number {
    if (key === "name" || key === "state") return a[key].localeCompare(b[key], undefined, { numeric: true });
    if (key === "peers") return a.num_seeds + a.num_leechs - (b.num_seeds + b.num_leechs);
    return (a as any)[key] - (b as any)[key];
  }

  function isPaused(torrent: UnifiedTorrentRow) {
    return torrent.state.toLowerCase().includes("pause");
  }

  function stateLabel(torrent: UnifiedTorrentRow) {
    const state = torrent.state.toLowerCase();
    if (torrent.progress >= 1 && state.includes("up")) return "Seeding stalled";
    if (torrent.progress >= 1 && state.includes("complet")) return "Complete";
    if (state.includes("meta")) return "Waiting for metadata";
    if (state.includes("stall")) return "Waiting for peers";
    if (state.includes("pause")) return "Paused";
    if (state.includes("check")) return "Checking";
    return "Downloading";
  }

  function stateVariant(torrent: UnifiedTorrentRow): BadgeVariant {
    const state = torrent.state.toLowerCase();
    if (isPaused(torrent)) return "grey";
    if (torrent.progress >= 1) return "success";
    if (state.includes("error")) return "danger";
    if (state.includes("stall")) return "warning";
    return "primary";
  }

  /** Only incomplete, running downloads show DL speed and ETA. */
  function isDownloading(torrent: UnifiedTorrentRow) {
    return torrent.progress < 1 && !isPaused(torrent) && !torrent.state.toLowerCase().includes("up");
  }

  function payloadStatus(torrent: UnifiedTorrentRow) {
    return `${stateLabel(torrent)}${torrent.progress >= 1 ? " · complete" : ""}`;
  }

  function toggleSort(key: SortKey) {
    sortDirection = sortKey === key && sortDirection === 1 ? -1 : 1;
    sortKey = key;
  }

  function handleRowKeydown(e: KeyboardEvent, torrent: UnifiedTorrentRow) {
    if (e.key === "Enter") {
      e.preventDefault();
      onOpenDetails(torrent);
    }
  }

  function toggleExpanded(torrent: UnifiedTorrentRow) {
    const next = new Set(expanded);
    if (next.has(torrent.hash)) next.delete(torrent.hash);
    else next.add(torrent.hash);
    expanded = next;
  }

  function payloadLabel(item: PayloadItem) {
    const season = Number(item.season || 0);
    const episode = Number(item.episode || 0);
    return season || episode
      ? `S${String(season).padStart(2, "0")}E${String(episode).padStart(2, "0")} · ${String(item.title || `Episode ${episode}`)}`
      : String(item.title || item.filename || "Payload item");
  }

  function updateScrollEnd(e: Event) {
    const el = e.currentTarget as HTMLElement;
    scrolledToEnd = el.scrollLeft + el.clientWidth >= el.scrollWidth - 1;
  }

  function handleScrollKeydown(e: KeyboardEvent) {
    const el = e.currentTarget as HTMLElement;
    if (e.key === "End") {
      e.preventDefault();
      el.scrollLeft = el.scrollWidth;
    } else if (e.key === "Home") {
      e.preventDefault();
      el.scrollLeft = 0;
    }
    updateScrollEnd(e);
  }

  async function openMenu(e: MouseEvent, torrent: UnifiedTorrentRow) {
    e.preventDefault();
    e.stopPropagation();
    menuTorrent = torrent;
    menuTrigger = e.currentTarget as HTMLElement;
    menuPosition = { x: e.clientX, y: e.clientY };
    await tick();
    const rect = menuEl?.getBoundingClientRect();
    if (rect) {
      // Keep the menu on screen; leave room for the mobile bottom nav.
      const compact = matchMedia("(max-width:720px), (pointer:coarse) and (max-height:500px)").matches;
      const bottom = window.innerHeight - (compact ? 70 : 10);
      menuPosition = {
        x: Math.max(10, Math.min(menuPosition.x, window.innerWidth - rect.width - 10)),
        y: Math.max(10, Math.min(menuPosition.y, bottom - rect.height)),
      };
    }
    menuEl?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
  }

  function closeMenu() {
    const trigger = menuTrigger;
    menuTorrent = null;
    menuTrigger = null;
    trigger?.focus();
  }

  /** Outside-pointer close and arrow-key navigation for the context menu. */
  function menuKeyboard(node: HTMLElement) {
    const onPointerDown = (e: PointerEvent) => {
      if (!node.contains(e.target as Node)) closeMenu();
    };
    const onKeydown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        closeMenu();
        return;
      }
      const items = [...node.querySelectorAll<HTMLElement>('[role="menuitem"]')];
      const index = items.indexOf(document.activeElement as HTMLElement);
      let next: number | undefined;
      if (e.key === "ArrowDown") next = (index + 1) % items.length;
      else if (e.key === "ArrowUp") next = (index - 1 + items.length) % items.length;
      else if (e.key === "Home") next = 0;
      else if (e.key === "End") next = items.length - 1;
      if (next !== undefined) {
        e.preventDefault();
        items[next]?.focus();
      }
    };
    document.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", onKeydown);
    return {
      destroy() {
        document.removeEventListener("pointerdown", onPointerDown, true);
        window.removeEventListener("keydown", onKeydown);
      },
    };
  }

  async function runAction(action: string, torrent: UnifiedTorrentRow) {
    await qbitStore.action(action, [torrent.hash]);
    await qbitStore.fetchTorrents();
    closeMenu();
  }
</script>

<div
  class="queue-panel"
  role="presentation"
  onclick={closeMenu}
  onkeydown={(e) => e.key === "Escape" && closeMenu()}
>
  <div
    class="table-scroll"
    class:scroll-end={scrolledToEnd}
    role="region"
    tabindex="0"
    aria-label="Torrent table. Scroll horizontally for more columns."
    onscroll={updateScrollEnd}
    onkeydown={handleScrollKeydown}
  >
    <table class="data-table">
      <thead>
        <tr>
          {#each COLUMNS as column}
            {#if visibleColumns.has(column[1])}
              <th
                aria-sort={sortKey === column[0] ? (sortDirection === 1 ? "ascending" : "descending") : undefined}
              >
                <button class="sort" onclick={() => toggleSort(column[0])}
                  >{column[1]}{sortKey === column[0] ? (sortDirection === 1 ? " ↑" : " ↓") : ""}</button
                >
              </th>
            {/if}
          {/each}
        </tr>
      </thead>
      <tbody>
        {#if qbitStore.loading && sortedTorrents.length === 0}
          <tr><td colspan="10" class="empty-cell">Loading torrents…</td></tr>
        {:else if sortedTorrents.length === 0}
          <tr><td colspan="10" class="empty-cell">No torrents match the current filter.</td></tr>
        {:else}
          {#each sortedTorrents as torrent (`${torrent.instance_id}:${torrent.hash}`)}
            <tr
              tabindex="0"
              aria-label={`Torrent ${torrent.name}. Press Enter for details.`}
              class:inspected={detailsTorrent?.hash === torrent.hash &&
                detailsTorrent?.instance_id === torrent.instance_id}
              oncontextmenu={(e) => openMenu(e, torrent)}
              onkeydown={(e) => handleRowKeydown(e, torrent)}
            >
              <td class="name-cell" title={torrent.name}>
                <div class="name-line">
                  {#if torrent.payload?.length}<button
                      class="expand-btn"
                      aria-label={`${expanded.has(torrent.hash) ? "Collapse" : "Expand"} ${torrent.media_title || torrent.name}`}
                      aria-expanded={expanded.has(torrent.hash)}
                      onclick={(e) => {
                        e.stopPropagation();
                        toggleExpanded(torrent);
                      }}
                      ><span class="material-icons" aria-hidden="true"
                        >{expanded.has(torrent.hash) ? "expand_more" : "chevron_right"}</span
                      ></button
                    >{/if}<button class="name-btn" onclick={() => onOpenDetails(torrent)}
                    >{torrent.media_title || torrent.name}</button
                  >
                </div>
                <div class="meta-line">{torrent.name}</div>
              </td>
              {#if visibleColumns.has("Type")}
                <td data-label="Type"
                  >{torrent.media_type === "tv" ? "TV SHOW" : torrent.media_type === "movie" ? "MOVIE" : "UNRESOLVED"}</td
                >
              {/if}
              <td data-label="State">
                <Badge
                  text={stateLabel(torrent)}
                  title={torrent.state}
                  variant={stateVariant(torrent)}
                  size="sm"
                  noDot
                />
              </td>
              <td data-label="Size">{formatTorrentBytes(torrent.size)}</td>
              <td data-label="Progress">
                <div class="progress-bar"><span style:width={`${Math.round(torrent.progress * 100)}%`}></span></div>
                <small>{formatTorrentProgress(torrent.progress)}</small>
              </td>
              <td data-label="DL">{isDownloading(torrent) ? formatTorrentSpeed(torrent.dlspeed) : "—"}</td>
              <td data-label="UL">{formatTorrentSpeed(torrent.upspeed)}</td>
              <td data-label="ETA">{isDownloading(torrent) ? formatTorrentEta(torrent.eta) : "—"}</td>
              <td data-label="Ratio">{Number.isFinite(torrent.ratio) ? torrent.ratio.toFixed(2) : "—"}</td>
              <td data-label="Seeds/Peers">{torrent.num_seeds}/{torrent.num_leechs}</td>
              {#if visibleColumns.has("Added on")}
                <td data-label="Added on">{new Date(torrent.added_on * 1000).toLocaleString()}</td>
              {/if}
              <td class="mobile-more-cell">
                <button
                  class="mobile-more"
                  aria-label={`More actions for ${torrent.media_title || torrent.name}`}
                  onclick={(e) => openMenu(e, torrent)}>More</button
                >
              </td>
            </tr>
            {#if expanded.has(torrent.hash)}
              {#each (torrent.payload as PayloadItem[]) || [] as item}
                <tr class="payload-row">
                  <td class="payload-name"
                    ><strong>{payloadLabel(item)}</strong><small>{String(item.filename || "")}</small></td
                  >
                  <td>{item.kind === "subtitle" ? "SUBTITLE" : "EPISODE"}</td>
                  <td colspan="9">{payloadStatus(torrent)}</td>
                </tr>
              {/each}
            {/if}
          {/each}
        {/if}
      </tbody>
    </table>
  </div>

  <footer class="queue-footer">
    <span>Showing {sortedTorrents.length} torrents</span><span class="transfer-total"
      >↓ {formatTorrentSpeed(sortedTorrents.reduce((sum, t) => sum + t.dlspeed, 0))} &nbsp; ↑ {formatTorrentSpeed(
        sortedTorrents.reduce((sum, t) => sum + t.upspeed, 0),
      )}</span
    ><span class="page-box">1 / 1</span>
  </footer>

  {#if menuTorrent}
    <div
      bind:this={menuEl}
      use:menuKeyboard
      class="ctx-menu"
      role="menu"
      tabindex="-1"
      style:left={`${menuPosition.x}px`}
      style:top={`${menuPosition.y}px`}
    >
      <div class="menu-group">
        <span class="menu-label">Transfer</span>
        {#if isPaused(menuTorrent)}
          <button role="menuitem" onclick={() => runAction("resume", menuTorrent!)}
            ><span class="material-icons" aria-hidden="true">play_arrow</span>Resume</button
          >
        {:else}
          <button role="menuitem" onclick={() => runAction("pause", menuTorrent!)}
            ><span class="material-icons" aria-hidden="true">pause</span>Pause</button
          >
        {/if}
        <button role="menuitem" onclick={() => runAction("recheck", menuTorrent!)}
          ><span class="material-icons" aria-hidden="true">fact_check</span>Recheck</button
        >
        <button role="menuitem" onclick={() => runAction("reannounce", menuTorrent!)}
          ><span class="material-icons" aria-hidden="true">campaign</span>Reannounce</button
        >
      </div>
      <div class="menu-group">
        <span class="menu-label">Organize</span>
        <button role="menuitem" onclick={() => onOpenIntent({ kind: "labels", mode: "category", torrent: menuTorrent })}
          ><span class="material-icons" aria-hidden="true">folder</span>Category</button
        >
        <button role="menuitem" onclick={() => onOpenIntent({ kind: "labels", mode: "tags", torrent: menuTorrent })}
          ><span class="material-icons" aria-hidden="true">sell</span>Tags</button
        >
        <button role="menuitem" onclick={() => onOpenIntent({ kind: "limits", torrent: menuTorrent })}
          ><span class="material-icons" aria-hidden="true">speed</span>Limits</button
        >
        <button role="menuitem" onclick={() => onOpenIntent({ kind: "move", torrent: menuTorrent })}
          ><span class="material-icons" aria-hidden="true">drive_file_move</span>Move</button
        >
      </div>
      <div class="menu-group">
        <span class="menu-label">Inspect</span>
        <button role="menuitem" onclick={() => onOpenDetails(menuTorrent!)}
          ><span class="material-icons" aria-hidden="true">info</span>Details</button
        >
      </div>
      <div class="menu-group menu-danger">
        <button role="menuitem" class="danger-item" onclick={() => onOpenIntent({ kind: "delete", torrent: menuTorrent })}
          ><span class="material-icons" aria-hidden="true">delete</span>Delete</button
        >
      </div>
    </div>
  {/if}
</div>

<style>
	.queue-panel {
		flex: 1;
		min-height: 0;
		overflow: hidden;
		display: flex;
		flex-direction: column;
	}

	.table-scroll {
		flex: 1;
		min-height: 0;
		overflow: auto;
		-webkit-overflow-scrolling: touch;
		scrollbar-color: rgba(0,243,255,.35) transparent;
		scrollbar-width: thin;
	}

	table {
		width: 100%;
		min-width: 1480px;
		border-collapse: collapse;
		table-layout: fixed;
	}

	thead {
		position: sticky;
		top: 0;
		z-index: 10;
		background: #0f172af2;
		-webkit-backdrop-filter: blur(10px);
		backdrop-filter: blur(10px);
	}

	th,
	td {
		height: 49px;
		padding: .62rem 1rem;
		border-bottom: 1px solid rgba(255,255,255,.05);
		text-align: left;
		font-size: .72rem;
		vertical-align: middle;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	td:first-child,
	th:first-child {
		width: 400px;
	}

	th:not(:first-child),
	td:not(:first-child) {
		white-space: nowrap;
	}

	th:nth-child(2),
	td:nth-child(2) {
		width: 92px;
	}

	th:nth-child(3),
	td:nth-child(3) {
		width: 180px;
	}

	th:nth-child(4),
	td:nth-child(4) {
		width: 84px;
	}

	th:nth-child(5),
	td:nth-child(5) {
		width: 150px;
	}

	th:nth-child(6),
	td:nth-child(6),
	th:nth-child(7),
	td:nth-child(7) {
		width: 101px;
	}

	th:nth-child(8),
	td:nth-child(8) {
		width: 90px;
	}

	th:nth-child(9),
	td:nth-child(9) {
		width: 82px;
	}

	th:nth-child(10),
	td:nth-child(10) {
		width: 116px;
	}

	th:nth-child(11),
	td:nth-child(11) {
		width: 165px;
	}

	th:nth-child(n + 4):nth-child(-n + 10),
	td:nth-child(n + 4):nth-child(-n + 10) {
		text-align: right;
		font-variant-numeric: tabular-nums;
	}

	.name-line {
		padding-left: 24px;
		position: relative;
	}

	.expand-btn {
		position: absolute;
		left: 0;
	}

	th {
		color: var(--cui-text-secondary);
		text-transform: uppercase;
		letter-spacing: .08em;
		font: 800 .7rem var(--cui-font-mono);
		border-bottom: 2px solid rgba(255,255,255,.08);
	}

	.sort {
		background: none;
		border: 0;
		color: inherit;
		font: inherit;
		text-transform: inherit;
		letter-spacing: inherit;
		cursor: pointer;
		padding: 0;
		min-height: var(--cui-touch-min);
	}

	tbody tr {
		cursor: pointer;
		transition: background .15s;
	}

	tbody tr:hover {
		background: #00f3ff09;
	}

	tbody tr.inspected {
		background: var(--cui-surface-active);
	}

	.name-cell {
		display: flex;
		flex-direction: column;
		gap: .35rem;
		min-width: 0;
	}

	.name-line {
		display: flex;
		align-items: center;
		min-width: 0;
	}

	.expand-btn {
		width: 24px;
		height: 24px;
		flex: none;
		display: grid;
		place-items: center;
		border: 0;
		background: transparent;
		color: var(--cui-primary);
		cursor: pointer;
	}

	.expand-btn .material-icons {
		font-size: 16px;
	}

	.payload-row {
		background: #00f3ff06;
	}

	.payload-row td {
		height: 38px;
		color: var(--cui-text-tertiary);
	}

	.payload-name {
		padding-left: 36px !important;
	}

	.payload-name strong {
		display: block;
		color: var(--cui-text-primary);
		font-size: .7rem;
	}

	.payload-name small {
		display: block;
		font-size: .62rem;
		margin-top: 2px;
	}

	.name-btn {
		background: none;
		border: 0;
		color: var(--cui-text-primary);
		font-weight: 600;
		text-align: left;
		cursor: pointer;
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		padding: 0;
		font-size: .78rem;
	}

	.name-btn:hover {
		color: var(--cui-primary);
	}

	.meta-line {
		font-size: .68rem;
		color: var(--cui-text-secondary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.progress-bar {
		width: 100%;
		height: 4px;
		background: var(--cui-surface-hover);
		overflow: hidden;
	}

	.progress-bar span {
		display: block;
		height: 100%;
		background: var(--cui-secondary);
	}

	.ctx-menu {
		position: fixed;
		z-index: calc(var(--cui-z-overlay) + 1);
		min-width: 210px;
		max-height: calc(100dvh - 20px);
		overflow: auto;
		background: var(--cui-glass-heavy);
		border: 1px solid var(--cui-glass-border);
		border-radius: var(--cui-radius-md);
		padding: .35rem;
		box-shadow: var(--cui-shadow-glass);
	}

	.menu-group {
		padding: .25rem 0;
	}

	.menu-group + .menu-group {
		border-top: 1px solid var(--cui-glass-border);
	}

	.menu-label {
		display: block;
		padding: .2rem .65rem .35rem;
		color: var(--cui-text-tertiary);
		font-family: var(--cui-font-mono);
		font-size: .59rem;
		letter-spacing: .09em;
		text-transform: uppercase;
	}

	.ctx-menu button {
		width: 100%;
		display: flex;
		align-items: center;
		gap: .6rem;
		background: transparent;
		border: 0;
		color: var(--cui-text-primary);
		text-align: left;
		padding: .58rem .65rem;
		border-radius: var(--cui-radius-sm);
		cursor: pointer;
		font-size: .78rem;
	}

	.ctx-menu button:hover,
	.ctx-menu button:focus-visible {
		background: var(--cui-surface-active);
		color: var(--cui-text-primary);
		outline: none;
	}

	.ctx-menu button .material-icons {
		font-size: 1rem;
		color: var(--cui-text-secondary);
	}

	.ctx-menu .danger-item {
		color: var(--cui-danger);
	}

	.ctx-menu .danger-item:hover {
		background: #ff52521f;
		color: var(--cui-danger);
	}

	.empty-cell {
		text-align: center;
		padding: 2rem;
		color: var(--cui-text-tertiary);
	}

	.queue-footer {
		margin-top: auto;
		min-height: 50px;
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 20px;
		padding: 0 16px;
		border-top: 1px solid var(--cui-glass-border);
		background: #0f172ab8;
		font: 600 .68rem var(--cui-font-mono);
		color: var(--cui-text-tertiary);
	}

	.transfer-total {
		color: var(--cui-text-secondary);
	}

	.page-box {
		height: 30px;
		min-width: 58px;
		display: grid;
		place-items: center;
		border: 1px solid var(--cui-glass-border);
		color: var(--cui-primary);
	}

	.table-scroll:not(.scroll-end) {
		box-shadow: inset -24px 0 18px -22px #00f3ffa6;
	}

	.table-scroll:focus-visible {
		outline: 2px solid var(--cui-primary);
		outline-offset: -2px;
	}

	.mobile-more-cell {
		display: none;
	}

	.mobile-more {
		min-width: 44px;
		min-height: 44px;
		background: var(--cui-surface-active);
		border: 1px solid var(--cui-glass-border);
		color: var(--cui-text-primary);
		border-radius: var(--cui-radius-sm);
	}

	@media (max-width:720px),(pointer:coarse)and (max-height:500px) {
		.ctx-menu {
			max-height: calc(100dvh - 80px - env(safe-area-inset-bottom,0px));
			max-width: calc(100vw - 20px);
			overflow-y: auto;
			overscroll-behavior: contain;
		}

		.ctx-menu button {
			min-height: 44px;
		}

		.mobile-more-cell {
			display: table-cell;
			position: sticky;
			right: 0;
			background: var(--cui-glass-heavy);
		}

		.queue-panel table,
		.queue-panel tbody,
		.queue-panel tr,
		.queue-panel td {
			display: block;
			width: 100%;
		}

		.queue-panel thead {
			display: none;
		}

		.queue-panel tbody {
			display: flex;
			flex-direction: column;
			gap: .75rem;
			padding: .75rem .75rem calc(70px + env(safe-area-inset-bottom,0px));
		}

		.queue-panel tbody tr {
			position: relative;
			border: 1px solid var(--cui-glass-border);
			border-radius: var(--cui-radius-md);
			background: var(--cui-surface-1);
			padding: .9rem .9rem .75rem;
		}

		.queue-panel tbody tr.inspected {
			background: #00f3ff14;
		}

		.queue-panel td {
			border: 0;
			padding: .35rem 0;
		}

		.queue-panel .name-btn {
			max-width: none;
			white-space: normal;
			word-break: break-word;
			padding-right: 2.25rem;
		}

		.queue-panel .meta-line {
			padding-right: 2.25rem;
			white-space: normal;
			word-break: break-word;
		}

		.queue-panel td[data-label] {
			display: grid;
			grid-template-columns: 72px 1fr;
			gap: .5rem;
			align-items: center;
		}

		.queue-panel td[data-label]:before {
			display: block;
			content: attr(data-label);
			color: var(--cui-text-secondary);
			text-transform: uppercase;
			letter-spacing: .08em;
			font-size: .62rem;
		}

		.queue-panel .progress-bar {
			width: 100%;
		}

		.queue-footer {
			display: none;
		}
	}
</style>
