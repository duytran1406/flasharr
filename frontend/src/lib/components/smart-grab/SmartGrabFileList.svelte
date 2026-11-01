<script lang="ts">
  import { Badge } from "@media-set/core-ui";
  import SourceSummary from "../smart-search/SourceSummary.svelte";
  import { formatResultSize } from "../smart-search/resultUtils";
  import type { SmartGrabFile } from "./smartGrabUtils";

  interface Props {
    files: SmartGrabFile[];
  }

  let { files }: Props = $props();

  const fileLabel = (entry: SmartGrabFile) =>
    entry.file.packageSelection
      ? `Season ${entry.seasonNum} · Episode ${entry.epNum} · package file #${entry.file.file_idx}`
      : entry.file.name;
</script>

<div class="files-list" role="list">
  {#each files as entry}
    <div class="file-item" role="listitem">
      <Badge text={`E${entry.epNum}`} variant={entry.isFallback ? "warning" : "primary"} size="xs" />
      <div class="file-main">
        <div class="file-name" title={fileLabel(entry)}>{fileLabel(entry)}</div>
        <SourceSummary file={entry.file} kind="episode" compact />
      </div>
      <span class="file-size"
        >{entry.file.packageSelection && !entry.file.size ? "Package item" : formatResultSize(entry.file.size ?? 0)}</span
      >
    </div>
  {/each}
</div>

<style>
  .files-list {
    margin-top: 12px;
    padding: 12px;
    background: #0000004d;
    border-radius: 8px;
    border: 1px solid rgba(255,255,255,.06);
    max-height: 300px;
    overflow-y: auto;
  }

  .file-item {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 8px 0;
    border-bottom: 1px solid rgba(255,255,255,.05);
  }

  .file-item:last-child {
    border-bottom: none;
  }

  .file-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: .35rem;
  }

  .file-name {
    font-size: 12px;
    color: #ffffffd1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-size {
    font-size: 11px;
    color: #fff6;
    flex-shrink: 0;
  }

  @media (max-width:768px) {
    .file-item {
      flex-wrap: wrap;
      gap: .5rem;
      padding: .5rem;
      border-radius: 6px;
    }
    .file-main {
      flex-basis: 100%;
      order: 2;
    }
    .file-name {
      font-size: .65rem;
    }
  }
</style>
