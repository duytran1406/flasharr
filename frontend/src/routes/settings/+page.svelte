<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { Badge, Button, Modal } from "@media-set/core-ui";
  import {
    systemStore,
    systemLogs,
    systemActivity,
    downloadSettings,
    indexerSettings,
    sonarrSettings,
    radarrSettings,
    prowlarrSettings,
    integrationStatus,
    metadataProviders,
    storageStatus,
    systemHealth,
    torboxSettings,
    torboxProfile,
    folderSourceUrl,
    importedFolderSources as folderImportedSourceCount,
    integrationStatusKind,
    integrationStatusLabel,
    type ActivityEvent,
    type LogEntry,
    type MetadataProvider,
    type RadarrSettings,
    type SonarrSettings,
    type ProwlarrSettings,
    type TorBoxProfile,
    type TorBoxSettings,
  } from "$lib/stores/system";
  import { toasts } from "$lib/stores/toasts";
  import { accountStore } from "$lib/stores/account.svelte";
  import { authStore } from "$lib/stores/auth.svelte";
  import { fetchLibraryOverview, formatDiskSize, type LibraryOverview } from "$lib/stores/arr";
  import { copyToClipboard } from "$lib/utils/clipboard";
  import SwitchAccountModal from "$lib/components/SwitchAccountModal.svelte";
  import IntegrationCards from "$lib/components/settings/IntegrationCards.svelte";
  import SettingsLogPanel from "$lib/components/settings/SettingsLogPanel.svelte";
  import SettingsMobileTabs from "$lib/components/settings/SettingsMobileTabs.svelte";
  import StremioAddonsSettings from "$lib/components/settings/StremioAddonsSettings.svelte";
  import FolderCacheSyncModal, { type FolderCacheStatus } from "$lib/components/settings/FolderCacheSyncModal.svelte";
  import FolderCacheRefreshModal from "$lib/components/settings/FolderCacheRefreshModal.svelte";
  import { SETTINGS_TABS, nextSettingsTab, type SettingsTabId } from "$lib/components/settings/settingsTabs";

  type AccountProvider = "fshare" | "torbox";

  interface AccountRow {
    id: string;
    provider: AccountProvider;
    email: string;
    plan: string;
    expiry: string;
    active: boolean;
    subtitle?: string;
    profile?: TorBoxProfile | null;
  }

  const METADATA_PROVIDERS = [
    { id: "tmdb", name: "TMDB", logo: "/images/integrations/tmdb.svg", url: "https://www.themoviedb.org" },
    { id: "tvdb", name: "TheTVDB", logo: "/images/integrations/tvdb.png", url: "https://thetvdb.com" },
  ];

  // ── Tabs (?tab= query parameter, General is the default) ──
  let logCategory = $state<"services" | "system">("services");
  let activeTab = $state<SettingsTabId>(
    SETTINGS_TABS.some((tab) => tab.id === $page.url.searchParams.get("tab"))
      ? ($page.url.searchParams.get("tab") as SettingsTabId)
      : "general",
  );

  function selectTab(tab: SettingsTabId) {
    activeTab = tab;
    const url = new URL($page.url);
    if (tab === "general") url.searchParams.delete("tab");
    else url.searchParams.set("tab", tab);
    goto(`${url.pathname}${url.search}`, { replaceState: true, noScroll: true });
  }

  // ── UI state ──
  let showIndexerApiKey = $state(false);
  let showSonarrApiKey = $state(false);
  let showRadarrApiKey = $state(false);
  let testingSonarr = $state(false);
  let testingRadarr = $state(false);
  let testingProwlarr = $state(false);
  let savingSonarr = $state(false);
  let savingRadarr = $state(false);
  let savingProwlarr = $state(false);
  let folderSourceMessage = $state("");
  let savingFolderSources = $state(false);
  let folderSourceInput = $state("");
  let savedFolderSourceUrl = $state("");
  let stagedSourceText = $state("");
  let stagedSourceName = $state("");
  let sourceFileInput = $state<HTMLInputElement | undefined>(undefined);
  let downloadMessage = $state("");
  let savingDownload = $state(false);
  let downloadError = $state("");
  let startingCacheSync = $state(false);
  let folderCacheStatus = $state<FolderCacheStatus | null>(null);
  let cacheModalOpen = $state(false);
  let refreshModalOpen = $state(false);
  let libraryOverview = $state<LibraryOverview | null>(null);
  let logs = $state<LogEntry[]>([]);
  let activities = $state<ActivityEvent[]>([]);
  let addAccountOpen = $state(false);
  let pendingRemoval = $state<AccountRow | null>(null);
  let removingAccount = $state(false);
  let refreshingAccountId = $state<string | null>(null);
  let accountActionResult = $state<{ id: string; message: string } | null>(null);
  let torboxConfig = $state<TorBoxSettings | null>(null);
  let torboxAccountProfile = $state<TorBoxProfile | null>(null);
  let editingProvider = $state<string | null>(null);
  let providerKeyInput = $state("");
  let showProviderKey = $state(false);
  let providerFeedback = $state("");
  let providerError = $state("");
  let testingProvider = $state(false);
  let savingProvider = $state(false);
  let passwordModalOpen = $state(false);
  let currentPassword = $state("");
  let newPassword = $state("");
  let confirmPassword = $state("");
  let changingPassword = $state(false);
  let passwordError = $state("");

  // ── Administrator password ──
  function closePasswordModal() {
    if (changingPassword) return;
    passwordModalOpen = false;
    currentPassword = newPassword = confirmPassword = passwordError = "";
  }

  async function changePassword() {
    passwordError = "";
    if (newPassword !== confirmPassword) {
      passwordError = "New passwords do not match.";
      return;
    }
    changingPassword = true;
    try {
      await authStore.changePassword(currentPassword, newPassword);
      changingPassword = false;
      toasts.success("Password changed. Sign in again.");
      closePasswordModal();
      goto("/");
    } catch (error) {
      passwordError = error instanceof Error ? error.message : "Unable to change password.";
    } finally {
      changingPassword = false;
    }
  }

  // ── Metadata providers ──
  function providerRow(provider: string): MetadataProvider {
    return (
      $metadataProviders.find((row) => row.provider === provider) ?? {
        provider,
        configured: false,
        source: "none",
        maskedKey: "",
        reachable: null,
        message: "",
      }
    );
  }

  function openProviderEditor(provider: string) {
    editingProvider = provider;
    providerKeyInput = "";
    showProviderKey = false;
    providerFeedback = "";
    providerError = "";
  }

  function closeProviderEditor() {
    if (savingProvider || testingProvider) return;
    editingProvider = null;
    providerKeyInput = "";
    showProviderKey = false;
    providerFeedback = "";
    providerError = "";
  }

  async function testProviderKey() {
    if (!editingProvider) return;
    testingProvider = true;
    providerFeedback = providerError = "";
    const result = await systemStore.testMetadataProvider(editingProvider, providerKeyInput);
    testingProvider = false;
    if (result.success) providerFeedback = result.message ?? "";
    else providerError = result.message ?? "";
  }

  async function saveProviderKey() {
    if (!editingProvider) return;
    if (!providerKeyInput.trim()) {
      providerError = "Enter an API key to save.";
      return;
    }
    savingProvider = true;
    providerFeedback = providerError = "";
    const result = await systemStore.saveMetadataProvider(editingProvider, providerKeyInput);
    savingProvider = false;
    if (!result.success) {
      providerError = result.message ?? "";
      return;
    }
    toasts.success(result.message ?? "");
    closeProviderEditor();
  }

  // ── Accounts (Fshare via accountStore, TorBox via system settings) ──
  function formatProfileDate(value: string | null | undefined): string {
    if (!value) return "Profile unavailable";
    const date = new Date(value);
    return Number.isNaN(date.getTime())
      ? "Profile unavailable"
      : date.toLocaleDateString("en-US", { year: "numeric", month: "short", day: "numeric" });
  }

  function accountRows(): AccountRow[] {
    const fshareRows: AccountRow[] = accountStore.listFormatted.map((account) => ({
      id: `fshare:${account.email}`,
      provider: "fshare",
      email: account.email,
      plan: account.rank,
      expiry: account.expiry,
      active: account.is_active,
    }));
    const torboxRows: AccountRow[] =
      torboxConfig?.enabled && torboxConfig.api_key
        ? [
            {
              id: "torbox",
              provider: "torbox",
              email: torboxAccountProfile?.email || "Profile unavailable",
              plan: torboxAccountProfile?.plan || "Profile unavailable",
              expiry: formatProfileDate(torboxAccountProfile?.premium_expires_at),
              active: ["active", "premium", "ok", "subscribed"].includes((torboxAccountProfile?.status || "").toLowerCase()),
              subtitle: torboxAccountProfile?.status || "TorBox",
              profile: torboxAccountProfile,
            },
          ]
        : [];
    return [...fshareRows, ...torboxRows];
  }

  async function removeAccount() {
    if (!pendingRemoval) return;
    removingAccount = true;
    try {
      const result =
        pendingRemoval.provider === "fshare"
          ? await accountStore.logout(pendingRemoval.email)
          : await systemStore.clearTorBoxSettings();
      if (result.success) {
        toasts.success(result.message || "Account removed");
        pendingRemoval = null;
        if (torboxConfig?.enabled) torboxConfig = { ...torboxConfig, enabled: false, api_key: "" };
        torboxAccountProfile = null;
      } else {
        toasts.error(result.message || "Failed to remove account");
      }
    } catch {
      toasts.error("Network error — account could not be removed");
    } finally {
      removingAccount = false;
    }
  }

  async function refreshAccount(account: AccountRow) {
    refreshingAccountId = account.id;
    accountActionResult = null;
    try {
      const result =
        account.provider === "fshare"
          ? await accountStore.refresh(account.email)
          : await systemStore.testTorBoxConnection({ ...get(torboxSettings), enabled: true });
      const message =
        result.message ||
        (result.success
          ? `${account.provider === "fshare" ? "Fshare refreshed" : "TorBox connected"}`
          : `Failed to ${account.provider === "fshare" ? "refresh Fshare" : "test TorBox connection"}`);
      accountActionResult = { id: account.id, message };
      if (result.success) toasts.success(message);
      else toasts.error(message);
    } catch {
      const message = `Network error — ${account.provider} could not be refreshed`;
      accountActionResult = { id: account.id, message };
      toasts.error(message);
    } finally {
      refreshingAccountId = null;
      await reloadAccounts();
    }
  }

  async function loadTorBoxAccount() {
    await systemStore.fetchTorBoxSettings();
    const settings = get(torboxSettings);
    if (settings.enabled && settings.api_key) await systemStore.fetchTorBoxProfile();
    torboxConfig = settings;
    torboxAccountProfile = get(torboxProfile);
  }

  async function reloadAccounts() {
    await Promise.all([accountStore.fetch(), loadTorBoxAccount()]);
  }

  // ── Form state mirrored from the stores ──
  let concurrency = $state(3);
  let segmentsPerDownload = $state(4);
  let downloadPath = $state("");
  let indexerApiKey = $state("");
  let sonarrUrl = $state("http://localhost:8989");
  let sonarrApiKey = $state("");
  let sonarrAutoImport = $state(true);
  let radarrUrl = $state("http://localhost:7878");
  let radarrApiKey = $state("");
  let radarrAutoImport = $state(true);
  let prowlarrUrl = $state("http://localhost:9696");
  let prowlarrApiKey = $state("");
  let savedDownloadPath = $state("");
  let savedConcurrency = $state(3);
  let savedSonarrUrl = $state("");
  let savedSonarrApiKey = $state("");
  let savedRadarrUrl = $state("");
  let savedRadarrApiKey = $state("");
  let savedProwlarrUrl = $state("");
  let savedProwlarrApiKey = $state("");
  let sonarrChangedElsewhere = $state(false);
  let radarrChangedElsewhere = $state(false);
  let prowlarrChangedElsewhere = $state(false);
  let loaded = $state(false);
  let loadError = $state("");
  let cacheStatusTimer: ReturnType<typeof setInterval> | undefined;
  let integrationStatusTimer: ReturnType<typeof setInterval> | undefined;

  // ── Fshare folder cache ──
  async function fetchFolderCacheStatus() {
    try {
      const response = await fetch("/api/folder-source/status");
      if (response.ok) folderCacheStatus = await response.json();
    } catch {
      // keep the last known status
    }
  }

  function cacheIsStale(): boolean {
    const lastSync = Number(folderCacheStatus?.last_sync);
    return !Number.isFinite(lastSync) || Date.now() / 1000 - lastSync > 86400;
  }

  function cacheHealth(): "syncing" | "attention" | "ready" | "empty" {
    if (folderCacheStatus?.is_syncing) return "syncing";
    if (
      folderCacheStatus?.last_error ||
      folderCacheStatus?.progress.outcome === "failed" ||
      ((folderCacheStatus?.total_items ?? 0) > 0 && cacheIsStale())
    )
      return "attention";
    return folderCacheStatus?.total_items ? "ready" : "empty";
  }

  function nextScheduledSync(): string {
    const now = new Date();
    const next = new Date(now);
    next.setHours(1, 0, 0, 0);
    if (next <= now) next.setDate(next.getDate() + 1);
    return next.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }

  function cacheSummary(): string {
    const folders = folderCacheStatus?.folder_count ?? 0;
    const health = cacheHealth();
    const label =
      health === "syncing" ? "Syncing" : health === "ready" ? "Healthy" : health === "attention" ? "Needs attention" : "Not indexed";
    return `${folders} folders cached · ${label} · Next sync ${nextScheduledSync()}`;
  }

  // ── Concurrency dot slider ──
  function handleSliderPointerDown(event: PointerEvent) {
    const slider = event.currentTarget as HTMLElement;
    slider.setPointerCapture(event.pointerId);
    setConcurrencyFromPointer(event, slider);
  }

  function handleSliderPointerMove(event: PointerEvent) {
    const slider = event.currentTarget as HTMLElement;
    if (slider.hasPointerCapture(event.pointerId)) setConcurrencyFromPointer(event, slider);
  }

  function handleSliderPointerUp(event: PointerEvent) {
    const slider = event.currentTarget as HTMLElement;
    if (slider.hasPointerCapture(event.pointerId)) slider.releasePointerCapture(event.pointerId);
  }

  function handleSliderKeydown(event: KeyboardEvent) {
    const steps: Record<string, number> = { ArrowRight: 1, ArrowUp: 1, ArrowLeft: -1, ArrowDown: -1 };
    if (event.key === "Home") concurrency = 1;
    else if (event.key === "End") concurrency = 10;
    else if (steps[event.key]) concurrency = Math.min(10, Math.max(1, concurrency + steps[event.key]));
    else return;
    event.preventDefault();
  }

  function setConcurrencyFromPointer(event: PointerEvent, slider: HTMLElement) {
    const rect = slider.getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    concurrency = Math.round(ratio * 9) + 1;
  }

  // ── Download configuration ──
  function validateDownloadSettings(): string {
    if (!downloadPath.trim().startsWith("/")) return "Download path must be absolute.";
    if (!Number.isInteger(Number(concurrency)) || concurrency < 1 || concurrency > 10) return "Concurrency must be between 1 and 10.";
    return "";
  }

  async function saveDownloadConfig() {
    downloadError = validateDownloadSettings();
    if (downloadError) return;
    savingDownload = true;
    downloadMessage = "";
    const result = await systemStore.saveDownloadSettings({
      directory: downloadPath.trim(),
      max_concurrent: Number(concurrency),
      segments_per_download: segmentsPerDownload,
    });
    savingDownload = false;
    if (!result.success) {
      downloadError = result.message || "Failed to save download configuration";
      return;
    }
    downloadPath = downloadPath.trim();
    savedDownloadPath = downloadPath;
    savedConcurrency = concurrency;
    downloadMessage = result.message || "Download configuration saved.";
  }

  // ── Folder sources ──
  async function stageSourceFile(files: FileList | null | undefined) {
    const file = files?.[0];
    if (!file) return;
    if (!file.name.toLowerCase().endsWith(".txt")) {
      folderSourceMessage = "Choose a .txt source list.";
      return;
    }
    if (file.size === 0 || file.size > 1024 * 1024) {
      folderSourceMessage = "Source list must be between 1 byte and 1 MiB.";
      return;
    }
    stagedSourceText = await file.text();
    stagedSourceName = file.name;
    folderSourceMessage = "";
  }

  async function updateFolderSources() {
    savingFolderSources = true;
    folderSourceMessage = "";
    const text = stagedSourceText.trim();
    const url = folderSourceInput.trim();
    const result = text
      ? await systemStore.importFolderSources(text)
      : url !== savedFolderSourceUrl
        ? await systemStore.saveFolderSourceConfig(url)
        : { success: false, message: "Enter a remote text-file URL or upload a local .txt list." };
    savingFolderSources = false;
    folderSourceMessage =
      result.message || (result.success ? "Folder sources updated" : "Folder sources could not be updated");
    if (result.success) {
      if (text) {
        stagedSourceText = "";
        stagedSourceName = "";
      } else {
        folderSourceInput = savedFolderSourceUrl = url;
      }
      await fetchFolderCacheStatus();
    }
  }

  async function startCacheSync() {
    cacheModalOpen = true;
    if (startingCacheSync || folderCacheStatus?.is_syncing) {
      await fetchFolderCacheStatus();
      return;
    }
    startingCacheSync = true;
    try {
      const response = await fetch("/api/folder-source/sync", { method: "POST" });
      const body = await response.json().catch(() => null);
      if (!response.ok) {
        toasts.error(body?.message || "Failed to trigger folder cache sync");
        return;
      }
      await fetchFolderCacheStatus();
      toasts.success(body?.already_running ? "Folder cache sync already running" : "Folder cache sync started");
    } catch {
      toasts.error("Network error — could not reach server");
    } finally {
      startingCacheSync = false;
    }
  }

  async function refreshSingleFolder(folderUrl: string): Promise<string | null> {
    try {
      const response = await fetch("/api/folder-source/refresh", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ folder_url: folderUrl }),
      });
      const body = await response.json().catch(() => null);
      if (!response.ok || body?.success !== true) return body?.message || "Folder refresh could not be started";
      refreshModalOpen = false;
      cacheModalOpen = true;
      await fetchFolderCacheStatus();
      toasts.success(body.message || "Folder refresh started");
      return null;
    } catch {
      return "Network error — could not reach server";
    }
  }

  // ── Indexer + *arr integrations ──
  async function regenerateIndexerKey() {
    const key = await systemStore.generateIndexerApiKey();
    if (key) {
      indexerApiKey = key;
      toasts.success("New API key generated");
    } else {
      toasts.error("Failed to generate API key");
    }
  }

  function sonarrFormDirty() {
    return sonarrUrl !== savedSonarrUrl || sonarrApiKey !== savedSonarrApiKey;
  }

  function radarrFormDirty() {
    return radarrUrl !== savedRadarrUrl || radarrApiKey !== savedRadarrApiKey;
  }

  function prowlarrFormDirty() {
    return prowlarrUrl !== savedProwlarrUrl || prowlarrApiKey !== savedProwlarrApiKey;
  }

  function resetSonarrForm(settings: SonarrSettings = get(sonarrSettings)) {
    sonarrUrl = savedSonarrUrl = settings.url;
    sonarrApiKey = savedSonarrApiKey = settings.api_key;
    sonarrAutoImport = settings.auto_import;
    sonarrChangedElsewhere = false;
  }

  function resetRadarrForm(settings: RadarrSettings = get(radarrSettings)) {
    radarrUrl = savedRadarrUrl = settings.url;
    radarrApiKey = savedRadarrApiKey = settings.api_key;
    radarrAutoImport = settings.auto_import;
    radarrChangedElsewhere = false;
  }

  function resetProwlarrForm(settings: ProwlarrSettings = get(prowlarrSettings)) {
    prowlarrUrl = savedProwlarrUrl = settings.url;
    prowlarrApiKey = savedProwlarrApiKey = settings.api_key;
    prowlarrChangedElsewhere = false;
  }

  async function saveSonarr() {
    savingSonarr = true;
    const result = await systemStore.saveSonarrSettings({
      enabled: true,
      url: sonarrUrl,
      api_key: sonarrApiKey,
      auto_import: sonarrAutoImport,
    });
    savingSonarr = false;
    if (result.success) resetSonarrForm();
    const message = result.message || (result.success ? "Sonarr settings saved." : "Failed to save Sonarr settings.");
    if (result.success) toasts.success(message);
    else toasts.error(message);
  }

  async function testSonarr() {
    testingSonarr = true;
    const result = await systemStore.testSonarrConnection({
      enabled: true,
      url: sonarrUrl,
      api_key: sonarrApiKey,
      auto_import: sonarrAutoImport,
    });
    const message = result.message || (result.success ? "Sonarr connection successful." : "Sonarr connection failed.");
    if (result.success) toasts.success(message);
    else toasts.error(message);
    testingSonarr = false;
  }

  async function saveRadarr() {
    savingRadarr = true;
    const result = await systemStore.saveRadarrSettings({
      enabled: true,
      url: radarrUrl,
      api_key: radarrApiKey,
      auto_import: radarrAutoImport,
    });
    savingRadarr = false;
    if (result.success) resetRadarrForm();
    const message = result.message || (result.success ? "Radarr settings saved." : "Failed to save Radarr settings.");
    if (result.success) toasts.success(message);
    else toasts.error(message);
  }

  async function testRadarr() {
    testingRadarr = true;
    const result = await systemStore.testRadarrConnection({
      enabled: true,
      url: radarrUrl,
      api_key: radarrApiKey,
      auto_import: radarrAutoImport,
    });
    const message = result.message || (result.success ? "Radarr connection successful." : "Radarr connection failed.");
    if (result.success) toasts.success(message);
    else toasts.error(message);
    testingRadarr = false;
  }

  async function saveProwlarr() {
    savingProwlarr = true;
    const result = await systemStore.saveProwlarrSettings({
      enabled: true,
      url: prowlarrUrl,
      api_key: prowlarrApiKey,
      limit: 50,
    } as ProwlarrSettings);
    savingProwlarr = false;
    if (result.success) resetProwlarrForm();
    const message = result.message || (result.success ? "Prowlarr settings saved." : "Failed to save Prowlarr.");
    if (result.success) toasts.success(message);
    else toasts.error(message);
  }

  async function testProwlarr() {
    testingProwlarr = true;
    const result = await systemStore.testProwlarrConnection({
      enabled: true,
      url: prowlarrUrl,
      api_key: prowlarrApiKey,
      limit: 50,
    } as ProwlarrSettings);
    const message = result.message || (result.success ? "Prowlarr connected." : "Prowlarr failed.");
    if (result.success) toasts.success(message);
    else toasts.error(message);
    testingProwlarr = false;
  }

  async function copyValue(value: string, label: string) {
    if (await copyToClipboard(value)) toasts.success(`${label} copied to clipboard`);
    else toasts.error("Failed to copy to clipboard");
  }

  async function clearLogs() {
    systemStore.clearLogs();
    toasts.success("Log view cleared until new entries arrive");
  }

  // ── Loading ──
  async function loadSettings() {
    loaded = false;
    loadError = "";
    try {
      const [downloadsOk, indexerOk, integrationsOk] = await Promise.all([
        systemStore.fetchDownloadSettings(),
        systemStore.fetchIndexerSettings(),
        systemStore.refreshIntegrationState(),
        systemStore.fetchLogs(),
        systemStore.fetchActivity(),
        fetchLibraryOverview().then((overview) => {
          libraryOverview = overview;
        }),
        systemStore.fetchFolderSourceConfig(),
        accountStore.fetch(),
        loadTorBoxAccount(),
        systemStore.fetchMetadataProviders(),
        systemStore.fetchSystemHealth(),
        systemStore.fetchStorageStatus(),
      ]);
      if (![downloadsOk, indexerOk, integrationsOk].every(Boolean)) {
        loadError = "Settings could not be loaded.";
        return;
      }
      const downloads = get(downloadSettings);
      concurrency = downloads.max_concurrent;
      segmentsPerDownload = downloads.segments_per_download;
      downloadPath = downloads.directory;
      savedConcurrency = concurrency;
      savedDownloadPath = downloadPath;
      indexerApiKey = get(indexerSettings).api_key;
      folderSourceInput = savedFolderSourceUrl = get(folderSourceUrl);
      resetSonarrForm();
      resetRadarrForm();
      resetProwlarrForm();
      await Promise.all(
        get(metadataProviders)
          .filter((provider) => provider.configured)
          .map((provider) => systemStore.testMetadataProvider(provider.provider)),
      );
      loaded = true;
    } catch (error) {
      loadError = error instanceof Error ? error.message : "Settings could not be loaded.";
    }
  }

  onMount(() => {
    loadSettings();
    const unsubscribeLogs = systemLogs.subscribe((value) => {
      logs = value;
    });
    const unsubscribeActivity = systemActivity.subscribe((value) => {
      activities = value;
    });
    // Another tab/session changed a connection: reload the form when it is clean, otherwise flag it.
    const unsubscribeSonarr = sonarrSettings.subscribe((settings) => {
      if (!loaded || !(settings.url !== savedSonarrUrl || settings.api_key !== savedSonarrApiKey)) return;
      if (sonarrFormDirty()) sonarrChangedElsewhere = true;
      else resetSonarrForm(settings);
    });
    const unsubscribeRadarr = radarrSettings.subscribe((settings) => {
      if (!loaded || !(settings.url !== savedRadarrUrl || settings.api_key !== savedRadarrApiKey)) return;
      if (radarrFormDirty()) radarrChangedElsewhere = true;
      else resetRadarrForm(settings);
    });
    const unsubscribeProwlarr = prowlarrSettings.subscribe((settings) => {
      if (!loaded || !(settings.url !== savedProwlarrUrl || settings.api_key !== savedProwlarrApiKey)) return;
      if (prowlarrFormDirty()) prowlarrChangedElsewhere = true;
      else resetProwlarrForm(settings);
    });

    fetchFolderCacheStatus();
    cacheStatusTimer = setInterval(() => {
      if (folderCacheStatus?.is_syncing) fetchFolderCacheStatus();
    }, 2000);
    integrationStatusTimer = setInterval(() => {
      systemStore.fetchIntegrationStatus();
    }, 30000);

    // Desktop tab strip lives in the shell header slot.
    const headerSlot = document.getElementById("header-dynamic-content");
    if (headerSlot) {
      headerSlot.innerHTML = "";
      const tabList = document.createElement("div");
      tabList.className = "settings-tabs-header media-toggle";
      tabList.setAttribute("role", "tablist");
      tabList.setAttribute("aria-label", "Settings sections");
      SETTINGS_TABS.forEach((tab) => {
        const button = document.createElement("button");
        button.id = `settings-tab-${tab.id}`;
        button.className = "toggle-btn";
        button.type = "button";
        button.setAttribute("role", "tab");
        button.setAttribute("aria-controls", `settings-tabpanel-${tab.id}`);
        button.setAttribute("tabindex", "-1");
        button.innerHTML = `<span class="material-icons" aria-hidden="true">${tab.icon}</span>${tab.label}`;
        button.onclick = () => {
          selectTab(tab.id);
          queueMicrotask(syncHeaderTabs);
        };
        button.onkeydown = (event) => {
          if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
          event.preventDefault();
          selectTab(nextSettingsTab(tab.id, event.key));
          queueMicrotask(() => {
            syncHeaderTabs();
            document.getElementById(`settings-tab-${activeTab}`)?.focus();
          });
        };
        tabList.appendChild(button);
      });
      headerSlot.appendChild(tabList);
      const syncHeaderTabs = () =>
        tabList.querySelectorAll("button").forEach((button, index) => {
          const selected = SETTINGS_TABS[index].id === activeTab;
          button.classList.toggle("active", selected);
          button.setAttribute("aria-selected", String(selected));
          button.setAttribute("tabindex", selected ? "0" : "-1");
        });
      syncHeaderTabs();
    }

    return () => {
      if (cacheStatusTimer) clearInterval(cacheStatusTimer);
      if (integrationStatusTimer) clearInterval(integrationStatusTimer);
      unsubscribeLogs();
      unsubscribeActivity();
      unsubscribeSonarr();
      unsubscribeRadarr();
      unsubscribeProwlarr();
    };
  });
