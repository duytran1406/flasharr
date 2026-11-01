<script lang="ts">
  import { Badge, Button } from "@media-set/core-ui";
  import SmartGrabFileList from "./SmartGrabFileList.svelte";
  import { coverageGradient, type SmartGrabSet } from "./smartGrabUtils";
  import { formatResultSize } from "../smart-search/resultUtils";

  interface Props {
    set: SmartGrabSet;
    expanded: boolean;
    isGrabbing: boolean;
    downloadedCount?: number;
    badgeVariant?: string;
    /** The Recommended card of the quick-grab section. */
    quick?: boolean;
    primaryBadgeText?: string;
    onToggleFiles?: () => void;
    onGrab?: () => void;
  }

  let {
    set,
    expanded,
    isGrabbing,
    downloadedCount = 0,
    badgeVariant = "quality",
    quick = false,
    primaryBadgeText,
    onToggleFiles,
    onGrab,
  }: Props = $props();

  const knownSizes = $derived(set.files.map((entry) => entry.file.size ?? 0).filter((size) => size > 0));
  const knownTotal = $derived(knownSizes.reduce((sum, size) => sum + size, 0));
  const unknownSizeCount = $derived(set.files.length - knownSizes.length);
</script>

<div class="set-card" class:quick-card={quick} class:uncut-card={set.isUncut}>
  <div class="set-header">
    <Badge
      text={primaryBadgeText ?? set.name}
      variant={badgeVariant as any}
      size="sm"
      noDot={badgeVariant === "purple"}
    />
    {#if set.isUncut}
      <Badge text="UNCUT" variant="warning" size="xs" />
    {:else if set.missingEpisodes.length > 0}
      <Badge text={`+${set.missingEpisodes.length} fallback`} variant="warning" size="xs" />
    {:else if quick}
      <Badge text="Complete - Best quality per episode" variant="success" size="xs" />
    {:else}
      <Badge text="Complete set" variant="success" size="xs" />
    {/if}
  </div>

  <div class="set-info">
    <div class="coverage-row">
      <div class="coverage-bar-container">
        <div
          class="coverage-bar"
          style={`width:${set.coverage}%;background:${set.isUncut ? "linear-gradient(90deg, #f59e0b, #d97706)" : coverageGradient(set.coverage)}`}
        ></div>
      </div>
      <span class="coverage-text">{set.coverage.toFixed(0)}%</span>
    </div>
    <div class="set-stats">
      <span>{set.episodeCount}/{set.totalEpisodes} episodes</span>
      <span class="dot">•</span>
      <span>{knownSizes.length ? `~${formatResultSize(set.avgSize)} avg known` : "Size unknown"}</span>
      <span class="dot">•</span>
      <span
        >{knownSizes.length ? `~${formatResultSize(knownTotal)} known total` : "Total unknown"}{unknownSizeCount
          ? ` · ${unknownSizeCount} unknown`
          : ""}</span
      >
    </div>
    {#if set.isUncut}
      <div class="uncut-warning"><span class="material-icons">info</span>{set.uncutReason}</div>
    {:else if set.missingEpisodes.length > 0}
      <div class="missing-section">
        <span class="missing-label">Missing:</span>
        <div class="missing-tags">
          {#each set.missingEpisodes.slice(0, 8) as episode}
            <Badge text={`E${episode}`} variant="danger" size="xs" />
          {/each}
          {#if set.missingEpisodes.length > 8}
            <span class="missing-more">+{set.missingEpisodes.length - 8} more</span>
          {/if}
        </div>
      </div>
    {/if}
  </div>

  <div class="set-actions">
    <button class="expand-files-btn" type="button" aria-expanded={expanded} onclick={onToggleFiles}>
      <span class="material-icons">{expanded ? "expand_less" : "expand_more"}</span>
      {set.files.length} files
    </button>
    <Button
      icon={isGrabbing ? "sync" : "download"}
      loading={isGrabbing}
      size="md"
      onclick={onGrab}
      disabled={isGrabbing}
    >
      GRAB {set.files.length}{downloadedCount > 0 ? " MISSING" : ""}
    </Button>
  </div>

  {#if expanded}
    <SmartGrabFileList files={set.files} />
  {/if}
</div>

<style>
  .set-card {
    background: #ffffff08;
    border: 1px solid rgba(255,255,255,.08);
    border-radius: 10px;
    padding: 16px;
    margin-bottom: 10px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .quick-card {
    background: linear-gradient(135deg,#8b5cf61a,#06b6d41a);
    border-color: #8b5cf633;
  }

  .uncut-card {
    border-color: #ef44444d !important;
    background: #ef44440d !important;
  }

  .set-header {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
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
    background: #ffffff1a;
    border-radius: 3px;
    overflow: hidden;
  }

  .coverage-bar {
    height: 100%;
    border-radius: 3px;
    transition: width .3s ease;
  }

  .coverage-text {
    font-size: 13px;
    font-weight: 600;
    color: #fffc;
    min-width: 40px;
    text-align: right;
  }

  .set-stats {
    font-size: 13px;
    color: #ffffff80;
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .dot {
    color: #ffffff4d;
  }

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

  .missing-more {
    font-size: 12px;
    color: #ffffff80;
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
    background: #f59e0b1a;
    border-radius: 6px;
    border: 1px solid rgba(245,158,11,.2);
  }

  .uncut-warning .material-icons {
    font-size: 16px;
  }

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
    background: #ffffff0d;
    border: 1px solid rgba(255,255,255,.1);
    border-radius: 8px;
    color: #ffffffb3;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all .2s;
  }

  .expand-files-btn:hover {
    background: #ffffff1a;
    color: #fff;
  }

  .expand-files-btn:focus-visible {
    outline: 2px solid var(--color-primary,#00f3ff);
    outline-offset: 2px;
  }

  .expand-files-btn .material-icons {
    font-size: 18px;
  }

  @media (max-width:768px) {
    .set-card {
      padding: .75rem;
      border-radius: 14px;
    }
    .set-actions {
      flex-direction: column;
      gap: .5rem;
    }
    .expand-files-btn {
      min-height: 40px;
      width: 100%;
      justify-content: center;
    }
  }
</style>
