<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { accountStore } from "$lib/stores/account.svelte";
  import {
    systemStore,
    systemLogs,
    downloadSettings,
    indexerSettings,
    sonarrSettings,
    radarrSettings,
  } from "$lib/stores/system";
  import { toasts } from "$lib/stores/toasts";
  import { IdentityCard } from "$lib/components";

  // UI State
  let activeCategory = $state("accounts");
  let showSonarrApiKey = $state(false);
  let showRadarrApiKey = $state(false);
  let showApiKey = $state(false);
  let sonarrTesting = $state(false);
  let radarrTesting = $state(false);
  let logInterval: any;

  // Local state for editing (bound to inputs)
  let concurrency = $state(3);
  let threads = $state(4);
  let downloadPath = $state("");

  let sonarrEnabled = $state(false);
  let sonarrUrl = $state("http://localhost:8989");
  let sonarrApiKey = $state("");
  let sonarrAutoImport = $state(true);

  let radarrEnabled = $state(false);
  let radarrUrl = $state("http://localhost:7878");
  let radarrApiKey = $state("");
  let radarrAutoImport = $state(true);

  let indexerApiKey = $state("");
  let indexerUrl = $state("");

  // Reactive: sync store data to local state
  $effect(() => {
    const dlSettings = $downloadSettings;
    concurrency = dlSettings.max_concurrent;
    threads = dlSettings.segments_per_download;
    downloadPath = dlSettings.directory;
  });

  $effect(() => {
    const idxSettings = $indexerSettings;
    indexerApiKey = idxSettings.api_key;
    indexerUrl = idxSettings.indexer_url;
  });

  $effect(() => {
    const snrSettings = $sonarrSettings;
    sonarrEnabled = snrSettings.enabled;
    sonarrUrl = snrSettings.url;
    sonarrApiKey = snrSettings.api_key;
    sonarrAutoImport = snrSettings.auto_import;
  });

  $effect(() => {
    const rdrSettings = $radarrSettings;
    radarrEnabled = rdrSettings.enabled;
    radarrUrl = rdrSettings.url;
    radarrApiKey = rdrSettings.api_key;
    radarrAutoImport = rdrSettings.auto_import;
  });

  let logs = $derived($systemLogs);

  onMount(() => {
    // Fetch settings-specific data using systemStore
    systemStore.fetchLogs();
    systemStore.fetchDownloadSettings();
    systemStore.fetchIndexerSettings();
    systemStore.fetchSonarrSettings();
    systemStore.fetchRadarrSettings();

    logInterval = setInterval(() => systemStore.fetchLogs(), 3000);

    // Set Page Header
    const headerContainer = document.getElementById("header-dynamic-content");
    if (headerContainer) {
      headerContainer.innerHTML = `
        <div style="display: flex; align-items: center; gap: 0.75rem;">
          <span class="material-icons" style="color: var(--color-primary); font-size: 1.5rem;">settings_suggest</span>
          <h1 style="font-size: 0.9rem; font-weight: 800; letter-spacing: 0.1em; text-transform: uppercase; margin: 0; color: #fff;">System Configuration</h1>
        </div>
      `;
    }

    return () => {
      if (logInterval) clearInterval(logInterval);
    };
  });

  async function saveEngineConfig() {
    const result = await systemStore.saveDownloadSettings({
      directory: downloadPath,
      max_concurrent: parseInt(concurrency.toString()),
      segments_per_download: parseInt(threads.toString()),
    });

    if (result.success) {
      toasts.success(result.message || "Engine configuration saved");
    } else {
      toasts.error(result.message || "Failed to save configuration");
    }
  }

  async function clearLogs() {
    systemStore.clearLogs();
    toasts.success("Terminal logs cleared");
  }

  async function generateApiKey() {
    const newKey = await systemStore.generateIndexerApiKey();
    if (newKey) {
      toasts.success("New API key generated");
    } else {
      toasts.error("Failed to generate API key");
    }
  }

  async function saveSonarrSettings() {
    const result = await systemStore.saveSonarrSettings({
      enabled: sonarrEnabled,
      url: sonarrUrl,
      api_key: sonarrApiKey,
      auto_import: sonarrAutoImport,
    });

    if (result.success) {
      toasts.success(result.message || "Sonarr settings saved");
    } else {
      toasts.error("Failed to save Sonarr settings");
    }
  }

  async function testSonarrConnection() {
    sonarrTesting = true;
    const result = await systemStore.testSonarrConnection({
      enabled: sonarrEnabled,
      url: sonarrUrl,
      api_key: sonarrApiKey,
      auto_import: sonarrAutoImport,
    });

    if (result.success) {
      toasts.success(result.message || "Sonarr connection successful");
    } else {
      toasts.error(result.message || "Sonarr connection failed");
    }
    sonarrTesting = false;
  }

  async function saveRadarrSettings() {
    const result = await systemStore.saveRadarrSettings({
      enabled: radarrEnabled,
      url: radarrUrl,
      api_key: radarrApiKey,
      auto_import: radarrAutoImport,
    });

    if (result.success) {
      toasts.success(result.message || "Radarr settings saved");
    } else {
      toasts.error("Failed to save Radarr settings");
    }
  }

  async function testRadarrConnection() {
    radarrTesting = true;
    const result = await systemStore.testRadarrConnection({
      enabled: radarrEnabled,
      url: radarrUrl,
      api_key: radarrApiKey,
      auto_import: radarrAutoImport,
    });

    if (result.success) {
      toasts.success(result.message || "Radarr connection successful");
    } else {
      toasts.error(result.message || "Radarr connection failed");
    }
    radarrTesting = false;
  }

  async function copyToClipboard(text: string, label: string) {
    try {
      await navigator.clipboard.writeText(text);
      toasts.success(`${label} copied to clipboard`);
    } catch (e) {
      toasts.error("Failed to copy to clipboard");
    }
  }
