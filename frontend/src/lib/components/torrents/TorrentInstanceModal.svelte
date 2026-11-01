<script lang="ts">
  import { Button, Modal } from "@media-set/core-ui";
  import { qbitStore, type QbitInstance } from "$lib/stores/qbit.svelte";
  import { toasts } from "$lib/stores/toasts";

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  /** Instance being edited; null = add form. */
  let editing = $state<QbitInstance | null>(null);
  let name = $state("");
  let url = $state("http://localhost:8080");
  let username = $state("admin");
  let password = $state("");
  let defaultCategory = $state("");
  let savePath = $state("");
  let enabled = $state(true);
  let pollInterval = $state(5);
  let saving = $state(false);
  let formError = $state("");

  function resetForm(instance: QbitInstance | null = null) {
    editing = instance;
    name = instance?.name ?? "";
    url = instance?.url ?? "http://localhost:8080";
    username = instance?.username ?? "admin";
    password = "";
    defaultCategory = instance?.default_category ?? "";
    savePath = instance?.save_path ?? "";
    enabled = instance?.enabled ?? true;
    pollInterval = instance?.poll_interval_secs ?? 5;
    formError = "";
  }

  function close() {
    if (saving) return;
    resetForm();
    onClose();
  }

  async function save() {
    saving = true;
    formError = "";
    try {
      const saved = await qbitStore.saveInstance({
        id: editing?.id,
        name,
        url,
        username,
        password,
        enabled,
        default_category: defaultCategory,
        save_path: savePath,
        poll_interval_secs: Number(pollInterval),
      });
      toasts.success(`Instance ${editing ? "updated" : "added"}`);
      resetForm();
      if (saved.enabled) onClose();
    } catch (err) {
      formError = err instanceof Error ? err.message : "Unable to save instance";
    } finally {
      saving = false;
    }
  }

  async function testConnection(instance: QbitInstance) {
    try {
      const result = await qbitStore.testInstance(instance.id);
      if (result.success) toasts.success(result.message || "qBittorrent connected");
      else toasts.error(result.message || "Connection failed");
    } catch (err) {
      toasts.error(err instanceof Error ? err.message : "Connection test failed");
    }
  }

  async function remove(instance: QbitInstance) {
    if (!confirm(`Delete ${instance.name}? Active Flasharr torrent tasks block deletion.`)) return;
    try {
      await qbitStore.deleteInstance(instance.id);
      if (editing?.id === instance.id) resetForm();
      toasts.success("Instance deleted");
    } catch (err) {
      toasts.error(err instanceof Error ? err.message : "Unable to delete instance");
    }
  }
</script>

