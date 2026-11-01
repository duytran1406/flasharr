<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { ui } from "$lib/stores/ui.svelte";
  import { setupStore } from "$lib/stores/setup.svelte";
  import SetupProgressTracker, { type SetupStep } from "$lib/components/setup/SetupProgressTracker.svelte";
  import SetupWelcomeStep from "$lib/components/setup/SetupWelcomeStep.svelte";
  import SetupFshareStep from "$lib/components/setup/SetupFshareStep.svelte";
  import SetupDownloadsStep from "$lib/components/setup/SetupDownloadsStep.svelte";
  import SetupIntegrationsStep from "$lib/components/setup/SetupIntegrationsStep.svelte";
  import SetupReviewStep from "$lib/components/setup/SetupReviewStep.svelte";

  let currentStep = $derived(setupStore.currentStep);
  let isLoading = $derived(setupStore.isLoading);

  let emailError = $state("");
  let passwordError = $state("");
  let validationError = $state("");
  let downloadPathError = $state("");

  const steps: SetupStep[] = [
    { id: 0, title: "Welcome", label: "INIT" },
    { id: 1, title: "FShare", label: "AUTH" },
    { id: 2, title: "Downloads", label: "CONFIG" },
    { id: 3, title: "Integrations", label: "EXTEND" },
    { id: 4, title: "Review", label: "DEPLOY" },
  ];

  onMount(async () => {
    if (await setupStore.checkStatus()) {
      goto("/");
      return;
    }
    setTimeout(() => {
      ui.finishIntro();
    }, 2000);
  });

  function isValidEmail(email: string): boolean {
    return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);
  }

  async function handleFshareNext() {
    emailError = "";
    passwordError = "";
    validationError = "";
    if (!setupStore.data.fshareEmail) {
      emailError = "Email is required";
      return;
    }
    if (!isValidEmail(setupStore.data.fshareEmail)) {
      emailError = "Invalid email format";
      return;
    }
    if (!setupStore.data.fsharePassword) {
      passwordError = "Password is required";
      return;
    }
    if (await setupStore.validateFshare()) setupStore.nextStep();
    else validationError = "Invalid FShare credentials. Please check your email and password.";
  }

  function handleDownloadsNext() {
    downloadPathError = "";
    if (!setupStore.data.downloadPath) {
      downloadPathError = "Download path is required";
      return;
    }
    setupStore.nextStep();
  }

  async function handleIntegrationsNext() {
    let allValid = true;
    if (setupStore.data.sonarrEnabled && !setupStore.sonarrValidated) {
      if (!(await setupStore.testSonarr())) allValid = false;
    }
    if (setupStore.data.radarrEnabled && !setupStore.radarrValidated) {
      if (!(await setupStore.testRadarr())) allValid = false;
    }
    if (allValid || (!setupStore.data.sonarrEnabled && !setupStore.data.radarrEnabled)) setupStore.nextStep();
  }

  async function handleComplete() {
    if (await setupStore.completeSetup()) {
      setTimeout(() => {
        window.location.href = "/";
      }, 1500);
    }
  }
</script>

<div class="setup-viewport" class:visible={!ui.showIntro}>
  <SetupProgressTracker {steps} {currentStep} />

  <div class="setup-panel">
    {#if currentStep === 0}
      <SetupWelcomeStep onNext={() => setupStore.nextStep()} />
    {/if}
    {#if currentStep === 1}
      <SetupFshareStep {isLoading} {emailError} {passwordError} {validationError} onBack={() => setupStore.prevStep()} onNext={handleFshareNext} />
    {/if}
    {#if currentStep === 2}
      <SetupDownloadsStep {downloadPathError} onBack={() => setupStore.prevStep()} onNext={handleDownloadsNext} />
    {/if}
    {#if currentStep === 3}
      <SetupIntegrationsStep {isLoading} onBack={() => setupStore.prevStep()} onSkip={() => setupStore.nextStep()} onNext={handleIntegrationsNext} />
    {/if}
    {#if currentStep === 4}
      <SetupReviewStep {isLoading} onBack={() => setupStore.prevStep()} onComplete={handleComplete} />
    {/if}
  </div>
</div>

<style>
  .setup-viewport {
    position: fixed;
    inset: 0;
    background: radial-gradient(circle at top right,var(--cui-surface-3) 0%,var(--cui-surface-0) 100%);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: var(--cui-space-xl);
    opacity: 0;
    transition: opacity var(--cui-duration-glacial) var(--cui-ease-standard);
  }

  .setup-viewport.visible {
    opacity: 1;
  }

  .setup-panel {
    background: linear-gradient(145deg,var(--cui-glass),var(--cui-glass-heavy));
    border: 1px solid var(--cui-glass-border);
    border-radius: var(--cui-radius-xl);
    width: min(900px,95vw);
    min-height: min(700px,75vh);
    max-height: 75vh;
    overflow-y: auto;
    box-shadow: var(--cui-shadow-glass),var(--cui-glow-primary);
    position: relative;
  }

  .setup-panel::-webkit-scrollbar {
    width: var(--cui-space-xs);
  }

  .setup-panel::-webkit-scrollbar-track {
    background: var(--cui-input-bg);
  }

  .setup-panel::-webkit-scrollbar-thumb {
    background: var(--cui-glass-border);
    border-radius: var(--cui-radius-sm);
  }

  @media (max-width:768px) {
    .setup-viewport {
      padding: var(--cui-space-md);
    }
    .setup-panel {
      width: 100%;
      min-height: 0;
      max-height: 85vh;
    }
  }

</style>
