import { settingsStore, accounts, formatExpiry, formatQuota, getQuotaPercentage, type FshareAccount } from './settings';

/**
 * Account Store
 * Uses Svelte 5 Runes for real-time reactivity
 */
class AccountStore {
    accountsList = $state<FshareAccount[]>([]);
    
    constructor() {
        // Synchronize with the underlying settingsStore accounts (which is a Svelte 4 store)
        accounts.subscribe(val => {
            this.accountsList = val;
        });
    }

    fetch() {
        return settingsStore.fetchAccounts();
    }

    async refresh(email: string) {
        return settingsStore.refreshAccount(email);
    }

    async logout(email: string) {
        return settingsStore.removeAccount(email);
    }

    get primary() {
        if (!this.accountsList || this.accountsList.length === 0) return null;
        return this.accountsList.find(a => a.is_active) || this.accountsList[0];
    }

    /** True only when the active account has a confirmed VIP/premium rank.
     *  Returns false if rank is empty (never verified via refresh). */
    get isVip() {
        const acc = this.primary;
        if (!acc) return false;
        const rank = (acc.rank || '').toUpperCase().trim();
        if (!rank) return false; // empty = unverified, treat as non-VIP
        return rank === 'VIP' || rank === 'PREMIUM' || rank === 'VIP ACCOUNT';
    }

    /** Replace credentials: remove old account then add new one.
     *  addAccount() already calls fetchAccounts() internally, so no extra fetch needed. */
    async switchAccount(newEmail: string, newPassword: string): Promise<boolean> {
        const old = this.primary;
        // Remove old account first (also calls fetchAccounts internally, but that's fast now)
        if (old?.email) {
            await settingsStore.removeAccount(old.email);
        }
        // addAccount handles the POST + internal fetchAccounts
        return settingsStore.addAccount(newEmail, newPassword);
    }

    get listFormatted() {
        return this.accountsList.map(account => {
            const used = account.quota_used || 0;
            const total = account.quota_total || 0;
            const expiry = account.valid_until || 0;

            return {
                email: account.email,
                rank: account.rank || 'UNVERIFIED',
                expiry: expiry > 0 ? formatExpiry(expiry) : 'N/A',
                quotaPercent: getQuotaPercentage(used, total),
                quotaUsed: (used / (1024 ** 3)).toFixed(2) + ' GB',
                quotaTotal: (total / (1024 ** 3)).toFixed(2) + ' GB',
                is_active: account.is_active
            };
        });
    }

    get primaryFormatted() {
        const account = this.primary;
        if (!account) return {
            email: 'No Account',
            rank: 'GUEST',
            expiry: 'N/A',
            quotaPercent: 0,
            quotaUsed: '0 GB',
            quotaTotal: '0 GB'
        };

        const used = account.quota_used || 0;
        const total = account.quota_total || 0;
        const expiry = account.valid_until || 0;

        return {
            email: account.email,
            rank: account.rank || 'UNVERIFIED',
            expiry: expiry > 0 ? formatExpiry(expiry) : 'N/A',
            quotaPercent: getQuotaPercentage(used, total),
            quotaUsed: (used / (1024 ** 3)).toFixed(2) + ' GB',
            quotaTotal: (total / (1024 ** 3)).toFixed(2) + ' GB'
        };
    }
}

export const accountStore = new AccountStore();
