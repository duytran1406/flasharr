<script lang="ts">
  import { Button, Modal } from "@media-set/core-ui";
  import { qbitStore, type QbitAddTorrentOptions } from "$lib/stores/qbit.svelte";
  import { toasts } from "$lib/stores/toasts";
  import { formatTorrentBytes } from "./torrentFormat";

  /**
   * Two modes:
   * - queue: `file` is a torrent search result; it is queued through
   *   POST /api/downloads (provider qbittorrent) so Flasharr tracks it.
   * - add: no `file`; a magnet/URL and/or .torrent file is added straight to
   *   the selected qBittorrent instance.
   */
  interface Props {
    open: boolean;
    onClose: () => void;
    onSuccess?: () => void;
    /** Torrent search result to queue (snake_case or camelCase fields). */
    file?: Record<string, any> | null;
    category?: string;
    /** TMDB metadata forwarded with the queued download. */
    tmdb?: Record<string, any> | null;
  }

  let { open, onClose, onSuccess, file = null, category = "other", tmdb }: Props = $props();

  const MAX_TORRENT_FILE_BYTES = 10 * 1024 * 1024;

  let magnet = $state("");
  let torrentFile = $state<File | null>(null);
  let addCategory = $state("");
  let savePath = $state("");
  let tags = $state("");
  let startPaused = $state(false);
  let skipChecking = $state(false);
  let sequential = $state(false);
  let firstLastPiecePrio = $state(false);
  let submitting = $state(false);
  let dragging = $state(false);
  let error = $state("");

  const isQueue = $derived(!!file);
  const infoHash = $derived(file?.info_hash ?? file?.infoHash);
  const indexer = $derived(file?.indexer ?? file?.origin_indexer ?? file?.originIndexer);

  function resetForm() {
    magnet = "";
    torrentFile = null;
    addCategory = "";
    savePath = "";
    tags = "";
    startPaused = false;
    skipChecking = false;
    sequential = false;
    firstLastPiecePrio = false;
    dragging = false;
  }

  function close() {
    if (submitting) return;
    error = "";
    if (!isQueue) resetForm();
    onClose();
  }

  function jsonHeaders(): HeadersInit {
    return { "Content-Type": "application/json" };
  }

  function buildQueueRequest() {
    const downloadRef = file?.download_ref ?? file?.downloadRef;
    const fileIdx = file?.file_idx ?? file?.fileIdx;
    const hintedFilename = file?.behavior_hints?.filename ?? file?.behaviorHints?.filename;
    const selectedEpisodes = file?._episodeContext ? JSON.stringify([file._episodeContext]) : undefined;
    const url = downloadRef ? undefined : file?.url;
    if (!infoHash && !downloadRef && !url) {
      throw new Error("No info hash, magnet URL, or download reference available");
    }
    return {
      url,
      download_ref: downloadRef,
      filename: file?.name,
      category,
      source: "torrent",
      provider: "qbittorrent",
      seeders: file?.seeders,
      leechers: file?.leechers,
      info_hash: infoHash,
      file_idx: fileIdx,
      source_filename: hintedFilename ?? (fileIdx !== undefined ? file?.name : undefined),
      selected_episodes: selectedEpisodes,
      size: file?.size,
      qbit_instance_id: qbitStore.selectedInstanceId || undefined,
      ...(tmdb ? { tmdb } : {}),
    };
  }

  function addOptions(): QbitAddTorrentOptions {
    return {
      category: addCategory.trim() || undefined,
      savepath: savePath.trim() || undefined,
      tags: tags.trim() || undefined,
      paused: startPaused || undefined,
      stopped: startPaused || undefined,
      skip_checking: skipChecking || undefined,
      sequential_download: sequential || undefined,
      first_last_piece_prio: firstLastPiecePrio || undefined,
    };
  }

  function pickTorrentFile(files: FileList | null | undefined) {
    const picked = files?.[0];
    if (!picked) return;
    if (picked.size === 0) return toasts.error("Torrent file is empty");
    if (picked.size > MAX_TORRENT_FILE_BYTES) return toasts.error("Torrent file exceeds 10 MiB");
    torrentFile = picked;
    error = "";
  }

  async function queueDownload() {
    const response = await fetch("/api/downloads", {
      method: "POST",
      headers: jsonHeaders(),
      body: JSON.stringify(buildQueueRequest()),
    });
    if (!response.ok) {
      const text = await response.text();
      let message = `HTTP ${response.status}`;
      try {
        const body = JSON.parse(text);
        message = body?.message ? `${body.error ?? message}: ${body.message}` : (body?.error ?? message);
      } catch {
        // not JSON: keep the status line
      }
      throw new Error(message);
    }
    toasts.success("Torrent queued");
    onSuccess?.();
  }

  function failureMessage() {
    return isQueue ? "Failed to queue torrent" : "Failed to add torrent to qBittorrent";
  }

  async function addToQbit() {
    if (!magnet.trim() && !torrentFile) throw new Error("Magnet URL or .torrent file required");
    if (torrentFile) {
      const result = await qbitStore.addTorrentFile(torrentFile, addOptions());
      if (!result?.success) throw new Error(result?.error || "Failed to add torrent file");
    }
    if (magnet.trim()) {
      const result = await qbitStore.addTorrent(magnet.trim(), addOptions());
      if (!result?.success) throw new Error(result?.error || "Failed to add torrent URL");
    }
    resetForm();
    await qbitStore.fetchTorrents();
  }

  async function submit() {
    error = "";
    submitting = true;
    try {
      if (isQueue) await queueDownload();
      else await addToQbit();
      error = "";
      if (!isQueue) resetForm();
      onClose();
    } catch (err: any) {
      error = err?.message || failureMessage();
    } finally {
      submitting = false;
    }
  }
