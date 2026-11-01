<script lang="ts">
  import { Button, Modal } from "@media-set/core-ui";
  import { qbitStore } from "$lib/stores/qbit.svelte";
  import type { UnifiedTorrentRow } from "$lib/stores/qbitReducer";

  interface Props {
    open: boolean;
    torrent: UnifiedTorrentRow | null;
    onClose: () => void;
  }

  let { open, torrent, onClose }: Props = $props();

  let targetInstanceId = $state("");
  let savePath = $state("");
  let saving = $state(false);

  $effect(() => {
    if (open) {
      targetInstanceId = "";
      savePath = "";
    }
  });

  async function crossSeed() {
    if (!torrent || !targetInstanceId) return;
    saving = true;
    try {
      if (!(await qbitStore.crossSeed(torrent.hash, targetInstanceId, savePath || undefined))?.success) return;
      onClose();
    } finally {
      saving = false;
    }
  }
</script>

<Modal {open} {onClose} ariaLabel="Cross-seed" maxWidth="480px">
  {#snippet header()}
    <span class="modal-icon material-icons">sync_alt</span><h2 class="modal-title">Cross-seed</h2><button
      class="close-btn"
      aria-label="Close"
      onclick={onClose}><span class="material-icons">close</span></button
    >
  {/snippet}

  <p class="muted-text">{torrent?.name ?? ""}</p><label class="stack"
    ><span>Target instance</span><select bind:value={targetInstanceId}
      ><option value="">Select target</option>{#each qbitStore.instances.filter((i) => i.id !== qbitStore.selectedInstanceId) as instance}<option
          value={instance.id}>{instance.name}</option
        >{/each}</select
    ></label
  ><label class="stack"><span>Save path on target</span><input bind:value={savePath} placeholder="/mnt/media" /></label>

  {#snippet footer()}
    <div class="modal-actions">
      <Button variant="ghost" onclick={onClose} disabled={saving}>Cancel</Button><Button
        variant="primary"
        onclick={crossSeed}
        disabled={saving || !targetInstanceId}
        loading={saving}>Cross-seed</Button
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

	input,
	select {
		background: #00000047;
		border: 1px solid rgba(255,255,255,.1);
		border-radius: 10px;
		color: #fff;
		padding: .55rem .75rem;
	}
</style>
