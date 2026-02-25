<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { animeFade } from "$lib/animations";

  // Health status types
  type HealthStatus = "healthy" | "degraded" | "unhealthy";

  interface ServiceHealth {
    status: HealthStatus;
    message: string | null;
    response_time_ms: number | null;
  }

  interface HealthCheckResponse {
    overall_status: HealthStatus;
    websocket: ServiceHealth;
    webhook: ServiceHealth;
    sonarr: ServiceHealth | null;
    radarr: ServiceHealth | null;
    fshare: ServiceHealth;
    fshare_ping: ServiceHealth;
    internet_speed: ServiceHealth;
    database: ServiceHealth;
  }

  // Props
  interface Props {
    showDetails?: boolean;
  }

  let { showDetails = false }: Props = $props();

  // State
  let healthData = $state<HealthCheckResponse | null>(null);
  let isLoading = $state(true);
  let showTooltip = $state(false);
  let pollInterval: ReturnType<typeof setInterval> | null = null;
  let hideTimeout: ReturnType<typeof setTimeout> | null = null;
  let abortController: AbortController | null = null;

  // Derived values
  let overallStatus = $derived(healthData?.overall_status || "unhealthy");
  let statusColor = $derived(
    overallStatus === "healthy"
      ? "#00ff80"
      : overallStatus === "degraded"
        ? "#ffd700"
        : "#ff5252",
  );
  let statusIcon = $derived(
    overallStatus === "healthy"
      ? "check_circle"
      : overallStatus === "degraded"
        ? "warning"
        : "error",
  );

  function toggleTooltip() {
    showTooltip = !showTooltip;

    // Clear any existing timeout
    if (hideTimeout) {
      clearTimeout(hideTimeout);
      hideTimeout = null;
    }

    // If showing tooltip, set auto-hide timer
    if (showTooltip) {
      hideTimeout = setTimeout(() => {
        showTooltip = false;
        hideTimeout = null;
      }, 3000);
    }
  }

  async function fetchHealthStatus() {
    // Cancel any previous in-flight request (e.g. during rapid navigation)
    if (abortController) abortController.abort();
    abortController = new AbortController();

    // 8s timeout — if backend is slow, don't block until the next 30s poll
    const timeoutId = setTimeout(() => abortController?.abort(), 8000);

    try {
      const response = await fetch("/api/health/status", {
        signal: abortController.signal,
      });
      clearTimeout(timeoutId);
      if (response.ok) {
        healthData = await response.json();
        isLoading = false;
      } else {
        console.warn("[StatusChecker] Health check returned:", response.status);
        healthData = null;
      }
    } catch (error: any) {
      clearTimeout(timeoutId);
      // AbortError = request was intentionally cancelled (navigation / timeout) — not a real error
      if (error?.name === "AbortError") {
        console.debug(
          "[StatusChecker] Health check aborted (navigation or timeout)",
        );
        return;
      }
      // Any other fetch failure (offline, DNS, CORS) — log as warn, not error
      console.warn(
        "[StatusChecker] Health check unavailable:",
        error?.message ?? error,
      );
      healthData = null;
    }
  }

  onMount(() => {
    // Initial fetch
    fetchHealthStatus();

    // Poll every 30 seconds
    pollInterval = setInterval(fetchHealthStatus, 30000);
  });

  onDestroy(() => {
    if (pollInterval) clearInterval(pollInterval);
    if (hideTimeout) clearTimeout(hideTimeout);
    // Cancel any pending fetch so it doesn't fire after component unmounts
    if (abortController) abortController.abort();
  });

  // Portal action to move element to document.body
  function portal(node: HTMLElement) {
    const originalParent = node.parentElement;
    console.log("[StatusChecker] Portal action: Moving tooltip to body");
    document.body.appendChild(node);

    return {
      destroy() {
        // Move back to original parent when destroyed
        if (originalParent && node.parentElement === document.body) {
          originalParent.appendChild(node);
        }
      },
    };
  }

  function getServiceIcon(serviceName: string): string {
    switch (serviceName) {
      case "websocket":
        return "wifi";
      case "webhook":
        return "webhook";
      case "sonarr":
        return "tv";
      case "radarr":
        return "movie";
      case "fshare":
        return "cloud";
      case "fshare_ping":
        return "network_ping";
      case "internet_speed":
        return "speed";
      case "database":
        return "storage";
      default:
        return "help";
    }
  }

  function getServiceLabel(serviceName: string): string {
    switch (serviceName) {
      case "websocket":
        return "WebSocket";
      case "webhook":
        return "Webhook";
      case "sonarr":
        return "Sonarr";
      case "radarr":
        return "Radarr";
      case "fshare":
        return "Fshare";
      case "fshare_ping":
        return "Fshare Ping";
      case "internet_speed":
        return "Internet Speed";
      case "database":
        return "Database";
      default:
        return serviceName;
    }
  }
