<script lang="ts">
  import { Button, Modal } from "@media-set/core-ui";
  import { qbitStore } from "$lib/stores/qbit.svelte";
  import { toasts } from "$lib/stores/toasts";

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  let sourcePath = $state("");
  let torrentName = $state("");
  let trackers = $state("");
  let sourceTag = $state("");
  let isPrivate = $state(false);
  let startSeeding = $state(false);
  let creating = $state(false);

  function reset() {
    sourcePath = "";
    torrentName = "";
    trackers = "";
    sourceTag = "";
    isPrivate = false;
    startSeeding = false;
  }

  async function create() {
    const path = sourcePath.trim();
    if (!path) return toasts.error("Source path is required");
    creating = true;
    try {
      const result = await qbitStore.createTorrent({
        source_path: path,
        torrent_name: torrentName.trim() || undefined,
        trackers: trackers.trim() || undefined,
        source: sourceTag.trim() || undefined,
        private: isPrivate,
        start_seeding: startSeeding,
      });
      if (!result?.success) return;
      reset();
      onClose();
    } finally {
      creating = false;
    }
  }
</script>

<Modal {open} {onClose} ariaLabel="Create torrent" maxWidth="540px">
  {#snippet header()}
    <span class="modal-icon material-icons">create_new_folder</span><h2 class="modal-title">Create .torrent</h2><button
      class="close-btn"
      aria-label="Close"
      onclick={onClose}><span class="material-icons">close</span></button
    >
  {/snippet}

  <div class="form-grid">
    <label class="stack"
      ><span>Source path (required)</span><input bind:value={sourcePath} placeholder="/downloads/show" /></label
    ><label class="stack"><span>Torrent name</span><input bind:value={torrentName} placeholder="Optional" /></label
    ><label class="stack"
      ><span>Trackers</span><textarea
        bind:value={trackers}
        placeholder="https://tracker/announce One per line"
        rows="3"
      ></textarea></label
    ><label class="stack"
      ><span>Source tag</span><input
        bind:value={sourceTag}
        placeholder="Optional (for private trackers)"
      /></label
    ><div class="checks">
      <label><input type="checkbox" bind:checked={isPrivate} />Private</label><label
        ><input type="checkbox" bind:checked={startSeeding} />Start seeding</label
      >
    </div>
  </div>

  {#snippet footer()}
    <div class="modal-actions">
      <Button
        variant="ghost"
        onclick={() => {
          reset();
          onClose();
        }}
        disabled={creating}>Cancel</Button
      ><Button variant="primary" onclick={create} disabled={creating || !sourcePath.trim()} loading={creating}
        >Create</Button
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

	.form-grid {
		display: flex;
		flex-direction: column;
		gap: .85rem;
	}

	.checks {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: .45rem;
		color: #cbd5e1;
	}

	.checks label {
		display: flex;
		gap: .4rem;
		align-items: center;
		color: #cbd5e1;
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

	input,
	textarea {
		background: #00000047;
		border: 1px solid rgba(255,255,255,.1);
		border-radius: 10px;
		color: #fff;
		padding: .55rem .75rem;
	}

	textarea {
		resize: vertical;
	}

	@media (max-width:720px) {
		.checks {
			grid-template-columns: 1fr;
		}
	}
</style>
