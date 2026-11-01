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
  let names = $state<string[]>([]);
  let count = $state(0);
  let deleteFiles = $state(false);
  let rememberChoice = $state(false);

  function resolveTargets(row: UnifiedTorrentRow | null) {
    const selected = Array.from(qbitStore.torrents.values()).filter(
      (t) =>
        (!qbitStore.selectedInstanceId || t.instance_id === qbitStore.selectedInstanceId) &&
        qbitStore.selectedHashes.has(`${t.instance_id}:${t.hash}`),
    );
    if (row && !qbitStore.selectedHashes.has(`${row.instance_id}:${row.hash}`)) {
      qbitStore.selectOnly([row.hash]);
      hashes = [row.hash];
      names = [row.name];
      count = 1;
      return;
    }
    hashes = selected.map((t) => t.hash);
    names = selected.map((t) => t.name);
    count = selected.length;
  }

  // Safe default on every open: remove the torrent, keep the files.
  $effect(() => {
    if (open) {
      resolveTargets(torrent);
      rememberChoice = false;
      deleteFiles = false;
    }
  });

  async function confirmDelete() {
    if (hashes.length === 0) return toasts.error("Select at least one torrent");
    if (rememberChoice) localStorage.setItem("qbit-delete-files", String(deleteFiles));
    localStorage.setItem("qbit-delete-files-remember", String(rememberChoice));
    onClose();
    await qbitStore.action("delete", hashes, { delete_files: deleteFiles });
    await qbitStore.fetchTorrents();
  }
</script>

<Modal {open} {onClose} ariaLabel="Delete torrents" accent="#ff5252" maxWidth="440px">
  {#snippet header()}
    <span class="modal-icon material-icons danger">delete_forever</span><h2 class="modal-title">
      Delete {count} torrent{count === 1 ? "" : "s"}?
    </h2><button class="close-btn" aria-label="Close" onclick={onClose}
      ><span class="material-icons">close</span></button
    >
  {/snippet}

  <p class="muted-text">
    {names.slice(0, 3).join(", ")}{count > 3 ? ` +${count - 3} more` : ""}
  </p><p class="muted-text">Default: remove torrent; keep downloaded files.</p><label class="check-row danger"
    ><input type="checkbox" bind:checked={deleteFiles} />Also permanently delete downloaded files</label
  >{#if deleteFiles}
    <p class="warning" role="alert">Downloaded data will be permanently deleted. This cannot be undone.</p>
  {/if}

  {#snippet footer()}
    <div class="modal-actions">
      <Button variant="ghost" onclick={onClose}>Cancel</Button><Button variant="danger" onclick={confirmDelete}
        >{deleteFiles ? "Delete files" : "Delete torrent"}</Button
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

	.modal-icon.danger {
		color: #ff5252;
	}

	.modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: .6rem;
	}

	.check-row {
		display: flex;
		gap: .4rem;
		align-items: center;
		color: #cbd5e1;
	}

	.check-row.danger {
		color: #ff6b6b;
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

	.warning {
		color: #ff6b6b;
		font-weight: 700;
		font-size: .8rem;
	}
</style>
