<script lang="ts">
  import { Badge, Button, ErrorState } from "@media-set/core-ui";
  import { qbitStore } from "$lib/stores/qbit.svelte";
  import type { UnifiedTorrentRow } from "$lib/stores/qbitReducer";
  import { toasts } from "$lib/stores/toasts";
  import {
    downsamplePieces,
    fileBaseName,
    formatTorrentBytes,
    formatTorrentDate,
    formatTorrentEta,
    formatTorrentProgress,
    formatTorrentSpeed,
    parentPath,
    peerEndpoint,
    peerEntries,
    peerFlags,
    peerList,
    pickField,
    pieceStateClass,
    trackerHost,
    trackerStatusLabel,
    trackerStatusTone,
  } from "./torrentFormat";
  import type { TorrentIntent } from "./torrentIntent";

  type InspectorTab = "general" | "trackers" | "files" | "peers";

  interface Props {
    detailsTorrent: UnifiedTorrentRow | null;
    onClose?: () => void;
    onOpenIntent: (intent: TorrentIntent) => void;
  }

  let { detailsTorrent, onClose, onOpenIntent }: Props = $props();

  const TABS: InspectorTab[] = ["general", "trackers", "files", "peers"];
  const MAX_PIECE_CELLS = 160;

  /** `instance:hash` of the inspected torrent (resets the tab when it changes). */
  let inspectedKey = $state("");
  let activeTab = $state<InspectorTab>("general");
  // Per `instance:hash:tab` detail payloads and their loading/error flags.
  let details = $state<Record<string, any>>({});
  let detailLoading = $state<Record<string, boolean>>({});
  let detailErrors = $state<Record<string, string>>({});
  // Per `instance:hash:piece-states` downsampled piece states.
  let pieces = $state<Record<string, number[]>>({});
  let pieceLoading = $state<Record<string, boolean>>({});
  let pieceErrors = $state<Record<string, string>>({});
  let peerInput = $state("");
  let peerBusy = $state(false);

  $effect(() => {
    const key = detailsTorrent ? `${detailsTorrent.instance_id}:${detailsTorrent.hash}` : "";
    if (key !== inspectedKey) {
      inspectedKey = key;
      activeTab = "general";
      peerInput = "";
    }
  });

  $effect(() => {
    if (detailsTorrent) loadTab(activeTab);
  });

  $effect(() => {
    if (detailsTorrent && activeTab === "general") loadPieces();
  });

  function cacheKey(tab: InspectorTab = activeTab) {
    return detailsTorrent ? `${detailsTorrent.instance_id}:${detailsTorrent.hash}:${tab}` : "";
  }

  function pieceCacheKey() {
    return detailsTorrent ? `${detailsTorrent.instance_id}:${detailsTorrent.hash}:piece-states` : "";
  }

  function tabId(tab: InspectorTab) {
    return `torrent-inspector-tab-${tab}`;
  }

  function panelId(tab: InspectorTab) {
    return `torrent-inspector-panel-${tab}`;
  }

  function selectTab(tab: InspectorTab) {
    activeTab = tab;
  }

  function focusTab(tab: InspectorTab) {
    if (typeof document === "undefined") return;
    document.getElementById(tabId(tab))?.focus();
  }

  function handleTabKeydown(e: KeyboardEvent, index: number) {
    const key = e.key;
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(key)) return;
    e.preventDefault();
    const nextIndex =
      key === "Home"
        ? 0
        : key === "End"
          ? TABS.length - 1
          : (index + (key === "ArrowRight" ? 1 : -1) + TABS.length) % TABS.length;
    const tab = TABS[nextIndex];
    selectTab(tab);
    focusTab(tab);
  }

  async function loadTab(tab: InspectorTab) {
    if (!detailsTorrent) return;
    const key = cacheKey(tab);
    if (details[key] || detailLoading[key]) return;
    detailLoading = { ...detailLoading, [key]: true };
    detailErrors = { ...detailErrors, [key]: "" };
    try {
      const kind = tab === "general" ? "properties" : tab;
      details = { ...details, [key]: await qbitStore.detail(detailsTorrent.hash, kind) };
    } catch (err) {
      detailErrors = {
        ...detailErrors,
        [key]: err instanceof Error ? err.message : "Failed to load details",
      };
    } finally {
      detailLoading = { ...detailLoading, [key]: false };
    }
  }

  async function refreshTab(tab: InspectorTab) {
    if (!detailsTorrent) return;
    const kind = tab === "general" ? "properties" : tab;
    qbitStore.invalidateDetail(detailsTorrent.hash, kind);
    const key = cacheKey(tab);
    const nextDetails = { ...details };
    delete nextDetails[key];
    details = nextDetails;
    if (tab === "general") {
      qbitStore.invalidateDetail(detailsTorrent.hash, "piece-states");
      const nextPieces = { ...pieces };
      delete nextPieces[pieceCacheKey()];
      pieces = nextPieces;
    }
    await loadTab(tab);
  }

  async function loadPieces() {
    if (!detailsTorrent) return;
    const key = pieceCacheKey();
    if (!key || pieces[key] || pieceLoading[key]) return;
    pieceLoading = { ...pieceLoading, [key]: true };
    pieceErrors = { ...pieceErrors, [key]: "" };
    try {
      const states = await qbitStore.detail(detailsTorrent.hash, "piece-states");
      pieces = {
        ...pieces,
        [key]: downsamplePieces(Array.isArray(states) ? states : [], MAX_PIECE_CELLS),
      };
    } catch (err) {
      pieceErrors = {
        ...pieceErrors,
        [key]: err instanceof Error ? err.message : "Failed to load piece states",
      };
    } finally {
      pieceLoading = { ...pieceLoading, [key]: false };
    }
  }

  async function reannounce() {
    if (!detailsTorrent) return;
    await qbitStore.action("reannounce", [detailsTorrent.hash]);
    await refreshTab("trackers");
  }

  async function setPriority(file: any, priority: number) {
    if (!detailsTorrent) return;
    const fileIndex = Number(pickField(file, ["index", "id"], -1));
    if (!Number.isInteger(fileIndex) || fileIndex < 0) return toasts.error("Invalid file index");
    await qbitStore.setFilePriority(detailsTorrent.hash, fileIndex, priority);
    await refreshTab("files");
  }

  async function addPeers() {
    if (!detailsTorrent) return;
    const peers = peerInput.trim();
    if (!peers) return toasts.error("Peer address is required");
    peerBusy = true;
    try {
      if ((await qbitStore.addPeers(detailsTorrent.hash, peers))?.success) {
        peerInput = "";
        await refreshTab("peers");
      }
    } finally {
      peerBusy = false;
    }
  }

  async function banPeer(peer: any) {
    if (!detailsTorrent) return;
    const endpoint = peerEndpoint(peer);
    if (endpoint === "—") return toasts.error("Peer endpoint missing");
    if (!confirm(`Ban peer ${endpoint}? This affects the qBittorrent instance globally.`)) return;
    peerBusy = true;
    try {
      if ((await qbitStore.banPeers(endpoint))?.success) await refreshTab("peers");
    } finally {
      peerBusy = false;
    }
  }
