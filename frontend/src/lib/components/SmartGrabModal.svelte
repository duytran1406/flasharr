<script lang="ts">
  import { untrack } from "svelte";
  import { smartGrabStore } from "$lib/stores/smartGrab";
  import { toasts } from "$lib/stores/toasts";
  import { animeSlideDown } from "$lib/animations";
  import Badge, { type BadgeVariant } from "$lib/components/ui/Badge.svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import Button from "$lib/components/ui/Button.svelte";

  /** Extract display tags from a file object (same flags the search API returns) */
  function getFileTags(
    file: any,
  ): { text: string; variant: BadgeVariant; noDot?: boolean }[] {
    const name = (file.name || "").toLowerCase();
    const tags: { text: string; variant: BadgeVariant; noDot?: boolean }[] = [];
    if (
      file.vietdub ||
      (name.includes("vietsub") && name.includes("vie")) ||
      (!file.vietsub &&
        (name.includes(".vie.") ||
          name.includes("[vie]") ||
          name.match(/\.vie[. \[\-_]/i)))
    )
      tags.push({ text: "VIE", variant: "language" });
    if (file.vietsub || name.includes("vietsub") || name.includes(".sub."))
      tags.push({ text: "SUB", variant: "success" });
    if (
      file.hdr ||
      name.includes(".hdr.") ||
      name.includes("[hdr]") ||
      name.match(/\.hdr[. \[\-_]/i)
    )
      tags.push({ text: "HDR", variant: "hdr", noDot: true });
    if (
      file.dolby_vision ||
      name.includes(".dv.") ||
      name.includes("[dv]") ||
      name.match(/\.dv[. \[\-_]/i)
    )
      tags.push({ text: "DV", variant: "dv", noDot: true });
    if (file.quality) tags.push({ text: file.quality, variant: "quality" });
    return tags;
  }

  // Types
  interface GrabSet {
    id: string;
    name: string;
    quality?: string;
    episodeCount: number;
    totalEpisodes: number;
    coverage: number;
    avgSize: number;
    missingEpisodes: number[]; // Episodes NOT covered by this set
    files: {
      seasonNum: number;
      epNum: number;
      file: any;
      isFallback?: boolean;
    }[];
    // Uncut detection fields
    maxEpisodeNumber?: number;
    isUncut?: boolean;
    uncutReason?: string;
  }

  // State
  let qualitySets = $state<GrabSet[]>([]);
  let releaseGroupSets = $state<GrabSet[]>([]);
  let patternSets = $state<GrabSet[]>([]);
  let quickGrabInfo = $state<{
    count: number;
    avgSize: number;
    files: {
      seasonNum: number;
      epNum: number;
      file: any;
      isFallback?: boolean;
    }[];
  }>({ count: 0, avgSize: 0, files: [] });
  let totalAvailableEpisodes = $state(0);
  let allExpectedEpisodes = $state<number[]>([]); // All episode numbers expected
  let tmdbEpisodeCount = $state(0); // TMDB official episode count
  let downloadedCount = $state(0); // Episodes already downloaded

  let expandedSection = $state<string | null>("quick");
  let expandedSet = $state<string | null>(null); // Track which set is expanded to show files
  let isGrabbing = $state(false);
  let skipDownloaded = $state(true); // Toggle: skip already-downloaded episodes

  // Get existing downloads from the store
  function getExistingDownloads(): Set<string> {
    return $smartGrabStore.data?.existingDownloads ?? new Set();
  }

  function isEpisodeDownloaded(seasonNum: number, epNum: number): boolean {
    const key = `S${String(seasonNum).padStart(2, "0")}E${String(epNum).padStart(2, "0")}`;
    return getExistingDownloads().has(key);
  }

  // Helpers
  function formatSize(bytes: number) {
    if (!bytes) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  function extractQuality(filename: string): string {
    if (!filename) return "SD";
    if (/4k|2160p/i.test(filename)) return "4K";
    if (/1080p|fullhd/i.test(filename)) return "1080p";
    if (/720p/i.test(filename)) return "720p";
    return "SD";
  }

  function extractReleaseGroup(filename: string): string {
    if (!filename) return "Unknown";
    const patterns = [
      /-([A-Za-z0-9]+)(?:\.[a-z]{2,4})?$/, // -GROUP.mkv
      /\[([A-Za-z0-9]+)\]/, // [GROUP]
      /\.([A-Za-z0-9]{2,10})\./,
    ];

    for (const pattern of patterns) {
      const match = filename.match(pattern);
      if (match && match[1] && match[1].length > 1) {
        return match[1];
      }
    }
    return "Unknown";
  }

  // Extract uploader pattern for grouping files from the same source
  function extractUploaderPattern(filename: string): string {
    if (!filename) return "unknown";
    return filename
      .replace(/\.(mkv|mp4|avi|mov|webm)$/i, "") // Remove extension
      .replace(/[._-]?[Ee]p?\.?\d{1,4}[._-]?/g, "{EP}") // Ep01, E01, .01.
      .replace(/[._-]?Tap\.?\d{1,4}[._-]?/gi, "{EP}") // Vietnamese Tập
      .replace(/S\d{1,2}E\d{1,4}/gi, "{SE}") // S01E01
      .replace(/\d{1,2}x\d{1,4}/gi, "{SE}") // 1x01 format
      .replace(/[._-](\d{1,3})[._-]/g, ".{EP}.") // Standalone numbers
      .replace(/[._-]+/g, ".") // Normalize separators
      .toLowerCase()
      .substring(0, 80); // Limit length for display
  }

  // Build all groupings when data changes - use untrack to prevent infinite loop
  $effect(() => {
    const storeValue = $smartGrabStore;
    const _skip = skipDownloaded; // Track this reactive dependency
    console.log(
      "[SmartGrabModal] Effect triggered - isOpen:",
      storeValue.isOpen,
      "hasData:",
      !!storeValue.data,
      "skipDownloaded:",
      _skip,
    );
    const seasons = storeValue.data?.seasons;
    if (seasons && Array.isArray(seasons)) {
      console.log(
        "[SmartGrabModal] Building sets for",
        seasons.length,
        "seasons",
        "skipDownloaded:",
        _skip,
      );
      // Use untrack to avoid reactive loop when we write to state variables inside buildAllSets
      untrack(() => {
        try {
          buildAllSets(seasons);
        } catch (e) {
          console.error("Error building Smart Grab sets:", e);
        }
      });
    }
  });

  function buildAllSets(seasons: any[]) {
    if (!seasons || !Array.isArray(seasons)) return;
    const qualityMap = new Map<string, GrabSet>();
    const releaseGroupMap = new Map<string, GrabSet>();
    const patternMap = new Map<string, GrabSet>(); // Pattern-based grouping
    const quickFiles: { seasonNum: number; epNum: number; file: any }[] = [];

    let totalSize = 0;
    const allEpisodeNumbers: number[] = [];
    const coveredEpisodes = {
      quality: new Map<string, Set<number>>(),
      releaseGroup: new Map<string, Set<number>>(),
      pattern: new Map<string, Set<number>>(),
    };
    const patternMaxEpisode = new Map<string, number>(); // Track max episode per pattern

    // Get TMDB official episode count from first non-zero season
    let officialEpisodeCount = 0;
    for (const season of seasons) {
      if (season.season !== 0 && season.episode_count) {
        officialEpisodeCount = season.episode_count;
        break;
      }
    }
    tmdbEpisodeCount = officialEpisodeCount;

    // First pass: collect all episode numbers and build sets
    const existing = getExistingDownloads();
    let dlCount = 0;

    for (const season of seasons) {
      if (season.season === 0) continue;

      for (const ep of season.episodes_grouped || []) {
        if (!ep.files || ep.files.length === 0) continue;

        const epNum = ep.episode_number;
        // Skip episode 0 - usually specials or trash results
        if (epNum === 0 || epNum > officialEpisodeCount) continue;

        // Skip episodes already downloaded (if toggle is on)
        const epKey = `S${String(season.season).padStart(2, "0")}E${String(epNum).padStart(2, "0")}`;
        if (skipDownloaded && existing.has(epKey)) {
          dlCount++;
          continue;
        }

        allEpisodeNumbers.push(epNum);

        // Quick Grab: best file per episode (excluding uncut if possible)
        const bestFile = ep.files[0];
        quickFiles.push({
          seasonNum: season.season,
          epNum: epNum,
          file: bestFile,
        });
        totalSize += bestFile.size || 0;

        // Group each file by quality, release group, and pattern
        for (const file of ep.files) {
          const quality = extractQuality(file.name);
          const releaseGroup = extractReleaseGroup(file.name);
          const pattern = extractUploaderPattern(file.name);

          // By Quality
          if (!qualityMap.has(quality)) {
            qualityMap.set(quality, {
              id: `quality-${quality}`,
              name: quality,
              quality: quality,
              episodeCount: 0,
              totalEpisodes: 0,
              coverage: 0,
              avgSize: 0,
              missingEpisodes: [],
              files: [],
            });
            coveredEpisodes.quality.set(quality, new Set());
          }
          const qSet = qualityMap.get(quality)!;
          const qCovered = coveredEpisodes.quality.get(quality)!;
          if (!qCovered.has(epNum)) {
            qCovered.add(epNum);
            qSet.files.push({
              seasonNum: season.season,
              epNum: epNum,
              file,
            });
          }

          // By Release Group
          if (!releaseGroupMap.has(releaseGroup)) {
            releaseGroupMap.set(releaseGroup, {
              id: `rg-${releaseGroup}`,
              name: releaseGroup,
              episodeCount: 0,
              totalEpisodes: 0,
              coverage: 0,
              avgSize: 0,
              missingEpisodes: [],
              files: [],
            });
            coveredEpisodes.releaseGroup.set(releaseGroup, new Set());
          }
          const rSet = releaseGroupMap.get(releaseGroup)!;
          const rCovered = coveredEpisodes.releaseGroup.get(releaseGroup)!;
          if (!rCovered.has(epNum)) {
            rCovered.add(epNum);
            rSet.files.push({
              seasonNum: season.season,
              epNum: epNum,
              file,
            });
          }

          // By Pattern (for uncut detection)
          if (!patternMap.has(pattern)) {
            patternMap.set(pattern, {
              id: `pattern-${pattern}`,
              name:
                file.name.substring(0, 50) +
                (file.name.length > 50 ? "..." : ""), // Display first file as name
              episodeCount: 0,
              totalEpisodes: 0,
              coverage: 0,
              avgSize: 0,
              missingEpisodes: [],
              files: [],
              maxEpisodeNumber: 0,
              isUncut: false,
            });
            coveredEpisodes.pattern.set(pattern, new Set());
            patternMaxEpisode.set(pattern, 0);
          }
          const pSet = patternMap.get(pattern)!;
          const pCovered = coveredEpisodes.pattern.get(pattern)!;
          if (!pCovered.has(epNum)) {
            pCovered.add(epNum);
            pSet.files.push({
              seasonNum: season.season,
              epNum: epNum,
              file,
            });
            // Track max episode number for uncut detection
            const currentMax = patternMaxEpisode.get(pattern) || 0;
            if (epNum > currentMax) {
              patternMaxEpisode.set(pattern, epNum);
            }
          }
        }
      }
    }

    // Store all expected episodes
    allExpectedEpisodes = [...new Set(allEpisodeNumbers)].sort((a, b) => a - b);
    const totalEps = allExpectedEpisodes.length;

    // Calculate stats and missing episodes for each set
    const finalizeSets = (
      map: Map<string, GrabSet>,
      coveredMap: Map<string, Set<number>>,
      checkUncut: boolean = false,
    ): GrabSet[] => {
      const sets: GrabSet[] = [];
      for (const [key, set] of map) {
        const covered = coveredMap.get(key) || new Set();
        set.episodeCount = covered.size;
        set.totalEpisodes = totalEps;
        set.coverage = totalEps > 0 ? (covered.size / totalEps) * 100 : 0;
        set.avgSize =
          set.files.length > 0
            ? set.files.reduce((sum, f) => sum + (f.file.size || 0), 0) /
              set.files.length
            : 0;

        // Calculate MISSING episodes
        set.missingEpisodes = allExpectedEpisodes.filter(
          (ep) => !covered.has(ep),
        );

        // Add fallback files for missing episodes (from quickFiles)
        for (const missingEp of set.missingEpisodes) {
          const fallbackFile = quickFiles.find((f) => f.epNum === missingEp);
          if (fallbackFile) {
            set.files.push({
              ...fallbackFile,
              isFallback: true,
            });
          }
        }

        // Sort files by episode number
        set.files.sort((a, b) => a.epNum - b.epNum);

        // Uncut detection for pattern sets
        if (checkUncut && officialEpisodeCount > 0) {
          const maxEp = patternMaxEpisode.get(key) || 0;
          set.maxEpisodeNumber = maxEp;
          if (maxEp > officialEpisodeCount) {
            set.isUncut = true;
            set.uncutReason = `${maxEp} eps detected (official: ${officialEpisodeCount})`;
          }
        }

        sets.push(set);
      }
      // Sort by coverage descending, but put uncut sets at the bottom
      return sets.sort((a, b) => {
        if (a.isUncut && !b.isUncut) return 1;
        if (!a.isUncut && b.isUncut) return -1;
        return b.coverage - a.coverage;
      });
    };

    qualitySets = finalizeSets(qualityMap, coveredEpisodes.quality);
    releaseGroupSets = finalizeSets(
      releaseGroupMap,
      coveredEpisodes.releaseGroup,
    );
    patternSets = finalizeSets(patternMap, coveredEpisodes.pattern, true); // Enable uncut detection

    quickGrabInfo = {
      count: quickFiles.length,
      avgSize: quickFiles.length > 0 ? totalSize / quickFiles.length : 0,
      files: quickFiles,
    };

    totalAvailableEpisodes = totalEps;
    downloadedCount = dlCount;
  }

  // Get fallback count for a set
  function getFallbackCount(set: GrabSet): number {
    return set.missingEpisodes.length;
  }

  // Toggle section expansion
  function toggleSection(section: string) {
    expandedSection = expandedSection === section ? null : section;
  }

  // Grab functions
  async function executeGrab(
    files: { seasonNum: number; epNum: number; file: any }[],
    fallbackCount: number,
  ) {
    // Extra safety: filter out any already-downloaded episodes (only when skip is on)
    if (skipDownloaded) {
      const filteredFiles = files.filter(
        (f) => !isEpisodeDownloaded(f.seasonNum, f.epNum),
      );
      if (filteredFiles.length === 0) {
        toasts.info("All episodes already downloaded!");
        isGrabbing = false;
        return;
      }
      files = filteredFiles;
    }
    try {
      // Generate a single batch ID for all downloads in this grab
      // Fallback for browsers that don't support crypto.randomUUID()
      const batchId =
        crypto?.randomUUID?.() ||
        `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
      const batchName = $smartGrabStore.data?.title || "Smart Grab";

      const batchSize = 3;
      for (let i = 0; i < files.length; i += batchSize) {
        const batch = files.slice(i, i + batchSize);
        await Promise.all(
          batch.map(async (item, batchIndex) => {
            const tmdbMetadata = {
              tmdb_id: parseInt($smartGrabStore.data?.tmdbId || ""),
              media_type: "tv",
              title: $smartGrabStore.data?.title,
              year: $smartGrabStore.data?.year
                ? typeof $smartGrabStore.data.year === "string"
                  ? parseInt($smartGrabStore.data.year)
                  : $smartGrabStore.data.year
                : undefined,
              season: item.seasonNum,
              episode: item.epNum,
            };

            // Calculate priority: earlier episodes get higher priority
            // Episode 1 = priority 1000, Episode 2 = 999, etc.
            const globalIndex = i + batchIndex;
            const priority = 1000 - globalIndex;

            return fetch("/api/downloads", {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({
                url: item.file.url,
                filename: item.file.name,
                category: "tv",
                priority: priority.toString(),
                tmdb: tmdbMetadata,
                batch_id: batchId,
                batch_name: batchName,
              }),
            });
          }),
        );
      }

      const msg =
        fallbackCount === 0
          ? `Smart Grab complete! ${files.length} episodes queued.`
          : `Smart Grab complete! ${files.length} episodes (${files.length - fallbackCount} direct + ${fallbackCount} fallback).`;

      toasts.success(msg);
      smartGrabStore.close();
    } catch (e: any) {
      console.error("Smart Grab error:", e);
      toasts.error(`Smart Grab failed: ${e?.message || "Unknown error"}`);
    } finally {
      isGrabbing = false;
    }
  }

  async function grabSet(set: GrabSet) {
    isGrabbing = true;
    const toDownload = [...set.files];
    let fallbackCount = 0;

    // Find fallbacks for missing episodes from Quick Grab (best available)
    if (set.missingEpisodes.length > 0) {
      for (const missingEp of set.missingEpisodes) {
        const fallback = quickGrabInfo.files.find((f) => f.epNum === missingEp);
        if (fallback && !toDownload.find((d) => d.epNum === missingEp)) {
          toDownload.push(fallback);
          fallbackCount++;
        }
      }
    }

    await executeGrab(toDownload, fallbackCount);
  }

  async function executeQuickGrab() {
    isGrabbing = true;
    await executeGrab(quickGrabInfo.files, 0);
  }

  // Get quality color
  function getQualityColor(quality: string): string {
    switch (quality) {
      case "4K":
        return "#a855f7";
      case "1080p":
        return "#3b82f6";
      case "720p":
        return "#22c55e";
      default:
        return "#71717a";
    }
  }

  // Get coverage bar gradient
  function getCoverageGradient(coverage: number): string {
    if (coverage >= 100) return "linear-gradient(90deg, #10b981, #059669)";
    if (coverage >= 80) return "linear-gradient(90deg, #22c55e, #16a34a)";
    if (coverage >= 50) return "linear-gradient(90deg, #f59e0b, #d97706)";
    return "linear-gradient(90deg, #ef4444, #dc2626)";
  }
</script>

<Modal
  open={$smartGrabStore.isOpen && !!$smartGrabStore.data}
  onClose={() => smartGrabStore.close()}
  maxWidth="900px"
  accent="var(--color-primary, #00f3ff)"
  ariaLabel="Smart Grab"
>
  {#snippet header()}
    <div class="header-info">
      <Badge text="SMART GRAB" variant="smart" size="sm" />
      <h2>{$smartGrabStore.data?.title || "Smart Grab"}</h2>
      {#if downloadedCount > 0 || getExistingDownloads().size > 0}
        <div class="downloaded-controls">
          <span class="downloaded-info">
            <span class="material-icons">check_circle</span>
            {getExistingDownloads().size} already downloaded
          </span>
          <label class="toggle-switch">
            <input type="checkbox" bind:checked={skipDownloaded} />
            <span class="toggle-slider"></span>
            <span class="toggle-label"
              >{skipDownloaded ? "Missing only" : "Grab all"}</span
            >
          </label>
        </div>
      {/if}
    </div>
    <button class="close-btn" onclick={() => smartGrabStore.close()}>
      <span class="material-icons">close</span>
    </button>
  {/snippet}

  {#snippet children()}
    {#if $smartGrabStore.isOpen && $smartGrabStore.data}
      <!-- Quick Grab Section -->
      <div class="grab-section" class:expanded={expandedSection === "quick"}>
        <button class="section-header" onclick={() => toggleSection("quick")}>
          <div class="section-left">
            <span class="material-icons section-icon">auto_awesome</span>
            <span class="section-title">QUICK GRAB</span>
            <Badge text="BEST" variant="best" size="xs" />
          </div>
          <div class="section-right">
            <span class="section-meta">{quickGrabInfo.count} episodes</span>
            <span class="material-icons chevron">
              {expandedSection === "quick" ? "expand_less" : "expand_more"}
            </span>
          </div>
        </button>

        {#if expandedSection === "quick"}
          <div class="section-content" transition:animeSlideDown>
            <div class="set-card quick-card">
              <div class="set-info">
                <div class="coverage-row">
                  <div class="coverage-bar-container">
                    <div
                      class="coverage-bar"
                      style="width: 100%; background: {getCoverageGradient(
                        100,
                      )}"
                    ></div>
                  </div>
                  <span class="coverage-text">100%</span>
                </div>
                <div class="set-stats">
                  <span
                    >{quickGrabInfo.count}/{totalAvailableEpisodes} episodes</span
                  >
                  <span class="dot">•</span>
                  <span>~{formatSize(quickGrabInfo.avgSize)} avg</span>
                  <span class="dot">•</span>
                  <span
                    >~{formatSize(quickGrabInfo.avgSize * quickGrabInfo.count)} total</span
                  >
                </div>
                <Badge
                  text="Complete - Best quality per episode"
                  variant="success"
                  size="xs"
                />
              </div>
              <div class="set-actions">
                <button
                  class="expand-files-btn"
                  onclick={() =>
                    (expandedSet =
                      expandedSet === "quick-grab" ? null : "quick-grab")}
                >
                  <span class="material-icons">
                    {expandedSet === "quick-grab"
                      ? "expand_less"
                      : "expand_more"}
                  </span>
                  {quickGrabInfo.files.length} files
                </button>
                <Button
                  icon={isGrabbing ? "sync" : "download"}
                  loading={isGrabbing}
                  size="md"
                  onclick={executeQuickGrab}
                  disabled={isGrabbing}
                  >GRAB {quickGrabInfo.count}{downloadedCount > 0
                    ? " MISSING"
                    : ""}</Button
                >
              </div>

              {#if expandedSet === "quick-grab"}
                <div class="files-list" transition:animeSlideDown>
                  {#each quickGrabInfo.files as item}
                    <div class="file-item">
                      <Badge
                        text="E{item.epNum}"
                        variant={item.isFallback ? "warning" : "primary"}
                        size="xs"
                      />
                      {#each getFileTags(item.file) as tag}
                        <Badge
                          text={tag.text}
                          variant={tag.variant}
                          size="xs"
                          noDot={tag.noDot ?? false}
                        />
                      {/each}
                      <span class="file-name" title={item.file.name}
                        >{item.file.name}</span
                      >
                      <span class="file-size">{formatSize(item.file.size)}</span
                      >
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
        {/if}
      </div>

      <!-- By Quality Section -->
      <div class="grab-section" class:expanded={expandedSection === "quality"}>
        <button class="section-header" onclick={() => toggleSection("quality")}>
          <div class="section-left">
            <span class="material-icons section-icon">high_quality</span>
            <span class="section-title">BY QUALITY</span>
          </div>
          <div class="section-right">
            <span class="section-meta">{qualitySets.length} groups</span>
            <span class="material-icons chevron">
              {expandedSection === "quality" ? "expand_less" : "expand_more"}
            </span>
          </div>
        </button>

        {#if expandedSection === "quality"}
          <div class="section-content" transition:animeSlideDown>
            {#each qualitySets as set}
              <div class="set-card">
                <div class="set-header">
                  <Badge text={set.name} variant="quality" size="sm" />
                  {#if set.missingEpisodes.length > 0}
                    <Badge
                      text="+{set.missingEpisodes.length} fallback"
                      variant="warning"
                      size="xs"
                    />
                  {/if}
                </div>
                <div class="set-info">
                  <div class="coverage-row">
                    <div class="coverage-bar-container">
                      <div
                        class="coverage-bar"
                        style="width: {set.coverage}%; background: {getCoverageGradient(
                          set.coverage,
                        )}"
                      ></div>
                    </div>
                    <span class="coverage-text">{set.coverage.toFixed(0)}%</span
                    >
                  </div>
                  <div class="set-stats">
                    <span>{set.episodeCount}/{set.totalEpisodes} episodes</span>
                    <span class="dot">•</span>
                    <span>~{formatSize(set.avgSize)} avg</span>
                  </div>

                  <!-- MISSING EPISODES - Red Tags -->
                  {#if set.missingEpisodes.length > 0}
                    <div class="missing-section">
                      <span class="missing-label">Missing:</span>
                      <div class="missing-tags">
                        {#each set.missingEpisodes.slice(0, 8) as ep}
                          <Badge text="E{ep}" variant="danger" size="xs" />
                        {/each}
                        {#if set.missingEpisodes.length > 8}
                          <span class="missing-more"
                            >+{set.missingEpisodes.length - 8} more</span
                          >
                        {/if}
                      </div>
                    </div>
                  {:else}
                    <Badge text="Complete set" variant="success" size="xs" />
                  {/if}
                </div>
                <div class="set-actions">
                  <button
                    class="expand-files-btn"
                    onclick={() =>
                      (expandedSet = expandedSet === set.id ? null : set.id)}
                  >
                    <span class="material-icons">
                      {expandedSet === set.id ? "expand_less" : "expand_more"}
                    </span>
                    {set.files.length} files
                  </button>
                  <Button
                    icon={isGrabbing ? "sync" : "download"}
                    loading={isGrabbing}
                    size="md"
                    onclick={() => grabSet(set)}
                    disabled={isGrabbing}
                    >GRAB {set.files.length}{downloadedCount > 0
                      ? " MISSING"
                      : ""}</Button
                  >
                </div>

                <!-- Expanded file list -->
                {#if expandedSet === set.id}
                  <div class="files-list" transition:animeSlideDown>
                    {#each set.files as item}
                      <div class="file-item">
                        <Badge
                          text="E{item.epNum}"
                          variant={item.isFallback ? "warning" : "primary"}
                          size="xs"
                        />
                        {#each getFileTags(item.file) as tag}
                          <Badge
                            text={tag.text}
                            variant={tag.variant}
                            size="xs"
                            noDot={tag.noDot ?? false}
                          />
                        {/each}
                        <span class="file-name" title={item.file.name}
                          >{item.file.name}</span
                        >
                        <span class="file-size"
                          >{formatSize(item.file.size)}</span
                        >
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <!-- By Release Group Section -->
      <div class="grab-section" class:expanded={expandedSection === "release"}>
        <button class="section-header" onclick={() => toggleSection("release")}>
          <div class="section-left">
            <span class="material-icons section-icon">group_work</span>
            <span class="section-title">BY RELEASE GROUP</span>
          </div>
          <div class="section-right">
            <span class="section-meta">{releaseGroupSets.length} groups</span>
            <span class="material-icons chevron">
              {expandedSection === "release" ? "expand_less" : "expand_more"}
            </span>
          </div>
        </button>

        {#if expandedSection === "release"}
          <div class="section-content" transition:animeSlideDown>
            {#each releaseGroupSets as set}
              <div class="set-card">
                <div class="set-header">
                  <Badge text={set.name} variant="source" size="sm" />
                  {#if set.missingEpisodes.length > 0}
                    <Badge
                      text="+{set.missingEpisodes.length} fallback"
                      variant="warning"
                      size="xs"
                    />
                  {/if}
                </div>
                <div class="set-info">
                  <div class="coverage-row">
                    <div class="coverage-bar-container">
                      <div
                        class="coverage-bar"
                        style="width: {set.coverage}%; background: {getCoverageGradient(
                          set.coverage,
                        )}"
                      ></div>
                    </div>
                    <span class="coverage-text">{set.coverage.toFixed(0)}%</span
                    >
                  </div>
                  <div class="set-stats">
                    <span>{set.episodeCount}/{set.totalEpisodes} episodes</span>
                    <span class="dot">•</span>
                    <span>~{formatSize(set.avgSize)} avg</span>
                  </div>

                  <!-- MISSING EPISODES - Red Tags -->
                  {#if set.missingEpisodes.length > 0}
                    <div class="missing-section">
                      <span class="missing-label">Missing:</span>
                      <div class="missing-tags">
                        {#each set.missingEpisodes.slice(0, 8) as ep}
                          <Badge text="E{ep}" variant="danger" size="xs" />
                        {/each}
                        {#if set.missingEpisodes.length > 8}
                          <span class="missing-more"
                            >+{set.missingEpisodes.length - 8} more</span
                          >
                        {/if}
                      </div>
                    </div>
                  {:else}
                    <Badge text="Complete set" variant="success" size="xs" />
                  {/if}
                </div>
                <div class="set-actions">
                  <button
                    class="expand-files-btn"
                    onclick={() =>
                      (expandedSet = expandedSet === set.id ? null : set.id)}
                  >
                    <span class="material-icons">
                      {expandedSet === set.id ? "expand_less" : "expand_more"}
                    </span>
                    {set.files.length} files
                  </button>
                  <button
                    class="grab-btn"
                    onclick={() => grabSet(set)}
                    disabled={isGrabbing}
                  >
                    {#if isGrabbing}
                      <span class="material-icons rotating">sync</span>
                    {:else}
                      <span class="material-icons">download</span>
                    {/if}
                    GRAB {set.files.length}{downloadedCount > 0
                      ? " MISSING"
                      : ""}
                  </button>
                </div>

                {#if expandedSet === set.id}
                  <div class="files-list" transition:animeSlideDown>
                    {#each set.files as item}
                      <div class="file-item">
                        <Badge
                          text="E{item.epNum}"
                          variant={item.isFallback ? "warning" : "primary"}
                          size="xs"
                        />
                        {#each getFileTags(item.file) as tag}
                          <Badge
                            text={tag.text}
                            variant={tag.variant}
                            size="xs"
                            noDot={tag.noDot ?? false}
                          />
                        {/each}
                        <span class="file-name" title={item.file.name}
                          >{item.file.name}</span
                        >
                        <span class="file-size"
                          >{formatSize(item.file.size)}</span
                        >
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <!-- By Pattern Section (with Uncut Detection) -->
      <div class="grab-section" class:expanded={expandedSection === "pattern"}>
        <button class="section-header" onclick={() => toggleSection("pattern")}>
          <div class="section-left">
            <span class="material-icons section-icon">fingerprint</span>
            <span class="section-title">BY UPLOADER PATTERN</span>
            {#if tmdbEpisodeCount > 0}
              <Badge
                text="{tmdbEpisodeCount} official eps"
                variant="info"
                size="xs"
              />
            {/if}
          </div>
          <div class="section-right">
            <span class="section-meta"
              >{patternSets.filter((s) => s.coverage >= 80).length} patterns</span
            >
            <span class="material-icons chevron">
              {expandedSection === "pattern" ? "expand_less" : "expand_more"}
            </span>
          </div>
        </button>

        {#if expandedSection === "pattern"}
          <div class="section-content" transition:animeSlideDown>
            {#each patternSets
              .filter((s) => s.coverage >= 80)
              .slice(0, 10) as set}
              <div class="set-card" class:uncut-card={set.isUncut}>
                <div class="set-header">
                  <Badge text={set.name} variant="purple" size="sm" noDot />
                  {#if set.isUncut}
                    <Badge text="UNCUT" variant="warning" size="xs" />
                  {:else if set.missingEpisodes.length > 0}
                    <Badge
                      text="+{set.missingEpisodes.length} fallback"
                      variant="warning"
                      size="xs"
                    />
                  {/if}
                </div>
                <div class="set-info">
                  <div class="coverage-row">
                    <div class="coverage-bar-container">
                      <div
                        class="coverage-bar"
                        style="width: {set.coverage}%; background: {set.isUncut
                          ? 'linear-gradient(90deg, #f59e0b, #d97706)'
                          : getCoverageGradient(set.coverage)}"
                      ></div>
                    </div>
                    <span class="coverage-text">{set.coverage.toFixed(0)}%</span
                    >
                  </div>
                  <div class="set-stats">
                    <span>{set.episodeCount}/{set.totalEpisodes} episodes</span>
                    {#if set.maxEpisodeNumber && set.maxEpisodeNumber > tmdbEpisodeCount}
                      <span class="dot">•</span>
                      <span class="uncut-info">max E{set.maxEpisodeNumber}</span
                      >
                    {/if}
                    <span class="dot">•</span>
                    <span>~{formatSize(set.avgSize)} avg</span>
                  </div>

                  {#if set.isUncut}
                    <div class="uncut-warning">
                      <span class="material-icons">info</span>
                      {set.uncutReason}
                    </div>
                  {:else if set.missingEpisodes.length > 0}
                    <div class="missing-section">
                      <span class="missing-label">Missing:</span>
                      <div class="missing-tags">
                        {#each set.missingEpisodes.slice(0, 8) as ep}
                          <Badge text="E{ep}" variant="danger" size="xs" />
                        {/each}
                        {#if set.missingEpisodes.length > 8}
                          <span class="missing-more"
                            >+{set.missingEpisodes.length - 8} more</span
                          >
                        {/if}
                      </div>
                    </div>
                  {:else}
                    <Badge
                      text="Official - Complete set"
                      variant="success"
                      size="xs"
                    />
                  {/if}
                </div>
                <div class="set-actions">
                  <button
                    class="expand-files-btn"
                    onclick={() =>
                      (expandedSet = expandedSet === set.id ? null : set.id)}
                  >
                    <span class="material-icons">
                      {expandedSet === set.id ? "expand_less" : "expand_more"}
                    </span>
                    {set.files.length} files
                  </button>
                  <button
                    class="grab-btn"
                    class:uncut-grab={set.isUncut}
                    onclick={() => grabSet(set)}
                    disabled={isGrabbing}
                    title={set.isUncut
                      ? "Warning: This appears to be an uncut version"
                      : ""}
                  >
                    {#if isGrabbing}
                      <span class="material-icons rotating">sync</span>
                    {:else}
                      <span class="material-icons">download</span>
                    {/if}
                    GRAB {set.files.length}{downloadedCount > 0
                      ? " MISSING"
                      : ""}
                  </button>
                </div>

                {#if expandedSet === set.id}
                  <div class="files-list" transition:animeSlideDown>
                    {#each set.files as item}
                      <div class="file-item">
                        <Badge
                          text="E{item.epNum}"
                          variant={item.isFallback ? "warning" : "primary"}
                          size="xs"
                        />
                        {#each getFileTags(item.file) as tag}
                          <Badge
                            text={tag.text}
                            variant={tag.variant}
                            size="xs"
                            noDot={tag.noDot ?? false}
                          />
                        {/each}
                        <span class="file-name" title={item.file.name}
                          >{item.file.name}</span
                        >
                        <span class="file-size"
                          >{formatSize(item.file.size)}</span
                        >
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  {/snippet}
</Modal>

<style>
  .downloaded-info {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 0.75rem;
    font-weight: 600;
    color: #10b981;
    background: rgba(16, 185, 129, 0.1);
    border: 1px solid rgba(16, 185, 129, 0.2);
    padding: 3px 10px;
    border-radius: 6px;
    margin-top: 6px;
  }

  .downloaded-info .material-icons {
    font-size: 0.85rem;
  }

  .downloaded-controls {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 8px;
    flex-wrap: wrap;
  }

  .toggle-switch {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    user-select: none;
  }

  .toggle-switch input {
    display: none;
  }

  .toggle-slider {
    width: 36px;
    height: 20px;
    background: rgba(255, 255, 255, 0.15);
    border-radius: 10px;
    position: relative;
    transition: background 0.2s;
  }

  .toggle-slider::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    background: #fff;
    border-radius: 50%;
    transition: transform 0.2s;
  }

  .toggle-switch input:checked + .toggle-slider {
    background: #10b981;
  }

  .toggle-switch input:checked + .toggle-slider::after {
    transform: translateX(16px);
  }

  .toggle-label {
    font-size: 0.7rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.7);
    letter-spacing: 0.02em;
  }

  .close-btn {
    background: rgba(255, 255, 255, 0.05);
    border: none;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    color: rgba(255, 255, 255, 0.6);
    transition: all 0.2s;
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }

  /* Sections */
  .grab-section {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 12px;
    margin-bottom: 12px;
    overflow: hidden;
    transition: all 0.2s;
  }

  .grab-section.expanded {
    border-color: rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.03);
  }

  .section-header {
    width: 100%;
    padding: 16px 20px;
    background: transparent;
    border: none;
    display: flex;
    justify-content: space-between;
    align-items: center;
    cursor: pointer;
    color: white;
    transition: background 0.2s;
  }

  .section-header:hover {
    background: rgba(255, 255, 255, 0.03);
  }

  .section-left {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .section-icon {
    font-size: 20px;
    color: rgba(255, 255, 255, 0.6);
  }

  .section-title {
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.5px;
    color: rgba(255, 255, 255, 0.9);
  }

  .section-right {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .section-meta {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.5);
  }

  .chevron {
    color: rgba(255, 255, 255, 0.4);
    transition: transform 0.2s;
  }

  .section-content {
    padding: 0 16px 16px;
  }

  /* Set Cards */
  .set-card {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    padding: 16px;
    margin-bottom: 10px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .set-card:last-child {
    margin-bottom: 0;
  }

  .quick-card {
    background: linear-gradient(
      135deg,
      rgba(139, 92, 246, 0.1),
      rgba(6, 182, 212, 0.1)
    );
    border-color: rgba(139, 92, 246, 0.2);
  }

  .set-header {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .set-info {
    flex: 1;
  }

  .coverage-row {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 8px;
  }

  .coverage-bar-container {
    flex: 1;
    height: 6px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 3px;
    overflow: hidden;
  }

  .coverage-bar {
    height: 100%;
    border-radius: 3px;
    transition: width 0.3s ease;
  }

  .coverage-text {
    font-size: 13px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.8);
    min-width: 40px;
    text-align: right;
  }

  .set-stats {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.5);
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .dot {
    color: rgba(255, 255, 255, 0.3);
  }

  /* Missing Episodes Section */
  .missing-section {
    margin-top: 10px;
    display: flex;
    align-items: flex-start;
    gap: 8px;
    flex-wrap: wrap;
  }

  .missing-label {
    font-size: 12px;
    font-weight: 600;
    color: #ef4444;
    padding-top: 2px;
  }

  .missing-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  /* Grab Button */
  .grab-btn {
    align-self: flex-end;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 10px 16px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 8px;
    color: white;
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
    transition: all 0.2s;
  }

  .grab-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
    transform: translateY(-1px);
  }

  .grab-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .grab-btn.primary {
    background: linear-gradient(135deg, #8b5cf6, #06b6d4);
    border: none;
  }

  .grab-btn.primary:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .grab-btn .material-icons {
    font-size: 18px;
  }

  /* Set Actions Container */
  .set-actions {
    display: flex;
    gap: 8px;
    align-items: center;
    margin-top: 12px;
  }

  .expand-files-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 8px 12px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    color: rgba(255, 255, 255, 0.7);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
  }

  .expand-files-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }

  .expand-files-btn .material-icons {
    font-size: 18px;
  }

  /* Expandable Files List */
  .files-list {
    margin-top: 12px;
    padding: 12px;
    background: rgba(0, 0, 0, 0.3);
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.06);
    max-height: 300px;
    overflow-y: auto;
  }

  .file-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .file-item:last-child {
    border-bottom: none;
  }

  .file-name {
    flex: 1;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.7);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-size {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.4);
    flex-shrink: 0;
  }

  /* Rotating animation */
  @keyframes rotate {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  .rotating {
    animation: rotate 1s linear infinite;
  }

  .uncut-card {
    border-color: rgba(239, 68, 68, 0.3) !important;
    background: rgba(239, 68, 68, 0.05) !important;
  }

  .uncut-info {
    color: #f59e0b;
    font-weight: 600;
  }

  .uncut-warning {
    margin-top: 10px;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: #f59e0b;
    font-weight: 500;
    padding: 8px 12px;
    background: rgba(245, 158, 11, 0.1);
    border-radius: 6px;
    border: 1px solid rgba(245, 158, 11, 0.2);
  }

  .uncut-warning .material-icons {
    font-size: 16px;
  }

  .grab-btn.uncut-grab {
    background: rgba(245, 158, 11, 0.15);
    border-color: rgba(245, 158, 11, 0.3);
    color: #f59e0b;
  }

  .grab-btn.uncut-grab:hover:not(:disabled) {
    background: rgba(245, 158, 11, 0.25);
  }

  /* ---- Mobile Responsive ---- */
  @media (max-width: 768px) {
    .section-header {
      min-height: 48px;
      padding: 0.75rem;
    }

    .section-title {
      font-size: 0.75rem;
    }

    .set-card {
      padding: 0.75rem;
      border-radius: 14px;
    }

    .set-actions {
      flex-direction: column;
      gap: 0.5rem;
    }

    .grab-btn {
      width: 100%;
      min-height: 48px;
      justify-content: center;
      border-radius: 8px;
    }

    .expand-files-btn {
      min-height: 40px;
    }

    .file-item {
      flex-wrap: wrap;
      gap: 0.5rem;
      padding: 0.5rem;
      border-radius: 6px;
    }

    .file-item .file-name {
      flex-basis: 100%;
      order: 2;
      font-size: 0.65rem;
    }

    .coverage-row {
      gap: 0.5rem;
    }

    .missing-tags {
      flex-wrap: wrap;
      gap: 0.25rem;
    }

    .uncut-warning {
      font-size: 0.7rem;
      padding: 6px 10px;
    }
  }
</style>
