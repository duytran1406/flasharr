<script lang="ts">
  import { accountStore } from "$lib/stores/account.svelte";
  import { toasts } from "$lib/stores/toasts";
  import { Badge } from "@media-set/core-ui";
  import { Modal } from "@media-set/core-ui";
  import { Button } from "@media-set/core-ui";

  let { onDismiss = () => {} }: { onDismiss?: () => void } = $props();

  let showSwitchForm = $state(false);
  let newEmail = $state("");
  let newPassword = $state("");
  let switching = $state(false);
  let showPassword = $state(false);

  const account = $derived(accountStore.primary);

  async function handleSwitch() {
    if (!newEmail.trim() || !newPassword.trim()) {
      toasts.error("Please enter both email and password");
      return;
    }
    switching = true;
    try {
      const result = await accountStore.replace(newEmail.trim(), newPassword.trim());
      if (result.success) {
        toasts.success(result.message || "Account replaced successfully!");
        newEmail = "";
        newPassword = "";
        showSwitchForm = false;
        onDismiss();
      } else {
        toasts.error(result.message || "Failed to replace account — check credentials");
      }
    } catch {
      toasts.error("Something went wrong while switching account");
    } finally {
      switching = false;
    }
  }

  function dismiss() {
    showSwitchForm = false;
    onDismiss();
  }
</script>

<Modal
  open={true}
  onClose={dismiss}
  maxWidth="460px"
  accent="#ffaa00"
  ariaLabel="Account Warning"
