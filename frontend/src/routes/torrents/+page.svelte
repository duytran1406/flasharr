<script lang="ts">
  import { onMount } from "svelte";
  import { Button, ErrorState } from "@media-set/core-ui";
  import TorrentActionModals from "$lib/components/torrents/TorrentActionModals.svelte";
  import TorrentDetailsModal from "$lib/components/torrents/TorrentDetailsModal.svelte";
  import TorrentFilterBar from "$lib/components/torrents/TorrentFilterBar.svelte";
  import TorrentHeader from "$lib/components/torrents/TorrentHeader.svelte";
  import TorrentInstanceModal from "$lib/components/torrents/TorrentInstanceModal.svelte";
  import TorrentQueueTable from "$lib/components/torrents/TorrentQueueTable.svelte";
  import TorrentSettingsHubModal from "$lib/components/torrents/TorrentSettingsHubModal.svelte";
  import type { TorrentIntent } from "$lib/components/torrents/torrentIntent";
  import { qbitStore } from "$lib/stores/qbit.svelte";
  import type { UnifiedTorrentRow } from "$lib/stores/qbitReducer";
  import { toasts } from "$lib/stores/toasts";

  let detailsTorrent = $state<UnifiedTorrentRow | null>(null);
  let intent = $state<TorrentIntent | null>(null);
  let instancesOpen = $state(false);
  let settingsOpen = $state(false);
  let visibleColumns = $state(
    new Set([
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
    ]),
  );

  /** "Media / Torrent" is always shown. */
  function toggleColumn(column: string) {
    const next = new Set(visibleColumns);
    if (column !== "Media / Torrent") {
      if (next.has(column)) next.delete(column);
      else next.add(column);
    }
    visibleColumns = next;
  }

  // Drop details/actions that belong to another (or no) instance.
  $effect(() => {
    if (detailsTorrent && detailsTorrent.instance_id !== qbitStore.selectedInstanceId) detailsTorrent = null;
    if (!qbitStore.selectedInstanceId) intent = null;
  });

  onMount(() => {
    qbitStore.fetchInstances().catch((err) => toasts.error(String(err)));
  });

  function openDetails(torrent: UnifiedTorrentRow) {
    detailsTorrent = torrent;
  }

  function openIntent(next: TorrentIntent) {
    intent = next;
  }
</script>

<svelte:head>
  <title>Torrents - Flasharr</title>
</svelte:head>

<div class="page">
  {#if qbitStore.selectedInstanceId}
    <section class="torrent-workspace">
      <TorrentHeader onOpenSettings={() => (settingsOpen = true)} {visibleColumns} onToggleColumn={toggleColumn} />
      <TorrentFilterBar />
      {#if qbitStore.error}
        <ErrorState
          title="qBittorrent error"
          message={qbitStore.error}
          showRetry={true}
          onRetry={() => qbitStore.fetchTorrents().catch(() => {})}
        />
      {/if}
      <TorrentQueueTable {detailsTorrent} onOpenDetails={openDetails} onOpenIntent={openIntent} {visibleColumns} />
    </section>
  {:else}
    <div class="empty-setup">
      <ErrorState
        title="No qBittorrent instance configured"
        message="Add an instance to start managing torrents."
        showRetry={false}
      />
      <Button variant="primary" icon="dns" onclick={() => (instancesOpen = true)}>Add instance</Button>
    </div>
  {/if}

  <TorrentInstanceModal open={instancesOpen} onClose={() => (instancesOpen = false)} />
  <TorrentSettingsHubModal
    open={settingsOpen}
    onClose={() => (settingsOpen = false)}
    onInstances={() => {
      settingsOpen = false;
      instancesOpen = true;
    }}
    onPreferences={() => {
      settingsOpen = false;
      openIntent({ kind: "prefs" });
    }}
    onCreate={() => {
      settingsOpen = false;
      openIntent({ kind: "create" });
    }}
  />
  <TorrentDetailsModal torrent={detailsTorrent} onClose={() => (detailsTorrent = null)} onOpenIntent={openIntent} />
  <TorrentActionModals {intent} onClose={() => (intent = null)} />
</div>

<style>
	.page {
		padding: 24px;
		height: 100%;
		overflow: hidden;
		color: var(--cui-text-primary);
		box-sizing: border-box;
		display: flex;
		flex-direction: column;
	}

	.torrent-workspace {
		flex: 1;
		min-height: 0;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		background: var(--cui-glass);
		border: 1px solid var(--cui-glass-border);
		border-radius: var(--cui-radius-lg);
	}

	.torrent-workspace :global(.bento-card) {
		border: 0;
		border-radius: 0;
		box-shadow: none;
		background: transparent;
	}

	.empty-setup {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--cui-space-md);
	}

	@media (max-width:720px) {
		.page {
			padding: var(--cui-space-sm);
			padding-bottom: calc(var(--cui-mobile-bottom-nav-height) + var(--cui-mobile-safe-area-bottom) + var(--cui-space-sm));
		}

		.torrent-workspace {
			overflow: visible;
		}

		.page {
			overflow-y: auto;
			scroll-padding-bottom: calc(var(--cui-mobile-bottom-nav-height) + var(--cui-mobile-safe-area-bottom) + var(--cui-space-sm));
		}
	}
</style>
