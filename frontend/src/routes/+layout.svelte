<script lang="ts">
  import "../app.css";
  import type { Snippet } from "svelte";
  import { page } from "$app/stores";
  import { beforeNavigate, goto } from "$app/navigation";
  import { onMount, onDestroy } from "svelte";
  import { fly, fade } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { wsClient, connectionIndicator } from "$lib/stores/websocket";
  import {
    downloadStore,
    engineStats,
    formatSpeed,
  } from "$lib/stores/downloads";
  import {
    AddDownloadModal,
    SmartSearchModal,
    Toasts,
    IntroOverlay,
  } from "$lib/components";
  import { theme } from "$lib/stores/theme";
  import { ui } from "$lib/stores/ui.svelte";
  import { setupStore } from "$lib/stores/setup.svelte";
  import { settingsStore } from "$lib/stores/settings";

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();

  let sidebarOpen = $state(false);
  let sidebarCollapsed = $state(false);

  // Intro animation states for initial loading
  let showIntro = $derived(ui.showIntro);
  let showTagline = $derived(ui.showTagline);
  let isInitialized = $state(false);

  // Reactive connection status from WebSocket store
  let connectionStatus = $derived($connectionIndicator);
  let stats = $derived($engineStats);

  // Derived values for header display
  let connected = $derived(connectionStatus.text === "Connected");
  let globalSpeed = $derived(formatSpeed(stats.total_speed));
  let activeCount = $derived(stats.active_downloads);

  // Get current route for active nav highlighting
  let currentRoute = $derived($page.url.pathname);

  const navItems = [
    { href: "/", label: "Dashboard", icon: "dashboard", route: "dashboard" },
    {
      href: "/discover",
      label: "Discover",
      icon: "movie_filter",
      route: "discover",
    },
    {
      href: "/search",
      label: "Search",
      icon: "search",
      route: "search",
    },
    {
      href: "/downloads",
      label: "Downloads",
      icon: "cloud_download",
      route: "downloads",
    },
    {
      href: "/settings",
      label: "Settings",
      icon: "settings_suggest",
      route: "settings",
    },
  ];

  function isActive(href: string): boolean {
    if (href === "/") return currentRoute === "/";
    return currentRoute.startsWith(href);
  }

  function toggleSidebar() {
    sidebarCollapsed = !sidebarCollapsed;
    if (sidebarCollapsed) {
      document.body.classList.add("sidebar-collapsed");
    } else {
      document.body.classList.remove("sidebar-collapsed");
    }
  }

  function closeMobileDrawer() {
    sidebarOpen = false;
  }

  function openMobileDrawer() {
    sidebarOpen = true;
  }

  // Initialize WebSocket and download store
  onMount(() => {
    console.log("[Layout] Initializing system...");

    // Start the intro animation sequence
    ui.startIntroSequence();

    // Async initialization wrapped in IIFE
    (async () => {
      const minDelay = new Promise((resolve) => setTimeout(resolve, 1200));

      // In setup page, we manually handle the intro if we came here directly
      if (currentRoute.startsWith("/setup")) {
        theme.init();
        await minDelay;
        ui.finishIntro();
        return;
      }

      // Check systems (single validation check using datastores only)
      const [isComplete, hasValidAccount] = await Promise.all([
        setupStore.checkStatus(),
        settingsStore.hasAccounts(), // ← Use store instead of direct fetch
      ]);

      if (!isComplete || !hasValidAccount) {
        console.log(
          "[Layout] Setup or Account missing, redirecting to wizard...",
        );
        await minDelay;
        goto("/setup");
        return;
      }

      // Account valid - initialize app systems
      console.log("[Layout] Validation passed, initializing app...");
      theme.init();

      // Initialize WebSocket and connect BEFORE fetching data
      downloadStore.initWebSocket();
      wsClient.connect();

      // Wait a bit for WebSocket to establish connection
      await new Promise((resolve) => setTimeout(resolve, 300));

      // Fetch all data once (centralized initialization)
      // This prevents duplicate calls from individual pages
      await Promise.all([
        downloadStore.fetchAll(),
        // Note: accountStore.fetch() is not needed here since we already
        // validated accounts above. Individual pages can use the store.
      ]);

      console.log("[Layout] App initialized successfully");

      await minDelay;
      ui.finishIntro();
    })();

    // Add keyboard shortcut for Add Download modal (Ctrl+N / Cmd+N)
    function handleKeydown(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key === "n") {
        e.preventDefault();
        ui.openAddDownload();
      }
    }
    window.addEventListener("keydown", handleKeydown);

    return () => {
      window.removeEventListener("keydown", handleKeydown);
    };
  });

  // Clear dynamic header on route change to prevent "ghost" headers from previous tabs
  beforeNavigate(() => {
    const headerContainer = document.getElementById("header-dynamic-content");
    if (headerContainer) {
      headerContainer.innerHTML = "";
    }
  });

  // Cleanup on unmount
  onDestroy(() => {
    console.log("[Layout] Disconnecting WebSocket...");
    wsClient.disconnect();
  });
