<script lang="ts">
  import { Badge, Button } from "@media-set/core-ui";
  import { animeSlideDown } from "$lib/animations";
  import { getPosterUrl } from "$lib/services/tmdb";
  import SmartSearchReleaseRow from "./SmartSearchReleaseRow.svelte";
  import {
    episodeReleaseRow,
    matchesSourceFilter,
    rankReleases,
    seasonPackageReleases,
    type RawSearchResult,
    type SourceFilter,
  } from "./resultUtils";

  interface Props {
    /** One `episodes_grouped` entry, or the synthetic `episode_number: "PACKS"` entry for season packs. */
    ep: any;
    season: number;
    seasonData?: any;
    expanded?: boolean;
    isDownloaded?: boolean;
    source?: SourceFilter;
    onToggle?: () => void;
    onDownload?: (file: RawSearchResult, event: MouseEvent) => void;
    onDetails?: (file: RawSearchResult) => void;
    onManualAdd?: () => void;
  }

  let {
    ep,
    season,
    seasonData,
    expanded = false,
    isDownloaded = false,
    source = "all",
    onToggle,
    onDownload,
    onDetails,
    onManualAdd,
  }: Props = $props();

  // Files inside season packs that cover this episode are offered next to the single-episode releases.
  const packageReleases = $derived(
    seasonData ? seasonPackageReleases(seasonData, source, ep.episode_number) : [],
  );
  const releases = $derived(
    rankReleases([
      ...(ep.files ?? []).filter((file: RawSearchResult) => matchesSourceFilter(file, source)),
      ...packageReleases,
    ]),
  );
  const row = $derived(
    episodeReleaseRow(
      season,
      { ...ep, files: releases },
      source,
      isDownloaded
        ? new Set([`S${String(season).padStart(2, "0")}E${String(ep.episode_number).padStart(2, "0")}`])
        : new Set(),
    ),
  );
  const isPacks = $derived(ep.episode_number === "PACKS");

  function handleKeydown(event: KeyboardEvent) {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    onToggle?.();
  }

  /** Tag a release with the episode it is grabbed for, so the download is attributed to it. */
  function withEpisodeContext(release: RawSearchResult): RawSearchResult {
    const episode = isPacks
      ? release.coveredEpisodes?.length === 1
        ? release.coveredEpisodes[0]
        : undefined
      : ep.episode_number;
    return episode === undefined ? release : { ...release, _episodeContext: { season, episode } };
  }
</script>

<div class="episode-card glass-panel" class:expanded class:downloaded={row.downloaded}>
  <div
    class="episode-trigger"
    role="button"
    tabindex="0"
    aria-expanded={expanded}
    onclick={onToggle}
    onkeydown={handleKeydown}
  >
    <div class="episode-thumb">
      {#if ep.still_path}
        <img src={getPosterUrl(ep.still_path, "w500")} alt="" class="ep-img" />
      {/if}
      <div class="ep-badge-wrapper">
        <Badge text={isPacks ? "PACKS" : row.key} variant="episode" size="xs" noDot />
      </div>
    </div>
    <div class="episode-main">
      <div class="ep-header-row">
        <span class="ep-num">{isPacks ? "PACKS" : row.key}</span>
        <h4>{row.title}</h4>
        {#if row.downloaded}
          <Badge text="DOWNLOADED" variant="downloaded" size="xs" />
        {/if}
        {#if row.missing}
          <Badge text="MISSING" variant="warning" size="xs" />
        {/if}
      </div>
      <div class="ep-meta-row">
        <span>{row.releaseCount} release{row.releaseCount === 1 ? "" : "s"}</span>
        <span>•</span><span>{row.quality}</span>
        <span>•</span><span>{row.health}</span>
        <span>•</span><span>{row.size}</span>
      </div>
      {#if ep.overview}
        <p class="ep-overview">{ep.overview}</p>
      {/if}
    </div>
    <span class="material-icons chevron" aria-hidden="true">{expanded ? "expand_less" : "expand_more"}</span>
  </div>
  {#if expanded}
    <div class="release-list" transition:animeSlideDown>
      {#each releases as release, index}
        <SmartSearchReleaseRow
          file={withEpisodeContext(release)}
          rank={index + 1}
          onDownload={(file, event) => onDownload?.(file, event)}
          {onDetails}
        />
      {/each}
      {#if !isPacks}
        <div class="manual-url-action">
          <Button size="sm" variant="ghost" icon="link" onclick={() => onManualAdd?.()}>Add Fshare URL</Button>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .episode-card {
    background: #ffffff05;
    border: 1px solid rgba(255,255,255,.05);
    border-radius: 12px;
    overflow: hidden;
  }

  .episode-card.downloaded {
    border-color: #10b9814d;
    background: #10b98108;
  }

  .episode-trigger {
    display: flex;
    padding: .75rem;
    align-items: center;
    gap: 1rem;
    cursor: pointer;
  }

  .episode-trigger:focus-visible {
    outline: 2px solid var(--color-primary,#00f3ff);
    outline-offset: -2px;
  }

  .episode-thumb {
    width: 160px;
    height: 90px;
    background: #000;
    border-radius: 8px;
    position: relative;
    overflow: hidden;
    flex-shrink: 0;
  }

  .ep-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .ep-badge-wrapper {
    position: absolute;
    bottom: 4px;
    left: 4px;
    z-index: 2;
  }

  .episode-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: .25rem;
    min-width: 0;
  }

  .ep-header-row,
  .ep-meta-row {
    display: flex;
    align-items: center;
    gap: .5rem;
    flex-wrap: wrap;
  }

  .ep-num {
    color: var(--color-primary);
    font-weight: 800;
    font-size: .78rem;
    font-family: var(--font-mono);
  }

  h4 {
    margin: 0;
    font-size: 1rem;
    font-weight: 700;
    color: #fff;
  }

  .ep-meta-row {
    font-size: .75rem;
    color: #ffffff80;
  }

  .ep-overview {
    margin: .25rem 0 0;
    font-size: .8rem;
    color: #ffffff80;
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .chevron {
    color: #ffffff80;
  }

  .release-list {
    display: flex;
    flex-direction: column;
    gap: var(--cui-space-sm);
    padding: var(--cui-space-sm);
    background: #0000004d;
  }

  .manual-url-action {
    display: flex;
    justify-content: flex-end;
  }

  @media (max-width:768px) {
    .episode-thumb {
      width: 72px;
      height: 48px;
    }
    .ep-overview {
      display: none;
    }
    h4 {
      font-size: .8rem;
    }
    .episode-trigger {
      gap: .75rem;
    }
  }
</style>
