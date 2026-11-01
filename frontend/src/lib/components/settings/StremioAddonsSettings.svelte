<script lang="ts">
  import { onMount } from "svelte";
  import { Button, Modal, Tooltip } from "@media-set/core-ui";
  import { toasts } from "$lib/stores/toasts";

  interface StremioAddon {
    id: string;
    name: string;
    version?: string | null;
    description?: string | null;
    logo?: string | null;
    enabled: boolean;
  }

  const PRESETS = [
    { name: "Torrentio", url: "https://torrentio.strem.fun/manifest.json" },
    { name: "MediaFusion", url: "https://mediafusion.elfhosted.com/manifest.json" },
    { name: "Comet", url: "https://comet.elfhosted.com/manifest.json" },
  ];

  let installUrl = $state("");
  let addons = $state<StremioAddon[]>([]);
  let preview = $state<StremioAddon | null>(null);
  let pendingDelete = $state<StremioAddon | null>(null);
  let message = $state("");
  let messageIsError = $state(false);
  let busy = $state(false);
  let refreshing = $state(false);
  let rowBusyId = $state<string | null>(null);
  let failedLogos = $state<string[]>([]);

  function setMessage(text: string, isError = false, toast = false) {
    message = text;
    messageIsError = isError;
    if (toast) isError ? toasts.error(text) : toasts.success(text);
  }

  function isValidManifestUrl(value: string): boolean {
    try {
      const url = new URL(value.trim());
      return url.protocol === "https:" && !url.username && !url.password && url.pathname.endsWith("/manifest.json");
    } catch {
      return false;
    }
  }

  async function loadAddons() {
    try {
      const response = await fetch("/api/settings/stremio/addons");
      if (!response.ok) throw new Error("Unable to load addons.");
      addons = await response.json();
      failedLogos = [];
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Unable to load addons.", true);
    }
  }

  async function previewAddon() {
    if (!isValidManifestUrl(installUrl)) {
      setMessage("Invalid manifest URL. Use a public HTTPS manifest endpoint.", true);
      return;
    }
    busy = true;
    setMessage("");
    try {
      const response = await fetch("/api/settings/stremio/addons/preview", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ installUrl: installUrl.trim() }),
      });
      if (!response.ok) throw new Error((await response.text()) || "Unable to preview addon.");
      preview = await response.json();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Unable to preview addon.", true, true);
    } finally {
      busy = false;
    }
  }

  async function installAddon() {
    busy = true;
    try {
      const response = await fetch("/api/settings/stremio/addons", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ installUrl: installUrl.trim() }),
      });
      if (response.status === 409) {
        const text = `${preview?.name ?? "Addon"} already installed`;
        setMessage(text, true);
        toasts.warning(text);
        return;
      }
      if (!response.ok) throw new Error((await response.text()) || "Unable to add addon.");
      installUrl = "";
      preview = null;
      setMessage("Addon added.", false, true);
      await loadAddons();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Unable to add addon.", true, true);
    } finally {
      busy = false;
    }
  }

  async function refreshAll() {
    if (refreshing || addons.length === 0) return;
    refreshing = true;
    setMessage("");
    const queue = [...addons];
    const total = queue.length;
    let succeeded = 0;
    let failed = 0;
    const progressText = (done: number) => `Refreshing ${done}/${total} addons. ${succeeded} success, ${failed} failed`;
    const toastId = toasts.info(progressText(0), null);
    try {
      for (const addon of queue) {
        try {
          const response = await fetch(`/api/settings/stremio/addons/${encodeURIComponent(addon.id)}`, { method: "POST" });
          if (!response.ok) throw new Error("Unable to refresh addon.");
          const refreshed: StremioAddon = await response.json();
          addons = addons.map((item) => (item.id === refreshed.id ? refreshed : item));
          succeeded += 1;
        } catch {
          failed += 1;
        }
        toasts.update(toastId, { message: progressText(succeeded + failed) });
      }
      toasts.update(toastId, {
        type: failed === 0 ? "success" : "warning",
        message: progressText(total),
        duration: 7000,
      });
      await loadAddons();
    } finally {
      refreshing = false;
    }
  }

  function pickPreset(name: string, url: string) {
    if (addons.some((addon) => addon.name.toLocaleLowerCase() === name.toLocaleLowerCase())) {
      toasts.warning(`${name} already installed`);
      return;
    }
    installUrl = url;
    preview = null;
    setMessage("");
  }

  async function toggleAddon(addon: StremioAddon) {
    rowBusyId = addon.id;
    try {
      const response = await fetch(`/api/settings/stremio/addons/${encodeURIComponent(addon.id)}/enabled`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ enabled: !addon.enabled }),
      });
      if (!response.ok) throw new Error("Unable to update addon status.");
      await loadAddons();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Unable to update addon status.", true);
    } finally {
      rowBusyId = null;
    }
  }

  async function moveAddon(addon: StremioAddon, offset: number) {
    const index = addons.indexOf(addon);
    const target = index + offset;
    if (target < 0 || target >= addons.length) return;
    const previous = [...addons];
    [addons[index], addons[target]] = [addons[target], addons[index]];
    addons = [...addons];
    rowBusyId = addon.id;
    try {
      const response = await fetch("/api/settings/stremio/addons/reorder", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ ids: addons.map((item) => item.id) }),
      });
      if (!response.ok) throw new Error("Unable to reorder addons.");
    } catch (error) {
      addons = previous;
      setMessage(error instanceof Error ? error.message : "Unable to reorder addons.", true);
    } finally {
      rowBusyId = null;
    }
  }

  async function deleteAddon() {
    if (!pendingDelete) return;
    busy = true;
    try {
      const response = await fetch(`/api/settings/stremio/addons/${encodeURIComponent(pendingDelete.id)}`, { method: "DELETE" });
      if (!response.ok) throw new Error("Unable to delete addon.");
      pendingDelete = null;
      setMessage("Addon deleted.");
      await loadAddons();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Unable to delete addon.", true);
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    loadAddons();
  });
