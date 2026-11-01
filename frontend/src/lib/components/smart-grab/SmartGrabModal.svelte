<script lang="ts">
  import { Modal } from "@media-set/core-ui";
  import { smartGrabStore } from "$lib/stores/smartGrab";
  import { toasts } from "$lib/stores/toasts";
  import SmartGrabHeader from "./SmartGrabHeader.svelte";
  import SmartGrabRecommendedSection from "./SmartGrabRecommendedSection.svelte";
  import SmartGrabSetSection from "./SmartGrabSetSection.svelte";
  import {
    buildEpisodeDownloadRequest,
    buildSmartGrabPlan,
    type SmartGrabFile,
    type SmartGrabPlan,
    type SmartGrabSet,
  } from "./smartGrabUtils";

  type SectionKey = "quick" | "quality" | "release" | "source";

  function headers(json = false): Record<string, string> {
    return json ? { "Content-Type": "application/json" } : {};
  }

  const emptyPlan: SmartGrabPlan = {
    quickGrabScenarios: [],
    qualitySets: [],
    releaseGroupSets: [],
    patternSets: [],
    quickGrabInfo: { count: 0, avgSize: 0, files: [] },
    totalAvailableEpisodes: 0,
    allExpectedEpisodes: [],
    tmdbEpisodeCount: 0,
    airedEpisodes: 0,
    downloadedCount: 0,
  };

  let expandedSection = $state<SectionKey | null>("quick");
  /** Expanded file list: a set id, or "quick-grab" for the recommended card. */
  let expandedSetId = $state<string | null>(null);
  let isGrabbing = $state(false);
  let skipDownloaded = $state(true);

  const plan = $derived.by(() => {
    const data = $smartGrabStore.data;
    return data?.seasons
      ? buildSmartGrabPlan(data.seasons, {
          skipDownloaded,
          existingDownloads: data.existingDownloads,
          targetSeason: data.targetSeason,
        })
      : emptyPlan;
  });

  const hasOptions = $derived(
    plan.quickGrabScenarios.length > 0 ||
      plan.quickGrabInfo.count > 0 ||
      plan.qualitySets.length > 0 ||
      plan.releaseGroupSets.length > 0 ||
      plan.patternSets.length > 0,
  );

  function toggleSection(section: SectionKey) {
    expandedSection = expandedSection === section ? null : section;
  }

  async function responseError(response: Response): Promise<string> {
    const text = await response.text();
    if (!text) return `HTTP ${response.status}`;
    try {
      return JSON.parse(text)?.error ?? text;
    } catch {
      return text;
    }
  }

  /** Queue every planned episode as one batch, three requests at a time. */
  async function grabFiles(files: SmartGrabFile[], fallbackCount: number) {
    const data = $smartGrabStore.data;
    if (!data || files.length === 0) return;
    isGrabbing = true;
    try {
      const batchId = crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
      const batchName = data.title || "Smart Grab";
      const concurrency = 3;
      let queued = 0;
      let failed = 0;
      let firstError = "";

      for (let start = 0; start < files.length; start += concurrency) {
        const chunk = files.slice(start, start + concurrency);
        const responses = await Promise.all(
          chunk.map(async (entry, offset) => {
            // Earlier episodes get a higher priority.
            const priority = 1000 - (start + offset);
            return fetch("/api/downloads", {
              method: "POST",
              headers: headers(true),
              body: JSON.stringify(buildEpisodeDownloadRequest(entry, data, priority, batchId, batchName)),
            });
          }),
        );
        for (const response of responses) {
          if (response.ok) {
            queued += 1;
          } else {
            failed += 1;
            if (!firstError) firstError = await responseError(response);
          }
        }
      }

      if (failed > 0) {
        const summary =
          queued > 0 ? `Smart Grab partially failed: queued ${queued}/${files.length}.` : "Smart Grab failed.";
        toasts.error(firstError ? `${summary} ${firstError}` : summary);
        return;
      }

      const message =
        fallbackCount === 0
          ? `Smart Grab complete! ${files.length} episodes queued.`
          : `Smart Grab complete! ${files.length} episodes (${files.length - fallbackCount} direct + ${fallbackCount} fallback).`;
      toasts.success(message);
      smartGrabStore.close();
    } catch (err: any) {
      toasts.error(`Smart Grab failed: ${err?.message || "Unknown error"}`);
    } finally {
      isGrabbing = false;
    }
  }

  async function grabSet(set: SmartGrabSet) {
    const files = [...set.files];
    const fallbackCount = set.files.filter((entry) => entry.isFallback).length;
    await grabFiles(files, fallbackCount);
  }

  async function grabQuick() {
    await grabFiles(plan.quickGrabInfo.files, 0);
  }

  async function grabScenario(files: SmartGrabFile[]) {
    await grabFiles(files, 0);
  }

  /** Same-source scenarios presented as sets for the BY SOURCE section. */
  const sourceSets = $derived(
    plan.quickGrabScenarios
      .filter((scenario) => scenario.kind === "same_source")
      .map((scenario) => ({
        id: scenario.id,
        name: scenario.name.replace("Same Source · ", ""),
        episodeCount: scenario.files.length,
        totalEpisodes: plan.totalAvailableEpisodes,
        coverage: scenario.coverage,
        avgSize: 0,
        missingEpisodes: [],
        files: scenario.files,
      })),
  );

  const toggleSet = (id: string) => (expandedSetId = expandedSetId === id ? null : id);
