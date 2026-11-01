<script lang="ts">
  import { Badge, Button, Modal } from "@media-set/core-ui";

  interface Props {
    open: boolean;
    file: any;
    onClose: () => void;
  }

  let { open, file, onClose }: Props = $props();

  let copied = $state(false);

  function formatSize(bytes: number): string {
    if (!bytes) return "0 B";
    const k = 1024;
    const units = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + units[i];
  }

  async function copyUrl() {
    if (!file?.url) return;
    await navigator.clipboard.writeText(file.url);
    copied = true;
    setTimeout(() => (copied = false), 2000);
  }
</script>

<Modal {open} {onClose} maxWidth="520px" ariaLabel="Fshare file detail">
  {#snippet header()}
    <span class="material-icons modal-icon">folder_open</span>
    <h2 class="modal-title">Fshare File Detail</h2>
    <button class="close-btn" onclick={onClose} aria-label="Close">
      <span class="material-icons">close</span>
    </button>
  {/snippet}

  {#if file}
    <div class="detail-body">
      <div class="detail-row">
        <span class="label">Filename</span>
        <span class="value mono">{file.name}</span>
      </div>
      <div class="detail-row">
        <span class="label">Size</span>
        <span class="value">{formatSize(file.size ?? 0)}</span>
      </div>
      {#if file.quality}
        <div class="detail-row">
          <span class="label">Quality</span>
          <span class="value">{file.quality}</span>
        </div>
      {/if}
      {#if file.url}
        <div class="detail-row">
          <span class="label">URL</span>
          <span class="value url-value">{file.url}</span>
        </div>
      {/if}
      <div class="flag-row">
        {#if file.vietsub}<Badge text="VIETSUB" variant="success" size="sm" />{/if}
        {#if file.vietdub}<Badge text="VIETDUB" variant="language" size="sm" />{/if}
        {#if file.hdr}<Badge text="HDR" variant="hdr" size="sm" noDot />{/if}
        {#if file.dolby_vision}<Badge text="DOLBY VISION" variant="dv" size="sm" noDot />{/if}
      </div>
      <div class="action-row">
        {#if file.url}
          <Button variant="ghost" size="sm" icon={copied ? "check" : "content_copy"} onclick={copyUrl}>
            {copied ? "Copied" : "Copy URL"}
          </Button>
          <a href={file.url} target="_blank" rel="noopener noreferrer">
            <Button variant="ghost" size="sm" icon="open_in_new">Open</Button>
          </a>
        {/if}
      </div>
    </div>
  {/if}
</Modal>

<style>
  .modal-icon {
    color: var(--color-primary, #00f3ff);
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

  .detail-body {
    display: flex;
    flex-direction: column;
    gap: .75rem;
  }

  .detail-row {
    display: flex;
    gap: 1rem;
    align-items: flex-start;
  }

  .label {
    width: 90px;
    flex-shrink: 0;
    font-size: .7rem;
    font-weight: 700;
    color: #64748b;
    text-transform: uppercase;
    letter-spacing: .05em;
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
    font-size: .75rem;
  }

  .url-value {
    font-family: var(--font-mono);
    font-size: .72rem;
    color: var(--color-primary, #00f3ff);
  }

  .flag-row {
    display: flex;
    gap: .4rem;
    flex-wrap: wrap;
    margin-top: .25rem;
  }

  .action-row {
    display: flex;
    gap: .5rem;
    margin-top: .5rem;
  }

  a {
    text-decoration: none;
  }
</style>