</script>

<svelte:head>
  <title>Settings - Flasharr</title>
</svelte:head>

<div class="settings-viewport">
  <SettingsMobileTabs active={activeTab} onSelect={selectTab} />
  {#if loadError}
    <div class="settings-load-error" role="alert">
      <span>{loadError}</span><Button
        variant="ghost"
        onclick={() => {
          loadSettings();
        }}>Retry</Button
      >
    </div>
  {:else if !loaded}
    <p class="settings-loading" aria-live="polite">Loading Settings…</p>
  {:else if activeTab === "general"}
    <div id="settings-tabpanel-general" class="settings-panel general-settings" role="tabpanel" aria-labelledby="settings-tab-general">
      <section class="bento-card bento-general" aria-labelledby="general-metadata">
        <div class="bento-head cyan-accent"><span class="material-icons" aria-hidden="true">hub</span><h2 id="general-metadata">METADATA PROVIDERS</h2></div>
        <div class="general-card-body">
          <div class="provider-grid">
            {#each METADATA_PROVIDERS as provider}
              {@const row = providerRow(provider.id)}
              <article class="provider-tile">
                <a class="provider-logo" href={provider.url} target="_blank" rel="noreferrer" aria-label={`Open ${provider.name}`}><img src={provider.logo} alt={provider.name} /></a>
                <div class="provider-copy">
                  <b>{provider.name}</b>
                  <span class="provider-status" class:connected={row.reachable === true} class:unavailable={row.reachable === false} role="status"><span class="provider-status-dot" aria-hidden="true"></span>{row.reachable === true ? "Connected" : row.reachable === false ? "Unavailable" : row.configured ? "Configured" : "Not configured"}</span>
                </div>
                <Button class="metadata-edit" size="sm" variant="ghost" title={`Edit ${provider.name} metadata provider`} onclick={() => openProviderEditor(provider.id)}><span class="material-icons" aria-hidden="true">edit</span><span class="sr-only">Edit {provider.name} metadata provider</span></Button>
              </article>
            {/each}
          </div>
          <p class="provider-attribution">This product uses the <a href="https://www.themoviedb.org" target="_blank" rel="noreferrer">TMDB API</a> but is not endorsed or certified by TMDB. Metadata provided by <a href="https://thetvdb.com/subscribe" target="_blank" rel="noreferrer">TheTVDB</a>.</p>
        </div>
      </section>

      <section class="bento-card bento-general" aria-labelledby="general-storage">
        <div class="bento-head cyan-accent"><span class="material-icons" aria-hidden="true">storage</span><h2 id="general-storage">MOUNTED STORAGE</h2></div>
        <div class="general-card-body storage-card">
          <div class="storage-status"><span class="material-icons" aria-hidden="true">{$storageStatus?.mounted ? ($storageStatus.read_only ? "lock" : "check_circle") : "error"}</span><div><b>{$storageStatus?.message || "Checking storage"}</b><small class="mono">{$storageStatus?.path || "—"}</small></div></div>
          <div class="storage-metrics"><div><small>AVAILABLE</small><b>{formatDiskSize($storageStatus?.available_bytes || 0)}</b></div><div><small>TOTAL</small><b>{formatDiskSize($storageStatus?.total_bytes || 0)}</b></div></div>
          <div class="storage-meter" role="progressbar" aria-label="Storage used" aria-valuemin="0" aria-valuemax="100" aria-valuenow={$storageStatus?.used_percent || 0}><span style={`width:${$storageStatus?.used_percent || 0}%`}></span></div>
          <small class="storage-caption">{$storageStatus?.used_percent || 0}% used</small>
        </div>
      </section>

      <section class="bento-card bento-general" aria-labelledby="general-admin">
        <div class="bento-head cyan-accent"><span class="material-icons" aria-hidden="true">admin_panel_settings</span><h2 id="general-admin">ADMINISTRATOR</h2></div>
        <div class="general-card-body administrator-card">
          <div class="administrator-avatar" aria-hidden="true"><span class="material-icons">person</span></div>
          <div class="administrator-copy"><small>ACTIVE IDENTITY</small><b>{authStore.session?.username || "Unknown user"}</b><Badge variant={authStore.session?.authenticated ? "success" : "warning"} noDot={false}>{authStore.session?.authenticated ? "Authenticated" : "Session unavailable"}</Badge></div>
          <Button class="administrator-action" variant="ghost" icon="password" onclick={() => (passwordModalOpen = true)}>Change password</Button>
        </div>
      </section>

      <section class="bento-card bento-general" aria-labelledby="general-runtime">
        <div class="bento-head cyan-accent"><span class="material-icons" aria-hidden="true">monitor_heart</span><h2 id="general-runtime">FLASHARR SERVICE</h2></div>
        <div class="general-card-body runtime-grid"><div><small>STATUS</small><b><Badge variant={$systemHealth?.status === "ok" ? "success" : "danger"} noDot={false}>{$systemHealth?.status === "ok" ? "Online" : "Unavailable"}</Badge></b></div><div><small>VERSION</small><b class="mono">{$systemHealth?.version || "Unknown"}</b></div></div>
      </section>
    </div>
  {:else if activeTab === "accounts"}
    <div id="settings-tabpanel-accounts" class="settings-panel account-settings" role="tabpanel" aria-labelledby="settings-tab-accounts">
      <div class="panel-heading account-heading"><p>Gateway to the media universe.</p><div class="panel-actions"><Button class="account-add-button" variant="primary" icon="add" onclick={() => (addAccountOpen = true)}>Add Account</Button></div></div>
      <div class="account-list" aria-live="polite">
        {#each accountRows() as account}
          <article class="status-card-v5 account-row-card" class:torbox-account={account.provider === "torbox"} class:inactive={!account.active}>
            {#if account.active}
              <Badge class="account-status-badge" variant={account.provider === "torbox" ? "success" : "info"}>ACTIVE</Badge>
            {/if}
            <div class="account-provider"><img src={account.provider === "fshare" ? "/images/logo_fshare.png" : "/images/logo_torbox.png"} alt={account.provider === "fshare" ? "Fshare" : "TorBox"} />{#if account.provider === "fshare"}<span class="account-online-dot" aria-label="Online"></span>{/if}</div>
            <div class="account-identity"><div class="account-email">{account.email}</div><small>{account.provider === "fshare" ? "Fshare" : account.subtitle}</small></div>
            <div class="account-detail account-plan-detail"><span>PLAN</span><b class="account-plan">{account.plan}</b></div>
            <div class="account-detail account-expiry-detail"><span>EXPIRES</span><b class="account-expiry">{account.expiry}</b></div>
            <div class="account-actions"><Button class="account-icon-action" variant="ghost" size="sm" title={account.provider === "fshare" ? "Refresh Fshare" : "Test TorBox connection"} disabled={refreshingAccountId === account.id} onclick={() => refreshAccount(account)}><span class="material-icons" aria-hidden="true">{refreshingAccountId === account.id ? "sync" : account.provider === "fshare" ? "refresh" : "wifi_tethering"}</span><span class="sr-only">{account.provider === "fshare" ? "Refresh Fshare" : "Test TorBox connection"}</span></Button><Button class="account-icon-action danger" variant="danger" size="sm" title={account.provider === "fshare" ? "Delete Fshare" : "Remove TorBox configuration"} disabled={refreshingAccountId === account.id} onclick={() => (pendingRemoval = account)}><span class="material-icons" aria-hidden="true">delete</span><span class="sr-only">{account.provider === "fshare" ? "Delete Fshare" : "Remove TorBox configuration"}</span></Button>{#if refreshingAccountId === account.id}<small class="account-action-status" role="status">Checking…</small>{:else if accountActionResult?.id === account.id}<small class="account-action-status" role="status">{accountActionResult.message}</small>{/if}</div>
          </article>
        {:else}
          <div class="empty-state"><span class="material-icons">key_off</span><h3>No accounts configured</h3><p>Add an Fshare or TorBox account to unlock provider downloads.</p><Button class="account-add-button" variant="primary" icon="add" onclick={() => (addAccountOpen = true)}>Add Account</Button></div>
        {/each}
      </div>
    </div>
  {:else if activeTab === "download"}
    <div id="settings-tabpanel-download" class="settings-panel download-grid" role="tabpanel" aria-labelledby="settings-tab-download">
      <article class="bento-card bento-download" aria-labelledby="download-engine-title">
        <div class="bento-head cyan-accent"><span class="material-icons" aria-hidden="true">speed</span><h2 id="download-engine-title">DOWNLOAD ENGINE</h2></div>
        <div class="card-banner cyan-banner"><div class="banner-dots"></div><div class="banner-logo-ring download-logo"><img src="/images/flasharr_logo.png" alt="Flasharr" /></div></div>
        <div class="bento-body dl-body">
          <div class="dl-section"><label class="dl-path-field" for="b-dl-path"><span class="dl-path-label"><span class="material-icons" aria-hidden="true">folder_open</span>PATH</span><input id="b-dl-path" class="dl-path-input" bind:value={downloadPath} placeholder="/media/downloads" aria-invalid={!!downloadError} /></label></div>
          <div class="dl-section dl-section--conc dl-section--sep"><div class="dl-conc-hud"><span class="dl-conc-label">CONCURRENCY</span><div
                class="dl-dot-slider"
                role="slider"
                aria-label="Concurrent downloads"
                aria-valuemin="1"
                aria-valuemax="10"
                aria-valuenow={concurrency}
                tabindex="0"
                style={`--pct:${(concurrency - 1) / 9}`}
                onpointerdown={handleSliderPointerDown}
                onpointermove={handleSliderPointerMove}
                onpointerup={handleSliderPointerUp}
                onpointercancel={handleSliderPointerUp}
                onkeydown={handleSliderKeydown}
              ><div class="dl-dots" aria-hidden="true">{#each Array.from({ length: 10 }) as _, index}<span class="dl-dot" class:lit={index < concurrency} class:hidden={index === concurrency - 1} style={`left:calc(${index / 9} * (100% - 40px) + 16px)`}></span>{/each}</div><span class="dl-thumb-orb" aria-hidden="true">{concurrency}</span></div></div></div>
          {#if downloadError}
            <p class="settings-result error" role="alert">{downloadError}</p>
          {:else if downloadMessage}
            <p class="settings-result success" role="status">{downloadMessage}</p>
          {/if}
          <div class="compact-actions"><Button accent="#00f3ff" icon="save" width="100%" loading={savingDownload} disabled={savingDownload || (downloadPath === savedDownloadPath && concurrency === savedConcurrency)} onclick={saveDownloadConfig}>APPLY</Button></div>
        </div>
      </article>
      <article class="settings-card config-card"><div><p class="eyebrow">BITTORRENT</p><h2>qBittorrent</h2><p>Instances, transfer state, and torrent actions live on the dedicated Torrents surface.</p></div><div class="managed"><span class="material-icons">downloading</span><div><b>Server managed clients</b><small>Connection credentials stay outside this browser.</small></div></div><div class="card-actions"><a class="primary-link" href="/torrents">Open Torrents</a></div></article>
    </div>
  {:else if activeTab === "integrations"}
    <div id="settings-tabpanel-integrations" class="settings-panel integrations-panel" role="tabpanel" aria-labelledby="settings-tab-integrations">
      <IntegrationCards
        bind:indexerApiKey
        {showIndexerApiKey}
        bind:sonarrUrl
        bind:sonarrApiKey
        sonarrStatus={$integrationStatus.sonarr}
        sonarrTesting={testingSonarr}
        radarrStatus={$integrationStatus.radarr}
        radarrTesting={testingRadarr}
        sonarrSaving={savingSonarr}
        {sonarrChangedElsewhere}
        bind:radarrUrl
        bind:radarrApiKey
        radarrSaving={savingRadarr}
        {radarrChangedElsewhere}
        {libraryOverview}
        {showSonarrApiKey}
        {showRadarrApiKey}
        onToggleIndexerKey={() => (showIndexerApiKey = !showIndexerApiKey)}
        onToggleSonarrKey={() => (showSonarrApiKey = !showSonarrApiKey)}
        onToggleRadarrKey={() => (showRadarrApiKey = !showRadarrApiKey)}
        onCopy={copyValue}
        onGenerateKey={regenerateIndexerKey}
        onTestSonarr={testSonarr}
        onSaveSonarr={saveSonarr}
        onReloadSonarr={() => resetSonarrForm()}
        onTestRadarr={testRadarr}
        onSaveRadarr={saveRadarr}
        onReloadRadarr={() => resetRadarrForm()}
      />
    </div>
  {:else if activeTab === "sources"}
    <div id="settings-tabpanel-sources" class="settings-panel sources-panel" role="tabpanel" aria-labelledby="settings-tab-sources">
      <div class="sources-layout">
        <aside class="sources-rail" aria-label="Source connections">
          <article class="bento-card bento-prowlarr" aria-labelledby="source-prowlarr-title">
            <div class="bento-head orange-accent"><img class="head-logo" src="/images/integrations/prowlarr.svg" alt="" /><h2 id="source-prowlarr-title">PROWLARR</h2></div>
            <div class="card-banner orange-banner"><div class="banner-dots"></div><div class="banner-logo-ring glow"><img src="/images/integrations/prowlarr.svg" alt="Prowlarr" /><span
                  class="banner-status-dot"
                  class:active={integrationStatusKind($integrationStatus.prowlarr, testingProwlarr || savingProwlarr) === "connected"}
                  class:unreachable={integrationStatusKind($integrationStatus.prowlarr, testingProwlarr || savingProwlarr) === "unreachable"}
                  role="status"
                  aria-label={`Prowlarr: ${integrationStatusLabel($integrationStatus.prowlarr, testingProwlarr || savingProwlarr)}`}
                ></span></div></div>
            <div class="bento-body">
              {#if prowlarrChangedElsewhere}
                <button class="reload-config" onclick={() => resetProwlarrForm()}>Configuration changed elsewhere · Reload</button>
              {/if}
              <div class="inline-field"><label class="inline-label" for="prowlarr-url">URL</label><input id="prowlarr-url" class="inline-input-edit" bind:value={prowlarrUrl} placeholder="http://localhost:9696" /></div>
              <div class="inline-field"><label class="inline-label" for="prowlarr-key">KEY</label><input id="prowlarr-key" class="inline-input-edit mono" type="password" bind:value={prowlarrApiKey} placeholder="API Key" /></div>
              <div class="compact-actions"><div><Button variant="ghost" size="sm" width="100%" loading={testingProwlarr} disabled={testingProwlarr || savingProwlarr} onclick={testProwlarr}><span class="material-icons">sync_alt</span> VERIFY</Button></div><div><Button accent="#f97316" size="sm" width="100%" loading={savingProwlarr} disabled={savingProwlarr || (prowlarrUrl === savedProwlarrUrl && prowlarrApiKey === savedProwlarrApiKey)} onclick={saveProwlarr}>SAVE</Button></div></div>
            </div>
          </article>
          <article class="bento-card bento-fshare" aria-labelledby="source-fshare-title">
            <div class="bento-head fshare-accent"><img class="head-logo" src="/images/logo_fshare.png" alt="" /><h2 id="source-fshare-title">FSHARE CACHE</h2></div>
            <div class="card-banner fshare-banner"><div class="banner-dots"></div><div class="banner-logo-ring glow"><img src="/images/logo_fshare.png" alt="Fshare" /></div></div>
            <div class="bento-body fshare-body">
              <p class="cache-summary" class:stale={cacheHealth() === "attention"} role="status">{cacheSummary()}</p>
              <div class="source-entry">
                <input id="fshare-source-url" aria-label="Remote Fshare source text-file URL" bind:value={folderSourceInput} placeholder="https://gist.githubusercontent.com/.../fshare_folder_list.txt" />
                <Button class="upload-button" variant="ghost" size="sm" title="Upload local .txt list" onclick={() => sourceFileInput?.click()}><span class="material-icons" aria-hidden="true">upload_file</span><span class="sr-only">Upload local .txt list</span></Button>
                <input bind:this={sourceFileInput} class="source-file-input" type="file" accept=".txt,text/plain" onchange={(event) => stageSourceFile(event.currentTarget.files)} />
              </div>
              <small class="source-helper">{stagedSourceName
                  ? `${stagedSourceName} staged · remote URL retained`
                  : $folderImportedSourceCount
                    ? `${$folderImportedSourceCount} local source${$folderImportedSourceCount === 1 ? "" : "s"} stored · remote URL optional`
                    : "Remote text-file URL · local .txt optional"}</small>
              {#if folderSourceMessage}
                <p class="settings-result" class:success={folderSourceMessage.includes("Imported")} class:error={!folderSourceMessage.includes("Imported")} role={folderSourceMessage.includes("Imported") ? "status" : "alert"}>{folderSourceMessage}</p>
              {/if}
              <div class="fshare-actions">
                <Button size="sm" accent="#ef5350" width="100%" loading={savingFolderSources} disabled={savingFolderSources || (!stagedSourceText.trim() && folderSourceInput.trim() === savedFolderSourceUrl)} onclick={updateFolderSources}>UPDATE</Button>
                <Button size="sm" variant="ghost" width="100%" loading={startingCacheSync} onclick={startCacheSync}>SYNC</Button>
              </div>
              <Button class="singular-sync" accent="#ef5350" icon="autorenew" size="sm" width="100%" onclick={() => (refreshModalOpen = true)}>SINGULAR SYNC</Button>
            </div>
          </article>
        </aside>
        <main class="sources-main"><StremioAddonsSettings /></main>
      </div>
    </div>
  {:else}
    <div id="settings-tabpanel-logs" class="settings-panel logs-panel" role="tabpanel" aria-labelledby="settings-tab-logs">
      <SettingsLogPanel {logs} {activities} onClearLogs={clearLogs} bind:activeCategory={logCategory} />
    </div>
  {/if}
</div>

<FolderCacheSyncModal open={cacheModalOpen} status={folderCacheStatus} onclose={() => (cacheModalOpen = false)} />
<FolderCacheRefreshModal open={refreshModalOpen} onclose={() => (refreshModalOpen = false)} onsubmit={refreshSingleFolder} />
<SwitchAccountModal open={addAccountOpen} onclose={() => (addAccountOpen = false)} onswitched={reloadAccounts} />

<Modal open={!!pendingRemoval} onClose={() => !removingAccount && (pendingRemoval = null)} maxWidth="380px" accent="var(--cui-danger, #ef4444)" ariaLabel="Remove account">
  {#snippet header()}
    <p class="remove-modal-title">REMOVE {pendingRemoval?.provider === "torbox" ? "TORBOX" : "FSHARE"} ACCOUNT</p>
  {/snippet}
  {#snippet children()}
    <p class="remove-modal-copy">{pendingRemoval?.provider === "torbox"
        ? "Remove the TorBox configuration? Flasharr will lose TorBox access until configured again."
        : `Remove ${pendingRemoval?.email}? Flasharr will lose access until this account is added again.`}</p>
    <div class="remove-modal-actions">
      <Button size="sm" variant="ghost" disabled={removingAccount} onclick={() => (pendingRemoval = null)}>Cancel</Button>
      <Button size="sm" variant="danger" loading={removingAccount} onclick={removeAccount}>Remove</Button>
    </div>
  {/snippet}
</Modal>

<Modal open={passwordModalOpen} onClose={closePasswordModal} maxWidth="480px" accent="var(--cui-primary)" ariaLabel="Change administrator password">
  {#snippet header()}
    <div class="general-modal-head"><div><p class="modal-title">CHANGE PASSWORD</p><p class="modal-sub">All active sessions will be signed out.</p></div><button class="close-btn" aria-label="Close" disabled={changingPassword} onclick={closePasswordModal}><span class="material-icons">close</span></button></div>
  {/snippet}
  <form
    class="general-form"
    onsubmit={(event) => {
      event.preventDefault();
      changePassword();
    }}
  ><label class="field-label" for="administrator-current-password">CURRENT PASSWORD</label><div class="general-input"><input id="administrator-current-password" name="current-password" type="password" bind:value={currentPassword} autocomplete="current-password" required /></div><label class="field-label" for="administrator-new-password">NEW PASSWORD</label><div class="general-input"><input id="administrator-new-password" name="new-password" type="password" bind:value={newPassword} autocomplete="new-password" minlength="12" required /></div><label class="field-label" for="administrator-confirm-password">CONFIRM NEW PASSWORD</label><div class="general-input"><input id="administrator-confirm-password" name="confirm-password" type="password" bind:value={confirmPassword} autocomplete="new-password" minlength="12" required /></div>{#if passwordError}<p class="general-error" role="alert">{passwordError}</p>{/if}<div class="general-form-actions"><Button variant="ghost" disabled={changingPassword} onclick={closePasswordModal}>Cancel</Button><Button type="submit" loading={changingPassword} disabled={changingPassword || !currentPassword || newPassword.length < 12 || !confirmPassword}>Change password</Button></div></form>
</Modal>

<Modal open={!!editingProvider} onClose={closeProviderEditor} maxWidth="480px" accent="var(--cui-gold)" ariaLabel="Edit metadata provider API key">
  {#snippet header()}
    <div class="general-modal-head"><div><p class="modal-title">EDIT {editingProvider?.toUpperCase()} API KEY</p><p class="modal-sub">Test before saving. Saved credentials apply server-wide.</p></div><button class="close-btn" aria-label="Close" disabled={savingProvider || testingProvider} onclick={closeProviderEditor}><span class="material-icons">close</span></button></div>
  {/snippet}
  {#snippet children()}
    <form
      class="general-form"
      onsubmit={(event) => {
        event.preventDefault();
        saveProviderKey();
      }}
    ><p class="field-label">CURRENT KEY <span class="mono">{editingProvider ? providerRow(editingProvider).maskedKey || "Not configured" : ""}</span></p><label class="field-label" for="general-provider-key">NEW API KEY</label><div class="general-input"><input id="general-provider-key" type={showProviderKey ? "text" : "password"} bind:value={providerKeyInput} placeholder="Paste a replacement key" autocomplete="off" /><button type="button" aria-label={showProviderKey ? "Hide API key" : "Reveal API key"} onclick={() => (showProviderKey = !showProviderKey)}><span class="material-icons">{showProviderKey ? "visibility_off" : "visibility"}</span></button></div>{#if providerError}<p class="general-error" role="alert">{providerError}</p>{:else if providerFeedback}<p class="general-feedback" role="status">{providerFeedback}</p>{/if}<div class="general-form-actions"><Button size="sm" variant="ghost" disabled={savingProvider || testingProvider} onclick={closeProviderEditor}>Cancel</Button><Button size="sm" variant="ghost" loading={testingProvider} disabled={savingProvider || testingProvider} onclick={testProviderKey}>Test</Button><Button type="submit" size="sm" accent="var(--cui-gold)" loading={savingProvider} disabled={savingProvider || testingProvider || !providerKeyInput.trim()}>Save</Button></div></form>
  {/snippet}
</Modal>

<style>
  :global(.settings-tabs-header) {
    display: flex;
    gap: .15rem;
    background: #00000059;
    border: 1px solid rgba(255,255,255,.06);
    border-radius: 10px;
    padding: .2rem;
    box-shadow: inset 0 2px 4px #00000080;
  }

  :global(.settings-tabs-header .toggle-btn) {
    min-height: 44px;
    padding: .45rem 1rem;
    border-radius: 8px;
    color: #ffffff61;
    background: transparent;
    border: none;
    cursor: pointer;
    transition: all .22s cubic-bezier(.2,.8,.2,1);
    font-size: .78rem;
    font-family: var(--font-mono,monospace);
  }

  :global(.settings-tabs-header .toggle-btn:focus-visible) {
    outline: 2px solid var(--cui-primary);
    outline-offset: 2px;
  }

  :global(.settings-tabs-header .toggle-btn:hover:not(.active)) {
    color: #ffffffbf;
  }

  :global(.settings-tabs-header .toggle-btn.active) {
    background: #00f3ff24;
    color: #00f3ff;
    box-shadow: 0 0 12px #00f3ff2e,inset 0 1px #00f3ff26;
    text-shadow: 0 0 8px rgba(0,243,255,.5);
  }

  :global(.settings-tabs-header .toggle-btn .material-icons) {
    font-size: 1.15rem;
    margin-right: .35rem;
    vertical-align: middle;
  }

  .settings-viewport {
    height: 100%;
    min-width: 0;
    overflow: hidden;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    color: var(--cui-text-primary,#f4f7fb);
  }

  .settings-loading,
  .settings-load-error {
    margin: auto;
    color: var(--cui-text-secondary);
    font-size: var(--cui-text-sm);
  }

  .settings-load-error {
    display: flex;
    align-items: center;
    gap: var(--cui-space-sm);
    color: var(--cui-danger);
  }

  .settings-panel {
    flex: 1;
    min-height: 0;
    overflow: auto;
    animation: slideUp .22s ease-out;
  }

  .general-settings {
    display: grid;
    grid-template-columns: repeat(2,minmax(0,1fr));
    grid-auto-rows: minmax(260px,auto);
    gap: var(--cui-space-md);
    max-width: 1180px;
    width: 100%;
    margin: 0 auto;
    padding-bottom: var(--cui-space-xl);
    overflow: auto;
  }

  .bento-general {
    --card-accent: var(--cui-primary);
    display: flex;
    min-width: 0;
    min-height: 0;
    flex-direction: column;
    border-color: color-mix(in srgb,var(--cui-primary) 35%,var(--cui-glass-border));
    box-shadow: var(--cui-shadow-glass),inset 3px 0 color-mix(in srgb,var(--cui-primary) 70%,transparent);
  }

  .general-card-body {
    display: flex;
    min-height: 0;
    flex: 1;
    flex-direction: column;
    gap: var(--cui-space-md);
    padding: var(--cui-space-lg);
  }

  .mono {
    font-family: var(--cui-font-mono,monospace);
    font-size: .72rem;
  }

  .provider-grid {
    display: grid;
    grid-template-columns: repeat(2,minmax(0,1fr));
    gap: var(--cui-space-sm);
  }

  .provider-tile {
    display: grid;
    grid-template-columns: 64px minmax(0,1fr) 44px;
    align-items: center;
    gap: var(--cui-space-sm);
    min-width: 0;
    min-height: 100px;
    padding: var(--cui-space-sm);
    border: 1px solid color-mix(in srgb,var(--cui-primary) 28%,var(--cui-glass-border));
    background: color-mix(in srgb,var(--cui-primary) 5%,var(--cui-surface-2));
    box-sizing: border-box;
  }

  .provider-logo {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    padding: var(--cui-space-xs);
    box-sizing: border-box;
  }

  .provider-logo img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .provider-copy {
    display: flex;
    min-width: 0;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--cui-space-xs);
  }

  .provider-copy b {
    color: var(--cui-text-primary);
    font-size: var(--cui-text-md);
  }

  .provider-status {
    display: inline-flex;
    align-items: center;
    gap: var(--cui-space-xs);
    color: var(--cui-warning);
    font: 700 .68rem/1 var(--cui-font-mono,monospace);
  }

  .provider-status.connected {
    color: var(--cui-success);
  }

  .provider-status.unavailable {
    color: var(--cui-danger);
  }

  .provider-status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: currentColor;
    box-shadow: 0 0 7px currentColor;
  }

  :global(.metadata-edit.flasharr-btn) {
    display: grid;
    width: 44px;
    min-width: 44px;
    height: 44px;
    place-items: center;
    padding: 0;
    border-color: color-mix(in srgb,var(--cui-primary) 30%,var(--cui-glass-border));
    color: var(--cui-primary);
  }

  :global(.metadata-edit.flasharr-btn .material-icons) {
    font-size: 1rem;
  }

  .provider-attribution {
    margin: auto 0 0;
    color: var(--cui-text-tertiary);
    font-size: .65rem;
    line-height: 1.5;
  }

  .provider-attribution a {
    color: var(--cui-primary);
  }

  .storage-status {
    display: flex;
    align-items: center;
    gap: var(--cui-space-md);
    min-width: 0;
  }

  .storage-status > .material-icons {
    font-size: 2rem;
    color: var(--cui-primary);
  }

  .storage-status div {
    min-width: 0;
  }

  .storage-status b,
  .storage-status small {
    display: block;
  }

  .storage-status small {
    margin-top: var(--cui-space-xxs);
    overflow: hidden;
    color: var(--cui-text-tertiary);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .storage-metrics {
    display: grid;
    grid-template-columns: repeat(2,minmax(0,1fr));
    gap: var(--cui-space-sm);
  }

  .storage-metrics div,
  .runtime-grid > div {
    padding: var(--cui-space-md);
    border: 1px solid color-mix(in srgb,var(--cui-primary) 22%,var(--cui-glass-border));
    background: color-mix(in srgb,var(--cui-primary) 4%,transparent);
  }

  .storage-metrics small,
  .runtime-grid small,
  .administrator-copy small {
    display: block;
    color: var(--cui-text-tertiary);
    font: 800 .62rem/1 var(--cui-font-mono,monospace);
    letter-spacing: .08em;
  }

  .storage-metrics b,
  .runtime-grid b {
    display: block;
    margin-top: var(--cui-space-xs);
    color: var(--cui-primary);
    font-size: var(--cui-text-sm);
  }

  .storage-meter {
    height: 6px;
    background: var(--cui-surface-hover);
  }

  .storage-meter span {
    display: block;
    height: 100%;
    background: var(--cui-primary);
  }

  .storage-caption {
    color: var(--cui-text-tertiary);
    font-size: var(--cui-text-xs);
  }

  .administrator-card {
    display: grid;
    grid-template-columns: 72px minmax(0,1fr);
    align-items: center;
  }

  .administrator-avatar {
    display: grid;
    width: 72px;
    height: 72px;
    place-items: center;
    border: 1px solid color-mix(in srgb,var(--cui-primary) 40%,var(--cui-glass-border));
    background: color-mix(in srgb,var(--cui-primary) 9%,transparent);
    color: var(--cui-primary);
  }

  .administrator-avatar .material-icons {
    font-size: 2rem;
  }

  .administrator-copy {
    display: flex;
    min-width: 0;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--cui-space-xs);
  }

  .administrator-copy b {
    font-size: var(--cui-text-lg);
  }

  :global(.administrator-action.flasharr-btn) {
    grid-column: 1/-1;
    width: 100%;
    margin-top: auto;
  }

  .runtime-grid {
    display: grid;
    grid-template-columns: repeat(2,minmax(0,1fr));
    align-content: start;
  }

  .general-card-body.runtime-grid {
    padding: var(--cui-space-lg);
  }

  .general-modal-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--cui-space-md);
  }

  .modal-title {
    margin: 0;
    color: var(--cui-text-primary);
    font: 800 .72rem/1 var(--cui-font-mono,monospace);
    letter-spacing: .1em;
  }

  .modal-sub {
    margin: var(--cui-space-xxs) 0 0;
    color: var(--cui-text-secondary);
    font-size: var(--cui-text-xs);
  }

  .general-form {
    display: flex;
    flex-direction: column;
    gap: var(--cui-space-sm);
  }

  .field-label {
    color: var(--cui-text-tertiary);
    font: 800 .64rem/1 var(--cui-font-mono,monospace);
    letter-spacing: .08em;
  }

  .field-label .mono {
    float: right;
    color: var(--cui-primary);
  }

  .general-input {
    display: flex;
    align-items: center;
    border: 1px solid var(--cui-glass-border);
    background: var(--cui-input-bg);
  }

  .general-input:focus-within {
    border-color: var(--cui-gold);
    box-shadow: 0 0 0 2px color-mix(in srgb,var(--cui-gold) 16%,transparent);
  }

  .general-input input {
    flex: 1;
    min-width: 0;
    padding: .7rem .8rem;
    border: 0;
    background: transparent;
    outline: 0;
    color: var(--cui-text-primary);
    font: var(--cui-text-sm) var(--cui-font-body);
  }

  .general-input button {
    display: grid;
    place-items: center;
    flex: 0 0 44px;
    width: 44px;
    height: 44px;
    margin-right: var(--cui-space-xs);
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--cui-text-secondary);
    cursor: pointer;
  }

  .general-input button:hover {
    color: var(--cui-gold);
  }

  .general-input .material-icons {
    font-size: 1rem;
  }

  .general-form-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--cui-space-xs);
    margin-top: var(--cui-space-sm);
  }

  .general-feedback,
  .general-error {
    margin: 0;
    font-size: var(--cui-text-xs);
  }

  .general-feedback {
    color: var(--cui-success);
  }

  .general-error {
    color: var(--cui-danger);
  }

  @media (max-width:900px) {
    .general-settings {
      grid-template-columns: 1fr;
      grid-auto-rows: auto;
      overflow: visible;
    }
  }

  @media (max-width:768px) {
    .general-settings {
      padding-bottom: var(--cui-space-lg);
    }
    .general-card-body {
      padding: var(--cui-space-md);
    }
  }

  @media (max-width:520px) {
    .provider-grid {
      grid-template-columns: 1fr;
    }
    .provider-tile {
      grid-template-columns: 56px minmax(0,1fr) 44px;
    }
    .provider-logo {
      width: 56px;
      height: 56px;
    }
    .storage-metrics,
    .runtime-grid {
      grid-template-columns: 1fr;
    }
    .general-form-actions {
      flex-wrap: wrap;
      justify-content: flex-start;
    }
  }

  .eyebrow {
    margin: 0 0 .45rem;
    color: var(--cui-primary,#22d3ee);
    font: 700 .68rem/1 var(--cui-font-mono,monospace);
    letter-spacing: .14em;
  }

  .settings-card h2 {
    margin: 0 0 .45rem;
    font-size: 1.05rem;
  }

  .settings-card p,
  .panel-heading p {
    color: var(--cui-text-secondary,#91a0ae);
    font-size: .82rem;
    line-height: 1.5;
  }

  .managed {
    margin-top: 1.25rem;
    display: flex;
    gap: .7rem;
    align-items: center;
    padding: .8rem;
    border-radius: .7rem;
    background: #ffffff09;
  }

  .managed small {
    display: block;
    color: var(--cui-text-tertiary,#71808e);
    font: 600 .64rem/1.5 var(--cui-font-mono,monospace);
  }

  .panel-heading {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .panel-actions {
    display: flex;
    gap: .5rem;
  }

  .account-settings {
    padding: .75rem;
    box-sizing: border-box;
  }

  .account-list {
    display: flex;
    flex-direction: column;
    gap: .75rem;
    max-height: calc(100% - 4.5rem);
    overflow: auto;
    padding-top: .75rem;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: .75rem;
  }

  :global(.account-add-button.flasharr-btn) {
    min-height: 44px;
    padding-inline: 1.15rem;
    font-size: .82rem;
    font-weight: 800;
  }

  .account-row-card {
    --account-accent: #ef4444;
    position: relative;
    isolation: isolate;
    display: grid;
    grid-template-columns: 42px calc(40% - 42px) 20% 25% 15%;
    align-items: center;
    column-gap: 0;
    min-height: 86px;
    box-sizing: border-box;
    padding: 1rem 1.25rem;
    border: 1px solid color-mix(in srgb,var(--account-accent) 35%,transparent);
    border-radius: 12px;
    background: linear-gradient(110deg,color-mix(in srgb,var(--account-accent) 9%,var(--cui-surface-1,#0e151c)),var(--cui-surface-1,#0e151c));
    box-shadow: inset 0 1px #ffffff0a;
    overflow: visible;
    transition: transform .25s cubic-bezier(.16,1,.3,1),border-color .25s,box-shadow .35s;
  }

  .torbox-account {
    --account-accent: #22c55e;
  }

  .account-row-card:before {
    content: "";
    position: absolute;
    z-index: -1;
    inset: 0;
    border-radius: inherit;
    opacity: .22;
    background-image: radial-gradient(color-mix(in srgb,var(--account-accent) 55%,transparent) 1px,transparent 1px);
    background-size: 12px 12px;
    mask-image: linear-gradient(90deg,#000,transparent 70%);
    pointer-events: none;
  }

  .account-row-card.inactive {
    opacity: .72;
  }

  .account-row-card:hover {
    transform: translateY(-3px);
    border-color: color-mix(in srgb,var(--account-accent) 80%,transparent);
    box-shadow: 0 8px 32px #00000080,0 0 28px color-mix(in srgb,var(--account-accent) 25%,transparent);
  }

  :global(.account-status-badge) {
    position: absolute;
    top: 0;
    left: 1.25rem;
    transform: translateY(-50%);
    border: 1px solid var(--account-accent) !important;
    background: var(--cui-surface-0,#0a1016) !important;
    color: var(--account-accent) !important;
    font: 700 .6rem/1 var(--cui-font-mono,monospace);
    letter-spacing: .08em;
    z-index: 1;
  }

  .account-provider {
    position: relative;
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
  }

  .account-provider img {
    max-width: 36px;
    max-height: 36px;
    object-fit: contain;
    filter: drop-shadow(0 0 8px color-mix(in srgb,var(--account-accent) 80%,transparent));
  }

  .account-online-dot {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 9px;
    height: 9px;
    border: 2px solid var(--cui-surface-1,#0e151c);
    border-radius: 50%;
    background: var(--cui-success,#22c55e);
    box-shadow: 0 0 8px var(--cui-success,#22c55e);
  }

  .account-identity {
    min-width: 0;
    padding-left: 1rem;
  }

  .account-email {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--cui-text-primary,#f4f7fb);
    font-size: 1rem;
  }

  .account-identity small,
  .account-detail span {
    display: block;
    color: var(--cui-text-tertiary,#71808e);
    font: 700 .6rem/1.4 var(--cui-font-mono,monospace);
    letter-spacing: .08em;
  }

  .account-detail {
    padding-left: 1rem;
    border-left: 1px solid color-mix(in srgb,var(--account-accent) 22%,transparent);
  }

  .account-detail b {
    display: block;
    margin-top: .2rem;
    color: var(--account-accent);
    font-size: .9rem;
  }

  .account-actions {
    display: flex;
    gap: .35rem;
    justify-self: end;
  }

  .account-actions :global(.account-icon-action) {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    min-width: 44px;
    padding: 0;
    border: 1px solid var(--cui-glass-border,rgba(255,255,255,.1));
    border-radius: .5rem;
  }

  .account-actions :global(.account-icon-action:hover) {
    border-color: var(--account-accent);
    color: var(--account-accent);
  }

  .account-actions :global(.account-icon-action.danger:hover) {
    border-color: var(--cui-danger,#ef4444);
    color: var(--cui-danger,#ef4444);
  }

  .account-actions :global(.account-icon-action:disabled) {
    opacity: .55;
    cursor: wait;
  }

  .account-actions :global(.account-icon-action) :global(.material-icons) {
    font-size: 1rem;
  }

  .account-action-status {
    position: absolute;
    right: 1.25rem;
    bottom: .25rem;
    color: var(--cui-text-secondary);
    font-size: .62rem;
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

  @media (max-width:900px) {
    .account-row-card {
      grid-template-columns: 42px minmax(0,1fr) auto;
      grid-template-areas: "provider identity delete" "provider plan expiry";
      gap: .6rem 1rem;
      min-height: 104px;
    }
    .account-provider {
      grid-area: provider;
    }
    .account-identity {
      grid-area: identity;
    }
    .account-plan-detail {
      grid-area: plan;
    }
    .account-expiry-detail {
      grid-area: expiry;
    }
    .account-actions {
      grid-area: delete;
    }
    .account-detail {
      padding-left: 0;
      border-left: 0;
    }
  }

  @media (max-width:560px) {
    .account-heading {
      align-items: flex-start;
      gap: .75rem;
    }
    .panel-actions {
      flex-wrap: wrap;
      justify-content: flex-end;
    }
    .account-row-card {
      grid-template-columns: 40px minmax(0,1fr) auto;
      min-height: 112px;
      padding: 1rem;
    }
    .account-provider {
      width: 40px;
      height: 40px;
    }
    .account-detail b {
      font-size: .72rem;
    }
    :global(.account-status-badge) {
      left: 1rem;
    }
  }

  .download-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
    overflow: hidden;
  }

  .bento-download {
    --card-accent: rgba(0,243,255,.5);
  }

  .bento-download .bento-head h2 {
    flex: 1;
    margin: 0;
    color: #fff;
    font-size: .75rem;
    font-weight: 800;
    letter-spacing: .12em;
  }

  .cyan-accent {
    border-left-color: #00f3ff8c;
  }

  .cyan-banner {
    background: linear-gradient(135deg,#00f3ff26,#00648c0f 50%,#0a0a0f80);
  }

  .download-logo img {
    filter: drop-shadow(0 0 10px rgba(0,243,255,.7)) drop-shadow(0 0 20px rgba(0,243,255,.3));
  }

  .dl-body {
    gap: 0;
    padding: 0;
  }

  .dl-section {
    padding: .85rem;
  }

  .dl-section--sep {
    border-top: 1px solid rgba(255,255,255,.05);
  }

  .dl-section--conc {
    display: flex;
    flex: 1;
    flex-direction: column;
  }

  .dl-path-field,
  .dl-conc-hud {
    display: flex;
    flex-direction: column;
    gap: .6rem;
  }

  .dl-path-label,
  .dl-conc-label {
    display: flex;
    align-items: center;
    gap: .35rem;
    color: var(--cui-text-tertiary);
    font: 700 .68rem/1 var(--cui-font-mono,monospace);
    letter-spacing: .12em;
  }

  .dl-path-label .material-icons {
    color: var(--cui-primary);
    font-size: 1rem;
  }

  .dl-path-input {
    width: 100%;
    box-sizing: border-box;
    padding: .65rem;
    border: 1px solid color-mix(in srgb,var(--cui-primary) 18%,transparent);
    border-radius: 8px;
    outline: none;
    background: color-mix(in srgb,var(--cui-primary) 3%,transparent);
    color: var(--cui-text-primary);
    font: .82rem var(--cui-font-mono,monospace);
  }

  .dl-path-input:focus-visible {
    border-color: color-mix(in srgb,var(--cui-primary) 55%,transparent);
    box-shadow: 0 0 0 2px color-mix(in srgb,var(--cui-primary) 12%,transparent);
  }

  .dl-dot-slider {
    position: relative;
    display: flex;
    align-items: center;
    height: 48px;
    cursor: col-resize;
    touch-action: none;
    outline: none;
    user-select: none;
  }

  .dl-dot-slider:focus-visible {
    outline: 2px solid var(--cui-primary);
    outline-offset: 2px;
    border-radius: 24px;
  }

  .dl-dot-slider:before {
    content: "";
    position: absolute;
    right: 22px;
    left: 22px;
    top: 50%;
    height: 2px;
    border-radius: 2px;
    background: linear-gradient(to right,#00f3ff80 0% calc(var(--pct) * 100%),#ffffff14 calc(var(--pct) * 100%));
    transform: translateY(-50%);
  }

  .dl-dots {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }

  .dl-dot {
    position: absolute;
    top: 50%;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #ffffff1f;
    transform: translateY(-50%);
  }

  .dl-dot.lit {
    background: var(--cui-primary);
    box-shadow: 0 0 9px color-mix(in srgb,var(--cui-primary) 75%,transparent);
  }

  .dl-dot.hidden {
    opacity: 0;
  }

  .dl-thumb-orb {
    position: absolute;
    z-index: 1;
    top: 50%;
    left: calc(var(--pct) * (100% - 40px));
    display: grid;
    width: 40px;
    height: 40px;
    place-items: center;
    border: 2px solid var(--cui-primary);
    border-radius: 50%;
    background: radial-gradient(circle at 38% 32%,#b4ffff38,#00507814);
    box-shadow: 0 0 16px color-mix(in srgb,var(--cui-primary) 35%,transparent);
    color: var(--cui-primary);
    font: 900 1rem var(--cui-font-mono,monospace);
    transform: translateY(-50%);
    pointer-events: none;
  }

  .dl-body > .settings-result {
    padding: 0 .85rem;
  }

  .dl-body > .compact-actions {
    padding: .4rem .85rem .85rem;
  }

  .dl-body > .compact-actions :global(.flasharr-btn) {
    min-height: 44px;
  }

  .integrations-panel {
    display: flex;
    align-items: center;
    overflow: hidden;
  }

  .integrations-panel :global(.integration-grid) {
    width: 100%;
    height: 72%;
    margin-inline: auto;
  }

  .config-card {
    display: flex;
    flex-direction: column;
    gap: .7rem;
    overflow: hidden;
  }

  .card-actions {
    display: flex;
    justify-content: flex-end;
    gap: .5rem;
    margin-top: auto;
  }

  .primary-link {
    align-self: flex-start;
    padding: .65rem .9rem;
    border-radius: .55rem;
    background: var(--cui-primary,#22d3ee);
    color: #041014;
    font: 800 .7rem var(--cui-font-mono,monospace);
    text-decoration: none;
    text-transform: uppercase;
  }

  .sources-panel {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .sources-layout {
    display: grid;
    grid-template-columns: minmax(0,35fr) minmax(0,65fr);
    grid-template-rows: auto;
    align-items: stretch;
    gap: 1rem;
    min-height: 0;
    overflow: hidden;
  }

  .sources-rail {
    display: flex;
    min-width: 0;
    min-height: 0;
    flex-direction: column;
    gap: 1rem;
  }

  .sources-main {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    contain: size;
  }

  .bento-prowlarr {
    --card-accent: rgba(249,115,22,.5);
  }

  .sources-rail .bento-head,
  .bento-download .bento-head {
    position: relative;
    overflow: hidden;
    border-left: 3px solid var(--card-accent);
    font-family: var(--cui-font-mono,monospace);
  }

  .sources-rail .bento-head:after,
  .bento-download .bento-head:after,
  .banner-dots {
    content: "";
    position: absolute;
    inset: 0;
    background-image: radial-gradient(rgba(0,0,0,.25) 1px,transparent 1px);
    background-size: 6px 6px;
  }

  .sources-rail .bento-head:after,
  .bento-download .bento-head:after {
    mask-image: linear-gradient(to right,#000,transparent 70%);
  }

  .sources-rail .bento-head > *,
  .bento-download .bento-head > * {
    position: relative;
    z-index: 1;
  }

  .bento-head h2 {
    flex: 1;
    margin: 0;
    color: #fff;
    font-size: .7rem;
    font-weight: 900;
    letter-spacing: .1em;
  }

  .bento-head .head-logo {
    width: 22px;
    height: 22px;
    object-fit: contain;
    filter: drop-shadow(0 0 6px rgba(249,115,22,.55));
  }

  .card-banner {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    height: 72px;
    flex-shrink: 0;
  }

  .orange-banner {
    background: linear-gradient(135deg,#f9731626,#c2410c0f 50%,#0a0a0f80);
  }

  .banner-logo-ring {
    position: relative;
    z-index: 1;
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
  }

  .banner-logo-ring > img {
    width: 44px;
    height: 44px;
    object-fit: contain;
    filter: drop-shadow(0 0 10px rgba(249,115,22,.7)) drop-shadow(0 0 20px rgba(249,115,22,.35));
  }

  .banner-status-dot {
    position: absolute;
    right: -3px;
    bottom: -3px;
    width: 11px;
    height: 11px;
    border: 2px solid rgba(8,10,15,.95);
    border-radius: 50%;
    background: #50505ad9;
  }

  .banner-status-dot.active {
    background: #22c55e;
    box-shadow: 0 0 8px #22c55ed9;
  }

  .banner-status-dot.unreachable {
    background: #ef4444;
    box-shadow: 0 0 8px #ef4444bf;
  }

  .reload-config {
    width: fit-content;
    padding: .3rem 0;
    border: 0;
    background: none;
    color: #fbbf24;
    font-size: var(--cui-text-xs);
    text-align: left;
    cursor: pointer;
  }

  .reload-config:focus-visible {
    outline: 2px solid var(--cui-primary);
    outline-offset: 2px;
  }

  .bento-body {
    display: flex;
    flex: 1;
    min-height: 0;
    flex-direction: column;
    padding: .6rem .85rem;
  }

  .bento-body .inline-field {
    display: grid;
    grid-template-columns: minmax(0,1fr) auto;
    align-items: center;
    gap: .4rem;
    padding: .55rem 0;
    border-bottom: 1px solid rgba(255,255,255,.03);
  }

  .bento-body .inline-label {
    grid-column: 1/-1;
    color: #ffffff59;
    font: 700 .72rem/1 var(--cui-font-mono,monospace);
    letter-spacing: .1em;
  }

  .bento-body .inline-input-edit {
    grid-column: 1;
    min-width: 0;
    padding: .05rem .2rem;
    border: 0;
    border-bottom: 1px solid rgba(255,255,255,.06);
    outline: none;
    background: transparent;
    color: #fffc;
    font-size: .8rem;
  }

  .bento-body .inline-input-edit:focus-visible {
    outline: 2px solid #f97316;
    outline-offset: 2px;
  }

  .compact-actions {
    display: flex;
    gap: .4rem;
    margin-top: auto;
    padding-top: .6rem;
  }

  .compact-actions > div {
    display: flex;
  }

  .compact-actions > div:first-child {
    flex: 4;
  }

  .compact-actions > div:last-child {
    flex: 6;
  }

  .compact-actions :global(.flasharr-btn) {
    height: 44px;
    min-height: 44px;
    box-sizing: border-box;
  }

  .bento-fshare {
    --card-accent: rgba(239,83,80,.5);
  }

  .fshare-accent {
    border-left-color: #ef535080;
  }

  .fshare-banner {
    background: linear-gradient(135deg,#ef535026,#b423280f 50%,#0a0a0f80);
  }

  .fshare-banner .banner-logo-ring > img {
    filter: drop-shadow(0 0 10px rgba(239,83,80,.7)) drop-shadow(0 0 20px rgba(239,83,80,.35));
  }

  .fshare-body {
    gap: .55rem;
  }

  .cache-summary {
    margin: 0;
    padding: .55rem 0;
    border-top: 1px solid rgba(239,83,80,.2);
    border-bottom: 1px solid rgba(239,83,80,.2);
    color: var(--cui-text-secondary);
    font-size: .7rem;
    line-height: 1.5;
  }

  .cache-summary.stale,
  .settings-result.error {
    color: var(--cui-danger);
  }

  .source-entry {
    display: grid;
    grid-template-columns: minmax(0,1fr) auto;
    gap: .45rem;
    align-items: stretch;
  }

  .source-entry > input {
    min-width: 0;
    height: 32px;
    box-sizing: border-box;
    padding: 0 .65rem;
    border: 1px solid rgba(239,83,80,.25);
    border-radius: 8px;
    background: var(--cui-surface-0);
    color: var(--cui-text-primary);
    font: var(--cui-text-xs) var(--cui-font-body);
  }

  .source-entry > input:focus-visible {
    outline: 2px solid #ef5350;
    outline-offset: 2px;
  }

  :global(.upload-button.flasharr-btn) {
    display: grid;
    place-items: center;
    width: 36px;
    min-width: 36px;
    height: 32px;
    min-height: 32px;
    padding: 0;
    border-color: #ef535059;
    color: #ff8a87;
  }

  :global(.upload-button.flasharr-btn .material-icons) {
    font-size: 1rem;
  }

  .source-file-input {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }

  .source-helper {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--cui-text-tertiary);
    font-size: .65rem;
  }

  .fshare-actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: .45rem;
    margin-top: auto;
  }

  .fshare-actions :global(.flasharr-btn),
  :global(.singular-sync.flasharr-btn) {
    height: 44px;
    min-height: 44px;
  }

  :global(.singular-sync.flasharr-btn) {
    position: relative;
    overflow: hidden;
  }

  :global(.singular-sync.flasharr-btn:before) {
    content: "";
    position: absolute;
    inset: 0;
    background-image: radial-gradient(rgba(80,0,0,.45) 1px,transparent 1px);
    background-size: 6px 6px;
    pointer-events: none;
    mask-image: linear-gradient(to right,#000,transparent 70%);
  }

  :global(.singular-sync.flasharr-btn>*) {
    position: relative;
    z-index: 1;
  }

  .settings-result {
    margin: 0;
    font-size: var(--cui-text-xs);
    color: var(--cui-text-secondary);
  }

  .settings-result.success {
    color: var(--cui-success);
  }

  .logs-panel {
    overflow: hidden;
    display: flex;
  }

  .logs-panel :global(.log-panel) {
    height: 100%;
    width: 100%;
  }

  .empty-state {
    text-align: center;
    padding: 5rem 1rem;
    color: var(--cui-text-secondary,#91a0ae);
  }

  .empty-state .material-icons {
    font-size: 2.5rem;
    color: var(--cui-primary,#22d3ee);
  }

  .remove-modal-title {
    margin: 0;
    font: 700 .75rem/1 var(--cui-font-mono,monospace);
    letter-spacing: .1em;
    color: #fff;
  }

  .remove-modal-copy {
    margin: 0 0 1.25rem;
    color: var(--cui-text-secondary,#91a0ae);
    font-size: .85rem;
    line-height: 1.5;
  }

  .remove-modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: .5rem;
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @media (prefers-reduced-motion:reduce) {
    .settings-panel,
    .account-row-card,
    .dl-dot,
    .dl-thumb-orb,
    :global(.settings-tabs-header .toggle-btn) {
      animation: none;
      transition: none;
    }
    .account-row-card:hover {
      transform: none;
    }
  }

  @media (max-width:900px) {
    .settings-viewport {
      overflow: auto;
      height: auto;
      min-height: 100%;
    }
    .settings-panel {
      flex: none;
      overflow: visible;
    }
    .integrations-panel :global(.integration-grid) {
      height: auto;
    }
    .sources-panel {
      overflow: visible;
    }
    .sources-layout {
      height: auto;
      overflow: visible;
      grid-template-columns: 1fr;
    }
    .sources-rail {
      display: grid;
      overflow: visible;
      grid-template-columns: repeat(2,minmax(0,1fr));
    }
    .sources-main {
      position: static;
      min-height: 0;
      overflow: visible;
      contain: none;
    }
    .sources-main :global(.stremio-surface) {
      position: static;
    }
  }

  @media (max-width:767px) {
    .settings-viewport {
      gap: .65rem;
      padding: .65rem;
    }
    .settings-panel {
      width: 100%;
      min-width: 0;
    }
    .download-grid {
      grid-template-columns: 1fr;
      overflow: visible;
    }
    .integrations-panel,
    .sources-main,
    .logs-panel {
      overflow: visible;
    }
    .logs-panel :global(.log-panel) {
      height: auto;
      min-height: 360px;
    }
    .account-list {
      max-height: none;
      overflow: visible;
    }
  }

  @media (max-width:560px) {
    .sources-rail {
      grid-template-columns: 1fr;
    }
    .sources-main {
      min-height: 0;
    }
    .source-entry {
      grid-template-columns: minmax(0,1fr) auto;
    }
    .fshare-actions {
      grid-template-columns: 1fr;
    }
  }

</style>
