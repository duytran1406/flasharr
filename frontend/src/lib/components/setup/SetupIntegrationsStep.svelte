<script lang="ts">
  import { animeFly } from "$lib/animations";
  import { setupStore } from "$lib/stores/setup.svelte";

  interface Props {
    isLoading: boolean;
    onBack?: () => void;
    onSkip?: () => void;
    onNext?: () => void;
  }

  let { isLoading, onBack, onSkip, onNext }: Props = $props();
</script>

<div class="panel-content" in:animeFly={{ y: 20, duration: 400 }}>
  <div class="section-header">
    <div class="section-icon"><span class="material-icons">extension</span></div>
    <div>
      <h2>Optional Integrations</h2>
      <p>Connect to Sonarr, Radarr, or Jellyfin (skip if not needed)</p>
    </div>
  </div>

  <div class="form-section">
    <div class="integration-box">
      <div class="integration-header">
        <div class="integration-info">
          <div class="integration-logo-container"><img src="https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/sonarr.png" alt="Sonarr" class="integration-logo" /></div>
          <span>Sonarr</span>
        </div>
        <label class="switch"><span class="sr-only">Enable Sonarr</span><input type="checkbox" bind:checked={setupStore.data.sonarrEnabled} aria-label="Enable Sonarr" /><span class="slider"></span></label>
      </div>
      {#if setupStore.data.sonarrEnabled}
        <div class="integration-fields" transition:animeFly={{ y: -10, duration: 300 }}><label for="setup-sonarr-url">Sonarr URL</label><input id="setup-sonarr-url" name="sonarr-url" type="url" inputmode="url" autocomplete="url" class="input-field-sm" bind:value={setupStore.data.sonarrUrl} placeholder="http://localhost:8989" />
          <label for="setup-sonarr-api-key">Sonarr API key</label><input id="setup-sonarr-api-key" name="sonarr-api-key" type="password" autocomplete="off" spellcheck={false} class="input-field-sm" bind:value={setupStore.data.sonarrApiKey} placeholder="API Key" />
          <button class="btn-test" onclick={() => setupStore.testSonarr()} disabled={isLoading} aria-label="Test Sonarr connection">{setupStore.sonarrValidated ? "✓ CONNECTED" : "TEST CONNECTION"}</button></div>
      {/if}
    </div>
    <div class="integration-box">
      <div class="integration-header">
        <div class="integration-info">
          <div class="integration-logo-container"><img src="https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/radarr.png" alt="Radarr" class="integration-logo" /></div>
          <span>Radarr</span>
        </div>
        <label class="switch"><span class="sr-only">Enable Radarr</span><input type="checkbox" bind:checked={setupStore.data.radarrEnabled} aria-label="Enable Radarr" /><span class="slider"></span></label>
      </div>
      {#if setupStore.data.radarrEnabled}
        <div class="integration-fields" transition:animeFly={{ y: -10, duration: 300 }}><label for="setup-radarr-url">Radarr URL</label><input id="setup-radarr-url" name="radarr-url" type="url" inputmode="url" autocomplete="url" class="input-field-sm" bind:value={setupStore.data.radarrUrl} placeholder="http://localhost:7878" />
          <label for="setup-radarr-api-key">Radarr API key</label><input id="setup-radarr-api-key" name="radarr-api-key" type="password" autocomplete="off" spellcheck={false} class="input-field-sm" bind:value={setupStore.data.radarrApiKey} placeholder="API Key" />
          <button class="btn-test" onclick={() => setupStore.testRadarr()} disabled={isLoading} aria-label="Test Radarr connection">{setupStore.radarrValidated ? "✓ CONNECTED" : "TEST CONNECTION"}</button></div>
      {/if}
    </div>
  </div>

  <div class="action-bar">
    <button class="btn-secondary" onclick={onBack}><span class="material-icons">arrow_back</span>BACK</button>
    <button class="btn-secondary" onclick={onSkip}>SKIP</button>
    <button class="btn-primary" onclick={onNext} disabled={isLoading}>CONTINUE<span class="material-icons">arrow_forward</span></button>
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

  .section-icon {
    width: 64px;
    height: 64px;
    background: linear-gradient(145deg,#00f3ff26,#00f3ff0d);
    border-radius: var(--cui-radius-md);
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid rgba(0,243,255,.3);
    flex-shrink: 0;
    box-shadow: 0 8px 20px #00f3ff1a,inset 0 1px #ffffff1a;
    backdrop-filter: blur(10px);
  }

  .section-icon .material-icons {
    font-size: 2rem;
    color: var(--color-primary);
    filter: drop-shadow(0 0 10px rgba(0,243,255,.5));
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

  .form-section {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    margin-bottom: 2rem;
    flex: 1;
  }

  .integration-box {
    background: #ffffff05;
    border: 1px solid rgba(255,255,255,.05);
    border-radius: var(--cui-radius-md);
    padding: 1.25rem;
    transition: all .2s;
  }

  .integration-box:hover {
    border-color: #00f3ff33;
  }

  .integration-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .integration-info {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .integration-logo-container {
    width: auto;
    height: auto;
    display: inline-block;
    position: relative;
  }

  .integration-logo-container:before {
    display: none;
  }

  .integration-logo {
    width: 48px;
    height: 48px;
    object-fit: contain;
    padding: 6px;
    border-radius: var(--cui-radius-md);
    position: relative;
    z-index: 1;
    filter: drop-shadow(0 0 1px rgba(255,255,255,.5)) drop-shadow(0 0 8px rgba(0,243,255,.3));
  }

  .integration-info span {
    font-size: 1rem;
    font-weight: 600;
    color: #fff;
  }

  .switch {
    position: relative;
    display: inline-block;
    width: 52px;
    height: 28px;
  }

  .switch input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .slider {
    position: absolute;
    cursor: pointer;
    inset: 0;
    background: #ffffff1a;
    transition: .3s;
    border-radius: 28px;
    border: 1px solid rgba(255,255,255,.1);
  }

  .slider:before {
    position: absolute;
    content: "";
    height: 20px;
    width: 20px;
    left: 4px;
    bottom: 3px;
    background: #fff;
    transition: .3s;
    border-radius: 50%;
  }

  input:checked + .slider {
    background: #00ffa333;
    border-color: var(--color-secondary);
    box-shadow: 0 0 10px #00ffa34d;
  }

  input:checked + .slider:before {
    transform: translate(24px);
    background: var(--color-secondary);
  }

  .integration-fields {
    display: flex;
    flex-direction: column;
    gap: .75rem;
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid rgba(255,255,255,.05);
  }

  .input-field-sm {
    width: 100%;
    background: #0000004d;
    border: 1px solid rgba(255,255,255,.1);
    border-radius: var(--cui-radius-md);
    padding: .75rem 1rem;
    color: #fff;
    font-size: .9rem;
    transition: all .2s;
  }

  .input-field-sm:focus {
    outline: none;
    border-color: var(--color-primary);
    box-shadow: 0 0 0 2px #00f3ff1a;
  }

  .btn-test {
    background: #00f3ff1a;
    border: 1px solid rgba(0,243,255,.3);
    color: var(--color-primary);
    padding: .75rem 1rem;
    border-radius: var(--cui-radius-md);
    font-weight: 700;
    font-size: .75rem;
    letter-spacing: .05em;
    cursor: pointer;
    transition: all .2s;
    font-family: var(--font-mono);
  }

  .btn-test:hover {
    background: #00f3ff33;
    border-color: var(--color-primary);
    box-shadow: 0 0 15px #00f3ff33;
  }

  .btn-test:disabled {
    opacity: .5;
    cursor: not-allowed;
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

  .integration-fields > label {
    color: var(--text-secondary);
    font: 700 .7rem var(--font-mono);
    letter-spacing: .06em;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0,0,0,0);
    white-space: nowrap;
    border: 0;
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
