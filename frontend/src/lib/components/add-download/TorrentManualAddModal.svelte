<!--
  Torrent intake of the Add Download chooser: TorrentAddModal without a preselected .torrent file.
  Classless wrapper (no scoped CSS), so its original path is unknown; the name is a guess.
-->
<script lang="ts">
  import TorrentAddModal from '$lib/components/torrents/TorrentAddModal.svelte';

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();
</script>

<TorrentAddModal file={null} {open} {onClose} />