</script>

<Modal
  {open}
  onClose={close}
  ariaLabel={isQueue ? "Queue torrent download" : "Add torrent"}
  maxWidth={isQueue ? "480px" : "540px"}
>
  {#snippet header()}
    <span class="modal-icon material-icons">{isQueue ? "download" : "add_circle_outline"}</span>
    <h2 class="modal-title">{isQueue ? "Queue torrent" : "Add torrent"}</h2>
    <button class="close-btn" aria-label="Close" onclick={close}><span class="material-icons">close</span></button>
  {/snippet}

  {#if isQueue && file}
    <div class="form-grid">
      <div class="field"><span class="label">File</span><span class="value mono">{file.name}</span></div>
      <div class="field">
        <span class="label">Source</span><span class="value"
          >torrent · {file.seeders ?? 0} seed · {file.leechers ?? 0} leech</span
        >
      </div>
      {#if infoHash}
        <div class="field">
          <span class="label">Hash</span><span class="value mono hash">{infoHash.slice(0, 16)}…</span>
        </div>
      {/if}
      {#if indexer}
        <div class="field"><span class="label">Indexer</span><span class="value">{indexer}</span></div>
      {/if}
      {#if error}
        <div class="err-msg"><span class="material-icons">error</span> {error}</div>
      {/if}
    </div>
  {:else}
    <div class="form-grid">
      <label class="stack">
        <span>Magnet / URL</span><textarea
          bind:value={magnet}
          placeholder="magnet:?xt=urn:btih:… or https://…"
          rows="4"
        ></textarea>
      </label>
      <label class="stack">
        <span>.torrent file</span><input
          type="file"
          accept=".torrent,application/x-bittorrent"
          onchange={(e) => pickTorrentFile(e.currentTarget.files)}
        />
      </label>
      <button
        type="button"
        class="dropzone"
        class:dragging
        ondragenter={(e) => {
          e.preventDefault();
          dragging = true;
        }}
        ondragover={(e) => e.preventDefault()}
        ondragleave={(e) => {
          e.preventDefault();
          dragging = false;
        }}
        ondrop={(e) => {
          e.preventDefault();
          dragging = false;
          pickTorrentFile(e.dataTransfer?.files ?? null);
        }}
      >
        {#if torrentFile}
          <strong>{torrentFile.name}</strong><small>{formatTorrentBytes(torrentFile.size)}</small>
        {:else}
          <strong>Drop .torrent here</strong><small>or use file picker above</small>
        {/if}
      </button>
      <div class="grid-two">
        <label class="stack"><span>Category</span><input bind:value={addCategory} placeholder="tv" /></label><label
          class="stack"><span>Save path</span><input bind:value={savePath} placeholder="/downloads/tv" /></label
        >
      </div>
      <label class="stack"><span>Tags</span><input bind:value={tags} placeholder="anime,manual" /></label>
      <div class="checks">
        <label><input type="checkbox" bind:checked={startPaused} />Start paused</label><label
          ><input type="checkbox" bind:checked={skipChecking} />Skip checking</label
        ><label><input type="checkbox" bind:checked={sequential} />Sequential</label><label
          ><input type="checkbox" bind:checked={firstLastPiecePrio} />First/last piece prio</label
        >
      </div>
      {#if error}
        <div class="err-msg"><span class="material-icons">error</span> {error}</div>
      {/if}
    </div>
  {/if}

  {#snippet footer()}
    <div class="modal-actions">
      <Button variant="ghost" onclick={close} disabled={submitting}>Cancel</Button>
      <Button
        variant="primary"
        onclick={submit}
        disabled={submitting || (!isQueue && !magnet.trim() && !torrentFile)}
        loading={submitting}>{isQueue ? (submitting ? "Queueing…" : "Queue torrent") : "Add"}</Button
      >
    </div>
  {/snippet}
</Modal>

<style>
	.modal-title {
		margin: 0;
		font-size: .95rem;
		font-weight: 800;
		text-transform: uppercase;
		letter-spacing: .06em;
		color: #fff;
	}

	.modal-icon {
		color: var(--color-primary,#00f3ff);
		font-size: 1.3rem;
	}

	.modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: .6rem;
		padding-bottom: env(safe-area-inset-bottom);
	}

	.form-grid {
		display: flex;
		flex-direction: column;
		gap: .85rem;
	}

	.grid-two {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: .7rem;
	}

	.field {
		display: flex;
		gap: 1rem;
		align-items: flex-start;
	}

	.label {
		width: 70px;
		flex-shrink: 0;
		font-size: .68rem;
		font-weight: 800;
		color: #64748b;
		text-transform: uppercase;
		letter-spacing: .08em;
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

	.hash {
		color: #ffffff80;
	}

	.checks {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: .45rem;
		color: #cbd5e1;
	}

	.checks label {
		display: flex;
		gap: .4rem;
		align-items: center;
		color: #cbd5e1;
	}

	.stack {
		display: flex;
		flex-direction: column;
		gap: .4rem;
	}

	.stack span {
		color: #94a3b8;
		font-size: .68rem;
		text-transform: uppercase;
		letter-spacing: .08em;
	}

	.dropzone {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: .3rem;
		min-height: 88px;
		border: 1px dashed rgba(0,243,255,.28);
		background: #00f3ff0a;
		color: #cbd5e1;
		border-radius: 12px;
		cursor: pointer;
	}

	.dropzone.dragging {
		border-color: #00f3ff;
		background: #00f3ff1f;
	}

	.close-btn {
		background: none;
		border: 0;
		color: #94a3b8;
		cursor: pointer;
		padding: 0;
		line-height: 1;
		display: inline-flex;
		align-items: center;
		border-radius: 8px;
	}

	.close-btn:hover {
		color: var(--color-primary,#00f3ff);
	}

	input,
	textarea {
		background: #00000047;
		border: 1px solid rgba(255,255,255,.1);
		border-radius: 10px;
		color: #fff;
		padding: .55rem .75rem;
	}

	.checks input,
	input[type=file] {
		min-width: auto;
		padding: 0;
	}

	textarea {
		resize: vertical;
	}

	.err-msg {
		display: flex;
		align-items: center;
		gap: .4rem;
		padding: .6rem .8rem;
		background: #ef44441a;
		border: 1px solid rgba(239,68,68,.3);
		border-radius: 10px;
		color: #ef4444;
		font-size: .82rem;
	}

	.err-msg .material-icons {
		font-size: 1.1rem;
	}

	@media (max-width:720px) {
		.grid-two,
		.checks {
			grid-template-columns: 1fr;
		}

		.field {
			flex-direction: column;
			gap: .35rem;
		}

		.label {
			width: auto;
		}

		.modal-actions {
			flex-direction: column-reverse;
		}

		.modal-actions :global(button) {
			width: 100%;
		}
	}
</style>
