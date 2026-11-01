<script lang="ts">
  import { tick } from 'svelte';
  import type { DownloadTask } from '$lib/stores/downloads.svelte';
  import type { UnifiedTorrentRow } from '$lib/stores/qbitReducer';
  import { copySourceLabel, type BatchGroup, type ContextMenuPosition } from './downloadsHelpers';

  type DownloadAction = 'pause' | 'resume' | 'retry' | 'redownload' | 'delete';
  type BatchAction = 'pause' | 'resume' | 'retry' | 'redownload' | 'delete';

  interface Props {
    contextMenuId: string | null;
    contextMenuPos: ContextMenuPosition;
    torrentContextMenuId: string | null;
    torrentContextMenuPos: ContextMenuPosition;
    batchContextMenuId: string | null;
    batchContextMenuPos: ContextMenuPosition;
    allDownloads: DownloadTask[];
    torrentRows: Map<string, UnifiedTorrentRow>;
    filteredBatches: BatchGroup[];
    onAction: (action: DownloadAction, id: string) => void;
    onOpenInfo: (task: DownloadTask) => void;
    onShowTaskDetails: (task: DownloadTask) => void;
    onCopySafeSource: (task: DownloadTask) => void;
    onCopyError: (message: string) => void;
    onShowDeleteConfirm: (task: DownloadTask) => void;
    onTorrentAction: (action: 'copy', torrent: UnifiedTorrentRow) => void;
    onBatchAction: (action: BatchAction, batchId: string) => void;
    onShowBatchDetails: (batch: BatchGroup) => void;
    onHide: () => void;
  }

  let {
    contextMenuId,
    contextMenuPos,
    torrentContextMenuId,
    torrentContextMenuPos,
    batchContextMenuId,
    batchContextMenuPos,
    allDownloads,
    torrentRows,
    filteredBatches,
    onAction,
    onOpenInfo,
    onShowTaskDetails,
    onCopySafeSource,
    onCopyError,
    onShowDeleteConfirm,
    onTorrentAction,
    onBatchAction,
    onShowBatchDetails,
    onHide,
  }: Props = $props();

  let contextTask = $derived(contextMenuId ? allDownloads.find((task) => task.id === contextMenuId) : undefined);
  let contextTorrent = $derived(torrentContextMenuId ? torrentRows.get(torrentContextMenuId) : undefined);

  /** Focus the first item and handle arrow/Home/End/Escape navigation. */
  function menuKeyboard(node: HTMLElement) {
    tick().then(() => node.querySelector<HTMLElement>('[role="menuitem"]')?.focus());
    const onKeydown = (e: KeyboardEvent) => {
      const items = [...node.querySelectorAll<HTMLElement>('[role="menuitem"]:not([disabled])')];
      const index = items.indexOf(document.activeElement as HTMLElement);
      if (e.key === 'Escape') {
        e.preventDefault();
        onHide();
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        items[(index + 1) % items.length]?.focus();
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        items[(index - 1 + items.length) % items.length]?.focus();
      } else if (e.key === 'Home') {
        e.preventDefault();
        items[0]?.focus();
      } else if (e.key === 'End') {
        e.preventDefault();
        items.at(-1)?.focus();
      }
    };
    node.addEventListener('keydown', onKeydown);
    return {
      destroy() {
        node.removeEventListener('keydown', onKeydown);
      },
    };
  }

  /** Keep the menu inside the viewport (10px margin), also on resize. */
  function clampToViewport(node: HTMLElement) {
    const clamp = () => {
      const rect = node.getBoundingClientRect();
      const left = Math.max(10, Math.min(rect.left, window.innerWidth - rect.width - 10));
      const top = Math.max(10, Math.min(rect.top, window.innerHeight - rect.height - 10));
      node.style.left = `${left}px`;
      node.style.top = `${top}px`;
    };
    requestAnimationFrame(clamp);
    window.addEventListener('resize', clamp);
    return {
      destroy() {
        window.removeEventListener('resize', clamp);
      },
    };
  }

  function truncate(text: string): string {
    return text.length > 40 ? text.substring(0, 40) + '...' : text;
  }
</script>