<Modal {open} onClose={close} ariaLabel="Manage qBittorrent instances" maxWidth="760px">
  {#snippet header()}
    <span class="material-icons modal-icon">dns</span><h2>qBittorrent instances</h2><button
      class="close"
      aria-label="Close"
      onclick={close}><span class="material-icons">close</span></button
    >
  {/snippet}

  <div class="layout">
    <section class="instances" aria-label="Configured qBittorrent instances">
      {#if qbitStore.instances.length === 0}
        <p class="empty">No instances configured.</p>
      {/if}
      {#each qbitStore.instances as instance}
        <article class:active={editing?.id === instance.id}>
          <div>
            <strong>{instance.name}</strong><small>{instance.url}</small><small class:error={!!instance.last_error}
              >{instance.last_error || (instance.enabled ? "Enabled" : "Disabled")}</small
            >
          </div>
          <div class="actions">
            <Button variant="ghost" size="sm" onclick={() => resetForm(instance)}>Edit</Button><Button
              variant="ghost"
              size="sm"
              onclick={() => testConnection(instance)}>Test</Button
            ><Button variant="danger" size="sm" onclick={() => remove(instance)}>Delete</Button>
          </div>
        </article>
      {/each}
    </section>

    <form
      onsubmit={(e) => {
        e.preventDefault();
        save();
      }}
    >
      <div class="form-head">
        <h3>{editing ? `Edit ${editing.name}` : "Add instance"}</h3>{#if editing}<Button
            variant="ghost"
            size="sm"
            onclick={() => resetForm()}>New</Button
          >{/if}
      </div>
      <label><span>Name</span><input bind:value={name} maxlength="80" required /></label>
      <label><span>HTTP(S) URL</span><input type="url" bind:value={url} required /></label>
      <div class="two">
        <label><span>Username</span><input bind:value={username} maxlength="256" /></label><label
          ><span>Password {editing?.has_password ? "(leave blank to keep)" : ""}</span><input
            type="password"
            bind:value={password}
            maxlength="1024"
          /></label
        >
      </div>
      <div class="two">
        <label><span>Default category</span><input bind:value={defaultCategory} /></label><label
          ><span>Save path</span><input bind:value={savePath} /></label
        >
      </div>
      <div class="two">
        <label class="check"><input type="checkbox" bind:checked={enabled} /> Enabled</label><label
          ><span>Sync interval seconds</span><input
            type="number"
            min="2"
            max="300"
            bind:value={pollInterval}
            required
          /></label
        >
      </div>
      {#if formError}
        <p class="error" role="alert">{formError}</p>
      {/if}
      <Button variant="primary" type="submit" loading={saving}>{editing ? "Save instance" : "Add instance"}</Button>
    </form>
  </div>
</Modal>

<style>
	h2,
	h3 {
		margin: 0;
		color: var(--cui-text-primary);
	}

	h2 {
		font-size: .95rem;
		text-transform: uppercase;
		letter-spacing: .06em;
	}

	.modal-icon {
		color: var(--cui-primary);
	}

	.close {
		margin-left: auto;
		background: none;
		border: 0;
		color: var(--cui-text-secondary);
		min-width: var(--cui-touch-min);
		min-height: var(--cui-touch-min);
	}

	.layout {
		display: grid;
		grid-template-columns: minmax(220px,.85fr) minmax(0,1.15fr);
		gap: 1rem;
	}

	.instances {
		display: flex;
		flex-direction: column;
		gap: .5rem;
		max-height: 420px;
		overflow: auto;
	}

	.instances article {
		border: 1px solid var(--cui-glass-border);
		border-radius: var(--cui-radius-sm);
		padding: .7rem;
		display: flex;
		flex-direction: column;
		gap: .55rem;
	}

	.instances article.active {
		border-color: var(--cui-primary);
	}

	strong,
	small {
		display: block;
	}

	small {
		color: var(--cui-text-secondary);
		font-size: .7rem;
		word-break: break-all;
	}

	.actions {
		display: flex;
		gap: .35rem;
		flex-wrap: wrap;
	}

	.empty {
		color: var(--cui-text-secondary);
	}

	form {
		display: flex;
		flex-direction: column;
		gap: .7rem;
	}

	.form-head {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	label {
		display: flex;
		flex-direction: column;
		gap: .3rem;
		color: var(--cui-text-secondary);
		font-size: .68rem;
		text-transform: uppercase;
		letter-spacing: .07em;
	}

	input {
		min-height: var(--cui-touch-min);
		border: 1px solid var(--cui-input-border);
		border-radius: var(--cui-radius-sm);
		background: var(--cui-input-bg);
		color: var(--cui-text-primary);
		padding: .55rem .65rem;
		font: inherit;
		text-transform: none;
		letter-spacing: normal;
	}

	.two {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: .6rem;
	}

	.check {
		justify-content: end;
		flex-direction: row;
		align-items: center;
		text-transform: none;
		letter-spacing: normal;
	}

	.check input {
		min-height: auto;
	}

	.error {
		color: var(--cui-danger);
		margin: 0;
		font-size: .78rem;
	}

	@media (max-width:720px) {
		.layout,
		.two {
			grid-template-columns: 1fr;
		}

		.instances {
			max-height: 220px;
		}
	}
</style>
