<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import {
    downloadStore,
    type DownloadState,
    type DownloadTask,
    type PaginationState,
  } from '$lib/stores/downloads.svelte';
  import type { UnifiedTorrentRow } from '$lib/stores/qbitReducer';
  import { wsClient, isConnected, connectionIndicator } from '$lib/stores/websocket';
  import { toasts } from '$lib/stores/toasts';
  import { copyToClipboard } from '$lib/utils/clipboard';
  import {
    buildMediaQueueTree,
    filterMediaQueueGroups,
    groupQueueItemsByMedia,
    mergeQueueRows,
    pickCurrentQueueItem,
  } from '$lib/components/downloads/mediaQueue';
  import {
    buildBatchGroups,
    buildDisplayRows,
    countSources,
    filterBatchGroups,
    filterStandaloneDownloads,
    filterTorrentRows,
    isTorrentTask,
    parseSourceFilter,
    torrentKey,
    type BatchDeleteTarget,
    type BatchGroup,
    type ContextMenuPosition,
    type SortColumn,
    type SourceFilter,
    type StatusFilter,
  } from '$lib/components/downloads/downloadsHelpers';
  import DownloadsToolbar from '$lib/components/downloads/DownloadsToolbar.svelte';
  import DownloadsTable from '$lib/components/downloads/DownloadsTable.svelte';
  import DownloadsMobileView from '$lib/components/downloads/DownloadsMobileView.svelte';
  import DownloadsContextMenus from '$lib/components/downloads/DownloadsContextMenus.svelte';
  import DownloadsModals from '$lib/components/downloads/DownloadsModals.svelte';

  type DownloadAction = 'pause' | 'resume' | 'delete' | 'retry' | 'redownload';
  type BatchAction = 'pause' | 'resume' | 'retry' | 'redownload' | 'delete';

  function apiHeaders(json = false): HeadersInit {
    return json ? { 'Content-Type': 'application/json' } : {};
  }

  // Filters
  let searchQuery = $state('');
  let statusFilter = $state<StatusFilter>('ALL');
  let sourceFilter = $state<SourceFilter>('ALL');
  let mediaTypeFilter = $state<'all' | 'movie' | 'tv' | 'unknown'>('all');
  let queueSort = $state('');
  let selectedIds = $state(new Set<string>()); // reserved for multi-select (unused)

  // Columns (Filename is always visible)
  let visibleColumns = $state(
    new Set(['Filename', 'Source', 'Status', 'Size', 'Progress', 'Speed', 'ETA', 'Date added']),
  );
  let isStatusDropdownOpen = $state(false);
  let isSourceDropdownOpen = $state(false);

  // Row selection / completion highlight
  let selectedRowId = $state<string | null>(null);
  let justCompletedIds = $state(new Set<string>());
  const previousStates = $state(new Map<string, DownloadState>());

  // Batch expansion (desktop) and mobile cards
  let expandedBatches = $state(new Set<string>());
  let expandedMobileCardId = $state<string | null>(null);
  let expandedMobileBatches = $state(new Set<string>());

  // Modals
  let deleteConfirmTask = $state<DownloadTask | null>(null);
  let deleteConfirmBatch = $state<BatchDeleteTarget | null>(null);
  let detailsTask = $state<DownloadTask | null>(null);
  let detailsBatch = $state<BatchGroup | null>(null);

  // Context menus
  let contextMenuId = $state<string | null>(null);
  let contextMenuPos = $state<ContextMenuPosition>({ x: 0, y: 0 });
  let torrentContextMenuId = $state<string | null>(null);
  let torrentContextMenuPos = $state<ContextMenuPosition>({ x: 0, y: 0 });
  let batchContextMenuId = $state<string | null>(null);
  let batchContextMenuPos = $state<ContextMenuPosition>({ x: 0, y: 0 });

  // Long press (mobile context menus)
  let longPressTimer: ReturnType<typeof setTimeout> | null = null;
  let longPressPointerId: number | null = null;
  let longPressStart = { x: 0, y: 0 };
  let longPressFired = $state(false);

  let totalPages = $derived(downloadStore.pagination.totalPages || 1);
  let currentPage = $derived(downloadStore.pagination.page);
  let filterCounts = $derived(downloadStore.statusCounts);
  let sourceCounts = $derived(
    countSources(downloadStore.downloadList, downloadStore.batchList, downloadStore.torrentList),
  );

  // Media-centric view of the queue (used for the screen-reader record count)
  let mediaGroups = $derived(
    filterMediaQueueGroups(
      groupQueueItemsByMedia(
        mergeQueueRows(
          downloadStore.downloadList,
          downloadStore.batchList,
          downloadStore.batchItems,
          downloadStore.torrentList,
          downloadStore.displayRows,
        ),
      ),
      {
        query: searchQuery,
        source: sourceFilter.toLowerCase(),
        status: statusFilter.toLowerCase(),
        mediaType: mediaTypeFilter,
        sort: queueSort,
      },
    ),
  );
  let mediaRecordCount = $derived(buildMediaQueueTree(mediaGroups.map((group) => pickCurrentQueueItem(group.items))).length);

  let batchGroups = $derived(buildBatchGroups(downloadStore.batchList, downloadStore.batchItems, sourceFilter));
  let filteredBatches = $derived(filterBatchGroups(batchGroups.groups, searchQuery, statusFilter));
  let filteredDownloads = $derived(
    filterStandaloneDownloads(
      downloadStore.downloadList,
      searchQuery,
      sourceFilter,
      downloadStore.pagination.sortBy,
      downloadStore.pagination.sortDir,
    ),
  );
  let filteredTorrents = $derived(
    filterTorrentRows(
      downloadStore.torrentList,
      searchQuery,
      sourceFilter,
      statusFilter,
      downloadStore.pagination.sortBy,
      downloadStore.pagination.sortDir,
    ),
  );
  let displayRows = $derived(
    buildDisplayRows(downloadStore.displayRows, filteredBatches, filteredDownloads, filteredTorrents),
  );
  /** Standalone tasks plus every loaded batch item (context menu lookup). */
  let allDownloads = $derived([...downloadStore.downloadList, ...Array.from(downloadStore.batchItems.values()).flat()]);

  // Flash rows that just transitioned to COMPLETED
  $effect(() => {
    for (const task of downloadStore.downloadList) {
      const previous = previousStates.get(task.id);
      if (previous && previous !== 'COMPLETED' && task.state === 'COMPLETED') {
        justCompletedIds = new Set([...justCompletedIds, task.id]);
        setTimeout(() => {
          justCompletedIds = new Set([...justCompletedIds].filter((id) => id !== task.id));
        }, 1500);
      }
      previousStates.set(task.id, task.state);
    }
  });

  onMount(() => {
    const storedStatus = downloadStore.pagination.statusFilter;
    statusFilter = storedStatus || 'ALL';
    sourceFilter = parseSourceFilter($page.url.searchParams.get('source'));
    downloadStore.pagination = {
      ...downloadStore.pagination,
      sourceFilter: sourceFilter === 'ALL' ? null : sourceFilter,
    };

    // ?task=<id> deep link: select and scroll to the row once loaded
    const taskId = $page.url.searchParams.get('task');
    downloadStore
      .fetchBatches()
      .then(() => {
        if (!taskId || !downloadStore.downloadList.some((task) => task.id === taskId)) return;
        selectedRowId = taskId;
        requestAnimationFrame(() =>
          document.querySelector(`[data-download-id="${CSS.escape(taskId)}"]`)?.scrollIntoView({ block: 'center' }),
        );
      })
      .catch((err) => console.error('[Downloads] Failed to refresh on mount:', err));
  });

  function applyStatusFilter(status: string) {
    statusFilter = status;
    isStatusDropdownOpen = false;
    downloadStore.setStatusFilter(status === 'ALL' ? null : (status as DownloadState));
  }

  function applySourceFilter(source: string, updateUrl = true) {
    const next = parseSourceFilter(source);
    sourceFilter = next;
    if (updateUrl) {
      const url = new URL($page.url);
      if (next === 'ALL') url.searchParams.delete('source');
      else url.searchParams.set('source', next);
      goto(`${url.pathname}${url.search}`, { replaceState: true, noScroll: true });
    }
    downloadStore.setSourceFilter(next === 'ALL' ? null : next);
  }

  function toggleSort(column: SortColumn) {
    const currentSortBy = downloadStore.pagination.sortBy;
    const currentSortDir = downloadStore.pagination.sortDir;
    downloadStore.setSort(
      column as PaginationState['sortBy'],
      currentSortBy === column ? (currentSortDir === 'asc' ? 'desc' : 'asc') : 'desc',
    );
  }

  function setPage(p: number) {
    if (p >= 1 && p <= totalPages) downloadStore.setPage(p);
  }

  function clearFilters() {
    searchQuery = '';
    mediaTypeFilter = 'all';
    queueSort = '';
    applyStatusFilter('ALL');
    applySourceFilter('ALL');
  }

  function toggleColumn(column: string) {
    const next = new Set(visibleColumns);
    if (next.has(column)) next.delete(column);
    else next.add(column);
    visibleColumns = next;
  }

  function hideContextMenu() {
    contextMenuId = null;
    batchContextMenuId = null;
    torrentContextMenuId = null;
  }

  function showContextMenu(e: MouseEvent, id: string) {
    e.preventDefault();
    contextMenuId = id;
    contextMenuPos = { x: e.clientX, y: e.clientY };
  }

  function showTorrentContextMenu(e: MouseEvent, torrent: UnifiedTorrentRow) {
    e.preventDefault();
    e.stopPropagation();
    torrentContextMenuId = torrentKey(torrent);
    torrentContextMenuPos = { x: e.clientX, y: e.clientY };
  }

  function showBatchContextMenu(e: MouseEvent, batchId: string) {
    e.preventDefault();
    e.stopPropagation();
    batchContextMenuId = batchId;
    batchContextMenuPos = { x: e.clientX, y: e.clientY };
  }

  async function handleAction(action: DownloadAction, id: string) {
    const actions = {
      pause: () => downloadStore.pauseDownload(id),
      resume: () => downloadStore.resumeDownload(id),
      delete: () => downloadStore.deleteDownload(id),
      retry: () => downloadStore.retryDownload(id),
      redownload: () => downloadStore.redownload(id),
    };
    const run = actions[action];
    if (!run) return;
    const result = await run();
    if (!result.success) toasts.error(`Failed to ${action}: ${result.error}`);
    hideContextMenu();
  }

  async function handleTorrentAction(action: 'copy', torrent: UnifiedTorrentRow) {
    if (action === 'copy') {
      if (await copyToClipboard(torrent.hash)) toasts.success('Torrent hash copied');
      else toasts.error('Failed to copy torrent hash');
      hideContextMenu();
    }
  }

  async function handleBatchAction(action: BatchAction, batchId: string) {
    try {
      if (action === 'delete') {
        const batch = filteredBatches.find((group) => group.batchId === batchId);
        if (batch) deleteConfirmBatch = { batchId, batchName: batch.batchName, itemCount: batch.totalItems };
        hideContextMenu();
        return;
      }
      if (action === 'pause') {
        const response = await fetch(`/api/downloads/batch/${batchId}/pause`, { method: 'POST', headers: apiHeaders() });
        if (response.ok) toasts.success('Batch paused');
      } else if (action === 'resume') {
        const response = await fetch(`/api/downloads/batch/${batchId}/resume`, { method: 'POST', headers: apiHeaders() });
        if (response.ok) toasts.success('Batch resumed');
      } else if (action === 'redownload') {
        await downloadStore.redownloadBatch(batchId);
      }
    } catch (err) {
      toasts.error(`Batch action failed: ${err}`);
    }
    hideContextMenu();
  }

  /** Copy what identifies the source: torrent hash for torrents, the link otherwise. */
  async function copySafeSource(task: DownloadTask) {
    if (isTorrentTask(task)) {
      if (!task.external_id) toasts.error('Torrent hash not available yet');
      else if (await copyToClipboard(task.external_id)) toasts.success('Torrent hash copied');
      else toasts.error('Failed to copy torrent hash');
      return;
    }
    if (await copyToClipboard(task.url || task.original_url || '')) toasts.success('Link copied');
    else toasts.error('Failed to copy link');
  }

  async function copyError(message: string) {
    if (await copyToClipboard(message)) toasts.success('Error message copied to clipboard');
    else toasts.error('Failed to copy error message');
    hideContextMenu();
  }

  function openMediaInfo(task: DownloadTask) {
    if (!task.info_path) {
      toasts.error('No media info available for this download');
      return;
    }
    window.location.href = task.info_path;
  }

  async function pauseAll() {
    const result = await downloadStore.pauseAll();
    if (!result.success) toasts.error(`Failed to pause all: ${result.error}`);
  }

  async function resumeAll() {
    const result = await downloadStore.resumeAll();
    if (!result.success) toasts.error(`Failed to resume all: ${result.error}`);
  }

  async function confirmDelete() {
    if (!deleteConfirmTask) return;
    const id = deleteConfirmTask.id;
    deleteConfirmTask = null;
    const result = await downloadStore.deleteDownload(id);
    if (!result.success) toasts.error(`Failed to delete: ${result.error}`);
  }

  async function confirmBatchDelete() {
    if (!deleteConfirmBatch) return;
    const { batchId, batchName } = deleteConfirmBatch;
    deleteConfirmBatch = null;
    try {
      const response = await fetch(`/api/downloads/batch/${batchId}`, { method: 'DELETE', headers: apiHeaders() });
      if (response.ok) toasts.success(`Batch "${batchName}" deleted`);
      else toasts.error('Failed to delete batch');
    } catch (err) {
      toasts.error(`Batch deletion failed: ${err}`);
    }
  }

  async function toggleBatch(batchId: string) {
    if (expandedBatches.has(batchId)) {
      expandedBatches = new Set([...expandedBatches].filter((id) => id !== batchId));
    } else {
      expandedBatches = new Set([...expandedBatches, batchId]);
      // Lazy-load batch items
      await downloadStore.fetchBatchItems(batchId).catch((err) => {
        console.error('Failed to load batch items:', err);
        toasts.error('Failed to load batch items');
      });
    }
  }

  async function toggleMobileBatch(batchId: string) {
    if (expandedMobileBatches.has(batchId)) {
      expandedMobileBatches = new Set([...expandedMobileBatches].filter((id) => id !== batchId));
    } else {
      expandedMobileBatches = new Set([...expandedMobileBatches, batchId]);
      await downloadStore.fetchBatchItems(batchId).catch((err) => console.error('Failed to load batch items:', err));
    }
  }

  function toggleMobileCard(id: string) {
    expandedMobileCardId = expandedMobileCardId === id ? null : id;
  }

  /** Start a 500ms long press that opens a context menu at the press point. */
  function startLongPress(e: PointerEvent, open: (e: MouseEvent) => void) {
    if ((e.target as HTMLElement | null)?.closest('button, a, input, select, textarea')) return;
    if (longPressPointerId !== null) {
      cancelLongPress();
      return;
    }
    if (!e.isPrimary) return;
    longPressFired = false;
    longPressPointerId = e.pointerId;
    longPressStart = { x: e.clientX, y: e.clientY };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    longPressTimer = setTimeout(() => {
      longPressTimer = null;
      longPressFired = true;
      open({
        clientX: longPressStart.x,
        clientY: longPressStart.y,
        preventDefault: () => {},
        stopPropagation: () => {},
      } as MouseEvent);
    }, 500);
  }

  /** Moving more than 10px cancels the long press. */
  function moveLongPress(e: PointerEvent) {
    if (e.pointerId !== longPressPointerId) return;
    if (Math.hypot(e.clientX - longPressStart.x, e.clientY - longPressStart.y) <= 10) return;
    cancelLongPress();
  }

  function cancelLongPress(e?: PointerEvent) {
    if (e && e.pointerId !== longPressPointerId) return;
    if (longPressTimer) clearTimeout(longPressTimer);
    longPressTimer = null;
    longPressPointerId = null;
  }

  function resetLongPressFired() {
    longPressFired = false;
  }

  onMount(() => {
    const onScroll = () => cancelLongPress();
    window.addEventListener('scroll', onScroll, true);
    return () => window.removeEventListener('scroll', onScroll, true);
  });

  function isEditableTarget(target: EventTarget | null): boolean {
    return !!(target as HTMLElement | null)?.closest('input, textarea, select, button, a, [contenteditable="true"]');
  }

  // Keyboard shortcuts on the standalone rows
  function handleKeyboard(e: KeyboardEvent) {
    if (isEditableTarget(e.target)) return;
    const list = filteredDownloads;
    if (!list.length) return;
    const index = selectedRowId ? list.findIndex((task) => task.id === selectedRowId) : -1;

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        if (index < list.length - 1) selectedRowId = list[index + 1].id;
        else if (index === -1) selectedRowId = list[0].id;
        break;
      case 'ArrowUp':
        e.preventDefault();
        if (index > 0) selectedRowId = list[index - 1].id;
        break;
      case ' ': {
        e.preventDefault();
        if (selectedRowId) {
          const task = list.find((t) => t.id === selectedRowId);
          if (task?.state === 'DOWNLOADING' || task?.state === 'STARTING') handleAction('pause', selectedRowId);
          else if (task?.state === 'PAUSED' || task?.state === 'QUEUED') handleAction('resume', selectedRowId);
        }
        break;
      }
      case 'Delete':
      case 'Backspace': {
        e.preventDefault();
        if (selectedRowId) {
          const task = list.find((t) => t.id === selectedRowId);
          if (task) deleteConfirmTask = task;
        }
        break;
      }
      case 'e':
      case 'E': {
        // Cycle through failed downloads
        e.preventDefault();
        const failed = list.filter((t) => t.state === 'FAILED');
        if (failed.length) {
          const current = selectedRowId ? failed.findIndex((t) => t.id === selectedRowId) : -1;
          const next = (current + 1) % failed.length;
          selectedRowId = failed[next].id;
          toasts.info(`Error ${next + 1} of ${failed.length}`);
        }
        break;
      }
      case 'r':
      case 'R':
        e.preventDefault();
        if (selectedRowId && list.find((t) => t.id === selectedRowId)?.state === 'FAILED') {
          handleAction('retry', selectedRowId);
        }
        break;
      case 'b': {
        e.preventDefault();
        if (selectedRowId) {
          const task = list.find((t) => t.id === selectedRowId);
          if (task?.batch_id) {
            toggleBatch(task.batch_id);
            toasts.info(expandedBatches.has(task.batch_id) ? 'Batch collapsed' : 'Batch expanded');
          }
        }
        break;
      }
      case 'B': {
        e.preventDefault();
        if (selectedRowId) {
          const task = list.find((t) => t.id === selectedRowId);
          if (task?.batch_id) {
            const batch = filteredBatches.find((group) => group.batchId === task.batch_id);
            if (batch) {
              if (batch.pausedCount > 0) {
                downloadStore.resumeBatch(batch.batchId);
                toasts.success('Batch resumed');
              } else if (batch.activeCount > 0 || batch.queuedCount > 0) {
                downloadStore.pauseBatch(batch.batchId);
                toasts.success('Batch paused');
              }
            }
          }
        }
        break;
      }
      case 'Escape':
        selectedRowId = null;
        hideContextMenu();
        break;
    }
  }
