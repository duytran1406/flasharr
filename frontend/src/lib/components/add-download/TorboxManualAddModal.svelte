<script lang="ts">
  import { Modal, Button } from '@media-set/core-ui';
  import { toasts } from '$lib/stores/toasts';

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  let downloadRef = $state('');
  let url = $state('');
  let filename = $state('');
  let category = $state('other');
  let error = $state('');
  let submitting = $state(false);

  function jsonHeaders(): HeadersInit {
    return { 'Content-Type': 'application/json' };
  }

  function reset() {
    downloadRef = '';
    url = '';
    filename = '';
    category = 'other';
    error = '';
  }

  function close() {
    if (submitting) return;
    reset();
    onClose();
  }

  async function submit() {
    error = '';
    if (!downloadRef.trim() && !url.trim()) {
      error = 'Download ref or URL required';
      return;
    }
    submitting = true;
    try {
      const response = await fetch('/api/downloads', {
        method: 'POST',
        headers: jsonHeaders(),
        body: JSON.stringify({
          url: url.trim() || undefined,
          download_ref: downloadRef.trim() || undefined,
          filename: filename.trim() || undefined,
          category,
          source: 'torbox',
          provider: 'torbox',
        }),
      });
      if (!response.ok) {
        const text = await response.text();
        let message = `HTTP ${response.status}`;
        try {
          message = JSON.parse(text)?.error ?? message;
        } catch {
          // non-JSON body: keep the HTTP status
        }
        throw new Error(message);
      }
      toasts.success('TorBox download queued');
      close();
    } catch (err) {
      error = (err as Error).message ?? 'Failed to queue TorBox download';
    } finally {
      submitting = false;
    }
  }
</script>

<Modal {open} onClose={close} maxWidth="500px" ariaLabel="Add TorBox download">
  {#snippet header()}
    <span class="material-icons modal-icon">bolt</span><h2 class="modal-title">Add TorBox Download</h2><button
      class="close-btn"
      aria-label="Close"
      onclick={close}><span class="material-icons">close</span></button
    >
  {/snippet}

  {#snippet children()}
    <div class="form-grid">
      <label class="stack"><span>Download ref</span><input bind:value={downloadRef} placeholder="TorBox ref id" /></label
      ><label class="stack"><span>or TorBox URL</span><input bind:value={url} placeholder="https://..." /></label
      ><p class="hint">Use this for TorBox refs/cached links. Raw magnets belong in Torrent.</p><label class="stack"
        ><span>Filename</span><input bind:value={filename} placeholder="Optional" /></label
      ><label class="stack"
        ><span>Category</span><select bind:value={category}
          ><option value="other">Other</option><option value="movies">Movies</option><option value="tv">TV Shows</option
          ></select
        ></label
      >{#if error}
        <div class="err-msg"><span class="material-icons">error</span> {error}</div>
      {/if}
    </div>
  {/snippet}

  {#snippet footer()}
    <div class="modal-actions">
      <Button variant="ghost" onclick={close} disabled={submitting}>Cancel</Button><Button
        variant="primary"
        onclick={submit}
        disabled={submitting || (!downloadRef.trim() && !url.trim())}
        loading={submitting}>Queue TorBox</Button
      >
    </div>
  {/snippet}
</Modal>

<style>
  .modal-title {
    margin:0;
    font-size:.95rem;
    font-weight:800;
    text-transform:uppercase;
    letter-spacing:.06em;
    color:#fff;
  }
  .modal-icon {
    color:var(--color-primary,#00f3ff);
    font-size:1.3rem;
  }
  .modal-actions {
    display:flex;
    justify-content:flex-end;
    gap:.6rem;
  }
  .form-grid {
    display:flex;
    flex-direction:column;
    gap:.85rem;
  }
  .stack {
    display:flex;
    flex-direction:column;
    gap:.4rem;
  }
  .stack span {
    color:#94a3b8;
    font-size:.68rem;
    text-transform:uppercase;
    letter-spacing:.08em;
  }
  .hint {
    margin:-.35rem 0 .2rem;
    color:#94a3b8;
    font-size:.72rem;
    line-height:1.4;
  }
  .close-btn {
    background:none;
    border:0;
    color:#94a3b8;
    cursor:pointer;
    padding:0;
    line-height:1;
    display:inline-flex;
    align-items:center;
    border-radius:8px;
  }
  .close-btn:hover {
    color:#fff;
  }
  input,
  select {
    background:#00000047;
    border:1px solid rgba(255,255,255,.1);
    border-radius:10px;
    color:#fff;
    padding:.55rem .75rem;
  }
  .err-msg {
    display:flex;
    align-items:center;
    gap:.4rem;
    padding:.6rem .8rem;
    background:#ef44441a;
    border:1px solid rgba(239,68,68,.3);
    border-radius:8px;
    color:#ef4444;
    font-size:.82rem;
  }
</style>