</script>

<svelte:head>
  <title>Settings - Flasharr</title>
</svelte:head>

<div class="settings-viewport">
  <div class="settings-layout-v3">
    <!-- Top Sub-Header with Tabs -->
    <header class="settings-sub-header">
      <div class="header-content">
        <div class="header-brand">
          <span class="material-icons">tune</span>
          <div class="brand-text">
            <h2>SYSTEM_CONFIG</h2>
            <div class="status-badge">
              <span class="dot"></span>
              CORE_ACTIVE
            </div>
          </div>
        </div>

        <nav class="settings-tabs-v3">
          <button
            class="tab-btn-v3"
            class:active={activeCategory === "accounts"}
            onclick={() => (activeCategory = "accounts")}
          >
            <span class="material-icons">account_circle</span>
            <span>Accounts</span>
          </button>
          <button
            class="tab-btn-v3"
            class:active={activeCategory === "engine"}
            onclick={() => (activeCategory = "engine")}
          >
            <span class="material-icons">bolt</span>
            <span>Engine</span>
          </button>
          <button
            class="tab-btn-v3"
            class:active={activeCategory === "indexer"}
            onclick={() => (activeCategory = "indexer")}
          >
            <span class="material-icons">travel_explore</span>
            <span>Indexer</span>
          </button>
          <button
            class="tab-btn-v3"
            class:active={activeCategory === "arrs"}
            onclick={() => (activeCategory = "arrs")}
          >
            <span class="material-icons">hub</span>
            <span>Services</span>
          </button>
          <button
            class="tab-btn-v3"
            class:active={activeCategory === "system"}
            onclick={() => (activeCategory = "system")}
          >
            <span class="material-icons">terminal</span>
            <span>System</span>
          </button>
        </nav>
      </div>
    </header>

    <!-- Main Dynamic Content -->
    <main class="settings-main-v3">
      <div class="content-container-v3">
        {#if activeCategory === "accounts"}
          <section class="settings-section">
            <div class="section-v3-title">
              <h3>FShare Cloud Access</h3>
              <p>Manage multiple premium connections and monitoring quotas.</p>
            </div>
            <div class="accounts-grid-v3">
              {#each accountStore.listFormatted as acc}
                <div class="identity-wrapper-v3">
                  <IdentityCard
                    email={acc.email}
                    rank={acc.rank}
                    expiry={acc.expiry}
                    quotaUsed={acc.quotaUsed}
                    quotaTotal={acc.quotaTotal}
                    quotaPercent={acc.quotaPercent}
                    onRefresh={() => accountStore.refresh(acc.email)}
                    onLogout={async () => {
                      if (
                        confirm(
                          `Are you sure you want to logout from ${acc.email}?`,
                        )
                      ) {
                        await accountStore.logout(acc.email);
                        toasts.success("Account removed");
                      }
                    }}
                  />
                </div>
              {:else}
                <div class="empty-placeholder-v3">
                  <span class="material-icons">no_accounts</span>
                  <p>No active connections detected</p>
                </div>
              {/each}
              <button
                class="add-account-card-v3"
                onclick={() => goto("/setup")}
              >
                <div class="inner">
                  <span class="material-icons">add</span>
                  <span>Link New Account</span>
                </div>
              </button>
            </div>
          </section>
        {/if}

        {#if activeCategory === "engine"}
          <section class="settings-section max-w-3xl">
            <div class="section-v3-title">
              <h3>Download Engine</h3>
              <p>Core performance tuning and storage orchestration.</p>
            </div>
            <div class="premium-config-card">
              <div class="input-v3-group">
                <label for="v3-path">SYSTEM DOWNLOAD PATH</label>
                <div class="input-v3-box">
                  <span class="material-icons">folder</span>
                  <input
                    type="text"
                    id="v3-path"
                    bind:value={downloadPath}
                    placeholder="/media/downloads"
                  />
                </div>
                <small>Absolute path on host filesystem</small>
              </div>

              <div class="input-v3-group">
                <div class="label-flex">
                  <label for="v3-concurrency">MAX CONCURRENCY</label>
                  <span class="value-chip">{concurrency} TASKS</span>
                </div>
                <div
                  class="range-v3-wrapper"
                  style="--slider-val: {((concurrency - 1) / 9) *
                    100}%; --current-val: '{concurrency}';"
                >
                  <div class="v3-track-dots">
                    {#each Array(10) as _, i}
                      <div class="dot" class:active={concurrency > i}></div>
                    {/each}
                  </div>
                  <input
                    type="range"
                    id="v3-concurrency"
                    min="1"
                    max="10"
                    bind:value={concurrency}
                  />
                  <div class="marks-premium">
                    <span>1</span>
                    <span>5</span>
                    <span>10</span>
                  </div>
                </div>
              </div>

              <div class="card-action-v3">
                <button class="btn-apply-v3" onclick={saveEngineConfig}>
                  <span class="material-icons">save</span>
                  APPLY ENGINE SETTINGS
                </button>
              </div>
            </div>
          </section>
        {/if}

        {#if activeCategory === "indexer"}
          <section class="settings-section max-w-3xl">
            <div class="section-v3-title">
              <h3>Newznab Indexer API</h3>
              <p>Expose local library metadata to external automation tools.</p>
            </div>
            <div class="premium-config-card">
              <div class="input-v3-group">
                <label for="indexer-endpoint">INDEXER ENDPOINT</label>
                <div class="input-v3-box readonly">
                  <span class="material-icons">link</span>
                  <input
                    type="text"
                    id="indexer-endpoint"
                    value={indexerUrl}
                    readonly
                  />
                  <button
                    class="icon-action"
                    onclick={() => copyToClipboard(indexerUrl, "Indexer URL")}
                  >
                    <span class="material-icons">content_copy</span>
                  </button>
                </div>
              </div>

              <div class="input-v3-group">
                <label for="indexer-key">API KEY</label>
                <div class="input-v3-box readonly">
                  <span class="material-icons">key</span>
                  <input
                    type={showApiKey ? "text" : "password"}
                    id="indexer-key"
                    value={indexerApiKey}
                    readonly
                  />
                  <button
                    class="icon-action"
                    onclick={() => (showApiKey = !showApiKey)}
                  >
                    <span class="material-icons"
                      >{showApiKey ? "visibility_off" : "visibility"}</span
                    >
                  </button>
                  <button
                    class="icon-action"
                    onclick={() => copyToClipboard(indexerApiKey, "API Key")}
                  >
                    <span class="material-icons">content_copy</span>
                  </button>
                </div>
              </div>

              <div class="card-action-v3">
                <button class="btn-secondary-v3" onclick={generateApiKey}>
                  <span class="material-icons">refresh</span>
                  REGENERATE API KEY
                </button>
              </div>
            </div>
          </section>
        {/if}

        {#if activeCategory === "arrs"}
          <section class="settings-section">
            <div class="section-v3-title">
              <h3>Arr Cloud Integrations</h3>
              <p>Automate library synchronization with Sonarr and Radarr.</p>
            </div>
            <div class="arr-grid-v3">
              <!-- Sonarr -->
              <div class="arr-node-v3 sonarr" class:active={sonarrEnabled}>
                <div class="node-header">
                  <div class="brand sonarr">
                    <img
                      src="https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/sonarr.png"
                      alt="Sonarr"
                      class="brand-logo"
                    />
                    <span>SONARR</span>
                  </div>
                  <label class="hybrid-switch">
                    <input type="checkbox" bind:checked={sonarrEnabled} />
                    <span class="switch-ui"></span>
                  </label>
                </div>
                <div class="node-body">
                  <div class="node-field">
                    <label for="sn-url">SERVICE URL</label>
                    <input
                      type="text"
                      id="sn-url"
                      bind:value={sonarrUrl}
                      placeholder="http://localhost:8989"
                    />
                  </div>
                  <div class="node-field">
                    <label for="sn-key">API KEY</label>
                    <div class="pass-box">
                      <input
                        type={showSonarrApiKey ? "text" : "password"}
                        id="sn-key"
                        bind:value={sonarrApiKey}
                        placeholder="Enter your Sonarr API key"
                      />
                      <button
                        class="visibility-toggle"
                        onclick={() => (showSonarrApiKey = !showSonarrApiKey)}
                      >
                        <span class="material-icons"
                          >{showSonarrApiKey
                            ? "visibility_off"
                            : "visibility"}</span
                        >
                      </button>
                    </div>
                  </div>
                  <div class="node-toggle">
                    <div class="txt">
                      <span>Auto-Import</span>
                      <small>Trigger import on download completion</small>
                    </div>
                    <label class="v3-switch-mini">
                      <input type="checkbox" bind:checked={sonarrAutoImport} />
                      <span class="slider-mini"></span>
                    </label>
                  </div>
                </div>
                <div class="node-actions">
                  <button
                    class="btn-test"
                    onclick={testSonarrConnection}
                    disabled={sonarrTesting}
                  >
                    <span class="material-icons" class:rotating={sonarrTesting}
                      >{sonarrTesting ? "refresh" : "sync_alt"}</span
                    >
                    {sonarrTesting ? "TESTING" : "TEST"}
                  </button>
                  <button class="btn-save" onclick={saveSonarrSettings}
                    >SAVE</button
                  >
                </div>
              </div>

              <!-- Radarr -->
              <div class="arr-node-v3 radarr" class:active={radarrEnabled}>
                <div class="node-header">
                  <div class="brand radarr">
                    <img
                      src="https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/radarr.png"
                      alt="Radarr"
                      class="brand-logo"
                    />
                    <span>RADARR</span>
                  </div>
                  <label class="hybrid-switch">
                    <input type="checkbox" bind:checked={radarrEnabled} />
                    <span class="switch-ui"></span>
                  </label>
                </div>
                <div class="node-body">
                  <div class="node-field">
                    <label for="rd-url">SERVICE URL</label>
                    <input
                      type="text"
                      id="rd-url"
                      bind:value={radarrUrl}
                      placeholder="http://localhost:7878"
                    />
                  </div>
                  <div class="node-field">
                    <label for="rd-key">API KEY</label>
                    <div class="pass-box">
                      <input
                        type={showRadarrApiKey ? "text" : "password"}
                        id="rd-key"
                        bind:value={radarrApiKey}
                        placeholder="Enter your Radarr API key"
                      />
                      <button
                        class="visibility-toggle"
                        onclick={() => (showRadarrApiKey = !showRadarrApiKey)}
                      >
                        <span class="material-icons"
                          >{showRadarrApiKey
                            ? "visibility_off"
                            : "visibility"}</span
                        >
                      </button>
                    </div>
                  </div>
                  <div class="node-toggle">
                    <div class="txt">
                      <span>Auto-Import</span>
                      <small>Trigger import on download completion</small>
                    </div>
                    <label class="v3-switch-mini">
                      <input type="checkbox" bind:checked={radarrAutoImport} />
                      <span class="slider-mini"></span>
                    </label>
                  </div>
                </div>
                <div class="node-actions">
                  <button
                    class="btn-test"
                    onclick={testRadarrConnection}
                    disabled={radarrTesting}
                  >
                    <span class="material-icons" class:rotating={radarrTesting}
                      >{radarrTesting ? "refresh" : "sync_alt"}</span
                    >
                    {radarrTesting ? "TESTING" : "TEST"}
                  </button>
                  <button class="btn-save" onclick={saveRadarrSettings}
                    >SAVE</button
                  >
                </div>
              </div>
            </div>
          </section>
        {/if}

        {#if activeCategory === "system"}
          <section
            class="settings-section h-full flex flex-col overflow-hidden"
          >
            <div class="section-v3-title">
              <h3>System Core Logs</h3>
              <p>
                Real-time telemetry stream and internal security diagnostics.
              </p>
            </div>

            <div class="system-v3-layout">
              <div class="terminal-pane-v3">
                <div class="pane-header">
                  <div class="stream-status">
                    <span class="dot-pulse"></span>
                    <span>LIVE_EVENT_FEED</span>
                  </div>
                  <button class="btn-clear" onclick={clearLogs}>
                    <span class="material-icons">delete_sweep</span>
                    CLEAR
                  </button>
                </div>
                <div class="pane-body">
                  {#if logs.length === 0}
                    <div class="empty-feed">Waiting for system signals...</div>
                  {:else}
                    {#each logs as log}
                      <div class="log-line-v3 {log.level.toLowerCase()}">
                        <span class="log-ts">[{log.timestamp}]</span>
                        <span class="log-lvl">{log.level}</span>
                        <span class="log-msg">{log.message}</span>
                      </div>
                    {/each}
                  {/if}
                </div>
              </div>

              <div class="controls-pane-v3">
                <div class="v3-side-card">
                  <div class="card-title">
                    <span class="material-icons">security</span>
                    <h4>Core Config</h4>
                  </div>
                  <div class="card-list">
                    <div class="v3-toggle-item">
                      <div class="info">
                        <span>Debug logs</span>
                        <p>Verbose diagnostics</p>
                      </div>
                      <label class="v3-switch-mini">
                        <input type="checkbox" checked={true} />
                        <span class="slider-mini"></span>
                      </label>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </section>
        {/if}
      </div>
    </main>
  </div>
</div>

<style>
  .settings-viewport {
    height: 100%;
    overflow: hidden;
    background: #06080b;
  }

  .settings-layout-v3 {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .settings-sub-header {
    background: rgba(10, 12, 18, 0.8);
    backdrop-filter: blur(20px);
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    padding: 0 2rem;
    z-index: 100;
    flex-shrink: 0;
  }

  .header-content {
    max-width: 1400px;
    margin: 0 auto;
    height: 80px;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .header-brand {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .header-brand .material-icons {
    font-size: 2rem;
    color: var(--color-primary);
    filter: drop-shadow(0 0 8px rgba(0, 243, 255, 0.3));
  }

  .brand-text h2 {
    font-size: 0.9rem;
    font-weight: 900;
    letter-spacing: 0.1em;
    color: #fff;
    margin: 0;
  }

  .status-badge {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.6rem;
    font-weight: 800;
    color: var(--color-primary);
    opacity: 0.8;
  }

  .status-badge .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-primary);
  }

  .settings-tabs-v3 {
    display: flex;
    gap: 0.5rem;
    height: 100%;
    align-items: center;
  }

  .tab-btn-v3 {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1.25rem;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 12px;
    color: var(--text-muted);
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    font-weight: 700;
    font-size: 0.85rem;
  }

  .tab-btn-v3:hover {
    color: #fff;
    background: rgba(255, 255, 255, 0.03);
  }

  .tab-btn-v3.active {
    color: var(--color-primary);
    background: rgba(0, 243, 255, 0.08);
    border-color: rgba(0, 243, 255, 0.2);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.2);
  }

  .settings-main-v3 {
    flex: 1;
    overflow-y: auto;
    padding: 2rem;
    background: radial-gradient(
      circle at 50% 0%,
      rgba(0, 243, 255, 0.03) 0%,
      transparent 70%
    );
  }

  .content-container-v3 {
    max-width: 1400px;
    margin: 0 auto;
    min-height: 100%;
  }

  .settings-section {
    animation: slideUp 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(15px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .section-v3-title {
    margin-bottom: 2.5rem;
  }

  .section-v3-title h3 {
    font-size: 1.75rem;
    font-weight: 800;
    color: #fff;
    margin: 0 0 0.5rem 0;
  }

  .section-v3-title p {
    color: var(--text-secondary);
    font-size: 1rem;
    margin: 0;
  }

  .accounts-grid-v3 {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: 1.5rem;
  }

  .add-account-card-v3 {
    background: rgba(255, 255, 255, 0.02);
    border: 2px dashed rgba(255, 255, 255, 0.1);
    border-radius: 24px;
    padding: 3rem;
    cursor: pointer;
    transition: all 0.3s;
    color: var(--text-muted);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .add-account-card-v3:hover {
    border-color: var(--color-primary);
    background: rgba(0, 243, 255, 0.05);
    color: var(--color-primary);
    transform: translateY(-4px);
  }

  .premium-config-card {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 24px;
    padding: 2.5rem;
    display: flex;
    flex-direction: column;
    gap: 2rem;
  }

  .input-v3-group {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .input-v3-group label {
    font-size: 0.7rem;
    font-weight: 900;
    color: var(--text-muted);
    letter-spacing: 0.1em;
  }

  .input-v3-box {
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    height: 54px;
    display: flex;
    align-items: center;
    padding: 0 1.25rem;
  }

  .input-v3-box input {
    background: transparent;
    border: none;
    color: #fff;
    flex: 1;
    padding-left: 1rem;
    outline: none;
    font-family: var(--font-mono);
    font-size: 0.85rem;
  }

  .input-v3-box.readonly {
    background: rgba(0, 0, 0, 0.5);
    border-color: rgba(255, 255, 255, 0.05);
  }

  .input-v3-box.readonly input {
    color: rgba(255, 255, 255, 0.7);
    cursor: default;
  }

  .icon-action {
    background: transparent;
    border: none;
    color: var(--text-muted);
    width: 36px;
    height: 36px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.2s;
    margin-left: 0.25rem;
  }

  .icon-action:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--color-primary);
  }

  .icon-action .material-icons {
    font-size: 1.1rem;
  }

  .card-action-v3 {
    display: flex;
    justify-content: flex-end;
    padding-top: 0.5rem;
  }

  .btn-apply-v3 {
    background: var(--color-primary);
    color: #000;
    border: none;
    border-radius: 14px;
    height: 52px;
    padding: 0 2rem;
    font-weight: 900;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    font-size: 0.8rem;
    transition: all 0.3s;
    box-shadow: 0 4px 15px rgba(0, 243, 255, 0.3);
  }

  .btn-apply-v3:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(0, 243, 255, 0.4);
    filter: brightness(1.1);
  }

  .btn-secondary-v3 {
    background: transparent;
    color: #fff;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 14px;
    height: 52px;
    padding: 0 2rem;
    font-weight: 800;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    font-size: 0.8rem;
    transition: all 0.3s;
  }

  .btn-secondary-v3:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: var(--color-primary);
    color: var(--color-primary);
    transform: translateY(-2px);
  }

  .btn-secondary-v3 .material-icons {
    font-size: 1.2rem;
  }

  .arr-grid-v3 {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(420px, 1fr));
    gap: 2rem;
  }

  .arr-node-v3 {
    background: rgba(10, 12, 18, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 24px;
    padding: 2rem;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    position: relative;
    overflow: hidden;
  }

  .arr-node-v3::before {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 3px;
    background: linear-gradient(
      90deg,
      transparent,
      var(--brand-color),
      transparent
    );
    opacity: 0;
    transition: opacity 0.3s;
  }

  .arr-node-v3:hover {
    border-color: rgba(255, 255, 255, 0.15);
    background: rgba(12, 14, 20, 0.8);
    transform: translateY(-4px);
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.3);
  }

  .arr-node-v3:hover::before {
    opacity: 1;
  }

  .arr-node-v3.sonarr {
    --brand-color: #00aaff;
  }

  .arr-node-v3.radarr {
    --brand-color: #ffc230;
  }

  .node-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 2rem;
    padding-bottom: 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .brand-logo {
    width: 48px;
    height: 48px;
    object-fit: contain;
    padding: 6px;
    border-radius: 12px;
    position: relative;
    z-index: 1;
    filter: drop-shadow(0 0 1px rgba(255, 255, 255, 0.5))
      drop-shadow(0 0 8px rgba(0, 243, 255, 0.3));
    transition: all 0.3s ease;
  }

  .arr-node-v3:hover .brand-logo {
    transform: scale(1.1);
    filter: drop-shadow(0 0 2px rgba(255, 255, 255, 0.7))
      drop-shadow(0 0 15px rgba(0, 243, 255, 0.5));
  }

  .brand span:first-child {
    width: 48px;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 12px;
    color: #fff;
    font-size: 1.5rem;
    transition: all 0.3s;
  }

  .arr-node-v3:hover .brand span:first-child {
    transform: scale(1.1);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
  }

  /* Legacy span styles - kept for fallback but images now used */
  .sonarr .brand span:first-child,
  .radarr .brand span:first-child {
    background: transparent;
    box-shadow: none;
  }

  .brand span:last-child {
    font-weight: 900;
    letter-spacing: 0.1em;
    font-size: 1.1rem;
    color: #fff;
  }

  .node-body {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    margin-bottom: 2rem;
  }

  .node-field {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .node-field label {
    font-size: 0.65rem;
    font-weight: 900;
    color: var(--text-muted);
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  .node-field input,
  .pass-box input {
    width: 100%;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    height: 48px;
    padding: 0 1rem;
    color: #fff;
    font-family: var(--font-mono);
    font-size: 0.85rem;
    outline: none;
    transition: all 0.2s;
  }

  .node-field input:focus,
  .pass-box input:focus {
    border-color: var(--color-primary);
    background: rgba(0, 0, 0, 0.6);
    box-shadow: 0 0 0 3px rgba(0, 243, 255, 0.1);
  }

  .node-field input::placeholder,
  .pass-box input::placeholder {
    color: rgba(255, 255, 255, 0.3);
  }

  .pass-box {
    position: relative;
  }

  .pass-box button,
  .visibility-toggle {
    position: absolute;
    right: 0.75rem;
    top: 50%;
    transform: translateY(-50%);
    background: rgba(255, 255, 255, 0.05);
    border: none;
    color: var(--text-muted);
    width: 32px;
    height: 32px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.2s;
  }

  .pass-box button:hover,
  .visibility-toggle:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--color-primary);
  }

  .node-toggle {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: rgba(255, 255, 255, 0.03);
    padding: 1rem 1.25rem;
    border-radius: 12px;
    transition: background 0.2s;
  }

  .node-toggle:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .node-toggle .txt span {
    font-weight: 700;
    color: #fff;
    font-size: 0.85rem;
    display: block;
    margin-bottom: 0.25rem;
  }

  .node-toggle .txt small {
    font-size: 0.7rem;
    color: var(--text-muted);
  }

  .v3-switch-mini {
    width: 44px;
    height: 24px;
    position: relative;
    display: inline-block;
    cursor: pointer;
  }

  .v3-switch-mini input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .slider-mini {
    position: absolute;
    inset: 0;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 24px;
    cursor: pointer;
    transition: all 0.3s;
  }

  .slider-mini:before {
    content: "";
    position: absolute;
    height: 18px;
    width: 18px;
    left: 3px;
    bottom: 3px;
    background: #fff;
    border-radius: 50%;
    transition: all 0.3s;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  }

  .v3-switch-mini input:checked + .slider-mini {
    background: var(--color-primary);
  }

  .v3-switch-mini input:checked + .slider-mini:before {
    transform: translateX(20px);
    background: #000;
  }

  .node-actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }

  /* Premium Range V3 */
  .range-v3-wrapper {
    position: relative;
    padding: 1.5rem 0 0.5rem 0;
    margin-top: 1rem;
    --thumb-size: 48px;
  }

  /* Value display overlay */
  .range-v3-wrapper::after {
    content: var(--current-val);
    position: absolute;
    top: 2.22rem;
    left: calc(var(--slider-val) * (100% - 48px) / 100% + 24px);
    transform: translate(-50%, -50%);
    color: #000;
    font-weight: 900;
    font-size: 0.9rem;
    font-family: var(--font-mono);
    pointer-events: none;
    z-index: 10;
    transition: left 0.15s ease-out;
  }

  .v3-track-dots {
    position: absolute;
    top: 2.22rem;
    left: 0;
    right: 0;
    height: 6px;
    display: flex;
    justify-content: space-between;
    padding: 0 calc(var(--thumb-size) / 2);
    pointer-events: none;
  }

  .v3-track-dots .dot {
    width: 4px;
    height: 4px;
    background: rgba(255, 255, 255, 0.2);
    border-radius: 50%;
    margin-top: 1px;
    transition: all 0.3s;
  }

  .v3-track-dots .dot.active {
    background: #000;
    box-shadow: 0 0 5px var(--color-primary);
    z-index: 3;
    background: var(--color-primary);
  }

  input[type="range"] {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    background: transparent;
    position: relative;
    z-index: 5;
    margin: 0;
  }

  input[type="range"]:focus {
    outline: none;
  }

  /* Track */
  input[type="range"]::-webkit-slider-runnable-track {
    width: 100%;
    height: 6px;
    background: linear-gradient(
      to right,
      var(--color-primary) 0%,
      var(--color-primary) var(--slider-val),
      rgba(255, 255, 255, 0.1) var(--slider-val),
      rgba(255, 255, 255, 0.1) 100%
    );
    border-radius: 3px;
    transition: all 0.1s;
  }

  /* Thumb */
  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    height: var(--thumb-size);
    width: var(--thumb-size);
    border-radius: 50%;
    background: var(--color-primary);
    cursor: pointer;
    margin-top: calc(-1 * var(--thumb-size) / 2 + 3px);
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.6);
    transition: transform 0.2s;
    border: 4px solid rgba(0, 0, 0, 0.3);
  }

  input[type="range"]:active::-webkit-slider-thumb {
    transform: scale(1.1);
  }

  .marks-premium {
    display: flex;
    justify-content: space-between;
    margin-top: 1.25rem;
    font-family: var(--font-mono);
    font-size: 0.65rem;
    color: var(--text-muted);
    padding: 0 5px;
  }

  .marks-premium span {
    position: relative;
    width: 20px;
    text-align: center;
  }

  /* Correct alignment for '5' which is at 4/9 position */
  .marks-premium span:nth-child(2) {
    position: absolute;
    left: 44.44%;
    transform: translateX(-50%);
  }

  .btn-test,
  .btn-save {
    height: 46px;
    border-radius: 12px;
    font-weight: 800;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    transition: all 0.2s;
  }

  .btn-save {
    background: var(--color-primary);
    border: none;
    color: #000;
  }

  .btn-save:hover {
    filter: brightness(1.1);
    transform: translateY(-2px);
  }

  .btn-test {
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #fff;
  }

  .btn-test:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .system-v3-layout {
    display: grid;
    grid-template-columns: 1fr 320px;
    gap: 2rem;
    height: 600px;
  }

  .terminal-pane-v3 {
    background: #05070a;
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 20px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .pane-header {
    background: rgba(255, 255, 255, 0.02);
    padding: 1rem 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    display: flex;
    justify-content: space-between;
  }

  .pane-body {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem;
    font-family: monospace;
    font-size: 0.75rem;
  }

  .log-line-v3 {
    display: flex;
    gap: 0.75rem;
    margin-bottom: 0.25rem;
  }
  .log-ts {
    color: var(--text-muted);
  }
  .log-lvl {
    font-weight: bold;
  }
  .log-line-v3.info .log-lvl {
    color: #00aaff;
  }
  .log-line-v3.error .log-lvl {
    color: #ff5252;
  }

  .hybrid-switch {
    width: 54px;
    height: 28px;
    position: relative;
    cursor: pointer;
  }
  .hybrid-switch input {
    opacity: 0;
    width: 0;
    height: 0;
  }
  .switch-ui {
    position: absolute;
    inset: 0;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 30px;
  }
  .switch-ui:before {
    content: "";
    position: absolute;
    height: 22px;
    width: 22px;
    left: 3px;
    bottom: 3px;
    background: #fff;
    border-radius: 50%;
    transition: 0.4s;
  }
  input:checked + .switch-ui {
    background: var(--color-primary);
  }
  input:checked + .switch-ui:before {
    transform: translateX(26px);
    background: #000;
  }

  .rotating {
    animation: rot 1s linear infinite;
  }
  @keyframes rot {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  .dot-pulse {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--color-primary);
    animation: dPulse 2s infinite;
  }
  @keyframes dPulse {
    0% {
      box-shadow: 0 0 0 0 rgba(0, 243, 255, 0.4);
    }
    70% {
      box-shadow: 0 0 0 10px rgba(0, 243, 255, 0);
    }
    100% {
      box-shadow: 0 0 0 0 rgba(0, 243, 255, 0);
    }
  }

  @media (max-width: 1024px) {
    .system-v3-layout {
      grid-template-columns: 1fr;
    }
    .arr-grid-v3 {
      grid-template-columns: 1fr;
    }
  }
</style>
