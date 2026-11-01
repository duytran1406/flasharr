<script lang="ts">
  import { Button } from "@media-set/core-ui";
  import { authStore } from "$lib/stores/auth.svelte";

  /**
   * AuthGate — full-screen admin sign-in form shown by the root layout while
   * the session is unauthenticated. When no administrator exists yet it
   * creates one instead (POST /api/auth/setup).
   */

  let username = $state("");
  let password = $state("");
  let submitting = $state(false);
  let error = $state("");

  const needsSetup = $derived(
    authStore.session && !authStore.session.configured,
  );

  async function submit() {
    error = "";
    submitting = true;
    try {
      await authStore.submit(
        needsSetup ? "setup" : "login",
        username,
        password,
      );
    } catch (err) {
      error = err instanceof Error ? err.message : "Authentication failed";
    } finally {
      submitting = false;
    }
  }
</script>

<svelte:head>
  <title>{needsSetup ? "Create administrator" : "Sign in"} – Flasharr</title>
</svelte:head>

<div class="gate" role="main">
  <form
    class="panel"
    onsubmit={(event) => {
      event.preventDefault();
      submit();
    }}
  >
    <span class="material-icons logo">downloading</span>
    <p class="eyebrow">Flasharr</p>
    <h1>{needsSetup ? "Create administrator" : "Sign in"}</h1>
    <p class="copy">
      {needsSetup
        ? "Create the Flasharr administrator account."
        : "Use your Flasharr administrator account."}
    </p>
    <label>
      <span>Username</span><input
        name="username"
        bind:value={username}
        autocomplete="username"
        minlength="3"
        maxlength="64"
        required=""
      />
    </label>
    <label>
      <span>Password</span><input
        name="password"
        type="password"
        bind:value={password}
        autocomplete={needsSetup ? "new-password" : "current-password"}
        minlength="12"
        maxlength="256"
        required=""
      />
    </label>
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
    <Button variant="primary" type="submit" loading={submitting}>
      {needsSetup ? "Create administrator" : "Sign in"}
    </Button>
  </form>
</div>

<style>
  .gate {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 1rem;
    background: radial-gradient(circle at top,rgba(0,243,255,.14),transparent 45%),var(--cui-surface-0,#071018);
  }

  .panel {
    width: min(100%,400px);
    display: flex;
    flex-direction: column;
    gap: .85rem;
    padding: 2rem;
    border: 1px solid var(--cui-glass-border);
    border-radius: var(--cui-radius-lg);
    background: var(--cui-glass-heavy);
    box-shadow: var(--cui-shadow-glass);
  }

  .logo {
    font-size: 2rem;
    color: var(--cui-primary);
  }

  .eyebrow {
    margin: 0;
    color: var(--cui-text-secondary);
    font-size: .7rem;
    letter-spacing: .16em;
    text-transform: uppercase;
  }

  .panel h1 {
    margin: 0;
    font-size: 1.35rem;
    color: var(--cui-text-primary);
  }

  .copy {
    margin: 0 0 .5rem;
    color: var(--cui-text-secondary);
    font-size: .86rem;
    line-height: 1.5;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: .35rem;
    color: var(--cui-text-secondary);
    font-size: .72rem;
    letter-spacing: .07em;
    text-transform: uppercase;
  }

  input {
    min-height: var(--cui-touch-min);
    border: 1px solid var(--cui-input-border);
    border-radius: var(--cui-radius-sm);
    background: var(--cui-input-bg);
    color: var(--cui-text-primary);
    padding: .6rem .75rem;
    font: inherit;
    text-transform: none;
    letter-spacing: normal;
  }

  .error {
    margin: 0;
    color: var(--cui-danger);
    font-size: .8rem;
  }

</style>
