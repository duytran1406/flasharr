<script lang="ts">
  import { Modal } from '@media-set/core-ui';
  import AddDownloadSourceChooser from './AddDownloadSourceChooser.svelte';
  import FshareManualAddModal from '$lib/components/fshare/FshareManualAddModal.svelte';
  import TorrentManualAddModal from './TorrentManualAddModal.svelte';
  import TorboxManualAddModal from './TorboxManualAddModal.svelte';

  type IntakeSource = 'fshare' | 'torrent' | 'torbox';

  interface Props {
    isOpen?: boolean;
    onClose?: () => void;
  }

  let { isOpen = $bindable(false), onClose }: Props = $props();

  let fshareOpen = $state(false);
  let torrentOpen = $state(false);
  let torboxOpen = $state(false);
  let returnFocus = $state<HTMLElement | null>(null);
  let wasOpen = $state(false);

  // Remember what had focus when the chooser opened, to restore it on close.
  $effect(() => {
    if (isOpen && !wasOpen) {
      returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    }
    wasOpen = isOpen;
  });

  function restoreFocus() {
    returnFocus?.focus?.();
    returnFocus = null;
  }

  function closeAll() {
    isOpen = false;
    fshareOpen = false;
    torrentOpen = false;
    torboxOpen = false;
    restoreFocus();
    onClose?.();
  }

  /** Swap the chooser for the picked intake modal. */
  function pick(source: IntakeSource) {
    isOpen = false;
    if (source === 'fshare') fshareOpen = true;
    if (source === 'torrent') torrentOpen = true;
    if (source === 'torbox') torboxOpen = true;
  }

  function closeIntake(source: IntakeSource) {
    if (source === 'fshare') fshareOpen = false;
    if (source === 'torrent') torrentOpen = false;
    if (source === 'torbox') torboxOpen = false;
    restoreFocus();
    onClose?.();
  }
</script>

<Modal
  open={isOpen}
  onClose={closeAll}
  maxWidth="720px"
  accent="var(--color-primary, #00f3ff)"
  ariaLabel="Choose download source"
>
  {#snippet header()}
    <div class="title-row">
      <span class="material-icons icon">add_circle</span><h2>Add Download</h2>
    </div><button class="close-btn" aria-label="Close" onclick={closeAll}><span class="material-icons">close</span></button>
  {/snippet}

  {#snippet children()}
    <div class="intro">
      <p>Pick the intake first: Flasharr queues direct and TorBox downloads, qBittorrent handles raw torrent adds.</p>
    </div><AddDownloadSourceChooser onPick={pick} />
  {/snippet}
</Modal>

<FshareManualAddModal open={fshareOpen} onClose={() => closeIntake('fshare')} />
<TorrentManualAddModal open={torrentOpen} onClose={() => closeIntake('torrent')} />
<TorboxManualAddModal open={torboxOpen} onClose={() => closeIntake('torbox')} />

<style>
  .title-row {
    display:flex;
    align-items:center;
    gap:.6rem;
  }
  .icon {
    color:var(--color-primary,#00f3ff);
    font-size:1.3rem;
  }
  h2 {
    font-size:1.05rem;
    font-weight:800;
    color:#fff;
    margin:0;
  }
  .intro p {
    margin:0 0 1rem;
    color:#ffffffa6;
    font-size:.82rem;
    line-height:1.5;
  }
  .close-btn {
    background:transparent;
    border:none;
    color:var(--text-muted,#888);
    cursor:pointer;
    padding:.5rem;
    border-radius:8px;
    transition:all .2s ease;
    display:flex;
    align-items:center;
    justify-content:center;
  }
  .close-btn:hover {
    background:#ffffff1a;
    color:var(--text-primary,#fff);
  }
</style>
