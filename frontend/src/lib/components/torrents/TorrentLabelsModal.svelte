<script lang="ts">
  import { Button, Modal } from "@media-set/core-ui";
  import { qbitStore } from "$lib/stores/qbit.svelte";
  import type { UnifiedTorrentRow } from "$lib/stores/qbitReducer";
  import { toasts } from "$lib/stores/toasts";

  interface Props {
    open: boolean;
    mode: "category" | "tags";
    /** Row the action was opened from; falls back to the current selection. */
    torrent?: UnifiedTorrentRow | null;
    onClose: () => void;
  }

  let { open, mode, torrent = null, onClose }: Props = $props();

  let hashes = $state<string[]>([]);
  let count = $state(0);
  let category = $state("");
  let tags = $state("");
  let saving = $state(false);

  function resolveTargets(row: UnifiedTorrentRow | null): UnifiedTorrentRow[] {
    const selected = Array.from(qbitStore.torrents.values()).filter(
      (t) =>
        (!qbitStore.selectedInstanceId || t.instance_id === qbitStore.selectedInstanceId) &&
        qbitStore.selectedHashes.has(`${t.instance_id}:${t.hash}`),
    );
    if (row && !qbitStore.selectedHashes.has(`${row.instance_id}:${row.hash}`)) {
      qbitStore.selectOnly([row.hash]);
      hashes = [row.hash];
      count = 1;
      return [row];
    }
    hashes = selected.map((t) => t.hash);
    count = selected.length;
    return selected;
  }

  $effect(() => {
    if (!open) return;
    const targets = resolveTargets(torrent);
    category = (targets.length === 1 && targets[0].category) || "";
    tags = (targets.length === 1 && targets[0].tags) || "";
    qbitStore.fetchLabels().catch(() => {});
  });

  async function apply() {
    if (hashes.length === 0) return toasts.error("Select at least one torrent");
    saving = true;
    try {
      const action = mode === "category" ? "set-category" : "set-tags";
      const extra = mode === "category" ? { category: category.trim() } : { tags: tags.trim() };
      if (!(await qbitStore.action(action, hashes, extra))?.success) return;
      onClose();
      await qbitStore.fetchTorrents();
    } finally {
      saving = false;
    }
  }
</script>

<Modal {open} {onClose} ariaLabel="Edit labels" maxWidth="400px">
  {#snippet header()}
    <span class="modal-icon material-icons">{mode === "category" ? "folder" : "sell"}</span><h2 class="modal-title">
      {mode === "category" ? "Set category" : "Edit tags"}
    </h2><button class="close-btn" aria-label="Close" onclick={onClose}
      ><span class="material-icons">close</span></button
    >
  {/snippet}

  <p class="muted-text">
    {count} torrent{count === 1 ? "" : "s"} selected. Empty value clears existing {mode === "category"
      ? "category"
      : "tags"}.
  </p>{#if mode === "category"}<label class="stack"
      ><span>Category</span><input
        bind:value={category}
        list="qbit-categories"
        placeholder="Leave empty to clear"
      /></label
    ><datalist id="qbit-categories"
      >{#each qbitStore.categoryNames as name}<option value={name}></option>{/each}</datalist
    >{:else}<label class="stack"><span>Tags</span><input bind:value={tags} placeholder="tag1,tag2" /></label
    >{#if qbitStore.tagNames.length > 0}
      <div class="suggestions">
        {#each qbitStore.tagNames as tag}
          <button type="button" class="pill-btn" onclick={() => (tags = tags ? `${tags},${tag}` : tag)}
            >{tag}</button
          >
        {/each}
      </div>
    {/if}{/if}

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

	.suggestions {
		display: flex;
		gap: .4rem;
		flex-wrap: wrap;
		margin-top: .65rem;
	}

	.pill-btn {
		border: 1px solid rgba(255,255,255,.1);
		background: #ffffff0a;
		color: #94a3b8;
		border-radius: 999px;
		padding: .2rem .6rem;
		cursor: pointer;
		font-size: .7rem;
	}

	.pill-btn:hover {
		color: #fff;
		border-color: #ffffff38;
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
