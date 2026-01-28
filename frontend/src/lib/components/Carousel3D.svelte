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

  let currentIndex = $state(0);
  let totalItems = $derived(items.length);
  let timer: any;

  // Swipe state
  let startX = 0;
  let isMoving = false;

  function next() {
    if (currentIndex < totalItems - 1) {
      currentIndex++;
    } else if (loop) {
      currentIndex = 0;
    }
  }

  function prev() {
    if (currentIndex > 0) {
      currentIndex--;
    } else if (loop) {
      currentIndex = totalItems - 1;
    }
  }

  function handleItemClick(index: number, e: MouseEvent) {
    if (index === currentIndex) {
      // Already centered, allow standard navigation/action
      return;
    } else {
      // Not centered, just focus it and stop propagation to prevent navigation
      e.preventDefault();
      e.stopPropagation();
      currentIndex = index;
    }
  }

  function goTo(index: number) {
    currentIndex = index;
  }

  function handleTouchStart(e: TouchEvent) {
    startX = e.touches[0].clientX;
    isMoving = true;
  }

  function handleMouseDown(e: MouseEvent) {
    startX = e.clientX;
    isMoving = true;
  }

  function handleTouchMove(e: TouchEvent) {
    if (!isMoving) return;
    const currentX = e.touches[0].clientX;
    handleMove(currentX);
  }

  function handleMouseMove(e: MouseEvent) {
    if (!isMoving) return;
    handleMove(e.clientX);
  }

  function handleMove(currentX: number) {
    const diff = startX - currentX;
    if (Math.abs(diff) > 70) {
      if (diff > 0) next();
      else prev();
      isMoving = false;
    }
  }

  function handleEnd() {
    isMoving = false;
  }

  // Calculate position and styles for an item
  function getItemStyles(index: number) {
    let diff = index - currentIndex;

    // Handle circular wrapping - find shortest path around the circle
    if (loop && totalItems > 0) {
      // Normalize diff to be within -totalItems/2 to +totalItems/2
      while (diff > totalItems / 2) diff -= totalItems;
      while (diff < -totalItems / 2) diff += totalItems;
    }

    const absDiff = Math.abs(diff);

    // Hide items beyond display range
    if (absDiff > Math.floor(display / 2)) {
      return "visibility: hidden; opacity: 0; pointer-events: none;";
    }

    const zIndex = totalItems - absDiff * 5; // Aggressive z-index priority
    const opacity = index === currentIndex ? 1 : 0.3; // Adjusted opacity for side items to 0.3
    const scale = index === currentIndex ? 1.15 : 1 - absDiff * 0.05; // Scaled focused item slightly more to 1.2
    const translateX = diff * space;
    const translateZ = -absDiff * 400;
    const rotateY = diff > 0 ? -45 : diff < 0 ? 45 : 0;

    return `
      transform: translateX(${translateX}px) translateZ(${translateZ}px) rotateY(${rotateY}deg) scale(${scale});
      z-index: ${Math.floor(zIndex)};
      opacity: ${opacity};
      visibility: visible;
      pointer-events: ${absDiff === 0 ? "auto" : "auto"}; 
    `;
  }

  onMount(() => {
    if (autoplay) {
      timer = setInterval(next, autoplayTimeout);
    }
    return () => clearInterval(timer);
  });
</script>

<section
  class="carousel-3d-container"
  aria-label="Trending items slider"
  style="perspective: {perspective}px; height: {height + 13}px;"
  ontouchstart={handleTouchStart}
  ontouchmove={handleTouchMove}
  ontouchend={handleEnd}
  onmousedown={handleMouseDown}
  onmousemove={handleMouseMove}
  onmouseup={handleEnd}
  onmouseleave={handleEnd}
>
  <div class="carousel-3d-slider" style="width: {width}px; height: {height}px;">
    {#each items as item, i}
      <div
        class="carousel-3d-item {i === currentIndex ? 'current' : ''}"
        style={getItemStyles(i)}
        onclick={(e) => handleItemClick(i, e)}
        role="button"
        tabindex="0"
        onkeydown={(e) =>
          (e.key === "Enter" || e.key === " ") && handleItemClick(i, e as any)}
      >
        <div
          class="item-content-wrapper {i === currentIndex ? 'is-focused' : ''}"
        >
          {@render children(item)}
        </div>
      </div>
    {/each}
  </div>

  <div class="carousel-3d-controls">
    <button class="ctrl-btn prev" onclick={prev} aria-label="Previous">
      <span class="material-icons">chevron_left</span>
    </button>
    <button class="ctrl-btn next" onclick={next} aria-label="Next">
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
    transition: all 0.6s cubic-bezier(0.23, 1, 0.32, 1);
    cursor: pointer;
    user-select: none;
    border-radius: 16px;
    -webkit-user-drag: none;
  }

  .carousel-3d-item.current {
    cursor: default;
    filter: drop-shadow(0 0 30px rgba(0, 243, 255, 0.4));
    z-index: 100 !important;
  }

  /* Center Item Glow Overlay */
  .carousel-3d-item.current::after {
    content: "";
    position: absolute;
    inset: -2px;
    border-radius: inherit;
    background: linear-gradient(
      45deg,
      var(--color-primary),
      var(--color-secondary),
      var(--color-primary)
    );
    z-index: -1;
    opacity: 0.3;
    filter: blur(15px);
    animation: rotate-glow 5s linear infinite;
  }

  @keyframes rotate-glow {
    from {
      filter: blur(15px) hue-rotate(0deg);
    }
    to {
      filter: blur(15px) hue-rotate(360deg);
    }
  }

  .carousel-3d-controls {
    position: absolute;
    top: 50%;
    left: -20px;
    right: -20px;
    transform: translateY(-50%);
    display: flex;
    justify-content: space-between;
    pointer-events: none;
    z-index: 100;
  }

  .ctrl-btn {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.6);
    border: 1px solid rgba(0, 243, 255, 0.3);
    color: #fff;
    cursor: pointer;
    pointer-events: auto;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.3s;
    backdrop-filter: blur(10px);
  }

  .ctrl-btn:hover {
    background: var(--color-primary, #00f3ff);
    color: #000;
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.5);
    border-color: #fff;
  }

  .ctrl-btn .material-icons {
    font-size: 20px;
  }
</style>
