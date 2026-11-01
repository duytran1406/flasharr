<script lang="ts">
  import { Badge, Button, Modal } from "@media-set/core-ui";
  import { toasts } from "$lib/stores/toasts";
  import { qbitStore } from "$lib/stores/qbit.svelte";

  interface Props {
    open: boolean;
    file: any;
    onClose: () => void;
    onSuccess?: () => void;
    tmdb?: any;
    category?: string;
  }

  let { open, file, onClose, onSuccess, tmdb, category = "other" }: Props = $props();

  let submitting = $state(false);
  let error = $state("");
  /** Set when TorBox could not resolve a cached torrent: offers a qBittorrent fallback for this hash. */
  let fallbackHash = $state<string | null>(null);

  const cached = $derived(file?.cached === true);
  const provider = $derived(file?.origin_provider ?? file?.originProvider ?? "TorBox");
  const indexer = $derived(file?.origin_indexer ?? file?.originIndexer ?? file?.indexer);

  function headers() {
    return { "Content-Type": "application/json" };
  }

  function buildPayload() {
    const downloadRef = file?.download_ref ?? file?.downloadRef;
    const infoHash = file?.info_hash ?? file?.infoHash;
    const fileIdx = file?.file_idx ?? file?.fileIdx;
    const hintedFilename = file?.behavior_hints?.filename ?? file?.behaviorHints?.filename;
    const selectedEpisodes = file?._episodeContext ? JSON.stringify([file._episodeContext]) : undefined;
    const url = downloadRef ? undefined : file?.url;
    if (!infoHash && !downloadRef && !url) {
      throw new Error("No TorBox info hash, download reference, or URL available");
    }
    return {
      url,
      download_ref: downloadRef,
      filename: file?.name,
      category,
      source: "torbox",
      provider: "torbox",
      seeders: file?.seeders,
      leechers: file?.leechers,
      info_hash: infoHash,
      file_idx: fileIdx,
      source_filename: hintedFilename ?? (fileIdx !== undefined ? file?.name : undefined),
      selected_episodes: selectedEpisodes,
      origin_provider: file?.origin_provider ?? file?.originProvider,
      origin_indexer: file?.origin_indexer ?? file?.originIndexer,
      size: file?.size,
      ...(tmdb ? { tmdb } : {}),
    };
  }

  async function postDownload(payload: Record<string, unknown>) {
    const response = await fetch("/api/downloads", {
      method: "POST",
      headers: headers(),
      body: JSON.stringify(payload),
    });
    if (response.ok) return;
    const body = await response.json().catch(() => null);
    if (body?.error === "torbox_resolution_failed" && body?.fallback?.info_hash) {
      fallbackHash = body.fallback.info_hash;
      error = body.message ?? "TorBox could not resolve this cached torrent.";
      return;
    }
    throw new Error(body?.message ?? body?.error ?? `HTTP ${response.status}`);
  }

  async function submit() {
    error = "";
    fallbackHash = null;
    submitting = true;
    try {
      await postDownload(buildPayload());
      if (fallbackHash) return;
      toasts.success("TorBox download queued");
      onSuccess?.();
      onClose();
    } catch (err: any) {
      error = err.message ?? "Failed to queue TorBox download";
    } finally {
      submitting = false;
    }
  }

  async function useQbittorrent() {
    if (!fallbackHash) return;
    const infoHash = fallbackHash;
    error = "";
    fallbackHash = null;
    submitting = true;
    try {
      const payload = {
        ...buildPayload(),
        source: "torrent",
        provider: "qbittorrent",
        info_hash: infoHash,
        qbit_instance_id: qbitStore.selectedInstanceId || undefined,
      };
      await postDownload(payload);
      if (fallbackHash) return;
      toasts.success("qBittorrent download queued");
      onSuccess?.();
      onClose();
    } catch (err: any) {
      error = err.message ?? "Failed to queue qBittorrent download";
    } finally {
      submitting = false;
    }
  }

  function cancelFallback() {
    fallbackHash = null;
    error = "";
  }

  function close() {
    if (submitting) return;
    fallbackHash = null;
    error = "";
    onClose();
  }
</script>

<Modal {open} onClose={close} maxWidth="480px" ariaLabel="Add TorBox download">
  {#snippet header()}
    <span class="material-icons modal-icon">bolt</span>
    <h2 class="modal-title">Add TorBox Download</h2>
    <button class="close-btn" onclick={close} aria-label="Close">
      <span class="material-icons">close</span>
    </button>
  {/snippet}

  {#if file}
    <div class="confirm-body">
      <div class="field">
        <span class="label">File</span>
        <span class="value mono">{file.name}</span>
      </div>
      <div class="field">
        <span class="label">Status</span>
        <Badge text={cached ? "CACHED" : "DIRECT"} variant={cached ? "success" : "source"} size="sm" />
      </div>
      <div class="field">
        <span class="label">Via</span>
        <span class="value">{provider}{indexer ? ` · ${indexer}` : ""}</span>
      </div>
      {#if error}
        <div class="err-msg">
          <span class="material-icons">error</span>
          {error}
        </div>
      {/if}
      <div class="actions">
        {#if fallbackHash}
          <Button variant="ghost" size="md" onclick={cancelFallback} disabled={submitting}>Cancel</Button>
          <Button
            size="md"
            icon={submitting ? "sync" : "download"}
            loading={submitting}
            onclick={useQbittorrent}
            disabled={submitting}
          >
            Use qBittorrent
          </Button>
        {:else}
          <Button variant="ghost" size="md" onclick={close} disabled={submitting}>Cancel</Button>
          <Button size="md" icon={submitting ? "sync" : "bolt"} loading={submitting} onclick={submit} disabled={submitting}>
            {#if submitting}
              Queueing…
            {:else if cached}
              Download (Cached)
            {:else}
              Download via TorBox
            {/if}
          </Button>
        {/if}
      </div>
    </div>
  {/if}
</Modal>

<style>
  .modal-icon {
    color: #a855f7;
    font-size: 1.2rem;
  }

  .modal-title {
    margin: 0;
    font-size: .95rem;
    font-weight: 800;
    color: #fff;
  }

  .close-btn {
    background: none;
    border: none;
    color: #94a3b8;
    cursor: pointer;
    display: flex;
    align-items: center;
  }

  .close-btn:hover {
    color: #fff;
  }

  .confirm-body {
    display: flex;
    flex-direction: column;
    gap: .75rem;
  }

  .field {
    display: flex;
    gap: 1rem;
    align-items: flex-start;
  }

  .label {
    width: 70px;
    flex-shrink: 0;
    font-size: .7rem;
    font-weight: 700;
    color: #64748b;
    text-transform: uppercase;
    letter-spacing: .04em;
    padding-top: .1rem;
  }

  .value {
    flex: 1;
    font-size: .82rem;
    color: #ffffffd9;
    word-break: break-all;
  }

  .mono {
    font-family: var(--font-mono);
    font-size: .72rem;
  }

  .err-msg {
    display: flex;
    align-items: center;
    gap: .4rem;
    padding: .6rem .8rem;
    background: #ef44441a;
    border: 1px solid rgba(239,68,68,.3);
    border-radius: 8px;
    color: #ef4444;
    font-size: .82rem;
  }

  .err-msg .material-icons {
    font-size: 1.1rem;
  }

  .actions {
    display: flex;
    gap: .75rem;
    margin-top: .25rem;
  }
</style>
