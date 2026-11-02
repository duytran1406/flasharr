<script lang="ts">
  import { Button, Modal } from "@media-set/core-ui";
  import { qbitStore } from "$lib/stores/qbit.svelte";
  import type { UnifiedTorrentRow } from "$lib/stores/qbitReducer";
  import { toasts } from "$lib/stores/toasts";

  interface Props {
    open: boolean;
    /** Row the action was opened from; falls back to the current selection. */
    torrent?: UnifiedTorrentRow | null;
    onClose: () => void;
  }

  let { open, torrent = null, onClose }: Props = $props();

  let hashes = $state<string[]>([]);
  let count = $state(0);
  let saving = $state(false);
  let location = $state("");

  function resolveTargets(row: UnifiedTorrentRow | null) {
    const selected = Array.from(qbitStore.torrents.values()).filter(
      (t) =>
        (!qbitStore.selectedInstanceId || t.instance_id === qbitStore.selectedInstanceId) &&
        qbitStore.selectedHashes.has(`${t.instance_id}:${t.hash}`),
    );
    if (row && !qbitStore.selectedHashes.has(`${row.instance_id}:${row.hash}`)) {
      qbitStore.selectOnly([row.hash]);
      hashes = [row.hash];
      count = 1;
      location = row.save_path || "";
      return;
    }
    hashes = selected.map((t) => t.hash);
    count = selected.length;
    location = (selected.length === 1 && selected[0].save_path) || "";
  }

  $effect(() => {
    if (open) resolveTargets(torrent);
  });

  async function apply() {
    const target = location.trim();
    if (hashes.length === 0) return toasts.error("Select at least one torrent");
    if (!target) return toasts.error("Location is required");
    saving = true;
    try {
      if (!(await qbitStore.setLocation(hashes, target))?.success) return;
      onClose();
      await qbitStore.fetchTorrents();
    } finally {
      saving = false;
    }
  }
</script>

<Modal {open} {onClose} ariaLabel="Move location" maxWidth="440px">
  {#snippet header()}
    <span class="modal-icon material-icons">drive_file_move</span><h2 class="modal-title">Move location</h2><button
      class="close-btn"
      aria-label="Close"
      onclick={onClose}><span class="material-icons">close</span></button
    >
  {/snippet}

  <p class="muted-text">{count} torrent{count === 1 ? "" : "s"} selected.</p><label class="stack"
    ><span>Location</span><input bind:value={location} placeholder="/downloads/tv" /></label
  >

  {#snippet footer()}
    <div class="modal-actions">
      <Button variant="ghost" onclick={onClose} disabled={saving}>Cancel</Button><Button
        variant="primary"
        onclick={apply}
        disabled={saving}
        loading={saving}>Apply</Button
      >
    </div>
  {/snippet}
</Modal>

<style>
	.modal-title {
		margin: 0;
		font-size: .95rem;
		font-weight: 800;
		text-transform: uppercase;
		letter-spacing: .06em;
		color: #fff;
	}

	.modal-icon {
		color: var(--color-primary,#00f3ff);
		font-size: 1.3rem;
	}

	.modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: .6rem;
	}

	.stack {
		display: flex;
		flex-direction: column;
		gap: .4rem;
	}

	.stack span {
		color: #94a3b8;
		font-size: .68rem;
		text-transform: uppercase;
		letter-spacing: .08em;
	}

	.muted-text {
		color: #94a3b8;
		font-size: .8rem;
		margin: 0;
		word-break: break-word;
	}

	.close-btn {
		background: none;
		border: 0;
		color: #94a3b8;
		cursor: pointer;
		padding: 0;
		line-height: 1;
		display: inline-flex;
		align-items: center;
		border-radius: 8px;
	}

	.close-btn:hover {
		color: var(--color-primary,#00f3ff);
	}

	input {
		background: #00000047;
		border: 1px solid rgba(255,255,255,.1);
		border-radius: 10px;
		color: #fff;
		padding: .55rem .75rem;
	}
</style>
