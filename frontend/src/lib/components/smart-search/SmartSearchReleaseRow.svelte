<script lang="ts">
  import { Button } from "@media-set/core-ui";
  import { isSingleFileRelease, releaseSummary, type RawSearchResult } from "./resultUtils";

  interface Props {
    file: RawSearchResult;
    rank?: number;
    onDownload?: (file: RawSearchResult, event: MouseEvent) => void;
    onDetails?: (file: RawSearchResult) => void;
  }

  let { file, rank, onDownload, onDetails }: Props = $props();

  const summary = $derived(releaseSummary(file));
  // Whole-season package rows are informational: only single files can be grabbed directly.
  const canDownload = $derived(isSingleFileRelease(file));
</script>

<div class="release-row">
  <div class="release-heading">
    {#if rank}<span class="rank">#{rank}</span>{/if}
    <span class="filename" title={file.name}>{file.name}</span>
  </div>
  <div class="signal-grid" aria-label="Release signals">
    <div class="signal"><span>Fit</span><strong>{summary.fit}</strong></div>
    <div class="signal"><span>Quality</span><strong>{summary.quality}</strong></div>
    <div class="signal"><span>Health</span><strong>{summary.health}</strong></div>
    <div class="signal"><span>Size</span><strong>{summary.size}</strong></div>
  </div>
  <div class="actions">
    <Button size="sm" icon="info_outline" variant="ghost" onclick={() => onDetails?.(file)}>Details</Button>
    {#if canDownload}
      <Button size="sm" icon="download" onclick={(event: MouseEvent) => onDownload?.(file, event)}>Get</Button>
    {/if}
  </div>
</div>

<style>
  .release-row {
    display: grid;
    grid-template-columns: minmax(0,1.5fr) minmax(360px,2fr) auto;
    gap: var(--cui-space-md);
    align-items: center;
    padding: var(--cui-space-md);
    border: 1px solid var(--cui-glass-border);
    border-radius: var(--cui-radius-md);
    background: var(--cui-input-bg);
  }

  .release-heading {
    display: flex;
    gap: var(--cui-space-sm);
    min-width: 0;
    align-items: baseline;
  }

  .rank {
    color: var(--cui-primary);
    font: 800 var(--cui-text-sm) var(--cui-font-mono);
  }

  .filename {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--cui-text-primary);
    font-size: var(--cui-text-sm);
    font-weight: 600;
  }

  .signal-grid {
    display: grid;
    grid-template-columns: repeat(4,minmax(0,1fr));
    gap: var(--cui-space-xs);
  }

  .signal {
    min-width: 0;
    padding: var(--cui-space-xs) var(--cui-space-sm);
    border-radius: var(--cui-radius-sm);
    background: var(--cui-surface-hover);
  }

  .signal span,
  .signal strong {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .signal span {
    color: var(--cui-text-tertiary);
    font-size: var(--cui-text-xs);
  }

  .signal strong {
    margin-top: 2px;
    color: var(--cui-text-primary);
    font-size: var(--cui-text-xs);
  }

  .actions {
    display: flex;
    gap: var(--cui-space-xs);
  }

  @media (max-width:760px) {
    .release-row {
      grid-template-columns: 1fr;
    }
    .filename {
      white-space: normal;
      word-break: break-word;
    }
    .signal-grid {
      grid-template-columns: repeat(2,minmax(0,1fr));
    }
    .actions {
      justify-content: stretch;
    }
    .actions :global(button) {
      flex: 1;
      min-height: var(--cui-touch-min);
    }
  }
</style>
