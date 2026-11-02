<script lang="ts">
  import { Button, Modal } from "@media-set/core-ui";
  import { qbitStore } from "$lib/stores/qbit.svelte";
  import { toasts } from "$lib/stores/toasts";
  import type { TorrentRenameMode } from "./torrentIntent";

  interface Props {
    open: boolean;
    mode: TorrentRenameMode;
    hash: string;
    /** What is being renamed ("torrent", "file", ...), shown in the title. */
    label: string;
    currentName?: string;
    /** Path inside the torrent (file/folder modes). */
    oldPath?: string;
    onClose: () => void;
  }

  let { open, mode, hash, label, currentName = "", oldPath = "", onClose }: Props = $props();

  let saving = $state(false);
  let name = $state("");

  $effect(() => {
    if (open) name = currentName.trim();
  });

  async function rename() {
    const nextName = name.trim();
    if (!nextName) return toasts.error("Name is required");
    if (/[\\/]/.test(nextName)) return toasts.error("Use a leaf name only");
    saving = true;
    try {
      const result =
        mode === "torrent"
          ? await qbitStore.renameTorrent(hash, nextName)
          : mode === "file"
            ? await qbitStore.renameFile(hash, oldPath, nextName)
            : await qbitStore.renameFolder(hash, oldPath, nextName);
      if (!result?.success) return;
      if (mode === "torrent") await qbitStore.fetchTorrents();
      else qbitStore.invalidateDetail(hash, "files");
      onClose();
    } finally {
      saving = false;
    }
  }
</script>

<Modal {open} {onClose} ariaLabel="Rename" maxWidth="420px">
  {#snippet header()}
    <span class="modal-icon material-icons">edit</span><h2 class="modal-title">Rename {label}</h2><button
      class="close-btn"
      aria-label="Close"
      onclick={onClose}><span class="material-icons">close</span></button
    >
  {/snippet}

  <label class="stack"><span>New name</span><input bind:value={name} placeholder="Enter new name" /></label><p class="muted-text hint">Leaf name only — path separators are blocked.</p>

  {#snippet footer()}
    <div class="modal-actions">
      <Button variant="ghost" onclick={onClose} disabled={saving}>Cancel</Button><Button
        variant="primary"
        onclick={rename}
        disabled={saving}
        loading={saving}>Rename</Button
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

	.hint {
		font-size: .78rem;
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
