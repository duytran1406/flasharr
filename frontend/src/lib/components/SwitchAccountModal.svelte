<script lang="ts">
  import { get } from "svelte/store";
  import { Button, Modal } from "@media-set/core-ui";
  import { toasts } from "$lib/stores/toasts";
  import { systemStore, torboxSettings, type TorBoxSettings } from "$lib/stores/system";

  type Provider = "fshare" | "torbox";

  interface Props {
    open: boolean;
    onclose: () => void;
    onswitched?: () => void;
  }

  let { open, onclose, onswitched }: Props = $props();

  let provider = $state<Provider>("fshare");
  let pickerOpen = $state(false);

  const PROVIDERS: Record<Provider, { label: string; icon: string }> = {
    fshare: { label: "Fshare", icon: "/images/logo_fshare.png" },
    torbox: { label: "TorBox", icon: "/images/logo_torbox.png" },
  };

  let email = $state("");
  let password = $state("");
  let showPassword = $state(false);
  let busy = $state(false);
  let torbox = $state<TorBoxSettings>({
    enabled: true,
    base_url: "https://api.torbox.app/v1/api",
    api_key: "",
    cache_timeout_secs: 3,
    show_cached_as_source: true,
  });
  let showApiKey = $state(false);

  async function loadTorBox() {
    await systemStore.fetchTorBoxSettings();
    torbox = get(torboxSettings);
  }

  async function submitTorBox(testOnly = false) {
    busy = true;
    try {
      const result = testOnly
        ? await systemStore.testTorBoxConnection({ ...torbox, enabled: true })
        : await systemStore.saveTorBoxSettings({ ...torbox, enabled: true });
      if (result.success) toasts.success(result.message || `TorBox ${testOnly ? "connected" : "settings saved"}`);
      else toasts.error(result.message || `TorBox ${testOnly ? "connection failed" : "save failed"}`);
      if (result.success && !testOnly) {
        onswitched?.();
        close();
      }
    } finally {
      busy = false;
    }
  }

  function selectProvider(next: Provider) {
    provider = next;
    pickerOpen = false;
    if (next === "torbox") loadTorBox();
  }

  function handleTriggerKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      pickerOpen = false;
      return;
    }
    if (["ArrowDown", "ArrowUp", "Enter", " "].includes(event.key)) {
      event.preventDefault();
      pickerOpen = true;
      queueMicrotask(() => document.getElementById(`provider-option-${provider}`)?.focus());
    }
  }

  function handleOptionKeydown(event: KeyboardEvent, option: Provider) {
    if (event.key === "Escape") {
      event.preventDefault();
      pickerOpen = false;
      document.getElementById("sw-provider")?.focus();
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const next: Provider = option === "fshare" ? "torbox" : "fshare";
      provider = next;
      if (next === "torbox") loadTorBox();
      queueMicrotask(() => document.getElementById(`provider-option-${next}`)?.focus());
    }
  }

  function reset() {
    provider = "fshare";
    pickerOpen = false;
    email = "";
    password = "";
    torbox = {
      enabled: true,
      base_url: "https://api.torbox.app/v1/api",
      api_key: "",
      cache_timeout_secs: 3,
      show_cached_as_source: true,
    };
    showPassword = false;
    showApiKey = false;
    busy = false;
  }

  function close() {
    reset();
    onclose();
  }

  async function submit() {
    if (provider === "torbox") {
      await submitTorBox();
      return;
    }
    if (!email.trim() || !password.trim()) {
      toasts.error("Please enter both email and password");
      return;
    }
    busy = true;
    try {
      const response = await fetch("/api/accounts", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email: email.trim(), password }),
      });
      const body = await response.json().catch(() => ({}));
      if (response.ok && body.success !== false) {
        toasts.success(body?.message || "Fshare account added");
        onswitched?.();
        close();
      } else {
        toasts.error(
          body?.message || (body?.persisted ? "Account saved but could not be verified" : "Failed to add Fshare account"),
        );
      }
    } catch (error) {
      toasts.error((error as Error)?.message || "Network error");
    } finally {
      busy = false;
    }
  }
</script>

