<script lang="ts">
  import { Badge } from "@media-set/core-ui";
  import {
    normalizeSearchResult,
    resultBadges,
    resultSourceMeta,
    type RawSearchResult,
  } from "./resultUtils";

  interface Props {
    file: RawSearchResult;
    kind?: "movie" | "episode";
    /** Compact mode keeps at most two language/HDR/cache badges. */
    compact?: boolean;
  }

  let { file, kind = "movie", compact = false }: Props = $props();

  const normalized = $derived(normalizeSearchResult(file, kind));
  const meta = $derived(resultSourceMeta(normalized));
  const badges = $derived.by(() => {
    // The source badge is redundant next to the meta line.
    const all = resultBadges(normalized).filter(
      (badge) => !["FSHARE", "TORRENT", "TORBOX"].includes(badge.text),
    );
    return compact
      ? all.filter((badge) => ["VIE", "SUB", "HDR", "DV", "CACHED"].includes(badge.text)).slice(0, 2)
      : all;
  });
</script>

<div class="source-summary" class:compact>
  <p>{meta}</p>
  {#if badges.length > 0}
    <div class="badges">
      {#each badges as badge}
        <Badge text={badge.text} variant={badge.variant} size="xs" noDot={badge.noDot} color={badge.color} />
      {/each}
    </div>
  {/if}
</div>

<style>
  .source-summary {
    display: flex;
    flex-direction: column;
    gap: .35rem;
    min-width: 0;
  }

  .badges {
    display: flex;
    gap: .35rem;
    flex-wrap: wrap;
  }

  p {
    margin: 0;
    font-size: .72rem;
    color: #ffffff80;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .compact p {
    font-size: .68rem;
  }
</style>
