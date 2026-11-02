/**
 * Admin authentication session (Svelte 5 runes).
 *
 * Backed by /api/auth/*. Used by the root layout's auth gate and by the
 * settings page (change password / logout). The websocket is connected after
 * a successful login/registration and disconnected on logout/password change.
 */
import { wsClient } from './websocket';

/** GET /api/auth/setup-status (also the body of login/setup responses). */
export interface AuthSession {
  /** An administrator account exists. */
  configured: boolean;
  authenticated: boolean;
  username: string | null;
}

/** `setup` creates the first administrator; `login` signs in. */
export type AuthMode = 'login' | 'setup';

const SIGNED_OUT: AuthSession = { configured: true, authenticated: false, username: null };

class AuthStore {
  session = $state<AuthSession | null>(null);
  /** True until the first refresh() completes (only while session is null). */
  loading = $state(true);
  error = $state<string | null>(null);

  /** Re-check the session. On failure the user is treated as signed out. */
  async refresh(): Promise<void> {
    this.loading = this.session === null;
    try {
      const response = await fetch('/api/auth/setup-status');
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      this.session = await response.json();
      this.error = null;
    } catch (err) {
      this.error = err instanceof Error ? err.message : 'Authentication check failed';
      this.session = { ...SIGNED_OUT };
    } finally {
      this.loading = false;
    }
  }

  /** POST /api/auth/{login|setup}. Throws a user-facing Error on failure. */
  async submit(mode: AuthMode, username: string, password: string): Promise<void> {
    const response = await fetch(`/api/auth/${mode}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    });
    if (!response.ok) {
      throw new Error(mode === 'login' ? 'Invalid credentials' : 'Unable to create administrator');
    }
    this.session = { configured: true, ...(await response.json()) };
    this.error = null;
    wsClient.connect();
  }

  /**
   * Change the admin password. Signs the user out on success (the backend
   * revokes sessions). Throws a user-facing Error on failure.
   */
  async changePassword(currentPassword: string, newPassword: string): Promise<void> {
    const response = await fetch('/api/auth/change-password', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
    });
    if (!response.ok) {
      const message =
        response.status === 401
          ? 'Current password is incorrect.'
          : response.status === 400
            ? 'Use a different password with at least 12 characters.'
            : 'Unable to change password.';
      throw new Error(message);
    }
    wsClient.disconnect();
    this.session = { ...SIGNED_OUT };
  }

  async logout(): Promise<void> {
    wsClient.disconnect();
    await fetch('/api/auth/logout', { method: 'POST' });
    this.session = { ...SIGNED_OUT };
  }
}

export const authStore = new AuthStore();
