<script lang="ts">
  import { Button, Modal } from "@media-set/core-ui";
  import { toasts } from "$lib/stores/toasts";
  import { buildFshareDownloadPayload } from "./fsharePayload";

  interface Props {
    open: boolean;
    /** Search result to queue; in manual mode only its `name` is used. */
    file: any;
    onClose: () => void;
    onSuccess?: () => void;
    tmdb?: any;
    category?: string;
    /** Manual mode asks for an Fshare file page URL instead of using the result's URL. */
    manual?: boolean;
  }

  let { open, file, onClose, onSuccess, tmdb, category = "other", manual = false }: Props = $props();

  let filenameOverride = $state("");
  let manualUrl = $state("");
  let submitting = $state(false);
  let error = $state("");

  function headers() {
    return { "Content-Type": "application/json" };
  }

  function buildPayload() {
    const url = manual ? manualUrl.trim() : (file?.url ?? "");
    if (!url) throw new Error("No URL available for this Fshare file");
    return buildFshareDownloadPayload(file, url, filenameOverride, category, tmdb);
  }

  async function submit() {
    error = "";
    submitting = true;
    try {
      const payload = buildPayload();
      const response = await fetch("/api/downloads", {
        method: "POST",
        headers: headers(),
        body: JSON.stringify(payload),
      });
      if (!response.ok) {
        const text = await response.text();
        let message = `HTTP ${response.status}`;
        try {
          message = JSON.parse(text)?.error ?? message;
        } catch {}
        throw new Error(message);
      }
      toasts.success("Download added");
      filenameOverride = "";
      manualUrl = "";
      onSuccess?.();
      onClose();
    } catch (err: any) {
      error = err.message ?? "Failed to add download";
    } finally {
      submitting = false;
    }
  }

  function close() {
    if (submitting) return;
    filenameOverride = "";
    manualUrl = "";
    error = "";
    onClose();
  }
</script>

<Modal {open} onClose={close} maxWidth="480px" ariaLabel="Add Fshare download">
  {#snippet header()}
    <span class="material-icons modal-icon">cloud_download</span>
    <h2 class="modal-title">{manual ? "Add Fshare URL" : "Add Fshare Download"}</h2>
    <button class="close-btn" onclick={close} aria-label="Close">
      <span class="material-icons">close</span>
    </button>
  {/snippet}

  <form
    class="form-body"
    onsubmit={(event) => {
      event.preventDefault();
      submit();
    }}
  >
    <div class="field">
      <span class="field-label">File</span>
      <span class="field-value mono">{file?.name ?? "—"}</span>
    </div>

    {#if manual}
      <div class="field-group">
        <label for="fshare-manual-url" class="field-label">Fshare file URL</label>
        <input
          id="fshare-manual-url"
          type="url"
          bind:value={manualUrl}
          placeholder="https://www.fshare.vn/file/..."
          required=""
          disabled={submitting}
        />
        <span class="hint">Fshare file page only. Direct download links are unsupported.</span>
      </div>
    {:else if file?.url}
      <div class="field">
        <span class="field-label">URL</span>
        <span class="field-value mono url-val">{file.url}</span>
      </div>
    {/if}

    <div class="field-group">
      <label for="fshare-filename" class="field-label">Filename override</label>
      <input
        id="fshare-filename"
        type="text"
        bind:value={filenameOverride}
        placeholder={file?.name ?? "Leave empty for original"}
        disabled={submitting}
      />
      <span class="hint">Optional. Defaults to original filename.</span>
    </div>

    {#if error}
      <div class="err-msg">
        <span class="material-icons">error</span>
        {error}
      </div>
    {/if}

    <div class="actions">
      <Button variant="ghost" size="md" type="button" onclick={close} disabled={submitting}>Cancel</Button>
      <Button
        size="md"
        icon={submitting ? "sync" : "download"}
        loading={submitting}
        type="submit"
        disabled={submitting}
      >
        {submitting ? "Adding…" : "Add Download"}
      </Button>
    </div>
  </form>
</Modal>

<style>
  .modal-icon {
    color: var(--cui-primary);
    font-size: var(--cui-text-xl);
  }

  .modal-title {
    margin: 0;
    font-size: var(--cui-text-base);
    font-weight: 800;
    color: var(--cui-text-primary);
  }

  .close-btn {
    background: none;
    border: none;
    color: var(--cui-text-secondary);
    cursor: pointer;
    display: flex;
    align-items: center;
  }

  .close-btn:hover {
    color: var(--cui-text-primary);
  }

  .form-body {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .field {
    display: flex;
    gap: .75rem;
    align-items: flex-start;
  }

  .field-label {
    width: 80px;
    flex-shrink: 0;
    font-size: .7rem;
    font-weight: 700;
    color: #64748b;
    text-transform: uppercase;
    letter-spacing: .04em;
    padding-top: .1rem;
  }

  .field-value {
    flex: 1;
    font-size: .82rem;
    color: #fffc;
    word-break: break-all;
  }

  .mono {
    font-family: var(--font-mono);
    font-size: .75rem;
  }

  .url-val {
    color: var(--color-primary, #00f3ff);
    font-size: .7rem;
  }

  .field-group {
    display: flex;
    flex-direction: column;
    gap: .35rem;
  }

  input {
    background: var(--cui-input-bg);
    border: 1px solid var(--cui-input-border);
    border-radius: var(--cui-radius-sm);
    color: var(--cui-text-primary);
    padding: var(--cui-space-xs) var(--cui-space-sm);
    font-size: var(--cui-text-sm);
    font-family: var(--cui-font-mono);
  }

  input:focus-visible {
    border-color: var(--cui-glass-border-active);
    box-shadow: var(--cui-glow-primary);
  }

  .hint {
    font-size: .65rem;
    color: #64748b;
    text-transform: uppercase;
    letter-spacing: .04em;
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