</script>

<svelte:head>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link
    rel="preconnect"
    href="https://fonts.gstatic.com"
    crossorigin="anonymous"
  />
  <link
    href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&family=Outfit:wght@400;600;700&family=JetBrains+Mono:wght@400;500&display=swap"
    rel="stylesheet"
  />
  <link
    href="https://fonts.googleapis.com/icon?family=Material+Icons"
    rel="stylesheet"
  />
</svelte:head>

<!-- Global Intro Animation during initial loading -->
{#if showIntro}
  <IntroOverlay {showTagline} />
{/if}

<div class="app-shell" class:hidden={showIntro}>
  {#if !currentRoute.startsWith("/setup")}
    <!-- Glass Sidebar -->
    <nav class="glass-sidebar" class:show={sidebarOpen}>
      <div class="sidebar-header">
        <div
          class="brand"
          onclick={toggleSidebar}
          onkeydown={(e) => e.key === "Enter" && toggleSidebar()}
          role="button"
          tabindex="0"
          style="cursor: pointer;"
        >
          <img
            src="/images/fshare.png"
            class="brand-logo-img"
            style="height: 32px; width: 32px; border-radius: 8px"
            alt="Flasharr"
          />
          <span class="brand-text">Flasharr</span>
        </div>
        <button
          class="icon-btn-tiny sidebar-collapse-btn"
          onclick={toggleSidebar}
          style="margin-left: auto;"
        >
          <span class="material-icons" id="collapse-icon">
            {sidebarCollapsed ? "menu" : "menu_open"}
          </span>
        </button>
      </div>

      <div class="nav-links">
        {#each navItems as item}
          <a
            href={item.href}
            class="nav-item"
            class:active={isActive(item.href)}
            data-route={item.route}
            onclick={closeMobileDrawer}
          >
            <span class="material-icons">{item.icon}</span>
            <span class="item-text">{item.label}</span>
          </a>
        {/each}
      </div>

      <div class="sidebar-footer">
        <button
          class="nav-item theme-toggle-btn"
          style="width: 100%; border: none; background: transparent; cursor: pointer;"
          onclick={() => theme.toggle()}
        >
          <span class="material-icons">
            {$theme === "dark" ? "light_mode" : "dark_mode"}
          </span>
          <span class="item-text">
            {$theme === "dark" ? "Light Mode" : "Dark Mode"}
          </span>
        </button>
        <div
          style="text-align: center; margin-top: 1rem; color: var(--text-muted); font-size: 0.7rem; opacity: 0.5; font-family: var(--font-mono);"
        >
          v3.0.0
        </div>
      </div>
    </nav>

    <!-- Main Content Area -->
    <main class="main-viewport">
      <!-- Global Header (Glass) -->
      <header class="glass-header">
        <div class="header-left">
          <div
            id="header-dynamic-content"
            style="flex: 1; margin-right: 2rem;"
          ></div>
        </div>

        <div class="header-right">
          <!-- Add Download Button -->
          {#if currentRoute === "/downloads"}
            <button
              class="add-download-btn"
              onclick={() => ui.openAddDownload()}
              title="Add Download (Ctrl+N)"
            >
              <span class="material-icons">add_circle</span>
              <span class="btn-text">Add Download</span>
            </button>
          {/if}

          <div
            class="connection-status"
            class:disconnected={!connected}
            style="margin-right: 1.5rem;"
          >
            <span class="status-dot"></span>
            <span
              class="status-text"
              style="font-size: 0.75rem; font-weight: 800; text-transform: uppercase; letter-spacing: 0.05em;"
            >
              {connected ? "Connected" : "Disconnected"}
            </span>
          </div>

          <!-- Global Stats Pills -->
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

      <!-- Dynamic View Container -->
      <div class="view-container">
        <div class="transition-wrapper">
          {@render children()}
        </div>
      </div>
    </main>
  {:else}
    <!-- Setup route - render children directly without app shell -->
    {@render children()}
  {/if}
</div>

<!-- Mobile Drawer Overlay -->
{#if !currentRoute.startsWith("/setup") && sidebarOpen}
  <button
    class="mobile-drawer-overlay active"
    onclick={closeMobileDrawer}
    aria-label="Close drawer"
  ></button>
{/if}

<!-- Bottom Navigation (Mobile Only) -->
{#if !currentRoute.startsWith("/setup")}
  <nav class="bottom-nav">
    {#each navItems as item}
      <a
        href={item.href}
        class="bottom-nav-item"
        class:active={isActive(item.href)}
        data-route={item.route}
      >
        <span class="material-icons">{item.icon}</span>
        <span>{item.label}</span>
      </a>
    {/each}
  </nav>
{/if}

<!-- Add Download Modal -->
<AddDownloadModal bind:isOpen={ui.addDownloadModalOpen} />

<!-- Smart Search Modal -->
<SmartSearchModal />

<!-- Toast Notifications -->
<Toasts />

<style>
  .transition-wrapper {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  /* Additional mobile overlay styling for Svelte */
  .mobile-drawer-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    z-index: var(--z-mobile-overlay, 999);
    display: none;
    border: none;
    cursor: pointer;
  }

  .mobile-drawer-overlay.active {
    display: block;
  }

  @media (min-width: 1025px) {
    .mobile-drawer-overlay {
      display: none !important;
    }
  }

  /* Add Download Button */
  .add-download-btn {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 1.25rem;
    background: linear-gradient(135deg, #0a1018 0%, #152030 50%, #0a1018 100%);
    color: var(--color-primary);
    border: 2px solid var(--color-primary);
    font-size: 0.75rem;
    font-weight: 900;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    cursor: pointer;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    margin-right: 1.5rem;
    position: relative;
    overflow: hidden;
    font-family: var(--font-mono, monospace);
    clip-path: polygon(
      8px 0%,
      100% 0%,
      100% calc(100% - 8px),
      calc(100% - 8px) 100%,
      0% 100%,
      0% 8px
    );
  }

  .add-download-btn::before {
    content: "";
    position: absolute;
    inset: -2px;
    background: linear-gradient(
      90deg,
      transparent 0%,
      rgba(0, 243, 255, 0.2) 45%,
      rgba(0, 243, 255, 0.6) 50%,
      rgba(0, 243, 255, 0.2) 55%,
      transparent 100%
    );
    animation: scan 3s linear infinite;
    opacity: 0;
    transition: opacity 0.3s;
    pointer-events: none;
  }

  .add-download-btn:hover {
    box-shadow: 0 0 25px rgba(0, 243, 255, 0.4);
    color: #fff;
    border-color: #fff;
    transform: translateY(-2px);
  }

  .add-download-btn:hover::before {
    opacity: 1;
  }

  .add-download-btn:active {
    transform: translateY(0) scale(0.98);
  }

  .add-download-btn .material-icons {
    font-size: 1.1rem;
  }

  @media (max-width: 768px) {
    .add-download-btn .btn-text {
      display: none;
    }

    .add-download-btn {
      padding: 0.6rem;
      margin-right: 1rem;
    }
  }

  /* Telemetry Panels (Header Stats) */
  :global(.stat-pill) {
    background: rgba(0, 0, 0, 0.3) !important;
    border: 1px solid rgba(255, 255, 255, 0.05) !important;
    border-radius: 0 !important;
    padding: 0.4rem 0.75rem !important;
    height: auto !important;
    gap: 0.75rem !important;
    clip-path: polygon(0% 0%, 100% 0%, 100% 75%, 85% 100%, 0% 100%);
    transition: all 0.3s;
  }

  :global(.stat-pill .stat-value) {
    font-family: var(--font-mono, monospace) !important;
    font-size: 0.8rem !important;
    font-weight: 700 !important;
    color: var(--color-primary) !important;
  }

  :global(.stat-pill .material-icons) {
    font-size: 1rem !important;
    opacity: 0.6;
  }

  :global(.connection-status) {
    background: rgba(0, 255, 128, 0.02) !important;
    border: 1px solid rgba(0, 255, 128, 0.1) !important;
    padding: 0.4rem 0.8rem !important;
    display: flex;
    align-items: center;
    gap: 0.6rem;
    clip-path: polygon(10px 0%, 100% 0%, 100% 100%, 0% 100%, 0% 10px);
  }

  :global(.connection-status .status-text) {
    font-family: var(--font-mono, monospace) !important;
    color: #00ff80 !important;
  }

  :global(.connection-status.disconnected) {
    background: rgba(255, 0, 100, 0.05) !important;
    border-color: rgba(255, 0, 100, 0.2) !important;
  }

  :global(.connection-status.disconnected .status-text) {
    color: #ff0064 !important;
  }

  /* Sidebar Nav Items Upgrade */
  :global(.nav-item) {
    margin: 0.25rem 1rem !important;
    padding: 0.75rem 1.25rem !important;
    border-radius: 0 !important;
    position: relative;
    clip-path: polygon(
      0% 0%,
      calc(100% - 10px) 0%,
      100% 10px,
      100% 100%,
      10px 100%,
      0% calc(100% - 10px)
    );
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1) !important;
  }

  :global(.nav-item.active) {
    background: linear-gradient(
      90deg,
      rgba(0, 243, 255, 0.15),
      transparent
    ) !important;
    border-left: 2px solid var(--color-primary) !important;
    color: #fff !important;
    box-shadow: -10px 0 20px rgba(0, 243, 255, 0.1);
  }

  :global(.nav-item:hover:not(.active)) {
    background: rgba(255, 255, 255, 0.05) !important;
    color: var(--color-primary) !important;
  }

  :global(.sidebar-header) {
    border-bottom: 1px solid rgba(255, 255, 255, 0.05) !important;
    padding-bottom: 1rem !important;
    margin-bottom: 1.5rem !important;
  }

  :global(.brand-text) {
    font-family: var(--font-heading) !important;
    letter-spacing: 0.15em !important;
    font-weight: 900 !important;
    text-transform: uppercase;
    background: linear-gradient(135deg, #fff 0%, var(--color-primary) 100%);
    -webkit-background-clip: text;
    background-clip: text;
    -webkit-text-fill-color: transparent;
  }

  /* ============================================
     PREMIUM INTRO ANIMATION STYLES
     ============================================ */

  .app-shell.hidden {
    opacity: 0;
    pointer-events: none;
  }
</style>
