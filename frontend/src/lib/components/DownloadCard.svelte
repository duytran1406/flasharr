<script lang="ts">
  import { Pause, Play, Trash2, RotateCcw, ExternalLink } from "lucide-svelte";
  import ProgressRing from "./ProgressRing.svelte";

  interface DownloadItem {
    id: string;
    filename: string;
    size: number;
    downloaded: number;
    speed: number;
    eta: number;
    state: string;
    progress: number;
  }

  interface Props {
    download: DownloadItem;
    onPause?: () => void;
    onResume?: () => void;
    onDelete?: () => void;
    onRetry?: () => void;
  }

  let { download, onPause, onResume, onDelete, onRetry }: Props = $props();

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
  }

  function formatSpeed(bytesPerSec: number): string {
    return formatBytes(bytesPerSec) + "/s";
  }

  function formatEta(seconds: number): string {
    if (seconds <= 0) return "--";
    if (seconds < 60) return `${seconds}s`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
    return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
  }

  const stateConfig: Record<string, { color: string; bg: string }> = {
    Downloading: { color: "text-blue-400", bg: "bg-blue-500/20" },
    Queued: { color: "text-yellow-400", bg: "bg-yellow-500/20" },
    Completed: { color: "text-green-400", bg: "bg-green-500/20" },
    Failed: { color: "text-red-400", bg: "bg-red-500/20" },
    Paused: { color: "text-slate-400", bg: "bg-slate-500/20" },
    Starting: { color: "text-cyan-400", bg: "bg-cyan-500/20" },
    Waiting: { color: "text-orange-400", bg: "bg-orange-500/20" },
  };

  const config = $derived(stateConfig[download.state] || stateConfig["Queued"]);
</script>

