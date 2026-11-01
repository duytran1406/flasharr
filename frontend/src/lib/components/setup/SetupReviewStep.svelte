<script lang="ts">
  import { animeFly } from "$lib/animations";
  import { setupStore } from "$lib/stores/setup.svelte";

  interface Props {
    isLoading: boolean;
    onBack?: () => void;
    onComplete?: () => void;
  }

  let { isLoading, onBack, onComplete }: Props = $props();
</script>

<div class="panel-content" in:animeFly={{ y: 20, duration: 400 }}>
  <div class="section-header">
    <div class="section-icon"><span class="material-icons">task_alt</span></div>
    <div>
      <h2>Configuration Review</h2>
      <p>Verify your settings before deployment</p>
    </div>
  </div>

  <div class="review-grid">
    <div class="review-section">
      <div class="review-label">FSHARE ACCOUNT</div>
      <div class="review-item"><span class="material-icons">email</span><span>{setupStore.data.fshareEmail}</span></div>
    </div>

    <div class="review-section">
      <div class="review-label">DOWNLOAD CONFIG</div>
      <div class="review-item"><span class="material-icons">folder</span><span>{setupStore.data.downloadPath}</span></div>
      <div class="review-item"><span class="material-icons">speed</span><span>{setupStore.data.maxConcurrent} concurrent downloads</span></div>
    </div>

    {#if setupStore.data.sonarrEnabled || setupStore.data.radarrEnabled || setupStore.data.jellyfinEnabled}
      <div class="review-section">
        <div class="review-label">INTEGRATIONS</div>
        {#if setupStore.data.sonarrEnabled}
          <div class="review-item"><span class="material-icons" class:validated={setupStore.sonarrValidated} class:not-validated={!setupStore.sonarrValidated}>{setupStore.sonarrValidated ? "check_circle" : "cancel"}</span><span>Sonarr: {setupStore.data.sonarrUrl}</span></div>
        {/if}
        {#if setupStore.data.radarrEnabled}
          <div class="review-item"><span class="material-icons" class:validated={setupStore.radarrValidated} class:not-validated={!setupStore.radarrValidated}>{setupStore.radarrValidated ? "check_circle" : "cancel"}</span><span>Radarr: {setupStore.data.radarrUrl}</span></div>
        {/if}
        {#if !setupStore.data.sonarrEnabled && !setupStore.data.radarrEnabled}
          <div class="review-item"><span class="material-icons" style="color: var(--text-muted);">info</span><span style="color: var(--text-muted);">No integrations configured</span></div>
        {/if}
      </div>
    {/if}
  </div>

  <div class="action-bar">
    <button class="btn-secondary" onclick={onBack} disabled={isLoading}><span class="material-icons">arrow_back</span>BACK</button>
    <button class="btn-primary-large" onclick={onComplete} disabled={isLoading}>{isLoading ? "DEPLOYING..." : "COMPLETE SETUP"} <span class="material-icons">rocket_launch</span></button>
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
    border-radius: 16px;
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

  .review-grid {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    margin-bottom: 2rem;
    flex: 1;
  }

  .review-section {
    background: #ffffff05;
    border: 1px solid rgba(255,255,255,.05);
    border-radius: 12px;
    padding: 1.5rem;
  }

  .review-label {
    font-family: var(--font-mono);
    font-size: .65rem;
    font-weight: 900;
    color: var(--color-primary);
    letter-spacing: .15em;
    margin-bottom: 1rem;
  }

  .review-item {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: .75rem 0;
    border-bottom: 1px solid rgba(255,255,255,.05);
  }

  .review-item:last-child {
    border-bottom: none;
  }

  .review-item .material-icons {
    font-size: 1.25rem;
  }

  .review-item .material-icons.validated {
    color: var(--color-secondary);
  }

  .review-item .material-icons.not-validated {
    color: #f44;
  }

  .review-item span:last-child {
    color: var(--text-primary);
    font-size: .95rem;
  }

  .action-bar {
    display: flex;
    gap: 1rem;
    justify-content: flex-end;
    padding-top: 1.5rem;
    border-top: 1px solid rgba(255,255,255,.05);
    margin-top: auto;
  }

  .btn-secondary,
  .btn-primary-large {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: .5rem;
    font-weight: 900;
    letter-spacing: .1em;
    border-radius: 10px;
    cursor: pointer;
    transition: all .2s;
    font-family: var(--font-mono);
    border: none;
  }

  .btn-secondary {
    padding: .875rem 1.75rem;
    font-size: .7rem;
    background: #ffffff0d;
    color: var(--text-secondary);
    border: 1px solid rgba(255,255,255,.1);
  }

  .btn-secondary:hover {
    background: #ffffff1a;
    color: #fff;
    border-color: #fff3;
  }

  .btn-primary-large {
    background: linear-gradient(135deg,#00f3ff33,#00f3ff1a);
    color: var(--color-primary);
    border: 2px solid var(--color-primary);
    box-shadow: 0 0 20px #00f3ff4d;
    padding: 1.125rem 2.5rem;
    font-size: .8rem;
  }

  .btn-primary-large:hover {
    background: linear-gradient(135deg,#00ffa333,#00ffa31a);
    border-color: var(--color-secondary);
    color: var(--color-secondary);
    box-shadow: 0 0 30px #00ffa366;
    transform: translateY(-2px);
  }

  .btn-primary-large:disabled,
  .btn-secondary:disabled {
    opacity: .5;
    cursor: not-allowed;
  }

  @media (max-width:768px) {
    .panel-content {
      padding: 2rem 1.5rem;
    }
    .action-bar {
      flex-direction: column;
    }
    .btn-secondary,
    .btn-primary-large {
      width: 100%;
    }
  }

</style>
