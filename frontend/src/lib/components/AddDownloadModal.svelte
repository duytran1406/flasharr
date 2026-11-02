<!--
  Root-layout entry point for "Add Download" (Ctrl+N). Since the 2026 intake redesign it only
  forwards to the source chooser in add-download/AddDownloadEntryModal.svelte.
-->
<script lang="ts">
  import AddDownloadEntryModal from './add-download/AddDownloadEntryModal.svelte';

  interface Props {
    isOpen?: boolean;
    onClose?: () => void;
  }

  let { isOpen = $bindable(false), onClose }: Props = $props();
</script>

<AddDownloadEntryModal bind:isOpen {onClose} />
