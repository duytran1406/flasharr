<script lang="ts">
  import { animeFly } from "$lib/animations";
  import { setupStore } from "$lib/stores/setup.svelte";

  interface Props {
    downloadPathError: string;
    onBack?: () => void;
    onNext?: () => void;
  }

  let { downloadPathError, onBack, onNext }: Props = $props();
</script>

<div class="panel-content" in:animeFly={{ y: 20, duration: 400 }}>
  <div class="section-header">
    <div class="section-icon"><span class="material-icons">folder_open</span></div>
    <div>
      <h2>Download Configuration</h2>
      <p>Configure storage and concurrency settings</p>
    </div>
  </div>

  <div class="form-section">
    <div class="input-group">
      <label for="path">DOWNLOAD DIRECTORY</label>
      <div class="input-with-icon">
        <span class="material-icons">folder</span>
        <input type="text" id="path" class="input-field" class:error={downloadPathError} bind:value={setupStore.data.downloadPath} placeholder="/downloads" />
      </div>
      {#if downloadPathError}
        <span class="input-error">{downloadPathError}</span>
      {/if}
      <span class="input-hint">Absolute path where files will be saved</span>
    </div>

    <div class="input-group">
      <label for="concurrent">MAX CONCURRENT DOWNLOADS <span class="value-pill">{setupStore.data.maxConcurrent}</span></label>
      <input type="range" id="concurrent" min="1" max="10" bind:value={setupStore.data.maxConcurrent} class="range-slider" />
      <span class="input-hint">Recommended: 3-5 concurrent downloads</span>
    </div>
  </div>

  <div class="action-bar">
    <button class="btn-secondary" onclick={onBack}><span class="material-icons">arrow_back</span>BACK</button>
    <button class="btn-primary" onclick={onNext}>CONTINUE<span class="material-icons">arrow_forward</span></button>
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

  .value-pill {
    display: inline-block;
    background: var(--color-primary);
    color: #000;
    padding: .25rem .75rem;
    border-radius: 12px;
    font-size: .7rem;
    font-weight: 700;
    margin-left: .5rem;
  }

  .input-field {
    width: 100%;
    background: #0006;
    border: 2px solid rgba(255,255,255,.1);
    border-radius: 10px;
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

  .input-with-icon {
    position: relative;
    display: flex;
    align-items: center;
  }

  .input-with-icon .material-icons {
    position: absolute;
    left: 1rem;
    color: var(--text-muted);
    pointer-events: none;
  }

  .input-with-icon .input-field {
    padding-left: 3rem;
  }

  .input-error {
    font-size: .8rem;
    color: #ff5252;
  }

  .input-hint {
    font-size: .75rem;
    color: var(--text-muted);
  }

  .range-slider {
    width: 100%;
    height: 6px;
    background: #ffffff1a;
    border-radius: 3px;
    outline: none;
    appearance: none;
    -webkit-appearance: none;
  }

  .range-slider::-webkit-slider-thumb {
    appearance: none;
    width: 20px;
    height: 20px;
    background: var(--color-primary);
    border-radius: 50%;
    cursor: pointer;
    box-shadow: 0 0 10px #00f3ff80;
  }

  .range-slider::-moz-range-thumb {
    width: 20px;
    height: 20px;
    background: var(--color-primary);
    border-radius: 50%;
    cursor: pointer;
    border: none;
    box-shadow: 0 0 10px #00f3ff80;
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
    border-radius: 10px;
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