</script>

{#if !showDetails}
  <!-- Compact Status Dot (for header) -->
  <div
    class="status-checker-compact"
    onclick={toggleTooltip}
    onkeydown={(e) => e.key === "Enter" && toggleTooltip()}
    role="button"
    tabindex="0"
    aria-label="System health status"
    title="Click to view system health"
  >
    <span class="status-dot-health" style="background: {statusColor};"></span>
  </div>

  <!-- Tooltip rendered outside to escape stacking context -->
  {#if showTooltip}
    <div
      use:portal
      class="status-tooltip"
      style="top: 72px; right: 20px;"
      transition:animeFade={{ duration: 150 }}
    >
      {#if isLoading}
        <div class="tooltip-header">
          <span class="material-icons rotating">refresh</span>
          <span>Loading...</span>
        </div>
      {:else if healthData}
        <div class="tooltip-header">
          <span class="material-icons">{statusIcon}</span>
          <span>System: {overallStatus.toUpperCase()}</span>
        </div>
        <div class="tooltip-services">
          <!-- WebSocket -->
          <div class="service-row">
            <span class="material-icons">{getServiceIcon("websocket")}</span>
            <span>WebSocket</span>
            <span
              class="service-status"
              style="color: {healthData.websocket.status === 'healthy'
                ? '#00ff80'
                : healthData.websocket.status === 'degraded'
                  ? '#ffd700'
                  : '#ff5252'};"
            >
              {healthData.websocket.status === "healthy"
                ? "✓"
                : healthData.websocket.status === "degraded"
                  ? "⚠"
                  : "✗"}
              {#if healthData.websocket.message}
                {healthData.websocket.message}
              {/if}
            </span>
          </div>

          <!-- Webhook -->
          {#if healthData.webhook}
            <div class="service-row">
              <span class="material-icons">{getServiceIcon("webhook")}</span>
              <span>Webhook</span>
              <span
                class="service-status"
                style="color: {healthData.webhook.status === 'healthy'
                  ? '#00ff80'
                  : '#ffd700'};"
              >
                {healthData.webhook.status === "healthy" ? "✓" : "⚠"}
                {#if healthData.webhook.message}
                  {healthData.webhook.message}
                {/if}
              </span>
            </div>
          {/if}

          {#if healthData.sonarr}
            <div class="service-row">
              <span class="material-icons">{getServiceIcon("sonarr")}</span>
              <span>Sonarr</span>
              <span
                class="service-status"
                style="color: {healthData.sonarr.status === 'healthy'
                  ? '#00ff80'
                  : healthData.sonarr.status === 'degraded'
                    ? '#ffd700'
                    : '#ff5252'};"
              >
                {healthData.sonarr.status === "healthy"
                  ? "✓"
                  : healthData.sonarr.status === "degraded"
                    ? "⚠"
                    : "✗"}
                {#if healthData.sonarr.response_time_ms}
                  {healthData.sonarr.response_time_ms}ms
                {/if}
              </span>
            </div>
          {/if}

          {#if healthData.radarr}
            <div class="service-row">
              <span class="material-icons">{getServiceIcon("radarr")}</span>
              <span>Radarr</span>
              <span
                class="service-status"
                style="color: {healthData.radarr.status === 'healthy'
                  ? '#00ff80'
                  : healthData.radarr.status === 'degraded'
                    ? '#ffd700'
                    : '#ff5252'};"
              >
                {healthData.radarr.status === "healthy"
                  ? "✓"
                  : healthData.radarr.status === "degraded"
                    ? "⚠"
                    : "✗"}
                {#if healthData.radarr.response_time_ms}
                  {healthData.radarr.response_time_ms}ms
                {/if}
              </span>
            </div>
          {/if}

          <!-- Fshare Handler -->
          <div class="service-row">
            <span class="material-icons">{getServiceIcon("fshare")}</span>
            <span>Fshare</span>
            <span
              class="service-status"
              style="color: {healthData.fshare.status === 'healthy'
                ? '#00ff80'
                : '#ffd700'};"
            >
              {healthData.fshare.status === "healthy" ? "✓" : "⚠"}
            </span>
          </div>

          <!-- Fshare Ping -->
          {#if healthData.fshare_ping}
            <div class="service-row">
              <span class="material-icons">{getServiceIcon("fshare_ping")}</span
              >
              <span>Fshare Ping</span>
              <span
                class="service-status"
                style="color: {healthData.fshare_ping.status === 'healthy'
                  ? '#00ff80'
                  : healthData.fshare_ping.status === 'degraded'
                    ? '#ffd700'
                    : '#ff5252'};"
              >
                {healthData.fshare_ping.status === "healthy"
                  ? "✓"
                  : healthData.fshare_ping.status === "degraded"
                    ? "⚠"
                    : "✗"}
                {#if healthData.fshare_ping.message}
                  {healthData.fshare_ping.message}
                {/if}
              </span>
            </div>
          {/if}

          <!-- Internet Speed -->
          {#if healthData.internet_speed}
            <div class="service-row">
              <span class="material-icons"
                >{getServiceIcon("internet_speed")}</span
              >
              <span>Internet Speed</span>
              <span class="service-status" style="color: #888;">
                {healthData.internet_speed.message || "N/A"}
              </span>
            </div>
          {/if}
        </div>
      {:else}
        <div class="tooltip-header">
          <span class="material-icons">error</span>
          <span>Failed to load</span>
        </div>
      {/if}
    </div>
  {/if}
{:else}
  <!-- Detailed Status Panel (for Settings page) -->
  <div class="status-checker-detailed">
    {#if isLoading}
      <div class="loading-state">
        <span class="material-icons rotating">refresh</span>
        <span>Checking system health...</span>
      </div>
    {:else if healthData}
      <div class="status-grid">
        <!-- WebSocket -->
        <div class="status-card">
          <div class="card-header">
            <span class="material-icons">{getServiceIcon("websocket")}</span>
            <span class="service-name">WebSocket</span>
          </div>
          <div class="card-body">
            <span
              class="status-badge"
              style="background: {healthData.websocket.status === 'healthy'
                ? 'rgba(0, 255, 128, 0.1)'
                : 'rgba(255, 215, 0, 0.1)'}; color: {healthData.websocket
                .status === 'healthy'
                ? '#00ff80'
                : '#ffd700'};"
            >
              {healthData.websocket.status}
            </span>
            {#if healthData.websocket.message}
              <p class="status-message">{healthData.websocket.message}</p>
            {/if}
          </div>
        </div>

        <!-- Sonarr -->
        {#if healthData.sonarr}
          <div class="status-card">
            <div class="card-header">
              <span class="material-icons">{getServiceIcon("sonarr")}</span>
              <span class="service-name">Sonarr</span>
            </div>
            <div class="card-body">
              <span
                class="status-badge"
                style="background: {healthData.sonarr.status === 'healthy'
                  ? 'rgba(0, 255, 128, 0.1)'
                  : healthData.sonarr.status === 'degraded'
                    ? 'rgba(255, 215, 0, 0.1)'
                    : 'rgba(255, 82, 82, 0.1)'}; color: {healthData.sonarr
                  .status === 'healthy'
                  ? '#00ff80'
                  : healthData.sonarr.status === 'degraded'
                    ? '#ffd700'
                    : '#ff5252'};"
              >
                {healthData.sonarr.status}
              </span>
              {#if healthData.sonarr.message}
                <p class="status-message">{healthData.sonarr.message}</p>
              {/if}
            </div>
          </div>
        {/if}

        <!-- Radarr -->
        {#if healthData.radarr}
          <div class="status-card">
            <div class="card-header">
              <span class="material-icons">{getServiceIcon("radarr")}</span>
              <span class="service-name">Radarr</span>
            </div>
            <div class="card-body">
              <span
                class="status-badge"
                style="background: {healthData.radarr.status === 'healthy'
                  ? 'rgba(0, 255, 128, 0.1)'
                  : healthData.radarr.status === 'degraded'
                    ? 'rgba(255, 215, 0, 0.1)'
                    : 'rgba(255, 82, 82, 0.1)'}; color: {healthData.radarr
                  .status === 'healthy'
                  ? '#00ff80'
                  : healthData.radarr.status === 'degraded'
                    ? '#ffd700'
                    : '#ff5252'};"
              >
                {healthData.radarr.status}
              </span>
              {#if healthData.radarr.message}
                <p class="status-message">{healthData.radarr.message}</p>
              {/if}
            </div>
          </div>
        {/if}

        <!-- Fshare -->
        <div class="status-card">
          <div class="card-header">
            <span class="material-icons">{getServiceIcon("fshare")}</span>
            <span class="service-name">Fshare</span>
          </div>
          <div class="card-body">
            <span
              class="status-badge"
              style="background: {healthData.fshare.status === 'healthy'
                ? 'rgba(0, 255, 128, 0.1)'
                : 'rgba(255, 215, 0, 0.1)'}; color: {healthData.fshare
                .status === 'healthy'
                ? '#00ff80'
                : '#ffd700'};"
            >
              {healthData.fshare.status}
            </span>
            {#if healthData.fshare.message}
              <p class="status-message">{healthData.fshare.message}</p>
            {/if}
          </div>
        </div>

        <!-- Database -->
        <div class="status-card">
          <div class="card-header">
            <span class="material-icons">{getServiceIcon("database")}</span>
            <span class="service-name">Database</span>
          </div>
          <div class="card-body">
            <span
              class="status-badge"
              style="background: rgba(0, 255, 128, 0.1); color: #00ff80;"
            >
              {healthData.database.status}
            </span>
            {#if healthData.database.message}
              <p class="status-message">{healthData.database.message}</p>
            {/if}
          </div>
        </div>
      </div>
    {:else}
      <div class="error-state">
        <span class="material-icons">error_outline</span>
        <span>Failed to load health status</span>
      </div>
    {/if}
  </div>
{/if}

<style>
  /* Compact Status (Header) */
  .status-checker-compact {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.05);
    clip-path: polygon(
      6px 0%,
      100% 0%,
      100% calc(100% - 6px),
      calc(100% - 6px) 100%,
      0% 100%,
      0% 6px
    );
    cursor: pointer;
    transition: all 0.3s;
  }

  .status-checker-compact:hover {
    background: rgba(0, 0, 0, 0.5);
    border-color: rgba(255, 255, 255, 0.1);
    transform: scale(1.05);
  }

  .status-dot-health {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    animation: pulse 2s ease-in-out infinite;
    box-shadow: 0 0 10px currentColor;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
      transform: scale(1);
    }
    50% {
      opacity: 0.6;
      transform: scale(1.1);
    }
  }

  /* Tooltip */
  .status-tooltip {
    position: fixed;
    min-width: 280px;
    background: #0a0e14;
    border: 2px solid #00f3ff;
    padding: 0;
    z-index: 10000;
    font-family: var(--font-mono), "Courier New", monospace;
    box-shadow:
      0 0 20px rgba(0, 243, 255, 0.3),
      0 10px 40px rgba(0, 0, 0, 0.8);
    clip-path: polygon(
      8px 0%,
      100% 0%,
      100% calc(100% - 8px),
      calc(100% - 8px) 100%,
      0% 100%,
      0% 8px
    );
  }

  .tooltip-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    background: rgba(0, 243, 255, 0.1);
    border-bottom: 1px solid #00f3ff;
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: #00f3ff;
  }

  .tooltip-header .material-icons {
    font-size: 1rem;
  }

  .tooltip-services {
    display: flex;
    flex-direction: column;
    padding: 0.75rem 1rem;
    gap: 0.75rem;
  }

  .service-row {
    display: grid;
    grid-template-columns: 24px 1fr auto;
    align-items: center;
    gap: 0.75rem;
    font-size: 0.75rem;
    padding: 0.5rem;
    background: rgba(0, 243, 255, 0.03);
    border-left: 2px solid rgba(0, 243, 255, 0.2);
    transition: all 0.2s;
  }

  .service-row:hover {
    background: rgba(0, 243, 255, 0.08);
    border-left-color: #00f3ff;
  }

  .service-row .material-icons {
    font-size: 1.1rem;
    color: #00f3ff;
  }

  .service-row span:nth-child(2) {
    color: #fff;
    font-weight: 600;
  }

  .service-status {
    font-family: var(--font-mono);
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 0.25rem 0.5rem;
    background: rgba(0, 0, 0, 0.3);
    border-radius: 2px;
  }

  /* Detailed Status (Settings Page) */
  .status-checker-detailed {
    width: 100%;
  }

  .loading-state,
  .error-state {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    padding: 2rem;
    color: var(--text-secondary);
    font-size: 0.9rem;
  }

  .loading-state .material-icons,
  .error-state .material-icons {
    font-size: 1.5rem;
  }

  .rotating {
    animation: rotate 1s linear infinite;
  }

  @keyframes rotate {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  .status-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    gap: 1rem;
  }

  .status-card {
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.05);
    padding: 1rem;
    clip-path: polygon(
      12px 0%,
      100% 0%,
      100% calc(100% - 12px),
      calc(100% - 12px) 100%,
      0% 100%,
      0% 12px
    );
    transition: all 0.3s;
  }

  .status-card:hover {
    background: rgba(0, 0, 0, 0.3);
    border-color: rgba(0, 243, 255, 0.2);
  }

  .card-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 0.75rem;
  }

  .card-header .material-icons {
    font-size: 1.5rem;
    color: var(--color-primary);
  }

  .service-name {
    font-size: 0.9rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #fff;
  }

  .card-body {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .status-badge {
    display: inline-block;
    padding: 0.25rem 0.75rem;
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    font-family: var(--font-mono);
    width: fit-content;
    clip-path: polygon(
      4px 0%,
      100% 0%,
      100% calc(100% - 4px),
      calc(100% - 4px) 100%,
      0% 100%,
      0% 4px
    );
  }

  .status-message {
    font-size: 0.75rem;
    color: var(--text-secondary);
    margin: 0;
  }
</style>
