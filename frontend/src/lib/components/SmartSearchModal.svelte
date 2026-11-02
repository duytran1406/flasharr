<script lang="ts">
  import { tick } from "svelte";
  import { Modal } from "@media-set/core-ui";
  import { ui } from "$lib/stores/ui.svelte";
  import { smartGrabStore } from "$lib/stores/smartGrab";
  import SmartSearchHeader from "./smart-search/SmartSearchHeader.svelte";
  import SmartSearchResults from "./smart-search/SmartSearchResults.svelte";
  import SmartSearchEmptyState from "./smart-search/SmartSearchEmptyState.svelte";
  import SmartSearchErrorState from "./smart-search/SmartSearchErrorState.svelte";
  import FshareDetailModal from "./fshare/FshareDetailModal.svelte";
  import FshareAddDownloadModal from "./fshare/FshareAddDownloadModal.svelte";
  import TorrentDetailModal from "./torrents/TorrentDetailModal.svelte";
  import TorrentAddModal from "./torrents/TorrentAddModal.svelte";
  import TorboxDetailModal from "./torbox/TorboxDetailModal.svelte";
  import TorboxAddDownloadModal from "./torbox/TorboxAddDownloadModal.svelte";
  import { withEpisodeTarget } from "./fshare/fsharePayload";
  import {
    hasSmartSearchResults,
    resultSource,
    type RawSearchResult,
    type SourceFilter,
  } from "./smart-search/resultUtils";

  type ProviderModal = "fshare" | "torrent" | "torbox";

  let source = $state<SourceFilter>("best");
  let loading = $state(true);
  let results = $state<any>(null);
  let error = $state<string | null>(null);
  /** Episode keys ("S01E02") that already have a Flasharr download. */
  let existingDownloads = $state<Set<string>>(new Set());

  // "Details" modal
  let detailFile = $state<RawSearchResult | null>(null);
  let detailModal = $state<ProviderModal | null>(null);
  // "Get" (queue download) modal
  let downloadFile = $state<RawSearchResult | null>(null);
  let downloadModal = $state<ProviderModal | null>(null);
  let downloadTmdb = $state<any>(null);

  function providerModal(file: RawSearchResult): ProviderModal {
    const kind = resultSource(file);
    return (kind === "debrid" ? "torbox" : kind) as ProviderModal;
  }

  function openDetails(file: RawSearchResult) {
    detailFile = file;
    detailModal = providerModal(file);
  }

  function closeDetails() {
    detailFile = null;
    detailModal = null;
  }

  function openDownload(file: RawSearchResult) {
    downloadFile = file;
    downloadModal = providerModal(file);
    downloadTmdb = file?._episodeContext
      ? withEpisodeTarget(tmdbMetadata() || {}, file._episodeContext)
      : null;
  }

  function closeDownload() {
    downloadFile = null;
    downloadModal = null;
    downloadTmdb = null;
  }

  /** "Add Fshare URL": queue a pasted Fshare link for this title (TV needs a concrete episode). */
  function openManualAdd(season?: number, episode?: any) {
    const tmdb = tmdbMetadata();
    if (!tmdb?.tmdb_id || !tmdb.title) {
      error = "This result has no TMDB metadata for a managed download.";
      return;
    }
    if (tmdb.media_type === "tv" && (!season || !episode?.episode_number)) {
      error = "Select a specific episode before adding a Fshare URL.";
      return;
    }
    downloadTmdb = {
      ...tmdb,
      ...(season ? { season } : {}),
      ...(episode ? { episode: episode.episode_number } : {}),
    };
    downloadFile = { name: episode?.name || tmdb.title };
    downloadModal = "fshare";
  }

  function downloadCategory() {
    return ui.smartSearchData?.type === "tv" ? "tv" : "movies";
  }

  function tmdbMetadata() {
    if (!ui.smartSearchData) return;
    return {
      tmdb_id: parseInt(ui.smartSearchData.tmdbId || "") || undefined,
      tvdb_id: parseInt(ui.smartSearchData.tvdbId || "") || undefined,
      media_type: ui.smartSearchData.type,
      title: ui.smartSearchData.title,
      year: ui.smartSearchData.year?.toString(),
      season: ui.smartSearchData.season,
      episode: ui.smartSearchData.episode,
    };
  }

  async function performSearch() {
    if (!ui.smartSearchData) return;
    loading = true;
    error = null;
    source = "all";
    existingDownloads = new Set();

    try {
      const searchPromise = fetch("/api/search/smart", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          // A TV request without an episode asks for the whole season plan.
          mode: ui.smartSearchData.type === "tv" && !ui.smartSearchData.episode ? "season" : "target",
          kind:
            ui.smartSearchData.type === "movie"
              ? "movie"
              : ui.smartSearchData.episode
                ? "episode"
                : undefined,
          title: ui.smartSearchData.title,
          year: ui.smartSearchData.year,
          type: ui.smartSearchData.type,
          tmdb_id: ui.smartSearchData.tmdbId,
          tvdb_id: ui.smartSearchData.tvdbId,
          season: ui.smartSearchData.season,
          episode: ui.smartSearchData.episode,
        }),
      });

      // Existing Flasharr downloads for this title, to mark episodes as downloaded.
      const tmdbId = ui.smartSearchData.tmdbId;
      const downloadsPromise = tmdbId
        ? fetch(`/api/media/${tmdbId}/downloads`)
            .then((r) => (r.ok ? r.json() : null))
            .catch(() => null)
        : Promise.resolve(null);

      const [response, downloads] = await Promise.all([searchPromise, downloadsPromise]);

      if (response.ok) {
        results = await response.json();
      } else {
        error = `Search failed with status ${response.status}`;
        results = null;
      }

      const downloaded = new Set<string>();
      if (downloads?.episodes) {
        for (const key of Object.keys(downloads.episodes)) downloaded.add(key);
      }
      existingDownloads = downloaded;
    } catch (err: any) {
      error = err.message || "Network error";
      results = null;
    } finally {
      loading = false;
    }
  }

  let expandedGroups = $state<Record<string, boolean>>({});
  function toggleGroup(key: string) {
    expandedGroups[key] = !expandedGroups[key];
  }

  let expandedEpisodes = $state<Record<string, boolean>>({});
  function toggleEpisode(key: string) {
    expandedEpisodes[key] = !expandedEpisodes[key];
  }

  /** Hand the season results over to the Smart Grab planner. */
  async function openSmartGrab() {
    const data = {
      tmdbId: ui.smartSearchData?.tmdbId || "",
      tvdbId: ui.smartSearchData?.tvdbId ?? null,
      type: "tv" as const,
      title: ui.smartSearchData?.title || "",
      year: ui.smartSearchData?.year,
      seasons: results.seasons,
      targetSeason: ui.smartSearchData?.season,
      existingDownloads,
    };
    ui.closeSmartSearch();
    await tick();
    smartGrabStore.open(data);
  }

  $effect(() => {
    if (ui.smartSearchModalOpen) {
      source = "all";
      performSearch();
    } else {
      closeDetails();
      closeDownload();
    }
  });
