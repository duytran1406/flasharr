/**
 * @media-set/core-ui — shared Svelte 5 UI kit (Cyber-Dark Elite design system).
 *
 * Global styles live in ./styles/*.css and are imported by the app's CSS entry;
 * animations are exported separately from "@media-set/core-ui/animations".
 */

export { default as Button } from "./lib/components/Button.svelte";
export type { ButtonVariant, ButtonSize } from "./lib/components/Button.svelte";

export { default as Badge } from "./lib/components/Badge.svelte";
export type { BadgeVariant, BadgeSize } from "./lib/components/Badge.svelte";
export { typeBadgeVariant } from "./lib/utils/badge";

export { default as Modal } from "./lib/components/Modal.svelte";

export { toasts, createToastStore } from "./lib/stores/toast";
export type { Toast, ToastType } from "./lib/stores/toast";

export { default as MediaCard } from "./lib/components/MediaCard.svelte";
export { default as UserAvatar } from "./lib/components/UserAvatar.svelte";
export { default as ErrorState } from "./lib/components/ErrorState.svelte";
export { default as Tooltip } from "./lib/components/Tooltip.svelte";
export { default as MediaShelf } from "./lib/components/MediaShelf.svelte";

export { stripVietnameseDiacritics, normalizeTitle } from "./lib/utils/text-utils";
