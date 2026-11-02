<script lang="ts">
  import { Badge, Button, Modal } from "@media-set/core-ui";
  import { downloadStore, type AddDownloadRequest } from "$lib/stores/downloads.svelte";

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  let url = $state("");
  let filename = $state("");
  let category = $state("movies");
  let priority = $state("NORMAL");
  let forceRedownload = $state(false);
  let error = $state("");
  let submitting = $state(false);
  let detectedType = $state("");

  /** Label shown next to the URL field: "Fshare", "Direct URL" or nothing. */
  function detectUrlType(value: string): string {
    if (!value) return "";
    try {
      const host = new URL(value).hostname.toLowerCase();
      return host.includes("fshare.vn") ? "Fshare" : host ? "Direct URL" : "";
    } catch {
      return "";
    }
  }

  $effect(() => {
    detectedType = detectUrlType(url);
  });

  function validate(value: string): boolean {
    if (!value.trim()) {
      error = "URL is required";
      return false;
    }
    try {
      new URL(value);
    } catch {
      error = "Invalid URL format";
      return false;
    }
    return true;
  }

  function reset() {
    url = "";
    filename = "";
    category = "movies";
    priority = "NORMAL";
    forceRedownload = false;
    error = "";
    detectedType = "";
  }

  function close() {
    if (submitting) return;
    reset();
    onClose();
  }

  function submitLabel() {
    return detectedType === "Fshare" ? "Queue Fshare" : "Queue Download";
  }

  async function submit() {
    error = "";
    if (!validate(url)) return;
    submitting = true;
    try {
      const request: AddDownloadRequest = {
        url: url.trim(),
        category: category || undefined,
        priority,
      };
      if (filename.trim()) request.filename = filename.trim();
      if (forceRedownload) request.force_redownload = true;
      const result = await downloadStore.addDownload(request);
      if (result.success) close();
      else error = result.error || "Failed to add download";
    } catch (err: any) {
      error = err.message || "An unexpected error occurred";
    } finally {
      submitting = false;
    }
  }
</script>

<Modal
  {open}
  onClose={close}
  maxWidth="520px"
  accent="var(--color-primary, #00f3ff)"
  ariaLabel="Add Fshare / direct download"
>
  {#snippet header()}
    <div class="modal-title-row"><span class="material-icons modal-icon">cloud_download</span><h2>Fshare / Direct URL</h2></div><button
      class="close-btn"
      onclick={close}
      aria-label="Close modal"><span class="material-icons">close</span></button
    >
  {/snippet}

  <form
    onsubmit={(event) => {
      event.preventDefault();
      submit();
    }}
    ><div class="form-group"
      ><label for="download-url"><span class="label-text">URL</span><span class="required">*</span></label><input
        id="download-url"
        type="url"
        bind:value={url}
        placeholder="https://fshare.vn/file/..."
        required
        disabled={submitting}
        autocomplete="off"
      />{#if detectedType}<Badge
          text={detectedType}
          variant={detectedType === "Fshare" ? "success" : "source"}
          size="sm"
        />{/if}<div class="hint">Flasharr queues direct links here. Use Torrent for magnet or .torrent intake.</div></div
    ><div class="form-group"
      ><label for="download-filename"><span class="label-text">Filename</span><span class="optional">(optional)</span></label
      ><input
        id="download-filename"
        type="text"
        bind:value={filename}
        placeholder="movie.mkv"
        disabled={submitting}
        autocomplete="off"
      /><div class="hint">Leave empty to use original filename</div></div
    ><div class="form-row"
      ><div class="form-group"
        ><label for="download-category"><span class="label-text">Category</span></label><select
          id="download-category"
          bind:value={category}
          disabled={submitting}
          ><option value="movies">Movies</option><option value="tv">TV Shows</option><option value="music">Music</option
          ><option value="other">Other</option></select
        ></div
      ><div class="form-group"
        ><label for="download-priority"><span class="label-text">Priority</span></label><select
          id="download-priority"
          bind:value={priority}
          disabled={submitting}
          ><option value="LOW">Low</option><option value="NORMAL">Normal</option><option value="HIGH">High</option></select
        ></div
      ></div
    ><label class="checkbox-row" for="force-redownload"
      ><input id="force-redownload" type="checkbox" bind:checked={forceRedownload} disabled={submitting} /><span
        ><span class="label-text">Re-download if completed</span><span class="hint">Overrides completed duplicates only</span
        ></span
      ></label
    >{#if error}<div class="error-message"><span class="material-icons">error</span><span>{error}</span></div>{/if}<div
      class="modal-actions"
      ><Button variant="ghost" size="md" type="button" onclick={close} disabled={submitting}>Cancel</Button><Button
        size="md"
        icon={submitting ? "sync" : "download"}
        loading={submitting}
        type="submit"
        disabled={submitting}>{submitting ? "Initializing…" : submitLabel()}</Button
      ></div
    ></form
  >
</Modal>

<style>
  .modal-title-row {
    display: flex;
    align-items: center;
    gap: .6rem;
  }

  .modal-icon {
    color: var(--color-primary,#00f3ff);
    font-size: 1.3rem;
  }

  h2 {
    font-size: 1.1rem;
    font-weight: 800;
    color: #fff;
    margin: 0;
  }

  .close-btn {
    background: transparent;
    border: none;
    color: var(--text-muted,#888);
    cursor: pointer;
    padding: .5rem;
    border-radius: 8px;
    transition: all .2s ease;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    background: #ffffff1a;
    color: var(--text-primary,#fff);
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: .5rem;
  }

  .form-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }

  label {
    font-size: .875rem;
    font-weight: 600;
    color: var(--text-primary,#fff);
    display: flex;
    align-items: center;
    gap: .25rem;
  }

  .required {
    color: #ff0064;
  }

  .optional {
    color: var(--text-muted,#888);
    font-weight: 400;
    font-size: .75rem;
  }

  input,
  select {
    background: #0009;
    border: 1px solid rgba(255,255,255,.1);
    padding: .8rem 1.2rem;
    color: var(--text-primary,#fff);
    font-size: .875rem;
    font-family: var(--font-mono,monospace);
    transition: all .2s ease;
    border-radius: 10px;
  }

  input:focus,
  select:focus {
    outline: none;
    border-color: var(--color-primary,#00f3ff);
    background: #00f3ff0d;
    box-shadow: none;
  }

  .checkbox-row {
    align-items: flex-start;
    background: #ffffff0a;
    border: 1px solid rgba(255,255,255,.08);
    border-radius: 8px;
    cursor: pointer;
    gap: .75rem;
    padding: .8rem 1rem;
  }

  .checkbox-row input {
    cursor: pointer;
    margin-top: .1rem;
    padding: 0;
    width: 1rem;
  }

  .checkbox-row .hint {
    display: block;
    margin-top: .2rem;
  }

  .hint {
    font-size: .65rem;
    color: var(--text-muted,#888);
    margin-top: .25rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: .05em;
  }

  .error-message {
    display: flex;
    align-items: center;
    gap: .5rem;
    padding: .75rem 1rem;
    background: #ff00641a;
    border: 1px solid rgba(255,0,100,.3);
    border-radius: 8px;
    color: #ff0064;
    font-size: .875rem;
  }

  .error-message .material-icons {
    font-size: 1.25rem;
  }

  .modal-actions {
    display: flex;
    gap: 1rem;
    margin-top: 1rem;
  }

  @media (max-width:640px) {
    .form-row {
      grid-template-columns: 1fr;
    }
    .modal-actions {
      flex-direction: column;
    }
  }
</style>
