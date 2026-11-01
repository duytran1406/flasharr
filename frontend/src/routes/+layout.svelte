<script lang="ts">
  import "../app.css";
  import type { Snippet } from "svelte";
  import { page } from "$app/stores";
  import { beforeNavigate, afterNavigate, goto } from "$app/navigation";
  import { onMount, onDestroy } from "svelte";
  import { animate, animeStagger } from "$lib/animations";
  import { wsClient } from "$lib/stores/websocket";
  import {
    downloadStore,
    engineStats,
    formatSpeed,
  } from "$lib/stores/downloads.svelte";
  import { qbitStore } from "$lib/stores/qbit.svelte";
  import { systemStore } from "$lib/stores/system";
  import { theme } from "$lib/stores/theme";
  import { ui } from "$lib/stores/ui.svelte";
  import { settingsStore } from "$lib/stores/settings";
  import { accountStore } from "$lib/stores/account.svelte";
  import { authStore } from "$lib/stores/auth.svelte";
  import IntroOverlay from "$lib/components/ui/IntroOverlay.svelte";
  import AuthGate from "$lib/components/AuthGate.svelte";
  import AppSidebar from "$lib/components/layout/AppSidebar.svelte";
  import AppViewport from "$lib/components/layout/AppViewport.svelte";
  import MobileNav from "$lib/components/MobileNav.svelte";
  import AccountWarningModal from "$lib/components/AccountWarningModal.svelte";
  import AddDownloadModal from "$lib/components/AddDownloadModal.svelte";
  import SmartSearchModal from "$lib/components/SmartSearchModal.svelte";
  import SmartGrabModal from "$lib/components/SmartGrabModal.svelte";
  import Toasts from "$lib/components/Toasts.svelte";

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();

  let sidebarOpen = $state(false);
  let sidebarCollapsed = $state(false);
  let viewContainerEl: HTMLDivElement | undefined = $state();

  // Account VIP warning modal
  let showAccountWarn = $state(false);
  let isAccountVip = $derived(accountStore.isVip);

  let showIntro = $derived(ui.showIntro);

  let stats = $derived($engineStats);
  let globalSpeed = $derived(formatSpeed(stats.total_speed));
  let activeCount = $derived(stats.active_downloads);

  let currentRoute = $derived($page.url.pathname);
  let isAuthenticated = $derived(!!authStore.session?.authenticated);

  /** Element focused before the mobile drawer opened (restored on close). */
  let drawerReturnFocus: HTMLElement | null = null;
  let mobileDrawer = $state(false);

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
      href: "/torrents",
      label: "Torrents",
      icon: "downloading",
      route: "torrents",
    },
    {
      href: "/settings",
      label: "Settings",
      icon: "settings_suggest",
      route: "settings",
    },
  ];

  // Compute data-view from route for CSS targeting
  function getViewName(path: string): string {
    if (path === "/") return "dashboard";
    if (path.startsWith("/tv/")) return "media/tv";
    if (path.startsWith("/movie/")) return "media/movie";
    if (path.startsWith("/collection/"))
      return "collection/" + path.split("/")[2];
    return path.split("/")[1] || "dashboard";
  }
  let viewName = $derived(getViewName(currentRoute));

  function isActive(href: string): boolean {
    if (href === "/") return currentRoute === "/";
    return currentRoute.startsWith(href);
  }

  function toggleSidebar() {
    sidebarCollapsed = !sidebarCollapsed;
    document.body.classList.toggle("sidebar-collapsed", sidebarCollapsed);
  }

  function closeMobileDrawer() {
    sidebarOpen = false;
    queueMicrotask(() => drawerReturnFocus?.focus());
  }

  function openMobileDrawer() {
    drawerReturnFocus = document.activeElement as HTMLElement | null;
    sidebarOpen = true;
    queueMicrotask(() =>
      document
        .querySelector<HTMLElement>(".glass-sidebar.show .drawer-close")
        ?.focus(),
    );
  }

  function syncMobileDrawer() {
    // Same breakpoint as the CSS: phones and short landscape touch screens
    mobileDrawer = matchMedia(
      "(max-width: 767px), (pointer: coarse) and (max-height: 500px)",
    ).matches;
    if (!mobileDrawer) sidebarOpen = false;
  }

  /** IntroOverlay asks where to go once the intro has played. */
  async function resolveStartRoute(): Promise<string> {
    if (currentRoute.startsWith("/setup")) return "/setup";
    return (await settingsStore.hasAccounts()) ? "/" : "/setup";
  }

  /** Runs when the intro finishes: kick off background initialisation. */
  async function handleIntroComplete(destination: string) {
    theme.init();

    if (destination === "/setup") {
      goto("/setup");
      return;
    }

    downloadStore.initWebSocket();
    qbitStore.initWebSocket();
    systemStore.initIntegrationStatusSync();
    wsClient.connect();

    try {
      await systemStore.refreshIntegrationState();
    } catch (_) {}
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
  }

  // Animate page content on route change
  afterNavigate(() => {
    const wrapper = viewContainerEl?.querySelector(".transition-wrapper");
    if (wrapper) {
      animate(wrapper, {
        opacity: [0, 1],
        translateY: [12, 0],
        duration: 350,
        ease: "outQuad",
      });
    }
  });

  onMount(() => {
    authStore.refresh();
    ui.startIntroSequence();

    const drawerQuery = matchMedia(
      "(max-width: 767px), (pointer: coarse) and (max-height: 500px)",
    );
    const onDrawerQueryChange = () => syncMobileDrawer();
    onDrawerQueryChange();
    drawerQuery.addEventListener("change", onDrawerQueryChange);

    // Animate sidebar nav items with stagger
    requestAnimationFrame(() => {
      const navLinks = document.querySelectorAll<HTMLElement>(
        ".nav-links .nav-item",
      );
      if (navLinks.length) {
        navLinks.forEach((el) => (el.style.opacity = "0"));
        animate(Array.from(navLinks), {
          opacity: [0, 1],
          translateX: [-20, 0],
          duration: 400,
          delay: animeStagger(50, { start: 300 }),
          ease: "outCubic",
        });
      }
    });

    function handleKeydown(e: KeyboardEvent) {
      // Mobile drawer: Escape closes, Tab is trapped inside the drawer
      if (sidebarOpen && e.key === "Escape") {
        e.preventDefault();
        closeMobileDrawer();
        return;
      }
      if (sidebarOpen && e.key === "Tab") {
        const focusable = [
          ...document.querySelectorAll<HTMLElement>(
            '.glass-sidebar.show a[href]:not([tabindex="-1"]), .glass-sidebar.show button:not([disabled]):not([tabindex="-1"])',
          ),
        ].filter((el) => el.getClientRects().length > 0);
        if (!focusable.length) return;
        const first = focusable[0];
        const last = focusable.at(-1)!;
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }

      // Add Download modal shortcut (Ctrl+N / Cmd+N)
      if ((e.ctrlKey || e.metaKey) && e.key === "n") {
        e.preventDefault();
        ui.openAddDownload();
      }
    }
    window.addEventListener("keydown", handleKeydown);

    return () => {
      window.removeEventListener("keydown", handleKeydown);
      drawerQuery.removeEventListener("change", onDrawerQueryChange);
    };
  });

  // Clear the dynamic header on route change to prevent "ghost" headers
  beforeNavigate(({ to }) => {
    if (to?.url.pathname === currentRoute) return;
    const headerContainer = document.getElementById("header-dynamic-content");
    if (headerContainer) headerContainer.innerHTML = "";
  });

  onDestroy(() => wsClient.disconnect());
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
  <title>Flasharr</title>
