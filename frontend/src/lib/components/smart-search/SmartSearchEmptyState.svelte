<script lang="ts">
  interface Props {
    results?: any;
  }

  let { results = null }: Props = $props();

  /** Torrent-side providers that did not answer cleanly, e.g. "Prowlarr: timeout". */
  const sourceIssues = $derived(
    ["torrent", "torbox"].flatMap((key) => {
      const status = results?.sources?.[key];
      return status && status.state !== "ok"
        ? [`${key === "torrent" ? "Prowlarr" : "TorBox"}: ${status.message || status.state}`]
        : [];
    }),
  );
</script>

<div class="empty-state" role="status" aria-live="polite">
  <span class="material-icons">search_off</span>
  <p>No high-quality matches found.</p>
  <span class="hint">Try another source tab, broader title, or manual add.</span>
  {#if sourceIssues.length > 0}
    <span class="source-status">{sourceIssues.join(" · ")}</span>
  {/if}
</div>

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 300px;
    gap: var(--cui-space-md);
    color: var(--cui-text-secondary);
    text-align: center;
  }

  p {
    margin: 0;
  }

  .hint {
    font-size: var(--cui-text-xs);
    color: var(--cui-text-tertiary);
  }

  .source-status {
    color: #ffb84dd9;
    font-size: .72rem;
  }
</style>
