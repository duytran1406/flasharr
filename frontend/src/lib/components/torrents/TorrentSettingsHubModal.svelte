<script lang="ts">
  import { Button, Modal } from "@media-set/core-ui";

  interface Props {
    open: boolean;
    onClose: () => void;
    onInstances: () => void;
    onPreferences: () => void;
    onCreate: () => void;
  }

  let { open, onClose, onInstances, onPreferences, onCreate }: Props = $props();
</script>

<Modal {open} {onClose} maxWidth="560px" ariaLabel="Torrent settings">
  {#snippet header()}
    <h2>Torrent settings</h2>
  {/snippet}

  <div class="hub">
    <Button variant="ghost" icon="dns" onclick={onInstances}>Manage instances</Button><Button
      variant="ghost"
      icon="tune"
      onclick={onPreferences}>qBittorrent preferences</Button
    ><Button variant="ghost" icon="construction" onclick={onCreate}>Create torrent</Button>
  </div>
</Modal>

<style>
	h2 {
		margin: 0;
	}

	.hub {
		display: grid;
		gap: .6rem;
	}

	.hub :global(button) {
		justify-content: flex-start;
	}
</style>