</script>

<Modal
  open={ui.smartSearchModalOpen}
  onClose={() => ui.closeSmartSearch()}
  maxWidth="900px"
  accent="var(--color-primary, #00f3ff)"
  ariaLabel="Smart Search"
>
  {#snippet header()}
    <SmartSearchHeader
      {loading}
      title={ui.smartSearchData?.title || "Results"}
      year={ui.smartSearchData?.year}
      canSmartGrab={((results?.seasons?.length ?? 0) > 0 || !!results?.seasonPlan) && !loading}
      onClose={() => ui.closeSmartSearch()}
      onSmartGrab={openSmartGrab}
    />
  {/snippet}

  {#if loading}
    <div class="loading-state" role="status" aria-live="polite">
      <div class="loading-spinner"></div>
      <p>Scanning indexes for high-quality releases...</p>
    </div>
  {:else if error}
    <SmartSearchErrorState {error} onRetry={performSearch} />
  {:else if results && hasSmartSearchResults(results)}
    <SmartSearchResults
      {results}
      {existingDownloads}
      {expandedGroups}
      {expandedEpisodes}
      {source}
      targetSeason={ui.smartSearchData?.season}
      onSelectSource={(next) => (source = next)}
      onToggleGroup={toggleGroup}
      onToggleEpisode={toggleEpisode}
      onDownload={(file, _event) => openDownload(file)}
      onDetails={openDetails}
      onManualAdd={openManualAdd}
    />
  {:else}
    <SmartSearchEmptyState {results} />
  {/if}
</Modal>

<FshareDetailModal file={detailFile} open={detailModal === "fshare"} onClose={closeDetails} />
<TorrentDetailModal file={detailFile} open={detailModal === "torrent"} onClose={closeDetails} />
<TorboxDetailModal file={detailFile} open={detailModal === "torbox"} onClose={closeDetails} />

<FshareAddDownloadModal
  file={downloadFile}
  open={downloadModal === "fshare"}
  onClose={closeDownload}
  tmdb={downloadTmdb || tmdbMetadata()}
  category={downloadCategory()}
  manual={downloadTmdb !== null}
/>
<TorrentAddModal
  file={downloadFile}
  open={downloadModal === "torrent"}
  onClose={closeDownload}
  tmdb={downloadTmdb || tmdbMetadata()}
  category={downloadCategory()}
/>
<TorboxAddDownloadModal
  file={downloadFile}
  open={downloadModal === "torbox"}
  onClose={closeDownload}
  tmdb={downloadTmdb || tmdbMetadata()}
  category={downloadCategory()}
/>

<style>
  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 300px;
    gap: 1rem;
    color: #ffffff80;
  }

  .loading-spinner {
    width: 32px;
    height: 32px;
    border: 3px solid rgba(0,243,255,.1);
    border-top-color: var(--color-primary);
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion:reduce) {
    .loading-spinner {
      animation: none;
      border-top-color: #00f3ff66;
    }
  }
</style>
