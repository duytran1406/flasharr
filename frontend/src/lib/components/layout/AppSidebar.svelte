<script lang="ts">
  /**
   * AppSidebar — glass navigation sidebar (desktop) / slide-in drawer (mobile).
   * State lives in the root layout; this component only renders it.
   */

  export interface NavItem {
    href: string;
    label: string;
    icon: string;
    route: string;
  }

  interface Props {
    navItems: NavItem[];
    currentRoute?: string;
    sidebarOpen: boolean;
    mobileDrawer: boolean;
    sidebarCollapsed: boolean;
    isActive: (href: string) => boolean;
    toggleSidebar: () => void;
    closeMobileDrawer: () => void;
  }

  let {
    navItems,
    sidebarOpen,
    mobileDrawer,
    sidebarCollapsed,
    isActive,
    toggleSidebar,
    closeMobileDrawer,
  }: Props = $props();
</script>

<nav
  class="glass-sidebar"
  class:show={sidebarOpen}
  class:collapsed={sidebarCollapsed}
  aria-label="Application drawer"
  aria-modal={mobileDrawer && sidebarOpen ? "true" : undefined}
  role={mobileDrawer && sidebarOpen ? "dialog" : undefined}
  inert={mobileDrawer && !sidebarOpen}
>
  <div class="sidebar-brand">
    {#if sidebarOpen}
      <button
        class="drawer-close"
        type="button"
        aria-label="Close navigation menu"
        onclick={closeMobileDrawer}
      >
        <span class="material-icons" aria-hidden="true">close</span>
      </button>
    {/if}
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

  <div class="sidebar-divider"></div>

  <div class="nav-links">
    {#each navItems as item}
      <a
        href={item.href}
        class="nav-item"
        class:active={isActive(item.href)}
        data-route={item.route}
        title={sidebarCollapsed ? item.label : ""}
        onclick={closeMobileDrawer}
      >
        <span class="nav-hover-particles" aria-hidden="true"></span>
        <span class="nav-icon material-icons">{item.icon}</span>
        <span class="nav-label">{item.label}</span>
      </a>
    {/each}
  </div>
</nav>

<style>
  .glass-sidebar {
    width: var(--cui-sidebar-width);
    height: 100dvh;
    display: flex;
    flex-direction: column;
    position: relative;
    z-index: 50;
    background: var(--cui-glass-heavy);
    backdrop-filter: blur(24px);
    -webkit-backdrop-filter: blur(24px);
    border-right: 1px solid var(--cui-glass-border);
    overflow: hidden;
    flex-shrink: 0;
    transition: width var(--cui-duration-slow) var(--cui-ease-standard);
  }

  .glass-sidebar:before {
    content: "";
    position: absolute;
    top: 0;
    right: 0;
    width: 180px;
    height: 100%;
    background-image: radial-gradient(circle,var(--cui-primary) 1px,transparent 1px);
    background-size: 8px 8px;
    opacity: .025;
    pointer-events: none;
    mask-image: linear-gradient(to left,black 0%,transparent 70%);
    -webkit-mask-image: linear-gradient(to left,black 0%,transparent 70%);
  }

  .glass-sidebar:after {
    content: "";
    position: absolute;
    top: 8%;
    bottom: 8%;
    right: 0;
    width: 1px;
    background: linear-gradient(180deg,transparent 0%,var(--cui-primary) 25%,var(--cui-primary) 75%,transparent 100%);
    opacity: .18;
    filter: blur(.5px);
  }

  .glass-sidebar.collapsed {
    width: var(--cui-sidebar-collapsed-width);
  }

  .glass-sidebar.collapsed .sidebar-brand {
    justify-content: center;
    padding-left: 0 !important;
    padding-right: 0 !important;
  }

  .glass-sidebar.collapsed .brand-btn {
    flex: 0 0 auto;
    gap: 0;
    pointer-events: auto;
  }

  .glass-sidebar.collapsed .brand-info, .glass-sidebar.collapsed .nav-label {
    display: none;
  }

  .glass-sidebar.collapsed .nav-links {
    padding-left: 0 !important;
    padding-right: 0 !important;
  }

  .glass-sidebar.collapsed .nav-item {
    justify-content: center !important;
    padding-left: 0 !important;
    padding-right: 0 !important;
    gap: 0 !important;
  }

  .sidebar-brand {
    display: flex;
    align-items: center;
    padding: 1.1rem .75rem .9rem calc(1.6rem - 17px);
    gap: 0;
    flex-shrink: 0;
    min-height: 64px;
  }

  .drawer-close {
    display: none;
    width: 44px;
    height: 44px;
    place-items: center;
    flex: 0 0 44px;
    padding: 0;
    border: 1px solid var(--cui-glass-border);
    border-radius: 6px;
    background: var(--cui-surface-hover);
    color: var(--cui-primary);
    cursor: pointer;
  }

  .drawer-close:focus-visible {
    outline: 2px solid var(--cui-primary);
    outline-offset: 2px;
  }

  .brand-btn {
    display: flex;
    align-items: center;
    gap: .85rem;
    flex: 1;
    background: none;
    border: none;
    padding: 0;
    min-width: 0;
    text-align: left;
    color: inherit;
  }

  .logo-container {
    position: relative;
    width: 54px;
    height: 54px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }

  .logo-container:before {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: linear-gradient(to bottom,transparent 0%,rgba(0,243,255,.6) 50%,transparent 100%);
    animation: vertical-scan .6s ease-out forwards;
    pointer-events: none;
    z-index: 2;
  }

  .logo-container:after {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: linear-gradient(to right,transparent 0%,rgba(0,243,255,.8) 50%,transparent 100%);
    animation: horizontal-sweep .5s .4s ease-out forwards;
    pointer-events: none;
    z-index: 2;
  }

  .brand-logo-img {
    animation: logo-flash .2s .8s ease-out,logo-breathe 3s 1.2s ease-in-out infinite;
    filter: drop-shadow(0 0 0 rgba(0,243,255,0));
  }

  .brand-info {
    display: flex;
    flex-direction: column;
    gap: .1rem;
    overflow: hidden;
    transition: opacity .2s .05s,max-width .35s;
    max-width: 200px;
  }

  .brand-name {
    font-family: var(--cui-font-heading);
    font-size: .82rem;
    font-weight: 900;
    letter-spacing: .2em;
    text-transform: uppercase;
    background: linear-gradient(135deg,#fff 0%,var(--cui-primary) 100%);
    -webkit-background-clip: text;
    background-clip: text;
    -webkit-text-fill-color: transparent;
    white-space: nowrap;
  }

  .brand-tag {
    font-family: var(--cui-font-mono);
    font-size: .48rem;
    font-weight: 700;
    letter-spacing: .18em;
    text-transform: uppercase;
    color: #00f3ff61;
    white-space: nowrap;
  }

  .collapse-btn {
    width: 26px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--cui-surface-hover);
    border: 1px solid rgba(255,255,255,.07);
    border-radius: 5px;
    color: var(--cui-text-tertiary);
    cursor: pointer;
    flex-shrink: 0;
    transition: all .2s;
    margin-left: auto;
  }

  .collapse-btn:hover {
    background: var(--cui-surface-selected);
    border-color: #00f3ff47;
    color: var(--cui-primary);
  }

  .collapse-btn .material-icons {
    font-size: 15px;
  }

  .sidebar-divider {
    height: 1px;
    margin: .35rem .85rem;
    background: linear-gradient(90deg,transparent 0%,rgba(255,255,255,.07) 25%,rgba(255,255,255,.07) 75%,transparent 100%);
    flex-shrink: 0;
  }

  .nav-links {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: .15rem;
    padding: .4rem .6rem;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: .9rem;
    padding: .72rem 1rem;
    clip-path: polygon(0% 0%,calc(100% - 10px) 0%,100% 10px,100% 100%,10px 100%,0% calc(100% - 10px));
    color: var(--cui-text-secondary);
    text-decoration: none;
    font-size: .78rem;
    font-weight: 600;
    letter-spacing: .035em;
    white-space: nowrap;
    overflow: visible;
    position: relative;
    cursor: pointer;
    background: transparent;
    width: 100%;
    text-align: left;
    border: none;
    outline: none;
    transition: color .25s ease,background .25s ease;
  }

  .nav-item:hover:not(.active) {
    color: var(--cui-primary);
    background: var(--cui-surface-focus);
  }

  .nav-hover-particles {
    position: absolute;
    inset: 0;
    pointer-events: none;
    overflow: hidden;
    clip-path: polygon(0% 0%,calc(100% - 10px) 0%,100% 10px,100% 100%,10px 100%,0% calc(100% - 10px));
    background-image: radial-gradient(circle,var(--cui-primary) 1.2px,transparent 1.2px),radial-gradient(circle,var(--cui-primary) 1px,transparent 1px),radial-gradient(circle,var(--cui-primary) .8px,transparent .8px);
    background-size: 10px 10px,14px 14px,18px 18px;
    background-position: 0 0,4px 5px,9px 2px;
    opacity: 0;
    mask-image: linear-gradient(to right,rgba(0,0,0,.55) 0%,transparent 55%);
    -webkit-mask-image: linear-gradient(to right,rgba(0,0,0,.55) 0%,transparent 55%);
    transition: opacity .3s ease;
  }

  .nav-item:hover .nav-hover-particles {
    opacity: 1;
    animation: particles-drift 1.8s linear infinite;
  }

  .nav-item.active {
    background: linear-gradient(90deg,rgba(0,243,255,.13) 0%,rgba(0,243,255,.05) 50%,transparent 100%);
    color: var(--cui-text-primary);
  }

  .nav-item.active:before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: linear-gradient(180deg,var(--cui-primary) 0%,rgba(0,243,255,.4) 100%);
    box-shadow: 0 0 10px var(--cui-primary),2px 0 12px #00f3ff4d;
    pointer-events: none;
  }

  .nav-item.active:after {
    content: "";
    position: absolute;
    inset: 0;
    background-image: radial-gradient(circle,rgba(0,243,255,.5) 1.2px,transparent 1.2px);
    background-size: 6px 6px;
    pointer-events: none;
    mask-image: linear-gradient(to right,rgba(0,0,0,.5) 0%,transparent 50%);
    -webkit-mask-image: linear-gradient(to right,rgba(0,0,0,.5) 0%,transparent 50%);
    clip-path: polygon(0% 0%,calc(100% - 10px) 0%,100% 10px,100% 100%,10px 100%,0% calc(100% - 10px));
  }

  .nav-icon {
    font-size: 1.15rem;
    width: 20px;
    text-align: center;
    flex-shrink: 0;
    opacity: .65;
    transition: opacity .2s,color .2s;
    position: relative;
    z-index: 1;
  }

  .nav-item.active .nav-icon, .nav-item:hover .nav-icon {
    opacity: 1;
    color: var(--cui-primary);
  }

  .nav-label {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    transition: opacity .2s .05s,max-width .35s;
    max-width: 200px;
    position: relative;
    z-index: 1;
  }

  @keyframes particles-drift {
    0% {
      background-position: 0 0,4px 5px,9px 2px;
    }

    to {
      background-position: 20px 0,24px 5px,29px 2px;
    }

  }

  @keyframes vertical-scan {
    0% {
      transform: translateY(-100%);
      opacity: 0;
    }

    50% {
      opacity: 1;
    }

    to {
      transform: translateY(100%);
      opacity: 0;
    }

  }

  @keyframes horizontal-sweep {
    0% {
      transform: translate(-100%);
      opacity: 0;
    }

    50% {
      opacity: 1;
    }

    to {
      transform: translate(100%);
      opacity: 0;
    }

  }

  @keyframes logo-flash {
    0% {
      filter: drop-shadow(0 0 0 rgba(0,243,255,0)) brightness(1);
    }

    50% {
      filter: drop-shadow(0 0 20px rgba(0,243,255,1)) brightness(1.5);
    }

    to {
      filter: drop-shadow(0 0 8px rgba(0,243,255,.4)) brightness(1);
    }

  }

  @keyframes logo-breathe {
    0%,to {
      filter: drop-shadow(0 0 8px rgba(0,243,255,.4));
      transform: scale(1);
    }

    50% {
      filter: drop-shadow(0 0 12px rgba(0,243,255,.6));
      transform: scale(1.02);
    }

  }

  .logo-container:hover .brand-logo-img {
    animation: logo-breathe 1.5s ease-in-out infinite;
    filter: drop-shadow(0 0 15px rgba(0,243,255,.8)) !important;
  }

  @media (max-width:767px),(pointer:coarse) and (max-height:500px) {
    .glass-sidebar {
      display: flex !important;
      position: fixed !important;
      top: 0;
      left: 0;
      bottom: 0;
      width: min(82vw,320px) !important;
      max-width: 320px;
      height: 100vh;
      height: 100dvh;
      padding-top: env(safe-area-inset-top,0px);
      padding-bottom: env(safe-area-inset-bottom,0px);
      transform: translate(-100%);
      transition: transform .25s ease;
      z-index: 1000;
    }

    .glass-sidebar.show {
      transform: translate(0);
    }

    .glass-sidebar.show .drawer-close {
      display: grid;
    }

    .glass-sidebar.show .collapse-btn {
      display: none;
    }

    .glass-sidebar.show .sidebar-brand {
      gap: .5rem;
      padding-left: .75rem;
    }

  }

  @media (min-width:768px) and (min-height:501px) {
    .glass-sidebar.show {
      transform: none;
    }

  }

  @media (prefers-reduced-motion:reduce) {
    .glass-sidebar, .brand-logo-img, .logo-container:before, .logo-container:after, .nav-hover-particles {
      animation: none !important;
      transition: none !important;
    }

  }

</style>
