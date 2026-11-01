import type { BadgeVariant } from "../components/Badge.svelte";

/**
 * Badge variant for a media type label ("movie", "tv", ...).
 *
 * RECOVERY NOTE: inferred, no compiled evidence (never called by any surviving
 * build). Colors follow the Badge variant palette.
 */
export function typeBadgeVariant(type: string | null | undefined): BadgeVariant {
  switch ((type ?? "").toLowerCase()) {
    case "movie":
    case "film":
      return "primary";
    case "tv":
    case "series":
    case "show":
      return "purple";
    case "episode":
      return "episode";
    case "season":
      return "info";
    case "collection":
      return "orange";
    default:
      return "default";
  }
}