</script>

<svelte:head>
  <title>Downloads - Flasharr</title>
</svelte:head>

<div
  class="downloads-container"
  role="region"
  aria-label="Downloads queue"
  onclick={hideContextMenu}
  onkeydown={(e) => {
    handleKeyboard(e);
    if (e.key === 'Escape') hideContextMenu();
  }}
>
  <span class="sr-only" role="status" aria-live="polite" aria-label={`${mediaRecordCount} top-level media records`}
    >{mediaRecordCount} media</span
  >
  <div class="section-box bento-card">
    <DownloadsToolbar
      connectionText={$connectionIndicator.text}
      connected={$isConnected}
      onReconnect={() => wsClient.connect()}
      onResumeAll={resumeAll}
      onPauseAll={pauseAll}
      onRefresh={() => downloadStore.fetchAll()}
      {sourceFilter}
      {statusFilter}
      mediaTypeFilter={mediaTypeFilter}
      sort={queueSort}
      loadedCount={downloadStore.downloadList.length}
      onSourceFilter={applySourceFilter}
      onStatusFilter={applyStatusFilter}
      onMediaTypeFilter={(mediaType) => (mediaTypeFilter = mediaType as typeof mediaTypeFilter)}
      onSort={(sort) => (queueSort = sort)}
      onClear={clearFilters}
      {visibleColumns}
      onToggleColumn={toggleColumn}
      bind:searchQuery
    />
    <div class="downloads-content">
      <DownloadsTable
        {displayRows}
        {visibleColumns}
        {expandedBatches}
        {selectedRowId}
        {justCompletedIds}
        {statusFilter}
        {sourceFilter}
        {searchQuery}
        {filterCounts}
        {sourceCounts}
        {isStatusDropdownOpen}
        {isSourceDropdownOpen}
        loading={downloadStore.loading}
        error={downloadStore.error}
        {currentPage}
        {totalPages}
        onSort={toggleSort}
        onSourceFilter={applySourceFilter}
        onSourceDropdownToggle={(open) => (isSourceDropdownOpen = open)}
        onStatusFilter={applyStatusFilter}
        onStatusDropdownToggle={(open) => (isStatusDropdownOpen = open)}
        onRowSelect={(id) => (selectedRowId = id)}
        onClearStatus={() => applyStatusFilter('ALL')}
        onClearSearch={() => (searchQuery = '')}
        onPage={setPage}
        onToggleBatch={toggleBatch}
        onBatchContextMenu={showBatchContextMenu}
        onDownloadContextMenu={showContextMenu}
        onTorrentContextMenu={showTorrentContextMenu}
      />
      <DownloadsMobileView
        {displayRows}
        {expandedMobileCardId}
        {expandedMobileBatches}
        {statusFilter}
        {searchQuery}
        {filterCounts}
        {longPressFired}
        {currentPage}
        {totalPages}
        onStatusFilter={applyStatusFilter}
        onToggleMobileCard={toggleMobileCard}
        onToggleMobileBatch={toggleMobileBatch}
        onStartCardLongPress={(e, id) => startLongPress(e, (me) => showContextMenu(me, id))}
        onStartBatchLongPress={(e, batchId) => startLongPress(e, (me) => showBatchContextMenu(me, batchId))}
        onStartTorrentLongPress={(e, torrent) => startLongPress(e, (me) => showTorrentContextMenu(me, torrent))}
        onMoveLongPress={moveLongPress}
        onCancelLongPress={cancelLongPress}
        onAction={handleAction}
        onTorrentAction={handleTorrentAction}
        onOpenInfo={openMediaInfo}
        onShowTaskDetails={(task) => (detailsTask = task)}
        onShowDeleteConfirm={(task) => (deleteConfirmTask = task)}
        onCopySafeSource={copySafeSource}
        onPage={setPage}
        onResumeAll={resumeAll}
        onPauseAll={pauseAll}
        onRefresh={() => downloadStore.fetchAll()}
        onLongPressFiredReset={resetLongPressFired}
      />
      {#if downloadStore.error}
        <div class="downloads-error" role="alert">{downloadStore.error}</div>
      {/if}
    </div>
  </div>
</div>

<DownloadsContextMenus
  {contextMenuId}
  {contextMenuPos}
  {torrentContextMenuId}
  {torrentContextMenuPos}
  {batchContextMenuId}
  {batchContextMenuPos}
  {allDownloads}
  torrentRows={downloadStore.torrentRows}
  {filteredBatches}
  onAction={handleAction}
  onOpenInfo={openMediaInfo}
  onShowTaskDetails={(task) => {
    detailsTask = task;
    hideContextMenu();
  }}
  onCopySafeSource={(task) => {
    copySafeSource(task);
  }}
  onCopyError={copyError}
  onShowDeleteConfirm={(task) => {
    deleteConfirmTask = task;
    hideContextMenu();
  }}
  onTorrentAction={handleTorrentAction}
  onBatchAction={handleBatchAction}
  onShowBatchDetails={(batch) => {
    detailsBatch = batch;
    hideContextMenu();
  }}
  onHide={hideContextMenu}
/>

<DownloadsModals
  {deleteConfirmTask}
  {deleteConfirmBatch}
  {detailsTask}
  {detailsBatch}
  selectedCount={0}
  onHideDeleteConfirm={() => (deleteConfirmTask = null)}
  onHideBatchDeleteConfirm={() => (deleteConfirmBatch = null)}
  onHideTaskDetails={() => (detailsTask = null)}
  onHideBatchDetails={() => (detailsBatch = null)}
  onConfirmDelete={confirmDelete}
  onConfirmBatchDelete={confirmBatchDelete}
  onBatchAction={handleBatchAction}
  onCopySafeSource={copySafeSource}
/>

<style>
  .downloads-container {
    padding:24px;
    height:100%;
    box-sizing:border-box;
    display:flex;
    flex-direction:column;
    overflow:hidden;
  }
  .sr-only {
    position:absolute;
    width:1px;
    height:1px;
    padding:0;
    margin:-1px;
    overflow:hidden;
    clip:rect(0,0,0,0);
    white-space:nowrap;
    border:0;
  }
  .downloads-error {
    margin-top:.75rem;
    padding:.85rem 1rem;
    border:1px solid var(--cui-danger);
    border-radius:var(--cui-radius-md);
    background:#ff525214;
    color:var(--cui-danger);
    font-size:.8rem;
  }
  .section-box {
    background:var(--cui-glass);
    border:1px solid var(--cui-glass-border);
    border-radius:var(--cui-radius-lg);
    padding:0;
    overflow:hidden;
    display:flex;
    flex-direction:column;
    flex:1;
    min-height:0;
  }
  .downloads-content {
    flex:1;
    overflow:hidden;
    display:flex;
    flex-direction:column;
    min-height:0;
  }
  @media(max-width:768px) {
    .downloads-container {
      padding:1rem 1rem max(1rem,env(safe-area-inset-bottom,0px));
      height:auto;
    }
    .downloads-content {
      display:flex;
      flex-direction:column;
    }
    .section-box {
      display:flex;
    }
  }
  @media(prefers-reduced-motion:reduce) {
    .downloads-container :global(*),
    .downloads-container :global(*:before),
    .downloads-container :global(*:after) {
      animation:none!important;
      transition:none!important;
      scroll-behavior:auto!important;
    }
  }
</style>
