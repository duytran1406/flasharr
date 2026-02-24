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

<section class="premium-card netflow-section">
  <div class="card-header-premium">
    <span class="material-icons">insights</span>
    <span class="label-text">NETFLOW STATISTIC</span>
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

    <div class="telemetry-sub">
      <div class="sub-item">
        <span class="l">SESSION PEAK</span>
        <span class="v">{sessionPeak.toFixed(1)} <small>MB/s</small></span>
      </div>
      <div class="sub-separator"></div>
      <div class="sub-item">
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
  .netflow-section {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .netflow-telemetry {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1.5rem;
    margin-bottom: 1rem;
    padding: 0 0.5rem;
  }

  .telemetry-main {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0;
  }

  .telemetry-core {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .telemetry-core .material-icons {
    font-size: 1.5rem;
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
    font-size: 2rem;
    font-weight: 900;
    color: var(--text-primary);
    font-family: var(--font-mono, monospace);
    line-height: 1;
  }

  .core-text .u {
    font-size: 0.8rem;
    font-weight: 700;
    color: var(--text-muted);
    font-family: var(--font-mono, monospace);
  }

  .telemetry-label {
    font-size: 0.55rem;
    font-weight: 800;
    color: var(--text-muted);
    letter-spacing: 0.1rem;
    font-family: var(--font-mono, monospace);
    opacity: 0.6;
    margin-top: 0.2rem;
  }

  .telemetry-sub {
    display: flex;
    align-items: center;
    gap: 1.25rem;
    background: rgba(255, 255, 255, 0.02);
    padding: 0.5rem 1rem;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.03);
  }

  .sub-item {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .sub-item .l {
    font-size: 0.5rem;
    font-weight: 800;
    color: var(--text-muted);
    letter-spacing: 0.05rem;
    opacity: 0.5;
  }

  .sub-item .v {
    font-size: 0.8rem;
    font-weight: 700;
    color: var(--text-primary);
    font-family: var(--font-mono, monospace);
  }

  .sub-item .v small {
    font-size: 0.6rem;
    opacity: 0.7;
  }

  .sub-separator {
    width: 1px;
    height: 20px;
    background: rgba(255, 255, 255, 0.1);
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
