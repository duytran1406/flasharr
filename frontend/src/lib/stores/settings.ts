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
 * Result of an account mutation (POST/DELETE /api/accounts, POST .../refresh).
 * `persisted` is set by the backend when the account was stored even though
 * the login check failed (the list is refreshed in that case too).
 */
export interface AccountMutationResult {
  success: boolean;
  message?: string;
  persisted?: boolean;
}

/**
 * Settings Store State
 */
interface SettingsStoreState {
  settings: Settings;
  accounts: FshareAccount[];
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
      const response = await fetch(`${API_BASE}/settings?_=${Date.now()}`);
      
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const data = await response.json();
      
      update(state => {
        const currentSettings = state.settings;
        return {
          ...state,
          settings: {
            ...currentSettings,
            download_path: data.downloads?.directory || currentSettings.download_path,
            max_concurrent_downloads: data.downloads?.max_concurrent || currentSettings.max_concurrent_downloads,
            segments_per_download: data.downloads?.segments_per_download || currentSettings.segments_per_download,
          },
          loading: false,
        };
      });
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

    } catch (err) {
      console.error('[SettingsStore] Fetch accounts error:', err);
    }
  }

  /**
   * Add (or replace) an Fshare account. Returns the backend response body.
   * The account list is refreshed when the request succeeded or the backend
   * reports the account was persisted anyway.
   */
  async function addAccount(email: string, password: string): Promise<AccountMutationResult> {
    try {
      const response = await fetch(`${API_BASE}/accounts`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email, password }),
      });
      const data: AccountMutationResult = await response
        .json()
        .catch(() => ({ success: false, message: `HTTP ${response.status}` }));
      if (response.ok || data.persisted) await fetchAccounts();
      return data;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to add account';
      console.error('[SettingsStore] Add account error:', errorMsg);
      update(state => ({ ...state, error: errorMsg }));
      return { success: false, message: errorMsg };
    }
  }

  /**
   * Remove an Fshare account. Returns the backend response body.
   */
  async function removeAccount(email: string): Promise<AccountMutationResult> {
    try {
      const response = await fetch(`${API_BASE}/accounts/${encodeURIComponent(email)}`, {
        method: 'DELETE',
      });
      const data: AccountMutationResult = await response
        .json()
        .catch(() => ({ success: false, message: `HTTP ${response.status}` }));
      if (response.ok) await fetchAccounts();
      return data;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to remove account';
      console.error('[SettingsStore] Remove account error:', errorMsg);
      return { success: false, message: errorMsg };
    }
  }

  /**
   * Re-login / refresh quota info for an account. Returns the backend response body.
   */
  async function refreshAccount(email: string): Promise<AccountMutationResult> {
    try {
      const response = await fetch(`${API_BASE}/accounts/${encodeURIComponent(email)}/refresh`, {
        method: 'POST',
      });
      const data: AccountMutationResult = await response
        .json()
        .catch(() => ({ success: false, message: `HTTP ${response.status}` }));
      if (response.ok) await fetchAccounts();
      return data;
    } catch (err) {
      console.error('[SettingsStore] Refresh account error:', err);
      return { success: false, message: 'Network error' };
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
 * Helper: Format quota (not referenced by the 2026-09-02 build)
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
