<script lang="ts">
  import { page } from "$app/stores";

  /**
   * MobileNav — fixed bottom navigation bar (mobile only).
   * Visible exclusively on < 768px viewports via CSS.
   * Tabs: Discover, Downloads, Library, Settings.
   * Uses core-ui design tokens (--color-primary, --bg-glass, etc.)
   */

  const tabs = [
    { href: "/discover",   label: "Discover",   icon: "movie_filter"    },
    { href: "/downloads",  label: "Downloads",  icon: "cloud_download"  },
    { href: "/library",    label: "Library",    icon: "video_library"   },
    { href: "/settings",   label: "Settings",   icon: "settings_suggest"},
  ] as const;

  let currentPath = $derived($page.url.pathname);

  function isActive(href: string): boolean {
    if (href === "/") return currentPath === "/";
    return currentPath.startsWith(href);
  }
</script>

<nav class="mobile-bottom-nav" aria-label="Mobile navigation">
  {#each tabs as tab}
    <a
      href={tab.href}
      class="mobile-nav-tab"
      class:active={isActive(tab.href)}
      aria-current={isActive(tab.href) ? "page" : undefined}
    >
      <!-- Active indicator bar -->
      <span class="tab-indicator" aria-hidden="true"></span>
      <!-- Icon -->
      <span class="material-icons tab-icon" aria-hidden="true">{tab.icon}</span>
      <!-- Label -->
      <span class="tab-label">{tab.label}</span>
    </a>
  {/each}
</nav>

<style>
  /* ── Container ─────────────────────────────────────────────── */
  .mobile-bottom-nav {
    display: none; /* hidden on desktop — shown via media query below */
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    z-index: var(--z-mobile-bottom-nav, 900);

    /* Height: 60px content + iOS safe-area inset */
    height: calc(60px + env(safe-area-inset-bottom, 0px));
    padding-bottom: env(safe-area-inset-bottom, 0px);

    /* Glassmorphism background */
    background: rgba(6, 8, 14, 0.93);
    backdrop-filter: blur(28px) saturate(180%);
    -webkit-backdrop-filter: blur(28px) saturate(180%);
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    box-shadow:
      0 -8px 32px rgba(0, 0, 0, 0.5),
      inset 0 1px 0 rgba(255, 255, 255, 0.04);

    /* Horizontal layout of tabs */
    flex-direction: row;
    align-items: stretch;
  }

  /* ── Show on mobile ───────────────────────────────────────── */
  @media (max-width: 767px) {
    .mobile-bottom-nav {
      display: flex;
    }
  }

  /* ── Tab ──────────────────────────────────────────────────── */
  .mobile-nav-tab {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;

    /* min 48px touch target — height is inherited from nav (60px) */
    min-height: var(--touch-target-min, 48px);
    padding: 0 8px;

    color: rgba(255, 255, 255, 0.38);
    text-decoration: none;
    position: relative;
    transition: color var(--duration-fast, 150ms) var(--ease-out, ease);

    /* focus ring using core-ui primary */
    outline: none;
  }

  .mobile-nav-tab:focus-visible {
    box-shadow:
      inset 0 0 0 2px var(--color-primary, #00f3ff),
      0 0 12px rgba(0, 243, 255, 0.25);
    border-radius: 8px;
    color: var(--color-primary, #00f3ff);
  }

  /* ── Active state ─────────────────────────────────────────── */
  .mobile-nav-tab.active {
    color: var(--color-primary, #00f3ff);
  }

  /* Top accent bar on active tab */
  .tab-indicator {
    position: absolute;
    top: 0;
    left: 20%;
    right: 20%;
    height: 2px;
    background: transparent;
    border-radius: 0 0 3px 3px;
    transition:
      background var(--duration-fast, 150ms),
      box-shadow var(--duration-fast, 150ms);
  }

  .mobile-nav-tab.active .tab-indicator {
    background: var(--color-primary, #00f3ff);
    box-shadow: 0 0 10px var(--color-primary, #00f3ff);
  }

  /* ── Icon ─────────────────────────────────────────────────── */
  .tab-icon {
    font-size: 22px;
    line-height: 1;
    transition: transform var(--duration-fast, 150ms) var(--ease-out, ease);
  }

  .mobile-nav-tab.active .tab-icon {
    transform: translateY(-1px);
  }

  /* ── Label ────────────────────────────────────────────────── */
  .tab-label {
    font-family: var(--font-body, "Inter", sans-serif);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    line-height: 1;
  }

  /* ── Hover (non-active) ───────────────────────────────────── */
  .mobile-nav-tab:not(.active):hover {
    color: rgba(255, 255, 255, 0.65);
  }

  /* Active tab: subtle radial glow bleed behind icon */
  .mobile-nav-tab.active::after {
    content: "";
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -55%);
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: radial-gradient(
      circle,
      rgba(0, 243, 255, 0.12) 0%,
      transparent 70%
    );
    pointer-events: none;
  }
</style>