</svelte:head>

{#if authStore.loading}
  <div class="auth-loading">Loading Flasharr…</div>
{:else if !isAuthenticated}
  <AuthGate />
{:else}
  <!-- Global intro animation during initial loading -->
  {#if showIntro}
    <IntroOverlay
      checkConfig={resolveStartRoute}
      onComplete={handleIntroComplete}
    />
  {/if}

  <div class="app-shell" class:hidden={showIntro}>
    {#if !currentRoute.startsWith("/setup")}
      <AppSidebar
        {navItems}
        {currentRoute}
        {sidebarOpen}
        {mobileDrawer}
        {sidebarCollapsed}
        {isActive}
        {toggleSidebar}
        {closeMobileDrawer}
      />
      <AppViewport
        {children}
        {currentRoute}
        {viewName}
        {globalSpeed}
        {activeCount}
        {openMobileDrawer}
        inert={sidebarOpen}
        bind:viewContainerEl
      />
    {:else}
      <!-- Setup route renders without the app shell -->
      {@render children()}
    {/if}
  </div>

  <!-- Mobile drawer backdrop -->
  {#if !currentRoute.startsWith("/setup") && sidebarOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div
      class="mobile-drawer-overlay active"
      onclick={closeMobileDrawer}
      aria-hidden="true"
    ></div>
  {/if}

  {#if !currentRoute.startsWith("/setup")}
    <MobileNav inert={sidebarOpen} onMore={openMobileDrawer} />
  {/if}

  {#if showAccountWarn && !isAccountVip}
    <AccountWarningModal
      onDismiss={() => {
        showAccountWarn = false;
        sessionStorage.setItem("vip-warn-dismissed", "1");
      }}
    />
  {/if}

  <AddDownloadModal bind:isOpen={ui.addDownloadModalOpen} />
  <SmartSearchModal />
  <SmartGrabModal />
  <Toasts />
{/if}

<style>
  .auth-loading {
    min-height: 100vh;
    min-height: 100dvh;
    display: grid;
    place-items: center;
    color: var(--cui-text-secondary);
    background: var(--cui-surface-0,#071018);
  }

  .app-shell.hidden {
    opacity: 0;
    pointer-events: none;
  }

  .mobile-drawer-overlay {
    position: fixed;
    inset: 0;
    background: #00000080;
    z-index: var(--z-mobile-overlay,999);
    display: none;
    border: none;
    cursor: pointer;
  }

  .mobile-drawer-overlay.active {
    display: block;
  }

  @media (min-width:768px) and (min-height:501px) {
    .mobile-drawer-overlay {
      display: none !important;
    }

  }

</style>
