<script lang="ts">
  import { toasts } from "$lib/stores/toasts";
  import Modal from "$lib/components/ui/Modal.svelte";
  import Button from "$lib/components/ui/Button.svelte";

  interface Props {
    open: boolean;
    onclose: () => void;
    onswitched?: () => void;
  }

  let { open = $bindable(), onclose, onswitched }: Props = $props();

  let email = $state("");
  let password = $state("");
  let showPassword = $state(false);
  let loading = $state(false);

  function reset() {
    email = "";
    password = "";
    showPassword = false;
    loading = false;
  }

  function close() {
    reset();
    onclose();
  }

  async function submit() {
    if (!email.trim() || !password.trim()) {
      toasts.error("Please enter both email and password");
      return;
    }
    loading = true;
    try {
      const res = await fetch("/api/accounts", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email: email.trim(), password }),
      });
      const data = await res.json().catch(() => ({}));
      if (res.ok && data.success !== false) {
        toasts.success("Account switched successfully");
        onswitched?.();
        close();
      } else {
        toasts.error(
          data?.message || data?.error || "Failed to switch account",
        );
      }
    } catch (e: any) {
      toasts.error(e?.message || "Network error");
    } finally {
      loading = false;
    }
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === "Escape") close();
    if (e.key === "Enter" && !loading) submit();
  }
</script>

<Modal
  {open}
  onClose={close}
  maxWidth="420px"
  accent="var(--color-primary, #00f3ff)"
  ariaLabel="Switch Account"
>
  {#snippet header()}
    <div class="modal-title-row">
      <span class="material-icons modal-icon">manage_accounts</span>
      <div>
        <p id="switch-modal-title" class="modal-title">SWITCH ACCOUNT</p>
        <p class="modal-sub">Login with a different Fshare account</p>
      </div>
    </div>
    <button class="close-btn" onclick={close} aria-label="Close">
      <span class="material-icons">close</span>
    </button>
  {/snippet}

  {#snippet children()}
    <div class="field-group">
      <label class="field-label" for="sw-email">EMAIL</label>
      <div class="input-row">
        <span class="material-icons input-icon">email</span>
        <input
          id="sw-email"
          type="email"
          bind:value={email}
          placeholder="your@email.com"
          disabled={loading}
          autocomplete="username"
        />
      </div>
    </div>

    <div class="field-group">
      <label class="field-label" for="sw-password">PASSWORD</label>
      <div class="input-row">
        <span class="material-icons input-icon">lock</span>
        <input
          id="sw-password"
          type={showPassword ? "text" : "password"}
          bind:value={password}
          placeholder="••••••••••••"
          disabled={loading}
          autocomplete="current-password"
        />
        <button
          class="vis-toggle"
          type="button"
          onclick={() => (showPassword = !showPassword)}
          tabindex="-1"
        >
          <span class="material-icons"
            >{showPassword ? "visibility_off" : "visibility"}</span
          >
        </button>
      </div>
    </div>

    <Button
      size="lg"
      icon={loading ? "sync" : "swap_horiz"}
      {loading}
      onclick={submit}
      disabled={loading || !email || !password}
      width="100%">{loading ? "Switching…" : "Switch Account"}</Button
    >
  {/snippet}
</Modal>

<style>
  /* Header title/sub text */
  .modal-title-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }
  .modal-icon {
    font-size: 1.6rem;
    color: var(--color-primary, #00f3ff);
  }
  .modal-title {
    font-family: var(--font-mono, monospace);
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    color: #fff;
    margin: 0;
  }
  .modal-sub {
    font-size: 0.68rem;
    color: rgba(255, 255, 255, 0.35);
    margin: 0.1rem 0 0;
  }
  .close-btn {
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.3);
    cursor: pointer;
    padding: 4px;
    border-radius: 8px;
    display: flex;
    transition:
      color 0.2s,
      background 0.2s;
  }
  .close-btn:hover {
    color: #fff;
    background: rgba(255, 255, 255, 0.06);
  }

  .field-group {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  .field-label {
    font-family: var(--font-mono, monospace);
    font-size: 0.58rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    color: rgba(255, 255, 255, 0.35);
  }

  .input-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    padding: 0 0.6rem;
    transition: border-color 0.2s;
  }
  .input-row:focus-within {
    border-color: rgba(220, 60, 60, 0.5);
    box-shadow: 0 0 0 2px rgba(220, 60, 60, 0.1);
  }
  .input-icon {
    font-size: 1rem;
    color: rgba(255, 255, 255, 0.25);
    flex-shrink: 0;
  }

  .input-row input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: rgba(255, 255, 255, 0.9);
    font-size: 0.82rem;
    padding: 0.62rem 0;
    font-family: inherit;
  }
  .input-row input::placeholder {
    color: rgba(255, 255, 255, 0.2);
  }
  .input-row input:disabled {
    opacity: 0.5;
  }

  .vis-toggle {
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.25);
    cursor: pointer;
    padding: 4px;
    display: flex;
    transition: color 0.2s;
    flex-shrink: 0;
  }
  .vis-toggle:hover {
    color: rgba(255, 255, 255, 0.6);
  }
  .vis-toggle .material-icons {
    font-size: 1rem;
  }

  .submit-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    padding: 0.7rem 1rem;
    background: rgba(220, 60, 60, 0.12);
    border: 1px solid rgba(220, 60, 60, 0.3);
    border-radius: 10px;
    color: rgba(220, 60, 60, 0.9);
    font-family: var(--font-mono, monospace);
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    cursor: pointer;
    transition: all 0.2s;
    margin-top: 0.2rem;
  }
  .submit-btn:hover:not(:disabled) {
    background: rgba(220, 60, 60, 0.2);
    border-color: rgba(220, 60, 60, 0.55);
    color: #ff6b6b;
    box-shadow: 0 0 16px rgba(220, 60, 60, 0.15);
  }
  .submit-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .submit-btn .material-icons {
    font-size: 1rem;
  }

  @keyframes rotating {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }
  .rotating {
    animation: rotating 0.8s linear infinite;
    display: inline-block;
  }
</style>
