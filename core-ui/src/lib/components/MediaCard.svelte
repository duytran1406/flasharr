<!--
  MediaCard — generic poster card used inside <MediaShelf>.

  RECOVERY NOTE: inferred, no compiled evidence. The component was tree-shaken
  out of every surviving build. This minimal version follows the props the
  2026-05-14 frontend passes (title, imgUrl, year, tag, href, badgeText,
  badgeVariant, isHovered) and is styled entirely by the global classes in
  styles/media-card.css (.media-card, .card-inner, .card-hud, ...), which are
  recovered from compiled CSS.
-->
<script lang="ts">
  import Badge, { type BadgeVariant } from "./Badge.svelte";

  interface Props {
    title: string;
    /** Poster image URL; a text fallback is shown when empty */
    imgUrl?: string;
    year?: string;
    /** Small type label, e.g. "MOVIE" / "TV" */
    tag?: string;
    href?: string;
    badgeText?: string;
    badgeVariant?: BadgeVariant;
    /** Expanded state, normally driven by MediaShelf */
    isHovered?: boolean;
    /** Keep poster width on hover (no 21:9 expansion) */
    disableExpansion?: boolean;
    accentColor?: string;
    onclick?: (e: MouseEvent) => void;
  }

  let {
    title,
    imgUrl = "",
    year = "",
    tag = "",
    href,
    badgeText = "",
    badgeVariant = "default",
    isHovered = false,
    disableExpansion = false,
    accentColor = "var(--color-primary, #00f3ff)",
    onclick,
  }: Props = $props();

  let imgFailed = $state(false);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions (renders <a> or <button>) -->
<svelte:element
  this={href ? "a" : "button"}
  {href}
  type={href ? undefined : "button"}
  class="media-card"
  class:is-hovered={isHovered}
  class:disable-expansion={disableExpansion}
  style="--accent-color: {accentColor}; --border-color: {accentColor};"
  aria-label={title}
  {onclick}
>
  <div class="card-inner">
    {#if imgUrl && !imgFailed}
      <img
        class="card-img card-img--poster"
        src={imgUrl}
        alt={title}
        loading="lazy"
        onerror={() => (imgFailed = true)}
      />
    {:else}
      <div class="fallback-bg"><span>{title}</span></div>
    {/if}

    <div class="card-overlay"></div>

    {#if badgeText}
      <div class="absolute top-2 right-2 z-20">
        <Badge text={badgeText} variant={badgeVariant} size="xs" />
      </div>
    {/if}

    <div class="card-hud">
      <div class="atmosphere"></div>
      <div class="accent-bar"></div>
      <div class="hud-content">
        <div class="card-title">{title}</div>
        <div class="flex items-center gap-2 mt-1">
          {#if tag}<span class="genre-badge">{tag}</span>{/if}
          {#if year}<span class="meta-year">{year}</span>{/if}
        </div>
      </div>
    </div>
  </div>
</svelte:element>
