<script lang="ts">
  import { Badge, Modal } from "@media-set/core-ui";

  interface Props {
    open: boolean;
    file: any;
    onClose: () => void;
  }

  let { open, file, onClose }: Props = $props();

  function formatSize(bytes: number): string {
    if (!bytes) return "0 B";
    const k = 1024;
    const units = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + units[i];
  }

  const cached = $derived(file?.cached === true);
  const provider = $derived(file?.origin_provider ?? file?.originProvider ?? "TorBox");
  const indexer = $derived(file?.origin_indexer ?? file?.originIndexer ?? file?.indexer);
  const downloadRef = $derived(file?.download_ref ?? file?.downloadRef);
</script>

<Modal {open} {onClose} maxWidth="520px" ariaLabel="TorBox detail">
  {#snippet header()}
    <span class="material-icons modal-icon">bolt</span>
    <h2 class="modal-title">TorBox Detail</h2>
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
        <span class="label">Status</span>
        <Badge
          text={cached ? "CACHED — instant download" : "DIRECT — sourced on demand"}
          variant={cached ? "success" : "source"}
          size="sm"
        />
      </div>
      <div class="detail-row">
        <span class="label">Provider</span>
        <span class="value">{provider}</span>
      </div>
      {#if indexer}
        <div class="detail-row">
          <span class="label">Indexer</span>
          <span class="value">{indexer}</span>
        </div>
      {/if}
      {#if file.seeders != null}
        <div class="detail-row">
          <span class="label">Seeds</span>
          <span class="value seed">{file.seeders}</span>
        </div>
      {/if}
      {#if file.leechers != null}
        <div class="detail-row">
          <span class="label">Peers</span>
          <span class="value">{file.leechers}</span>
        </div>
      {/if}
      {#if file.quality}
        <div class="detail-row">
          <span class="label">Quality</span>
          <span class="value">{file.quality}</span>
        </div>
      {/if}
      {#if downloadRef}
        <div class="detail-row">
          <span class="label">Ref ID</span>
          <span class="value mono ref">{downloadRef}</span>
        </div>
      {/if}
      <div class="note">
        <span class="material-icons note-icon">info</span>
        TorBox resolves download URLs server-side.
          {cached ? "This file is already cached for instant access." : "This file will be sourced from the peer network on demand."}
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
    width: 80px;
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

  .ref {
    color: #ffffff73;
  }

  .seed {
    color: #22c55e;
    font-weight: 700;
  }

  .note {
    display: flex;
    align-items: flex-start;
    gap: .5rem;
    margin-top: .5rem;
    padding: .65rem .8rem;
    background: #a855f714;
    border: 1px solid rgba(168,85,247,.2);
    border-radius: 8px;
    font-size: .78rem;
    color: #fff9;
    line-height: 1.4;
  }

  .note-icon {
    font-size: 1rem;
    color: #a855f7;
    flex-shrink: 0;
  }
</style>
