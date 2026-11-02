<script lang="ts" module>
  export type BadgeVariant =
    | "default"
    | "primary"
    | "success"
    | "warning"
    | "danger"
    | "info"
    | "purple"
    | "orange"
    | "grey"
    | "quality"
    | "source"
    | "language"
    | "episode"
    | "status"
    | "vip"
    | "free"
    | "best"
    | "smart"
    | "downloaded"
    | "count"
    | "hdr"
    | "dv";

  export type BadgeSize = "xs" | "sm" | "md";
</script>

<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    variant?: BadgeVariant;
    size?: BadgeSize;
    /** Plain-text label (ignored when children are passed) */
    text?: string;
    /** Override the variant color with any CSS color */
    color?: string;
    /** Hide the leading status dot (hidden by default) */
    noDot?: boolean;
    class?: string;
    title?: string;
    children?: Snippet;
  }

  let {
    variant = "default",
    size = "sm",
    text,
    color,
    noDot = true,
    class: cls = "",
    title,
    children,
  }: Props = $props();

  /** Every variant maps onto a core-ui token; tint/border are derived in CSS. */
  const VARIANT_COLORS: Record<BadgeVariant, string> = {
    default: "var(--cui-text-secondary)",
    primary: "var(--cui-primary)",
    success: "var(--cui-success)",
    warning: "var(--cui-warning)",
    danger: "var(--cui-danger)",
    info: "var(--cui-info)",
    purple: "var(--cui-score-elite)",
    orange: "var(--cui-score-suboptimal)",
    grey: "var(--cui-text-tertiary)",
    quality: "var(--cui-primary)",
    source: "var(--cui-text-secondary)",
    language: "var(--cui-score-elite)",
    episode: "var(--cui-score-elite)",
    status: "var(--cui-text-secondary)",
    vip: "var(--cui-warning)",
    free: "var(--cui-text-tertiary)",
    best: "var(--cui-warning)",
    smart: "var(--cui-score-elite)",
    downloaded: "var(--cui-success)",
    count: "var(--cui-text-secondary)",
    hdr: "var(--cui-score-elite)",
    dv: "var(--cui-warning)",
  };

  const badgeColor = $derived(color || VARIANT_COLORS[variant]);
</script>

<span
  class="badge badge--{size} {cls}"
  data-variant={variant}
  style="--badge-color: {badgeColor};"
  {title}
>
  {#if !noDot}
    <span class="badge-dot" aria-hidden="true"></span>
  {/if}
  {#if children}
    {@render children()}
  {:else if text}
    {text}
  {/if}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--cui-space-xxs);
    width: fit-content;
    border: 1px solid
      color-mix(in srgb, var(--badge-color) 32%, var(--cui-glass-border));
    clip-path: polygon(
      3px 0,
      calc(100% - 3px) 0,
      100% 3px,
      100% calc(100% - 3px),
      calc(100% - 3px) 100%,
      3px 100%,
      0 calc(100% - 3px),
      0 3px
    );
    background: color-mix(in srgb, var(--badge-color) 10%, transparent);
    color: var(--badge-color);
    font-family: var(--cui-font-mono);
    font-weight: 800;
    letter-spacing: 0.06em;
    line-height: 1;
    text-transform: uppercase;
    white-space: nowrap;
  }

  .badge--xs {
    padding: 3px 6px;
    font-size: 10px;
  }

  .badge--sm {
    padding: 4px 8px;
    font-size: 11px;
  }

  .badge--md {
    padding: 5px 10px;
    font-size: 12px;
  }

  .badge-dot {
    width: 5px;
    height: 5px;
    border-radius: var(--cui-radius-full);
    background: currentColor;
  }

  /* Neutral variants sit on the glass surface instead of a color tint */
  .badge[data-variant="default"],
  .badge[data-variant="source"],
  .badge[data-variant="status"],
  .badge[data-variant="count"],
  .badge[data-variant="grey"],
  .badge[data-variant="free"] {
    background: var(--cui-surface-hover);
    border-color: var(--cui-glass-border);
  }
</style>
