<script lang="ts">
  import { tick } from "svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { smartGrabStore } from "$lib/stores/smartGrab";
  import { toasts } from "$lib/stores/toasts";
  import { onMount } from "svelte";
  import { animeFade, animeSlideDown } from "$lib/animations";
  import { getPosterUrl } from "$lib/services/tmdb";
  import Badge from "$lib/components/ui/Badge.svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import Button from "$lib/components/ui/Button.svelte";

  let loading = $state(true);
  let results = $state<any>(null);
  let error = $state<string | null>(null);
  let existingDownloads = $state<Set<string>>(new Set());

  async function performSearch() {
    if (!ui.smartSearchData) return;
    loading = true;
    error = null;
    existingDownloads = new Set();

    try {
      // Fetch search results, Flasharr downloads, and Sonarr/Radarr library in parallel
      const searchPromise = fetch("/api/search/smart", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          title: ui.smartSearchData.title,
          year: ui.smartSearchData.year,
          type: ui.smartSearchData.type,
          tmdb_id: ui.smartSearchData.tmdbId,
          season: ui.smartSearchData.season,
          episode: ui.smartSearchData.episode,
        }),
      });

      // Fetch existing downloads from Flasharr DB (if we have a TMDB ID)
      const tmdbId = ui.smartSearchData.tmdbId;
      const downloadsPromise = tmdbId
        ? fetch(`/api/media/${tmdbId}/downloads`)
            .then((r) => (r.ok ? r.json() : null))
            .catch(() => null)
        : Promise.resolve(null);

      // Fetch Sonarr/Radarr library status to check which episodes have files on disk
      const mediaType = ui.smartSearchData.type;
      const arrPromise = tmdbId
        ? fetchArrLibraryStatus(tmdbId, mediaType)
        : Promise.resolve(new Set<string>());

      const [resp, dlData, arrExisting] = await Promise.all([
        searchPromise,
        downloadsPromise,
        arrPromise,
      ]);

      if (resp.ok) {
        results = await resp.json();
      } else {
        error = `Search failed with status ${resp.status}`;
      }

      // Build set of existing episode keys from both sources
      const downloaded = new Set<string>();

      // Source 1: Flasharr download records
      if (dlData?.episodes) {
        for (const key of Object.keys(dlData.episodes)) {
          downloaded.add(key);
        }
      }

      // Source 2: Sonarr/Radarr library (episodes with files on disk)
      for (const key of arrExisting) {
        downloaded.add(key);
      }

      existingDownloads = downloaded;
      if (downloaded.size > 0) {
      }
    } catch (e: any) {
      error = e.message || "Network error";
    } finally {
      loading = false;
    }
  }

  /** Fetch episode file status from Sonarr/Radarr by TMDB ID */
  async function fetchArrLibraryStatus(
    tmdbId: string,
    mediaType: string,
  ): Promise<Set<string>> {
    const existing = new Set<string>();
    try {
      if (mediaType === "tv") {
        // Fetch all series from Sonarr and find the one matching this TMDB ID
        const seriesResp = await fetch("/api/arr/series");
        if (!seriesResp.ok) return existing;
        const allSeries: any[] = await seriesResp.json();
        const match = allSeries.find(
          (s) => s.tmdbId === Number(tmdbId) || s.tvdbId === Number(tmdbId),
        );
        if (!match) return existing;

        // Fetch episodes for this series
        const epsResp = await fetch(`/api/arr/episodes?series_id=${match.id}`);
        if (!epsResp.ok) return existing;
        const episodes: any[] = await epsResp.json();

        for (const ep of episodes) {
          if (ep.hasFile) {
            const key = `S${String(ep.seasonNumber).padStart(2, "0")}E${String(ep.episodeNumber).padStart(2, "0")}`;
            existing.add(key);
          }
        }
      } else if (mediaType === "movie") {
        // For movies, check if Radarr has the file
        const moviesResp = await fetch("/api/arr/movies");
        if (!moviesResp.ok) return existing;
        const allMovies: any[] = await moviesResp.json();
        const match = allMovies.find((m) => m.tmdbId === Number(tmdbId));
        if (match?.hasFile) {
          existing.add("MOVIE");
        }
      }
    } catch (e) {
      console.warn("[Smart Search] Failed to fetch arr library status:", e);
    }
    return existing;
  }

  async function downloadItem(
    url: string,
    filename: string,
    event: MouseEvent,
  ) {
    const btn = event.currentTarget as HTMLButtonElement;
    const originalContent = btn.innerHTML;

    try {
      btn.innerHTML =
        '<span class="material-icons rotating">hourglass_empty</span>';
      btn.disabled = true;

      const tmdbMetadata = ui.smartSearchData
        ? {
            tmdb_id: parseInt(ui.smartSearchData.tmdbId) || undefined,
            media_type: ui.smartSearchData.type,
            title: ui.smartSearchData.title,
            year: ui.smartSearchData.year?.toString(),
            season: ui.smartSearchData.season,
            episode: ui.smartSearchData.episode,
          }
        : undefined;

      const resp = await fetch("/api/downloads", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          url,
          filename,
          category: ui.smartSearchData?.type || "other",
          tmdb: tmdbMetadata,
        }),
      });

      if (resp.ok) {
        btn.innerHTML = '<span class="material-icons">check</span>';
        btn.classList.add("success-btn");
        toasts.success("Download added to queue");
      } else {
        btn.innerHTML = '<span class="material-icons">error</span>';
        btn.classList.add("error-btn");
        toasts.error("Failed to add download");
        setTimeout(() => {
          btn.innerHTML = originalContent;
          btn.disabled = false;
          btn.classList.remove("error-btn");
        }, 3000);
      }
    } catch (e) {
      btn.innerHTML = '<span class="material-icons">error</span>';
      btn.classList.add("error-btn");
      toasts.error("Network error");
    }
  }

  $effect(() => {
    if (ui.smartSearchModalOpen) {
      performSearch();
    }
  });

  function formatSize(bytes: number) {
    if (!bytes) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  function getScoreColor(score: number) {
    if (score >= 70) return "#10b981";
    if (score >= 50) return "#f59e0b";
    return "#ef4444";
  }

  let expandedGroups = $state<Record<string, boolean>>({});
  function toggleGroup(qName: string) {
    expandedGroups[qName] = !expandedGroups[qName];
  }

  let expandedEpisodes = $state<Record<string, boolean>>({});
  function toggleEpisode(epId: string) {
    expandedEpisodes[epId] = !expandedEpisodes[epId];
  }

  let isGrabbing = $state(false);

  // Helper: Extract filename pattern by removing episode numbers
  function extractFilenamePattern(filename: string): string {
    // Remove common episode patterns: E01, E1, .01., x01, _01_, EP01, Episode01, etc.
    let pattern = filename
      // Remove file extension first
      .replace(/\.(mkv|mp4|avi|mov|wmv|flv|webm)$/i, "")
      // Replace episode patterns with placeholder
      .replace(/\.E(\d{1,3})\./gi, ".EPNUM.")
      .replace(/\.EP(\d{1,3})\./gi, ".EPNUM.")
      .replace(/[._\-\s]E(\d{1,3})([._\-\s]|$)/gi, ".EPNUM.")
      .replace(/[._\-\s]EP(\d{1,3})([._\-\s]|$)/gi, ".EPNUM.")
      .replace(/[._\-\s]Episode[._\-\s]?(\d{1,3})/gi, ".EPNUM.")
      .replace(/[._\-\s]Tap[._\-\s]?(\d{1,3})/gi, ".EPNUM.") // Vietnamese "Tập"
      .replace(/S(\d{1,2})E(\d{1,3})/gi, "S$1EPNUM")
      .replace(/(\d{1,2})x(\d{1,3})/gi, "$1xEPNUM")
      // Handle standalone episode numbers at boundaries
      .replace(/[._\-\s](\d{1,2})[._\-\s]/g, ".EPNUM.")
      // Normalize separators
      .replace(/[._\-\s]+/g, ".")
      .toLowerCase();

    return pattern;
  }

  // Helper: Check if a filename matches the reference pattern
  function matchesPattern(filename: string, referencePattern: string): boolean {
    const pattern = extractFilenamePattern(filename);
    // Simple match: patterns should be identical after normalization
    return pattern === referencePattern;
  }

  // NEW: Analyze all patterns and calculate coverage for each
  function analyzePatternCoverage(seasons: any[]): Map<
    string,
    {
      coverage: number;
      totalEpisodes: number;
      matchedEpisodes: number;
      avgScore: number;
      sampleFile: any;
    }
  > {
    const patternStats = new Map<
      string,
      {
        episodesWithMatch: Set<string>;
        totalScore: number;
        fileCount: number;
        sampleFile: any;
      }
    >();

    let totalEpisodes = 0;

    // First pass: collect all patterns and their episode coverage
    for (const season of seasons) {
      if (season.season === 0) continue; // Skip specials

      for (const ep of season.episodes_grouped) {
        if (!ep.files || ep.files.length === 0) continue;
        totalEpisodes++;

        const episodeKey = `S${season.season}E${ep.episode_number}`;

        for (const file of ep.files) {
          const pattern = extractFilenamePattern(file.name);

          if (!patternStats.has(pattern)) {
            patternStats.set(pattern, {
              episodesWithMatch: new Set(),
              totalScore: 0,
              fileCount: 0,
              sampleFile: file,
            });
          }

          const stats = patternStats.get(pattern)!;
          stats.episodesWithMatch.add(episodeKey);
          stats.totalScore += file.score || 0;
          stats.fileCount++;
        }
      }
    }

    // Calculate coverage percentages
    const coverageMap = new Map<
      string,
      {
        coverage: number;
        totalEpisodes: number;
        matchedEpisodes: number;
        avgScore: number;
        sampleFile: any;
      }
    >();

    for (const [pattern, stats] of patternStats) {
      const matchedCount = stats.episodesWithMatch.size;
      const coverage =
        totalEpisodes > 0 ? (matchedCount / totalEpisodes) * 100 : 0;
      const avgScore =
        stats.fileCount > 0 ? stats.totalScore / stats.fileCount : 0;

      coverageMap.set(pattern, {
        coverage,
        totalEpisodes,
        matchedEpisodes: matchedCount,
        avgScore,
        sampleFile: stats.sampleFile,
      });
    }

    return coverageMap;
  }

  // NEW: Find the best pattern based on coverage (prioritize coverage, then score)
  function findBestPattern(
    coverageMap: Map<string, any>,
  ): { pattern: string; stats: any } | null {
    let bestPattern: string | null = null;
    let bestStats: any = null;

    for (const [pattern, stats] of coverageMap) {
      if (!bestStats) {
        bestPattern = pattern;
        bestStats = stats;
        continue;
      }

      // Prioritize by coverage first, then by average score if coverage is similar (within 5%)
      const coverageDiff = stats.coverage - bestStats.coverage;

      if (coverageDiff > 5) {
        // Significantly better coverage - choose this pattern
        bestPattern = pattern;
        bestStats = stats;
      } else if (coverageDiff > -5 && stats.avgScore > bestStats.avgScore) {
        // Similar coverage but better score - choose this pattern
        bestPattern = pattern;
        bestStats = stats;
      }
    }

    return bestPattern ? { pattern: bestPattern, stats: bestStats } : null;
  }

  async function smartGrab() {
    if (!results || !results.seasons) return;
    isGrabbing = true;

    // Step 1: Analyze ALL patterns and their coverage across the entire season
    const coverageMap = analyzePatternCoverage(results.seasons);

    if (coverageMap.size === 0) {
      toasts.error("No suitable files found for Smart Grab");
      isGrabbing = false;
      return;
    }

    // Log all patterns for debugging (top 5 by coverage)
    const sortedPatterns = [...coverageMap.entries()].sort(
      (a, b) => b[1].coverage - a[1].coverage,
    );

    for (const [pattern, stats] of sortedPatterns.slice(0, 5)) {
    }

    // Step 2: Find the best pattern based on coverage
    const best = findBestPattern(coverageMap);

    if (!best) {
      toasts.error("No suitable pattern found for Smart Grab");
      isGrabbing = false;
      return;
    }

    const selectedPattern = best.pattern;

    // Step 3: Collect files for each episode using the best pattern
    const toDownload: any[] = [];
    let patternMatches = 0;
    let fallbacks = 0;
    let skippedExisting = 0;

    results.seasons.forEach((season: any) => {
      if (season.season === 0) return; // Skip specials/trash

      season.episodes_grouped.forEach((ep: any) => {
        if (!ep.files || ep.files.length === 0) return;

        // Skip episodes that are already downloaded
        const epKey = `S${String(season.season).padStart(2, "0")}E${String(ep.episode_number).padStart(2, "0")}`;
        if (existingDownloads.has(epKey)) {
          skippedExisting++;
          return;
        }

        // Try to find a file matching the selected pattern
        const patternMatch = ep.files.find((file: any) =>
          matchesPattern(file.name, selectedPattern),
        );

        let selectedFile;
        if (patternMatch) {
          selectedFile = patternMatch;
          patternMatches++;
        } else {
          // Fallback: take the highest-scored file
          selectedFile = ep.files[0];
          fallbacks++;
        }

        toDownload.push({
          url: selectedFile.url,
          filename: selectedFile.name,
          epNum: ep.episode_number,
          seasonNum: season.season,
          matched: !!patternMatch,
        });
      });
    });

    if (toDownload.length === 0) {
      toasts.error("No suitable files found for Smart Grab");
      isGrabbing = false;
      return;
    }

    // Log the final selection table
    console.table(
      toDownload.map((d) => ({
        Episode: `S${d.seasonNum}E${d.epNum}`,
        Filename:
          d.filename.substring(0, 60) + (d.filename.length > 60 ? "..." : ""),
        Status: d.matched ? "✓ MATCHED" : "⚠ FALLBACK",
      })),
    );

    try {
      const coveragePercent = best.stats.coverage.toFixed(0);
      const consistencyMsg =
        fallbacks === 0
          ? `✨ ${coveragePercent}% coverage - perfect match!`
          : `${patternMatches} matched, ${fallbacks} fallbacks`;

      // Calculate total and aired episode counts from backend response
      const totalEpisodes = results.seasons
        .filter((s: any) => s.season !== 0)
        .reduce((sum: number, s: any) => sum + (s.episode_count || 0), 0);
      const airedEpisodes = results.seasons
        .filter((s: any) => s.season !== 0)
        .reduce(
          (sum: number, s: any) =>
            sum + (s.aired_episode_count || s.episodes_grouped?.length || 0),
          0,
        );

      const skippedMsg =
        skippedExisting > 0 ? `, ${skippedExisting} already downloaded` : "";
      const episodeLabel =
        totalEpisodes > airedEpisodes
          ? `${toDownload.length}/${totalEpisodes} (${airedEpisodes} released${skippedMsg})`
          : `${toDownload.length} episodes${skippedMsg}`;

      toasts.info(`Smart Grab: ${episodeLabel} (${consistencyMsg})`);

      // Batch processing for smoother UI feedback
      const batchSize = 3;
      for (let i = 0; i < toDownload.length; i += batchSize) {
        const batch = toDownload.slice(i, i + batchSize);
        await Promise.all(
          batch.map(async (item) => {
            // Reuse TMDB metadata from the store — fetched once when the modal opened.
            // Guard all fields: undefined is dropped by JSON.stringify → backend 422.
            const rawId2 = parseInt(ui.smartSearchData?.tmdbId || "");
            const rawYear2 = ui.smartSearchData?.year ?? null;
            const tmdbMetadata = {
              tmdb_id: Number.isFinite(rawId2) ? rawId2 : null,
              media_type: "tv",
              title: ui.smartSearchData?.title ?? null,
              year:
                rawYear2 != null
                  ? typeof rawYear2 === "string"
                    ? parseInt(rawYear2)
                    : rawYear2
                  : null,
              season: item.seasonNum,
              episode: item.epNum,
            };

            return fetch("/api/downloads", {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({
                url: item.url,
                filename: item.filename,
                category: "tv",
                tmdb: tmdbMetadata,
              }),
            });
          }),
        );
      }

      const successMsg =
        fallbacks === 0
          ? `Smart Grab complete! ${toDownload.length} files from same source (${coveragePercent}% coverage).`
          : `Smart Grab complete! ${toDownload.length} files (${patternMatches} matched, ${fallbacks} best-available).`;

      toasts.success(successMsg);
    } catch (e) {
      toasts.error("Smart Grab encountered an error during batch processing");
    } finally {
      isGrabbing = false;
    }
  }

  function getScoreGradient(score: number) {
    if (score >= 80) return "linear-gradient(90deg, #8b5cf6, #06b6d4)";
    if (score >= 60) return "linear-gradient(90deg, #10b981, #3b82f6)";
    if (score >= 40) return "linear-gradient(90deg, #f59e0b, #f97316)";
    return "linear-gradient(90deg, #ef4444, #991b1b)";
  }

  function getScoreClass(score: number) {
    if (score >= 80) return "score-elite";
    if (score >= 60) return "score-optimal";
    if (score >= 40) return "score-suboptimal";
    return "score-trash";
  }
</script>

<Modal
  open={ui.smartSearchModalOpen}
  onClose={() => ui.closeSmartSearch()}
  maxWidth="900px"
  accent="var(--color-primary, #00f3ff)"
  ariaLabel="Smart Search"
>
  {#snippet header()}
    <div class="header-main">
      <Badge text="SMART SEARCH" variant="smart" size="sm" />
      <h2>
        {loading
          ? "Scanning Fshare..."
          : ui.smartSearchData?.title || "Results"}
        {#if ui.smartSearchData?.year && !loading}
          <span class="year">({ui.smartSearchData.year})</span>
        {/if}
      </h2>
    </div>
    <div class="header-actions">
      {#if results && results.seasons && !loading && ui.smartSearchData?.type === "tv"}
        <span transition:animeFade style="display:contents">
          <Button
            icon="auto_awesome"
            size="md"
            onclick={async () => {
              const grabData = {
                tmdbId: ui.smartSearchData?.tmdbId || "",
                type: "tv" as const,
                title: ui.smartSearchData?.title || "",
                year: ui.smartSearchData?.year,
                seasons: results.seasons,
                existingDownloads: existingDownloads,
              };
              ui.closeSmartSearch();
              await tick();
              smartGrabStore.open(grabData);
            }}>Smart Grab</Button
          >
        </span>
      {/if}
      <button class="close-btn" onclick={() => ui.closeSmartSearch()}>
        <span class="material-icons">close</span>
      </button>
    </div>
  {/snippet}

  {#snippet children()}
    {#if loading}
      <div class="loading-state">
        <div class="loading-spinner"></div>
        <p>Scanning indexes for high-quality releases...</p>
      </div>
    {:else if error}
      <div class="error-state">
        <span class="material-icons">error_outline</span>
        <p>{error}</p>
      </div>
    {:else if results && (results.total_found > 0 || (results.folder_matches && results.folder_matches.length > 0))}
      <div class="results-container">
        {#if results.folder_matches && results.folder_matches.length > 0}
          <!-- Folder Cache Matches -->
          <div class="folder-section">
            <div class="folder-section-header">
              <span class="material-icons">folder_special</span>
              <span class="folder-section-title">Folder Sources</span>
              <Badge
                text="{results.folder_matches.length} found"
                variant="count"
                size="xs"
                noDot
              />
            </div>
            <div class="folder-list">
              {#each results.folder_matches as fm}
                <a
                  href={fm.fshare_url}
                  target="_blank"
                  rel="noopener noreferrer"
                  class="folder-card glass-panel"
                  class:is-dir={fm.is_directory}
                >
                  <div class="folder-icon">
                    <span class="material-icons"
                      >{fm.is_directory ? "folder" : "insert_drive_file"}</span
                    >
                  </div>
                  <div class="folder-info">
                    <div class="folder-name" title={fm.name}>{fm.name}</div>
                    <div class="folder-meta">
                      {#if fm.quality && fm.quality !== "Unknown"}
                        <Badge
                          text={fm.quality}
                          variant="quality"
                          size="xs"
                          noDot
                        />
                      {/if}
                      {#if fm.year}
                        <span class="folder-year">{fm.year}</span>
                      {/if}
                      {#if fm.size > 0}
                        <span class="folder-size">{formatSize(fm.size)}</span>
                      {/if}
                    </div>
                  </div>
                  <span class="material-icons folder-open-icon"
                    >open_in_new</span
                  >
                </a>
              {/each}
            </div>
          </div>
        {/if}
        {#if results.groups}
          <!-- Movie Layout -->
          {#each results.groups as group}
            <div
              class="quality-card glass-panel"
              class:expanded={expandedGroups[group.quality]}
            >
              <div
                class="card-trigger"
                onclick={() => toggleGroup(group.quality)}
                onkeydown={(e) =>
                  e.key === "Enter" && toggleGroup(group.quality)}
                role="button"
                tabindex="0"
              >
                <div class="trigger-left">
                  <span class="material-icons">layers</span>
                  <span class="quality-label">{group.quality}</span>
                </div>
                <div class="trigger-right">
                  <div class="score-track large">
                    <div
                      class="score-bar {getScoreClass(group.score)}"
                      style="width: {group.score}%; background: {getScoreGradient(
                        group.score,
                      )}"
                    ></div>
                    <span class="score-text"
                      >Score {group.score.toFixed(1)}</span
                    >
                  </div>
                  <Badge
                    text="{group.count} files"
                    variant="count"
                    size="xs"
                    noDot
                  />
                  <span class="material-icons chevron"
                    >{expandedGroups[group.quality]
                      ? "expand_less"
                      : "expand_more"}</span
                  >
                </div>
              </div>

              {#if expandedGroups[group.quality]}
                <div class="file-list" transition:animeSlideDown>
                  {#each group.files as file}
                    <div class="file-row">
                      <div class="file-info">
                        <div class="file-name" title={file.name}>
                          {file.name}
                        </div>
                        <div class="file-meta">
                          <span class="size">{formatSize(file.size)}</span>
                          <span class="divider">•</span>
                          <div class="badges">
                            {#if file.vietdub || file.name
                                .toLowerCase()
                                .includes("vie")}
                              <Badge text="VIE" variant="language" size="xs" />
                            {/if}
                            {#if file.vietsub}
                              <Badge text="SUB" variant="success" size="xs" />
                            {/if}
                            {#if file.hdr || file.name
                                .toLowerCase()
                                .includes("hdr")}
                              <Badge text="HDR" variant="hdr" size="xs" noDot />
                            {/if}
                            {#if file.dolby_vision || file.name
                                .toLowerCase()
                                .includes("dv")}
                              <Badge text="DV" variant="dv" size="xs" noDot />
                            {/if}
                          </div>
                        </div>
                      </div>
                      <Button
                        size="sm"
                        icon="download"
                        onclick={(e) => downloadItem(file.url, file.name, e)}
                        >Get</Button
                      >
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          {/each}
        {:else if results.seasons}
          <!-- TV Series Layout -->
          {#each results.seasons.filter((s: any) => s.season !== 0) as season}
            <div class="season-section">
              <h3 class="season-title">
                {season.season === 0 ? "Specials" : `Season ${season.season}`}
              </h3>
              <div class="episode-list">
                {#each season.episodes_grouped.filter((e: any) => e.episode_number !== 0) as ep}
                  {@const epId = `ep-${season.season}-${ep.episode_number}`}
                  {@const epKey = `S${String(season.season).padStart(2, "0")}E${String(ep.episode_number).padStart(2, "0")}`}
                  {@const isDownloaded = existingDownloads.has(epKey)}
                  {@const bestScore =
                    ep.files && ep.files.length > 0
                      ? ep.files[0].score || 85
                      : 0}
                  <div
                    class="episode-card glass-panel"
                    class:expanded={expandedEpisodes[epId]}
                    class:downloaded={isDownloaded}
                  >
                    <div
                      class="episode-trigger"
                      onclick={() => toggleEpisode(epId)}
                      onkeydown={(e) =>
                        e.key === "Enter" && toggleEpisode(epId)}
                      role="button"
                      tabindex="0"
                    >
                      <div class="episode-thumb">
                        {#if ep.still_path}
                          <img
                            src={getPosterUrl(ep.still_path, "w500")}
                            alt=""
                            class="ep-img"
                          />
                        {:else}
                          <div class="thumb-placeholder">
                            <span class="material-icons">movie</span>
                          </div>
                        {/if}
                        <div class="ep-badge">E{ep.episode_number}</div>
                        {#if isDownloaded}
                          <div class="downloaded-overlay">
                            <span class="material-icons">check_circle</span>
                          </div>
                        {/if}
                      </div>
                      <div class="episode-main">
                        <div class="ep-header-row">
                          <span class="ep-num">E{ep.episode_number}</span>
                          <h4>{ep.name || `Episode ${ep.episode_number}`}</h4>
                          {#if isDownloaded}
                            <Badge
                              text="DOWNLOADED"
                              variant="downloaded"
                              size="xs"
                            />
                          {/if}
                        </div>
                        <div class="ep-meta-row">
                          {#if ep.air_date}
                            <span class="ep-date"
                              >{new Date(ep.air_date).getFullYear()}</span
                            >
                            <span class="dot">•</span>
                          {/if}
                          <span class="ep-files"
                            >{ep.files.length} files available</span
                          >
                        </div>
                        {#if ep.overview}
                          <p class="ep-overview">{ep.overview}</p>
                        {/if}
                      </div>
                      <div class="episode-right">
                        <span class="material-icons chevron"
                          >{expandedEpisodes[epId]
                            ? "expand_less"
                            : "expand_more"}</span
                        >
                      </div>
                    </div>

                    {#if expandedEpisodes[epId]}
                      <div class="file-list nested" transition:animeSlideDown>
                        {#each ep.files as file, i}
                          {@const displayScore = file.score || 85 - i * 5}
                          <div class="tv-file-row">
                            <div class="tv-file-content">
                              <div class="tv-file-name" title={file.name}>
                                {file.name}
                              </div>
                              <div class="tv-file-meta">
                                <span class="tv-file-size">
                                  <span class="material-icons">storage</span>
                                  {formatSize(file.size)}
                                </span>
                                <div class="tv-badges">
                                  {#if file.vietdub || file.name
                                      .toLowerCase()
                                      .includes("vie")}
                                    <Badge
                                      text="VIE"
                                      variant="language"
                                      size="xs"
                                    />
                                  {/if}
                                  {#if file.vietsub}
                                    <Badge
                                      text="SUB"
                                      variant="success"
                                      size="xs"
                                    />
                                  {/if}
                                  {#if file.hdr || file.name
                                      .toLowerCase()
                                      .includes("hdr")}
                                    <Badge
                                      text="HDR"
                                      variant="hdr"
                                      size="xs"
                                      noDot
                                    />
                                  {/if}
                                  {#if file.dolby_vision || file.name
                                      .toLowerCase()
                                      .includes("dv")}
                                    <Badge
                                      text="DV"
                                      variant="dv"
                                      size="xs"
                                      noDot
                                    />
                                  {/if}
                                  {#if file.quality}
                                    <Badge
                                      text={file.quality}
                                      variant="quality"
                                      size="xs"
                                    />
                                  {/if}
                                </div>
                                <div class="tv-score-container">
                                  <div class="tv-score-track">
                                    <div
                                      class="tv-score-bar {getScoreClass(
                                        displayScore,
                                      )}"
                                      style="width: {displayScore}%; background: {getScoreGradient(
                                        displayScore,
                                      )}"
                                    ></div>
                                  </div>
                                  <span class="tv-score-value"
                                    >{displayScore.toFixed(1)}</span
                                  >
                                </div>
                              </div>
                            </div>
                            <Button
                              size="sm"
                              icon="download"
                              onclick={(e) =>
                                downloadItem(file.url, file.name, e)}
                              >Get</Button
                            >
                          </div>
                        {/each}
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            </div>
          {/each}
        {/if}
      </div>
    {:else}
      <div class="empty-state">
        <span class="material-icons">search_off</span>
        <p>No high-quality matches found on Fshare for this title.</p>
      </div>
    {/if}
  {/snippet}
</Modal>

<style>
  .header-main {
    flex: 1;
  }

  .header-main h2 {
    margin: 0.5rem 0 0;
    font-size: 1.25rem;
    font-weight: 800;
    color: #fff;
  }

  .year {
    color: rgba(255, 255, 255, 0.4);
    font-weight: 400;
    margin-left: 0.5rem;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .smart-grab-btn {
    position: relative;
    background: linear-gradient(
      135deg,
      rgba(0, 243, 255, 0.15) 0%,
      rgba(6, 182, 212, 0.1) 100%
    );
    border: 1px solid rgba(0, 243, 255, 0.4);
    color: var(--color-primary);
    padding: 0.6rem 1.5rem;
    border-radius: 0;
    font-size: 0.72rem;
    font-weight: 900;
    display: flex;
    align-items: center;
    gap: 0.6rem;
    cursor: pointer;
    transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
    font-family: var(--font-mono);
    text-transform: uppercase;
    letter-spacing: 0.1em;
    overflow: hidden;
    clip-path: polygon(
      12px 0%,
      100% 0%,
      100% calc(100% - 12px),
      calc(100% - 12px) 100%,
      0% 100%,
      0% 12px
    );
  }

  .smart-grab-btn .btn-glow {
    position: absolute;
    inset: -2px;
    background: linear-gradient(
      135deg,
      var(--color-primary) 0%,
      #06b6d4 50%,
      #8b5cf6 100%
    );
    opacity: 0;
    filter: blur(8px);
    transition: opacity 0.4s ease;
    z-index: 0;
  }

  .smart-grab-btn:hover:not(:disabled) .btn-glow {
    opacity: 0.6;
    animation: glow-pulse 1.5s infinite ease-in-out;
  }

  @keyframes glow-pulse {
    0%,
    100% {
      opacity: 0.4;
      transform: scale(1);
    }
    50% {
      opacity: 0.7;
      transform: scale(1.02);
    }
  }

  .smart-grab-btn .btn-shine {
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(
      90deg,
      transparent 0%,
      rgba(255, 255, 255, 0.15) 50%,
      transparent 100%
    );
    transition: left 0.6s ease;
    z-index: 1;
  }

  .smart-grab-btn:hover:not(:disabled) .btn-shine {
    left: 100%;
  }

  .smart-grab-btn .btn-content {
    position: relative;
    z-index: 2;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .smart-grab-btn:hover:not(:disabled) {
    background: linear-gradient(
      135deg,
      rgba(0, 243, 255, 0.25) 0%,
      rgba(6, 182, 212, 0.2) 100%
    );
    border-color: var(--color-primary);
    color: #fff;
    transform: translateY(-2px);
    box-shadow:
      0 0 30px rgba(0, 243, 255, 0.4),
      0 0 60px rgba(0, 243, 255, 0.2),
      inset 0 0 20px rgba(0, 243, 255, 0.1);
    text-shadow: 0 0 10px rgba(0, 243, 255, 0.8);
  }

  .smart-grab-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    filter: grayscale(0.3);
  }

  .smart-grab-btn .material-icons {
    font-size: 1.1rem;
    transition: transform 0.3s ease;
  }

  .smart-grab-btn:hover:not(:disabled) .material-icons:not(.rotating) {
    animation: star-twinkle 0.8s ease infinite;
  }

  @keyframes star-twinkle {
    0%,
    100% {
      transform: scale(1) rotate(0deg);
    }
    25% {
      transform: scale(1.1) rotate(-5deg);
    }
    75% {
      transform: scale(1.1) rotate(5deg);
    }
  }

  .smart-grab-btn .btn-label {
    font-weight: 900;
  }

  .close-btn {
    background: rgba(255, 255, 255, 0.05);
    border: none;
    color: rgba(255, 255, 255, 0.4);
    width: 32px;
    height: 32px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.2s;
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }

  .modal-body {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem 2rem;
  }

  .custom-scrollbar::-webkit-scrollbar {
    width: 5px;
  }
  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 10px;
  }

  .loading-state,
  .error-state,
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 300px;
    gap: 1rem;
    color: rgba(255, 255, 255, 0.5);
  }

  .loading-spinner {
    width: 32px;
    height: 32px;
    border: 3px solid rgba(0, 243, 255, 0.1);
    border-top-color: var(--color-primary);
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Results List */
  .results-container {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  /* Folder Sources Section */
  .folder-section {
    margin-bottom: 0.5rem;
  }
  .folder-section-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0 0.25rem;
    margin-bottom: 0.75rem;
    color: rgba(255, 255, 255, 0.7);
    font-size: 0.8rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .folder-section-header .material-icons {
    font-size: 1.1rem;
    color: #f59e0b;
  }
  .folder-section-title {
    flex: 1;
  }
  .folder-list {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .folder-card {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 10px;
    text-decoration: none;
    color: inherit;
    transition: all 0.2s;
    cursor: pointer;
  }
  .folder-card:hover {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(245, 158, 11, 0.3);
    transform: translateX(2px);
  }
  .folder-card.is-dir {
    border-left: 3px solid rgba(245, 158, 11, 0.4);
  }
  .folder-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border-radius: 8px;
    background: rgba(245, 158, 11, 0.1);
    flex-shrink: 0;
  }
  .folder-icon .material-icons {
    font-size: 1.2rem;
    color: #f59e0b;
  }
  .folder-card:not(.is-dir) .folder-icon {
    background: rgba(99, 102, 241, 0.1);
  }
  .folder-card:not(.is-dir) .folder-icon .material-icons {
    color: #818cf8;
  }
  .folder-info {
    flex: 1;
    min-width: 0;
  }
  .folder-name {
    font-size: 0.82rem;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.9);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.3;
  }
  .folder-meta {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.2rem;
    font-size: 0.72rem;
    color: rgba(255, 255, 255, 0.4);
  }
  .folder-year {
    color: rgba(255, 255, 255, 0.5);
  }
  .folder-size {
    color: rgba(255, 255, 255, 0.4);
  }
  .folder-open-icon {
    font-size: 1rem;
    color: rgba(255, 255, 255, 0.2);
    flex-shrink: 0;
    transition: color 0.2s;
  }
  .folder-card:hover .folder-open-icon {
    color: #f59e0b;
  }

  .quality-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    transition: all 0.2s;
    overflow: hidden;
  }

  .quality-card.expanded {
    border-color: rgba(0, 243, 255, 0.2);
    background: rgba(255, 255, 255, 0.04);
  }

  .card-trigger {
    padding: 1.25rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    cursor: pointer;
  }

  .trigger-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .trigger-left .material-icons {
    color: var(--color-primary);
  }
  .quality-label {
    font-weight: 700;
    color: #fff;
    font-size: 1rem;
  }

  .trigger-right {
    display: flex;
    align-items: center;
    gap: 1.5rem;
  }

  .count-badge {
    background: rgba(255, 255, 255, 0.1);
    padding: 2px 10px;
    border-radius: 20px;
    font-size: 0.7rem;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.6);
  }

  /* Episode Cards */
  .season-section {
    margin-bottom: 2rem;
  }
  .season-title {
    font-size: 1rem;
    font-weight: 800;
    color: #fff;
    margin-bottom: 1rem;
    padding-left: 0.5rem;
    border-left: 3px solid var(--color-primary);
  }
  .episode-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .episode-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    overflow: hidden;
  }

  .episode-card.downloaded {
    border-color: rgba(16, 185, 129, 0.3);
    background: rgba(16, 185, 129, 0.03);
    opacity: 0.7;
  }

  .downloaded-overlay {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: rgba(16, 185, 129, 0.25);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .downloaded-overlay .material-icons {
    font-size: 1.5rem;
    color: #10b981;
    text-shadow: 0 0 8px rgba(16, 185, 129, 0.5);
  }

  .downloaded-badge {
    font-size: 0.55rem;
    font-weight: 800;
    letter-spacing: 0.05em;
    color: #10b981;
    background: rgba(16, 185, 129, 0.12);
    border: 1px solid rgba(16, 185, 129, 0.25);
    padding: 1px 6px;
    border-radius: 3px;
    margin-left: 8px;
  }

  .episode-trigger {
    display: flex;
    padding: 0.75rem;
    align-items: center;
    gap: 1rem;
    cursor: pointer;
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

  .thumb-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: rgba(255, 255, 255, 0.1);
  }
  .ep-badge {
    position: absolute;
    bottom: 4px;
    left: 4px;
    background: rgba(0, 243, 255, 0.8);
    color: #000;
    font-size: 0.6rem;
    font-weight: 900;
    padding: 1px 4px;
    border-radius: 3px;
  }

  .episode-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .ep-header-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .ep-num {
    color: var(--color-primary);
    font-weight: 800;
    font-size: 0.85rem;
    font-family: var(--font-mono);
  }

  .episode-main h4 {
    margin: 0;
    font-size: 1rem;
    font-weight: 700;
    color: #fff;
    letter-spacing: -0.01em;
  }

  .ep-meta-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.4);
  }

  .ep-meta-row .dot {
    opacity: 0.3;
  }

  .ep-overview {
    margin: 0.25rem 0 0;
    font-size: 0.8rem;
    line-height: 1.4;
    color: rgba(255, 255, 255, 0.5);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  /* File Rows */
  .file-list {
    background: rgba(0, 0, 0, 0.3);
  }
  .file-row {
    padding: 0.75rem 1.25rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-top: 1px solid rgba(255, 255, 255, 0.03);
  }

  .file-info {
    flex: 1;
    min-width: 0;
  }
  .file-name {
    font-size: 0.8rem;
    color: rgba(255, 255, 255, 0.85);
    font-weight: 500;
    margin-bottom: 0.25rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .file-meta {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    font-size: 0.7rem;
    color: rgba(255, 255, 255, 0.4);
  }

  .badges {
    display: flex;
    gap: 0.4rem;
  }
  .status-pill {
    font-size: 0.55rem;
    font-weight: 800;
    padding: 2px 6px;
    border-radius: 4px;
    text-transform: uppercase;
  }

  .status-pill.dub {
    background: rgba(139, 92, 246, 0.15);
    color: #a78bfa;
    border: 1px solid rgba(139, 92, 246, 0.3);
  }
  .status-pill.sub {
    background: rgba(16, 185, 129, 0.15);
    color: #34d399;
    border: 1px solid rgba(16, 185, 129, 0.3);
  }
  .status-pill.hdr {
    background: linear-gradient(135deg, #7c3aed, #f43f5e);
    color: #fff;
    border: none;
    box-shadow: 0 0 10px rgba(124, 58, 237, 0.4);
  }
  .status-pill.dv {
    background: linear-gradient(135deg, #f59e0b, #d946ef);
    color: #fff;
    border: none;
  }
  .status-pill.uhd {
    background: rgba(0, 243, 255, 0.15);
    color: var(--color-primary);
    border: 1px solid rgba(0, 243, 255, 0.3);
    box-shadow: 0 0 8px rgba(0, 243, 255, 0.2);
  }

  .get-btn {
    background: transparent;
    border: 1px solid rgba(0, 243, 255, 0.4);
    color: var(--color-primary);
    padding: 6px 16px;
    font-size: 0.7rem;
    font-weight: 800;
    border-radius: 4px;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    letter-spacing: 0.05em;
    clip-path: polygon(
      4px 0%,
      100% 0%,
      100% calc(100% - 4px),
      calc(100% - 4px) 100%,
      0% 100%,
      0% 4px
    );
  }

  .get-btn:hover {
    background: var(--color-primary);
    color: #000;
    border-color: var(--color-primary);
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.4);
    transform: translateY(-1px);
  }

  :global(.success-btn) {
    background: #10b981 !important;
    color: #fff !important;
    border-color: #10b981 !important;
    clip-path: none !important;
  }
  :global(.error-btn) {
    background: #ef4444 !important;
    color: #fff !important;
    border-color: #ef4444 !important;
    clip-path: none !important;
  }
  :global(.rotating) {
    animation: spin 2s linear infinite;
  }

  .score-track {
    margin-left: auto;
    width: 140px;
    height: 6px;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 3px;
    position: relative;
    display: flex;
    align-items: center;
    overflow: visible;
  }

  .score-bar {
    height: 100%;
    border-radius: 3px;
    transition: width 1.5s cubic-bezier(0.34, 1.56, 0.64, 1);
    position: relative;
  }

  .score-elite {
    box-shadow: 0 0 15px rgba(0, 243, 255, 0.5);
    animation: pulse-elite 2s infinite ease-in-out;
  }

  .score-optimal {
    box-shadow: 0 0 10px rgba(16, 185, 129, 0.3);
  }

  @keyframes pulse-elite {
    0%,
    100% {
      filter: brightness(1) drop-shadow(0 0 2px rgba(0, 243, 255, 0.4));
      transform: scaleY(1);
    }
    50% {
      filter: brightness(1.4) drop-shadow(0 0 10px rgba(0, 243, 255, 0.7));
      transform: scaleY(1.2);
    }
  }

  .score-text {
    position: absolute;
    right: 0;
    top: -16px;
    font-size: 0.65rem;
    font-weight: 900;
    font-family: var(--font-mono);
    color: #fff;
    opacity: 0.8;
    text-shadow: 0 0 10px rgba(0, 0, 0, 0.5);
  }
  .score-track.large {
    width: 180px;
    height: 8px;
    background: rgba(255, 255, 255, 0.05);
  }
  .score-track.large .score-text {
    top: -18px;
    font-size: 0.7rem;
    opacity: 1;
    color: var(--color-primary);
  }

  /* =====================================================
     TV SERIES FILE ROW - REVAMPED PREMIUM DESIGN
     ===================================================== */

  .tv-file-row {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 1rem 1.25rem;
    background: linear-gradient(
      135deg,
      rgba(15, 20, 35, 0.9) 0%,
      rgba(10, 15, 25, 0.95) 100%
    );
    border-top: 1px solid rgba(255, 255, 255, 0.04);
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    position: relative;
    overflow: hidden;
  }

  .tv-file-row::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: transparent;
    transition: background 0.3s ease;
  }

  .tv-file-row:hover {
    background: linear-gradient(
      135deg,
      rgba(20, 28, 50, 0.95) 0%,
      rgba(15, 22, 40, 0.98) 100%
    );
  }

  .tv-file-row:hover::before {
    background: linear-gradient(180deg, var(--color-primary) 0%, #06b6d4 100%);
    box-shadow: 0 0 15px rgba(0, 243, 255, 0.5);
  }

  .tv-file-content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .tv-file-name {
    font-size: 0.85rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    letter-spacing: -0.01em;
    line-height: 1.4;
    transition: color 0.2s ease;
  }

  .tv-file-row:hover .tv-file-name {
    color: #fff;
  }

  .tv-file-meta {
    display: flex;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .tv-file-size {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.72rem;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.5);
    font-family: var(--font-mono);
    background: rgba(255, 255, 255, 0.03);
    padding: 0.25rem 0.6rem;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .tv-file-size .material-icons {
    font-size: 12px;
    opacity: 0.6;
  }

  /* Premium Badge System */
  .tv-badges {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
  }

  .tv-badge {
    font-size: 0.58rem;
    font-weight: 900;
    padding: 3px 8px;
    border-radius: 5px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    transition: all 0.2s ease;
  }

  .tv-badge.vie {
    background: linear-gradient(
      135deg,
      rgba(139, 92, 246, 0.2) 0%,
      rgba(167, 139, 250, 0.15) 100%
    );
    color: #c4b5fd;
    border: 1px solid rgba(139, 92, 246, 0.35);
    box-shadow: 0 0 12px rgba(139, 92, 246, 0.15);
  }

  .tv-badge.sub {
    background: linear-gradient(
      135deg,
      rgba(16, 185, 129, 0.2) 0%,
      rgba(52, 211, 153, 0.15) 100%
    );
    color: #6ee7b7;
    border: 1px solid rgba(16, 185, 129, 0.35);
    box-shadow: 0 0 12px rgba(16, 185, 129, 0.15);
  }

  .tv-badge.hdr {
    background: linear-gradient(135deg, #7c3aed 0%, #f43f5e 100%);
    color: #fff;
    border: none;
    box-shadow: 0 0 15px rgba(124, 58, 237, 0.4);
    animation: badge-glow 2s infinite ease-in-out;
  }

  .tv-badge.dv {
    background: linear-gradient(135deg, #f59e0b 0%, #d946ef 100%);
    color: #fff;
    border: none;
    box-shadow: 0 0 15px rgba(217, 70, 239, 0.35);
  }

  .tv-badge.quality {
    background: rgba(0, 243, 255, 0.12);
    color: var(--color-primary);
    border: 1px solid rgba(0, 243, 255, 0.25);
    box-shadow: 0 0 10px rgba(0, 243, 255, 0.1);
  }

  @keyframes badge-glow {
    0%,
    100% {
      filter: brightness(1);
    }
    50% {
      filter: brightness(1.2);
    }
  }

  /* Score Container */
  .tv-score-container {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin-left: auto;
  }

  .tv-score-track {
    width: 100px;
    height: 6px;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 3px;
    overflow: hidden;
    position: relative;
  }

  .tv-score-bar {
    height: 100%;
    border-radius: 3px;
    transition: width 1s cubic-bezier(0.34, 1.56, 0.64, 1);
    position: relative;
  }

  .tv-score-bar.score-elite {
    box-shadow: 0 0 12px rgba(0, 243, 255, 0.5);
    animation: tv-pulse 2s infinite ease-in-out;
  }

  .tv-score-bar.score-optimal {
    box-shadow: 0 0 8px rgba(16, 185, 129, 0.3);
  }

  @keyframes tv-pulse {
    0%,
    100% {
      filter: brightness(1);
    }
    50% {
      filter: brightness(1.3);
    }
  }

  .tv-score-value {
    font-size: 0.7rem;
    font-weight: 900;
    font-family: var(--font-mono);
    color: var(--color-primary);
    min-width: 32px;
    text-align: right;
    text-shadow: 0 0 10px rgba(0, 243, 255, 0.3);
  }

  /* Premium GET Button */
  .tv-get-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.45rem;
    padding: 0.55rem 1.1rem;
    background: transparent;
    border: 1px solid rgba(0, 243, 255, 0.35);
    color: var(--color-primary);
    font-size: 0.68rem;
    font-weight: 900;
    letter-spacing: 0.08em;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    position: relative;
    overflow: hidden;
    flex-shrink: 0;
    clip-path: polygon(
      6px 0%,
      100% 0%,
      100% calc(100% - 6px),
      calc(100% - 6px) 100%,
      0% 100%,
      0% 6px
    );
  }

  .tv-get-btn::before {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(135deg, var(--color-primary) 0%, #06b6d4 100%);
    opacity: 0;
    transition: opacity 0.3s ease;
    z-index: 0;
  }

  .tv-get-btn:hover::before {
    opacity: 1;
  }

  .tv-get-btn:hover {
    color: #000;
    border-color: var(--color-primary);
    transform: translateY(-2px);
    box-shadow:
      0 0 25px rgba(0, 243, 255, 0.4),
      0 8px 20px rgba(0, 0, 0, 0.3);
  }

  .tv-get-btn .material-icons,
  .tv-get-btn .btn-text {
    position: relative;
    z-index: 1;
  }

  .tv-get-btn .material-icons {
    font-size: 16px;
    transition: transform 0.3s ease;
  }

  .tv-get-btn:hover .material-icons {
    transform: translateY(-1px);
    animation: download-bounce 0.6s ease infinite;
  }

  @keyframes download-bounce {
    0%,
    100% {
      transform: translateY(-1px);
    }
    50% {
      transform: translateY(2px);
    }
  }

  @media (max-width: 768px) {
    .modal-overlay {
      padding: 0;
      align-items: flex-end;
    }

    .modal-content {
      max-width: 100%;
      width: 100%;
      max-height: 100dvh;
      height: 100dvh;
      border-radius: 0;
      border: none;
    }

    .modal-header {
      padding: 1rem 1rem;
      padding-top: calc(env(safe-area-inset-top, 0px) + 0.75rem);
      position: sticky;
      top: 0;
      z-index: 10;
      background: #121212;
    }

    .header-main h2 {
      font-size: 1rem;
    }

    .smart-grab-btn {
      padding: 0.5rem 1rem;
      font-size: 0.65rem;
    }

    .modal-body {
      padding: 1rem;
    }

    /* TV file rows */
    .tv-file-row {
      flex-direction: column;
      align-items: stretch;
      gap: 0.75rem;
    }

    .tv-file-meta {
      flex-wrap: wrap;
    }

    .tv-score-container {
      margin-left: 0;
      width: 100%;
    }

    .tv-get-btn {
      width: 100%;
      justify-content: center;
      min-height: 48px;
      border-radius: 8px;
    }

    /* Movie file rows */
    .file-row {
      flex-direction: column;
      align-items: flex-start;
      gap: 0.5rem;
      padding: 0.75rem;
    }

    .file-info {
      width: 100%;
    }

    .file-name {
      font-size: 0.7rem;
      word-break: break-all;
      white-space: normal;
      line-height: 1.4;
    }

    .get-btn {
      width: 100%;
      min-height: 48px;
      justify-content: center;
      border-radius: 8px;
    }

    /* Episode cards */
    .episode-trigger {
      padding: 0.75rem;
      gap: 0.75rem;
    }

    .episode-thumb {
      width: 72px;
      height: 48px;
    }

    .ep-overview {
      display: none;
    }

    .episode-main h4 {
      font-size: 0.8rem;
    }

    /* Quality cards */
    .card-trigger {
      padding: 0.75rem;
      min-height: 48px;
    }

    .quality-label {
      font-size: 0.85rem;
    }

    .score-track.large {
      max-width: 100px;
    }
  }
</style>
