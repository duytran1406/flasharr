<script lang="ts">
  import SmartSearchTvResults from "./SmartSearchTvResults.svelte";
  import SmartSearchMovieResults from "./SmartSearchMovieResults.svelte";
  import type { RawSearchResult, SourceFilter } from "./resultUtils";

  interface Props {
    results: any;
    existingDownloads?: Set<string>;
    expandedGroups?: Record<string, boolean>;
    expandedEpisodes?: Record<string, boolean>;
    source?: SourceFilter;
    targetSeason?: number | null;
    onSelectSource?: (source: SourceFilter) => void;
    onToggleGroup?: (key: string) => void;
    onToggleEpisode?: (key: string) => void;
    onDownload?: (file: RawSearchResult, event: MouseEvent) => void;
    onDetails?: (file: RawSearchResult) => void;
    onManualAdd?: (season?: number, episode?: any) => void;
  }

  let {
    results,
    existingDownloads,
    expandedEpisodes,
    source,
    targetSeason,
    onSelectSource,
    onToggleEpisode,
    onDownload,
    onDetails,
    onManualAdd,
  }: Props = $props();
</script>

<!-- TV responses carry seasons[].episodes_grouped[]; movie responses carry groups[].files[]. -->
<div class="results-container">
  {#if (results?.seasons?.length ?? 0) > 0}
    <SmartSearchTvResults
      {results}
      {existingDownloads}
      {expandedEpisodes}
      {source}
      {targetSeason}
      {onToggleEpisode}
      {onDownload}
      {onDetails}
      {onManualAdd}
      {onSelectSource}
    />
  {:else if (results?.groups?.length ?? 0) > 0}
    <SmartSearchMovieResults {results} {source} {onDownload} {onDetails} {onManualAdd} {onSelectSource} />
  {/if}
</div>

<style>
  .results-container {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
</style>
