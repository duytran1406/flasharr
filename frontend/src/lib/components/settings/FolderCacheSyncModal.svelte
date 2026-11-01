<script lang="ts" module>
  /** GET /api/settings/folder-cache/status → SyncProgress (backend providers/fshare/folder_cache.rs) */
  export interface FolderCacheSyncProgress {
    phase: string;
    total_sources: number;
    completed_sources: number;
    folders_scanned: number;
    items_found: number;
    current_source: string | null;
    current_folder: string | null;
    started_at: string | null;
    completed_at: string | null;
    outcome: string | null;
    errors: string[];
  }

  /** SyncStatus (backend providers/fshare/folder_cache.rs) */
  export interface FolderCacheStatus {
    last_sync: string | null;
    total_items: number;
    folder_count: number;
    file_count: number;
    is_syncing: boolean;
    last_diff_added: number;
    last_diff_removed: number;
    last_diff_changed: number;
    last_sync_report: string | null;
    last_error: string | null;
    progress: FolderCacheSyncProgress;
  }
</script>

<script lang="ts">
  import { Modal } from "@media-set/core-ui";

  interface Props {
    open: boolean;
    status: FolderCacheStatus | null;
    onclose: () => void;
  }

  let { open, status, onclose }: Props = $props();

  const PHASE_LABELS: Record<string, string> = {
    fetching_sources: "Fetching sources",
    scanning: "Scanning folders",
    updating_cache: "Updating cache",
    mapping_metadata: "Mapping metadata",
    complete: "Sync complete",
    failed: "Sync failed",
    idle: "Idle",
  };
</script>

<Modal {open} onClose={onclose} maxWidth="520px" ariaLabel="Folder cache sync">
  {#snippet header()}
    <div class="title"><span class="material-icons">sync</span><div><b>FOLDER CACHE</b><small>{PHASE_LABELS[status?.progress.phase ?? "idle"] ?? status?.progress.phase}</small></div></div>
    <button class="close-btn" onclick={onclose} aria-label="Close"><span class="material-icons">close</span></button>
  {/snippet}

  <div class="content" aria-busy={status?.is_syncing ?? false}>
    {#if status?.is_syncing}
      <div class="pulse" aria-hidden="true"><span></span><span></span><span></span></div>
      <p role="status" aria-live="polite">{PHASE_LABELS[status.progress.phase] ?? status.progress.phase}: {status.progress.completed_sources}/{status.progress.total_sources || "?"} sources · {status.progress.folders_scanned} folders scanned · {status.progress.items_found} items found</p>
      {#if status.progress.current_source}
        <p class="current">Source: {status.progress.current_source}</p>
      {/if}
      {#if status.progress.current_folder}
        <p class="current mono">Folder: {status.progress.current_folder}</p>
      {/if}
      <p class="hint">You can close this window. Sync continues in background.</p>
    {:else if status?.progress.outcome === "success"}
      <span class="material-icons success">check_circle</span>
      <p>Cache updated: {status.folder_count} folders · {status.file_count} files.</p>
      <p class="hint">+{status.last_diff_added} added · {status.last_diff_changed} changed · {status.last_diff_removed} removed</p>
    {:else if status?.progress.outcome === "failed"}
      <span class="material-icons failure">error</span>
      <p role="alert">Cache sync failed.</p>
    {:else}
      <p>No cache sync is running.</p>
    {/if}
    {#if status?.progress.errors.length}
      <details open={status.progress.outcome === "failed"}>
        <summary>{status.progress.errors.length} issue{status.progress.errors.length === 1 ? "" : "s"}</summary>
        <ul>
          {#each status.progress.errors as error}
            <li>{error}</li>
          {/each}
        </ul>
      </details>
    {/if}
  </div>
</Modal>

<style>
  .title {
    display: flex;
    align-items: center;
    gap: .7rem;
    color: #fff;
  }

  .title > .material-icons {
    color: #00f3ff;
  }

  .title b {
    display: block;
    letter-spacing: .1em;
    font-size: .8rem;
  }

  .title small {
    display: block;
    margin-top: .15rem;
    color: #ffffff8c;
    font-size: .7rem;
  }

  .content {
    text-align: center;
    color: #fffc;
  }

  .pulse {
    display: flex;
    justify-content: center;
    gap: .4rem;
    margin: .5rem 0 1.1rem;
  }

  .pulse span {
    width: .5rem;
    height: .5rem;
    border-radius: 50%;
    background: #00f3ff;
    animation: pulse 1s ease-in-out infinite;
  }

  .pulse span:nth-child(2) {
    animation-delay: .15s;
  }

  .pulse span:nth-child(3) {
    animation-delay: .3s;
  }

  @keyframes pulse {
    50% {
      opacity: .25;
      transform: translateY(-.3rem);
    }
  }

  .current {
    overflow-wrap: anywhere;
    text-align: left;
    border: 1px solid rgba(0,243,255,.15);
    background: #00f3ff0d;
    padding: .55rem .7rem;
    font-size: .78rem;
  }

  .mono {
    font-family: monospace;
  }

  .hint {
    font-size: .78rem;
    color: #ffffff7a;
  }

  .success,
  .failure {
    font-size: 2rem;
  }

  .success {
    color: #00d4aa;
  }

  .failure {
    color: #ff6374;
  }

  details {
    text-align: left;
    margin-top: 1rem;
    font-size: .78rem;
    color: #ffb4bc;
  }

  summary {
    cursor: pointer;
  }

  ul {
    max-height: 7rem;
    overflow: auto;
    padding-left: 1.2rem;
    word-break: break-word;
  }

</style>
