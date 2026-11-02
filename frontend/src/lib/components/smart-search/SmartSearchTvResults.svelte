<script lang="ts">
  import SmartSearchSourceTabs from "./SmartSearchSourceTabs.svelte";
  import SmartSearchSeasonGroup from "./SmartSearchSeasonGroup.svelte";
  import { seasonsHaveReleases, type RawSearchResult, type SourceFilter } from "./resultUtils";

  interface Props {
    results: any;
    existingDownloads?: Set<string>;
    expandedEpisodes?: Record<string, boolean>;
    source?: SourceFilter;
    targetSeason?: number | null;
    onToggleEpisode?: (key: string) => void;
    onDownload?: (file: RawSearchResult, event: MouseEvent) => void;
    onDetails?: (file: RawSearchResult) => void;
    onManualAdd?: (season: number, episode: any) => void;
    onSelectSource?: (source: SourceFilter) => void;
  }

  let {
    results,
    existingDownloads = new Set(),
    expandedEpisodes = {},
    source = "all",
    targetSeason = null,
    onToggleEpisode,
    onDownload,
    onDetails,
    onManualAdd,
    onSelectSource,
  }: Props = $props();

  // Specials are hidden; a targeted search only shows the requested season.
  const seasons = $derived(
    (results?.seasons ?? []).filter(
      (season: any) => season.season !== 0 && (!targetSeason || Number(season.season) === targetSeason),
    ),
  );
  const hasReleases = $derived(seasonsHaveReleases(seasons, source));
</script>

<SmartSearchSourceTabs {results} active={source} onSelect={onSelectSource} />
{#if !hasReleases}
  <div class="filtered-empty" role="status">
    <span>No {source === "all" ? "" : `${source} `}episodes in this view.</span>
    {#if source !== "all"}
      <button type="button" onclick={() => onSelectSource?.("all")}>Show all sources</button>
    {/if}
  </div>
{/if}
{#each seasons as season}
  <SmartSearchSeasonGroup
    {season}
    {existingDownloads}
    {expandedEpisodes}
    {source}
    {onToggleEpisode}
    {onDownload}
    {onDetails}
    {onManualAdd}
  />
{/each}

<style>
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
