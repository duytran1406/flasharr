<script lang="ts">
  import { onMount, type Snippet } from "svelte";

  interface Props {
    items: any[];
    width?: number;
    height?: number;
    space?: number;
    perspective?: number;
    display?: number;
    loop?: boolean;
    autoplay?: boolean;
    autoplayTimeout?: number;
    children: Snippet<[any]>;
  }

  let {
    items = [],
    width = 540,
    height = 304,
    space = 180, // Spacing between carousel items
    perspective = 1200,
    display = 10,
    loop = true,
    autoplay = true,
    autoplayTimeout = 3000,
    children,
  }: Props = $props();

  // ── Pointer / swipe tuning ─────────────────────────────────────────────
  const SWIPE_THRESHOLD = 60; // px of horizontal travel that counts as a swipe
  const DRAG_INTENT_MIN = 8; // px before a drag is considered intentional
  const HORIZONTAL_BIAS = 1.25; // dx must beat dy by this factor to be horizontal

  let reducedMotion = $state(false);

  // ── 3D layout tuning ───────────────────────────────────────────────────
  const ROTATE_STEP = 32; // deg of rotateY per step away from centre (capped at 46)
  const DEPTH_STEP = 220; // px of translateZ per step away from centre
  const SCALE_STEP = 0.08;
  const MIN_OPACITY = 0.42;
  const RENDER_BUFFER = 1; // extra items kept rendered beyond the visible window

  let interactionPaused = $state(false);
  let autoplayTimer: ReturnType<typeof setInterval> | null = null;

  let currentIndex = $state(0);
  let totalItems = $derived(items.length);

  // Pointer state
  let startX = 0;
  let startY = 0;
  let activePointerId: number | null = null;
  let pointerActive = false;
  let suppressNextClick = false;

  // ── Autoplay ───────────────────────────────────────────────────────────
  function stopAutoplay() {
    if (autoplayTimer) {
      clearInterval(autoplayTimer);
      autoplayTimer = null;
    }
  }

  function startAutoplay() {
    stopAutoplay();
    if (!autoplay || reducedMotion || totalItems <= 1) return;
    autoplayTimer = setInterval(next, autoplayTimeout);
  }

  function resumeAutoplay() {
    if (!interactionPaused) startAutoplay();
  }

  function pauseInteraction() {
    interactionPaused = true;
    stopAutoplay();
  }

  function resumeInteraction() {
    interactionPaused = false;
    resumeAutoplay();
  }

  // ── Navigation ─────────────────────────────────────────────────────────
  function normalizeIndex(index: number): number {
    if (totalItems === 0) return 0;
    return loop
      ? (index + totalItems) % totalItems
      : Math.max(0, Math.min(index, totalItems - 1));
  }

  function goTo(index: number) {
    currentIndex = normalizeIndex(index);
  }

  function next() {
    goTo(currentIndex + 1);
  }

  function prev() {
    goTo(currentIndex - 1);
  }

  function swipe(delta: number) {
    if (delta > 0) next();
    else prev();
  }

  function handleArrowKeys(e: KeyboardEvent) {
    if (e.key === "ArrowRight") {
      e.preventDefault();
      next();
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      prev();
    }
  }

  /** Signed distance from the current item, taking the short way round when looping. */
  function offsetFromCurrent(index: number): number {
    let diff = index - currentIndex;
    if (loop && totalItems > 0) {
      while (diff > totalItems / 2) diff -= totalItems;
      while (diff < -totalItems / 2) diff += totalItems;
    }
    return diff;
  }

  function visibleRadius(): number {
    return Math.floor(display / 2) + RENDER_BUFFER;
  }

  function isBeyondDisplay(absDiff: number): boolean {
    return absDiff > visibleRadius();
  }

  function isWithinRenderWindow(index: number): boolean {
    return Math.abs(offsetFromCurrent(index)) <= visibleRadius();
  }

  // ── Pointer handling (swipe on the whole stage) ─────────────────────────
  function isHorizontalDrag(clientX: number, clientY: number): boolean {
    const dx = Math.abs(clientX - startX);
    const dy = Math.abs(clientY - startY);
    return dx >= DRAG_INTENT_MIN && dx >= dy * HORIZONTAL_BIAS;
  }

  function endPointer() {
    activePointerId = null;
    pointerActive = false;
    resumeInteraction();
  }

  function handlePointerDown(e: PointerEvent) {
    if (!e.isPrimary || e.button !== 0) return;
    activePointerId = e.pointerId;
    startX = e.clientX;
    startY = e.clientY;
    pointerActive = true;
    pauseInteraction();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function handlePointerMove(e: PointerEvent) {
    if (!pointerActive || e.pointerId !== activePointerId) return;
    if (!isHorizontalDrag(e.clientX, e.clientY)) return;
    e.preventDefault();
  }

  function handlePointerUp(e: PointerEvent) {
    if (!pointerActive || e.pointerId !== activePointerId) return;
    const delta = startX - e.clientX;
    if (isHorizontalDrag(e.clientX, e.clientY) && Math.abs(delta) >= SWIPE_THRESHOLD) {
      swipe(delta);
      // The pointerup that ends a swipe also fires a click on the item underneath
      suppressNextClick = true;
    }
    endPointer();
  }

  function handlePointerCancel(e: PointerEvent) {
    if (e.pointerId === activePointerId) endPointer();
  }

  function handleLostPointerCapture(e: PointerEvent) {
    if (e.pointerId === activePointerId) endPointer();
  }

  function handleMouseEnter() {
    pauseInteraction();
  }

  function handleMouseLeave() {
    endPointer();
    resumeInteraction();
  }

  function handleFocusIn() {
    pauseInteraction();
  }

  function handleFocusOut() {
    resumeInteraction();
  }

  // ── Item interaction ────────────────────────────────────────────────────
  function handleItemClick(index: number, e: MouseEvent) {
    if (suppressNextClick) {
      e.preventDefault();
      e.stopPropagation();
      suppressNextClick = false;
      return;
    }
    if (index !== currentIndex) {
      // Not centered, just focus it and stop propagation to prevent navigation
      e.preventDefault();
      e.stopPropagation();
      currentIndex = index;
    }
  }

  function handleItemKeydown(index: number, e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      if (index !== currentIndex) {
        goTo(index);
        return;
      }
      // Centered item: activate its link / button
      (e.currentTarget as HTMLElement)
        .querySelector<HTMLElement>("a[href],button:not([disabled])")
        ?.click();
    } else {
      handleArrowKeys(e);
    }
  }

  function handleControlClick(direction: "prev" | "next") {
    if (direction === "prev") prev();
    else next();
    resumeInteraction();
  }

  // ── Per-item styling ────────────────────────────────────────────────────
  function transitionValue(): string {
    return reducedMotion
      ? "opacity 0.18s ease"
      : "transform 0.42s cubic-bezier(0.22, 1, 0.36, 1), opacity 0.24s ease, filter 0.24s ease";
  }

  function translateFor(diff: number): number {
    return diff * space;
  }

  function depthFor(absDiff: number): number {
    return -absDiff * DEPTH_STEP;
  }

  function rotationFor(diff: number): number {
    if (reducedMotion) return 0;
    if (diff > 0) return -Math.min(Math.abs(diff) * ROTATE_STEP, 46);
    if (diff < 0) return Math.min(Math.abs(diff) * ROTATE_STEP, 46);
    return 0;
  }

  function scaleFor(absDiff: number): number {
    return Math.max(0.82, 1 - absDiff * SCALE_STEP);
  }

  function zIndexFor(absDiff: number): number {
    return Math.max(1, 100 - absDiff * 10);
  }

  function opacityFor(absDiff: number): number {
    return absDiff === 0 ? 1 : Math.max(MIN_OPACITY, 0.82 - absDiff * 0.16);
  }

  function blurFor(absDiff: number): number {
    return absDiff === 0 ? 0 : Math.min(absDiff * 1.6, 4);
  }

  function shadowFor(absDiff: number): string {
    return absDiff === 0
      ? "drop-shadow(0 18px 40px rgba(0, 0, 0, 0.32))"
      : `drop-shadow(0 10px 24px rgba(0, 0, 0, ${Math.max(0.18, 0.28 - absDiff * 0.04)}))`;
  }

  function pointerEventsFor(absDiff: number): string {
    return absDiff <= 1 ? "auto" : "none";
  }

  // Calculate position and styles for an item
  function getItemStyles(index: number): string {
    const diff = offsetFromCurrent(index);
    const absDiff = Math.abs(diff);

    // Hide items beyond display range
    if (isBeyondDisplay(absDiff)) {
      return `visibility: hidden; opacity: 0; pointer-events: none; transform: translateX(${diff * space}px); transition: ${transitionValue()};`;
    }

    return `
      transform: translateX(${translateFor(diff)}px) translateZ(${depthFor(absDiff)}px) rotateY(${rotationFor(diff)}deg) scale(${scaleFor(absDiff)});
      z-index: ${zIndexFor(absDiff)};
      opacity: ${opacityFor(absDiff)};
      visibility: visible;
      pointer-events: ${pointerEventsFor(absDiff)};
      filter: blur(${blurFor(absDiff)}px) ${shadowFor(absDiff)};
      transition: ${transitionValue()};
    `;
  }

  function glowStyle(index: number): string {
    return `--glow-opacity: ${index === currentIndex ? 0.18 : 0}`;
  }

  function itemStyle(index: number): string {
    return `${getItemStyles(index)} ${glowStyle(index)}`;
  }

  function itemClass(index: number): string {
    return `carousel-3d-item ${index === currentIndex ? "current" : ""}`;
  }

  function wrapperClass(index: number): string {
    return `item-content-wrapper ${index === currentIndex ? "is-focused" : ""}`;
  }

  // ── Accessibility helpers ───────────────────────────────────────────────
  function sliderAriaLabel(): string {
    return `Trending items slider, ${totalItems} items`;
  }

  function itemAriaLabel(index: number): string {
    return `Open item ${index + 1} of ${totalItems}`;
  }

  function itemTitle(index: number): string {
    return index === currentIndex ? "Current item" : "Focus item";
  }

  function itemTabIndex(index: number): number {
    return index === currentIndex ? 0 : -1;
  }

  function itemAriaCurrent(index: number): "true" | undefined {
    return index === currentIndex ? "true" : undefined;
  }

  function controlLabel(direction: "prev" | "next"): string {
    return direction === "prev" ? "Previous slide" : "Next slide";
  }

  function controlsClass(): string {
    return totalItems > 1 ? "" : "hidden";
  }

  onMount(() => {
    const motionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
    const updateMotion = () => {
      reducedMotion = motionQuery.matches;
      resumeAutoplay();
    };
    updateMotion();
    motionQuery.addEventListener("change", updateMotion);
    startAutoplay();
    return () => {
      motionQuery.removeEventListener("change", updateMotion);
      stopAutoplay();
    };
  });
