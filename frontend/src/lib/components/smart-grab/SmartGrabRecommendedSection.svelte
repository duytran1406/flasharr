<script lang="ts">
  import { Badge } from "@media-set/core-ui";
  import SmartGrabSetCard from "./SmartGrabSetCard.svelte";
  import type { QuickGrabScenario, SmartGrabFile, SmartGrabPlan, SmartGrabSet } from "./smartGrabUtils";

  interface Props {
    plan: SmartGrabPlan;
    expanded: boolean;
    filesExpanded: boolean;
    isGrabbing: boolean;
    onToggle?: () => void;
    onToggleFiles?: () => void;
    onGrab: () => void;
    onGrabScenario: (files: SmartGrabFile[]) => void;
  }

  let { plan, expanded, filesExpanded, isGrabbing, onToggle, onToggleFiles, onGrab, onGrabScenario }: Props =
    $props();

  /** Id of the expanded alternative plan card. */
  let expandedAlternativeId = $state<string | null>(null);

  const balanced = $derived(plan.quickGrabScenarios.find((scenario) => scenario.kind === "balanced"));
  // Same-source plans are listed in the BY SOURCE section instead.
  const alternatives = $derived(
    plan.quickGrabScenarios.filter((scenario) => scenario.kind !== "balanced" && scenario.kind !== "same_source"),
  );
  const balancedFiles = $derived(balanced?.files ?? plan.quickGrabInfo.files);
  const knownSizes = $derived(balancedFiles.map((entry) => entry.file.size ?? 0).filter((size) => size > 0));
  const balancedSet = $derived<SmartGrabSet>({
    id: balanced?.id ?? "balanced",
    name: "Balanced",
    episodeCount: balancedFiles.length,
    totalEpisodes: plan.totalAvailableEpisodes,
    coverage:
      balanced?.coverage ??
      (plan.totalAvailableEpisodes ? (plan.quickGrabInfo.count / plan.totalAvailableEpisodes) * 100 : 0),
    avgSize: knownSizes.length ? knownSizes.reduce((sum, size) => sum + size, 0) / knownSizes.length : 0,
    missingEpisodes: [],
    files: balancedFiles,
  });

  const scenarioSet = (scenario: QuickGrabScenario): SmartGrabSet => ({
    id: scenario.id,
    name: scenario.name,
    episodeCount: scenario.files.length,
    totalEpisodes: plan.totalAvailableEpisodes,
    coverage: scenario.coverage,
    avgSize: 0,
    missingEpisodes: [],
    files: scenario.files,
  });
</script>

<div class="grab-section" class:expanded>
  <button class="section-header" type="button" aria-expanded={expanded} onclick={onToggle}>
    <div class="section-left"
      ><span class="material-icons section-icon">auto_awesome</span><span class="section-title">RECOMMENDED</span
      ><Badge text="BALANCED" variant="best" size="xs" /></div
    >
    <div class="section-right"
      ><span class="section-meta">{balancedSet.episodeCount}/{plan.totalAvailableEpisodes}</span><span
        class="material-icons chevron">{expanded ? "expand_less" : "expand_more"}</span
      ></div
    >
  </button>
  {#if expanded}
    <div class="section-content">
      <SmartGrabSetCard
        set={balancedSet}
        quick
        expanded={filesExpanded}
        {isGrabbing}
        downloadedCount={plan.downloadedCount}
        badgeVariant="smart"
        primaryBadgeText="Recommended"
        {onToggleFiles}
        onGrab={() => (balanced ? onGrabScenario(balanced.files) : onGrab())}
      />
      {#if balanced}
        <div class="facts" aria-label="Balanced plan summary"
          ><span>{balanced.sourceCount} sources</span><span>{balanced.acquisitionCount} acquisitions</span
          >{#each balanced.warnings as warning}<span class="warning">{warning}</span>{/each}</div
        >
      {/if}
      {#if alternatives.length}
        <h3>Alternative plans</h3>
        {#each alternatives as scenario}
          <SmartGrabSetCard
            set={scenarioSet(scenario)}
            expanded={expandedAlternativeId === scenario.id}
            {isGrabbing}
            downloadedCount={plan.downloadedCount}
            badgeVariant="source"
            primaryBadgeText={scenario.kind.replaceAll("_", " ")}
            onToggleFiles={() =>
              (expandedAlternativeId = expandedAlternativeId === scenario.id ? null : scenario.id)}
            onGrab={() => onGrabScenario(scenario.files)}
          />
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .grab-section {
    background: #ffffff05;
    border: 1px solid rgba(255,255,255,.06);
    border-radius: 12px;
    margin-bottom: 12px;
    overflow: hidden;
    transition: all .2s;
  }

  .grab-section.expanded {
    border-color: #ffffff1f;
    background: #ffffff08;
  }

  .section-header {
    width: 100%;
    min-height: var(--cui-touch-min);
    padding: 16px 20px;
    background: transparent;
    border: none;
    display: flex;
    justify-content: space-between;
    align-items: center;
    cursor: pointer;
    color: #fff;
  }

  .section-header:hover {
    background: #ffffff08;
  }

  .section-header:focus-visible {
    outline: 2px solid var(--color-primary,#00f3ff);
    outline-offset: -2px;
  }

  .section-left,
  .section-right,
  .facts {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .section-icon {
    font-size: 20px;
    color: #fff9;
  }

  .section-title {
    font-size: 13px;
    font-weight: 700;
    letter-spacing: .5px;
    color: #ffffffe6;
  }

  .section-meta {
    font-size: 13px;
    color: #ffffff80;
  }

  .chevron {
    color: #fff6;
  }

  .section-content {
    padding: 0 16px 16px;
  }

  h3 {
    margin: 1rem .25rem .5rem;
    color: #ffffffa6;
    font-size: .72rem;
    letter-spacing: .08em;
    text-transform: uppercase;
  }

  .facts {
    padding: .25rem .35rem .6rem;
    flex-wrap: wrap;
    color: #ffffff8c;
    font-size: .72rem;
  }

  .facts span {
    padding: .2rem .45rem;
    border-radius: 999px;
    background: #ffffff0d;
  }

  .facts .warning {
    color: #ffb84d;
  }

  @media (max-width:768px) {
    .section-header {
      padding: .75rem;
    }
    .section-title {
      font-size: .75rem;
    }
  }
</style>
