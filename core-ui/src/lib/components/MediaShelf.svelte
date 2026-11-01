<!--
  MediaShelf — horizontal, snap-scrolling row of media cards.

  RECOVERY NOTE: the scoped <style> below is recovered verbatim from compiled
  CSS (svelte-pv6a8e in every 2026-07..09 build, svelte-hblx8l in 2026-04-28).
  The component's JS was tree-shaken out of every surviving build, so the
  markup and script are a reconstruction from (a) the class names in that CSS,
  (b) the delegated events registered by its module (touchstart/move/end,
  mousedown/move/up, focusin/focusout, click) and (c) how the 2026-05-14
  frontend calls it: <MediaShelf title items keyExtractor>{#snippet card(item, index, isHovered)}.
-->
<script lang="ts" generics="T">
  import type { Snippet } from "svelte";

  interface Props {
    items: T[];
    /** Card renderer: (item, index, isHovered) */
    card: Snippet<[T, number, boolean]>;
    title?: string;
    keyExtractor?: (item: T, index: number) => string | number;
    /** Netflix-style "Top 10" row with outlined rank numbers */
    top10?: boolean;
    /** Card height (any CSS length); feeds --card-h */
    cardHeight?: string;
    class?: string;
  }

  let {
    items,
    card,
    title,
    keyExtractor,
    top10 = false,
    cardHeight,
    class: cls = "",
  }: Props = $props();

  let track = $state<HTMLElement>();
  let hoveredIndex = $state<number | null>(null);

  // ── drag-to-scroll (mouse) ──
  let dragging = false;
  let dragMoved = false;
  let dragStartX = 0;
  let dragStartScroll = 0;

  function onMouseDown(e: MouseEvent) {
    if (e.button !== 0 || !track) return;
    dragging = true;
    dragMoved = false;
    dragStartX = e.clientX;
    dragStartScroll = track.scrollLeft;
  }

  function onMouseMove(e: MouseEvent) {
    if (!dragging || !track) return;
    const dx = e.clientX - dragStartX;
    if (Math.abs(dx) > 5) dragMoved = true;
    if (dragMoved) {
      e.preventDefault();
      track.scrollLeft = dragStartScroll - dx;
    }
  }

  function onMouseUp() {
    dragging = false;
  }

  /** Swallow the click that ends a drag so cards don't navigate. */
  function onClick(e: MouseEvent) {
    if (dragMoved) {
      e.preventDefault();
      e.stopPropagation();
      dragMoved = false;
    }
  }

  // ── touch: first tap "hovers" (expands) a card, scrolling cancels it ──
  let touchStartX = 0;
  let touchMoved = false;

  function onTouchStart(e: TouchEvent) {
    touchStartX = e.touches[0]?.clientX ?? 0;
    touchMoved = false;
  }

  function onTouchMove(e: TouchEvent) {
    const x = e.touches[0]?.clientX ?? 0;
    if (Math.abs(x - touchStartX) > 8) {
      touchMoved = true;
      hoveredIndex = null;
    }
  }

  function onTouchEnd(e: TouchEvent) {
    if (touchMoved) return;
    const slot = (e.target as HTMLElement | null)?.closest<HTMLElement>("[data-index]");
    if (slot) hoveredIndex = Number(slot.dataset.index);
  }

  function keyOf(item: T, index: number) {
    return keyExtractor ? keyExtractor(item, index) : index;
  }
</script>

<section
  class="row-wrapper {cls}"
  style={cardHeight ? `--card-h: ${cardHeight};` : undefined}
>
  {#if title}
    <h2
      class="px-[var(--screen-margin)] text-lg font-bold tracking-tight text-white/90 font-heading"
    >
      {title}
    </h2>
  {/if}

  <div class="balloon-overflow">
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions (drag-to-scroll) -->
    <div
      bind:this={track}
      class="row-track no-scrollbar"
      class:top10-track={top10}
      role="list"
      ontouchstart={onTouchStart}
      ontouchmove={onTouchMove}
      ontouchend={onTouchEnd}
      onmousedown={onMouseDown}
      onmousemove={onMouseMove}
      onmouseup={onMouseUp}
      onmouseleave={onMouseUp}
    >
      {#each items as item, index (keyOf(item, index))}
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_click_events_have_key_events (hover tracking; cards themselves are links) -->
        <div
          class="card-slot"
          class:top10-slot={top10}
          role="listitem"
          data-index={index}
          onmouseenter={() => (hoveredIndex = index)}
          onmouseleave={() => (hoveredIndex = null)}
          onfocusin={() => (hoveredIndex = index)}
          onfocusout={() => (hoveredIndex = null)}
          onclick={onClick}
        >
          {#if top10}
            <span class="top10-number" aria-hidden="true">{index + 1}</span>
            <div class="top10-card-wrapper">
              {@render card(item, index, hoveredIndex === index)}
            </div>
          {:else}
            {@render card(item, index, hoveredIndex === index)}
          {/if}
        </div>
      {/each}
    </div>
  </div>
</section>

<style>
  .row-wrapper {
    width: 100vw;
    max-width: 100%;
  }

  .row-wrapper:hover {
    z-index: 50;
  }

  .row-track {
    display: flex;
    flex-flow: row nowrap;
    width: 100%;
    gap: var(--s-sm);
    overflow-x: auto;
    overflow-y: visible;
    padding: var(--s-md) var(--screen-margin);
    scroll-padding-left: var(--screen-margin);
    align-items: flex-start;
    scroll-snap-type: x mandatory;
  }

  .balloon-overflow {
    overflow-x: hidden;
    position: relative;
    z-index: 1;
    width: 100%;
  }

  .row-track.top10-track {
    padding-top: calc(var(--s-md) + 30px);
    padding-bottom: calc(var(--s-md) + 40px);
  }

  .card-slot {
    height: var(--card-h, 250px);
    flex-shrink: 0;
    scroll-snap-align: start;
    position: relative;
    z-index: 1;
    transition:
      transform 0.5s cubic-bezier(0.25, 0.46, 0.45, 0.94),
      z-index 0s;
  }

  .card-slot:hover {
    z-index: 100;
  }

  @media (max-width: 768px) {
    :global(.media-card) {
      --card-h: 170px;
    }
  }

  /* ── Top 10 ── */
  .top10-slot {
    display: flex;
    align-items: flex-end;
    position: relative;
    padding-left: 3rem;
    overflow: visible;
  }

  .top10-number {
    position: relative;
    height: var(--card-h);
    display: flex;
    align-items: flex-end;
    font-size: calc(var(--card-h) * 0.784);
    line-height: 0.69;
    font-weight: 250;
    font-family: "Zen Dots", sans-serif;
    color: transparent;
    -webkit-text-stroke: 3px #595959;
    letter-spacing: 0.05em;
    margin-right: calc(var(--card-h) * -0.2);
    z-index: 0;
    user-select: none;
    pointer-events: none;
    transition: all 0.4s var(--ease-netflix);
    overflow: visible;
  }

  .card-slot:hover .top10-number {
    color: #fff;
    -webkit-text-stroke: 0px transparent;
    text-shadow:
      0 0 15px rgba(0, 243, 255, 0.8),
      0 0 30px rgba(0, 243, 255, 0.5),
      0 0 50px rgba(0, 243, 255, 0.2);
    transform: translateX(-5px) scale(1.02);
  }

  .top10-slot:hover {
    transform: none !important;
  }

  .top10-card-wrapper {
    position: relative;
    z-index: 2;
  }

  @media (max-width: 768px) {
    .top10-number {
      margin-right: calc(var(--card-h) * -0.2);
      -webkit-text-stroke: 2px #595959;
    }

    .top10-slot {
      padding-left: 1rem;
    }
  }
</style>
