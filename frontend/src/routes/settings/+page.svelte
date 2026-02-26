<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import SwitchAccountModal from "$lib/components/SwitchAccountModal.svelte";
  import Badge from "$lib/components/ui/Badge.svelte";
  import { get } from "svelte/store";
  import { goto } from "$app/navigation";
  import { accountStore } from "$lib/stores/account.svelte";
  import {
    systemStore,
    systemLogs,
    downloadSettings,
    indexerSettings,
    sonarrSettings,
    radarrSettings,
    folderSourceUrl,
  } from "$lib/stores/system";
  import { toasts } from "$lib/stores/toasts";
  import { IdentityCard, Button } from "$lib/components";

  // UI State
  let activeCategory: "services" | "system" = $state("services");
  let endpointTab: "newznab" | "sabnzbd" = $state("newznab");
  let showSonarrApiKey = $state(false);
  let showRadarrApiKey = $state(false);
  let showApiKey = $state(false);
  let sonarrTesting = $state(false);
  let radarrTesting = $state(false);
  let sonarrConnected = $state(false);
  let radarrConnected = $state(false);
  let logInterval: any;

  // Switch account form state
  let showSwitchForm = $state(false);
  let switchEmail = $state("");
  let switchPassword = $state("");
  let switchLoading = $state(false);
  let showSwitchModal = $state(false);
  let showSwitchPassword = $state(false);

  // Radial HUD state
  let activeSector: string | null = $state(null);

  function selectSector(name: string) {
    activeSector = activeSector === name ? null : name;
  }

  // Derived VIP status
  let isVip = $derived(accountStore.isVip);

  // Local state for editing (bound to inputs)
  let concurrency = $state(3);
  let threads = $state(4);
  let downloadPath = $state("");
  let folderSourceGistUrl = $state("");
  let folderCacheRefreshing = $state(false);

  /** Custom drag handler for the concurrency hex slider */
  function onSliderPointerDown(e: PointerEvent) {
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    updateConcurrencyFromPointer(e, el);
  }
  function onSliderPointerMove(e: PointerEvent) {
    if (!(e.currentTarget as HTMLElement).hasPointerCapture(e.pointerId))
      return;
    updateConcurrencyFromPointer(e, e.currentTarget as HTMLElement);
  }
  function updateConcurrencyFromPointer(e: PointerEvent, el: HTMLElement) {
    const rect = el.getBoundingClientRect();
    const pct = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
    concurrency = Math.round(pct * 9) + 1;
  }

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

  // Logs - use manual subscription to avoid reactive re-renders
  let logs = $state<LogEntry[]>([]);

  // Import LogEntry type
  import type { LogEntry } from "$lib/stores/system";

  onMount(() => {
    // Fetch account info
    accountStore.fetch();

    // Fetch all settings once on mount and initialize form state
    (async () => {
      await Promise.all([
        systemStore.fetchDownloadSettings(),
        systemStore.fetchIndexerSettings(),
        systemStore.fetchSonarrSettings(),
        systemStore.fetchRadarrSettings(),
        systemStore.fetchLogs(),
        systemStore.fetchFolderSourceConfig(),
      ]);

      // Initialize local form state from stores (one-time, non-reactive read)
      const dlSettings = get(downloadSettings);
      concurrency = dlSettings.max_concurrent;
      threads = dlSettings.segments_per_download;
      downloadPath = dlSettings.directory;

      const fsUrl = get(folderSourceUrl);
      folderSourceGistUrl = fsUrl;

      const idxSettings = get(indexerSettings);
      indexerApiKey = idxSettings.api_key;
      indexerUrl =
        idxSettings.indexer_url || "http://flasharr:8484/api/indexer";

      const snrSettings = get(sonarrSettings);
      sonarrEnabled = snrSettings.enabled;
      sonarrUrl = snrSettings.url;
      sonarrApiKey = snrSettings.api_key;
      sonarrAutoImport = snrSettings.auto_import;

      const rdrSettings = get(radarrSettings);
      radarrEnabled = rdrSettings.enabled;
      radarrUrl = rdrSettings.url;
      radarrApiKey = rdrSettings.api_key;
      radarrAutoImport = rdrSettings.auto_import;
    })();

    // Subscribe to logs manually (doesn't cause component re-render)
    const unsubscribeLogs = systemLogs.subscribe((value) => {
      logs = value;
    });

    // Re-enable log polling - now it won't cause form resets!
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

    // Also save folder source URL
    if (folderSourceGistUrl.trim()) {
      await systemStore.saveFolderSourceConfig(folderSourceGistUrl.trim());
    } else {
      // Clear the saved URL if input is empty
      await systemStore.saveFolderSourceConfig("");
    }

    if (result.success) {
      toasts.success(result.message || "Engine configuration saved");
    } else {
      toasts.error(result.message || "Failed to save configuration");
    }
  }

  async function refreshFolderCache() {
    if (folderCacheRefreshing) return;

    // Validate the URL before triggering sync
    const url = folderSourceGistUrl.trim();
    if (!url) {
      toasts.error("Folder source URL is empty — enter a URL first");
      return;
    }

    // Validate URL format
    try {
      const parsed = new URL(url);
      if (!parsed.protocol.startsWith("http")) {
        toasts.error("Folder source must be an HTTP/HTTPS URL");
        return;
      }
    } catch {
      toasts.error("Invalid URL format — please check the folder source link");
      return;
    }

    folderCacheRefreshing = true;
    try {
      // Save the URL first
      await systemStore.saveFolderSourceConfig(url);
      // Trigger backend sync
      const resp = await fetch("/api/folder-source/sync", { method: "POST" });
      if (resp.ok) {
        toasts.success("Folder cache sync started — refreshing in background");
      } else {
        toasts.error("Failed to trigger folder cache sync");
      }
    } catch (err) {
      toasts.error("Network error — could not reach server");
    } finally {
      folderCacheRefreshing = false;
    }
  }

  async function clearLogs() {
    systemStore.clearLogs();
    toasts.success("Terminal logs cleared");
  }

  async function generateApiKey() {
    const newKey = await systemStore.generateIndexerApiKey();
    if (newKey) {
      indexerApiKey = newKey; // Update local state
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
      sonarrConnected = true;
    } else {
      toasts.error(result.message || "Sonarr connection failed");
      sonarrConnected = false;
    }
    sonarrTesting = false;
  }

  $effect(() => {
    sonarrUrl;
    sonarrApiKey;
    sonarrConnected = false;
  });
  $effect(() => {
    radarrUrl;
    radarrApiKey;
    radarrConnected = false;
  });

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
      radarrConnected = true;
    } else {
      toasts.error(result.message || "Radarr connection failed");
      radarrConnected = false;
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

  async function switchAccount() {
    if (!switchEmail.trim() || !switchPassword.trim()) {
      toasts.error("Please enter both email and password");
      return;
    }
    switchLoading = true;
    try {
      const ok = await accountStore.switchAccount(
        switchEmail.trim(),
        switchPassword.trim(),
      );
      if (ok) {
        toasts.success("Account switched and activated!");
        switchEmail = "";
        switchPassword = "";
        showSwitchForm = false;
      } else {
        toasts.error("Failed to switch account — check your credentials");
      }
    } finally {
      switchLoading = false;
    }
  }
</script>

<svelte:head>
  <title>Settings - Flasharr</title>
</svelte:head>

<div class="settings-viewport">
  <div class="settings-two-col">
    <!-- ═══════════ LEFT: Widget Grid (3 columns) ═══════════ -->
    <div class="widget-grid">
      <!-- Column 1: Account + Download Engine (1:1.5) -->
      <div class="widget-col col-first">
        <!-- Account -->
        <section class="bento-card bento-account">
          <div class="bento-head red-accent">
            <span class="material-icons">person</span>
            <h3>ACCOUNT INFO</h3>
          </div>
          <div class="acc-hero-banner">
            <div class="acc-dots-overlay"></div>
            <div class="acc-logo-ring">
              <img src="/images/logo_fshare.png" alt="Fshare" />
            </div>
          </div>
          <!-- Info -->
          <div class="acc-card-info">
            <div class="acc-info-left">
              <span class="acc-card-email"
                >{accountStore.listFormatted.length > 0
                  ? accountStore.listFormatted[0]?.email
                  : "No account"}</span
              >
              <span class="acc-card-expiry"
                >{accountStore.listFormatted.length > 0
                  ? `Expires ${accountStore.listFormatted[0]?.expiry || "—"}`
                  : "—"}</span
              >
            </div>
            <Badge
              text={isVip ? "VIP" : "FREE"}
              variant={isVip ? "vip" : "free"}
              size="sm"
            />
          </div>
          <div class="acc-card-actions">
            <!-- Refresh: fixed 36px square -->
            <Button
              variant="ghost"
              size="sm"
              icon="refresh"
              width="36px"
              onclick={() => {
                if (accountStore.listFormatted[0]?.email) {
                  accountStore.refresh(accountStore.listFormatted[0].email);
                  toasts.success("Refreshing...");
                }
              }}
              title="Refresh"
            />
            <!-- Switch Account: fills all remaining space -->
            <div style="flex:1; display:flex;">
              <Button
                accent="#dc3c3c"
                icon="swap_horiz"
                size="sm"
                width="100%"
                onclick={() => (showSwitchModal = true)}>Switch Account</Button
              >
            </div>
          </div>
        </section>

        <!-- R1C2: Download Engine -->
        <section class="bento-card bento-download">
          <div class="bento-head cyan-accent">
            <span class="material-icons">speed</span>
            <h3>DOWNLOAD ENGINE</h3>
          </div>
          <div class="card-banner cyan-banner">
            <div class="banner-dots"></div>
            <div class="banner-logo-ring glow">
              <img src="/images/flasharr_logo.png" alt="Flasharr" />
            </div>
          </div>
          <!-- Download Engine Body -->
          <div class="bento-body dl-body">
            <!-- PATH section -->
            <div class="dl-section">
              <div class="dl-path-field">
                <div class="dl-path-label">
                  <span class="material-icons">folder_open</span>
                  <span>PATH</span>
                </div>
                <input
                  type="text"
                  id="b-dl-path"
                  bind:value={downloadPath}
                  placeholder="/media/downloads"
                  class="dl-path-input"
                />
              </div>
            </div>

            <!-- Folder Source URL section -->
            <div class="dl-section dl-section--sep">
              <div class="dl-path-field">
                <div class="dl-path-label">
                  <span class="material-icons">link</span>
                  <span>FOLDER SOURCE</span>
                </div>
                <div class="dl-folder-source-row">
                  <input
                    type="text"
                    id="b-folder-source"
                    bind:value={folderSourceGistUrl}
                    placeholder="https://gist.githubusercontent.com/.../fshare_folder_list.txt"
                    class="dl-path-input"
                  />
                  <button
                    class="dl-refresh-btn"
                    class:spinning={folderCacheRefreshing}
                    onclick={refreshFolderCache}
                    disabled={folderCacheRefreshing}
                    title="Refresh folder cache"
                  >
                    <span class="material-icons"
                      >{folderCacheRefreshing
                        ? "hourglass_empty"
                        : "sync"}</span
                    >
                  </button>
                </div>
              </div>
            </div>

            <!-- Concurrency section -->
            <div class="dl-section dl-section--conc dl-section--sep">
              <div class="dl-conc-hud">
                <div class="dl-conc-header">
                  <span class="dl-conc-label">CONCURRENCY</span>
                </div>
                <div
                  class="dl-dot-slider"
                  style="--pct: {(concurrency - 1) / 9};"
                  role="slider"
                  aria-label="Concurrency"
                  aria-valuemin="1"
                  aria-valuemax="10"
                  aria-valuenow={concurrency}
                  tabindex="0"
                  onpointerdown={onSliderPointerDown}
                  onpointermove={onSliderPointerMove}
                  onkeydown={(e) => {
                    if (e.key === "ArrowRight" || e.key === "ArrowUp")
                      concurrency = Math.min(10, concurrency + 1);
                    if (e.key === "ArrowLeft" || e.key === "ArrowDown")
                      concurrency = Math.max(1, concurrency - 1);
                  }}
                >
                  <div class="dl-dots" aria-hidden="true">
                    {#each Array.from({ length: 10 }) as _, i}
                      <div
                        class="dl-dot"
                        class:lit={i < concurrency}
                        class:hidden={i === concurrency - 1}
                        style="left: calc({i / 9} * (100% - 40px) + 16px);"
                      ></div>
                    {/each}
                  </div>
                  <div class="dl-thumb-orb" aria-hidden="true">
                    {concurrency}
                  </div>
                </div>
              </div>
            </div>

            <div class="compact-actions">
              <div style="flex:1; display:flex;">
                <Button
                  accent="#00f3ff"
                  icon="save"
                  size="sm"
                  width="100%"
                  onclick={saveEngineConfig}>APPLY</Button
                >
              </div>
            </div>
          </div>
        </section>
      </div>

      <!-- Column 2: Newznab + Sonarr (1:1) -->
      <div class="widget-col">
        <!-- Newznab -->
        <section class="bento-card bento-newznab">
          <div class="bento-head violet-accent">
            <img src="/images/newznab-logo.png" alt="" class="head-logo" />
            <h3>NEWZNAB</h3>
          </div>
          <div class="card-banner violet-banner">
            <div class="banner-dots"></div>
            <div class="banner-logo-ring glow">
              <img src="/images/newznab-logo.png" alt="Newznab" />
            </div>
          </div>
          <div class="bento-body">
            <div class="inline-field">
              <span class="inline-label">URL</span><span class="inline-value"
                >http://flasharr:8484/newznab/api</span
              ><button
                class="copy-inline-btn"
                onclick={() =>
                  copyToClipboard(
                    "http://flasharr:8484/newznab/api",
                    "Endpoint",
                  )}><span class="material-icons">content_copy</span></button
              >
            </div>
            <div class="inline-field">
              <span class="inline-label">USER</span><span class="inline-value"
                >flasharr</span
              ><button
                class="copy-inline-btn"
                onclick={() => copyToClipboard("flasharr", "Username")}
                ><span class="material-icons">content_copy</span></button
              >
            </div>
            <div class="inline-field">
              <span class="inline-label">PASS</span><span class="inline-value"
                >flasharr-pwd</span
              ><button
                class="copy-inline-btn"
                onclick={() => copyToClipboard("flasharr-pwd", "Password")}
                ><span class="material-icons">content_copy</span></button
              >
            </div>
            <div class="inline-field">
              <span class="inline-label">KEY</span><span
                class="inline-value mono"
                >{showApiKey ? indexerApiKey : "•".repeat(12)}</span
              ><button
                class="copy-inline-btn"
                onclick={() => (showApiKey = !showApiKey)}
                ><span class="material-icons"
                  >{showApiKey ? "visibility_off" : "visibility"}</span
                ></button
              ><button
                class="copy-inline-btn"
                onclick={() => copyToClipboard(indexerApiKey, "API Key")}
                ><span class="material-icons">content_copy</span></button
              >
            </div>
          </div>
          <div class="acc-card-actions">
            <div style="flex:1; display:flex;">
              <Button
                accent="#8b5cf6"
                icon="autorenew"
                size="sm"
                width="100%"
                onclick={generateApiKey}>Regen Key</Button
              >
            </div>
          </div>
        </section>

        <!-- R2C1: Sonarr -->
        <section class="bento-card bento-sonarr">
          <div class="bento-head sky-accent">
            <img
              src="https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/sonarr.png"
              alt=""
              class="head-logo"
            />
            <h3>SONARR</h3>
          </div>
          <div class="card-banner sky-banner">
            <div class="banner-dots"></div>
            <div class="banner-logo-ring glow">
              <img
                src="https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/sonarr.png"
                alt="Sonarr"
              />
              <span
                class="banner-status-dot"
                class:active={sonarrConnected}
                title={sonarrConnected ? "Connected" : "Not tested"}
              ></span>
            </div>
          </div>
          <div class="bento-body">
            <!-- URL field -->
            <div class="inline-field">
              <span class="inline-label">URL</span>
              <input
                type="text"
                id="b-sn-url"
                bind:value={sonarrUrl}
                placeholder="http://localhost:8989"
                class="inline-input-edit"
              />
            </div>
            <!-- KEY field with visibility toggle -->
            <div class="inline-field">
              <span class="inline-label">KEY</span>
              <input
                type={showSonarrApiKey ? "text" : "password"}
                id="b-sn-key"
                bind:value={sonarrApiKey}
                placeholder="API Key"
                class="inline-input-edit mono"
              />
              <button
                class="copy-inline-btn"
                onclick={() => (showSonarrApiKey = !showSonarrApiKey)}
                ><span class="material-icons"
                  >{showSonarrApiKey ? "visibility_off" : "visibility"}</span
                ></button
              >
            </div>
            <div class="compact-actions">
              <div style="flex:4; display:flex;">
                <Button
                  variant="ghost"
                  size="sm"
                  width="100%"
                  disabled={sonarrTesting}
                  onclick={testSonarrConnection}
                >
                  <span class="material-icons" class:rotating={sonarrTesting}
                    >{sonarrTesting ? "refresh" : "sync_alt"}</span
                  > TEST</Button
                >
              </div>
              <div style="flex:6; display:flex;">
                <Button
                  accent="#38bdf8"
                  size="sm"
                  width="100%"
                  onclick={saveSonarrSettings}>SAVE</Button
                >
              </div>
            </div>
          </div>
        </section>
      </div>

      <!-- Column 3: SABnzbd + Radarr (1:1) -->
      <div class="widget-col">
        <!-- R2C3: SABnzbd -->
        <section class="bento-card bento-sabnzbd">
          <div class="bento-head teal-accent">
            <img
              src="https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/sabnzbd.png"
              alt=""
              class="head-logo"
            />
            <h3>SABNZBD</h3>
          </div>
          <div class="card-banner teal-banner">
            <div class="banner-dots"></div>
            <div class="banner-logo-ring glow">
              <img
                src="https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/sabnzbd.png"
                alt="SABnzbd"
              />
            </div>
          </div>
          <div class="bento-body">
            <div class="inline-field">
              <span class="inline-label">URL</span><span class="inline-value"
                >http://flasharr:8484/sabnzbd/api</span
              ><button
                class="copy-inline-btn"
                onclick={() =>
                  copyToClipboard(
                    "http://flasharr:8484/sabnzbd/api",
                    "Endpoint",
                  )}><span class="material-icons">content_copy</span></button
              >
            </div>
            <div class="inline-field">
              <span class="inline-label">USER</span><span class="inline-value"
                >flasharr</span
              ><button
                class="copy-inline-btn"
                onclick={() => copyToClipboard("flasharr", "Username")}
                ><span class="material-icons">content_copy</span></button
              >
            </div>
            <div class="inline-field">
              <span class="inline-label">PASS</span><span class="inline-value"
                >flasharr-pwd</span
              ><button
                class="copy-inline-btn"
                onclick={() => copyToClipboard("flasharr-pwd", "Password")}
                ><span class="material-icons">content_copy</span></button
              >
            </div>
            <div class="inline-field">
              <span class="inline-label">KEY</span><span
                class="inline-value mono"
                >{showApiKey ? indexerApiKey : "•".repeat(12)}</span
              ><button
                class="copy-inline-btn"
                onclick={() => (showApiKey = !showApiKey)}
                ><span class="material-icons"
                  >{showApiKey ? "visibility_off" : "visibility"}</span
                ></button
              ><button
                class="copy-inline-btn"
                onclick={() => copyToClipboard(indexerApiKey, "API Key")}
                ><span class="material-icons">content_copy</span></button
              >
            </div>
          </div>
          <div class="acc-card-actions">
            <div style="flex:1; display:flex;">
              <Button
                accent="#00d4aa"
                icon="autorenew"
                size="sm"
                width="100%"
                onclick={generateApiKey}>Regen Key</Button
              >
            </div>
          </div>
        </section>

        <!-- Radarr -->
        <section class="bento-card bento-radarr">
          <div class="bento-head amber-accent">
            <img
              src="https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/radarr.png"
              alt=""
              class="head-logo"
            />
            <h3>RADARR</h3>
          </div>
          <div class="card-banner amber-banner">
            <div class="banner-dots"></div>
            <div class="banner-logo-ring glow">
              <img
                src="https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/radarr.png"
                alt="Radarr"
              />
              <span
                class="banner-status-dot"
                class:active={radarrConnected}
                title={radarrConnected ? "Connected" : "Not tested"}
              ></span>
            </div>
          </div>
          <div class="bento-body">
            <!-- URL field -->
            <div class="inline-field">
              <span class="inline-label">URL</span>
              <input
                type="text"
                id="b-rd-url"
                bind:value={radarrUrl}
                placeholder="http://localhost:7878"
                class="inline-input-edit"
              />
            </div>
            <!-- KEY field with visibility toggle -->
            <div class="inline-field">
              <span class="inline-label">KEY</span>
              <input
                type={showRadarrApiKey ? "text" : "password"}
                id="b-rd-key"
                bind:value={radarrApiKey}
                placeholder="API Key"
                class="inline-input-edit mono"
              />
              <button
                class="copy-inline-btn"
                onclick={() => (showRadarrApiKey = !showRadarrApiKey)}
                ><span class="material-icons"
                  >{showRadarrApiKey ? "visibility_off" : "visibility"}</span
                ></button
              >
            </div>
            <div class="compact-actions">
              <div style="flex:4; display:flex;">
                <Button
                  variant="ghost"
                  size="sm"
                  width="100%"
                  disabled={radarrTesting}
                  onclick={testRadarrConnection}
                >
                  <span class="material-icons" class:rotating={radarrTesting}
                    >{radarrTesting ? "refresh" : "sync_alt"}</span
                  > TEST</Button
                >
              </div>
              <div style="flex:6; display:flex;">
                <Button
                  accent="#f59e0b"
                  size="sm"
                  width="100%"
                  onclick={saveRadarrSettings}>SAVE</Button
                >
              </div>
            </div>
          </div>
        </section>
      </div>
    </div>

    <!-- ═══════════ RIGHT: System Log / Activity Panel ═══════════ -->
    <aside class="log-panel">
      <div class="log-panel-header">
        <div class="log-toggle">
          <button
            class="log-tab"
            class:active={activeCategory === "services"}
            onclick={() => (activeCategory = "services")}
          >
            <span class="material-icons">terminal</span> System Log
          </button>
          <button
            class="log-tab"
            class:active={activeCategory === "system"}
            onclick={() => (activeCategory = "system")}
          >
            <span class="material-icons">history</span> Activity
          </button>
        </div>
        {#if activeCategory === "services"}
          <button class="btn-clear-log" onclick={clearLogs} title="Clear logs">
            <span class="material-icons">delete_sweep</span>
          </button>
        {/if}
      </div>

      <div class="log-panel-body">
        {#if activeCategory === "services"}
          <!-- System Log Mode -->
          {#if logs.length === 0}
            <div class="log-empty">
              <span class="material-icons">hourglass_empty</span> Waiting for system
              signals...
            </div>
          {:else}
            {#each logs as log}
              <div class="log-line {log.level.toLowerCase()}">
                <span class="log-ts">[{log.timestamp}]</span>
                <span class="log-lvl">{log.level}</span>
                <span class="log-msg">{log.message}</span>
              </div>
            {/each}
          {/if}
        {:else}
          <!-- Activity Mode -->
          <div class="activity-feed">
            {#if logs.length === 0}
              <div class="log-empty">
                <span class="material-icons">inbox</span> No recent activity
              </div>
            {:else}
              {#each logs.filter((l) => l.level === "INFO" || l.level === "WARN") as log}
                <div class="activity-item">
                  <div class="activity-icon">
                    <span class="material-icons"
                      >{log.level === "WARN"
                        ? "warning"
                        : log.message.includes("download")
                          ? "cloud_download"
                          : log.message.includes("import")
                            ? "move_to_inbox"
                            : "check_circle"}</span
                    >
                  </div>
                  <div class="activity-content">
                    <span class="activity-msg">{log.message}</span>
                    <span class="activity-time">{log.timestamp}</span>
                  </div>
                </div>
              {/each}
            {/if}
          </div>
        {/if}
      </div>

      <div class="log-panel-footer">
        <span class="dot-pulse"></span>
        <span class="log-status">LIVE · {logs.length} events</span>
      </div>
    </aside>
  </div>
</div>

<SwitchAccountModal
  bind:open={showSwitchModal}
  onclose={() => (showSwitchModal = false)}
/>

<style>
  /* ============================== */
  /* Two-Column Settings Layout      */
  /* ============================== */
  .settings-viewport {
    height: 100%;
    overflow: hidden;
    padding: 1rem;
  }
  .settings-two-col {
    display: grid;
    grid-template-columns: 3fr 1fr;
    gap: 1rem;
    height: 100%;
    animation: slideUp 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  }
  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(12px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  /* ── Left: 3-Column Widget Grid ── */
  .widget-grid {
    display: grid;
    grid-template-columns: minmax(0, 0.8fr) minmax(0, 1.1fr) minmax(0, 1.1fr);
    gap: 0.65rem;
    height: 100%;
    overflow: hidden;
  }
  .widget-col {
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
    min-height: 0;
  }
  .widget-col .bento-card {
    flex: 1;
    min-height: 0;
  }
  .col-first .bento-card:first-child {
    flex: 1;
  }
  .col-first .bento-card:last-child {
    flex: 2;
  }

  .bento-card {
    border-radius: 14px;
    background: rgba(8, 10, 15, 0.7);
    backdrop-filter: blur(12px);
    border: 1px solid
      color-mix(
        in srgb,
        var(--card-accent, rgba(255, 255, 255, 0.05)) 35%,
        transparent
      );
    display: flex;
    flex-direction: column;
    overflow: hidden;
    transition:
      transform 0.25s cubic-bezier(0.16, 1, 0.3, 1),
      border-color 0.25s,
      box-shadow 0.3s;
  }
  .bento-card:hover {
    transform: translateY(-3px);
    border-color: color-mix(
      in srgb,
      var(--card-accent, rgba(255, 255, 255, 0.12)) 80%,
      transparent
    );
    box-shadow:
      0 0 0 1px
        color-mix(in srgb, var(--card-accent, transparent) 40%, transparent),
      0 12px 40px rgba(0, 0, 0, 0.4),
      0 0 32px
        color-mix(in srgb, var(--card-accent, transparent) 20%, transparent);
  }
  /* Per-card accent colors */
  .bento-account {
    --card-accent: rgba(220, 60, 60, 0.5);
  }
  .bento-download {
    --card-accent: rgba(0, 243, 255, 0.4);
  }
  .bento-newznab {
    --card-accent: rgba(139, 92, 246, 0.5);
  }
  .bento-sonarr {
    --card-accent: rgba(56, 189, 248, 0.5);
  }
  .bento-radarr {
    --card-accent: rgba(245, 158, 11, 0.5);
  }
  .bento-sabnzbd {
    --card-accent: rgba(0, 212, 170, 0.5);
  }

  /* ── Card Header ── */
  .bento-head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.55rem 0.7rem;
    border-bottom: none;
    font-family: var(--font-mono, monospace);
    flex-shrink: 0;
    position: relative;
    z-index: 1;
    overflow: hidden;
    background: transparent;
  }
  .bento-head::after {
    content: "";
    position: absolute;
    inset: 0;
    background-image: radial-gradient(rgba(0, 0, 0, 0.25) 1px, transparent 1px);
    background-size: 6px 6px;
    mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 1) 0%,
      rgba(0, 0, 0, 0) 100%
    );
    -webkit-mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 1) 0%,
      rgba(0, 0, 0, 0) 100%
    );
    pointer-events: none;
  }
  .bento-head h3 {
    font-size: 0.75rem;
    font-weight: 800;
    letter-spacing: 0.12em;
    color: #fff;
    margin: 0;
    flex: 1;
  }
  .bento-head .material-icons {
    font-size: 1.2rem;
  }
  .head-logo {
    width: 22px;
    height: 22px;
    border-radius: 4px;
    object-fit: contain;
  }
  .bento-sw {
    margin-left: auto;
  }

  /* ── Accent Colors ── */
  .cyan-accent {
    border-left: 3px solid rgba(0, 243, 255, 0.5);
  }
  .cyan-accent .material-icons {
    color: #00f3ff;
  }
  .cyan-accent::after {
    background-image: radial-gradient(
      rgba(0, 243, 255, 0.25) 1px,
      transparent 1px
    );
    mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 1) 0%,
      rgba(0, 0, 0, 0) 70%
    );
    -webkit-mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 1) 0%,
      rgba(0, 0, 0, 0) 70%
    );
  }

  .red-accent {
    border-left: 3px solid rgba(220, 60, 60, 0.6);
  }
  .red-accent .material-icons {
    color: #dc3c3c;
  }
  .red-accent::after {
    background-image: radial-gradient(
      rgba(220, 60, 60, 0.3) 1px,
      transparent 1px
    );
    mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 1) 0%,
      rgba(0, 0, 0, 0) 70%
    );
    -webkit-mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 1) 0%,
      rgba(0, 0, 0, 0) 70%
    );
  }

  .violet-accent {
    border-left: 3px solid rgba(139, 92, 246, 0.5);
  }
  .violet-accent .material-icons {
    color: #8b5cf6;
  }
  .violet-accent::after {
    background-image: radial-gradient(
      rgba(139, 92, 246, 0.25) 1px,
      transparent 1px
    );
    mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 1) 0%,
      rgba(0, 0, 0, 0) 70%
    );
    -webkit-mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 1) 0%,
      rgba(0, 0, 0, 0) 70%
    );
  }

  .sky-accent {
    border-left: 3px solid rgba(56, 189, 248, 0.5);
  }
  .sky-accent .material-icons {
    color: #38bdf8;
  }
  .sky-accent::after {
    background-image: radial-gradient(
      rgba(56, 189, 248, 0.25) 1px,
      transparent 1px
    );
    mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 1) 0%,
      rgba(0, 0, 0, 0) 70%
    );
    -webkit-mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 1) 0%,
      rgba(0, 0, 0, 0) 70%
    );
  }

  .amber-accent {
    border-left: 3px solid rgba(245, 158, 11, 0.5);
  }
  .amber-accent .material-icons {
    color: #f59e0b;
  }
  .amber-accent::after {
    background-image: radial-gradient(
      rgba(245, 158, 11, 0.25) 1px,
      transparent 1px
    );
    mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 1) 0%,
      rgba(0, 0, 0, 0) 70%
    );
    -webkit-mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 1) 0%,
      rgba(0, 0, 0, 0) 70%
    );
  }

  .teal-accent {
    border-left: 3px solid rgba(0, 212, 170, 0.5);
  }
  .teal-accent .material-icons {
    color: #00d4aa;
  }
  .teal-accent::after {
    background-image: radial-gradient(
      rgba(0, 212, 170, 0.25) 1px,
      transparent 1px
    );
    mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 1) 0%,
      rgba(0, 0, 0, 0) 70%
    );
    -webkit-mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 1) 0%,
      rgba(0, 0, 0, 0) 70%
    );
  }

  /* ── Shared Card Banner ── */
  .card-banner {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    height: 115px;
    box-sizing: border-box;
    flex-shrink: 0;
  }
  .banner-dots {
    position: absolute;
    inset: 0;
    background-image: radial-gradient(rgba(0, 0, 0, 0.25) 1px, transparent 1px);
    background-size: 6px 6px;
    pointer-events: none;
  }
  .banner-logo-ring {
    width: 48px;
    height: 48px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.15);
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.3);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
    position: relative;
    z-index: 1;
  }
  .banner-logo-ring img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .banner-logo-ring .material-icons {
    font-size: 1.4rem;
    color: rgba(255, 255, 255, 0.7);
  }

  /* Glow variant: for transparent-bg logos — no crop, color drop-shadow */
  .banner-logo-ring.glow {
    border-radius: 0;
    border: none;
    overflow: visible;
    background: transparent;
    box-shadow: none;
  }
  .banner-logo-ring.glow img {
    /* Base: overridden per card below */
    width: 44px;
    height: 44px;
    object-fit: contain;
  }
  /* Optical normalization: target ~33px visible content.
     each logo's fill ratio = visible content / canvas size.
     img size = 33px / fill_ratio                            */
  .bento-download .banner-logo-ring.glow img {
    width: 94px;
    height: 94px;
  } /* Flasharr: ~40% fill, +15% */
  .bento-newznab .banner-logo-ring.glow img {
    width: 44px;
    height: 44px;
  } /* Newznab:  ~87% fill, +15% */
  .bento-sonarr .banner-logo-ring.glow img {
    width: 40px;
    height: 40px;
  } /* Sonarr:   ~82% fill */
  .bento-radarr .banner-logo-ring.glow img {
    width: 44px;
    height: 44px;
  } /* Radarr:   ~75% fill */
  .bento-sabnzbd .banner-logo-ring.glow img {
    width: 44px;
    height: 44px;
  } /* SABnzbd:  ~75% fill */

  /* ── Status Dot ── */
  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.2);
    margin-left: auto;
    flex-shrink: 0;
    box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.06);
    transition:
      background 0.3s,
      box-shadow 0.3s;
  }
  .status-dot.active {
    background: #22c55e;
    box-shadow:
      0 0 6px rgba(34, 197, 94, 0.7),
      0 0 0 2px rgba(34, 197, 94, 0.2);
  }

  /* Banner logo status dot — bottom-right overlay */
  .banner-logo-ring {
    position: relative;
  }
  .banner-status-dot {
    position: absolute;
    bottom: -3px;
    right: -3px;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: rgba(80, 80, 90, 0.85);
    border: 2px solid rgba(8, 10, 15, 0.95);
    transition:
      background 0.3s,
      box-shadow 0.3s;
    z-index: 2;
  }
  .banner-status-dot.active {
    background: #22c55e;
    box-shadow: 0 0 8px rgba(34, 197, 94, 0.85);
  }
  .cyan-banner .banner-logo-ring.glow img {
    filter: drop-shadow(0 0 10px rgba(0, 243, 255, 0.7))
      drop-shadow(0 0 20px rgba(0, 243, 255, 0.35));
  }
  .sky-banner .banner-logo-ring.glow img {
    filter: drop-shadow(0 0 10px rgba(56, 189, 248, 0.7))
      drop-shadow(0 0 20px rgba(56, 189, 248, 0.35));
  }
  .amber-banner .banner-logo-ring.glow img {
    filter: drop-shadow(0 0 10px rgba(245, 158, 11, 0.7))
      drop-shadow(0 0 20px rgba(245, 158, 11, 0.35));
  }
  .teal-banner .banner-logo-ring.glow img {
    filter: drop-shadow(0 0 10px rgba(0, 212, 170, 0.7))
      drop-shadow(0 0 20px rgba(0, 212, 170, 0.35));
  }
  .violet-banner .banner-logo-ring.glow img {
    filter: drop-shadow(0 0 10px rgba(139, 92, 246, 0.7))
      drop-shadow(0 0 20px rgba(139, 92, 246, 0.35));
  }

  /* Also glow the Fshare logo in Account card */
  .acc-logo-ring img {
    filter: drop-shadow(0 0 8px rgba(220, 60, 60, 0.5));
  }

  /* Banner color variants */
  .cyan-banner {
    background: linear-gradient(
      135deg,
      rgba(0, 243, 255, 0.15) 0%,
      rgba(0, 180, 200, 0.06) 50%,
      rgba(10, 10, 15, 0.5) 100%
    );
  }
  .violet-banner {
    background: linear-gradient(
      135deg,
      rgba(139, 92, 246, 0.15) 0%,
      rgba(100, 60, 200, 0.06) 50%,
      rgba(10, 10, 15, 0.5) 100%
    );
  }
  .sky-banner {
    background: linear-gradient(
      135deg,
      rgba(56, 189, 248, 0.15) 0%,
      rgba(40, 140, 200, 0.06) 50%,
      rgba(10, 10, 15, 0.5) 100%
    );
  }
  .amber-banner {
    background: linear-gradient(
      135deg,
      rgba(245, 158, 11, 0.15) 0%,
      rgba(200, 120, 10, 0.06) 50%,
      rgba(10, 10, 15, 0.5) 100%
    );
  }
  .teal-banner {
    background: linear-gradient(
      135deg,
      rgba(0, 212, 170, 0.15) 0%,
      rgba(0, 160, 130, 0.06) 50%,
      rgba(10, 10, 15, 0.5) 100%
    );
  }

  /* ── Card Body ── */
  .bento-body {
    padding: 0.5rem 0.75rem 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    flex: 1;
    overflow-y: auto;
  }
  .bento-disabled {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    flex: 1;
    color: rgba(255, 255, 255, 0.2);
    font-size: 0.7rem;
    font-family: var(--font-mono, monospace);
    letter-spacing: 0.1em;
  }
  .bento-disabled .material-icons {
    font-size: 1.1rem;
  }

  /* ── Account Card (Profile Hero) ── */
  .bento-account {
    overflow: hidden;
  }

  .acc-hero-banner {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    height: 115px;
    box-sizing: border-box;
    background: linear-gradient(
      135deg,
      rgba(220, 60, 60, 0.25) 0%,
      rgba(180, 40, 40, 0.12) 50%,
      rgba(10, 10, 15, 0.6) 100%
    );
    flex-shrink: 0;
  }
  .acc-dots-overlay {
    position: absolute;
    inset: 0;
    background-image: radial-gradient(rgba(0, 0, 0, 0.3) 1px, transparent 1px);
    background-size: 6px 6px;
    pointer-events: none;
  }
  .acc-logo-ring {
    width: 50px;
    height: 50px;
    border-radius: 50%;
    border: 2.5px solid rgba(255, 255, 255, 0.2);
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.3);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
    position: relative;
    z-index: 1;
  }
  .acc-logo-ring img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .acc-card-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 0.85rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  }
  .acc-info-left {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
    min-width: 0;
  }
  .acc-card-email {
    font-size: 0.92rem;
    font-weight: 700;
    color: #fff;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .acc-card-expiry {
    font-size: 0.7rem;
    color: rgba(255, 255, 255, 0.35);
    font-family: var(--font-mono, monospace);
  }
  .acc-type-badge {
    padding: 0.25rem 0.65rem;
    border-radius: 6px;
    font-size: 0.65rem;
    font-weight: 900;
    letter-spacing: 0.1em;
    font-family: var(--font-mono, monospace);
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.08);
    flex-shrink: 0;
  }
  .acc-type-badge.vip {
    background: rgba(0, 243, 255, 0.1);
    color: #00f3ff;
    border-color: rgba(0, 243, 255, 0.3);
  }

  .acc-card-actions {
    display: flex;
    gap: 0.4rem;
    padding: 0 0.75rem 0.75rem;
    margin-top: auto;
  }
  .acc-action-btn.icon-only {
    flex: none;
    width: 36px;
    height: 36px;
    padding: 0;
    aspect-ratio: 1;
  }
  .acc-action-btn.primary {
    flex: 5;
  }
  .acc-action-btn {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    padding: 0.55rem 0.6rem;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    color: rgba(255, 255, 255, 0.5);
    font-size: 0.72rem;
    font-weight: 700;
    font-family: var(--font-mono, monospace);
    cursor: pointer;
    transition: all 0.2s;
    white-space: nowrap;
  }
  .acc-action-btn .material-icons {
    font-size: 1rem;
  }
  .acc-action-btn:hover {
    border-color: rgba(255, 255, 255, 0.2);
    color: #fff;
    background: rgba(255, 255, 255, 0.06);
  }
  .acc-action-btn.primary {
    background: rgba(0, 243, 255, 0.08);
    border-color: rgba(0, 243, 255, 0.2);
    color: #00f3ff;
  }
  .acc-action-btn.primary:hover {
    background: rgba(0, 243, 255, 0.15);
    border-color: rgba(0, 243, 255, 0.4);
    box-shadow: 0 0 12px rgba(0, 243, 255, 0.1);
  }

  /* ── Compact field ── */
  .compact-field {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .compact-field label {
    font-size: 0.55rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: rgba(255, 255, 255, 0.4);
    font-family: var(--font-mono, monospace);
  }
  .compact-field input[type="text"],
  .compact-field input[type="password"] {
    width: 100%;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    color: #fff;
    font-size: 0.72rem;
    padding: 0.35rem 0.5rem;
    font-family: var(--font-mono, monospace);
    transition: border-color 0.2s;
  }
  .compact-field input:focus {
    border-color: rgba(0, 243, 255, 0.4);
    outline: none;
  }
  .compact-actions {
    display: flex;
    gap: 0.4rem;
    margin-top: auto;
    /* same bottom padding as .acc-card-actions so all card buttons sit at equal distance from card edge */
    padding: 0.4rem 0.75rem 0.75rem;
  }

  /* ── Download Engine Card ── */
  /* Two-section body: PATH above, CONCURRENCY below */
  .dl-body {
    gap: 0; /* sections carry their own padding; no extra gap */
    padding: 0; /* sections carry horizontal padding */
  }
  .dl-section {
    padding: 0.85rem 0.85rem;
  }
  .dl-section--sep {
    border-top: 1px solid rgba(255, 255, 255, 0.05);
  }
  .dl-section--conc {
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  .dl-path-field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .dl-path-label {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    color: rgba(255, 255, 255, 0.4);
    font-family: var(--font-mono, monospace);
    margin-bottom: 0.35rem;
  }
  .dl-path-label .material-icons {
    font-size: 1rem;
    color: var(--color-primary, #00f3ff);
    opacity: 0.65;
  }
  .dl-path-input {
    width: 100%;
    box-sizing: border-box;
    background: rgba(0, 243, 255, 0.03);
    border: 1px solid rgba(0, 243, 255, 0.12);
    border-radius: 8px;
    color: #fff;
    font-size: 0.82rem;
    padding: 0.45rem 0.65rem;
    font-family: var(--font-mono, monospace);
    outline: none;
    transition:
      border-color 0.2s,
      box-shadow 0.2s;
  }
  .dl-path-input::placeholder {
    color: rgba(255, 255, 255, 0.2);
  }
  .dl-path-input:focus {
    border-color: rgba(0, 243, 255, 0.4);
    box-shadow: 0 0 10px rgba(0, 243, 255, 0.1);
  }

  /* Folder source row: input + refresh btn */
  .dl-folder-source-row {
    display: flex;
    gap: 0.4rem;
    align-items: stretch;
  }
  .dl-folder-source-row .dl-path-input {
    flex: 1;
    min-width: 0;
  }
  .dl-refresh-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    min-width: 36px;
    background: rgba(0, 243, 255, 0.06);
    border: 1px solid rgba(0, 243, 255, 0.15);
    border-radius: 8px;
    color: rgba(0, 243, 255, 0.7);
    cursor: pointer;
    transition: all 0.2s ease;
  }
  .dl-refresh-btn .material-icons {
    font-size: 1.1rem;
    transition: transform 0.3s ease;
  }
  .dl-refresh-btn:hover:not(:disabled) {
    background: rgba(0, 243, 255, 0.12);
    border-color: rgba(0, 243, 255, 0.35);
    color: #00f3ff;
    box-shadow: 0 0 12px rgba(0, 243, 255, 0.15);
  }
  .dl-refresh-btn:active:not(:disabled) {
    transform: scale(0.92);
  }
  .dl-refresh-btn:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }
  .dl-refresh-btn.spinning .material-icons {
    animation: spin-icon 1s linear infinite;
  }
  @keyframes spin-icon {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  .dl-conc-hud {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    flex: 1;
  }
  .dl-conc-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .dl-conc-label {
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    color: rgba(255, 255, 255, 0.4);
    font-family: var(--font-mono, monospace);
  }
  .dl-conc-value {
    font-size: 1rem;
    font-weight: 900;
    font-family: var(--font-mono, monospace);
    color: var(--color-primary, #00f3ff);
    text-shadow: 0 0 12px rgba(0, 243, 255, 0.6);
    min-width: 1.5ch;
    text-align: right;
  }

  /* Segmented bar */
  /* ── Dot-track slider ── */
  .dl-dot-slider {
    position: relative;
    height: 48px;
    display: flex;
    align-items: center;
    cursor: col-resize;
    outline: none;
    user-select: none;
    -webkit-user-select: none;
  }
  .dl-dot-slider:focus-visible {
    outline: 1px solid rgba(0, 243, 255, 0.35);
    border-radius: 24px;
  }

  /* Track line: cyan left of orb, dim right */
  .dl-dot-slider::before {
    content: "";
    position: absolute;
    left: 22px; /* half of new 44px orb */
    right: 22px;
    top: 50%;
    transform: translateY(-50%);
    height: 1.5px;
    background: linear-gradient(
      to right,
      rgba(0, 243, 255, 0.5) 0%,
      rgba(0, 243, 255, 0.5) calc(var(--pct, 0) * 100%),
      rgba(255, 255, 255, 0.07) calc(var(--pct, 0) * 100%),
      rgba(255, 255, 255, 0.07) 100%
    );
    border-radius: 1px;
    pointer-events: none;
  }

  /* 10 dots — absolute, each positioned to match orb travel axis */
  .dl-dots {
    position: absolute;
    inset: 0;
    pointer-events: none;
    z-index: 1;
  }
  .dl-dot {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.12);
    transition:
      background 0.15s,
      box-shadow 0.15s,
      transform 0.15s,
      opacity 0.12s;
  }
  .dl-dot.lit {
    background: rgba(0, 243, 255, 0.85);
    box-shadow: 0 0 9px rgba(0, 243, 255, 0.75);
    transform: translateY(-50%) scale(1.25);
  }
  /* Hide dot sitting beneath the orb so value text is readable */
  .dl-dot.hidden {
    opacity: 0;
  }

  /* Pulsing circle thumb — value inside */
  .dl-thumb-orb {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    left: calc(var(--pct, 0) * (100% - 40px));
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: radial-gradient(
      circle at 38% 32%,
      rgba(180, 255, 255, 0.22) 0%,
      rgba(0, 180, 210, 0.14) 60%,
      rgba(0, 80, 120, 0.08) 100%
    );
    border: 2px solid #00f3ff;
    color: #00f3ff;
    font-family: var(--font-mono, monospace);
    font-weight: 900;
    font-size: 1rem;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
    z-index: 5;
    transition: left 0.04s linear;
    animation: orb-pulse 2.4s ease-in-out infinite;
  }
  @keyframes orb-pulse {
    0%,
    100% {
      box-shadow:
        0 0 10px rgba(0, 243, 255, 0.4),
        0 0 20px rgba(0, 243, 255, 0.15),
        inset 0 0 8px rgba(0, 243, 255, 0.1);
    }
    50% {
      box-shadow:
        0 0 20px rgba(0, 243, 255, 0.8),
        0 0 42px rgba(0, 243, 255, 0.3),
        inset 0 0 14px rgba(0, 243, 255, 0.2);
    }
  }

  /* ── Inline field ── */
  .inline-field {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.25rem 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
  }
  .inline-field:last-of-type {
    border-bottom: none;
  }
  .inline-label {
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: rgba(255, 255, 255, 0.35);
    font-family: var(--font-mono, monospace);
    min-width: 32px;
  }
  .inline-value {
    flex: 1;
    font-size: 0.8rem;
    color: rgba(255, 255, 255, 0.8);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .inline-value.mono {
    font-family: var(--font-mono, monospace);
  }
  /* Editable inline input — matches .inline-value visually but is an <input> */
  .inline-input-edit {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.8);
    font-size: 0.8rem;
    font-family: inherit;
    padding: 0.05rem 0.2rem;
    outline: none;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: border-color 0.2s;
  }
  .inline-input-edit.mono {
    font-family: var(--font-mono, monospace);
  }
  .inline-input-edit::placeholder {
    color: rgba(255, 255, 255, 0.2);
  }
  .inline-input-edit:focus {
    border-bottom-color: rgba(0, 243, 255, 0.4);
  }
  .copy-inline-btn {
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.25);
    cursor: pointer;
    padding: 2px;
    display: flex;
    transition: color 0.2s;
  }
  .copy-inline-btn .material-icons {
    font-size: 0.8rem;
  }
  .copy-inline-btn:hover {
    color: #00f3ff;
  }

  .info-badge {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.3rem 0.5rem;
    background: rgba(0, 255, 163, 0.06);
    border: 1px solid rgba(0, 255, 163, 0.15);
    border-radius: 6px;
    color: rgba(0, 255, 163, 0.8);
    font-size: 0.6rem;
    font-weight: 600;
    font-family: var(--font-mono, monospace);
  }
  .info-badge .material-icons {
    font-size: 0.85rem;
  }

  .btn-regen {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.3rem 0.5rem;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    color: rgba(255, 255, 255, 0.5);
    font-size: 0.6rem;
    font-weight: 700;
    font-family: var(--font-mono, monospace);
    cursor: pointer;
    transition: all 0.2s;
    margin-top: auto;
  }
  .btn-regen .material-icons {
    font-size: 0.85rem;
  }
  .btn-regen:hover {
    border-color: rgba(0, 243, 255, 0.3);
    color: #00f3ff;
    background: rgba(0, 243, 255, 0.06);
  }

  /* ═══════════════════════════════ */
  /* Right: Log / Activity Panel    */
  /* ═══════════════════════════════ */
  .log-panel {
    display: flex;
    flex-direction: column;
    border-radius: 14px;
    background: rgba(8, 10, 15, 0.85);
    backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.05);
    overflow: hidden;
  }

  .log-panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    flex-shrink: 0;
  }
  .log-toggle {
    display: flex;
    gap: 0.25rem;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 8px;
    padding: 2px;
  }
  .log-tab {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.4rem 0.75rem;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: rgba(255, 255, 255, 0.4);
    font-size: 0.75rem;
    font-weight: 700;
    font-family: var(--font-mono, monospace);
    letter-spacing: 0.06em;
    cursor: pointer;
    transition: all 0.2s;
  }
  .log-tab .material-icons {
    font-size: 1rem;
  }
  .log-tab.active {
    background: rgba(0, 243, 255, 0.1);
    color: #00f3ff;
    box-shadow: 0 0 8px rgba(0, 243, 255, 0.08);
  }
  .log-tab:hover:not(.active) {
    color: rgba(255, 255, 255, 0.6);
  }

  .btn-clear-log {
    width: 28px;
    height: 28px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.3);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.2s;
  }
  .btn-clear-log .material-icons {
    font-size: 0.9rem;
  }
  .btn-clear-log:hover {
    color: #ff5252;
    border-color: rgba(255, 82, 82, 0.3);
    background: rgba(255, 82, 82, 0.06);
  }

  /* ── Log Panel Body ── */
  .log-panel-body {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0.5rem;
    font-family: var(--font-mono, monospace);
    font-size: 0.65rem;
    line-height: 1.6;
  }
  .log-panel-body::-webkit-scrollbar {
    width: 4px;
  }
  .log-panel-body::-webkit-scrollbar-thumb {
    background: rgba(0, 243, 255, 0.15);
    border-radius: 2px;
  }

  .log-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    height: 100%;
    color: rgba(255, 255, 255, 0.2);
    font-size: 0.7rem;
  }
  .log-empty .material-icons {
    font-size: 1.2rem;
  }

  .log-line {
    padding: 0.15rem 0.4rem;
    border-radius: 3px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .log-line:hover {
    background: rgba(255, 255, 255, 0.03);
  }
  .log-ts {
    color: rgba(255, 255, 255, 0.25);
    margin-right: 0.4rem;
  }
  .log-lvl {
    font-weight: 800;
    margin-right: 0.4rem;
    min-width: 36px;
    display: inline-block;
  }
  .log-msg {
    color: rgba(255, 255, 255, 0.7);
  }

  .log-line.info .log-lvl {
    color: #00f3ff;
  }
  .log-line.warn .log-lvl {
    color: #ffb400;
  }
  .log-line.error .log-lvl {
    color: #ff5252;
  }
  .log-line.debug .log-lvl {
    color: rgba(255, 255, 255, 0.35);
  }

  /* ── Activity Feed ── */
  .activity-feed {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .activity-item {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.5rem;
    border-radius: 8px;
    transition: background 0.2s;
  }
  .activity-item:hover {
    background: rgba(255, 255, 255, 0.03);
  }
  .activity-icon {
    width: 24px;
    height: 24px;
    border-radius: 6px;
    background: rgba(0, 243, 255, 0.08);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .activity-icon .material-icons {
    font-size: 0.85rem;
    color: #00f3ff;
  }
  .activity-content {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .activity-msg {
    font-size: 0.65rem;
    color: rgba(255, 255, 255, 0.75);
    line-height: 1.4;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .activity-time {
    font-size: 0.55rem;
    color: rgba(255, 255, 255, 0.25);
  }

  /* ── Log Panel Footer ── */
  .log-panel-footer {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.75rem;
    border-top: 1px solid rgba(255, 255, 255, 0.04);
    flex-shrink: 0;
  }
  .log-status {
    font-size: 0.55rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.3);
    font-family: var(--font-mono, monospace);
    letter-spacing: 0.05em;
  }
  .dot-pulse {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #00ffa3;
    animation: dot-blink 1.5s ease-in-out infinite;
  }
  @keyframes dot-blink {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.3;
    }
  }

  /* ── Responsive ── */
  @media (max-width: 1100px) {
    .settings-two-col {
      grid-template-columns: 1fr;
      height: auto;
      overflow-y: auto;
    }
    .log-panel {
      min-height: 300px;
    }
  }
  @media (max-width: 700px) {
    .widget-grid {
      grid-template-columns: 1fr;
      height: auto;
    }
    .widget-col {
      min-height: 300px;
    }
  }

  /* ============================== */
  /* Glass Card System              */
  /* ============================== */
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

  .services-grid-3col {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 1rem;
  }

  @media (max-width: 900px) {
    .services-grid-3col {
      grid-template-columns: 1fr;
    }
  }

  .info-badge {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    background: rgba(0, 243, 255, 0.05);
    border: 1px solid rgba(0, 243, 255, 0.15);
    border-radius: 10px;
    color: var(--color-primary);
    font-size: 0.85rem;
    font-weight: 600;
  }

  .info-badge .material-icons {
    font-size: 1.2rem;
    color: var(--color-primary);
  }

  .service-icon {
    width: 48px;
    height: 48px;
    font-size: 2rem !important;
    color: var(--color-primary);
    filter: drop-shadow(0 0 8px rgba(0, 243, 255, 0.3));
  }

  .input-with-copy {
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .input-with-copy input {
    flex: 1;
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

  .input-with-copy input:focus {
    border-color: var(--color-primary);
    background: rgba(0, 0, 0, 0.6);
    box-shadow: 0 0 0 3px rgba(0, 243, 255, 0.1);
  }

  .copy-btn {
    background: rgba(255, 255, 255, 0.05);
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
    flex-shrink: 0;
  }

  .copy-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--color-primary);
  }

  .copy-btn .material-icons {
    font-size: 1.1rem;
  }

  .btn-secondary-full {
    width: 100%;
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: var(--text-secondary);
    border-radius: 12px;
    height: 48px;
    padding: 0 1.5rem;
    font-weight: 700;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    font-size: 0.75rem;
    transition: all 0.3s;
  }

  .btn-secondary-full:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: var(--color-primary);
    color: var(--color-primary);
  }

  .btn-secondary-full .material-icons {
    font-size: 1.2rem;
  }

  /* Integration Card - Wizard Style */
  .integration-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    padding: 1rem;
    transition: all 0.2s;
  }

  .integration-card:hover {
    border-color: rgba(0, 243, 255, 0.2);
  }

  .integration-card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-bottom: 0.5rem;
  }

  .integration-card-body {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    margin-top: 0.5rem;
    padding-top: 0.5rem;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
  }

  .integration-brand {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .integration-icon {
    width: 32px;
    height: 32px;
    object-fit: contain;
    padding: 4px;
    border-radius: 8px;
    position: relative;
    z-index: 1;
    filter: drop-shadow(0 0 1px rgba(255, 255, 255, 0.5))
      drop-shadow(0 0 8px rgba(0, 243, 255, 0.3));
  }

  .integration-brand > span {
    font-weight: 600;
    font-size: 0.9rem;
    color: #fff;
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
    position: relative;
    z-index: 1;
    filter: drop-shadow(0 0 1px rgba(255, 255, 255, 0.5))
      drop-shadow(0 0 8px rgba(0, 243, 255, 0.3));
    transition: all 0.3s ease;
  }

  .arr-node-v3:hover .brand-logo {
    filter: drop-shadow(0 0 2px rgba(255, 255, 255, 0.7))
      drop-shadow(0 0 12px rgba(0, 243, 255, 0.5));
  }

  .brand > span {
    font-weight: 600;
    font-size: 1.05rem;
    color: #fff;
    letter-spacing: 0.02em;
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
    gap: 0.4rem;
  }

  .node-field label,
  .node-field .field-label {
    font-size: 0.65rem;
    font-weight: 900;
    color: var(--text-muted);
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  .node-field input,
  .pass-box input {
    width: 100%;
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 10px;
    height: 40px;
    padding: 0 0.85rem;
    color: #fff;
    font-family: var(--font-mono);
    font-size: 0.8rem;
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
    padding: 0.65rem 1rem;
    border-radius: 10px;
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
    gap: 0.75rem;
  }

  .node-actions.single-action {
    grid-template-columns: 1fr;
  }

  .readonly-input {
    background: rgba(0, 0, 0, 0.4) !important;
    border: 1px solid rgba(255, 255, 255, 0.08) !important;
    cursor: default !important;
    color: rgba(255, 255, 255, 0.7) !important;
  }

  .full-width {
    width: 100%;
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
    height: 40px;
    border-radius: 10px;
    font-weight: 800;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    transition: all 0.2s;
    font-size: 0.75rem;
    letter-spacing: 0.05em;
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
