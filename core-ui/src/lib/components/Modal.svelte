<script lang="ts" module>
  /*
   * Body scroll lock shared by every open Modal instance. Nested / stacked
   * modals increment the counter; the original overflow values are only
   * restored when the last one closes.
   */
  let openModalCount = 0;
  let prevHtmlOverflow = "";
  let prevBodyOverflow = "";
</script>

<script lang="ts">
  import { tick, type Snippet } from "svelte";

  interface Props {
    open: boolean;
    onClose: () => void;
    /** Panel max-width (any CSS length) */
    maxWidth?: string;
    /** Panel max-height (any CSS length) */
    maxHeight?: string;
    /** Accent color used for the header banner / border glow */
    accent?: string;
    ariaLabel?: string;
    /** Remove the default body padding */
    noPad?: boolean;
    header?: Snippet;
    children?: Snippet;
    footer?: Snippet;
  }

  let {
    open,
    onClose,
    maxWidth = "900px",
    maxHeight = "88vh",
    accent = "var(--color-primary, #00f3ff)",
    ariaLabel = "Dialog",
    noPad = false,
    header,
    children,
    footer,
  }: Props = $props();

  let panel = $state<HTMLElement>();
  let previouslyFocused: HTMLElement | null = null;

  function lockScroll() {
    if (openModalCount++ === 0) {
      prevHtmlOverflow = document.documentElement.style.overflow;
      prevBodyOverflow = document.body.style.overflow;
      document.documentElement.style.overflow = "hidden";
      document.body.style.overflow = "hidden";
      document.body.classList.add("cui-modal-open");
    }
  }

  function unlockScroll() {
    if (openModalCount && --openModalCount === 0) {
      document.documentElement.style.overflow = prevHtmlOverflow;
      document.body.style.overflow = prevBodyOverflow;
      document.body.classList.remove("cui-modal-open");
    }
  }

  const FOCUSABLE = [
    "a[href]",
    "button:not([disabled])",
    "input:not([disabled])",
    "select:not([disabled])",
    "textarea:not([disabled])",
    "[tabindex]:not([tabindex='-1'])",
  ].join(",");

  function close() {
    onClose();
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) close();
  }

  /** Escape closes; Tab / Shift+Tab is trapped inside the panel. */
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
      return;
    }
    if (e.key !== "Tab" || !panel) return;

    const focusable = [...panel.querySelectorAll<HTMLElement>(FOCUSABLE)];
    if (!focusable.length) {
      e.preventDefault();
      panel.focus();
      return;
    }

    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }

  $effect(() => {
    if (!open) return;

    previouslyFocused =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    lockScroll();
    tick().then(() => panel?.focus());

    return () => {
      unlockScroll();
      previouslyFocused?.focus();
    };
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_no_static_element_interactions (backdrop click / Escape close) -->
  <div
    class="modal-backdrop"
    style="--modal-accent: {accent}; --modal-max-width: {maxWidth}; --modal-max-height: {maxHeight};"
    onclick={handleBackdropClick}
    onkeydown={handleKeydown}
  >
    <div
      bind:this={panel}
      class="modal-panel"
      role="dialog"
      aria-modal="true"
      aria-label={ariaLabel}
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={handleKeydown}
    >
      {#if header}
        <div class="modal-header-banner">
          <div class="banner-dot-grid"></div>
          <div class="banner-vignette"></div>
          <div class="banner-glow"></div>
          <div class="modal-header-content">
            {@render header()}
          </div>
        </div>
      {/if}

      <div class="modal-body custom-scrollbar" class:no-pad={noPad}>
        {#if children}
          {@render children()}
        {/if}
      </div>

      {#if footer}
        <div class="modal-footer">
          {@render footer()}
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: color-mix(in srgb, var(--cui-surface-0) 82%, transparent);
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
    z-index: var(--cui-z-modal);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: max(var(--cui-space-lg), env(safe-area-inset-top, 0px))
      max(var(--cui-space-lg), env(safe-area-inset-right, 0px))
      max(var(--cui-space-lg), env(safe-area-inset-bottom, 0px))
      max(var(--cui-space-lg), env(safe-area-inset-left, 0px));
    animation: modal-fade-in var(--cui-duration-fast) var(--cui-ease-standard)
      both;
  }

  @keyframes modal-fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .modal-panel {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 100%;
    max-width: var(--modal-max-width, 900px);
    max-height: min(
      var(--modal-max-height, 88vh),
      calc(
        100dvh - (2 * var(--cui-space-lg)) - env(safe-area-inset-top, 0px) -
          env(safe-area-inset-bottom, 0px)
      )
    );
    background: linear-gradient(
      160deg,
      var(--cui-surface-3) 0%,
      var(--cui-surface-2) 60%,
      var(--cui-surface-1) 100%
    );
    border: 1px solid
      color-mix(
        in srgb,
        var(--modal-accent, var(--cui-primary)) 22%,
        var(--cui-glass-border)
      );
    clip-path: polygon(
      12px 0,
      100% 0,
      100% calc(100% - 12px),
      calc(100% - 12px) 100%,
      0 100%,
      0 12px
    );
    overflow: hidden;
    box-shadow:
      var(--cui-shadow-glass),
      0 0 40px
        color-mix(
          in srgb,
          var(--modal-accent, var(--cui-primary)) 12%,
          transparent
        );
    animation: modal-slide-up var(--cui-duration-base)
      var(--cui-ease-emphasized) both;
  }

  @keyframes modal-slide-up {
    from {
      opacity: 0;
      transform: translateY(18px) scale(0.97);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  /* ── Header banner ── */
  .modal-header-banner {
    position: relative;
    flex-shrink: 0;
    background: linear-gradient(
      160deg,
      color-mix(in srgb, var(--modal-accent, #00f3ff) 18%, #0a0c14) 0%,
      color-mix(in srgb, var(--modal-accent, #00f3ff) 10%, #090b12) 55%,
      #0a0c12 100%
    );
    border-bottom: 1px solid
      color-mix(
        in srgb,
        var(--modal-accent, #00f3ff) 18%,
        rgba(255, 255, 255, 0.06)
      );
    overflow: hidden;
  }

  .banner-dot-grid {
    position: absolute;
    inset: 0;
    background-image: radial-gradient(
      color-mix(in srgb, var(--modal-accent, #00f3ff) 7%, rgba(0, 0, 0, 0.6)) 1px,
      transparent 1px
    );
    background-size: 7px 7px;
    pointer-events: none;
    z-index: 0;
    opacity: 0.5;
  }

  .banner-vignette {
    position: absolute;
    inset: 0;
    background: radial-gradient(
      ellipse 70% 120% at 50% 0%,
      color-mix(in srgb, var(--modal-accent, #00f3ff) 8%, transparent) 0%,
      transparent 70%
    );
    pointer-events: none;
    z-index: 1;
  }

  .banner-glow {
    position: absolute;
    top: -20px;
    left: 50%;
    transform: translateX(-50%);
    width: 60%;
    height: 60px;
    background: var(--modal-accent, #00f3ff);
    filter: blur(32px);
    opacity: 0.12;
    pointer-events: none;
    z-index: 1;
  }

  /* accent hairline along the top edge */
  .modal-header-banner::before {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 2px;
    background: linear-gradient(
      90deg,
      transparent 0%,
      var(--modal-accent, #00f3ff) 25%,
      color-mix(in srgb, var(--modal-accent, #00f3ff) 70%, #fff) 50%,
      var(--modal-accent, #00f3ff) 75%,
      transparent 100%
    );
    z-index: 3;
  }

  .modal-header-content {
    position: relative;
    z-index: 2;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 1.1rem 1.75rem 1rem;
  }

  /* ── Body / footer ── */
  .modal-body {
    flex: 1;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 1.5rem 1.75rem;
    min-height: 0;
  }

  .modal-body.no-pad {
    padding: 0;
  }

  .modal-footer {
    padding: 1rem 1.75rem 1.25rem;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    flex-shrink: 0;
  }

  /* Close button rendered by consumers inside the header snippet */
  :global(.modal-header-content .close-btn) {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--cui-touch-min);
    height: var(--cui-touch-min);
    border-radius: var(--cui-radius-full);
    background: var(--cui-surface-hover);
    border: 1px solid var(--cui-glass-border);
    color: var(--cui-text-secondary);
    cursor: pointer;
    transition:
      background var(--cui-duration-fast) var(--cui-ease-standard),
      color var(--cui-duration-fast) var(--cui-ease-standard),
      border-color var(--cui-duration-fast) var(--cui-ease-standard);
    padding: 0;
  }

  :global(.modal-header-content .close-btn:hover) {
    background: var(--cui-surface-active);
    color: var(--cui-text-primary);
    border-color: var(--cui-glass-border-active);
  }

  :global(.modal-header-content .close-btn .material-icons) {
    font-size: 1.1rem;
    line-height: 1;
  }

  /* ── Mobile ── */
  @media (max-width: 767px) {
    .modal-backdrop {
      padding: max(1rem, env(safe-area-inset-top, 0px))
        max(1rem, env(safe-area-inset-right, 0px))
        max(1rem, env(safe-area-inset-bottom, 0px))
        max(1rem, env(safe-area-inset-left, 0px));
      align-items: center;
    }

    .modal-panel {
      max-height: calc(
        100dvh - 2rem - env(safe-area-inset-top, 0px) -
          env(safe-area-inset-bottom, 0px)
      );
      width: 100%;
    }

    .modal-header-content {
      padding: 1rem 1.25rem 0.9rem;
    }

    .modal-body {
      padding: 1.25rem;
    }

    .modal-footer {
      padding: 0.85rem 1.25rem 1rem;
    }

    .modal-footer :global(*) {
      flex-wrap: wrap;
    }

    .modal-footer :global(button) {
      min-height: var(--cui-touch-min);
    }
  }

  /* ── Scrollbar ── */
  .custom-scrollbar::-webkit-scrollbar {
    width: 5px;
  }

  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }

  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 8px;
  }

  .custom-scrollbar::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.18);
  }
</style>
