<script lang="ts">
  import "../app.css";
  import type { Snippet } from "svelte";
  import { page } from "$app/stores";
  import { beforeNavigate, afterNavigate, goto } from "$app/navigation";
  import { onMount, onDestroy } from "svelte";
  import {
    anime,
    stagger as staggerAction,
    animatePageEntrance,
  } from "$lib/animations";
  import { wsClient, connectionIndicator } from "$lib/stores/websocket";
  import {
    downloadStore,
    engineStats,
    formatSpeed,
  } from "$lib/stores/downloads";
  import {
    AddDownloadModal,
    SmartSearchModal,
    SmartGrabModal,
    Toasts,
    IntroOverlay,
    StatusChecker,
    AccountWarningModal,
    Button,
  } from "$lib/components";
  import { theme } from "$lib/stores/theme";
  import { ui } from "$lib/stores/ui.svelte";
  import { setupStore } from "$lib/stores/setup.svelte";
  import { settingsStore } from "$lib/stores/settings";
  import { accountStore } from "$lib/stores/account.svelte";

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();

  let sidebarOpen = $state(false);
  let sidebarCollapsed = $state(false);
  let viewContainerEl: HTMLElement | undefined = $state();

  // Account VIP warning modal
  let showAccountWarn = $state(false);
  // Derived reactively from the account store so it updates if the account changes
  let isAccountVip = $derived(accountStore.isVip);

  // Intro animation states for initial loading
  let showIntro = $derived(ui.showIntro);
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

  // Compute data-view from route for CSS targeting
  function getViewName(path: string): string {
    if (path === "/") return "dashboard";
    if (path.startsWith("/tv/")) return "media/tv";
    if (path.startsWith("/movie/")) return "media/movie";
    if (path.startsWith("/collection/"))
      return "collection/" + path.split("/")[2];
    return path.split("/")[1] || "dashboard";
  }
  let viewName = $derived(getViewName($page.url.pathname));

  const navItems = [
    { href: "/", label: "Dashboard", icon: "dashboard", route: "dashboard" },
    {
      href: "/discover",
      label: "Discover",
      icon: "movie_filter",
      route: "discover",
    },
    {
      href: "/library",
      label: "Library",
      icon: "video_library",
      route: "library",
    },
    {
      href: "/calendar",
      label: "Calendar",
      icon: "calendar_month",
      route: "calendar",
    },
    {
      href: "/downloads",
      label: "Downloads",
      icon: "cloud_download",
      route: "downloads",
    },
    {
      href: "/search",
      label: "Search",
      icon: "search",
      route: "search",
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
  // Animate page content on route change
  afterNavigate(() => {
    if (viewContainerEl) {
      const wrapper = viewContainerEl.querySelector(".transition-wrapper");
      if (wrapper) {
        anime({
          targets: wrapper,
          opacity: [0, 1],
          translateY: [12, 0],
          duration: 350,
          easing: "easeOutQuad",
        });
      }
    }
  });

  onMount(() => {
    console.log("[Layout] Initializing system...");

    // Start the intro animation sequence
    ui.startIntroSequence();

    // Animate sidebar nav items with stagger
    requestAnimationFrame(() => {
      const navLinks = document.querySelectorAll(".nav-links .nav-item");
      if (navLinks.length) {
        navLinks.forEach((el) => {
          (el as HTMLElement).style.opacity = "0";
        });
        anime({
          targets: navLinks,
          opacity: [0, 1],
          translateX: [-20, 0],
          duration: 400,
          delay: anime.stagger(50, { start: 300 }),
          easing: "easeOutCubic",
        });
      }
    });
    // All check logic now delegated to IntroOverlay via checkConfig/onComplete props.

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
  <IntroOverlay
    checkConfig={async () => {
      // Always respect a minimum display time so Appear + start of Idle is visible
      const minDelay = new Promise<void>((r) => setTimeout(r, 1200));

      // Setup page handled separately
      if (currentRoute.startsWith("/setup")) {
        await minDelay;
        return "/setup";
      }

      // Check if a Fshare account has been configured
      const [hasAccount] = await Promise.all([
        settingsStore.hasAccounts(),
        minDelay,
      ]);

      return hasAccount ? "/" : "/setup";
    }}
    onComplete={async (destination) => {
      theme.init();

      if (destination === "/setup") {
        goto("/setup");
        return;
      }

      // Already on dashboard — kick off background init
      downloadStore.initWebSocket();
      wsClient.connect();

      try {
        await settingsStore.fetchSettings();
      } catch (_) {}
      try {
        await accountStore.fetch();
        if (
          !accountStore.isVip &&
          !sessionStorage.getItem("vip-warn-dismissed")
        ) {
          showAccountWarn = true;
        }
      } catch (_) {}

      setTimeout(async () => {
        try {
          await downloadStore.fetchAll();
        } catch (_) {}
      }, 100);
    }}
  />
{/if}

<div class="app-shell" class:hidden={showIntro}>
  {#if !currentRoute.startsWith("/setup")}
    <!-- Glass Sidebar -->
    <nav
      class="glass-sidebar"
      class:show={sidebarOpen}
      class:collapsed={sidebarCollapsed}
    >
      <!-- ── Brand Header ── -->
      <!-- When collapsed: clicking the logo expands the sidebar.
           When expanded: logo area is display-only; collapse-btn handles collapse. -->
      <div class="sidebar-brand">
        <button
          class="brand-btn"
          onclick={sidebarCollapsed ? toggleSidebar : undefined}
          aria-label={sidebarCollapsed ? "Expand sidebar" : "Flasharr"}
          tabindex={sidebarCollapsed ? 0 : -1}
          style={sidebarCollapsed
            ? "cursor:pointer"
            : "cursor:default; pointer-events:none"}
        >
          <div class="logo-container">
            <img
              src="/images/flasharr_logo.png"
              class="brand-logo-img"
              width="54"
              height="54"
              alt="Flasharr"
            />
          </div>
          <div class="brand-info">
            <span class="brand-name">FLASHARR</span>
            <span class="brand-tag">MEDIA SUITE</span>
          </div>
        </button>
        <!-- Collapse button hidden when already collapsed -->
        {#if !sidebarCollapsed}
          <button
            class="collapse-btn"
            onclick={toggleSidebar}
            aria-label="Collapse sidebar"
          >
            <span class="material-icons">chevron_left</span>
          </button>
        {/if}
      </div>

      <!-- ── Divider ── -->
      <div class="sidebar-divider"></div>

      <!-- ── Nav Items ── -->
      <div class="nav-links">
        {#each navItems as item}
          <a
            href={item.href}
            class="nav-item"
            class:active={isActive(item.href)}
            data-route={item.route}
            onclick={closeMobileDrawer}
            title={sidebarCollapsed ? item.label : ""}
          >
            <!-- Halftone hover particle layer -->
            <span class="nav-hover-particles" aria-hidden="true"></span>
            <span class="nav-icon material-icons">{item.icon}</span>
            <span class="nav-label">{item.label}</span>
          </a>
        {/each}
      </div>

      <!-- ── Footer ── -->
      <div class="sidebar-footer">
        <div class="sidebar-divider"></div>
        <button
          class="nav-item theme-toggle-btn"
          onclick={() => theme.toggle()}
        >
          <span class="nav-hover-particles" aria-hidden="true"></span>
          <span class="nav-icon material-icons">
            {$theme === "dark" ? "light_mode" : "dark_mode"}
          </span>
          <span class="nav-label">
            {$theme === "dark" ? "Light Mode" : "Dark Mode"}
          </span>
        </button>
        <div class="version-chip">
          <span class="version-dot"></span>
          <span class="version-text">v3.0.0</span>
        </div>
      </div>
    </nav>

    <!-- Main Content Area -->
    <main class="main-viewport">
      <!-- Global Header (Glass) -->
      <header class="glass-header">
        <div class="header-left">
          <!-- Mobile hamburger menu -->
          <button
            class="mobile-menu-btn"
            onclick={openMobileDrawer}
            aria-label="Open menu"
          >
            <span class="material-icons">menu</span>
          </button>
          <div
            id="header-dynamic-content"
            style="flex: 1; margin-right: 2rem;"
          ></div>
        </div>

        <div class="header-right">
          <!-- Add Download Button -->
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

          <!-- System Health Status -->
          <div style="margin-right: 1.5rem;">
            <StatusChecker />
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
      <div
        class="view-container"
        data-view={viewName}
        bind:this={viewContainerEl}
      >
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

<!-- Account VIP Warning Modal -->
{#if showAccountWarn && !isAccountVip}
  <AccountWarningModal
    onDismiss={() => {
      showAccountWarn = false;
      sessionStorage.setItem("vip-warn-dismissed", "1");
    }}
  />
{/if}

<!-- Add Download Modal -->
<AddDownloadModal bind:isOpen={ui.addDownloadModalOpen} />

<!-- Smart Search Modal -->
<SmartSearchModal />

<!-- Smart Grab Modal -->
<SmartGrabModal />

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

  /* Add Download wrapper — just spacing */
  .add-dl-wrap {
    margin-right: 1.5rem;
    display: flex;
    align-items: center;
  }
  @media (max-width: 768px) {
    .add-dl-wrap {
      margin-right: 1rem;
    }
    .dl-btn-text {
      display: none;
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

  /* ============================================================
     SIDEBAR — Polygonal + Halftone Design System
     ============================================================ */

  :global(.glass-sidebar) {
    width: var(--sidebar-width, 280px);
    height: 100vh;
    display: flex;
    flex-direction: column;
    position: relative;
    z-index: 50;
    background: rgba(6, 8, 14, 0.92);
    backdrop-filter: blur(24px);
    -webkit-backdrop-filter: blur(24px);
    border-right: 1px solid rgba(255, 255, 255, 0.06);
    overflow: hidden;
    flex-shrink: 0;
    transition: width 0.35s cubic-bezier(0.4, 0, 0.2, 1);
  }

  /* Halftone ambient background on right side */
  :global(.glass-sidebar::before) {
    content: "";
    position: absolute;
    top: 0;
    right: 0;
    width: 180px;
    height: 100%;
    background-image: radial-gradient(
      circle,
      var(--color-primary, #00f3ff) 1px,
      transparent 1px
    );
    background-size: 8px 8px;
    opacity: 0.025;
    pointer-events: none;
    mask-image: linear-gradient(to left, black 0%, transparent 70%);
    -webkit-mask-image: linear-gradient(to left, black 0%, transparent 70%);
  }

  /* Right-edge glow line */
  :global(.glass-sidebar::after) {
    content: "";
    position: absolute;
    top: 8%;
    bottom: 8%;
    right: 0;
    width: 1px;
    background: linear-gradient(
      180deg,
      transparent 0%,
      var(--color-primary, #00f3ff) 25%,
      var(--color-primary, #00f3ff) 75%,
      transparent 100%
    );
    opacity: 0.18;
    filter: blur(0.5px);
  }

  :global(.glass-sidebar.collapsed) {
    width: var(--sidebar-collapsed-width, 72px);
  }

  /* ── Collapsed: center ALL icons symmetrically ── */
  :global(.glass-sidebar.collapsed .sidebar-brand) {
    justify-content: center;
    padding-left: 0 !important;
    padding-right: 0 !important;
  }

  /* brand-btn must not stretch so justify-content:center can center it */
  :global(.glass-sidebar.collapsed .brand-btn) {
    flex: 0 0 auto; /* shrink to logo size only */
    gap: 0;
    pointer-events: auto;
  }

  /* Fully remove brand-info from the layout — max-width:0 still occupies a flex slot */
  :global(.glass-sidebar.collapsed .brand-info) {
    display: none;
  }

  :global(.glass-sidebar.collapsed .nav-links) {
    padding-left: 0 !important;
    padding-right: 0 !important;
  }

  :global(.glass-sidebar.collapsed .sidebar-footer) {
    padding-left: 0 !important;
    padding-right: 0 !important;
  }

  :global(.glass-sidebar.collapsed .nav-item) {
    justify-content: center !important;
    padding-left: 0 !important;
    padding-right: 0 !important;
    gap: 0 !important; /* remove gap so label slot doesn't offset icon */
  }

  /* Fully remove nav-label from layout — opacity:0 still occupies flex space */
  :global(.glass-sidebar.collapsed .nav-label) {
    display: none;
  }

  :global(.glass-sidebar.collapsed .version-chip) {
    justify-content: center;
  }

  /* ── Brand Header ── */
  :global(.sidebar-brand) {
    display: flex;
    align-items: center;
    /* calc aligns logo center-x with nav icon center-x:
       nav icon center = nav-links-pad(0.6rem) + nav-item-pad(1rem) + half-icon(10px)
       logo center     = this-padding-left + half-logo(27px)
       => padding-left = 0.6rem + 1rem + 10px - 27px = calc(1.6rem - 17px) */
    padding: 1.1rem 0.75rem 0.9rem calc(1.6rem - 17px);
    gap: 0;
    flex-shrink: 0;
    min-height: 64px;
  }

  :global(.brand-btn) {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    flex: 1;
    background: none;
    border: none;
    padding: 0;
    min-width: 0;
    text-align: left;
    color: inherit;
    /* Cursor set via inline style reactively */
  }

  /* Logo — 54px (36 × 1.5). Container fixed so logo center-x aligns with nav icon center. */
  :global(.logo-container) {
    width: 54px;
    height: 54px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
  }

  :global(.brand-info) {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    overflow: hidden;
    transition:
      opacity 0.2s 0.05s,
      max-width 0.35s;
    max-width: 200px;
  }

  :global(.glass-sidebar.collapsed .brand-info) {
    opacity: 0;
    max-width: 0;
    pointer-events: none;
    transition:
      opacity 0.15s,
      max-width 0.35s;
  }

  :global(.brand-name) {
    font-family: var(--font-heading, "Outfit", sans-serif);
    font-size: 0.82rem;
    font-weight: 900;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    background: linear-gradient(
      135deg,
      #ffffff 0%,
      var(--color-primary, #00f3ff) 100%
    );
    -webkit-background-clip: text;
    background-clip: text;
    -webkit-text-fill-color: transparent;
    white-space: nowrap;
  }

  :global(.brand-tag) {
    font-family: var(--font-mono, monospace);
    font-size: 0.48rem;
    font-weight: 700;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: rgba(0, 243, 255, 0.38);
    white-space: nowrap;
  }

  /* Collapse button — hidden when collapsed (logo takes over) */
  :global(.collapse-btn) {
    width: 26px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 5px;
    color: var(--text-muted, #64748b);
    cursor: pointer;
    flex-shrink: 0;
    transition: all 0.2s;
    margin-left: auto;
  }

  :global(.collapse-btn:hover) {
    background: rgba(0, 243, 255, 0.08);
    border-color: rgba(0, 243, 255, 0.28);
    color: var(--color-primary, #00f3ff);
  }

  :global(.collapse-btn .material-icons) {
    font-size: 15px;
  }

  /* ── Divider ── */
  :global(.sidebar-divider) {
    height: 1px;
    margin: 0.35rem 0.85rem;
    background: linear-gradient(
      90deg,
      transparent 0%,
      rgba(255, 255, 255, 0.07) 25%,
      rgba(255, 255, 255, 0.07) 75%,
      transparent 100%
    );
    flex-shrink: 0;
  }

  /* ── Nav Links Container ── */
  :global(.nav-links) {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    padding: 0.4rem 0.6rem;
    overflow-y: auto;
    overflow-x: hidden;
  }

  /* ── Nav Item base: CLIPPED POLYGON ── */
  :global(.nav-item) {
    /* Layout */
    display: flex;
    align-items: center;
    gap: 0.9rem;
    padding: 0.72rem 1rem;
    /* Clipped polygon — matches the dashboard button style */
    clip-path: polygon(
      0% 0%,
      calc(100% - 10px) 0%,
      100% 10px,
      100% 100%,
      10px 100%,
      0% calc(100% - 10px)
    );
    border-radius: 0;
    /* Text */
    color: var(--text-secondary, #94a3b8);
    text-decoration: none;
    font-size: 0.78rem;
    font-weight: 600;
    letter-spacing: 0.035em;
    white-space: nowrap;
    overflow: visible; /* allow ::after pseudo particles to extend */
    /* State */
    position: relative;
    cursor: pointer;
    background: transparent;
    width: 100%;
    text-align: left;
    border: none;
    outline: none;
    /* Transitions */
    transition:
      color 0.25s ease,
      background 0.25s ease;
  }

  /* ── HOVER: cyan text + animated halftone particle stream ── */
  :global(.nav-item:hover:not(.active)) {
    color: var(--color-primary, #00f3ff);
    background: rgba(0, 243, 255, 0.04);
  }

  /* Particle layer (span.nav-hover-particles inside each nav-item) */
  :global(.nav-hover-particles) {
    position: absolute;
    inset: 0;
    pointer-events: none;
    overflow: hidden;
    clip-path: polygon(
      0% 0%,
      calc(100% - 10px) 0%,
      100% 10px,
      100% 100%,
      10px 100%,
      0% calc(100% - 10px)
    );
    /* Dots that drift rightward */
    background-image: radial-gradient(
        circle,
        var(--color-primary, #00f3ff) 1.2px,
        transparent 1.2px
      ),
      radial-gradient(
        circle,
        var(--color-primary, #00f3ff) 1px,
        transparent 1px
      ),
      radial-gradient(
        circle,
        var(--color-primary, #00f3ff) 0.8px,
        transparent 0.8px
      );
    background-size:
      10px 10px,
      14px 14px,
      18px 18px;
    background-position:
      0 0,
      4px 5px,
      9px 2px;
    opacity: 0;
    /* fade out toward right */
    mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 0.55) 0%,
      transparent 55%
    );
    -webkit-mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 0.55) 0%,
      transparent 55%
    );
    transition: opacity 0.3s ease;
  }

  /* Animate position on hover — dots scroll rightward */
  :global(.nav-item:hover .nav-hover-particles) {
    opacity: 1;
    animation: particles-drift 1.8s linear infinite;
  }

  @keyframes particles-drift {
    0% {
      background-position:
        0px 0px,
        4px 5px,
        9px 2px;
    }
    100% {
      background-position:
        20px 0px,
        24px 5px,
        29px 2px;
    }
  }

  /* ── ACTIVE: polygon + gradient fill + halftone bleed left→right ── */
  :global(.nav-item.active) {
    background: linear-gradient(
      90deg,
      rgba(0, 243, 255, 0.13) 0%,
      rgba(0, 243, 255, 0.05) 50%,
      transparent 100%
    );
    color: #ffffff;
  }

  /* Active: left glow accent bar (pseudo element, clipped by polygon so it appears as a tapered edge) */
  :global(.nav-item.active::before) {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: linear-gradient(
      180deg,
      var(--color-primary, #00f3ff) 0%,
      rgba(0, 243, 255, 0.4) 100%
    );
    box-shadow:
      0 0 10px var(--color-primary, #00f3ff),
      2px 0 12px rgba(0, 243, 255, 0.3);
    pointer-events: none;
  }

  /* Active: halftone dot bleed fading to the right (always visible) */
  :global(.nav-item.active::after) {
    content: "";
    position: absolute;
    inset: 0;
    background-image: radial-gradient(
      circle,
      rgba(0, 243, 255, 0.5) 1.2px,
      transparent 1.2px
    );
    background-size: 6px 6px;
    pointer-events: none;
    mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 0.5) 0%,
      transparent 50%
    );
    -webkit-mask-image: linear-gradient(
      to right,
      rgba(0, 0, 0, 0.5) 0%,
      transparent 50%
    );
    clip-path: polygon(
      0% 0%,
      calc(100% - 10px) 0%,
      100% 10px,
      100% 100%,
      10px 100%,
      0% calc(100% - 10px)
    );
  }

  /* Nav icon — always 20px width so logo aligns with icons in collapsed state */
  :global(.nav-icon) {
    font-size: 1.15rem;
    width: 20px;
    text-align: center;
    flex-shrink: 0;
    opacity: 0.65;
    transition:
      opacity 0.2s,
      color 0.2s;
    position: relative;
    z-index: 1;
  }

  :global(.nav-item.active .nav-icon),
  :global(.nav-item:hover .nav-icon) {
    opacity: 1;
    color: var(--color-primary, #00f3ff);
  }

  /* Nav text label */
  :global(.nav-label) {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    transition:
      opacity 0.2s 0.05s,
      max-width 0.35s;
    max-width: 200px;
    position: relative;
    z-index: 1;
  }

  :global(.glass-sidebar.collapsed .nav-label) {
    opacity: 0;
    max-width: 0;
    pointer-events: none;
    transition:
      opacity 0.12s,
      max-width 0.35s;
  }

  /* ── Footer ── */
  :global(.sidebar-footer) {
    flex-shrink: 0;
    padding: 0 0.6rem 0.85rem;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  :global(.theme-toggle-btn) {
    font-family: var(--font-body, "Inter", sans-serif);
  }

  :global(.version-chip) {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    padding: 0.5rem 0;
    font-family: var(--font-mono, monospace);
    font-size: 0.58rem;
    font-weight: 700;
    letter-spacing: 0.14em;
    color: rgba(255, 255, 255, 0.15);
    text-transform: uppercase;
    transition: opacity 0.2s;
  }

  :global(.glass-sidebar.collapsed .version-chip) {
    opacity: 0;
    pointer-events: none;
  }

  :global(.version-text) {
    transition: opacity 0.15s;
  }

  :global(.version-dot) {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--color-primary, #00f3ff);
    opacity: 0.35;
    flex-shrink: 0;
    animation: version-pulse 3s ease-in-out infinite;
  }

  @keyframes version-pulse {
    0%,
    100% {
      opacity: 0.2;
      box-shadow: none;
    }
    50% {
      opacity: 0.7;
      box-shadow: 0 0 6px var(--color-primary, #00f3ff);
    }
  }
  /* ============================================
     LOGO ANIMATION: Lightning Strike + Scan
     ============================================ */

  :global(.logo-container) {
    position: relative;
    width: 54px;
    height: 54px;
    overflow: hidden;
  }

  /* Initial reveal: Vertical scan line */
  :global(.logo-container::before) {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: linear-gradient(
      to bottom,
      transparent 0%,
      rgba(0, 243, 255, 0.6) 50%,
      transparent 100%
    );
    animation: vertical-scan 0.6s ease-out forwards;
    pointer-events: none;
    z-index: 2;
  }

  /* Horizontal light sweep */
  :global(.logo-container::after) {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: linear-gradient(
      to right,
      transparent 0%,
      rgba(0, 243, 255, 0.8) 50%,
      transparent 100%
    );
    animation: horizontal-sweep 0.5s 0.4s ease-out forwards;
    pointer-events: none;
    z-index: 2;
  }

  /* Logo image animations */
  :global(.logo-container .brand-logo-img) {
    animation:
      logo-flash 0.2s 0.8s ease-out,
      logo-breathe 3s 1.2s ease-in-out infinite;
    filter: drop-shadow(0 0 0px rgba(0, 243, 255, 0));
  }

  /* Vertical scan animation */
  @keyframes vertical-scan {
    0% {
      transform: translateY(-100%);
      opacity: 0;
    }
    50% {
      opacity: 1;
    }
    100% {
      transform: translateY(100%);
      opacity: 0;
    }
  }

  /* Horizontal sweep animation */
  @keyframes horizontal-sweep {
    0% {
      transform: translateX(-100%);
      opacity: 0;
    }
    50% {
      opacity: 1;
    }
    100% {
      transform: translateX(100%);
      opacity: 0;
    }
  }

  /* Lightning flash at intersection */
  @keyframes logo-flash {
    0% {
      filter: drop-shadow(0 0 0px rgba(0, 243, 255, 0)) brightness(1);
    }
    50% {
      filter: drop-shadow(0 0 20px rgba(0, 243, 255, 1)) brightness(1.5);
    }
    100% {
      filter: drop-shadow(0 0 8px rgba(0, 243, 255, 0.4)) brightness(1);
    }
  }

  /* Breathing glow (idle state) */
  @keyframes logo-breathe {
    0%,
    100% {
      filter: drop-shadow(0 0 8px rgba(0, 243, 255, 0.4));
      transform: scale(1);
    }
    50% {
      filter: drop-shadow(0 0 12px rgba(0, 243, 255, 0.6));
      transform: scale(1.02);
    }
  }

  /* Hover effect: Intensify glow */
  :global(.logo-container:hover .brand-logo-img) {
    animation: logo-breathe 1.5s ease-in-out infinite;
    filter: drop-shadow(0 0 15px rgba(0, 243, 255, 0.8)) !important;
  }

  /* ============================================
     PREMIUM INTRO ANIMATION STYLES
     ============================================ */

  .app-shell.hidden {
    opacity: 0;
    pointer-events: none;
  }
</style>
