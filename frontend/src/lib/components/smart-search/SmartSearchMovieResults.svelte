<script lang="ts">
  import { Button } from "@media-set/core-ui";
  import SmartSearchSourceTabs from "./SmartSearchSourceTabs.svelte";
  import SmartSearchReleaseRow from "./SmartSearchReleaseRow.svelte";
  import {
    collectResultFiles,
    matchesSourceFilter,
    rankReleases,
    type RawSearchResult,
    type SourceFilter,
  } from "./resultUtils";

  interface Props {
    results: any;
    source?: SourceFilter;
    onDownload?: (file: RawSearchResult, event: MouseEvent) => void;
    onDetails?: (file: RawSearchResult) => void;
    onManualAdd?: () => void;
    onSelectSource?: (source: SourceFilter) => void;
  }

  let { results, source = "all", onDownload, onDetails, onManualAdd, onSelectSource }: Props = $props();

  const ranked = $derived(
    rankReleases(collectResultFiles(results).filter((file) => matchesSourceFilter(file, source))),
  );
</script>

<div class="movie-toolbar">
  <Button size="sm" variant="ghost" icon="link" onclick={() => onManualAdd?.()}>Add Fshare URL</Button>
</div>
<SmartSearchSourceTabs {results} active={source} onSelect={onSelectSource} />
{#if ranked.length === 0}
  <div class="filtered-empty" role="status">
    <span>No {source === "all" ? "" : `${source} `}results in this view.</span>
    {#if source !== "all"}
      <button type="button" onclick={() => onSelectSource?.("all")}>Show all sources</button>
    {/if}
  </div>
{:else}
  <div class="ranked-list" aria-label="Ranked releases">
    {#each ranked as file, index}
      <SmartSearchReleaseRow {file} rank={index + 1} {onDownload} {onDetails} />
    {/each}
  </div>
{/if}

<style>
  .movie-toolbar {
    display: flex;
    justify-content: flex-end;
  }

  .ranked-list {
    display: flex;
    flex-direction: column;
    gap: var(--cui-space-sm);
  }

  .filtered-empty {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: var(--cui-space-sm);
    min-height: 120px;
    color: var(--cui-text-secondary);
    font-size: var(--cui-text-sm);
    text-align: center;
  }

  .filtered-empty button {
    background: transparent;
    border: 0;
    color: var(--cui-primary);
    cursor: pointer;
    text-decoration: underline;
    font: inherit;
  }
</style>
