<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    /** Controls visibility — caller manages this */
    open: boolean;
    /** Called when backdrop/Escape pressed */
    onClose: () => void;
    /** Max width of the panel (CSS value, default 900px) */
    maxWidth?: string;
    /** Max height of the panel (CSS value, default 88vh) */
    maxHeight?: string;
    /** Accent color CSS value — drives the banner gradient & glow */
    accent?: string;
    /** aria-label for the panel */
    ariaLabel?: string;
    /** If true, the body area will NOT have internal padding (for full-bleed content) */
    noPad?: boolean;
    /** Header slot content (rendered inside the dots-banner) */
    header?: Snippet;
    /** Body slot content */
    children?: Snippet;
    /** Footer slot content (optional) */
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

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <!-- Backdrop -->
  <div
    class="modal-backdrop"
    onclick={handleBackdropClick}
    onkeydown={handleKeydown}
    role="dialog"
    aria-modal="true"
    aria-label={ariaLabel}
    tabindex="-1"
    style="--modal-accent: {accent}; --modal-max-width: {maxWidth}; --modal-max-height: {maxHeight};"
  >
    <!-- Panel -->
    <div
      class="modal-panel"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
      role="document"
    >
      <!-- ── Gradient Dots Banner Header ── -->
      {#if header}
        <div class="modal-header-banner">
          <!-- Dot grid texture -->
          <div class="banner-dot-grid"></div>
          <!-- Radial vignette — fades dots at center and near edges -->
          <div class="banner-vignette"></div>
          <!-- Accent ambient glow blob -->
          <div class="banner-glow"></div>
          <!-- Actual header content -->
          <div class="modal-header-content">
            {@render header()}
          </div>
        </div>
      {/if}

      <!-- Body -->
      <div class="modal-body custom-scrollbar" class:no-pad={noPad}>
        {#if children}
          {@render children()}
        {/if}
      </div>

      <!-- Footer -->
      {#if footer}
        <div class="modal-footer">
          {@render footer()}
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  /* ── Backdrop ── */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.82);
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
    z-index: 9000;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1.5rem;
    animation: modal-fade-in 0.18s ease both;
  }

  @keyframes modal-fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  /* ── Panel ── */
  .modal-panel {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 100%;
    max-width: var(--modal-max-width, 900px);
    max-height: var(--modal-max-height, 88vh);
    background: linear-gradient(160deg, #0e1018 0%, #0a0c12 60%, #080a0f 100%);
    border: 1px solid
      color-mix(
        in srgb,
        var(--modal-accent, #00f3ff) 22%,
        rgba(255, 255, 255, 0.08)
      );
    border-radius: 18px;
    overflow: hidden;
    box-shadow:
      0 0 0 1px rgba(255, 255, 255, 0.04),
      0 0 40px color-mix(in srgb, var(--modal-accent, #00f3ff) 12%, transparent),
      0 32px 64px rgba(0, 0, 0, 0.7);
    animation: modal-slide-up 0.22s cubic-bezier(0.16, 1, 0.3, 1) both;
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

  /* ────────────────────────────────────────────
     Gradient Dots Header Banner
  ──────────────────────────────────────────── */
  .modal-header-banner {
    position: relative;
    flex-shrink: 0;
    /* Accent-tinted gradient, darkens toward panel body */
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

  /* Dot grid — very subtle texture, dimmed so content is always readable */
  .banner-dot-grid {
    position: absolute;
    inset: 0;
    background-image: radial-gradient(
      color-mix(in srgb, var(--modal-accent, #00f3ff) 7%, rgba(0, 0, 0, 0.6))
        1px,
      transparent 1px
    );
    background-size: 7px 7px;
    pointer-events: none;
    z-index: 0;
    opacity: 0.5;
  }

  /* Radial vignette — softens the dots at top-center (where content sits) */
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

  /* Accent glow blob (subtle center halo) */
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

  /* Accent line along the very top edge */
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

  /* Slot content sits above all layers */
  .modal-header-content {
    position: relative;
    z-index: 2;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 1.1rem 1.75rem 1rem;
  }

  /* ── Body ── */
  .modal-body {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem 1.75rem;
    min-height: 0;
  }
  .modal-body.no-pad {
    padding: 0;
  }

  /* ── Footer ── */
  .modal-footer {
    padding: 1rem 1.75rem 1.25rem;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    flex-shrink: 0;
  }

  /* ── Shared close button (used inside {#snippet header()} across all modals) ── */
  :global(.modal-header-content .close-btn) {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.35);
    cursor: pointer;
    transition:
      background 0.18s,
      color 0.18s,
      border-color 0.18s;
    padding: 0;
  }
  :global(.modal-header-content .close-btn:hover) {
    background: rgba(255, 255, 255, 0.12);
    color: #fff;
    border-color: rgba(255, 255, 255, 0.2);
  }
  :global(.modal-header-content .close-btn .material-icons) {
    font-size: 1.1rem;
    line-height: 1;
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
