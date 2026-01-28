<script lang="ts">
  import SpeedGraph from "../SpeedGraph.svelte";

  interface Props {
    speedHistory: number[];
    sessionPeak: number;
    sessionTotalBytes: number;
    currentSpeedValue: string;
    currentSpeedUnit: string;
  }

  let {
    speedHistory,
    sessionPeak,
    sessionTotalBytes,
    currentSpeedValue,
    currentSpeedUnit,
  }: Props = $props();

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }
</script>

<section class="box-section netflow-section">
  <div class="box-label">
    <span class="material-icons">insights</span>
    NETFLOW STATISTIC
  </div>

  <div class="netflow-telemetry">
    <div class="telemetry-main">
      <div class="telemetry-core">
        <span class="material-icons dl-pulse">settings_input_antenna</span>
        <div class="core-text">
          <span class="v">{currentSpeedValue}</span>
          <span class="u">{currentSpeedUnit}</span>
        </div>
      </div>
      <div class="telemetry-label">INBOUND BITRATE</div>
    </div>

    <div class="telemetry-grid">
      <div class="tele-item">
        <span class="l">SESSION PEAK</span>
        <span class="v">{sessionPeak.toFixed(1)} MB/s</span>
      </div>
      <div class="tele-item">
        <span class="l">SESSION DATA</span>
        <span class="v">{formatBytes(sessionTotalBytes)}</span>
      </div>
    </div>
  </div>

  <div class="chart-wrapper">
    <SpeedGraph data={speedHistory} labels={new Array(30).fill("")} />
  </div>
</section>

<style>
  .box-section {
    background: rgba(10, 15, 25, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 0;
    position: relative;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .box-section::before {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    width: 2px;
    height: 100%;
    background: var(--color-primary);
    opacity: 0.5;
  }

  .box-label {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    font-size: 0.65rem;
    font-weight: 900;
    letter-spacing: 0.2rem;
    text-transform: uppercase;
    margin-bottom: 0.75rem;
    color: var(--text-muted);
    font-family: var(--font-mono, monospace);
    padding: 0 0.5rem;
    flex-shrink: 0;
  }

  .box-label .material-icons {
    font-size: 1.1rem;
    opacity: 0.8;
  }

  .netflow-section {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .netflow-telemetry {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    margin-bottom: 0.5rem;
  }

  .telemetry-main {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
  }

  .telemetry-core {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .telemetry-core .material-icons {
    font-size: 2rem;
    color: var(--color-primary);
  }

  .dl-pulse {
    animation: pulse-icon 2s ease-in-out infinite;
  }

  @keyframes pulse-icon {
    0%,
    100% {
      opacity: 0.6;
    }
    50% {
      opacity: 1;
    }
  }

  .core-text {
    display: flex;
    align-items: baseline;
    gap: 0.25rem;
  }

  .core-text .v {
    font-size: 2.5rem;
    font-weight: 900;
    color: #fff;
    font-family: var(--font-mono, monospace);
  }

  .core-text .u {
    font-size: 0.9rem;
    font-weight: 700;
    color: var(--text-muted);
    font-family: var(--font-mono, monospace);
  }

  .telemetry-label {
    font-size: 0.6rem;
    font-weight: 800;
    color: var(--text-muted);
    letter-spacing: 0.15em;
    font-family: var(--font-mono, monospace);
  }

  .telemetry-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }

  .tele-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    padding: 0.75rem;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.03);
    border-radius: 8px;
  }

  .tele-item .l {
    font-size: 0.55rem;
    font-weight: 800;
    color: var(--text-muted);
    letter-spacing: 0.1em;
    opacity: 0.7;
  }

  .tele-item .v {
    font-size: 0.85rem;
    font-weight: 700;
    color: #fff;
    font-family: var(--font-mono, monospace);
  }

  .chart-wrapper {
    flex: 1;
    margin-top: 0.5rem;
    position: relative;
    min-height: 120px;
    width: 100%;
  }

  :global(.netflow-section .glass-card) {
    background: transparent !important;
    border: none !important;
    padding: 0 !important;
    box-shadow: none !important;
  }
</style>