<div class="switch-account-modal">
  <Modal {open} onClose={close} maxWidth="420px" accent="var(--color-primary, #00f3ff)" ariaLabel="Add account">
    {#snippet header()}
      <div class="modal-title-row">
        <span class="material-icons modal-icon">manage_accounts</span>
        <div>
          <p id="switch-modal-title" class="modal-title">ADD ACCOUNT</p>
          <p class="modal-sub">Connect an Fshare or TorBox account</p>
        </div>
      </div>
      <button class="close-btn" onclick={close} aria-label="Close"><span class="material-icons">close</span></button>
    {/snippet}

    <div class="field-group">
      <label class="field-label" for="sw-provider">SERVICE</label>
      <div class="provider-picker">
        <button
          id="sw-provider"
          class="provider-trigger"
          type="button"
          aria-haspopup="listbox"
          aria-expanded={pickerOpen}
          aria-controls="provider-options"
          disabled={busy}
          onclick={() => (pickerOpen = !pickerOpen)}
          onkeydown={handleTriggerKeydown}
        ><img src={PROVIDERS[provider].icon} alt="" /><span>{PROVIDERS[provider].label}</span><span class="material-icons" aria-hidden="true">expand_more</span></button>
        {#if pickerOpen}
          <div id="provider-options" class="provider-options" role="listbox" aria-label="Service provider">
            {#each ["fshare", "torbox"] as const as option}
              <button
                id={`provider-option-${option}`}
                type="button"
                role="option"
                aria-selected={provider === option}
                onclick={() => selectProvider(option)}
                onkeydown={(event) => handleOptionKeydown(event, option)}
              ><img src={PROVIDERS[option].icon} alt="" /><span>{PROVIDERS[option].label}</span>{#if provider === option}<span class="material-icons" aria-hidden="true">check</span>{/if}</button>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <form
      class="provider-body"
      onsubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      {#if provider === "fshare"}
        <div class="field-group">
          <label class="field-label" for="sw-email">EMAIL</label>
          <div class="input-row"><span class="material-icons input-icon">email</span><input id="sw-email" type="email" bind:value={email} placeholder="your@email.com" autocomplete="username" disabled={busy} /></div>
        </div>
        <div class="field-group">
          <label class="field-label" for="sw-password">PASSWORD</label>
          <div class="input-row"><span class="material-icons input-icon">lock</span><input id="sw-password" type={showPassword ? "text" : "password"} bind:value={password} placeholder="••••••••••••" autocomplete="current-password" disabled={busy} /><button class="vis-toggle" type="button" aria-label={showPassword ? "Hide password" : "Show password"} aria-pressed={showPassword} onclick={() => (showPassword = !showPassword)}><span class="material-icons" aria-hidden="true">{showPassword ? "visibility_off" : "visibility"}</span></button></div>
        </div>
        <Button type="submit" size="lg" icon={busy ? "sync" : "swap_horiz"} loading={busy} disabled={busy || !email || !password} width="100%">{busy ? "Switching…" : "Add Fshare"}</Button>
      {:else}
        <div class="field-group">
          <label class="field-label" for="sw-torbox-key">API KEY</label>
          <div class="input-row"><span class="material-icons input-icon">key</span><input id="sw-torbox-key" type={showApiKey ? "text" : "password"} bind:value={torbox.api_key} placeholder="TorBox API key" autocomplete="off" disabled={busy} /><button class="vis-toggle" type="button" aria-label={showApiKey ? "Hide API key" : "Show API key"} aria-pressed={showApiKey} onclick={() => (showApiKey = !showApiKey)}><span class="material-icons" aria-hidden="true">{showApiKey ? "visibility_off" : "visibility"}</span></button></div>
        </div>
        <p class="provider-note">Official API · api.torbox.app</p>
        <div class="torbox-actions"><Button variant="ghost" size="md" onclick={() => submitTorBox(true)} disabled={busy || !torbox.api_key}>Test</Button><Button size="md" icon="save" loading={busy} onclick={submit} disabled={busy || !torbox.api_key}>Save TorBox</Button></div>
      {/if}
    </form>
  </Modal>
</div>

<style>
  .switch-account-modal :global(.modal-panel) {
    width: min(520px,calc(100vw - 2rem));
    height: 470px;
    max-height: calc(100vh - 2rem);
  }

  .switch-account-modal :global(.modal-body) {
    overflow-y: hidden;
    padding: 2rem 2.25rem;
  }

  .modal-title-row {
    display: flex;
    align-items: center;
    gap: .75rem;
  }

  .modal-icon {
    font-size: 1.6rem;
    color: var(--color-primary, #00f3ff);
  }

  .modal-title {
    font-family: var(--font-mono, monospace);
    font-size: .75rem;
    font-weight: 700;
    letter-spacing: .12em;
    color: #fff;
    margin: 0;
  }

  .modal-sub {
    font-size: .68rem;
    color: #ffffff59;
    margin: .1rem 0 0;
  }

  .close-btn {
    width: 44px;
    height: 44px;
    background: none;
    border: none;
    color: #ffffff4d;
    cursor: pointer;
    padding: 4px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: color .2s,background .2s;
  }

  .close-btn:hover {
    color: #fff;
    background: #ffffff0f;
  }

  .field-group {
    display: flex;
    flex-direction: column;
    gap: .55rem;
  }

  .field-label {
    font-family: var(--font-mono, monospace);
    font-size: .58rem;
    font-weight: 700;
    letter-spacing: .12em;
    color: #ffffff59;
  }

  .provider-picker {
    position: relative;
  }

  .provider-trigger,
  .provider-options button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: .65rem;
    border: 1px solid rgba(255,255,255,.08);
    color: #ffffffe6;
    font: 700 .75rem var(--font-mono,monospace);
    cursor: pointer;
  }

  .provider-trigger {
    min-height: 44px;
    padding: .55rem .7rem;
    border-radius: 10px;
    background: #ffffff0a;
    text-align: left;
  }

  .provider-trigger img,
  .provider-options img {
    width: 22px;
    height: 22px;
    object-fit: contain;
    filter: drop-shadow(0 0 5px rgba(255,255,255,.15));
  }

  .provider-trigger .material-icons {
    margin-left: auto;
    color: #ffffff73;
  }

  .provider-trigger:focus-visible,
  .provider-options button:focus-visible {
    outline: 2px solid var(--cui-primary,#22d3ee);
    outline-offset: 2px;
  }

  .provider-options {
    position: absolute;
    z-index: 2;
    top: calc(100% + 6px);
    left: 0;
    right: 0;
    padding: .3rem;
    border: 1px solid var(--cui-glass-border,rgba(255,255,255,.1));
    border-radius: 10px;
    background: var(--cui-surface-2,#121820);
    box-shadow: var(--cui-shadow-glass,0 10px 28px rgba(0,0,0,.45));
  }

  .provider-options button {
    min-height: 44px;
    padding: .45rem .55rem;
    border-color: transparent;
    border-radius: 7px;
    background: transparent;
    text-align: left;
  }

  .provider-options button:hover,
  .provider-options button[aria-selected=true] {
    background: color-mix(in srgb,var(--cui-primary,#22d3ee) 10%,transparent);
  }

  .provider-options button .material-icons {
    margin-left: auto;
    color: var(--cui-primary,#22d3ee);
    font-size: 1rem;
  }

  .input-row {
    display: flex;
    align-items: center;
    gap: .5rem;
    background: #ffffff0a;
    border: 1px solid rgba(255,255,255,.08);
    border-radius: 10px;
    padding: 0 .6rem;
    transition: border-color .2s;
  }

  .input-row:focus-within {
    border-color: #dc3c3c80;
    box-shadow: 0 0 0 2px #dc3c3c1a;
  }

  .input-icon {
    font-size: 1rem;
    color: #ffffff40;
    flex-shrink: 0;
  }

  .input-row input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: #ffffffe6;
    font-size: .82rem;
    padding: .62rem 0;
    font-family: inherit;
  }

  .input-row input::placeholder {
    color: #fff3;
  }

  .input-row input:disabled {
    opacity: .5;
  }

  .provider-body {
    height: 15rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    padding-top: 1.25rem;
  }

  .provider-note {
    margin: -.1rem 0 .2rem;
    color: #ffffff59;
    font: .64rem var(--font-mono,monospace);
  }

  .torbox-actions {
    display: flex;
    justify-content: flex-end;
    gap: .55rem;
  }

  .vis-toggle {
    width: 44px;
    height: 44px;
    background: none;
    border: none;
    color: #ffffff40;
    cursor: pointer;
    padding: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: color .2s;
    flex-shrink: 0;
  }

  .vis-toggle:hover {
    color: #fff9;
  }

  .vis-toggle .material-icons {
    font-size: 1rem;
  }

</style>
