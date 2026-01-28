<script lang="ts">
  interface Props {
    text: string;
    variant?:
      | "default"
      | "outline"
      | "score"
      | "quality"
      | "source"
      | "language"
      | "episode";
    size?: "sm" | "md";
    color?: string; // Override color
  }

  let { text, variant = "default", size = "md", color }: Props = $props();

  function getVariantStyle(v: string) {
    // If explicit color is provided, use it for text and border/bg logic
    const baseColor = color || "var(--color-primary)";

    switch (v) {
      case "quality":
        if (text === "4K")
          return `background: rgba(255, 215, 0, 0.15); color: #ffd700; border: 1px solid rgba(255, 215, 0, 0.3)`;
        if (text === "1080p")
          return `background: rgba(0, 255, 163, 0.15); color: #00ffa3; border: 1px solid rgba(0, 255, 163, 0.3)`;
        return `background: rgba(0, 243, 255, 0.1); color: #00f3ff; border: 1px solid rgba(0, 243, 255, 0.3)`;
      case "source":
        return `background: rgba(255,255,255,0.1); color: #e0e0e0; border: 1px solid rgba(255,255,255,0.15)`;
      case "language":
        return `background: ${color ? color + "20" : "rgba(255, 107, 107, 0.2)"}; color: ${color || "#ff6b6b"}; border: 1px solid ${color ? color + "40" : "rgba(255, 107, 107, 0.3)"}`;
      case "episode":
        return `background: rgba(138, 43, 226, 0.2); color: #c084fc; border: 1px solid rgba(138, 43, 226, 0.4); font-family: var(--font-mono)`;
      default:
        return "";
    }
  }
</script>

<span
  class="badge badge-{variant} size-{size}"
  style={getVariantStyle(variant)}
>
  {text}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-weight: 700;
    border-radius: 4px;
    white-space: nowrap;
    font-family: var(--font-mono, monospace);
    letter-spacing: 0.05em;
  }

  /* Sizes */
  .size-sm {
    font-size: 0.65rem;
    padding: 0.15rem 0.4rem;
  }

  .size-md {
    font-size: 0.75rem;
    padding: 0.25rem 0.6rem;
  }

  /* Default fallbacks if style prop isn't fully overriding */
  .badge-default {
    background: var(--color-primary);
    color: #000;
  }
</style>