<div class="download-card glass-panel group">
  <!-- Progress Ring -->
  <div class="ring-container">
    <ProgressRing
      progress={download.progress}
      size={52}
      strokeWidth={4}
      color={download.state === "Downloading"
        ? "#00f3ff"
        : download.state === "Starting"
          ? "#00d4ff"
          : download.state === "Completed"
            ? "#00ff80"
            : download.state === "Failed"
              ? "#ff0064"
              : download.state === "Waiting"
                ? "#ff9500"
                : download.state === "Extracting"
                  ? "#a855f7"
                  : download.state === "Cancelled"
                    ? "#ff6b6b"
                    : download.state === "Skipped"
                      ? "#6b7280"
                      : "#888"}
    />
  </div>

  <!-- Info -->
  <div class="download-info">
    <div class="info-top">
      <h4 class="filename" title={download.filename}>{download.filename}</h4>
      <span class="state-tag {download.state.toLowerCase()}">
        {download.state}
      </span>
    </div>

    <div class="info-meta">
      <span class="size-progress"
        >{formatBytes(download.downloaded)} / {formatBytes(download.size)}</span
      >
      {#if download.state === "Downloading"}
        <span class="speed">{formatSpeed(download.speed)}</span>
        <span class="eta">ETA: {formatEta(download.eta)}</span>
      {/if}
    </div>

    <!-- Progress Bar -->
    <div class="progress-bar-container">
      <div
        class="progress-fill {download.state.toLowerCase()}"
        style="width: {download.progress}%"
      ></div>
    </div>
  </div>

  <!-- Actions -->
  <div class="action-btns">
    {#if download.state === "Downloading"}
      <button onclick={() => onPause?.()} class="action-btn" title="Pause">
        <Pause size={16} />
      </button>
    {:else if download.state === "Paused" || download.state === "Queued"}
      <button onclick={() => onResume?.()} class="action-btn" title="Resume">
        <Play size={16} />
      </button>
    {:else if download.state === "Failed"}
      <button onclick={() => onRetry?.()} class="action-btn" title="Retry">
        <RotateCcw size={16} />
      </button>
    {/if}
    <button
      onclick={() => onDelete?.()}
      class="action-btn delete"
      title="Remove"
    >
      <Trash2 size={16} />
    </button>
  </div>
</div>

<style>
  .download-card {
    display: flex;
    align-items: center;
    gap: 1.25rem;
    padding: 1rem 1.5rem;
    background: rgba(10, 15, 25, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-left: 2px solid var(--state-color, rgba(255, 255, 255, 0.1));
    position: relative;
    overflow: visible;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    clip-path: polygon(
      0% 0%,
      calc(100% - 12px) 0%,
      100% 12px,
      100% 100%,
      12px 100%,
      0% calc(100% - 12px)
    );
  }

  .download-card:hover {
    background: rgba(0, 243, 255, 0.02);
    border-color: rgba(0, 243, 255, 0.2);
    transform: translateX(4px);
  }

  .ring-container {
    flex-shrink: 0;
  }

  .download-info {
    flex: 1;
    min-width: 0;
  }

  .info-top {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 0.35rem;
  }

  .filename {
    margin: 0;
    font-size: 0.85rem;
    font-weight: 700;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: var(--font-mono, monospace);
  }

  .state-tag {
    font-size: 0.55rem;
    font-weight: 900;
    text-transform: uppercase;
    padding: 2px 6px;
    letter-spacing: 0.05em;
    clip-path: polygon(
      4px 0%,
      100% 0%,
      100% calc(100% - 4px),
      calc(100% - 4px) 100%,
      0% 100%,
      0% 4px
    );
  }

  .state-tag.downloading {
    background: rgba(0, 243, 255, 0.1);
    color: #00f3ff;
  }
  .state-tag.starting {
    background: rgba(0, 212, 255, 0.1);
    color: #00d4ff;
  }
  .state-tag.completed {
    background: rgba(0, 255, 128, 0.1);
    color: #00ff80;
  }
  .state-tag.failed {
    background: rgba(255, 0, 100, 0.1);
    color: #ff0064;
  }
  .state-tag.paused {
    background: rgba(255, 255, 255, 0.05);
    color: #888;
  }
  .state-tag.queued {
    background: rgba(255, 204, 0, 0.1);
    color: #ffcc00;
  }
  .state-tag.waiting {
    background: rgba(255, 149, 0, 0.1);
    color: #ff9500;
  }
  .state-tag.extracting {
    background: rgba(168, 85, 247, 0.1);
    color: #a855f7;
  }
  .state-tag.cancelled {
    background: rgba(255, 107, 107, 0.1);
    color: #ff6b6b;
  }
  .state-tag.skipped {
    background: rgba(107, 114, 128, 0.1);
    color: #6b7280;
  }

  .info-meta {
    display: flex;
    align-items: center;
    gap: 1rem;
    font-size: 0.7rem;
    color: var(--text-muted);
    font-family: var(--font-mono, monospace);
  }

  .speed {
    color: var(--color-primary);
    font-weight: 700;
  }

  .progress-bar-container {
    height: 3px;
    background: rgba(0, 0, 0, 0.4);
    margin-top: 0.75rem;
    overflow: hidden;
    position: relative;
  }

  .progress-fill {
    height: 100%;
    transition: width 0.3s;
  }

  .progress-fill.downloading,
  .progress-fill.starting {
    background: var(--color-primary);
    box-shadow: 0 0 10px var(--color-primary);
    /* Halftone dot overlay on progress fill */
    background-image: radial-gradient(
      circle,
      rgba(255, 255, 255, 0.15) 0.5px,
      transparent 0.5px
    );
    background-size: 4px 4px;
  }
  .progress-fill.completed {
    background: #00ff80;
  }
  .progress-fill.failed,
  .progress-fill.cancelled {
    background: #ff0064;
  }
  .progress-fill.paused,
  .progress-fill.queued,
  .progress-fill.skipped {
    background: #555;
  }
  .progress-fill.waiting {
    background: #ff9500;
  }
  .progress-fill.extracting {
    background: #a855f7;
    box-shadow: 0 0 10px #a855f7;
  }

  .action-btns {
    display: flex;
    gap: 0.5rem;
    opacity: 0;
    transition: opacity 0.2s;
  }

  .download-card:hover .action-btns {
    opacity: 1;
  }

  .action-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.05);
    color: var(--text-muted);
    cursor: pointer;
    transition: all 0.2s;
    clip-path: polygon(
      4px 0%,
      100% 0%,
      100% calc(100% - 4px),
      calc(100% - 4px) 100%,
      0% 100%,
      0% 4px
    );
  }

  .action-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
    border-color: rgba(255, 255, 255, 0.2);
  }

  .action-btn.delete:hover {
    background: rgba(255, 0, 100, 0.1);
    color: #ff0064;
    border-color: rgba(255, 0, 100, 0.3);
  }
</style>
