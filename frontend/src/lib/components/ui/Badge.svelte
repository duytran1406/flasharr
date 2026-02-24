<script lang="ts">
  /**
   * Universal Badge Component
   *
   * Design: cut-corner clip-path, leading dot, transparent accent bg, monospace uppercase.
   * All badges in the application should use this component.
   *
   * Variants map to a color palette — each has transparent bg + accent border + dot.
   * Use `color` to pass a raw hex/rgb when no variant matches, or `noDot` to suppress the dot.
   */
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
    | "hdr"
    | "dv"
    | "downloaded"
    | "count";

  interface Props {
    text: string;
    variant?: BadgeVariant;
    /** Explicit accent color when variant not enough (hex or rgb string) */
    color?: string;
    size?: "xs" | "sm" | "md";
    /** Suppress the leading dot */
    noDot?: boolean;
    /** Extra CSS classes to pass through */
    class?: string;
  }

  let {
    text,
    variant = "default",
    color,
    size = "sm",
    noDot = false,
    class: extraClass = "",
  }: Props = $props();

  // Palette: [bg, border, text-color]
  const PALETTE: Record<string, [string, string, string]> = {
    default: [
      "rgba(255,255,255,0.06)",
      "rgba(255,255,255,0.15)",
      "rgba(255,255,255,0.55)",
    ],
    primary: ["rgba(0,243,255,0.08)", "rgba(0,243,255,0.28)", "#00f3ff"],
    success: ["rgba(0,255,163,0.08)", "rgba(0,255,163,0.28)", "#00ffa3"],
    warning: ["rgba(245,158,11,0.10)", "rgba(245,158,11,0.30)", "#f59e0b"],
    danger: ["rgba(255,82,82,0.10)", "rgba(255,82,82,0.30)", "#ff5252"],
    info: ["rgba(33,150,243,0.10)", "rgba(33,150,243,0.30)", "#2196f3"],
    purple: ["rgba(167,139,250,0.10)", "rgba(167,139,250,0.30)", "#a78bfa"],
    orange: ["rgba(255,152,0,0.10)", "rgba(255,152,0,0.30)", "#ff9800"],
    grey: [
      "rgba(255,255,255,0.05)",
      "rgba(255,255,255,0.10)",
      "rgba(255,255,255,0.38)",
    ],
    // Semantic aliases
    quality: ["rgba(0,243,255,0.08)", "rgba(0,243,255,0.28)", "#00f3ff"],
    source: [
      "rgba(255,255,255,0.06)",
      "rgba(255,255,255,0.12)",
      "rgba(255,255,255,0.55)",
    ],
    language: ["rgba(167,139,250,0.10)", "rgba(167,139,250,0.30)", "#c4b5fd"],
    episode: ["rgba(167,139,250,0.10)", "rgba(167,139,250,0.30)", "#a78bfa"],
    status: [
      "rgba(255,255,255,0.06)",
      "rgba(255,255,255,0.12)",
      "rgba(255,255,255,0.55)",
    ],
    vip: ["rgba(245,158,11,0.10)", "rgba(245,158,11,0.30)", "#f59e0b"],
    free: [
      "rgba(255,255,255,0.05)",
      "rgba(255,255,255,0.10)",
      "rgba(255,255,255,0.35)",
    ],
    best: ["rgba(245,158,11,0.10)", "rgba(245,158,11,0.30)", "#f59e0b"],
    smart: ["rgba(139,92,246,0.10)", "rgba(139,92,246,0.30)", "#a78bfa"],
    downloaded: ["rgba(0,255,163,0.08)", "rgba(0,255,163,0.28)", "#00ffa3"],
    count: [
      "rgba(255,255,255,0.07)",
      "rgba(255,255,255,0.10)",
      "rgba(255,255,255,0.45)",
    ],
    // HDR and DV use gradient fills — handled separately
    hdr: ["", "", "#fff"],
    dv: ["", "", "#fff"],
  };

  // For custom `color` prop, generate transparent palette from it
  function hexToRgba(hex: string, alpha: number): string {
    // Accept both hex and rgb strings
    if (hex.startsWith("rgb")) return hex; // pass-through
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgba(${r},${g},${b},${alpha})`;
  }

  function getStyle(): string {
    if (variant === "hdr") {
      return "background: linear-gradient(135deg, rgba(124,58,237,0.3), rgba(244,63,94,0.3)); border: 1px solid rgba(124,58,237,0.5); color: #fff;";
    }
    if (variant === "dv") {
      return "background: linear-gradient(135deg, rgba(245,158,11,0.3), rgba(217,70,239,0.3)); border: 1px solid rgba(245,158,11,0.4); color: #fff;";
    }
    if (color) {
      const bg = color.startsWith("#") ? hexToRgba(color, 0.1) : color;
      const bdr = color.startsWith("#") ? hexToRgba(color, 0.3) : color;
      return `background:${bg}; border:1px solid ${bdr}; color:${color};`;
    }
    const p = PALETTE[variant] ?? PALETTE.default;
    return `background:${p[0]}; border:1px solid ${p[1]}; color:${p[2]};`;
  }

  function getDotColor(): string {
    if (variant === "hdr" || variant === "dv") return "#fff";
    if (color) return color;
    return (PALETTE[variant] ?? PALETTE.default)[2];
  }
</script>

<span class="badge size-{size} {extraClass}" style={getStyle()}>
  {#if !noDot}
    <span class="badge-dot" style="background:{getDotColor()}"></span>
  {/if}
  {text}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-weight: 800;
    white-space: nowrap;
    font-family: var(--font-mono, monospace);
    letter-spacing: 0.07em;
    text-transform: uppercase;
    /* Tactical cut-corner shape — matches the search page quality badge */
    clip-path: polygon(
      3px 0%,
      calc(100% - 3px) 0%,
      100% 3px,
      100% calc(100% - 3px),
      calc(100% - 3px) 100%,
      3px 100%,
      0% calc(100% - 3px),
      0% 3px
    );
  }

  .badge-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    flex-shrink: 0;
    opacity: 0.9;
  }

  /* Sizes */
  .size-xs {
    font-size: 0.5rem;
    padding: 0.1rem 0.4rem;
  }
  .size-sm {
    font-size: 0.58rem;
    padding: 0.18rem 0.45rem;
  }
  .size-md {
    font-size: 0.68rem;
    padding: 0.25rem 0.6rem;
  }
</style>
