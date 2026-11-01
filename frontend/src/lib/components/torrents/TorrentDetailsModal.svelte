<script lang="ts">
  import { Modal } from "@media-set/core-ui";
  import type { UnifiedTorrentRow } from "$lib/stores/qbitReducer";
  import TorrentInspector from "./TorrentInspector.svelte";
  import type { TorrentIntent } from "./torrentIntent";

  interface Props {
    /** Torrent to inspect; null closes the modal. */
    torrent: UnifiedTorrentRow | null;
    onClose: () => void;
    onOpenIntent: (intent: TorrentIntent) => void;
  }

  let { torrent, onClose, onOpenIntent }: Props = $props();
</script>

<Modal open={torrent !== null} {onClose} maxWidth="1100px" ariaLabel="Torrent details">
  {#snippet header()}
    <h2>Torrent details</h2><button class="close" aria-label="Close torrent details" onclick={onClose}
      ><span class="material-icons">close</span></button
    >
  {/snippet}

  <TorrentInspector detailsTorrent={torrent} {onClose} {onOpenIntent} />
</Modal>

<style>
	h2 {
		margin: 0;
		font-size: 1rem;
	}

	.close {
		margin-left: auto;
		background: none;
		border: 0;
		color: var(--cui-text-primary);
		cursor: pointer;
	}
</style>
