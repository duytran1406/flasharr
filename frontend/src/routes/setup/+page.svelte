<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { ui } from "$lib/stores/ui.svelte";
  import { setupStore } from "$lib/stores/setup.svelte";
  import { toasts } from "$lib/stores/toasts";
  import { animeFly } from "$lib/animations";

  let currentStep = $derived(setupStore.currentStep);
  // Access data directly from store for two-way binding (don't use $derived for bindable data)
  let isLoading = $derived(setupStore.isLoading);

  // Form validation states
  let fshareEmailError = $state("");
  let fsharePasswordError = $state("");
  let fshareValidationError = $state("");
  let downloadPathError = $state("");

  const steps = [
    { id: 0, title: "Welcome", label: "INIT" },
    { id: 1, title: "FShare", label: "AUTH" },
    { id: 2, title: "Downloads", label: "CONFIG" },
    { id: 3, title: "Integrations", label: "EXTEND" },
    { id: 4, title: "Review", label: "DEPLOY" },
  ];

  onMount(async () => {
    const complete = await setupStore.checkStatus();
    if (complete) {
      goto("/");
      return;
    }

    // Layout handles the intro mostly, but we ensure it finishes if we are here
    setTimeout(() => {
      ui.finishIntro();
    }, 2000);
  });

  function validateEmail(email: string): boolean {
    return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);
  }

  async function handleFshareNext() {
    fshareEmailError = "";
    fsharePasswordError = "";
    fshareValidationError = "";

    if (!setupStore.data.fshareEmail) {
      fshareEmailError = "Email is required";
      return;
    }

    if (!validateEmail(setupStore.data.fshareEmail)) {
      fshareEmailError = "Invalid email format";
      return;
    }

    if (!setupStore.data.fsharePassword) {
      fsharePasswordError = "Password is required";
      return;
    }

    const success = await setupStore.validateFshare();
    if (success) {
      setupStore.nextStep();
    } else {
      fshareValidationError =
        "Invalid FShare credentials. Please check your email and password.";
    }
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
      const valid = await setupStore.testSonarr();
      if (!valid) allValid = false;
    }

    if (setupStore.data.radarrEnabled && !setupStore.radarrValidated) {
      const valid = await setupStore.testRadarr();
      if (!valid) allValid = false;
    }

    if (
      allValid ||
      (!setupStore.data.sonarrEnabled && !setupStore.data.radarrEnabled)
    ) {
      setupStore.nextStep();
    }
  }

  async function handleComplete() {
    const success = await setupStore.completeSetup();
    if (success) {
      setTimeout(() => {
        // Use full page reload to ensure WebSocket connects
        window.location.href = "/";
      }, 1500);
    }
  }
</script>

