import { writable, derived } from 'svelte/store';

/**
 * Settings Interface
 */
export interface Settings {
  download_path: string;
  max_concurrent_downloads: number;
  segments_per_download: number;
  rate_limit_enabled: boolean;
  rate_limit_mbps: number;
  auto_extract: boolean;
  debug_logs: boolean;
}

/**
 * Account Interface
 */
export interface FshareAccount {
  email: string;
  password?: string;
  rank: string;
  valid_until: number;
  quota_used: number;
  quota_total: number;
  is_active: boolean;
}

/**
 * Integration Settings
 */
export interface IntegrationSettings {
  radarr_url: string;
  radarr_api_key: string;
  radarr_enabled: boolean;
  sonarr_url: string;
  sonarr_api_key: string;
  sonarr_enabled: boolean;
}

/**
 * Settings Store State
 */
interface SettingsStoreState {
  settings: Settings;
  accounts: FshareAccount[];
  integrations: IntegrationSettings;
  loading: boolean;
  error: string | null;
  saveStatus: 'idle' | 'saving' | 'saved' | 'error';
}

/**
 * Create the settings store
 */
function createSettingsStore() {
  const initialState: SettingsStoreState = {
    settings: {
      download_path: '/downloads',
      max_concurrent_downloads: 3,
      segments_per_download: 4,
      rate_limit_enabled: false,
      rate_limit_mbps: 0,
      auto_extract: false,
      debug_logs: true,
    },
    accounts: [],
    integrations: {
      radarr_url: 'http://localhost:7878',
      radarr_api_key: '',
      radarr_enabled: false,
      sonarr_url: 'http://localhost:8989',
      sonarr_api_key: '',
      sonarr_enabled: false,
    },
    loading: false,
    error: null,
    saveStatus: 'idle',
  };

  const { subscribe, set, update } = writable<SettingsStoreState>(initialState);

  /**
   * API Base URL
   */
  const API_BASE = '/api';

  /**
   * Fetch all settings
   */
  async function fetchSettings(): Promise<void> {
    update(state => ({ ...state, loading: true, error: null }));

    try {
      const response = await fetch(`${API_BASE}/settings`);
      
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const data = await response.json();
      
      update(state => ({
        ...state,
        settings: data.settings || state.settings,
        loading: false,
      }));

      console.log('[SettingsStore] Fetched settings');
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to fetch settings';
      console.error('[SettingsStore] Fetch error:', errorMsg);
      
      update(state => ({
        ...state,
        loading: false,
        error: errorMsg,
      }));
    }
  }

  /**
   * Save settings
   */
  async function saveSettings(newSettings: Partial<Settings>): Promise<boolean> {
    update(state => ({ ...state, saveStatus: 'saving' }));

    try {
      const response = await fetch(`${API_BASE}/settings`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(newSettings),
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      update(state => ({
        ...state,
        settings: { ...state.settings, ...newSettings },
        saveStatus: 'saved',
      }));

      // Reset save status after 2 seconds
      setTimeout(() => {
        update(state => ({ ...state, saveStatus: 'idle' }));
      }, 2000);

      console.log('[SettingsStore] Settings saved');
      return true;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to save settings';
      console.error('[SettingsStore] Save error:', errorMsg);
      
      update(state => ({
        ...state,
        saveStatus: 'error',
        error: errorMsg,
      }));

      setTimeout(() => {
        update(state => ({ ...state, saveStatus: 'idle' }));
      }, 2000);

      return false;
    }
  }

  /**
   * Fetch Fshare accounts
   */
  async function fetchAccounts(): Promise<void> {
    try {
      const response = await fetch(`${API_BASE}/accounts`);
      
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const data = await response.json();
      
      update(state => ({
        ...state,
        accounts: data.accounts || [],
      }));

      console.log('[SettingsStore] Fetched accounts');
    } catch (err) {
      console.error('[SettingsStore] Fetch accounts error:', err);
    }
  }

  /**
   * Add Fshare account
   */
  async function addAccount(email: string, password: string): Promise<boolean> {
    try {
      const response = await fetch(`${API_BASE}/accounts`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email, password }),
      });

      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || `HTTP ${response.status}`);
      }

      // Refresh accounts list
      await fetchAccounts();
      
      console.log('[SettingsStore] Account added');
      return true;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to add account';
      console.error('[SettingsStore] Add account error:', errorMsg);
      update(state => ({ ...state, error: errorMsg }));
      return false;
    }
  }

  /**
   * Remove Fshare account
   */
  async function removeAccount(email: string): Promise<boolean> {
    try {
      const response = await fetch(`${API_BASE}/accounts/${encodeURIComponent(email)}`, {
        method: 'DELETE',
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      // Refresh accounts list
      await fetchAccounts();
      
      console.log('[SettingsStore] Account removed');
      return true;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to remove account';
      console.error('[SettingsStore] Remove account error:', errorMsg);
      return false;
    }
  }

  /**
   * Refresh account info
   */
  async function refreshAccount(email: string): Promise<boolean> {
    try {
      const response = await fetch(`${API_BASE}/accounts/${encodeURIComponent(email)}/refresh`, {
        method: 'POST',
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      // Refresh accounts list
      await fetchAccounts();
      
      console.log('[SettingsStore] Account refreshed');
      return true;
    } catch (err) {
      console.error('[SettingsStore] Refresh account error:', err);
      return false;
    }
  }

  /**
   * Test integration connection
   */
  async function testIntegration(type: 'radarr' | 'sonarr'): Promise<{ success: boolean; message: string }> {
    try {
      const response = await fetch(`${API_BASE}/integrations/${type}/test`, {
        method: 'POST',
      });

      const data = await response.json();

      if (!response.ok) {
        return { success: false, message: data.error || 'Connection failed' };
      }

      return { success: true, message: 'Connection successful' };
    } catch (err) {
      return { success: false, message: 'Connection failed' };
    }
  }

  /**
   * Save integration settings
   */
  async function saveIntegrations(newIntegrations: Partial<IntegrationSettings>): Promise<boolean> {
    try {
      const response = await fetch(`${API_BASE}/integrations`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(newIntegrations),
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      update(state => ({
        ...state,
        integrations: { ...state.integrations, ...newIntegrations },
      }));

      console.log('[SettingsStore] Integrations saved');
      return true;
    } catch (err) {
      console.error('[SettingsStore] Save integrations error:', err);
      return false;
    }
  }

  /**
   * Check if accounts exist (for validation)
   */
   async function hasAccounts(): Promise<boolean> {
    try {
      const response = await fetch(`${API_BASE}/accounts`);
      
      if (!response.ok) {
        return false;
      }

      const data = await response.json();
      const accountsList = data.accounts || [];
      
      // Update store with fetched accounts
      update(state => ({
        ...state,
        accounts: accountsList,
      }));
      
      return accountsList.length > 0;
    } catch (err) {
      console.error('[SettingsStore] Check accounts error:', err);
      return false;
    }
  }

  return {
    subscribe,
    fetchSettings,
    saveSettings,
    fetchAccounts,
    hasAccounts,
    addAccount,
    removeAccount,
    refreshAccount,
    testIntegration,
    saveIntegrations,
  };
}

/**
 * Global settings store instance
 */
export const settingsStore = createSettingsStore();

/**
 * Derived store: Settings
 */
export const settings = derived(
  settingsStore,
  $store => $store.settings
);

/**
 * Derived store: Accounts
 */
export const accounts = derived(
  settingsStore,
  $store => $store.accounts
);

/**
 * Derived store: Integrations
 */
export const integrations = derived(
  settingsStore,
  $store => $store.integrations
);

/**
 * Derived store: Loading state
 */
export const isLoadingSettings = derived(
  settingsStore,
  $store => $store.loading
);

/**
 * Derived store: Save status
 */
export const saveStatus = derived(
  settingsStore,
  $store => $store.saveStatus
);

/**
 * Helper: Format timestamp to date
 */
export function formatExpiry(timestamp: number): string {
  const date = new Date(timestamp * 1000);
  return date.toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

/**
 * Helper: Format quota
 */
export function formatQuota(used: number, total: number): string {
  const usedGB = (used / (1024 ** 3)).toFixed(2);
  const totalGB = (total / (1024 ** 3)).toFixed(2);
  return `${usedGB} GB / ${totalGB} GB`;
}

/**
 * Helper: Calculate quota percentage
 */
export function getQuotaPercentage(used: number, total: number): number {
  if (total === 0) return 0;
  return Math.round((used / total) * 100);
}
