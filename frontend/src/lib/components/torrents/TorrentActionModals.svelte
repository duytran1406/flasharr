<script lang="ts">
  import TorrentAddModal from "./TorrentAddModal.svelte";
  import TorrentCreateModal from "./TorrentCreateModal.svelte";
  import TorrentCrossSeedModal from "./TorrentCrossSeedModal.svelte";
  import TorrentDeleteModal from "./TorrentDeleteModal.svelte";
  import TorrentLabelsModal from "./TorrentLabelsModal.svelte";
  import TorrentLimitsModal from "./TorrentLimitsModal.svelte";
  import TorrentMoveModal from "./TorrentMoveModal.svelte";
  import TorrentPrefsModal from "./TorrentPrefsModal.svelte";
  import TorrentRenameModal from "./TorrentRenameModal.svelte";
  import type { TorrentIntent } from "./torrentIntent";

  interface Props {
    /** The open action, or null when every modal is closed. */
    intent: TorrentIntent | null;
    onClose: () => void;
  }

  let { intent, onClose }: Props = $props();
</script>

<TorrentAddModal open={intent?.kind === "add"} {onClose} />
<TorrentCreateModal open={intent?.kind === "create"} {onClose} />
<TorrentLabelsModal
  open={intent?.kind === "labels" && intent.mode === "category"}
  mode="category"
  torrent={intent?.kind === "labels" ? (intent.torrent ?? null) : null}
  {onClose}
/>
<TorrentLabelsModal
  open={intent?.kind === "labels" && intent.mode === "tags"}
  mode="tags"
  torrent={intent?.kind === "labels" ? (intent.torrent ?? null) : null}
  {onClose}
/>
<TorrentDeleteModal
  open={intent?.kind === "delete"}
  torrent={intent?.kind === "delete" ? (intent.torrent ?? null) : null}
  {onClose}
/>
<TorrentPrefsModal open={intent?.kind === "prefs"} {onClose} />
<TorrentLimitsModal
  open={intent?.kind === "limits"}
  torrent={intent?.kind === "limits" ? (intent.torrent ?? null) : null}
  {onClose}
/>
<TorrentRenameModal
  open={intent?.kind === "rename"}
  mode={intent?.kind === "rename" ? intent.mode : "torrent"}
  hash={intent?.kind === "rename" ? intent.hash : ""}
  label={intent?.kind === "rename" ? intent.label : "item"}
  currentName={intent?.kind === "rename" ? (intent.currentName ?? "") : ""}
  oldPath={intent?.kind === "rename" ? (intent.oldPath ?? "") : ""}
  {onClose}
/>
<TorrentMoveModal
  open={intent?.kind === "move"}
  torrent={intent?.kind === "move" ? (intent.torrent ?? null) : null}
  {onClose}
/>
<TorrentCrossSeedModal
  open={intent?.kind === "cross-seed"}
  torrent={intent?.kind === "cross-seed" ? intent.torrent : null}
  {onClose}
/>
