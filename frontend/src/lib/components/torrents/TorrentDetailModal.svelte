<script lang="ts">
  import { Button, Modal } from "@media-set/core-ui";

  interface Props {
    open: boolean;
    file: any;
    onClose: () => void;
  }

  let { open, file, onClose }: Props = $props();

  let hashCopied = $state(false);

  const infoHash = $derived(file?.info_hash ?? file?.infoHash);
  const indexer = $derived(file?.indexer ?? file?.origin_indexer ?? file?.originIndexer);

  function formatSize(bytes: number): string {
    if (!bytes) return "0 B";
    const k = 1024;
    const units = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + units[i];
  }

  async function copyHash() {
    if (!infoHash) return;
    await navigator.clipboard.writeText(infoHash);
    hashCopied = true;
    setTimeout(() => (hashCopied = false), 2000);
  }

  function magnetLink(): string | undefined {
    if (!infoHash) return;
    const displayName = file?.name ? `&dn=${encodeURIComponent(file.name)}` : "";
    return `magnet:?xt=urn:btih:${infoHash}${displayName}`;
  }
</script>

<Modal {open} {onClose} maxWidth="560px" ariaLabel="Torrent detail">
  {#snippet header()}
    <span class="modal-icon material-icons">security</span>
    <h2 class="modal-title">Torrent detail</h2>
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
      <div class="detail-row">
        <span class="label">Source</span>
        <span class="value">torrent · {file.seeders ?? 0} seed · {file.leechers ?? 0} leech</span>
      </div>
      {#if indexer}
        <div class="detail-row">
          <span class="label">Indexer</span>
          <span class="value">{indexer}</span>
        </div>
      {/if}
      {#if infoHash}
        <div class="detail-row">
          <span class="label">Info hash</span>
          <div class="hash-row">
            <span class="value mono hash">{infoHash}</span>
            <Button
              size="sm"
              variant="ghost"
              icon={hashCopied ? "check" : "content_copy"}
              onclick={copyHash}
              title={hashCopied ? "Hash copied" : "Copy hash"}
            >
              <span class="sr-only">{hashCopied ? "Hash copied" : "Copy hash"}</span>
            </Button>
          </div>
        </div>
        {#if magnetLink()}
          <div class="detail-row">
            <span class="label">Magnet</span>
            <a class="magnet-link value mono" href={magnetLink()} target="_blank" rel="noopener noreferrer">magnet link</a>
          </div>
        {/if}
      {/if}
      {#if file.quality}
        <div class="detail-row">
          <span class="label">Quality</span>
          <span class="value">{file.quality}</span>
        </div>
      {/if}
    </div>
  {/if}

  {#snippet footer()}
    <div class="modal-actions">
      <Button variant="ghost" onclick={onClose}>Close</Button>
    </div>
  {/snippet}
</Modal>

<style>
  .modal-icon {
    color: var(--color-primary,#00f3ff);
    font-size: 1.2rem;
  }

  .modal-title {
    margin: 0;
    font-size: .95rem;
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: .06em;
    color: #fff;
  }

  .close-btn {
    background: none;
    border: none;
    color: #94a3b8;
    cursor: pointer;
    display: flex;
    align-items: center;
    border-radius: 8px;
  }

  .close-btn:hover {
    color: #fff;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: .6rem;
    padding-bottom: env(safe-area-inset-bottom);
  }

  .detail-body {
    display: flex;
    flex-direction: column;
    gap: .65rem;
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
    font-size: .72rem;
  }

  .hash {
    font-size: .65rem;
    color: #ffffff80;
  }

  .hash-row {
    display: flex;
    align-items: center;
    gap: .5rem;
    flex: 1;
  }

  .magnet-link {
    color: var(--color-primary,#00f3ff);
    font-size: .72rem;
    font-family: var(--font-mono);
  }

  @media (max-width:720px) {
    .detail-row {
      flex-direction: column;
      gap: .35rem;
    }
    .label {
      width: auto;
    }
    .hash-row {
      flex-direction: column;
      align-items: flex-start;
    }
    .modal-actions {
      flex-direction: column-reverse;
    }
    .modal-actions :global(button) {
      width: 100%;
    }
  }
</style>
