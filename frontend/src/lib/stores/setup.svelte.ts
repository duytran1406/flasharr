/**
 * Setup Wizard Store (Svelte 5 Runes)
 * Manages onboarding wizard state and API interactions
 */

import { toasts } from './toasts';

interface WizardData {
  // Step 1: FShare Account
  fshareEmail: string;
  fsharePassword: string;
  
  // Step 2: Download Config
  downloadPath: string;
  maxConcurrent: number;
  
  // Step 3: Integrations (Optional)
  sonarrEnabled: boolean;
  sonarrUrl: string;
  sonarrApiKey: string;
  
  radarrEnabled: boolean;
  radarrUrl: string;
  radarrApiKey: string;
  
  jellyfinEnabled: boolean;
  jellyfinUrl: string;
  jellyfinApiKey: string;
}

class SetupStore {
  currentStep = $state(0);
  isComplete = $state(false);
  isLoading = $state(false);
  
  // Wizard form data
  data = $state<WizardData>({
    fshareEmail: '',
    fsharePassword: '',
    downloadPath: '/downloads',
    maxConcurrent: 3,
    sonarrEnabled: false,
    sonarrUrl: 'http://localhost:8989',
    sonarrApiKey: '',
    radarrEnabled: false,
    radarrUrl: 'http://localhost:7878',
    radarrApiKey: '',
    jellyfinEnabled: false,
    jellyfinUrl: 'http://localhost:8096',
    jellyfinApiKey: '',
  });
  
  // Validation states
  fshareValidated = $state(false);
  sonarrValidated = $state(false);
  radarrValidated = $state(false);
  jellyfinValidated = $state(false);
  
  /**
   * Check if setup is complete
   */
  async checkStatus(): Promise<boolean> {
    try {
      const res = await fetch('/api/setup/status');
      if (res.ok) {
        const data = await res.json();
        this.isComplete = data.complete;
        return data.complete;
      }
    } catch (e) {
      console.error('Failed to check setup status:', e);
    }
    return false;
  }
  
  /**
   * Validate FShare credentials
   */
  async validateFshare(): Promise<boolean> {
    this.isLoading = true;
    try {
      const res = await fetch('/api/setup/fshare', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          email: this.data.fshareEmail,
          password: this.data.fsharePassword,
        }),
      });
      
      if (res.ok) {
        const result = await res.json();
        if (result.success) {
          this.fshareValidated = true;
          toasts.success('FShare account connected successfully!');
          return true;
        } else {
          toasts.error(result.message || 'Invalid credentials');
          return false;
        }
      }
    } catch (e) {
      toasts.error('Failed to connect to FShare');
      console.error('FShare validation error:', e);
    } finally {
      this.isLoading = false;
    }
    return false;
  }
  
  /**
   * Test Sonarr connection
   */
  async testSonarr(): Promise<boolean> {
    this.isLoading = true;
    try {
      const res = await fetch('/api/setup/sonarr/test', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          url: this.data.sonarrUrl,
          api_key: this.data.sonarrApiKey,
        }),
      });
      
      if (res.ok) {
        const result = await res.json();
        if (result.success) {
          this.sonarrValidated = true;
          const msg = result.version 
            ? `Connected to Sonarr v${result.version}` 
            : 'Connected to Sonarr';
          toasts.success(msg);
          return true;
        } else {
          toasts.error(result.message || 'Connection failed');
          return false;
        }
      }
    } catch (e) {
      toasts.error('Failed to connect to Sonarr');
      console.error('Sonarr test error:', e);
    } finally {
      this.isLoading = false;
    }
    return false;
  }
  
  /**
   * Test Radarr connection
   */
  async testRadarr(): Promise<boolean> {
    this.isLoading = true;
    try {
      const res = await fetch('/api/setup/radarr/test', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          url: this.data.radarrUrl,
          api_key: this.data.radarrApiKey,
        }),
      });
      
      if (res.ok) {
        const result = await res.json();
        if (result.success) {
          this.radarrValidated = true;
          const msg = result.version 
            ? `Connected to Radarr v${result.version}` 
            : 'Connected to Radarr';
          toasts.success(msg);
          return true;
        } else {
          toasts.error(result.message || 'Connection failed');
          return false;
        }
      }
    } catch (e) {
      toasts.error('Failed to connect to Radarr');
      console.error('Radarr test error:', e);
    } finally {
      this.isLoading = false;
    }
    return false;
  }
  
  /**
   * Test Jellyfin connection
   */
  async testJellyfin(): Promise<boolean> {
    this.isLoading = true;
    try {
      const res = await fetch('/api/setup/jellyfin/test', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          url: this.data.jellyfinUrl,
          api_key: this.data.jellyfinApiKey,
        }),
      });
      
      if (res.ok) {
        const result = await res.json();
        if (result.success) {
          this.jellyfinValidated = true;
          const msg = result.version 
            ? `Connected to Jellyfin v${result.version}` 
            : 'Connected to Jellyfin';
          toasts.success(msg);
          return true;
        } else {
          toasts.error(result.message || 'Connection failed');
          return false;
        }
      }
    } catch (e) {
      toasts.error('Failed to connect to Jellyfin');
      console.error('Jellyfin test error:', e);
    } finally {
      this.isLoading = false;
    }
    return false;
  }
  
  /**
   * Complete setup and save all settings
   */
  async completeSetup(): Promise<boolean> {
    this.isLoading = true;
    try {
      const payload: any = {
        fshare: {
          email: this.data.fshareEmail,
          password: this.data.fsharePassword,
        },
        downloads: {
          directory: this.data.downloadPath,
          max_concurrent: this.data.maxConcurrent,
        },
      };
      
      // Add optional integrations
      if (this.data.sonarrEnabled && this.sonarrValidated) {
        payload.sonarr = {
          url: this.data.sonarrUrl,
          api_key: this.data.sonarrApiKey,
        };
      }
      
      if (this.data.radarrEnabled && this.radarrValidated) {
        payload.radarr = {
          url: this.data.radarrUrl,
          api_key: this.data.radarrApiKey,
        };
      }
      
      if (this.data.jellyfinEnabled && this.jellyfinValidated) {
        payload.jellyfin = {
          url: this.data.jellyfinUrl,
          api_key: this.data.jellyfinApiKey,
        };
      }
      
      const res = await fetch('/api/setup/complete', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      
      if (res.ok) {
        const result = await res.json();
        if (result.success) {
          this.isComplete = true;
          toasts.success('Setup completed successfully!');
          return true;
        } else {
          toasts.error(result.message || 'Failed to complete setup');
          return false;
        }
      }
    } catch (e) {
      toasts.error('Failed to complete setup');
      console.error('Complete setup error:', e);
    } finally {
      this.isLoading = false;
    }
    return false;
  }
  
  /**
   * Navigate to next step
   */
  nextStep() {
    if (this.currentStep < 4) {
      this.currentStep++;
    }
  }
  
  /**
   * Navigate to previous step
   */
  prevStep() {
    if (this.currentStep > 0) {
      this.currentStep--;
    }
  }
  
  /**
   * Go to specific step
   */
  goToStep(step: number) {
    if (step >= 0 && step <= 4) {
      this.currentStep = step;
    }
  }
  
  /**
   * Reset wizard
   */
  reset() {
    this.currentStep = 0;
    this.isComplete = false;
    this.fshareValidated = false;
    this.sonarrValidated = false;
    this.radarrValidated = false;
    this.jellyfinValidated = false;
  }
}

export const setupStore = new SetupStore();
