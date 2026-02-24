<script lang="ts">
  import { accountStore } from "$lib/stores/account.svelte";
  import { toasts } from "$lib/stores/toasts";
  import Badge from "$lib/components/ui/Badge.svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import Button from "$lib/components/ui/Button.svelte";

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
      const ok = await accountStore.switchAccount(
        newEmail.trim(),
        newPassword.trim(),
      );
      if (ok) {
        toasts.success("Account switched successfully!");
        newEmail = "";
        newPassword = "";
        showSwitchForm = false;
        onDismiss();
      } else {
        toasts.error("Failed to switch account — check credentials");
      }
    } catch (e) {
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
  /* ── Header icon in modal header slot ── */
  .header-icon {
    width: 44px;
    height: 44px;
    border-radius: 10px;
    background: rgba(255, 170, 0, 0.12);
    border: 1px solid rgba(255, 170, 0, 0.25);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .header-icon .material-icons {
    font-size: 1.4rem;
    color: #ffaa00;
  }

  .header-text {
    flex: 1;
  }

  .header-text h2 {
    margin: 0 0 0.2rem;
    font-size: 1.05rem;
    font-weight: 800;
    color: #fff;
    letter-spacing: 0.02em;
  }

  .header-text p {
    margin: 0;
    font-size: 0.78rem;
    color: rgba(255, 255, 255, 0.45);
    line-height: 1.4;
  }

  .close-btn {
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 0.25rem;
    color: rgba(255, 255, 255, 0.3);
    transition: color 0.2s;
    flex-shrink: 0;
  }
  .close-btn:hover {
    color: #fff;
  }
  .close-btn .material-icons {
    font-size: 1.2rem;
  }

  /* ── Account Status Row ── */
  .account-status-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    background: rgba(255, 170, 0, 0.05);
    border: 1px solid rgba(255, 170, 0, 0.15);
    border-radius: 10px;
    padding: 0.75rem 1rem;
    margin-bottom: 1rem;
  }

  .acc-icon {
    font-size: 2rem;
    color: rgba(255, 255, 255, 0.3);
    flex-shrink: 0;
  }

  .acc-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .acc-email {
    font-size: 0.85rem;
    font-weight: 600;
    color: #fff;
    font-family: var(--font-mono, monospace);
  }

  .manage-link {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.72rem;
    font-weight: 700;
    color: rgba(0, 243, 255, 0.7);
    text-decoration: none;
    letter-spacing: 0.05em;
    transition: color 0.2s;
    flex-shrink: 0;
  }
  .manage-link:hover {
    color: #00f3ff;
  }
  .manage-link .material-icons {
    font-size: 0.9rem;
  }

  /* ── What list ── */
  .what-list {
    list-style: none;
    padding: 0;
    margin: 0 0 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .what-list li {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    font-size: 0.78rem;
    color: rgba(255, 255, 255, 0.5);
  }

  .what-list .material-icons {
    font-size: 1rem;
    color: rgba(255, 170, 0, 0.6);
    flex-shrink: 0;
  }

  /* ── Actions ── */
  .modal-actions {
    display: flex;
    gap: 0.75rem;
  }

  .btn-switch {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding: 0.7rem 1rem;
    background: linear-gradient(
      135deg,
      rgba(0, 243, 255, 0.15),
      rgba(0, 150, 200, 0.1)
    );
    border: 1px solid rgba(0, 243, 255, 0.4);
    color: #00f3ff;
    font-size: 0.8rem;
    font-weight: 800;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    cursor: pointer;
    border-radius: 8px;
    transition: all 0.2s;
    font-family: var(--font-mono, monospace);
  }
  .btn-switch:hover {
    background: linear-gradient(
      135deg,
      rgba(0, 243, 255, 0.25),
      rgba(0, 150, 200, 0.2)
    );
    border-color: #00f3ff;
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.2);
  }
  .btn-switch .material-icons {
    font-size: 1rem;
  }

  .btn-dismiss {
    padding: 0.7rem 1.1rem;
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.4);
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
    border-radius: 8px;
    transition: all 0.2s;
  }
  .btn-dismiss:hover {
    border-color: rgba(255, 255, 255, 0.2);
    color: rgba(255, 255, 255, 0.7);
  }

  /* ── Switch Form ── */
  .switch-form {
    border-top: 1px solid rgba(255, 255, 255, 0.07);
    padding-top: 1.1rem;
  }

  .form-title {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.82rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.65);
    margin: 0 0 1rem;
  }
  .form-title .material-icons {
    font-size: 1rem;
    color: var(--color-primary, #00f3ff);
  }

  .field-group {
    margin-bottom: 0.75rem;
  }

  .field-group label {
    display: block;
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: rgba(255, 255, 255, 0.35);
    margin-bottom: 0.35rem;
    font-family: var(--font-mono, monospace);
  }

  .input-box {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 0.6rem 0.85rem;
    transition: border-color 0.2s;
  }
  .input-box:focus-within {
    border-color: rgba(0, 243, 255, 0.4);
    background: rgba(0, 243, 255, 0.03);
  }
  .input-box .material-icons {
    font-size: 1rem;
    color: rgba(255, 255, 255, 0.25);
    flex-shrink: 0;
  }
  .input-box input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: #fff;
    font-size: 0.85rem;
    font-family: var(--font-mono, monospace);
  }
  .input-box input::placeholder {
    color: rgba(255, 255, 255, 0.2);
  }
  .input-box input:disabled {
    opacity: 0.5;
  }

  .vis-toggle {
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 0;
    color: rgba(255, 255, 255, 0.25);
    transition: color 0.2s;
    flex-shrink: 0;
  }
  .vis-toggle:hover {
    color: rgba(255, 255, 255, 0.6);
  }
  .vis-toggle .material-icons {
    font-size: 1rem;
  }

  .form-actions {
    display: flex;
    gap: 0.75rem;
    margin-top: 1rem;
  }

  .btn-cancel {
    padding: 0.65rem 1rem;
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.4);
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
    border-radius: 8px;
    transition: all 0.2s;
  }
  .btn-cancel:hover {
    border-color: rgba(255, 255, 255, 0.2);
    color: rgba(255, 255, 255, 0.7);
  }
  .btn-cancel:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .btn-confirm {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding: 0.65rem 1rem;
    background: linear-gradient(
      135deg,
      rgba(0, 243, 255, 0.2),
      rgba(0, 200, 255, 0.1)
    );
    border: 1px solid rgba(0, 243, 255, 0.5);
    color: #00f3ff;
    font-size: 0.8rem;
    font-weight: 800;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    cursor: pointer;
    border-radius: 8px;
    transition: all 0.2s;
    font-family: var(--font-mono, monospace);
  }
  .btn-confirm:hover:not(:disabled) {
    background: linear-gradient(
      135deg,
      rgba(0, 243, 255, 0.3),
      rgba(0, 200, 255, 0.2)
    );
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.25);
  }
  .btn-confirm:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .btn-confirm .material-icons {
    font-size: 1rem;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .spin {
    display: inline-block;
    animation: spin 0.8s linear infinite;
  }
</style>
