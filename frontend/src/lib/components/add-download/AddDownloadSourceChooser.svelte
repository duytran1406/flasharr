<script lang="ts">
  export type DownloadIntake = "fshare" | "torrent" | "torbox";

  interface Props {
    onPick: (intake: DownloadIntake) => void;
  }

  let { onPick }: Props = $props();

  const options: { key: DownloadIntake; title: string; desc: string; icon: string }[] = [
    { key: "fshare", title: "Fshare / Direct", desc: "Queue URL downloads in Flasharr.", icon: "cloud_download" },
    { key: "torrent", title: "Torrent", desc: "Add magnet or .torrent to qBittorrent.", icon: "downloading" },
    { key: "torbox", title: "TorBox", desc: "Queue TorBox refs or cached links in Flasharr.", icon: "bolt" },
  ];
</script>

<div class="chooser-grid">
  {#each options as option}
    <button type="button" class="chooser-card" onclick={() => onPick(option.key)}>
      <span class="material-icons icon">{option.icon}</span>
      <span class="title">{option.title}</span>
      <span class="desc">{option.desc}</span>
    </button>
  {/each}
</div>

<style>
  .chooser-grid {
    display: grid;
    grid-template-columns: repeat(3,minmax(0,1fr));
    gap: var(--cui-space-sm);
  }

  .chooser-card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--cui-space-xs);
    min-height: var(--cui-touch-min);
    padding: var(--cui-space-md);
    background: var(--cui-surface-hover);
    border: 1px solid var(--cui-glass-border);
    border-radius: var(--cui-radius-md);
    color: var(--cui-text-primary);
    cursor: pointer;
    transition: transform var(--cui-duration-fast) var(--cui-ease-standard),background var(--cui-duration-fast) var(--cui-ease-standard),border-color var(--cui-duration-fast) var(--cui-ease-standard);
    text-align: left;
  }

  .chooser-card:hover {
    border-color: var(--cui-glass-border-active);
    background: var(--cui-surface-focus);
    transform: translateY(-1px);
  }

  .chooser-card:focus-visible {
    border-color: var(--cui-glass-border-active);
    background: var(--cui-surface-focus);
    box-shadow: var(--cui-glow-primary);
  }

  .icon {
    font-size: var(--cui-text-xl);
    color: var(--cui-primary);
  }

  .title {
    font-size: var(--cui-text-sm);
    font-weight: 800;
  }

  .desc {
    font-size: var(--cui-text-xs);
    color: var(--cui-text-secondary);
    line-height: 1.4;
  }

  @media (max-width:720px) {
    .chooser-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
