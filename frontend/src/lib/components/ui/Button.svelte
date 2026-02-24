<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    /** Click handler */
    onclick?: (e: MouseEvent) => void;
    /** Accent / border color (CSS value). Defaults to --color-primary */
    accent?: string;
    /** Button variant. 'primary' = full accent border, 'ghost' = dimmer */
    variant?: "primary" | "ghost" | "danger";
    /** Optional icon name (Material Icons ligature) shown left of children */
    icon?: string;
    /** Size preset */
    size?: "sm" | "md" | "lg";
    /** Whether button is disabled */
    disabled?: boolean;
    /** Show spinner instead of icon */
    loading?: boolean;
    /** Extra CSS class(es) */
    class?: string;
    /** Button type attribute */
    type?: "button" | "submit" | "reset";
    /** Slot content */
    children?: Snippet;
    /** title attribute */
    title?: string;
    /** Explicit width — sets min-width on the button (any CSS value). e.g. "36px", "180px", "100%" */
    width?: string;
  }

  let {
    onclick,
    accent = "var(--color-primary, #00f3ff)",
    variant = "primary",
    icon,
    size = "md",
    disabled = false,
    loading = false,
    class: cls = "",
    type = "button",
    children,
    title,
    width,
  }: Props = $props();
</script>

<button
  {type}
  {title}
  {disabled}
  class="flasharr-btn flasharr-btn--{variant} flasharr-btn--{size} {cls}"
  style="--btn-accent: {accent};{width ? ` --btn-width: ${width};` : ''}"
  onclick={!disabled && !loading ? onclick : undefined}
>
  <!-- dot-grid texture layer -->
  <span class="btn-dots" aria-hidden="true"></span>
  <!-- holographic scan shimmer -->
  <span class="btn-scan" aria-hidden="true"></span>
  <!-- content row -->
  <span class="btn-inner">
    {#if loading}
      <span class="material-icons btn-spin">autorenew</span>
    {:else if icon}
      <span class="material-icons btn-icon">{icon}</span>
    {/if}
    {#if children}
      <span class="btn-label">{@render children()}</span>
    {/if}
  </span>
</button>

<style>
  /* ── Base ── */
  .flasharr-btn {
    /* Positioning */
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;

    /* Typography */
    font-family: var(--font-mono, monospace);
    font-weight: 900;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    white-space: nowrap;

    /* Interaction */
    cursor: pointer;
    user-select: none;
    border: none;
    outline: none;

    /* The signature angled clip-path corners */
    clip-path: polygon(
      8px 0%,
      100% 0%,
      100% calc(100% - 8px),
      calc(100% - 8px) 100%,
      0% 100%,
      0% 8px
    );

    /* Transitions */
    transition:
      box-shadow 0.3s cubic-bezier(0.4, 0, 0.2, 1),
      transform 0.25s cubic-bezier(0.4, 0, 0.2, 1),
      opacity 0.2s,
      color 0.2s;

    /* Explicit width if provided */
    min-width: var(--btn-width, auto);
  }

  /* ── Sizes ── */
  .flasharr-btn--sm {
    font-size: 0.65rem;
    padding: 0.4rem 0.9rem;
    gap: 0.3rem;
  }
  .flasharr-btn--md {
    font-size: 0.75rem;
    padding: 0.6rem 1.25rem;
    gap: 0.4rem;
  }
  .flasharr-btn--lg {
    font-size: 0.85rem;
    padding: 0.75rem 1.6rem;
    gap: 0.5rem;
  }

  /* ── Primary variant ── */
  .flasharr-btn--primary {
    background: linear-gradient(
      135deg,
      rgba(10, 16, 24, 0.7) 0%,
      rgba(21, 32, 48, 0.7) 50%,
      rgba(10, 16, 24, 0.7) 100%
    );
    color: var(--btn-accent, var(--color-primary, #00f3ff));
    /* 2px inset box-shadow acts as the border inside clip-path */
    box-shadow:
      inset 0 0 0 1px var(--btn-accent, var(--color-primary, #00f3ff)),
      0 0 18px color-mix(in srgb, var(--btn-accent, #00f3ff) 20%, transparent);
    opacity: 0.7;
  }

  .flasharr-btn--primary:hover:not(:disabled) {
    opacity: 1;
    color: #fff;
    box-shadow:
      inset 0 0 0 1px #fff,
      0 0 28px color-mix(in srgb, var(--btn-accent, #00f3ff) 45%, transparent);
    transform: translateY(-2px);
  }

  /* ── Ghost variant ── */
  .flasharr-btn--ghost {
    background: rgba(255, 255, 255, 0.04);
    color: rgba(255, 255, 255, 0.5);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
    opacity: 0.7;
  }
  .flasharr-btn--ghost:hover:not(:disabled) {
    opacity: 1;
    color: #fff;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.3);
    transform: translateY(-1px);
  }

  /* ── Danger variant ── */
  .flasharr-btn--danger {
    background: linear-gradient(
      135deg,
      rgba(24, 8, 8, 0.7) 0%,
      rgba(40, 12, 12, 0.7) 50%,
      rgba(24, 8, 8, 0.7) 100%
    );
    color: #ff5252;
    box-shadow:
      inset 0 0 0 1px #ff5252,
      0 0 18px rgba(255, 82, 82, 0.18);
    opacity: 0.7;
  }
  .flasharr-btn--danger:hover:not(:disabled) {
    opacity: 1;
    color: #fff;
    box-shadow:
      inset 0 0 0 1px #ff5252,
      0 0 28px rgba(255, 82, 82, 0.45);
    transform: translateY(-2px);
  }

  /* ── Active state ── */
  .flasharr-btn:active:not(:disabled) {
    transform: translateY(0) scale(0.97);
  }

  /* ── Disabled state ── */
  .flasharr-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
    transform: none !important;
  }

  /* ── Dot-grid background texture ── */
  .btn-dots {
    position: absolute;
    inset: 0;
    background-image: radial-gradient(
      color-mix(in srgb, var(--btn-accent, #00f3ff) 22%, rgba(0, 0, 0, 0.4)) 1px,
      transparent 1px
    );
    background-size: 5px 5px;
    pointer-events: none;
    z-index: 0;
  }

  /* ── Holographic scan shimmer (visible on hover) ── */
  .btn-scan {
    position: absolute;
    inset: -2px;
    background: linear-gradient(
      90deg,
      transparent 0%,
      color-mix(in srgb, var(--btn-accent, #00f3ff) 20%, transparent) 45%,
      color-mix(in srgb, var(--btn-accent, #00f3ff) 60%, transparent) 50%,
      color-mix(in srgb, var(--btn-accent, #00f3ff) 20%, transparent) 55%,
      transparent 100%
    );
    transform: translateX(-100%);
    opacity: 0;
    pointer-events: none;
    z-index: 1;
    transition: opacity 0.3s;
  }

  .flasharr-btn:hover:not(:disabled) .btn-scan {
    opacity: 1;
    animation: btn-scan-sweep 3s linear infinite;
  }

  @keyframes btn-scan-sweep {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(200%);
    }
  }

  /* ── Spinning loading icon ── */
  .btn-spin {
    animation: btn-spin-rotate 1s linear infinite;
    font-size: inherit;
  }
  @keyframes btn-spin-rotate {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  /* ── Inner content row (above dots + scan) ── */
  .btn-inner {
    position: relative;
    z-index: 2;
    display: flex;
    align-items: center;
    gap: inherit;
  }

  .btn-icon {
    font-size: 1.1rem;
    line-height: 1;
  }

  .btn-label {
    line-height: 1;
  }
</style>
