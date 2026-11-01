<script lang="ts">
  import type { Snippet } from "svelte";
  import { Button } from "@media-set/core-ui";
  import StatusChecker from "$lib/components/StatusChecker.svelte";
  import { ui } from "$lib/stores/ui.svelte";

  /**
   * AppViewport — main content column: mobile header, desktop glass header
   * (dynamic page slot, health dot, speed / active stats) and the routed view.
   */

  interface Props {
    children: Snippet;
    currentRoute: string;
    viewName: string;
    globalSpeed: string;
    activeCount: number;
    openMobileDrawer: () => void;
    /** Set while the mobile drawer is open */
    inert?: boolean;
    viewContainerEl?: HTMLDivElement;
  }

  let {
    children,
    currentRoute,
    viewName,
    globalSpeed,
    activeCount,
    openMobileDrawer,
    viewContainerEl = $bindable(),
    inert = false,
  }: Props = $props();
</script>

<main class="main-viewport" {inert}>
  <!-- Mobile header -->
  <header class="mobile-header">
    <div class="mobile-header-left">
      <button
        class="mobile-menu-btn"
        onclick={openMobileDrawer}
        aria-label="Open navigation menu"
        title="Menu"
      >
        <span class="material-icons">menu</span>
      </button>
      <img
        src="/images/flasharr_logo.png"
        class="mobile-logo"
        width="32"
        height="32"
        alt="Flasharr"
      />
      <span class="mobile-brand-name">FLASHARR</span>
    </div>
    <div class="mobile-header-right">
      <div class="mobile-stat-pill" title="Current Speed">
        <span class="material-icons">speed</span>
        <span class="mobile-stat-value">{globalSpeed}</span>
      </div>
      <button
        class="mobile-icon-btn"
        onclick={() => ui.openAddDownload()}
        aria-label="Add Download"
        title="Add Download"
      >
        <span class="material-icons">add_circle</span>
      </button>
    </div>
  </header>

  <!-- Desktop header -->
  <header class="glass-header desktop-header">
    <div class="header-left">
      <!-- Pages portal their own header content into this element -->
      <div id="header-dynamic-content" style="flex: 1; margin-right: 2rem;"></div>
    </div>
    <div class="header-right">
      {#if currentRoute === "/downloads"}
        <div class="add-dl-wrap">
          <Button
            icon="add_circle"
            onclick={() => ui.openAddDownload()}
            title="Add Download (Ctrl+N)"
          >
            <span class="dl-btn-text">Add Download</span>
          </Button>
        </div>
      {/if}
      <div style="margin-right: 1.5rem;">
        <StatusChecker />
      </div>
      <div class="stat-pill" title="Current Speed">
        <span class="material-icons">speed</span>
        <span class="stat-value" id="global-speed">{globalSpeed}</span>
      </div>
      <div class="stat-pill" title="Active Downloads">
        <span class="material-icons">downloading</span>
        <span class="stat-value" id="global-active">{activeCount}</span>
      </div>
    </div>
  </header>

  <div class="view-container" bind:this={viewContainerEl} data-view={viewName}>
    <div class="transition-wrapper">
      {@render children()}
    </div>
  </div>
</main>

<style>
  .header-left, .header-right {
    display: flex;
    align-items: center;
  }

  .main-viewport {
    display: flex;
    min-width: 0;
    height: 100vh;
    height: 100dvh;
    flex: 1;
    flex-direction: column;
    overflow: hidden;
  }

  .mobile-header {
    display: none;
    height: var(--cui-mobile-header-height);
    align-items: center;
    justify-content: space-between;
    padding: 0 var(--cui-space-md);
    flex-shrink: 0;
    background: var(--cui-glass-heavy);
    border-bottom: 1px solid var(--cui-glass-border);
    z-index: var(--cui-z-sticky);
    position: sticky;
    top: 0;
  }

  .mobile-header-left {
    display: flex;
    align-items: center;
    gap: .6rem;
  }

  .mobile-menu-btn, .mobile-icon-btn {
    width: var(--cui-touch-min);
    height: var(--cui-touch-min);
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--cui-radius-md);
    border: 1px solid var(--cui-glass-border);
    color: var(--cui-primary);
    cursor: pointer;
    flex-shrink: 0;
  }

  .mobile-menu-btn {
    background: var(--cui-surface-hover);
  }

  .mobile-icon-btn {
    background: var(--cui-surface-selected);
    transition: background var(--cui-duration-fast) var(--cui-ease-standard),box-shadow var(--cui-duration-fast) var(--cui-ease-standard);
  }

  .mobile-menu-btn .material-icons {
    font-size: 1.35rem;
  }

  .mobile-logo {
    width: 32px;
    height: 32px;
    object-fit: contain;
  }

  .mobile-brand-name {
    font-family: var(--cui-font-heading);
    font-size: .78rem;
    font-weight: 900;
    letter-spacing: .22em;
    text-transform: uppercase;
    color: var(--cui-text-primary);
  }

  .mobile-header-right {
    display: flex;
    align-items: center;
    gap: .5rem;
  }

  .mobile-stat-pill {
    display: flex;
    align-items: center;
    gap: .35rem;
    padding: .35rem .65rem;
    background: var(--cui-input-bg);
    border: 1px solid var(--cui-glass-border);
    border-radius: var(--cui-radius-full);
  }

  .mobile-stat-pill .material-icons {
    font-size: .9rem;
    opacity: .55;
    color: var(--cui-primary);
  }

  .mobile-stat-value {
    font-family: var(--cui-font-mono);
    font-size: .72rem;
    font-weight: 700;
    color: var(--cui-primary);
    font-variant-numeric: tabular-nums;
  }

  .mobile-icon-btn:hover {
    background: var(--cui-surface-active);
    box-shadow: var(--cui-glow-primary);
  }

  .mobile-icon-btn .material-icons {
    font-size: 1.2rem;
  }

  .desktop-header {
    display: flex;
  }

  .transition-wrapper {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .add-dl-wrap {
    margin-right: 1.5rem;
    display: flex;
    align-items: center;
  }

  /* Header stat pills / connection badge (global: also used by page headers) */
  :global(.stat-pill) {
    background: var(--cui-input-bg) !important;
    border: 1px solid var(--cui-glass-border) !important;
    border-radius: var(--cui-radius-md) !important;
    padding: .45rem .8rem !important;
    height: auto !important;
    gap: .6rem !important;
    clip-path: none;
    transition: all var(--cui-duration-base) var(--cui-ease-standard);
  }

  :global(.stat-pill .stat-value) {
    font-family: var(--cui-font-mono) !important;
    font-size: .8rem !important;
    font-weight: 700 !important;
    color: var(--cui-primary) !important;
    font-variant-numeric: tabular-nums;
  }

  :global(.stat-pill .material-icons) {
    font-size: 1rem !important;
    opacity: .6;
  }

  :global(.connection-status) {
    background: var(--cui-surface-focus) !important;
    border: 1px solid var(--cui-glass-border) !important;
    padding: .45rem .8rem !important;
    display: flex;
    align-items: center;
    gap: .6rem;
    border-radius: var(--cui-radius-md);
    clip-path: none;
  }

  :global(.connection-status .status-text) {
    font-family: var(--cui-font-mono) !important;
    color: var(--cui-success) !important;
  }

  :global(.connection-status.disconnected) {
    background: rgba(255, 82, 82, .05) !important;
    border-color: var(--cui-danger) !important;
  }

  :global(.connection-status.disconnected .status-text) {
    color: var(--cui-danger) !important;
  }

  @media (max-width:767px),(pointer:coarse) and (max-height:500px) {
    .mobile-header {
      display: flex;
      height: calc(var(--cui-mobile-header-height) + env(safe-area-inset-top,0px));
      padding-top: env(safe-area-inset-top,0px);
    }

    .desktop-header {
      display: none !important;
    }

    .header-right .stat-pill {
      display: none;
    }

  }

  @media (max-width:360px) {
    .mobile-stat-value {
      display: none;
    }

    .mobile-stat-pill {
      width: 44px;
      height: 44px;
      justify-content: center;
      padding: 0;
    }

    .mobile-brand-name {
      font-size: .68rem;
      letter-spacing: .14em;
    }

  }

  @media (max-width:768px) {
    .add-dl-wrap {
      margin-right: 1rem;
    }

    .dl-btn-text {
      display: none;
    }

  }

</style>
