<script lang="ts">
  import { Badge } from "@media-set/core-ui";
  import SmartGrabSetCard from "./SmartGrabSetCard.svelte";
  import type { SmartGrabSet } from "./smartGrabUtils";

  interface Props {
    title: string;
    icon: string;
    meta: string;
    sets: SmartGrabSet[];
    expanded: boolean;
    expandedSetId: string | null;
    isGrabbing: boolean;
    downloadedCount?: number;
    badgeText?: string;
    badgeVariant?: string;
    onToggle?: () => void;
    onToggleSet: (id: string) => void;
    onGrabSet: (set: SmartGrabSet) => void;
  }

  let {
    title,
    icon,
    meta,
    sets,
    expanded,
    expandedSetId,
    isGrabbing,
    downloadedCount = 0,
    badgeText,
    badgeVariant = "info",
    onToggle,
    onToggleSet,
    onGrabSet,
  }: Props = $props();
</script>

<div class="grab-section" class:expanded>
  <button class="section-header" type="button" aria-expanded={expanded} onclick={onToggle}>
    <div class="section-left">
      <span class="material-icons section-icon">{icon}</span>
      <span class="section-title">{title}</span>
      {#if badgeText}
        <Badge text={badgeText} variant={badgeVariant as any} size="xs" />
      {/if}
    </div>
    <div class="section-right">
      <span class="section-meta">{meta}</span>
      <span class="material-icons chevron">{expanded ? "expand_less" : "expand_more"}</span>
    </div>
  </button>
  {#if expanded}
    <div class="section-content">
      {#if sets.length === 0}
        <div class="section-empty" role="status">No eligible {title.toLowerCase()} groups.</div>
      {:else}
        {#each sets as set}
          <SmartGrabSetCard
            {set}
            expanded={expandedSetId === set.id}
            {isGrabbing}
            {downloadedCount}
            badgeVariant={title === "BY QUALITY" ? "quality" : title === "BY RELEASE GROUP" ? "source" : "purple"}
            onToggleFiles={() => onToggleSet(set.id)}
            onGrab={() => onGrabSet(set)}
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
    padding: 16px 20px;
    background: transparent;
    border: none;
    display: flex;
    justify-content: space-between;
    align-items: center;
    cursor: pointer;
    color: #fff;
    transition: background .2s;
  }

  .section-header:hover {
    background: #ffffff08;
  }

  .section-header:focus-visible {
    outline: 2px solid var(--color-primary,#00f3ff);
    outline-offset: -2px;
  }

  .section-left,
  .section-right {
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

  .section-empty {
    padding: 1rem;
    text-align: center;
    color: #ffffff73;
    font-size: .78rem;
  }

  @media (max-width:768px) {
    .section-header {
      min-height: 48px;
      padding: .75rem;
    }
    .section-title {
      font-size: .75rem;
    }
  }
</style>
