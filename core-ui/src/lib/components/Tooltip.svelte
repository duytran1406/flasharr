<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    /** Bubble text, shown above the trigger on hover */
    text: string;
    /** Trigger element(s) */
    children: Snippet;
  }

  let { text, children }: Props = $props();
</script>

<div class="tooltip-wrap">
  {@render children()}
  <div class="tooltip-bubble">
    <div class="tooltip-content">{text}</div>
    <div class="tooltip-arrow"></div>
  </div>
</div>

<style>
  .tooltip-wrap {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .tooltip-bubble {
    pointer-events: none;
    position: absolute;
    bottom: calc(100% + 8px);
    left: 50%;
    transform: translateX(-50%);
    z-index: 50;
    display: flex;
    flex-direction: column;
    align-items: center;
    opacity: 0;
    transition: opacity 0.2s ease;
  }

  .tooltip-wrap:hover .tooltip-bubble {
    opacity: 1;
  }

  .tooltip-content {
    white-space: nowrap;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.95);
    padding: 6px 12px;
    font-size: 0.75rem;
    font-weight: 700;
    color: #000;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }

  .tooltip-arrow {
    width: 0;
    height: 0;
    border-left: 6px solid transparent;
    border-right: 6px solid transparent;
    border-top: 8px solid rgba(255, 255, 255, 0.95);
  }
</style>