</script>

<Modal
  open={$smartGrabStore.isOpen && !!$smartGrabStore.data}
  onClose={() => smartGrabStore.close()}
  maxWidth="900px"
  accent="var(--color-primary, #00f3ff)"
  ariaLabel="Smart Grab"
>
  {#snippet header()}
    <SmartGrabHeader
      title={$smartGrabStore.data?.title || "Smart Grab"}
      downloadedCount={plan.downloadedCount}
      {skipDownloaded}
      disabled={isGrabbing}
      onSkipToggle={(skip) => (skipDownloaded = skip)}
      onClose={() => smartGrabStore.close()}
    />
  {/snippet}

  {#if $smartGrabStore.isOpen && $smartGrabStore.data}
    {#if !hasOptions}
      <div class="empty-state" role="status" aria-live="polite">
        <span class="material-icons">done_all</span>
        <p>No eligible episodes to queue.</p>
        <span>Everything is already downloaded or no matching files remain.</span>
      </div>
    {:else}
      <SmartGrabRecommendedSection
        {plan}
        expanded={expandedSection === "quick"}
        filesExpanded={expandedSetId === "quick-grab"}
        {isGrabbing}
        onToggle={() => toggleSection("quick")}
        onToggleFiles={() => (expandedSetId = expandedSetId === "quick-grab" ? null : "quick-grab")}
        onGrab={grabQuick}
        onGrabScenario={grabScenario}
      />
      <SmartGrabSetSection
        title="BY QUALITY"
        icon="high_quality"
        meta={`${plan.qualitySets.length} groups`}
        sets={plan.qualitySets}
        expanded={expandedSection === "quality"}
        {expandedSetId}
        downloadedCount={plan.downloadedCount}
        {isGrabbing}
        onToggle={() => toggleSection("quality")}
        onToggleSet={toggleSet}
        onGrabSet={grabSet}
      />
      <SmartGrabSetSection
        title="BY RELEASE GROUP"
        icon="group_work"
        meta={`${plan.releaseGroupSets.length} groups`}
        sets={plan.releaseGroupSets}
        expanded={expandedSection === "release"}
        {expandedSetId}
        downloadedCount={plan.downloadedCount}
        {isGrabbing}
        onToggle={() => toggleSection("release")}
        onToggleSet={toggleSet}
        onGrabSet={grabSet}
      />
      <SmartGrabSetSection
        title="BY SOURCE"
        icon="source"
        meta={`${sourceSets.length} sources`}
        sets={sourceSets}
        expanded={expandedSection === "source"}
        {expandedSetId}
        downloadedCount={plan.downloadedCount}
        {isGrabbing}
        onToggle={() => toggleSection("source")}
        onToggleSet={toggleSet}
        onGrabSet={grabSet}
      />
    {/if}
  {/if}
</Modal>

<style>
  .empty-state {
    min-height: 260px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: .75rem;
    text-align: center;
    color: #ffffff8c;
  }

  .empty-state .material-icons {
    font-size: 2rem;
    color: #10b981;
  }

  .empty-state p,
  .empty-state span:last-child {
    margin: 0;
  }

  .empty-state span:last-child {
    font-size: .78rem;
    color: #ffffff61;
  }
</style>
