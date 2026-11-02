<script lang="ts">
  import SmartSearchEpisodeGroup from "./SmartSearchEpisodeGroup.svelte";
  import { seasonPackageReleases, type RawSearchResult, type SourceFilter } from "./resultUtils";

  interface Props {
    season: any;
    existingDownloads?: Set<string>;
    expandedEpisodes?: Record<string, boolean>;
    source?: SourceFilter;
    onToggleEpisode?: (key: string) => void;
    onDownload?: (file: RawSearchResult, event: MouseEvent) => void;
    onDetails?: (file: RawSearchResult) => void;
    onManualAdd?: (season: number, episode: any) => void;
  }

  let {
    season,
    existingDownloads = new Set(),
    expandedEpisodes = {},
    source = "best",
    onToggleEpisode,
    onDownload,
    onDetails,
    onManualAdd,
  }: Props = $props();

  const packages = $derived(seasonPackageReleases(season, source));
  const packsEntry = $derived({
    episode_number: "PACKS",
    name: "Season packs",
    overview: `${packages.length} unique torrents · grouped by infoHash`,
    files: packages,
  });
  const episodes = $derived(
    (season.episodes_grouped ?? []).filter((ep: any) => ep.episode_number !== 0),
  );
</script>

{#if episodes.length > 0 || packages.length > 0}
  <div class="season-section">
    <h3 class="season-title">{season.season === 0 ? "Specials" : `Season ${season.season}`}</h3>
    <div class="episode-list">
      {#if packages.length > 0}
        <SmartSearchEpisodeGroup
          ep={packsEntry}
          season={season.season}
          expanded={expandedEpisodes[`packs-${season.season}`]}
          {source}
          onToggle={() => onToggleEpisode?.(`packs-${season.season}`)}
          {onDownload}
          {onDetails}
        />
      {/if}
      {#each episodes as ep}
        {@const key = `ep-${season.season}-${ep.episode_number}`}
        {@const code = `S${String(season.season).padStart(2, "0")}E${String(ep.episode_number).padStart(2, "0")}`}
        <SmartSearchEpisodeGroup
          {ep}
          season={season.season}
          seasonData={season}
          expanded={expandedEpisodes[key]}
          isDownloaded={existingDownloads.has(code)}
          {source}
          onToggle={() => onToggleEpisode?.(key)}
          {onDownload}
          {onDetails}
          onManualAdd={() => onManualAdd?.(season.season, ep)}
        />
      {/each}
    </div>
  </div>
{/if}

<style>
  .season-section {
    margin-bottom: 2rem;
  }

  .season-title {
    font-size: 1rem;
    font-weight: 800;
    color: #fff;
    margin: 0 0 1rem;
    padding-left: .5rem;
    border-left: 3px solid var(--color-primary);
  }

  .episode-list {
    display: flex;
    flex-direction: column;
    gap: .75rem;
  }
</style>