<div class="setup-viewport" class:visible={!ui.showIntro}>
  <!-- Progress Tracker -->
  <div class="progress-tracker">
    {#each steps as step, i}
      <div
        class="step-indicator"
        class:active={currentStep === i}
        class:complete={currentStep > i}
      >
        <div class="step-dot">
          {#if currentStep > i}
            <span class="material-icons">check</span>
          {:else}
            <span class="step-num">{i + 1}</span>
          {/if}
        </div>
        <div class="step-meta">
          <span class="step-label">{step.label}</span>
          <span class="step-title">{step.title}</span>
        </div>
      </div>
      {#if i < steps.length - 1}
        <div class="step-connector" class:filled={currentStep > i}></div>
      {/if}
    {/each}
  </div>

  <!-- Main Panel -->
  <div class="setup-panel">
    <!-- Step 0: Welcome -->
    {#if currentStep === 0}
      <div class="panel-content" in:animeFly={{ y: 20, duration: 400 }}>
        <div class="features-grid">
          <div class="feature-item">
            <div class="feature-icon">
              <span class="material-icons">bolt</span>
            </div>
            <h3>High-Speed Engine</h3>
            <p>Optimized download manager with resume support</p>
          </div>
          <div class="feature-item">
            <div class="feature-icon">
              <span class="material-icons">hub</span>
            </div>
            <h3>Smart Integration</h3>
            <p>Seamless connection with Sonarr, Radarr & Jellyfin</p>
          </div>
          <div class="feature-item">
            <div class="feature-icon">
              <span class="material-icons">auto_awesome</span>
            </div>
            <h3>Auto Detection</h3>
            <p>Intelligent media parsing and organization</p>
          </div>
        </div>

        <button class="btn-primary-large" onclick={() => setupStore.nextStep()}>
          BEGIN SETUP
          <span class="material-icons">arrow_forward</span>
        </button>
      </div>
    {/if}

    <!-- Step 1: FShare Account -->
    {#if currentStep === 1}
      <div class="panel-content" in:animeFly={{ y: 20, duration: 400 }}>
        <div class="section-header">
          <div class="fshare-logo-container">
            <div class="fshare-logo-ring">
              <div class="fshare-logo-inner">
                <div class="fshare-logo-bg">
                  <img src="/images/flasharr_logo.png" alt="Flasharr" />
                </div>
              </div>
            </div>
          </div>
          <div>
            <h2>FShare Authentication</h2>
            <p>Connect your FShare account to enable downloads</p>
          </div>
        </div>

        <div class="form-section">
          <div class="input-group">
            <label for="email">EMAIL ADDRESS</label>
            <input
              type="email"
              id="email"
              class="input-field"
              class:error={fshareEmailError}
              bind:value={setupStore.data.fshareEmail}
              placeholder="your.email@example.com"
              disabled={isLoading}
            />
            {#if fshareEmailError}
              <span class="input-error">{fshareEmailError}</span>
            {/if}
          </div>

          <div class="input-group">
            <label for="password">PASSWORD</label>
            <input
              type="password"
              id="password"
              class="input-field"
              class:error={fsharePasswordError}
              bind:value={setupStore.data.fsharePassword}
              placeholder="Enter your password"
              disabled={isLoading}
            />
            {#if fsharePasswordError}
              <span class="input-error">{fsharePasswordError}</span>
            {/if}
          </div>

          {#if fshareValidationError}
            <div
              class="alert-box error"
              transition:animeFly={{ y: -10, duration: 300 }}
            >
              <span class="material-icons">error_outline</span>
              <div>
                <strong>Authentication Failed</strong>
                <p>{fshareValidationError}</p>
              </div>
            </div>
          {/if}

          <div class="info-box">
            <span class="material-icons">info</span>
            <span>
              Don't have an account?
              <a
                href="https://fshare.vn/reg.php"
                target="_blank"
                rel="noopener noreferrer"
              >
                Register for FShare
              </a>
            </span>
          </div>
        </div>

        <div class="action-bar">
          <button
            class="btn-secondary"
            onclick={() => setupStore.prevStep()}
            disabled={isLoading}
          >
            <span class="material-icons">arrow_back</span>
            BACK
          </button>
          <button
            class="btn-primary"
            onclick={handleFshareNext}
            disabled={isLoading}
          >
            {isLoading ? "VALIDATING..." : "CONTINUE"}
            <span class="material-icons">arrow_forward</span>
          </button>
        </div>
      </div>
    {/if}

    <!-- Step 2: Downloads -->
    {#if currentStep === 2}
      <div class="panel-content" in:animeFly={{ y: 20, duration: 400 }}>
        <div class="section-header">
          <div class="section-icon">
            <span class="material-icons">folder_open</span>
          </div>
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
              <input
                type="text"
                id="path"
                class="input-field"
                class:error={downloadPathError}
                bind:value={setupStore.data.downloadPath}
                placeholder="/downloads"
              />
            </div>
            {#if downloadPathError}
              <span class="input-error">{downloadPathError}</span>
            {/if}
            <span class="input-hint"
              >Absolute path where files will be saved</span
            >
          </div>

          <div class="input-group">
            <label for="concurrent">
              MAX CONCURRENT DOWNLOADS
              <span class="value-pill">{setupStore.data.maxConcurrent}</span>
            </label>
            <input
              type="range"
              id="concurrent"
              min="1"
              max="10"
              bind:value={setupStore.data.maxConcurrent}
              class="range-slider"
            />
            <span class="input-hint">Recommended: 3-5 concurrent downloads</span
            >
          </div>
        </div>

        <div class="action-bar">
          <button class="btn-secondary" onclick={() => setupStore.prevStep()}>
            <span class="material-icons">arrow_back</span>
            BACK
          </button>
          <button class="btn-primary" onclick={handleDownloadsNext}>
            CONTINUE
            <span class="material-icons">arrow_forward</span>
          </button>
        </div>
      </div>
    {/if}

    <!-- Step 3: Integrations -->
    {#if currentStep === 3}
      <div class="panel-content" in:animeFly={{ y: 20, duration: 400 }}>
        <div class="section-header">
          <div class="section-icon">
            <span class="material-icons">extension</span>
          </div>
          <div>
            <h2>Optional Integrations</h2>
            <p>Connect to Sonarr, Radarr, or Jellyfin (skip if not needed)</p>
          </div>
        </div>

        <div class="form-section">
          <!-- Sonarr -->
          <div class="integration-box">
            <div class="integration-header">
              <div class="integration-info">
                <div class="integration-logo-container">
                  <img
                    src="https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/sonarr.png"
                    alt="Sonarr"
                    class="integration-logo"
                  />
                </div>
                <span>Sonarr</span>
              </div>
              <label class="switch">
                <input
                  type="checkbox"
                  bind:checked={setupStore.data.sonarrEnabled}
                />
                <span class="slider"></span>
              </label>
            </div>
            {#if setupStore.data.sonarrEnabled}
              <div
                class="integration-fields"
                transition:animeFly={{ y: -10, duration: 300 }}
              >
                <input
                  type="text"
                  class="input-field-sm"
                  bind:value={setupStore.data.sonarrUrl}
                  placeholder="http://localhost:8989"
                />
                <input
                  type="password"
                  class="input-field-sm"
                  bind:value={setupStore.data.sonarrApiKey}
                  placeholder="API Key"
                />
                <button
                  class="btn-test"
                  onclick={() => setupStore.testSonarr()}
                  disabled={isLoading}
                >
                  {setupStore.sonarrValidated
                    ? "✓ CONNECTED"
                    : "TEST CONNECTION"}
                </button>
              </div>
            {/if}
          </div>

          <!-- Radarr -->
          <div class="integration-box">
            <div class="integration-header">
              <div class="integration-info">
                <div class="integration-logo-container">
                  <img
                    src="https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/radarr.png"
                    alt="Radarr"
                    class="integration-logo"
                  />
                </div>
                <span>Radarr</span>
              </div>
              <label class="switch">
                <input
                  type="checkbox"
                  bind:checked={setupStore.data.radarrEnabled}
                />
                <span class="slider"></span>
              </label>
            </div>
            {#if setupStore.data.radarrEnabled}
              <div
                class="integration-fields"
                transition:animeFly={{ y: -10, duration: 300 }}
              >
                <input
                  type="text"
                  class="input-field-sm"
                  bind:value={setupStore.data.radarrUrl}
                  placeholder="http://localhost:7878"
                />
                <input
                  type="password"
                  class="input-field-sm"
                  bind:value={setupStore.data.radarrApiKey}
                  placeholder="API Key"
                />
                <button
                  class="btn-test"
                  onclick={() => setupStore.testRadarr()}
                  disabled={isLoading}
                >
                  {setupStore.radarrValidated
                    ? "✓ CONNECTED"
                    : "TEST CONNECTION"}
                </button>
              </div>
            {/if}
          </div>
        </div>

        <div class="action-bar">
          <button class="btn-secondary" onclick={() => setupStore.prevStep()}>
            <span class="material-icons">arrow_back</span>
            BACK
          </button>
          <button class="btn-secondary" onclick={() => setupStore.nextStep()}>
            SKIP
          </button>
          <button
            class="btn-primary"
            onclick={handleIntegrationsNext}
            disabled={isLoading}
          >
            CONTINUE
            <span class="material-icons">arrow_forward</span>
          </button>
        </div>
      </div>
    {/if}

    <!-- Step 4: Review -->
    {#if currentStep === 4}
      <div class="panel-content" in:animeFly={{ y: 20, duration: 400 }}>
        <div class="section-header">
          <div class="section-icon">
            <span class="material-icons">task_alt</span>
          </div>
          <div>
            <h2>Configuration Review</h2>
            <p>Verify your settings before deployment</p>
          </div>
        </div>

        <div class="review-grid">
          <div class="review-section">
            <div class="review-label">FSHARE ACCOUNT</div>
            <div class="review-item">
              <span class="material-icons">email</span>
              <span>{setupStore.data.fshareEmail}</span>
            </div>
          </div>

          <div class="review-section">
            <div class="review-label">DOWNLOAD CONFIG</div>
            <div class="review-item">
              <span class="material-icons">folder</span>
              <span>{setupStore.data.downloadPath}</span>
            </div>
            <div class="review-item">
              <span class="material-icons">speed</span>
              <span>{setupStore.data.maxConcurrent} concurrent downloads</span>
            </div>
          </div>

          {#if setupStore.data.sonarrEnabled || setupStore.data.radarrEnabled || setupStore.data.jellyfinEnabled}
            <div class="review-section">
              <div class="review-label">INTEGRATIONS</div>
              {#if setupStore.data.sonarrEnabled}
                <div class="review-item">
                  <span
                    class="material-icons"
                    class:validated={setupStore.sonarrValidated}
                    class:not-validated={!setupStore.sonarrValidated}
                  >
                    {setupStore.sonarrValidated ? "check_circle" : "cancel"}
                  </span>
                  <span>Sonarr: {setupStore.data.sonarrUrl}</span>
                </div>
              {/if}
              {#if setupStore.data.radarrEnabled}
                <div class="review-item">
                  <span
                    class="material-icons"
                    class:validated={setupStore.radarrValidated}
                    class:not-validated={!setupStore.radarrValidated}
                  >
                    {setupStore.radarrValidated ? "check_circle" : "cancel"}
                  </span>
                  <span>Radarr: {setupStore.data.radarrUrl}</span>
                </div>
              {/if}
              {#if !setupStore.data.sonarrEnabled && !setupStore.data.radarrEnabled}
                <div class="review-item">
                  <span class="material-icons" style="color: var(--text-muted);"
                    >info</span
                  >
                  <span style="color: var(--text-muted);"
                    >No integrations configured</span
                  >
                </div>
              {/if}
            </div>
          {/if}
        </div>

        <div class="action-bar">
          <button
            class="btn-secondary"
            onclick={() => setupStore.prevStep()}
            disabled={isLoading}
          >
            <span class="material-icons">arrow_back</span>
            BACK
          </button>
          <button
            class="btn-primary-large"
            onclick={handleComplete}
            disabled={isLoading}
          >
            {isLoading ? "DEPLOYING..." : "COMPLETE SETUP"}
            <span class="material-icons">rocket_launch</span>
          </button>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  :global(body) {
    --color-primary: #00f3ff;
    --color-secondary: #00ffa3;
    --text-primary: #e2e8f0;
    --text-secondary: #94a3b8;
    --text-muted: #64748b;
    --font-body: "Inter", sans-serif;
    --font-mono: "JetBrains Mono", monospace;
  }

  @keyframes delete-text {
    from {
      width: 25ch;
      content: "INITIALIZING SYSTEM...";
    }
    to {
      width: 0;
    }
  }

  @keyframes tagline-fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  @keyframes typing {
    from {
      width: 0;
    }
    to {
      width: 100%;
    }
  }

  @keyframes blink-cursor {
    50% {
      border-color: transparent;
    }
  }

  @keyframes logo-intro {
    0% {
      transform: scale(0.8) translateZ(0);
      opacity: 0;
      filter: drop-shadow(0 0 0px rgba(0, 243, 255, 0)) brightness(0.5)
        blur(10px);
    }
    40% {
      opacity: 0.3;
      filter: drop-shadow(0 0 20px rgba(0, 243, 255, 0.4))
        drop-shadow(0 0 40px rgba(138, 43, 226, 0.3)) brightness(1.2) blur(5px);
    }
    70% {
      opacity: 0.8;
      transform: scale(1.05) translateZ(0);
      filter: drop-shadow(0 0 3px rgba(0, 243, 255, 0.9))
        drop-shadow(0 0 25px rgba(0, 243, 255, 0.5))
        drop-shadow(0 0 50px rgba(138, 43, 226, 0.3)) brightness(1.3);
    }
    100% {
      transform: scale(1) translateZ(0);
      opacity: 1;
      filter: drop-shadow(0 0 2px rgba(0, 243, 255, 0.8))
        drop-shadow(0 0 20px rgba(0, 243, 255, 0.4))
        drop-shadow(0 0 40px rgba(138, 43, 226, 0.2)) brightness(1);
    }
  }

  @keyframes logo-float {
    0%,
    100% {
      transform: translateY(0) scale(1);
    }
    50% {
      transform: translateY(-10px) scale(1.02);
    }
  }

  @keyframes glow-pulse {
    0%,
    100% {
      opacity: 0.5;
      transform: scale(1);
    }
    50% {
      opacity: 1;
      transform: scale(1.1);
    }
  }

  @keyframes text-blink {
    50% {
      opacity: 0.3;
    }
  }

  /* Setup Viewport */
  .setup-viewport {
    position: fixed;
    inset: 0;
    background: radial-gradient(circle at top right, #0a0e1a 0%, #010203 100%);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 2rem;
    opacity: 0;
    transition: opacity 0.5s ease;
  }

  .setup-viewport.visible {
    opacity: 1;
  }

  /* Progress Tracker */
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
    gap: 0.75rem;
    text-align: center;
  }

  .step-dot {
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.03);
    border: 2px solid rgba(255, 255, 255, 0.1);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.3s ease;
    position: relative;
    z-index: 2;
  }

  .step-indicator.active .step-dot {
    background: linear-gradient(
      135deg,
      rgba(0, 243, 255, 0.2),
      rgba(0, 243, 255, 0.1)
    );
    border-color: var(--color-primary);
    box-shadow: 0 0 30px rgba(0, 243, 255, 0.4);
  }

  .step-indicator.complete .step-dot {
    background: rgba(0, 255, 163, 0.1);
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
    gap: 0.25rem;
  }

  .step-label {
    font-family: var(--font-mono);
    font-size: 0.6rem;
    font-weight: 700;
    color: var(--text-muted);
    letter-spacing: 0.15em;
  }

  .step-indicator.active .step-label {
    color: var(--color-primary);
  }

  .step-title {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-secondary);
  }

  .step-indicator.active .step-title {
    color: var(--text-primary);
  }

  .step-connector {
    width: 60px;
    height: 2px;
    background: rgba(255, 255, 255, 0.1);
    margin: 0 -1rem;
    position: relative;
    z-index: 1;
    transition: all 0.3s ease;
  }

  .step-connector.filled {
    background: linear-gradient(
      90deg,
      var(--color-primary),
      var(--color-secondary)
    );
  }

  /* Setup Panel - CONSISTENT SIZE */
  .setup-panel {
    background: linear-gradient(
      145deg,
      rgba(15, 23, 42, 0.9),
      rgba(2, 4, 8, 0.95)
    );
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 20px;
    width: 900px;
    height: 700px;
    max-width: 95vw;
    max-height: 75vh;
    overflow-y: auto;
    box-shadow:
      0 30px 60px -12px rgba(0, 0, 0, 0.8),
      0 0 40px rgba(0, 243, 255, 0.05);
    position: relative;
  }

  .setup-panel::-webkit-scrollbar {
    width: 8px;
  }

  .setup-panel::-webkit-scrollbar-track {
    background: rgba(0, 0, 0, 0.2);
  }

  .setup-panel::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 4px;
  }

  .panel-content {
    padding: 3rem;
    min-height: 100%;
    display: flex;
    flex-direction: column;
  }

  /* Features Grid - 3 COLUMNS HORIZONTAL */
  .features-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1.5rem;
    margin-bottom: 2.5rem;
  }

  .feature-item {
    background: linear-gradient(
      145deg,
      rgba(255, 255, 255, 0.03),
      rgba(255, 255, 255, 0.01)
    );
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    padding: 2rem 1.5rem;
    text-align: center;
    transition: all 0.3s ease;
    backdrop-filter: blur(10px);
  }

  .feature-item:hover {
    background: linear-gradient(
      145deg,
      rgba(0, 243, 255, 0.08),
      rgba(0, 243, 255, 0.03)
    );
    border-color: rgba(0, 243, 255, 0.3);
    transform: translateY(-4px);
    box-shadow: 0 10px 30px rgba(0, 243, 255, 0.1);
  }

  .feature-icon {
    width: auto;
    height: auto;
    margin: 0 auto 1rem;
    display: inline-block;
    position: relative;
  }

  .feature-icon::before {
    display: none;
  }

  .feature-icon .material-icons {
    font-size: 2.5rem;
    color: var(--color-primary);
    padding: 10px;
    border-radius: 16px;
    display: block;
    filter: drop-shadow(0 0 1px rgba(0, 243, 255, 0.6))
      drop-shadow(0 0 15px rgba(0, 243, 255, 0.4))
      drop-shadow(0 0 30px rgba(0, 243, 255, 0.2));
  }

  .feature-item h3 {
    font-size: 1rem;
    font-weight: 700;
    margin: 0 0 0.5rem;
    color: #fff;
  }

  .feature-item p {
    font-size: 0.85rem;
    color: var(--text-secondary);
    line-height: 1.5;
    margin: 0;
  }

  /* Section Header */
  .section-header {
    display: flex;
    align-items: center;
    gap: 1.5rem;
    margin-bottom: 2rem;
    padding-bottom: 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .section-icon {
    width: 64px;
    height: 64px;
    background: linear-gradient(
      145deg,
      rgba(0, 243, 255, 0.15),
      rgba(0, 243, 255, 0.05)
    );
    border-radius: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid rgba(0, 243, 255, 0.3);
    flex-shrink: 0;
    box-shadow:
      0 8px 20px rgba(0, 243, 255, 0.1),
      inset 0 1px 0 rgba(255, 255, 255, 0.1);
    backdrop-filter: blur(10px);
  }

  .section-icon .material-icons {
    font-size: 2rem;
    color: var(--color-primary);
    filter: drop-shadow(0 0 10px rgba(0, 243, 255, 0.5));
  }

  .section-logo {
    width: 40px;
    height: 40px;
    object-fit: contain;
  }

  /* FShare Logo Styling (matching UserAvatar) */
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
    background: linear-gradient(135deg, #00f3ff 0%, #4facfe 100%);
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.3);
    padding: 2px;
    transition: all 0.3s ease;
    animation: pulse-fshare 2s infinite;
  }

  @keyframes pulse-fshare {
    0%,
    100% {
      box-shadow: 0 0 20px rgba(0, 243, 255, 0.3);
    }
    50% {
      box-shadow: 0 0 30px rgba(0, 243, 255, 0.5);
    }
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

  .section-header h2 {
    font-size: 1.5rem;
    font-weight: 800;
    margin: 0 0 0.25rem;
    color: #fff;
  }

  .section-header p {
    font-size: 0.9rem;
    color: var(--text-secondary);
    margin: 0;
  }

  /* Form Section */
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
    gap: 0.5rem;
  }

  .input-group label {
    font-family: var(--font-mono);
    font-size: 0.7rem;
    font-weight: 700;
    color: var(--text-secondary);
    letter-spacing: 0.1em;
  }

  .value-pill {
    display: inline-block;
    background: var(--color-primary);
    color: #000;
    padding: 0.25rem 0.75rem;
    border-radius: 12px;
    font-size: 0.7rem;
    font-weight: 700;
    margin-left: 0.5rem;
  }

  .input-field {
    width: 100%;
    background: rgba(0, 0, 0, 0.4);
    border: 2px solid rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    padding: 1rem 1.25rem;
    color: #fff;
    font-size: 0.95rem;
    transition: all 0.2s ease;
    font-family: var(--font-body);
  }

  .input-field:focus {
    outline: none;
    border-color: var(--color-primary);
    box-shadow: 0 0 0 3px rgba(0, 243, 255, 0.1);
    background: rgba(0, 0, 0, 0.6);
  }

  .input-field.error {
    border-color: #ff5252;
    box-shadow: 0 0 0 3px rgba(255, 82, 82, 0.1);
  }

  .input-field:disabled {
    opacity: 0.5;
    cursor: not-allowed;
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
    font-size: 0.8rem;
    color: #ff5252;
  }

  .input-hint {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .range-slider {
    width: 100%;
    height: 6px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 3px;
    outline: none;
    appearance: none;
    -webkit-appearance: none;
  }

  .range-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 20px;
    height: 20px;
    background: var(--color-primary);
    border-radius: 50%;
    cursor: pointer;
    box-shadow: 0 0 10px rgba(0, 243, 255, 0.5);
  }

  .range-slider::-moz-range-thumb {
    width: 20px;
    height: 20px;
    background: var(--color-primary);
    border-radius: 50%;
    cursor: pointer;
    border: none;
    box-shadow: 0 0 10px rgba(0, 243, 255, 0.5);
  }

  /* Alert Box */
  .alert-box {
    display: flex;
    align-items: flex-start;
    gap: 1rem;
    padding: 1rem 1.25rem;
    border-radius: 10px;
  }

  .alert-box.error {
    background: rgba(255, 82, 82, 0.1);
    border: 1px solid rgba(255, 82, 82, 0.3);
  }

  .alert-box .material-icons {
    color: #ff5252;
    font-size: 1.5rem;
    flex-shrink: 0;
  }

  .alert-box strong {
    display: block;
    color: #ff5252;
    font-size: 0.9rem;
    margin-bottom: 0.25rem;
  }

  .alert-box p {
    color: var(--text-secondary);
    font-size: 0.85rem;
    margin: 0;
  }

  /* Info Box */
  .info-box {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.875rem 1.25rem;
    background: rgba(0, 243, 255, 0.05);
    border: 1px solid rgba(0, 243, 255, 0.1);
    border-radius: 10px;
    font-size: 0.85rem;
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
    transition: all 0.2s;
  }

  .info-box a:hover {
    text-decoration: underline;
    color: #fff;
  }

  /* Integration Box */
  .integration-box {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    padding: 1.25rem;
    transition: all 0.2s;
  }

  .integration-box:hover {
    border-color: rgba(0, 243, 255, 0.2);
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

  .integration-logo-container::before {
    display: none;
  }

  .integration-logo {
    width: 48px;
    height: 48px;
    object-fit: contain;
    padding: 6px;
    border-radius: 12px;
    position: relative;
    z-index: 1;
    filter: drop-shadow(0 0 1px rgba(255, 255, 255, 0.5))
      drop-shadow(0 0 8px rgba(0, 243, 255, 0.3));
  }

  .integration-info span {
    font-size: 1rem;
    font-weight: 600;
    color: #fff;
  }

  /* Toggle Switch */
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
    background: rgba(255, 255, 255, 0.1);
    transition: 0.3s;
    border-radius: 28px;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .slider:before {
    position: absolute;
    content: "";
    height: 20px;
    width: 20px;
    left: 4px;
    bottom: 3px;
    background: #fff;
    transition: 0.3s;
    border-radius: 50%;
  }

  input:checked + .slider {
    background: rgba(0, 255, 163, 0.2);
    border-color: var(--color-secondary);
    box-shadow: 0 0 10px rgba(0, 255, 163, 0.3);
  }

  input:checked + .slider:before {
    transform: translateX(24px);
    background: var(--color-secondary);
  }

  .integration-fields {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
  }

  .input-field-sm {
    width: 100%;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 0.75rem 1rem;
    color: #fff;
    font-size: 0.9rem;
    transition: all 0.2s;
  }

  .input-field-sm:focus {
    outline: none;
    border-color: var(--color-primary);
    box-shadow: 0 0 0 2px rgba(0, 243, 255, 0.1);
  }

  .btn-test {
    background: rgba(0, 243, 255, 0.1);
    border: 1px solid rgba(0, 243, 255, 0.3);
    color: var(--color-primary);
    padding: 0.75rem 1rem;
    border-radius: 8px;
    font-weight: 700;
    font-size: 0.75rem;
    letter-spacing: 0.05em;
    cursor: pointer;
    transition: all 0.2s;
    font-family: var(--font-mono);
  }

  .btn-test:hover {
    background: rgba(0, 243, 255, 0.2);
    border-color: var(--color-primary);
    box-shadow: 0 0 15px rgba(0, 243, 255, 0.2);
  }

  .btn-test:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Review Grid */
  .review-grid {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    margin-bottom: 2rem;
    flex: 1;
  }

  .review-section {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    padding: 1.5rem;
  }

  .review-label {
    font-family: var(--font-mono);
    font-size: 0.65rem;
    font-weight: 900;
    color: var(--color-primary);
    letter-spacing: 0.15em;
    margin-bottom: 1rem;
  }

  .review-item {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.75rem 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
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
    color: #ff4444;
  }

  .review-item span:last-child {
    color: var(--text-primary);
    font-size: 0.95rem;
  }

  /* Action Bar */
  .action-bar {
    display: flex;
    gap: 1rem;
    justify-content: flex-end;
    padding-top: 1.5rem;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    margin-top: auto;
  }

  /* Buttons */
  .btn-primary,
  .btn-secondary,
  .btn-primary-large {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding: 0.875rem 1.75rem;
    font-weight: 900;
    font-size: 0.7rem;
    letter-spacing: 0.1em;
    border-radius: 10px;
    cursor: pointer;
    transition: all 0.2s;
    font-family: var(--font-mono);
    border: none;
  }

  .btn-primary {
    background: linear-gradient(
      135deg,
      rgba(0, 243, 255, 0.2),
      rgba(0, 243, 255, 0.1)
    );
    color: var(--color-primary);
    border: 2px solid var(--color-primary);
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.3);
  }

  .btn-primary:hover {
    background: linear-gradient(
      135deg,
      rgba(0, 255, 163, 0.2),
      rgba(0, 255, 163, 0.1)
    );
    border-color: var(--color-secondary);
    color: var(--color-secondary);
    box-shadow: 0 0 30px rgba(0, 255, 163, 0.4);
    transform: translateY(-2px);
  }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-secondary {
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-secondary);
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .btn-secondary:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
    border-color: rgba(255, 255, 255, 0.2);
  }

  .btn-primary-large {
    background: linear-gradient(
      135deg,
      rgba(0, 243, 255, 0.2),
      rgba(0, 243, 255, 0.1)
    );
    color: var(--color-primary);
    border: 2px solid var(--color-primary);
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.3);
    padding: 1.125rem 2.5rem;
    font-size: 0.8rem;
  }

  .btn-primary-large:hover {
    background: linear-gradient(
      135deg,
      rgba(0, 255, 163, 0.2),
      rgba(0, 255, 163, 0.1)
    );
    border-color: var(--color-secondary);
    color: var(--color-secondary);
    box-shadow: 0 0 30px rgba(0, 255, 163, 0.4);
    transform: translateY(-2px);
  }

  /* Responsive */
  @media (max-width: 768px) {
    .setup-viewport {
      padding: 1rem;
    }

    .progress-tracker {
      flex-wrap: wrap;
      gap: 1rem;
      margin-bottom: 2rem;
    }

    .step-connector {
      display: none;
    }

    .setup-panel {
      width: 100%;
      height: auto;
      max-height: 85vh;
    }

    .panel-content {
      padding: 2rem 1.5rem;
    }

    .features-grid {
      grid-template-columns: 1fr;
    }

    .action-bar {
      flex-direction: column;
    }

    .btn-primary,
    .btn-secondary,
    .btn-primary-large {
      width: 100%;
    }
  }
</style>
