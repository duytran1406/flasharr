<script lang="ts">
  export interface SetupStep {
    id: number;
    title: string;
    label: string;
  }

  interface Props {
    steps: SetupStep[];
    currentStep: number;
  }

  let { steps, currentStep }: Props = $props();
</script>

<ol class="progress-tracker" aria-label="Setup progress">
  {#each steps as step, i}
    <li class="step-indicator" class:active={currentStep === i} class:complete={currentStep > i} aria-current={currentStep === i ? "step" : undefined}>
      <div class="step-dot">
        {#if currentStep > i}
          <span class="material-icons" aria-hidden="true">check</span>
        {:else}
          <span class="step-num">{i + 1}</span>
        {/if}
      </div>
      <div class="step-meta">
        <span class="step-label">{step.label}</span>
        <span class="step-title">{step.title}</span>
      </div>
      {#if i < steps.length - 1}
        <span class="step-connector" class:filled={currentStep > i} aria-hidden="true"></span>
      {/if}
    </li>
  {/each}
</ol>

<style>
  .progress-tracker {
    display: flex;
    align-items: center;
    gap: 1.5rem;
    margin-bottom: 3rem;
    justify-content: center;
  }

  .step-indicator {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: .75rem;
    text-align: center;
    position: relative;
    list-style: none;
  }

  .step-dot {
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: #ffffff08;
    border: 2px solid rgba(255,255,255,.1);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all .3s ease;
    position: relative;
    z-index: 2;
  }

  .step-indicator.active .step-dot {
    background: linear-gradient(135deg,#00f3ff33,#00f3ff1a);
    border-color: var(--color-primary);
    box-shadow: 0 0 30px #00f3ff66;
  }

  .step-indicator.complete .step-dot {
    background: #00ffa31a;
    border-color: var(--color-secondary);
  }

  .step-num {
    font-family: var(--font-mono);
    font-size: 1.1rem;
    font-weight: 700;
    color: var(--text-muted);
  }

  .step-indicator.active .step-num {
    color: var(--color-primary);
  }

  .step-dot .material-icons {
    font-size: 24px;
    color: var(--color-secondary);
  }

  .step-meta {
    display: flex;
    flex-direction: column;
    gap: .25rem;
  }

  .step-label {
    font-family: var(--font-mono);
    font-size: .6rem;
    font-weight: 700;
    color: var(--text-muted);
    letter-spacing: .15em;
  }

  .step-indicator.active .step-label {
    color: var(--color-primary);
  }

  .step-title {
    font-size: .85rem;
    font-weight: 600;
    color: var(--text-secondary);
  }

  .step-indicator.active .step-title {
    color: var(--text-primary);
  }

  .step-connector {
    width: 60px;
    height: 2px;
    background: #ffffff1a;
    margin: 0 -1rem;
    position: relative;
    z-index: 1;
    transition: all .3s ease;
  }

  .step-connector.filled {
    background: linear-gradient(90deg,var(--color-primary),var(--color-secondary));
  }

  @media (max-width:768px) {
    .progress-tracker {
      flex-wrap: wrap;
      gap: 1rem;
      margin-bottom: 2rem;
    }
    .step-connector {
      display: none;
    }
  }

</style>
