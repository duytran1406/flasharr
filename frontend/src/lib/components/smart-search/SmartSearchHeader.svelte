<script lang="ts">
  import { Badge, Button } from "@media-set/core-ui";

  interface Props {
    loading?: boolean;
    title?: string;
    year?: string | number | null;
    canSmartGrab?: boolean;
    onClose?: () => void;
    onSmartGrab?: () => void;
  }

  let {
    loading = false,
    title = "Results",
    year = null,
    canSmartGrab = false,
    onClose,
    onSmartGrab,
  }: Props = $props();
</script>

<div class="header-main">
  <Badge text="SMART SEARCH" variant="smart" size="sm" />
  <h2>
    {loading ? "Scanning indexes..." : title}
    {#if year && !loading}<span class="year">({year})</span>{/if}
  </h2>
</div>
<div class="header-actions">
  {#if canSmartGrab}
    <Button icon="auto_awesome" size="md" onclick={() => onSmartGrab?.()}>Smart Grab</Button>
  {/if}
  <button type="button" class="close-btn" onclick={() => onClose?.()} aria-label="Close Smart Search">
    <span class="material-icons">close</span>
  </button>
</div>

<style>
  .header-main {
    flex: 1;
  }

  h2 {
    margin: var(--cui-space-xs) 0 0;
    font-size: var(--cui-text-xl);
    font-weight: 800;
    color: var(--cui-text-primary);
  }

  .year {
    color: var(--cui-text-tertiary);
    font-weight: 400;
    margin-left: var(--cui-space-xs);
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .close-btn {
    background: var(--cui-surface-hover);
    border: 1px solid var(--cui-glass-border);
    color: var(--cui-text-secondary);
    width: var(--cui-touch-min);
    height: var(--cui-touch-min);
    border-radius: var(--cui-radius-full);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: background var(--cui-duration-fast) var(--cui-ease-standard),color var(--cui-duration-fast) var(--cui-ease-standard),border-color var(--cui-duration-fast) var(--cui-ease-standard);
  }

  .close-btn:hover {
    background: var(--cui-surface-active);
    border-color: var(--cui-glass-border-active);
    color: var(--cui-text-primary);
  }
</style>