{#if contextMenuId && contextTask}
  <div
    class="context-menu"
    role="menu"
    tabindex="-1"
    style="left:{contextMenuPos.x}px;top:{contextMenuPos.y}px;"
    use:clampToViewport
    use:menuKeyboard
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onHide()}
  >
    <div class="context-header">{truncate(contextTask.filename)}</div>
    {#if contextTask.state === 'DOWNLOADING' || contextTask.state === 'STARTING'}
      <button role="menuitem" class="context-item" onclick={() => onAction('pause', contextMenuId)}
        ><span class="material-icons">pause</span>Pause Download</button
      >
    {:else if contextTask.state === 'PAUSED'}
      <button role="menuitem" class="context-item" onclick={() => onAction('resume', contextMenuId)}
        ><span class="material-icons">play_arrow</span>Resume Download</button
      >
    {:else if contextTask.state === 'QUEUED' || contextTask.state === 'WAITING'}
      <button role="menuitem" class="context-item" onclick={() => onAction('resume', contextMenuId)}
        ><span class="material-icons">play_arrow</span>Start Now</button
      >
    {/if}
    {#if contextTask.state === 'COMPLETED' || contextTask.state === 'FAILED' || contextTask.state === 'PAUSED' || contextTask.state === 'CANCELLED'}
      <button
        role="menuitem"
        class="context-item"
        style="color:var(--cui-secondary)"
        onclick={() => onAction('redownload', contextMenuId)}
        ><span class="material-icons">cloud_download</span>Re-download Task</button
      >
    {/if}
    <button role="menuitem" class="context-item" onclick={() => onOpenInfo(contextTask)}
      ><span class="material-icons">open_in_new</span>Open Media Info</button
    >
    <button role="menuitem" class="context-item" onclick={() => onShowTaskDetails(contextTask)}
      ><span class="material-icons">info_outline</span>View Details</button
    >
    <div class="context-divider"></div>
    <button
      role="menuitem"
      class="context-item"
      onclick={() => {
        onCopySafeSource(contextTask);
        onHide();
      }}><span class="material-icons">content_copy</span> {copySourceLabel(contextTask)}</button
    >
    {#if contextTask.error_message}
      <button role="menuitem" class="context-item" onclick={() => onCopyError(contextTask.error_message ?? '')}
        ><span class="material-icons">bug_report</span>Copy Error Message</button
      >
    {/if}
    <div class="context-divider"></div>
    <button role="menuitem" class="context-item danger" onclick={() => onShowDeleteConfirm(contextTask)}
      ><span class="material-icons">delete</span>Delete Task</button
    >
  </div>
{/if}

{#if torrentContextMenuId && contextTorrent}
  <div
    class="context-menu"
    role="menu"
    tabindex="-1"
    style="left:{torrentContextMenuPos.x}px;top:{torrentContextMenuPos.y}px;"
    use:clampToViewport
    use:menuKeyboard
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onHide()}
  >
    <div class="context-header">{truncate(contextTorrent.name)}</div>
    <div class="context-note">Queue overview only. Use Torrents for qBittorrent controls.</div>
    <button role="menuitem" class="context-item" onclick={() => onTorrentAction('copy', contextTorrent)}
      ><span class="material-icons">content_copy</span>Copy Hash</button
    >
    <a role="menuitem" class="context-item context-link" href="/torrents"
      ><span class="material-icons">open_in_new</span>Open in Torrents</a
    >
  </div>
{/if}

{#if batchContextMenuId}
  <div
    class="context-menu"
    role="menu"
    tabindex="-1"
    style="left:{batchContextMenuPos.x}px;top:{batchContextMenuPos.y}px;"
    use:clampToViewport
    use:menuKeyboard
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onHide()}
  >
    <div class="context-header">Batch Actions</div>
    <button role="menuitem" class="context-item" onclick={() => onBatchAction('resume', batchContextMenuId)}
      ><span class="material-icons">play_arrow</span>Resume All</button
    >
    <button role="menuitem" class="context-item" onclick={() => onBatchAction('pause', batchContextMenuId)}
      ><span class="material-icons">pause</span>Pause All</button
    >
    <button
      role="menuitem"
      class="context-item"
      onclick={() => {
        const batch = filteredBatches.find((group) => group.batchId === batchContextMenuId);
        if (batch) onShowBatchDetails(batch);
      }}><span class="material-icons">info_outline</span>View Details</button
    >
    <button
      role="menuitem"
      class="context-item"
      style="color:var(--cui-secondary)"
      onclick={() => onBatchAction('redownload', batchContextMenuId)}
      ><span class="material-icons">cloud_download</span>Re-download All Tasks</button
    >
    <div class="context-divider"></div>
    <button role="menuitem" class="context-item danger" onclick={() => onBatchAction('delete', batchContextMenuId)}
      ><span class="material-icons">delete</span>Delete Batch</button
    >
  </div>
{/if}

<style>
  .context-menu {
    position:fixed;
    background:#0a0f1ef2;
    -webkit-backdrop-filter:blur(12px);
    backdrop-filter:blur(12px);
    border:1px solid rgba(255,255,255,.1);
    border-radius:12px;
    padding:.5rem;
    min-width:220px;
    max-height:calc(100dvh - 20px);
    overflow:auto;
    box-shadow:0 10px 40px #00000080;
    z-index:1000;
    animation:menu-pop .2s cubic-bezier(.175,.885,.32,1.275);
  }
  .context-item {
    width:100%;
    min-height:44px;
    display:flex;
    align-items:center;
    gap:.75rem;
    padding:.75rem 1rem;
    border:none;
    background:transparent;
    color:var(--cui-text-primary);
    font-size:.8rem;
    font-weight:500;
    cursor:pointer;
    border-radius:8px;
    text-align:left;
    transition:all .2s;
  }
  .context-item:hover {
    background:#ffffff0d;
    color:var(--cui-text-primary);
  }
  .context-item:focus-visible {
    outline:2px solid var(--cui-primary);
    outline-offset:-2px;
  }
  .context-link {
    text-decoration:none;
    box-sizing:border-box;
  }
  .context-item .material-icons {
    font-size:18px;
    color:var(--cui-text-tertiary);
  }
  .context-item.danger:hover {
    background:#ff52521a;
    color:var(--cui-danger);
  }
  .context-item.danger:hover .material-icons {
    color:var(--cui-danger);
  }
  .context-divider {
    height:1px;
    background:#ffffff0d;
    margin:.5rem 0;
  }
  .context-note {
    padding:.35rem 1rem .6rem;
    color:var(--cui-text-tertiary);
    font-size:.68rem;
    line-height:1.4;
  }
  .context-header {
    padding:.75rem 1rem;
    font-size:.65rem;
    font-weight:800;
    color:var(--cui-text-tertiary);
    text-transform:uppercase;
    border-bottom:1px solid rgba(255,255,255,.05);
    margin-bottom:.5rem;
    overflow:hidden;
    text-overflow:ellipsis;
    white-space:nowrap;
  }
  @media(max-width:767px),(pointer:coarse)and (max-height:500px) {
    .context-item {
      min-height:44px;
    }
  }
  @keyframes menu-pop {
    0% {
      opacity:0;
      transform:scale(.95);
    }
    to {
      opacity:1;
      transform:scale(1);
    }
  }
</style>
