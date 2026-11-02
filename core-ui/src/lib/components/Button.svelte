<script lang="ts" module>
  export type ButtonVariant =
    | "primary"
    | "flash"
    | "ghost"
    | "secondary"
    | "danger"
    | "success"
    | "white";

  export type ButtonSize = "sm" | "md" | "lg" | "xl";
</script>

<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    /** Click handler (suppressed while disabled / loading) */
    onclick?: (e: MouseEvent) => void;
    /** Accent / border color (any CSS color). Defaults to --color-primary */
    accent?: string;
    variant?: ButtonVariant;
    /** Material Icons ligature shown left of the label */
    icon?: string;
    size?: ButtonSize;
    disabled?: boolean;
    /** Replaces the icon with a spinner and blocks clicks */
    loading?: boolean;
    /** Extra CSS class(es) */
    class?: string;
    /** <button type>, ignored when rendered as a link */
    type?: "button" | "submit" | "reset";
    children?: Snippet;
    title?: string;
    /** Explicit min-width (any CSS length), e.g. "36px", "180px", "100%" */
    width?: string;
    /** When set, the button renders as an <a> */
    href?: string;
    target?: string;
    rel?: string;
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
    href,
    target,
    rel,
  }: Props = $props();

  function handleLinkClick(e: MouseEvent) {
    if (disabled || loading) e.preventDefault();
    else onclick?.(e);
  }
</script>

{#snippet content()}
  <!-- dot-grid texture layer -->
  <span class="btn-dots" aria-hidden="true"></span>
  <!-- holographic scan shimmer -->
  <span class="btn-scan" aria-hidden="true"></span>
  <!-- content row -->
  <span class="btn-inner">
    {#if loading}
      <svg class="btn-spinner" viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <circle
          cx="12"
          cy="12"
          r="10"
          stroke="currentColor"
          stroke-width="2.5"
          stroke-dasharray="31.4 31.4"
          stroke-linecap="round"
        />
      </svg>
    {:else if icon}
      <span class="btn-icon-text material-icons" aria-hidden="true">{icon}</span>
    {/if}
    {#if children}
      <span class="btn-label">{@render children()}</span>
    {/if}
  </span>
{/snippet}

{#if href}
  <a
    {href}
    {target}
    {rel}
    {title}
    aria-busy={loading || undefined}
    aria-disabled={disabled || loading}
    tabindex={disabled || loading ? -1 : undefined}
    data-variant={variant}
    class="flasharr-btn flasharr-btn--{variant} flasharr-btn--{size} {cls}"
    style="--btn-accent: {accent};{width ? ` --btn-width: ${width};` : ''}"
    onclick={handleLinkClick}
  >
    {@render content()}
  </a>
{:else}
  <button
    {type}
    {title}
    disabled={disabled || loading}
    aria-busy={loading || undefined}
    data-variant={variant}
    class="flasharr-btn flasharr-btn--{variant} flasharr-btn--{size} {cls}"
    style="--btn-accent: {accent};{width ? ` --btn-width: ${width};` : ''}"
    onclick={!disabled && !loading ? onclick : undefined}
  >
    {@render content()}
  </button>
{/if}

<style>
  /* ── Base ── */
  .flasharr-btn {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;

    font-family: var(--font-mono, monospace);
    font-weight: 900;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    white-space: nowrap;
    text-decoration: none;

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

    transition:
      box-shadow 0.3s cubic-bezier(0.4, 0, 0.2, 1),
      transform 0.25s cubic-bezier(0.4, 0, 0.2, 1),
      opacity 0.2s,
      color 0.2s;

    min-width: var(--btn-width, auto);
    min-height: 44px;
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
  .flasharr-btn--xl {
    font-size: 1rem;
    padding: 0.9rem 2rem;
    gap: 0.6rem;
  }

  /* ── Primary / flash ── */
  .flasharr-btn--primary,
  .flasharr-btn--flash {
    background: linear-gradient(
      135deg,
      rgba(10, 16, 24, 0.7) 0%,
      rgba(21, 32, 48, 0.7) 50%,
      rgba(10, 16, 24, 0.7) 100%
    );
    color: var(--btn-accent, var(--color-primary, #00f3ff));
    /* inset box-shadow acts as the border inside clip-path */
    box-shadow:
      inset 0 0 0 1px var(--btn-accent, var(--color-primary, #00f3ff)),
      0 0 18px color-mix(in srgb, var(--btn-accent, #00f3ff) 20%, transparent);
    opacity: 0.7;
  }
  .flasharr-btn--primary:hover:not(:disabled),
  .flasharr-btn--flash:hover:not(:disabled) {
    opacity: 1;
    color: #fff;
    box-shadow:
      inset 0 0 0 1px #fff,
      0 0 28px color-mix(in srgb, var(--btn-accent, #00f3ff) 45%, transparent);
    transform: translateY(-2px);
  }

  /* ── Ghost / secondary ── */
  .flasharr-btn--ghost,
  .flasharr-btn--secondary {
    background: rgba(255, 255, 255, 0.04);
    color: rgba(255, 255, 255, 0.5);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
    opacity: 0.7;
  }
  .flasharr-btn--ghost:hover:not(:disabled),
  .flasharr-btn--secondary:hover:not(:disabled) {
    opacity: 1;
    color: #fff;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.3);
    transform: translateY(-1px);
  }

  /* ── Danger ── */
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

  /* ── Success ── */
  .flasharr-btn--success {
    background: linear-gradient(
      135deg,
      rgba(8, 24, 16, 0.7) 0%,
      rgba(12, 40, 24, 0.7) 50%,
      rgba(8, 24, 16, 0.7) 100%
    );
    color: #00ffa3;
    box-shadow:
      inset 0 0 0 1px #00ffa3,
      0 0 18px rgba(0, 255, 163, 0.18);
    opacity: 0.7;
  }
  .flasharr-btn--success:hover:not(:disabled) {
    opacity: 1;
    color: #fff;
    box-shadow:
      inset 0 0 0 1px #00ffa3,
      0 0 28px rgba(0, 255, 163, 0.45);
    transform: translateY(-2px);
  }

  /* ── White (solid) ── */
  .flasharr-btn--white {
    background: #fff;
    color: #000;
    box-shadow: none;
    opacity: 1;
  }
  .flasharr-btn--white:hover:not(:disabled) {
    opacity: 0.9;
    transform: translateY(-2px);
  }

  /* ── States ── */
  .flasharr-btn:active:not(:disabled) {
    transform: translateY(0) scale(0.97);
  }

  .flasharr-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
    transform: none !important;
  }

  /* ── Dot-grid background texture ── */
  .flasharr-btn:not(.flasharr-btn--white) .btn-dots {
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
  .flasharr-btn:not(.flasharr-btn--white) .btn-scan {
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

  .flasharr-btn:hover:not(:disabled):not(.flasharr-btn--white) .btn-scan {
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

  /* ── Loading spinner ── */
  .btn-spinner {
    width: 1.1em;
    height: 1.1em;
    animation: btn-spin-rotate 0.8s linear infinite;
    flex-shrink: 0;
  }

  .btn-spinner circle {
    /* stroke comes from currentColor via the stroke attribute */
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

  .btn-icon-text {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    line-height: 1;
  }

  .btn-label {
    line-height: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
  }
</style>