</script>

<section class="stremio-surface" aria-labelledby="stremio-addons-title">
  <div class="bento-head">
    <svg class="head-logo" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M15.39 4.39a1 1 0 0 0 1.68-.474 2.5 2.5 0 1 1 3.014 3.015 1 1 0 0 0-.474 1.68l1.683 1.682a2.414 2.414 0 0 1 0 3.414L19.61 15.39a1 1 0 0 1-1.68-.474 2.5 2.5 0 1 0-3.014 3.015 1 1 0 0 1 .474 1.68l-1.683 1.682a2.414 2.414 0 0 1-3.414 0L8.61 19.61a1 1 0 0 0-1.68.474 2.5 2.5 0 1 1-3.014-3.015 1 1 0 0 0 .474-1.68l-1.683-1.682a2.414 2.414 0 0 1 0-3.414L4.39 8.61a1 1 0 0 1 1.68.474 2.5 2.5 0 1 0 3.014-3.015 1 1 0 0 1-.474-1.68l1.683-1.682a2.414 2.414 0 0 1 3.414 0z"></path></svg>
    <h2 id="stremio-addons-title">STREMIO ADDONS</h2>
    <span class="installed-count">{addons.length} INSTALLED</span>
    <Tooltip text="Refresh addon manifests">
      <button class="refresh-action" type="button" aria-label="Refresh addon manifests" disabled={refreshing || busy || addons.length === 0} onclick={refreshAll}><span class="material-icons" class:spinning={refreshing} aria-hidden="true">refresh</span></button>
    </Tooltip>
  </div>
  <div class="card-banner">
    <div class="banner-dots"></div>
    <div class="banner-logo-ring" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M15.39 4.39a1 1 0 0 0 1.68-.474 2.5 2.5 0 1 1 3.014 3.015 1 1 0 0 0-.474 1.68l1.683 1.682a2.414 2.414 0 0 1 0 3.414L19.61 15.39a1 1 0 0 1-1.68-.474 2.5 2.5 0 1 0-3.014 3.015 1 1 0 0 1 .474 1.68l-1.683 1.682a2.414 2.414 0 0 1-3.414 0L8.61 19.61a1 1 0 1 0-1.68.474 2.5 2.5 0 1 1-3.014-3.015 1 1 0 0 0 .474-1.68l-1.683-1.682a2.414 2.414 0 0 1 0-3.414L4.39 8.61a1 1 0 0 1 1.68.474 2.5 2.5 0 1 0 3.014-3.015 1 1 0 0 1-.474-1.68l1.683-1.682a2.414 2.414 0 0 1 3.414 0z"></path></svg></div>
  </div>
  <div class="bento-body">
    <div class="add-block">
      <form
        onsubmit={(event) => {
          event.preventDefault();
          previewAddon();
        }}
      >
        <input id="stremio-install-url" aria-label="Addon manifest URL" type="url" bind:value={installUrl} placeholder="https://addon.example.com/manifest.json" autocomplete="url" disabled={busy} />
        <Button class="primary-add" type="submit" size="sm" accent="var(--addon-accent)" title="Add addon" disabled={busy || !installUrl.trim()}>+</Button>
      </form>
      <div class="quick-add">
        <span>Quick add</span>
        {#each PRESETS as preset}
          <button type="button" disabled={busy} onclick={() => pickPreset(preset.name, preset.url)}><svg class="puzzle-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M15.39 4.39a1 1 0 0 0 1.68-.474 2.5 2.5 0 1 1 3.014 3.015 1 1 0 0 0-.474 1.68l1.683 1.682a2.414 2.414 0 0 1 0 3.414L19.61 15.39a1 1 0 0 1-1.68-.474 2.5 2.5 0 1 0-3.014 3.015 1 1 0 0 1 .474 1.68l-1.683 1.682a2.414 2.414 0 0 1-3.414 0L8.61 19.61a1 1 0 0 0-1.68.474 2.5 2.5 0 1 1-3.014-3.015 1 1 0 0 0 .474-1.68l-1.683-1.682a2.414 2.414 0 0 1 0-3.414L4.39 8.61a1 1 0 0 1 1.68.474 2.5 2.5 0 1 0 3.014-3.015 1 1 0 0 1-.474-1.68l1.683-1.682a2.414 2.414 0 0 1 3.414 0z"></path></svg> {preset.name}</button>
        {/each}
      </div>
    </div>
    <div class="block-divider"></div>
    {#if message && messageIsError}
      <p class="status error" role="status" aria-live="polite">{message}</p>
    {/if}
    {#if addons.length}
      <div class="list-header">
        <h3>YOUR ADDONS</h3>
        <span>{addons.filter((addon) => addon.enabled).length} enabled</span>
      </div>
      <ol class="addon-list" aria-label="Addon resolution priority">
        {#each addons as addon, index}
          <li class="addon-card" class:disabled={!addon.enabled}>
            <div class="card-summary">
              <div class="addon-icon" aria-hidden="true">
                {#if addon.logo && !failedLogos.includes(addon.id)}
                  <img src={addon.logo} alt="" referrerpolicy="no-referrer" onerror={() => (failedLogos = [...failedLogos, addon.id])} />
                {:else}
                  <svg class="puzzle-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M15.39 4.39a1 1 0 0 0 1.68-.474 2.5 2.5 0 1 1 3.014 3.015 1 1 0 0 0-.474 1.68l1.683 1.682a2.414 2.414 0 0 1 0 3.414L19.61 15.39a1 1 0 0 1-1.68-.474 2.5 2.5 0 1 0-3.014 3.015 1 1 0 0 1 .474 1.68l-1.683 1.682a2.414 2.414 0 0 1-3.414 0L8.61 19.61a1 1 0 0 0-1.68.474 2.5 2.5 0 1 1-3.014-3.015 1 1 0 0 0 .474-1.68l-1.683-1.682a2.414 2.414 0 0 1 0-3.414L4.39 8.61a1 1 0 0 1 1.68.474 2.5 2.5 0 1 0 3.014-3.015 1 1 0 0 1-.474-1.68l1.683-1.682a2.414 2.414 0 0 1 3.414 0z"></path></svg>
                {/if}
              </div>
              <div class="addon-copy">
                <div class="addon-name"><strong>{addon.name}</strong><span>· v{addon.version || "—"}</span></div>
                <p>{addon.description || "No description provided."}</p>
              </div>
            </div>
            <div class="card-divider"></div>
            <div class="card-controls">
              <label class="addon-toggle">
                <input type="checkbox" checked={addon.enabled} disabled={rowBusyId !== null} onchange={() => toggleAddon(addon)} />
                <span aria-hidden="true"></span>
                <b>{addon.enabled ? "Enabled" : "Disabled"}</b>
              </label>
              <div class="row-actions">
                <Tooltip text="Move up in resolution priority">
                  <button type="button" class="icon-action" aria-label={`Move ${addon.name} up`} disabled={index === 0 || rowBusyId !== null} onclick={() => moveAddon(addon, -1)}>↑</button>
                </Tooltip>
                <Tooltip text="Move down in resolution priority">
                  <button type="button" class="icon-action" aria-label={`Move ${addon.name} down`} disabled={index === addons.length - 1 || rowBusyId !== null} onclick={() => moveAddon(addon, 1)}>↓</button>
                </Tooltip>
                <Tooltip text="Delete addon">
                  <button type="button" class="icon-action danger material-icons" aria-label={`Delete ${addon.name}`} disabled={rowBusyId !== null} onclick={() => (pendingDelete = addon)}>delete</button>
                </Tooltip>
              </div>
            </div>
          </li>
        {/each}
      </ol>
    {:else}
      <p class="empty-state">No addons installed. Add a manifest URL or choose a preset.</p>
    {/if}
  </div>
</section>

{#if preview}
  {@const addon = preview}
  <Modal open={true} onClose={() => !busy && (preview = null)} maxWidth="480px" ariaLabel={`Add ${addon.name}`}>
    {#snippet header()}
      <div class="modal-heading">
        <span class="modal-addon-icon" aria-hidden="true">
          {#if addon.logo && !failedLogos.includes(addon.id)}
            <img src={addon.logo} alt="" referrerpolicy="no-referrer" onerror={() => (failedLogos = [...failedLogos, addon.id])} />
          {:else}
            <svg class="puzzle-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M15.39 4.39a1 1 0 0 0 1.68-.474 2.5 2.5 0 1 1 3.014 3.015 1 1 0 0 0-.474 1.68l1.683 1.682a2.414 2.414 0 0 1 0 3.414L19.61 15.39a1 1 0 0 1-1.68-.474 2.5 2.5 0 1 0-3.014 3.015 1 1 0 0 1 .474 1.68l-1.683 1.682a2.414 2.414 0 0 1-3.414 0L8.61 19.61a1 1 0 0 0-1.68.474 2.5 2.5 0 1 1-3.014-3.015 1 1 0 0 0 .474-1.68l-1.683-1.682a2.414 2.414 0 0 1 0-3.414L4.39 8.61a1 1 0 0 1 1.68.474 2.5 2.5 0 1 0 3.014-3.015 1 1 0 0 1-.474-1.68l1.683-1.682a2.414 2.414 0 0 1 3.414 0z"></path></svg>
          {/if}
        </span>
        <strong>{addon.name} · v{addon.version || "—"}</strong>
      </div>
      <button class="close-action material-icons" type="button" aria-label="Close" disabled={busy} onclick={() => (preview = null)}>close</button>
    {/snippet}
    {#snippet children()}
      <p class="modal-copy">{addon.description || "No description provided."}</p>
    {/snippet}
    {#snippet footer()}
      <div class="modal-actions"><Button variant="ghost" size="md" disabled={busy} onclick={() => (preview = null)}>Cancel</Button><Button variant="primary" size="md" accent="var(--addon-accent)" loading={busy} onclick={installAddon}>Import addon</Button></div>
    {/snippet}
  </Modal>
{/if}

{#if pendingDelete}
  {@const addon = pendingDelete}
  <Modal open={true} onClose={() => !busy && (pendingDelete = null)} maxWidth="440px" accent="var(--cui-danger)" ariaLabel={`Delete ${addon.name}`}>
    {#snippet header()}
      <div class="modal-heading danger"><span class="material-icons" aria-hidden="true">delete</span><strong>Delete {addon.name}?</strong></div>
      <button class="close-action material-icons" type="button" aria-label="Close" disabled={busy} onclick={() => (pendingDelete = null)}>close</button>
    {/snippet}
    {#snippet children()}
      <p class="modal-copy">This removes {addon.name} from the Stremio stream-resolution pipeline. Future searches and playback will no longer receive streams from this addon.</p>
    {/snippet}
    {#snippet footer()}
      <div class="modal-actions"><Button variant="ghost" size="md" disabled={busy} onclick={() => (pendingDelete = null)}>Cancel</Button><Button variant="danger" size="md" loading={busy} onclick={deleteAddon}>Delete addon</Button></div>
    {/snippet}
  </Modal>
{/if}

<style>
  .stremio-surface {
    --addon-accent: #40FD14;
    display: flex;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    max-height: 100%;
    box-sizing: border-box;
    flex-direction: column;
    overflow: hidden;
    border: 1px solid color-mix(in srgb,var(--addon-accent) 35%,transparent);
    border-radius: 14px;
    background: #080a0fb3;
    backdrop-filter: blur(12px);
  }

  .bento-head,
  .card-banner,
  .refresh-action,
  .quick-add,
  .list-header,
  .card-summary,
  .card-controls,
  .addon-toggle,
  .row-actions,
  .modal-heading,
  .modal-actions {
    display: flex;
    align-items: center;
  }

  .bento-head {
    position: relative;
    flex: none;
    gap: .5rem;
    overflow: hidden;
    padding: .55rem .7rem;
    border-left: 3px solid var(--addon-accent);
    font-family: var(--cui-font-mono,monospace);
  }

  .bento-head:after,
  .banner-dots {
    content: "";
    position: absolute;
    inset: 0;
    background-image: radial-gradient(rgba(0,0,0,.25) 1px,transparent 1px);
    background-size: 6px 6px;
  }

  .bento-head:after {
    mask-image: linear-gradient(to right,#000,transparent 70%);
  }

  .bento-head > * {
    position: relative;
    z-index: 1;
  }

  .bento-head h2 {
    flex: 1;
    margin: 0;
    color: #fff;
    font-size: .75rem;
    font-weight: 800;
    letter-spacing: .12em;
  }

  .head-logo {
    width: 22px;
    height: 22px;
    color: var(--addon-accent);
    filter: drop-shadow(0 0 6px color-mix(in srgb,var(--addon-accent) 65%,transparent));
  }

  .installed-count {
    color: color-mix(in srgb,var(--addon-accent) 75%,var(--cui-text-secondary));
    font: 800 .58rem/1 var(--cui-font-mono,monospace);
    letter-spacing: .08em;
  }

  .card-banner {
    position: relative;
    justify-content: center;
    height: 72px;
    box-sizing: border-box;
    flex: none;
    background: linear-gradient(135deg,color-mix(in srgb,var(--addon-accent) 15%,transparent),color-mix(in srgb,var(--addon-accent) 6%,transparent) 50%,#0a0a0f80);
  }

  .banner-logo-ring {
    position: relative;
    z-index: 1;
    display: grid;
    width: 48px;
    height: 48px;
    place-items: center;
    flex: none;
  }

  .banner-logo-ring svg {
    width: 42px;
    height: 42px;
    color: var(--addon-accent);
    filter: drop-shadow(0 0 10px color-mix(in srgb,var(--addon-accent) 70%,transparent)) drop-shadow(0 0 20px color-mix(in srgb,var(--addon-accent) 35%,transparent));
  }

  .bento-body {
    display: flex;
    min-width: 0;
    min-height: 0;
    flex: 1;
    flex-direction: column;
    overflow: hidden;
    padding: .75rem .85rem .25rem;
  }

  .refresh-action {
    display: grid;
    width: 44px;
    min-width: 44px;
    height: 44px;
    min-height: 44px;
    place-items: center;
    flex: none;
    padding: 0;
    border: 1px solid color-mix(in srgb,var(--addon-accent) 45%,var(--cui-glass-border));
    border-radius: 4px;
    background: color-mix(in srgb,var(--addon-accent) 8%,#070b07);
    color: var(--addon-accent);
    cursor: pointer;
  }

  .refresh-action:hover:not(:disabled),
  .refresh-action:active:not(:disabled) {
    border-color: var(--addon-accent);
    background: color-mix(in srgb,var(--addon-accent) 16%,#070b07);
    color: var(--addon-accent);
  }

  .refresh-action:disabled {
    opacity: .4;
    cursor: not-allowed;
  }

  .refresh-action .material-icons {
    font-size: 1rem;
  }

  .spinning {
    animation: spin .8s linear infinite;
  }

  .block-divider {
    height: 1px;
    background: var(--cui-glass-border);
  }

  .card-divider {
    height: 1px;
    background: #183f19;
  }

  .block-divider {
    margin: 1rem 0;
  }

  .list-header h3 {
    color: var(--cui-text-secondary);
    font: 800 .7rem/1 var(--cui-font-mono,monospace);
    letter-spacing: .08em;
  }

  .add-block form {
    display: grid;
    grid-template-columns: minmax(0,1fr) auto;
    gap: .5rem;
  }

  .add-block input {
    min-width: 0;
    min-height: 44px;
    padding: .55rem .8rem;
    border: 1px solid var(--cui-glass-border);
    border-radius: 8px;
    background: var(--cui-surface-0);
    color: var(--cui-text-primary);
    font-size: .8rem;
  }

  .add-block input:focus-visible,
  .quick-add button:focus-visible,
  .refresh-action:focus-visible,
  .icon-action:focus-visible {
    outline: 2px solid var(--addon-accent);
    outline-offset: 2px;
  }

  :global(.primary-add.flasharr-btn) {
    display: grid;
    width: 44px;
    min-width: 44px;
    height: 44px;
    min-height: 44px;
    place-items: center;
    padding: 0;
    font-size: 1.15rem;
  }

  :global(.primary-add.flasharr-btn:disabled) {
    opacity: .45;
    cursor: not-allowed;
  }

  .quick-add {
    gap: .45rem;
    margin-top: .7rem;
    flex-wrap: wrap;
  }

  .quick-add > span {
    margin-right: .2rem;
    color: var(--cui-text-secondary);
    font-size: .72rem;
  }

  .quick-add button {
    display: flex;
    align-items: center;
    gap: .3rem;
    min-height: 44px;
    padding: .35rem .65rem;
    border: 1px solid color-mix(in srgb,var(--addon-accent) 20%,var(--cui-glass-border));
    border-radius: 4px;
    background: #070b07;
    color: var(--cui-text-secondary);
    font-size: .72rem;
    cursor: pointer;
  }

  .quick-add button:hover:not(:disabled) {
    border-color: color-mix(in srgb,var(--addon-accent) 55%,var(--cui-glass-border));
    background: color-mix(in srgb,var(--addon-accent) 8%,#070b07);
    color: var(--addon-accent);
  }

  .quick-add .puzzle-icon {
    width: 15px;
    height: 15px;
    color: var(--cui-text-tertiary);
  }

  .list-header {
    justify-content: space-between;
    gap: 1rem;
    flex: none;
  }

  .list-header h3 {
    margin: 0;
  }

  .list-header span {
    color: var(--cui-text-secondary);
    font-size: .72rem;
  }

  .status,
  .empty-state {
    margin: .75rem 0 0;
    color: var(--cui-success);
    font-size: .75rem;
    line-height: 1.45;
  }

  .status.error {
    color: var(--cui-danger);
  }

  .empty-state {
    padding: 1rem 0;
    color: var(--cui-text-secondary);
  }

  .addon-list {
    display: grid;
    width: 100%;
    min-width: 0;
    min-height: 0;
    align-content: start;
    gap: .65rem;
    padding: 0 .35rem .25rem 0;
    margin: .8rem 0 0;
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior-y: contain;
    list-style: none;
    scrollbar-gutter: stable;
  }

  .addon-card {
    position: relative;
    width: auto;
    max-width: 100%;
    min-width: 0;
    box-sizing: border-box;
    padding: .8rem;
    border: 1px solid color-mix(in srgb,var(--addon-accent) 22%,var(--cui-glass-border));
    border-radius: 10px;
    background: linear-gradient(135deg,color-mix(in srgb,var(--addon-accent) 7%,var(--cui-surface-2,#17212b)),var(--cui-surface-2,#17212b) 42%,color-mix(in srgb,var(--cui-primary) 4%,var(--cui-surface-2,#17212b)));
    box-shadow: inset 3px 0 color-mix(in srgb,var(--addon-accent) 65%,transparent),0 8px 24px #00000029;
    transition: opacity .18s,border-color .18s,background .18s;
  }

  .addon-card:hover {
    border-color: color-mix(in srgb,var(--addon-accent) 40%,var(--cui-glass-border));
    background: linear-gradient(135deg,color-mix(in srgb,var(--addon-accent) 10%,var(--cui-surface-2,#17212b)),var(--cui-surface-2,#17212b) 48%,color-mix(in srgb,var(--cui-primary) 7%,var(--cui-surface-2,#17212b)));
  }

  .addon-card.disabled {
    opacity: .62;
  }

  .card-summary {
    min-width: 0;
    align-items: flex-start;
    gap: .65rem;
  }

  .addon-icon {
    display: grid;
    width: 44px;
    height: 44px;
    place-items: center;
    flex: none;
    overflow: hidden;
    border-radius: 50%;
  }

  .puzzle-icon {
    width: 24px;
    height: 24px;
    flex: none;
  }

  .addon-icon img {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    object-fit: cover;
  }

  .addon-copy {
    min-width: 0;
    flex: 1;
  }

  .addon-name {
    display: flex;
    align-items: baseline;
    gap: .35rem;
    min-width: 0;
  }

  .addon-name strong {
    overflow: hidden;
    color: var(--cui-text-primary);
    font-size: .86rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .addon-name span {
    color: var(--cui-text-tertiary);
    font: 600 .7rem var(--cui-font-mono,monospace);
    white-space: nowrap;
  }

  .addon-copy p {
    display: -webkit-box;
    overflow: hidden;
    margin: .3rem 0 0;
    color: var(--cui-text-secondary);
    font-size: .75rem;
    line-height: 1.45;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
  }

  .card-divider {
    margin: .15rem 0;
  }

  .card-controls {
    justify-content: space-between;
    gap: .75rem;
  }

  .addon-toggle {
    gap: .55rem;
    min-height: 40px;
    cursor: pointer;
  }

  .addon-toggle input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }

  .addon-toggle > span {
    position: relative;
    width: 34px;
    height: 20px;
    border: 1px solid color-mix(in srgb,var(--addon-accent) 40%,#101410);
    border-radius: 2px;
    background: #050805;
    box-shadow: inset 0 0 0 1px #000000e6;
  }

  .addon-toggle > span:after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 12px;
    height: 12px;
    border: 1px solid #252b25;
    border-radius: 1px;
    background: #111611;
    transition: transform .14s,background .14s,box-shadow .14s;
  }

  .addon-toggle input:checked + span {
    border-color: var(--addon-accent);
    background: #071006;
    box-shadow: inset 0 0 8px color-mix(in srgb,var(--addon-accent) 20%,transparent),0 0 10px color-mix(in srgb,var(--addon-accent) 14%,transparent);
  }

  .addon-toggle input:checked + span:after {
    transform: translate(14px);
    border-color: var(--addon-accent);
    background: var(--addon-accent);
    box-shadow: 0 0 8px color-mix(in srgb,var(--addon-accent) 75%,transparent);
  }

  .addon-toggle input:focus-visible + span {
    outline: 2px solid var(--addon-accent);
    outline-offset: 2px;
  }

  .addon-toggle b {
    color: var(--cui-text-secondary);
    font: 800 .68rem/1 var(--cui-font-mono,monospace);
    letter-spacing: .04em;
  }

  .addon-toggle input:checked ~ b {
    color: var(--addon-accent);
  }

  .row-actions {
    gap: .25rem;
  }

  .icon-action {
    display: grid;
    place-items: center;
    width: 44px;
    min-width: 44px;
    height: 44px;
    min-height: 44px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 7px;
    background: transparent;
    color: var(--cui-text-secondary);
    font-size: .9rem;
    cursor: pointer;
  }

  .icon-action:hover:not(:disabled) {
    border-color: var(--cui-glass-border);
    background: var(--cui-surface-hover);
    color: var(--cui-text-primary);
  }

  .icon-action.danger {
    color: var(--cui-danger);
    font-size: 1rem;
  }

  .icon-action:disabled {
    opacity: .3;
    cursor: not-allowed;
  }

  .modal-heading {
    gap: .55rem;
    min-width: 0;
  }

  .modal-heading .material-icons {
    color: var(--addon-accent);
  }

  .modal-addon-icon {
    display: grid;
    width: 34px;
    height: 34px;
    place-items: center;
    flex: none;
    overflow: hidden;
    border: 1px solid color-mix(in srgb,var(--addon-accent) 30%,var(--cui-glass-border));
    border-radius: 8px;
    background: color-mix(in srgb,var(--addon-accent) 8%,transparent);
  }

  .modal-addon-icon img {
    width: 24px;
    height: 24px;
    object-fit: contain;
  }

  .modal-heading.danger .material-icons,
  .modal-heading.danger strong {
    color: var(--cui-danger);
  }

  .close-action {
    display: grid;
    place-items: center;
    width: 44px;
    min-width: 44px;
    height: 44px;
    min-height: 44px;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--cui-text-secondary);
    cursor: pointer;
  }

  .modal-copy {
    margin: 0;
    color: var(--cui-text-secondary);
    font-size: .82rem;
    line-height: 1.55;
  }

  .modal-actions {
    justify-content: flex-end;
    gap: .5rem;
    width: 100%;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (max-width:900px) {
    .stremio-surface {
      height: auto;
      max-height: none;
      overflow: visible;
    }
    .addon-list {
      height: auto;
      overflow: visible;
    }
  }

  @media (max-width:600px) {
    .card-banner {
      min-height: 72px;
    }
    .refresh-action {
      width: 44px;
      height: 44px;
    }
    .bento-body {
      padding: .7rem .7rem .25rem;
    }
    .add-block form {
      grid-template-columns: minmax(0,1fr) auto;
    }
    :global(.primary-add.flasharr-btn) {
      width: 44px;
      height: 44px;
    }
    .quick-add button {
      min-height: 44px;
    }
    .card-summary {
      display: grid;
      grid-template-columns: 44px minmax(0,1fr);
    }
    .card-controls {
      align-items: flex-end;
      flex-direction: column;
    }
    .addon-toggle {
      min-height: 44px;
      align-self: flex-start;
    }
    .row-actions {
      align-self: stretch;
      justify-content: flex-end;
    }
    .icon-action {
      width: 44px;
      height: 44px;
    }
  }

  @media (prefers-reduced-motion:reduce) {
    .spinning {
      animation: none;
    }
    .addon-card,
    .addon-toggle > span:after {
      transition: none;
    }
  }

</style>
