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

  interface LimitsForm {
    unlimitedDownload: boolean;
    unlimitedUpload: boolean;
    unlimitedRatio: boolean;
    unlimitedSeedingTime: boolean;
    /** "" = leave unchanged */
    downloadLimit: number | string;
    uploadLimit: number | string;
    ratioLimit: number | string;
    seedingTimeLimit: number | string;
  }

  let { open, torrent = null, onClose }: Props = $props();

  let hashes = $state<string[]>([]);
  let count = $state(0);
  let saving = $state(false);
  let form = $state<LimitsForm>({} as LimitsForm);

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
      return;
    }
    hashes = selected.map((t) => t.hash);
    count = selected.length;
  }

  $effect(() => {
    if (open) {
      resolveTargets(torrent);
      form = {
        unlimitedDownload: false,
        unlimitedUpload: false,
        unlimitedRatio: false,
        unlimitedSeedingTime: false,
        downloadLimit: "",
        uploadLimit: "",
        ratioLimit: "",
        seedingTimeLimit: "",
      };
    }
  });

  async function apply() {
    if (hashes.length === 0) return toasts.error("Select at least one torrent");
    saving = true;
    try {
      // Speed: 0 = unlimited. Share limits: -1 = unlimited. undefined = unchanged.
      const download = form.unlimitedDownload
        ? 0
        : form.downloadLimit === ""
          ? undefined
          : Number(form.downloadLimit);
      const upload = form.unlimitedUpload ? 0 : form.uploadLimit === "" ? undefined : Number(form.uploadLimit);
      const ratio = form.unlimitedRatio ? -1 : form.ratioLimit === "" ? undefined : Number(form.ratioLimit);
      const seedingTime = form.unlimitedSeedingTime
        ? -1
        : form.seedingTimeLimit === ""
          ? undefined
          : Number(form.seedingTimeLimit);

      if (
        (download !== undefined && (!Number.isFinite(download) || download < 0)) ||
        (upload !== undefined && (!Number.isFinite(upload) || upload < 0)) ||
        (ratio !== undefined && (!Number.isFinite(ratio) || (ratio < 0 && ratio !== -1))) ||
        (seedingTime !== undefined && (!Number.isFinite(seedingTime) || (seedingTime < 0 && seedingTime !== -1)))
      )
        return toasts.error("Limits must be non-negative");

      const hasSpeed = download !== undefined || upload !== undefined;
      const hasShare = ratio !== undefined || seedingTime !== undefined;
      if (!hasSpeed && !hasShare) return toasts.error("Set at least one limit or choose unlimited");
      if ((ratio === undefined) != (seedingTime === undefined))
        return toasts.error("Set ratio and seeding time together");

      if (hasSpeed && !(await qbitStore.setSpeedLimits(hashes, download, upload))?.success) return;
      if (hasShare && !(await qbitStore.setShareLimits(hashes, ratio, seedingTime))?.success) return;
      onClose();
    } finally {
      saving = false;
    }
  }
</script>

<Modal {open} {onClose} ariaLabel="Set limits" maxWidth="500px">
  {#snippet header()}
    <span class="modal-icon material-icons">speed</span><h2 class="modal-title">Set limits</h2><button
      class="close-btn"
      aria-label="Close"
      onclick={onClose}><span class="material-icons">close</span></button
    >
  {/snippet}

  <p class="muted-text">{count} torrent{count === 1 ? "" : "s"} selected.</p><div class="grid-two">
    <label class="stack"
      ><span>Download B/s</span><input
        type="number"
        min="0"
        bind:value={form.downloadLimit}
        disabled={form.unlimitedDownload}
      /></label
    ><label class="check-row"
      ><input type="checkbox" bind:checked={form.unlimitedDownload} />Unlimited download</label
    ><label class="stack"
      ><span>Upload B/s</span><input
        type="number"
        min="0"
        bind:value={form.uploadLimit}
        disabled={form.unlimitedUpload}
      /></label
    ><label class="check-row"><input type="checkbox" bind:checked={form.unlimitedUpload} />Unlimited upload</label
    ><label class="stack"
      ><span>Ratio</span><input
        type="number"
        min="0"
        step="0.1"
        bind:value={form.ratioLimit}
        disabled={form.unlimitedRatio}
      /></label
    ><label class="check-row"><input type="checkbox" bind:checked={form.unlimitedRatio} />Unlimited ratio</label
    ><label class="stack"
      ><span>Seeding minutes</span><input
        type="number"
        min="0"
        bind:value={form.seedingTimeLimit}
        disabled={form.unlimitedSeedingTime}
      /></label
    ><label class="check-row"
      ><input type="checkbox" bind:checked={form.unlimitedSeedingTime} />Unlimited seeding</label
    >
  </div><p class="muted-text hint">
    Blank field = leave unchanged. Unlimited sets qB's unlimited value (-1 for ratio/seeding).
  </p>

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

	.grid-two {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: .7rem;
	}

	.check-row {
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

	@media (max-width:720px) {
		.grid-two {
			grid-template-columns: 1fr;
		}
	}
</style>
