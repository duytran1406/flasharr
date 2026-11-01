<script lang="ts">
  import { animeFly } from "$lib/animations";
  import { setupStore } from "$lib/stores/setup.svelte";

  interface Props {
    isLoading: boolean;
    emailError: string;
    passwordError: string;
    validationError: string;
    onBack?: () => void;
    onNext?: () => void;
  }

  let { isLoading, emailError, passwordError, validationError, onBack, onNext }: Props = $props();
</script>

<div class="panel-content" in:animeFly={{ y: 20, duration: 400 }}>
  <div class="section-header">
    <div class="fshare-logo-container">
      <div class="fshare-logo-ring"><div class="fshare-logo-inner"><div class="fshare-logo-bg"><img src="/images/flasharr_logo.png" alt="Flasharr" /></div></div></div>
    </div>
    <div>
      <h2>FShare Authentication</h2>
      <p>Connect your FShare account to enable downloads</p>
    </div>
  </div>

  <div class="form-section">
    <div class="input-group">
      <label for="setup-fshare-email">EMAIL ADDRESS</label>
      <input
        type="email"
        id="setup-fshare-email"
        name="email"
        autocomplete="email"
        class="input-field"
        class:error={emailError}
        bind:value={setupStore.data.fshareEmail}
        placeholder="your.email@example.com"
        disabled={isLoading}
        aria-invalid={!!emailError}
        aria-describedby={emailError ? "setup-fshare-email-error" : undefined}
      />
      {#if emailError}
        <span id="setup-fshare-email-error" class="input-error" role="alert">{emailError}</span>
      {/if}
    </div>

    <div class="input-group">
      <label for="setup-fshare-password">PASSWORD</label>
      <input
        type="password"
        id="setup-fshare-password"
        name="password"
        autocomplete="current-password"
        class="input-field"
        class:error={passwordError}
        bind:value={setupStore.data.fsharePassword}
        placeholder="Enter your password"
        disabled={isLoading}
        aria-invalid={!!passwordError}
        aria-describedby={passwordError ? "setup-fshare-password-error" : undefined}
      />
      {#if passwordError}
        <span id="setup-fshare-password-error" class="input-error" role="alert">{passwordError}</span>
      {/if}
    </div>

    {#if validationError}
      <div class="alert-box error" role="alert" transition:animeFly={{ y: -10, duration: 300 }}>
        <span class="material-icons">error_outline</span>
        <div><strong>Authentication Failed</strong><p>{validationError}</p></div>
      </div>
    {/if}

    <div class="info-box">
      <span class="material-icons">info</span>
      <span>Don't have an account? <a href="https://fshare.vn/reg.php" target="_blank" rel="noopener noreferrer">Register for FShare</a></span>
    </div>
  </div>

  <div class="action-bar">
    <button class="btn-secondary" onclick={onBack} disabled={isLoading}><span class="material-icons">arrow_back</span>BACK</button>
    <button class="btn-primary" onclick={onNext} disabled={isLoading}>{isLoading ? "VALIDATING..." : "CONTINUE"} <span class="material-icons">arrow_forward</span></button>
  </div>
</div>

<style>
  .panel-content {
    padding: 3rem;
    min-height: 100%;
    display: flex;
    flex-direction: column;
  }

  .section-header {
    display: flex;
    align-items: center;
    gap: 1.5rem;
    margin-bottom: 2rem;
    padding-bottom: 1.5rem;
    border-bottom: 1px solid rgba(255,255,255,.05);
  }

  .section-header h2 {
    font-size: 1.5rem;
    font-weight: 800;
    margin: 0 0 .25rem;
    color: #fff;
  }

  .section-header p {
    font-size: .9rem;
    color: var(--text-secondary);
    margin: 0;
  }

  .fshare-logo-container {
    width: 64px;
    height: 64px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .fshare-logo-ring {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(135deg,#00f3ff,#4facfe);
    box-shadow: 0 0 20px #00f3ff4d;
    padding: 2px;
    transition: all .3s ease;
    animation: pulse-fshare 2s infinite;
  }

  .fshare-logo-inner {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    background: #0f172a;
    padding: 2px;
    overflow: hidden;
  }

  .fshare-logo-bg {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #1e293b;
  }

  .fshare-logo-bg img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 50%;
  }

  .fshare-logo-container:hover .fshare-logo-ring {
    transform: scale(1.05);
  }

  @keyframes pulse-fshare {
    0%,to {
      box-shadow: 0 0 20px #00f3ff4d;
    }
    50% {
      box-shadow: 0 0 30px #00f3ff80;
    }
  }

  .form-section {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    margin-bottom: 2rem;
    flex: 1;
  }

  .input-group {
    display: flex;
    flex-direction: column;
    gap: .5rem;
  }

  .input-group label {
    font-family: var(--font-mono);
    font-size: .7rem;
    font-weight: 700;
    color: var(--text-secondary);
    letter-spacing: .1em;
  }

  .input-field {
    width: 100%;
    background: #0006;
    border: 2px solid rgba(255,255,255,.1);
    border-radius: var(--cui-radius-md);
    padding: 1rem 1.25rem;
    color: #fff;
    font-size: .95rem;
    transition: all .2s ease;
    font-family: var(--font-body);
  }

  .input-field:focus {
    outline: none;
    border-color: var(--color-primary);
    box-shadow: 0 0 0 3px #00f3ff1a;
    background: #0009;
  }

  .input-field.error {
    border-color: #ff5252;
    box-shadow: 0 0 0 3px #ff52521a;
  }

  .input-field:disabled {
    opacity: .5;
    cursor: not-allowed;
  }

  .input-error {
    font-size: .8rem;
    color: #ff5252;
  }

  .alert-box {
    display: flex;
    align-items: flex-start;
    gap: 1rem;
    padding: 1rem 1.25rem;
    border-radius: var(--cui-radius-md);
  }

  .alert-box.error {
    background: #ff52521a;
    border: 1px solid rgba(255,82,82,.3);
  }

  .alert-box .material-icons {
    color: #ff5252;
    font-size: 1.5rem;
    flex-shrink: 0;
  }

  .alert-box strong {
    display: block;
    color: #ff5252;
    font-size: .9rem;
    margin-bottom: .25rem;
  }

  .alert-box p {
    color: var(--text-secondary);
    font-size: .85rem;
    margin: 0;
  }

  .info-box {
    display: flex;
    align-items: center;
    gap: .75rem;
    padding: .875rem 1.25rem;
    background: #00f3ff0d;
    border: 1px solid rgba(0,243,255,.1);
    border-radius: var(--cui-radius-md);
    font-size: .85rem;
    color: var(--text-secondary);
  }

  .info-box .material-icons {
    color: var(--color-primary);
    font-size: 1.25rem;
  }

  .info-box a {
    color: var(--color-primary);
    text-decoration: none;
    font-weight: 600;
    transition: all .2s;
  }

  .info-box a:hover {
    text-decoration: underline;
    color: #fff;
  }

  .action-bar {
    display: flex;
    gap: 1rem;
    justify-content: flex-end;
    padding-top: 1.5rem;
    border-top: 1px solid rgba(255,255,255,.05);
    margin-top: auto;
  }

  .btn-primary,
  .btn-secondary {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: .5rem;
    padding: .875rem 1.75rem;
    font-weight: 900;
    font-size: .7rem;
    letter-spacing: .1em;
    border-radius: var(--cui-radius-md);
    cursor: pointer;
    transition: all .2s;
    font-family: var(--font-mono);
    border: none;
  }

  .btn-primary {
    background: linear-gradient(135deg,#00f3ff33,#00f3ff1a);
    color: var(--color-primary);
    border: 2px solid var(--color-primary);
    box-shadow: 0 0 20px #00f3ff4d;
  }

  .btn-primary:hover {
    background: linear-gradient(135deg,#00ffa333,#00ffa31a);
    border-color: var(--color-secondary);
    color: var(--color-secondary);
    box-shadow: 0 0 30px #00ffa366;
    transform: translateY(-2px);
  }

  .btn-primary:disabled {
    opacity: .5;
    cursor: not-allowed;
  }

  .btn-secondary {
    background: #ffffff0d;
    color: var(--text-secondary);
    border: 1px solid rgba(255,255,255,.1);
  }

  .btn-secondary:hover {
    background: #ffffff1a;
    color: #fff;
    border-color: #fff3;
  }

  @media (max-width:768px) {
    .panel-content {
      padding: 2rem 1.5rem;
    }
    .action-bar {
      flex-direction: column;
    }
    .btn-primary,
    .btn-secondary {
      width: 100%;
    }
  }

</style>
