<script lang="ts">
  import { Button, Modal } from "@media-set/core-ui";
  import { qbitStore, type QbitPreferences } from "$lib/stores/qbit.svelte";
  import { toasts } from "$lib/stores/toasts";

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  const NUMERIC_KEYS = [
    "dl_limit",
    "up_limit",
    "alt_dl_limit",
    "alt_up_limit",
    "max_seeding_time",
    "max_active_downloads",
    "max_active_uploads",
  ] as const;
  const BOOLEAN_KEYS = ["max_ratio_enabled", "max_seeding_time_enabled", "queueing_enabled"] as const;

  let loading = $state(false);
  let saving = $state(false);
  /** Preferences as loaded (diff base: only changed keys are saved). */
  let original = $state<QbitPreferences>({});
  let form = $state<QbitPreferences>({});

  $effect(() => {
    if (open) load();
  });

  async function load() {
    loading = true;
    try {
      const prefs = await qbitStore.preferences();
      original = prefs;
      form = {
        dl_limit: prefs.dl_limit ?? 0,
        up_limit: prefs.up_limit ?? 0,
        alt_dl_limit: prefs.alt_dl_limit ?? 0,
        alt_up_limit: prefs.alt_up_limit ?? 0,
        max_ratio_enabled: !!prefs.max_ratio_enabled,
        max_ratio: prefs.max_ratio ?? 0,
        max_seeding_time_enabled: !!prefs.max_seeding_time_enabled,
        max_seeding_time: prefs.max_seeding_time ?? 0,
        queueing_enabled: !!prefs.queueing_enabled,
        max_active_downloads: prefs.max_active_downloads ?? 0,
        max_active_uploads: prefs.max_active_uploads ?? 0,
      };
    } catch (err) {
      toasts.error(err instanceof Error ? err.message : "Failed to load preferences");
      onClose();
    } finally {
      loading = false;
    }
  }

  async function save() {
    saving = true;
    try {
      const changes: QbitPreferences = {};
      for (const key of NUMERIC_KEYS) {
        const next = Number(form[key]);
        const previous = Number(original[key] ?? 0);
        if (!Number.isFinite(next) || next < 0) return toasts.error(`${key} must be non-negative`);
        if (next !== previous) changes[key] = next;
      }
      for (const key of BOOLEAN_KEYS) {
        const next = !!form[key];
        const previous = !!original[key];
        if (next !== previous) changes[key] = next;
      }
      const maxRatio = Number(form.max_ratio ?? 0);
      const previousRatio = Number(original.max_ratio ?? 0);
      if (!Number.isFinite(maxRatio) || maxRatio < 0) return toasts.error("max_ratio must be non-negative");
      if (maxRatio !== previousRatio) changes.max_ratio = maxRatio;
      if (!(await qbitStore.savePreferences(changes))?.success) return;
      onClose();
    } finally {
      saving = false;
    }
  }
</script>

<Modal {open} {onClose} ariaLabel="Instance preferences" maxWidth="600px">
  {#snippet header()}
    <span class="modal-icon material-icons">settings</span><h2 class="modal-title">Instance preferences</h2><button
      class="close-btn"
      aria-label="Close"
      onclick={onClose}><span class="material-icons">close</span></button
    >
  {/snippet}

  {#if loading}
    <p class="empty-cell">Loading preferences…</p>
  {:else}
    <div class="grid-two">
      <label class="stack"
        ><span>Download limit (B/s, 0=∞)</span><input type="number" min="0" bind:value={form.dl_limit} /></label
      ><label class="stack"
        ><span>Upload limit (B/s, 0=∞)</span><input type="number" min="0" bind:value={form.up_limit} /></label
      ><label class="stack"
        ><span>Alt DL limit</span><input type="number" min="0" bind:value={form.alt_dl_limit} /></label
      ><label class="stack"
        ><span>Alt UL limit</span><input type="number" min="0" bind:value={form.alt_up_limit} /></label
      ><label class="stack"
        ><span>Max ratio</span><input type="number" min="0" step="0.1" bind:value={form.max_ratio} /></label
      ><label class="stack"
        ><span>Max seeding minutes</span><input type="number" min="0" bind:value={form.max_seeding_time} /></label
      ><label class="stack"
        ><span>Max active downloads</span><input
          type="number"
          min="0"
          bind:value={form.max_active_downloads}
        /></label
      ><label class="stack"
        ><span>Max active seeds</span><input type="number" min="0" bind:value={form.max_active_uploads} /></label
      >
    </div><div class="checks">
      <label><input type="checkbox" bind:checked={form.max_ratio_enabled} />Max ratio enabled</label><label
        ><input type="checkbox" bind:checked={form.max_seeding_time_enabled} />Max seeding time enabled</label
      ><label><input type="checkbox" bind:checked={form.queueing_enabled} />Queueing enabled</label>
    </div>
  {/if}

  {#snippet footer()}
    <div class="modal-actions">
      <Button variant="ghost" onclick={onClose} disabled={saving}>Cancel</Button><Button
        variant="primary"
        onclick={save}
        disabled={loading || saving}
        loading={saving}>Save</Button
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

	.empty-cell {
		text-align: center;
		padding: 2rem;
		color: #64748b;
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
		.grid-two,
		.checks {
			grid-template-columns: 1fr;
		}
	}
</style>