</script>

<section
  class="carousel-3d-container"
  aria-label={sliderAriaLabel()}
  style="perspective: {perspective}px; height: {height + 13}px;"
  onpointerdown={handlePointerDown}
  onpointermove={handlePointerMove}
  onpointerup={handlePointerUp}
  onpointercancel={handlePointerCancel}
  onlostpointercapture={handleLostPointerCapture}
  onmouseleave={handleMouseLeave}
  onmouseenter={handleMouseEnter}
  onfocusin={handleFocusIn}
  onfocusout={handleFocusOut}
>
  <div class="carousel-3d-slider" style="width: {width}px; height: {height}px;">
    {#each items as _, i}
      <div
        class={itemClass(i)}
        style={itemStyle(i)}
        role="group"
        tabindex={itemTabIndex(i)}
        aria-current={itemAriaCurrent(i)}
        aria-label={itemAriaLabel(i)}
        title={itemTitle(i)}
        aria-hidden={!isWithinRenderWindow(i)}
        onclick={(e) => handleItemClick(i, e)}
        onkeydown={(e) => handleItemKeydown(i, e)}
      >
        <div class={wrapperClass(i)}>
          {@render children(items[i])}
        </div>
      </div>
    {/each}
  </div>

  <div class="carousel-3d-controls {controlsClass()}">
    <button
      type="button"
      class="ctrl-btn prev"
      aria-label={controlLabel("prev")}
      onclick={() => handleControlClick("prev")}
    >
      <span class="material-icons">chevron_left</span>
    </button>
    <button
      type="button"
      class="ctrl-btn next"
      aria-label={controlLabel("next")}
      onclick={() => handleControlClick("next")}
    >
      <span class="material-icons">chevron_right</span>
    </button>
  </div>
</section>

<style>
  .carousel-3d-container {
    position: relative;
    width: 100%;
    margin: 0 auto;
    overflow: visible;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    user-select: none;
    cursor: grab;
    touch-action: pan-y;
    contain: layout paint style;
  }

  .carousel-3d-container:active {
    cursor: grabbing;
  }

  .carousel-3d-slider {
    position: relative;
    margin: 0 auto;
    transform-style: preserve-3d;
  }

  .carousel-3d-item {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    cursor: pointer;
    user-select: none;
    border-radius: 16px;
    -webkit-user-drag: none;
    will-change: transform, opacity, filter;
    backface-visibility: hidden;
    transform-origin: center center;
  }

  .carousel-3d-item.current {
    cursor: default;
    z-index: 100 !important;
  }

  .item-content-wrapper {
    width: 100%;
    height: 100%;
    position: relative;
  }

  .carousel-3d-item.current .item-content-wrapper::after {
    content: "";
    position: absolute;
    inset: -2px;
    border-radius: inherit;
    background: linear-gradient(
      45deg,
      rgba(0, 243, 255, 0.28),
      rgba(255, 255, 255, 0.08),
      rgba(0, 243, 255, 0.18)
    );
    z-index: -1;
    opacity: var(--glow-opacity, 0.18);
    filter: blur(10px);
    transition: opacity 0.24s ease;
  }

  .carousel-3d-controls {
    position: absolute;
    top: 50%;
    left: -12px;
    right: -12px;
    transform: translateY(-50%);
    display: flex;
    justify-content: space-between;
    pointer-events: none;
    z-index: 100;
    opacity: 0.92;
  }

  .carousel-3d-controls.hidden {
    display: none;
  }

  .ctrl-btn {
    width: 40px;
    height: 40px;
    border-radius: 999px;
    background: rgba(6, 8, 14, 0.72);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #fff;
    cursor: pointer;
    pointer-events: auto;
    display: flex;
    align-items: center;
    justify-content: center;
    transition:
      transform 0.2s ease,
      background 0.2s ease,
      border-color 0.2s ease,
      box-shadow 0.2s ease;
    backdrop-filter: blur(10px);
  }

  .ctrl-btn:hover {
    background: rgba(0, 243, 255, 0.16);
    color: #fff;
    box-shadow: 0 10px 24px rgba(0, 243, 255, 0.22);
    border-color: rgba(0, 243, 255, 0.35);
    transform: translateY(-1px);
  }

  .ctrl-btn .material-icons {
    font-size: 20px;
  }

  @media (prefers-reduced-motion: reduce) {
    .carousel-3d-item,
    .ctrl-btn,
    .carousel-3d-item.current .item-content-wrapper::after {
      transition: none !important;
      animation: none !important;
    }
  }
</style>