>
  {#snippet header()}
    <div class="header-icon">
      <span class="material-icons">warning_amber</span>
    </div>
    <div class="header-text">
      <h2>Account No Longer VIP</h2>
      <p>Downloads are paused until a valid VIP account is connected</p>
    </div>
    <button class="close-btn" onclick={dismiss} aria-label="Dismiss">
      <span class="material-icons">close</span>
    </button>
  {/snippet}

  {#snippet children()}
    <!-- Account info -->
    <div class="account-status-row">
      <span class="material-icons acc-icon">account_circle</span>
      <div class="acc-info">
        <span class="acc-email">{account?.email ?? "Unknown"}</span>
        <Badge
          text="{account?.rank ?? 'FREE'} — No download access"
          variant="danger"
          size="sm"
        />
      </div>
      <a href="/settings" class="manage-link" onclick={dismiss}>
        <span class="material-icons">open_in_new</span>
        Manage
      </a>
    </div>

    <!-- What this means -->
    <ul class="what-list">
      <li>
        <span class="material-icons">download_for_offline</span>New downloads
        cannot be resolved
      </li>
      <li>
        <span class="material-icons">lock</span>Fshare VIP subscription required
        for access
      </li>
      <li>
        <span class="material-icons">swap_horiz</span>Switch to a VIP account to
        resume
      </li>
    </ul>

    <!-- Actions -->
    {#if !showSwitchForm}
      <div class="modal-actions">
        <Button
          icon="swap_horiz"
          size="md"
          onclick={() => (showSwitchForm = true)}>Switch Account</Button
        >
        <Button variant="ghost" size="md" onclick={dismiss}
          >Dismiss for now</Button
        >
      </div>
    {:else}
      <div class="switch-form">
        <p class="form-title">
          <span class="material-icons">manage_accounts</span>
          Switch Account
        </p>

        <div class="field-group">
          <label for="warn-email">EMAIL</label>
          <div class="input-box">
            <span class="material-icons">email</span>
            <input
              id="warn-email"
              type="email"
              bind:value={newEmail}
              placeholder="fshare@email.com"
              disabled={switching}
              autocomplete="username"
            />
          </div>
        </div>

        <div class="field-group">
          <label for="warn-pass">PASSWORD</label>
          <div class="input-box">
            <span class="material-icons">lock</span>
            <input
              id="warn-pass"
              type={showPassword ? "text" : "password"}
              bind:value={newPassword}
              placeholder="••••••••"
              disabled={switching}
              autocomplete="current-password"
            />
            <button
              class="vis-toggle"
              type="button"
              onclick={() => (showPassword = !showPassword)}
            >
              <span class="material-icons"
                >{showPassword ? "visibility_off" : "visibility"}</span
              >
            </button>
          </div>
        </div>

        <div class="form-actions">
          <Button
            variant="ghost"
            size="md"
            onclick={() => (showSwitchForm = false)}
            disabled={switching}>Cancel</Button
          >
          <Button
            size="md"
            icon={switching ? "sync" : "swap_horiz"}
            loading={switching}
            onclick={handleSwitch}
            disabled={switching || !newEmail || !newPassword}
            >{switching ? "Switching…" : "Confirm Switch"}</Button
          >
        </div>
      </div>
    {/if}
  {/snippet}
</Modal>

<style>
  .header-icon {
    width: 44px;
    height: 44px;
    border-radius: 10px;
    background: #ffaa001f;
    border: 1px solid rgba(255,170,0,.25);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .header-icon .material-icons {
    font-size: 1.4rem;
    color: #fa0;
  }

  .header-text {
    flex: 1;
  }

  .header-text h2 {
    margin: 0 0 .2rem;
    font-size: 1.05rem;
    font-weight: 800;
    color: #fff;
    letter-spacing: .02em;
  }

  .header-text p {
    margin: 0;
    font-size: .78rem;
    color: #ffffff73;
    line-height: 1.4;
  }

  .close-btn {
    background: transparent;
    border: none;
    cursor: pointer;
    padding: .25rem;
    color: #ffffff4d;
    transition: color .2s;
    flex-shrink: 0;
  }

  .close-btn:hover {
    color: #fff;
  }

  .close-btn .material-icons {
    font-size: 1.2rem;
  }

  .account-status-row {
    display: flex;
    align-items: center;
    gap: .75rem;
    background: #ffaa000d;
    border: 1px solid rgba(255,170,0,.15);
    border-radius: 10px;
    padding: .75rem 1rem;
    margin-bottom: 1rem;
  }

  .acc-icon {
    font-size: 2rem;
    color: #ffffff4d;
    flex-shrink: 0;
  }

  .acc-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: .2rem;
  }

  .acc-email {
    font-size: .85rem;
    font-weight: 600;
    color: #fff;
    font-family: var(--font-mono, monospace);
  }

  .manage-link {
    display: flex;
    align-items: center;
    gap: .3rem;
    font-size: .72rem;
    font-weight: 700;
    color: #00f3ffb3;
    text-decoration: none;
    letter-spacing: .05em;
    transition: color .2s;
    flex-shrink: 0;
  }

  .manage-link:hover {
    color: #00f3ff;
  }

  .manage-link .material-icons {
    font-size: .9rem;
  }

  .what-list {
    list-style: none;
    padding: 0;
    margin: 0 0 1.25rem;
    display: flex;
    flex-direction: column;
    gap: .5rem;
  }

  .what-list li {
    display: flex;
    align-items: center;
    gap: .6rem;
    font-size: .78rem;
    color: #ffffff80;
  }

  .what-list .material-icons {
    font-size: 1rem;
    color: #fa09;
    flex-shrink: 0;
  }

  .modal-actions {
    display: flex;
    gap: .75rem;
  }

  .switch-form {
    border-top: 1px solid rgba(255,255,255,.07);
    padding-top: 1.1rem;
  }

  .form-title {
    display: flex;
    align-items: center;
    gap: .5rem;
    font-size: .82rem;
    font-weight: 700;
    letter-spacing: .06em;
    text-transform: uppercase;
    color: #ffffffa6;
    margin: 0 0 1rem;
  }

  .form-title .material-icons {
    font-size: 1rem;
    color: var(--color-primary, #00f3ff);
  }

  .field-group {
    margin-bottom: .75rem;
  }

  .field-group label {
    display: block;
    font-size: .68rem;
    font-weight: 700;
    letter-spacing: .1em;
    color: #ffffff59;
    margin-bottom: .35rem;
    font-family: var(--font-mono, monospace);
  }

  .input-box {
    display: flex;
    align-items: center;
    gap: .6rem;
    background: #ffffff0a;
    border: 1px solid rgba(255,255,255,.1);
    border-radius: 8px;
    padding: .6rem .85rem;
    transition: border-color .2s;
  }

  .input-box:focus-within {
    border-color: #00f3ff66;
    background: #00f3ff08;
  }

  .input-box .material-icons {
    font-size: 1rem;
    color: #ffffff40;
    flex-shrink: 0;
  }

  .input-box input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: #fff;
    font-size: .85rem;
    font-family: var(--font-mono, monospace);
  }

  .input-box input::placeholder {
    color: #fff3;
  }

  .input-box input:disabled {
    opacity: .5;
  }

  .vis-toggle {
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 0;
    color: #ffffff40;
    transition: color .2s;
    flex-shrink: 0;
  }

  .vis-toggle:hover {
    color: #fff9;
  }

  .vis-toggle .material-icons {
    font-size: 1rem;
  }

  .form-actions {
    display: flex;
    gap: .75rem;
    margin-top: 1rem;
  }

</style>