</script>

<aside class="bento-card inspector-panel">
  {#if detailsTorrent}
    {@const key = cacheKey()}
    {@const payload = details[key] || {}}
    {@const pieceKey = pieceCacheKey()}
    <div class="bento-head cyan-accent">
      <div class="inspector-name">
        <span class="material-icons">info_outline</span>
        <span class="inspector-title">{detailsTorrent.name}</span>
      </div>
      <div class="inspector-head-actions">
        <Button variant="ghost" size="sm" onclick={() => refreshTab(activeTab)}>Refresh</Button>
        <button class="icon-btn" aria-label="Close inspector" onclick={onClose}
          ><span class="material-icons">close</span></button
        >
      </div>
    </div>
    <p class="hash-line">{detailsTorrent.hash}</p>

    <div class="detail-tabs" role="tablist" aria-label="Torrent details tabs">
      {#each TABS as tab, index}
        <button
          role="tab"
          type="button"
          id={tabId(tab)}
          aria-selected={activeTab === tab}
          aria-controls={panelId(tab)}
          tabindex={activeTab === tab ? 0 : -1}
          class:active={activeTab === tab}
          onclick={() => selectTab(tab)}
          onkeydown={(e) => handleTabKeydown(e, index)}>{tab}</button
        >
      {/each}
    </div>

    {#if detailLoading[key]}
      <div class="detail-panel" role="tabpanel" id={panelId(activeTab)} aria-labelledby={tabId(activeTab)} tabindex="0">
        <p class="empty-cell">Loading {activeTab}…</p>
      </div>
    {:else if detailErrors[key]}
      <div class="detail-panel" role="tabpanel" id={panelId(activeTab)} aria-labelledby={tabId(activeTab)} tabindex="0">
        <ErrorState
          title={`${activeTab} load failed`}
          message={detailErrors[key]}
          showRetry={true}
          onRetry={() => refreshTab(activeTab)}
        />
      </div>
    {:else if activeTab === "general"}
      <div class="detail-panel" role="tabpanel" id={panelId("general")} aria-labelledby={tabId("general")} tabindex="0">
        <dl class="detail-dl">
          <dt>State</dt><dd>{detailsTorrent.state}</dd>
          <dt>Progress</dt><dd>{formatTorrentProgress(detailsTorrent.progress)}</dd>
          <dt>Size</dt><dd>
            {formatTorrentBytes(Number(pickField(payload, ["total_size", "size"], detailsTorrent.size)))}
          </dd>
          <dt>Save path</dt><dd class="break-all">{pickField(payload, ["save_path"], detailsTorrent.save_path)}</dd>
          <dt>Content path</dt><dd class="break-all">
            {pickField(payload, ["content_path"], detailsTorrent.content_path)}
          </dd>
          <dt>Ratio</dt><dd>{Number(pickField(payload, ["share_ratio", "ratio"], detailsTorrent.ratio)).toFixed(2)}</dd>
          <dt>Downloaded</dt><dd>
            {formatTorrentBytes(
              Number(pickField(payload, ["total_downloaded", "downloaded"], detailsTorrent.downloaded)),
            )}
          </dd>
          <dt>Uploaded</dt><dd>
            {formatTorrentBytes(Number(pickField(payload, ["total_uploaded", "uploaded"], detailsTorrent.uploaded)))}
          </dd>
          <dt>DL speed</dt><dd>
            {formatTorrentSpeed(Number(pickField(payload, ["dl_speed", "dlspeed"], detailsTorrent.dlspeed)))}
          </dd>
          <dt>UL speed</dt><dd>
            {formatTorrentSpeed(Number(pickField(payload, ["up_speed", "upspeed"], detailsTorrent.upspeed)))}
          </dd>
          <dt>ETA</dt><dd>{formatTorrentEta(Number(pickField(payload, ["eta"], detailsTorrent.eta)))}</dd>
          <dt>Added</dt><dd>
            {formatTorrentDate(Number(pickField(payload, ["addition_date", "added_on"], detailsTorrent.added_on)))}
          </dd>
          <dt>Completed</dt><dd>
            {formatTorrentDate(
              Number(pickField(payload, ["completion_date", "completion_on"], detailsTorrent.completion_on)),
            )}
          </dd>
        </dl>
        <section class="piece-section">
          <div class="piece-head">
            <span class="detail-label">Pieces</span>
            <Button variant="ghost" size="sm" onclick={() => refreshTab("general")}>Refresh</Button>
          </div>
          {#if pieceLoading[pieceKey]}
            <p class="empty-cell">Loading pieces…</p>
          {:else if pieceErrors[pieceKey]}
            <p class="muted-text">{pieceErrors[pieceKey]}</p>
          {:else if (pieces[pieceKey] || []).length === 0}
            <p class="muted-text">No piece data.</p>
          {:else}
            <div class="piece-bar" aria-label="Piece completion bar">
              {#each pieces[pieceKey] as state}
                <span class={pieceStateClass(state)}></span>
              {/each}
            </div>
          {/if}
        </section>
      </div>
    {:else if activeTab === "trackers"}
      <div class="detail-panel" role="tabpanel" id={panelId("trackers")} aria-labelledby={tabId("trackers")} tabindex="0">
        <div class="detail-subhead">
          <span class="muted-text">Tracker health, swarm counts, announce errors.</span>
          <Button variant="ghost" size="sm" onclick={reannounce}>Reannounce</Button>
        </div>
        {#if peerList(payload).length === 0}
          <p class="empty-cell">No trackers.</p>
        {:else}
          <div class="detail-scroll">
            <table class="detail-table">
              <thead>
                <tr><th>Domain</th><th>URL</th><th>Status</th><th>Seeds</th><th>Peers</th><th>Msg</th></tr>
              </thead>
              <tbody>
                {#each peerList(payload) as tracker}
                  <tr>
                    <td>{trackerHost(pickField(tracker, ["url"], "—"))}</td>
                    <td class="tracker-url">{pickField(tracker, ["url"], "—")}</td>
                    <td>
                      <Badge
                        text={trackerStatusLabel(tracker.status)}
                        variant={trackerStatusTone(tracker.status) === "ok"
                          ? "success"
                          : trackerStatusTone(tracker.status) === "warn"
                            ? "warning"
                            : trackerStatusTone(tracker.status) === "error"
                              ? "danger"
                              : "grey"}
                        size="xs"
                        noDot
                      />
                    </td>
                    <td>{pickField(tracker, ["num_seeds", "seeds"], "—")}</td>
                    <td>{pickField(tracker, ["num_peers", "peers"], "—")}</td>
                    <td>{pickField(tracker, ["msg", "message"], "—")}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </div>
    {:else if activeTab === "files"}
      <div class="detail-panel" role="tabpanel" id={panelId("files")} aria-labelledby={tabId("files")} tabindex="0">
        {#if peerList(payload).length === 0}
          <p class="empty-cell">No files.</p>
        {:else}
          <div class="detail-scroll">
            <table class="detail-table">
              <thead><tr><th>Name</th><th>Size</th><th>Progress</th><th>Priority</th></tr></thead>
              <tbody>
                {#each peerList(payload) as file}
                  <tr>
                    <td>
                      <div class="file-cell">
                        <strong>{pickField(file, ["name"], "—")}</strong>
                        <small class="muted-text">{parentPath(pickField(file, ["name"], ""))}</small>
                      </div>
                    </td>
                    <td>{formatTorrentBytes(Number(file.size || 0))}</td>
                    <td>{formatTorrentProgress(Number(file.progress || 0))}</td>
                    <td>
                      <div class="file-actions">
                        <select
                          class="priority-select"
                          aria-label={`Priority for ${pickField(file, ["name"], "file")}`}
                          value={Number(pickField(file, ["priority"], 1))}
                          onchange={(e) => setPriority(file, Number(e.currentTarget.value))}
                        >
                          <option value={0}>skip</option>
                          <option value={1}>normal</option>
                          <option value={6}>high</option>
                          <option value={7}>max</option>
                        </select>
                        <Button
                          variant="ghost"
                          size="sm"
                          onclick={() =>
                            onOpenIntent({
                              kind: "rename",
                              mode: "file",
                              hash: detailsTorrent.hash,
                              label: "file",
                              currentName: fileBaseName(pickField(file, ["name"], "")),
                              oldPath: pickField(file, ["name"], ""),
                            })}>Rename</Button
                        >
                      </div>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </div>
    {:else if activeTab === "peers"}
      <div class="detail-panel" role="tabpanel" id={panelId("peers")} aria-labelledby={tabId("peers")} tabindex="0">
        <div class="detail-subhead peer-head">
          <span class="muted-text">Inspect peers, add explicit peers, ban bad actors.</span>
          <div class="peer-controls">
            <input
              class="filter-input"
              placeholder="host:port …"
              aria-label="Peer addresses"
              onkeydown={(e) => e.key === "Enter" && addPeers()}
              bind:value={peerInput}
            />
            <Button variant="primary" size="sm" onclick={addPeers} disabled={peerBusy || !peerInput.trim()}
              >Add peer</Button
            >
          </div>
        </div>
        {#if peerEntries(payload).length === 0}
          <p class="empty-cell">No peers.</p>
        {:else}
          <div class="detail-scroll">
            <table class="detail-table">
              <thead>
                <tr><th>Endpoint</th><th>Client</th><th>Flags</th><th>Progress</th><th>Down</th><th>Up</th><th></th></tr>
              </thead>
              <tbody>
                {#each peerEntries(payload) as peer}
                  <tr>
                    <td>{peerEndpoint(peer)}</td>
                    <td>{pickField(peer, ["client"], "—")}</td>
                    <td>{peerFlags(peer)}</td>
                    <td>{formatTorrentProgress(Number(peer.progress || 0))}</td>
                    <td>{formatTorrentSpeed(Number(peer.dl_speed || 0))}</td>
                    <td>{formatTorrentSpeed(Number(peer.up_speed || 0))}</td>
                    <td>
                      <Button variant="danger" size="sm" onclick={() => banPeer(peer)} disabled={peerBusy}>Ban</Button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </div>
    {/if}
  {:else}
    <div class="bento-card inspector-empty">
      <span class="material-icons empty-icon">info_outline</span>
      <p class="eyebrow">Inspector</p>
      <p class="muted-text">
        Open a torrent name or choose Details to inspect trackers, files, peers, and piece state.
      </p>
    </div>
  {/if}
</aside>

<style>
	.inspector-panel {
		padding: 0;
		position: sticky;
		top: 1rem;
		max-height: calc(100vh - 2rem);
		overflow: auto;
		overscroll-behavior: contain;
		display: flex;
		flex-direction: column;
	}

	.inspector-name {
		display: flex;
		align-items: center;
		gap: .5rem;
		overflow: hidden;
		flex: 1;
		min-width: 0;
	}

	.inspector-title {
		font-size: .8rem;
		font-weight: 700;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		color: var(--cui-text-primary);
	}

	.inspector-head-actions {
		display: flex;
		gap: .4rem;
		flex-shrink: 0;
	}

	.hash-line {
		font-family: var(--cui-font-mono);
		font-variant-numeric: tabular-nums;
		font-size: .65rem;
		color: var(--cui-text-tertiary);
		word-break: break-all;
		padding: .35rem 1rem 0;
		margin: 0;
	}

	.detail-tabs {
		display: flex;
		gap: .35rem;
		padding: .65rem 1rem;
		border-bottom: 1px solid var(--cui-glass-border);
		overflow: auto;
		scrollbar-width: none;
	}

	.detail-tabs::-webkit-scrollbar {
		display: none;
	}

	.detail-tabs button {
		text-transform: capitalize;
		border: 1px solid var(--cui-glass-border);
		background: var(--cui-surface-hover);
		color: var(--cui-text-primary);
		border-radius: 999px;
		padding: .35rem .7rem;
		cursor: pointer;
		font-size: .72rem;
		white-space: nowrap;
	}

	.detail-tabs button.active {
		border-color: var(--cui-glass-border-active);
		color: var(--cui-primary);
		background: var(--cui-surface-active);
	}

	.detail-tabs button:focus-visible,
	.icon-btn:focus-visible {
		outline: 2px solid var(--cui-primary);
		outline-offset: 2px;
	}

	.detail-panel {
		padding-bottom: max(.85rem,env(safe-area-inset-bottom));
	}

	.detail-dl {
		display: grid;
		grid-template-columns: 120px 1fr;
		gap: .4rem .75rem;
		padding: .85rem 1rem;
		margin: 0;
		font-size: .78rem;
	}

	.detail-dl dt {
		color: var(--cui-text-secondary);
	}

	.detail-dl dd {
		margin: 0;
		word-break: break-word;
	}

	.break-all {
		word-break: break-all;
	}

	.detail-label {
		color: var(--cui-text-secondary);
		font-size: .68rem;
		text-transform: uppercase;
		letter-spacing: .08em;
	}

	.detail-subhead {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: .75rem;
		padding: .75rem 1rem;
		flex-wrap: wrap;
	}

	.peer-head {
		flex-direction: column;
		align-items: stretch;
		gap: .5rem;
	}

	.peer-controls {
		display: flex;
		gap: .45rem;
		align-items: center;
		flex-wrap: wrap;
	}

	.piece-section {
		padding: .75rem 1rem;
		border-top: 1px solid var(--cui-glass-border);
		display: flex;
		flex-direction: column;
		gap: .5rem;
	}

	.piece-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.piece-bar {
		display: grid;
		grid-template-columns: repeat(160,minmax(0,1fr));
		gap: 2px;
		width: 100%;
	}

	.piece-bar span {
		display: block;
		height: 16px;
		border-radius: 2px;
		background: #ffffff14;
	}

	.piece-bar span.partial {
		background: var(--cui-warning);
	}

	.piece-bar span.done {
		background: var(--cui-secondary);
	}

	.detail-scroll {
		overflow-x: auto;
		-webkit-overflow-scrolling: touch;
		padding: 0 1rem;
	}

	.detail-table {
		width: 100%;
		min-width: 640px;
		border-collapse: collapse;
		font-size: .76rem;
	}

	.detail-table th,
	.detail-table td {
		padding: .55rem .75rem;
		border-bottom: 1px solid var(--cui-glass-border);
		text-align: left;
		vertical-align: top;
	}

	.detail-table th {
		color: var(--cui-text-secondary);
		text-transform: uppercase;
		letter-spacing: .07em;
		font-size: .62rem;
	}

	.tracker-url {
		max-width: 280px;
		word-break: break-all;
	}

	.file-cell {
		display: flex;
		flex-direction: column;
		gap: .15rem;
	}

	.file-actions {
		display: flex;
		align-items: center;
		gap: .5rem;
		flex-wrap: wrap;
	}

	.priority-select,
	.filter-input {
		background: var(--cui-input-bg);
		border: 1px solid var(--cui-input-border);
		border-radius: var(--cui-radius-md);
		color: var(--cui-text-primary);
		padding: .45rem .55rem;
		font-size: .73rem;
	}

	.empty-cell {
		text-align: center;
		padding: 2rem;
		color: var(--cui-text-tertiary);
	}

	.muted-text {
		color: var(--cui-text-secondary);
		font-size: .8rem;
		margin: 0;
		word-break: break-word;
	}

	.eyebrow {
		text-transform: uppercase;
		letter-spacing: .12em;
		font-size: .65rem;
		color: var(--cui-text-secondary);
		margin: 0;
	}

	.icon-btn {
		background: none;
		border: 0;
		color: var(--cui-text-secondary);
		cursor: pointer;
		padding: 0;
		line-height: 1;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-width: var(--cui-touch-min);
		min-height: var(--cui-touch-min);
		border-radius: var(--cui-radius-sm);
	}

	.icon-btn:hover {
		color: var(--cui-primary);
	}

	.inspector-empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: .6rem;
		min-height: 240px;
		padding: 2rem;
		text-align: center;
	}

	.empty-icon {
		font-size: 2.5rem;
		color: var(--cui-primary);
		opacity: .35;
	}

	@media (max-width:1024px) {
		.inspector-panel {
			position: static;
			max-height: none;
		}
	}

	@media (max-width:720px) {
		.detail-dl {
			grid-template-columns: 1fr;
		}

		.detail-subhead {
			flex-direction: column;
			align-items: stretch;
		}

		.peer-controls {
			width: 100%;
			flex-direction: column;
		}

		.peer-controls :global(input),
		.peer-controls :global(button) {
			width: 100%;
		}

		.detail-scroll {
			padding: 0 .75rem;
		}

		.detail-table {
			min-width: 560px;
		}

		.piece-bar {
			grid-template-columns: repeat(80,minmax(0,1fr));
			gap: 1px;
		}

		.piece-bar span {
			height: 12px;
		}
	}
</style>
