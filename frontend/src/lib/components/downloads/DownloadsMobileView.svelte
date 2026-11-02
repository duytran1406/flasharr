<script lang="ts">
  import { Badge } from '@media-set/core-ui';
  import { formatBytes, type DownloadTask, type StatusCounts } from '$lib/stores/downloads.svelte';
  import type { UnifiedTorrentRow } from '$lib/stores/qbitReducer';
  import {
    getBatchStatus,
    getStateColor,
    isActiveState,
    normalizeSource,
    sourceSubline,
    taskSource,
    torrentKey,
    torrentState,
    type DisplayRow,
    type StatusFilter,
  } from './downloadsHelpers';

  type DownloadAction = 'pause' | 'resume' | 'retry' | 'redownload' | 'delete';

  interface Props {
    displayRows: DisplayRow[];
    expandedMobileCardId: string | null;
    expandedMobileBatches: Set<string>;
    statusFilter: StatusFilter;
    searchQuery: string;
    filterCounts: StatusCounts;
    longPressFired: boolean;
    currentPage: number;
    totalPages: number;
    onStatusFilter: (status: string) => void;
    onToggleMobileCard: (id: string) => void;
    onToggleMobileBatch: (batchId: string) => void;
    onStartCardLongPress: (e: PointerEvent, id: string) => void;
    onStartBatchLongPress: (e: PointerEvent, batchId: string) => void;
    onStartTorrentLongPress: (e: PointerEvent, torrent: UnifiedTorrentRow) => void;
    onMoveLongPress?: (e: PointerEvent) => void;
    onCancelLongPress?: (e: PointerEvent) => void;
    onAction: (action: DownloadAction, id: string) => void;
    onTorrentAction: (action: 'copy', torrent: UnifiedTorrentRow) => void;
    onOpenInfo: (task: DownloadTask) => void;
    onShowTaskDetails: (task: DownloadTask) => void;
    onShowDeleteConfirm: (task: DownloadTask) => void;
    onCopySafeSource: (task: DownloadTask) => void;
    onPage: (page: number) => void;
    onResumeAll?: () => void;
    onPauseAll?: () => void;
    onRefresh?: () => void;
    onLongPressFiredReset: () => void;
  }

  let {
    displayRows,
    expandedMobileCardId,
    expandedMobileBatches,
    statusFilter,
    searchQuery,
    filterCounts,
    longPressFired,
    currentPage,
    totalPages,
    onStatusFilter,
    onToggleMobileCard,
    onToggleMobileBatch,
    onStartCardLongPress,
    onStartBatchLongPress,
    onStartTorrentLongPress,
    onMoveLongPress,
    onCancelLongPress,
    onAction,
    onTorrentAction,
    onOpenInfo,
    onShowTaskDetails,
    onShowDeleteConfirm,
    onCopySafeSource,
    onPage,
    onResumeAll,
    onPauseAll,
    onRefresh,
    onLongPressFiredReset,
  }: Props = $props();

  type RowOf<K extends DisplayRow['kind']> = Extract<DisplayRow, { kind: K }>;

  let batchRows = $derived(displayRows.filter((row): row is RowOf<'batch'> => row.kind === 'batch'));
  let torrentRows = $derived(displayRows.filter((row): row is RowOf<'torrent'> => row.kind === 'torrent'));
  let downloadRows = $derived(displayRows.filter((row): row is RowOf<'download'> => row.kind === 'download'));

  const STATUS_CHIPS: { label: string; key: string; count: (c: StatusCounts) => number }[] = [
    { label: 'All', key: 'ALL', count: (c) => c.all },
    { label: 'Active', key: 'DOWNLOADING', count: (c) => c.downloading },
    { label: 'Queued', key: 'QUEUED', count: (c) => c.queued },
    { label: 'Paused', key: 'PAUSED', count: (c) => c.paused },
    { label: 'Done', key: 'COMPLETED', count: (c) => c.completed },
    { label: 'Failed', key: 'FAILED', count: (c) => c.failed },
  ];

  /** "More" toggles the card unless a long press just opened the context menu. */
  function handleMoreClick(e: MouseEvent, id: string) {
    e.stopPropagation();
    if (longPressFired) {
      e.preventDefault();
      onLongPressFiredReset();
      return;
    }
    onToggleMobileCard(id);
  }
</script>

{#snippet primaryAction(task: DownloadTask)}
  {#if task.state === 'DOWNLOADING' || task.state === 'STARTING'}
    <button
      class="mobile-action-btn action-pause"
      onclick={(e) => {
        e.stopPropagation();
        onAction('pause', task.id);
      }}><span class="material-icons">pause</span>Pause</button
    >
  {:else if task.state === 'PAUSED'}
    <button
      class="mobile-action-btn action-resume"
      onclick={(e) => {
        e.stopPropagation();
        onAction('resume', task.id);
      }}><span class="material-icons">play_arrow</span>Resume</button
    >
  {:else if task.state === 'FAILED'}
    <button
      class="mobile-action-btn action-retry"
      onclick={(e) => {
        e.stopPropagation();
        onAction('retry', task.id);
      }}><span class="material-icons">refresh</span>Retry</button
    >
  {:else if task.state === 'QUEUED' || task.state === 'WAITING'}
    <button
      class="mobile-action-btn action-start"
      onclick={(e) => {
        e.stopPropagation();
        onAction('resume', task.id);
      }}><span class="material-icons">play_arrow</span>Start</button
    >
  {/if}
{/snippet}

{#snippet redownloadAction(task: DownloadTask)}
  {#if task.state === 'COMPLETED' || task.state === 'FAILED' || task.state === 'PAUSED' || task.state === 'CANCELLED'}
    <button
      class="mobile-action-btn"
      style="color:var(--cui-secondary)"
      onclick={(e) => {
        e.stopPropagation();
        onAction('redownload', task.id);
      }}><span class="material-icons">cloud_download</span>Re-download</button
    >
  {/if}
{/snippet}

<div class="mobile-cards">
  <div class="mobile-status-chips">
    {#each STATUS_CHIPS as chip}
      <button class="mobile-status-chip" class:active={statusFilter === chip.key} onclick={() => onStatusFilter(chip.key)}
        >{chip.label} <span class="chip-count">({chip.count(filterCounts)})</span></button
      >
    {/each}
  </div>

  {#each batchRows as row (row.id)}
    {@const group = row.group}
    {@const batchStatus = getBatchStatus(group)}
    {@const expanded = expandedMobileBatches.has(group.batchId)}
    <div class="mobile-batch-card">
      <button
        type="button"
        class="mobile-batch-header"
        aria-expanded={expanded}
        aria-controls={`mobile-batch-${group.batchId}`}
        onclick={(e) => {
          if (longPressFired) {
            e.preventDefault();
            onLongPressFiredReset();
            e.stopPropagation();
            return;
          }
          onToggleMobileBatch(group.batchId);
        }}
        onpointerdown={(e) => onStartBatchLongPress(e, group.batchId)}
        onpointermove={onMoveLongPress}
        onpointerup={onCancelLongPress}
        onpointercancel={onCancelLongPress}
        onlostpointercapture={onCancelLongPress}
      >
        <div class="batch-header-top">
          <span class="material-icons expand-icon">{expanded ? 'expand_less' : 'expand_more'}</span>
          <span class="batch-name">{group.batchName}</span>
          <span
            class="status-badge-v2"
            style="background:{batchStatus.color}15;color:{batchStatus.color};border-color:{batchStatus.color}30;flex-shrink:0;"
            >{group.completedCount}/{group.totalItems}</span
          >
        </div>
        <div class="batch-progress-mobile">
          <div class="batch-progress-bar-mobile">
            <div class="batch-progress-fill-mobile" style="width:{group.progress}%;background:{batchStatus.color};"></div>
          </div>
          <div class="batch-meta-mobile">
            <span class="batch-pct-mobile">{Math.round(group.progress)}%</span>
            <span class="source-badge">{normalizeSource(group.source)}</span>
            {#if group.activeCount > 0}
              <span class="batch-speed-mobile" style="color:{batchStatus.color};">{formatBytes(group.speed)}/s</span>
            {:else}
              <span class="batch-count-mobile">{group.totalItems} files</span>
            {/if}
          </div>
          <div class="mobile-subline">{sourceSubline(group.source)}</div>
        </div>
      </button>
      {#if expanded}
        <div class="mobile-batch-children" id={`mobile-batch-${group.batchId}`}>
          {#each group.downloads as task (task.id)}
            {@const stateColor = getStateColor(task.state)}
            {@const active = isActiveState(task.state)}
            {@const cardExpanded = expandedMobileCardId === task.id}
            <div class="download-card-mobile" class:expanded={cardExpanded} role="group">
              <div
                class="card-header-mobile"
                role="presentation"
                onpointerdown={(e) => onStartCardLongPress(e, task.id)}
                onpointermove={onMoveLongPress}
                onpointerup={onCancelLongPress}
                onpointercancel={onCancelLongPress}
                onlostpointercapture={onCancelLongPress}
              >
                <div class="card-name">
                  {task.filename}
                  {#if task.quality}
                    <Badge variant="purple" size="xs" class="ml-[6px]" title={task.quality}>
                      {task.resolution || task.quality}
                    </Badge>
                  {/if}
                </div>
                <span
                  class="status-badge-v2"
                  style="background:{stateColor}15;color:{stateColor};border-color:{stateColor}30;">{task.state}</span
                >
                <button
                  class="mobile-more-btn"
                  aria-label={`More actions for ${task.filename}`}
                  aria-expanded={cardExpanded}
                  onclick={(e) => handleMoreClick(e, task.id)}>More</button
                >
              </div>
              <div class="mobile-subline">{sourceSubline(taskSource(task))}</div>
              <div class="card-progress-mobile">
                <div class="progress-fill-v2" style="width:{task.progress}%;background:{stateColor};"></div>
              </div>
              <div class="card-meta-mobile">
                <div class="meta-item"><span class="material-icons">storage</span><span>{formatBytes(task.size)}</span></div>
                <div class="meta-item">
                  <span class="material-icons">speed</span><span style="color:{stateColor};"
                    >{active ? `${formatBytes(task.speed || 0)}/s` : '-'}</span
                  >
                </div>
                <div class="meta-item"><span>{Math.round(task.progress)}%</span></div>
                <div class="meta-item"><span class="source-badge">{taskSource(task)}</span></div>
              </div>
              {#if task.error_message}
                <div class="card-error-mobile">⚠ {task.error_message}</div>
              {/if}
              <div class="mobile-card-actions">
                {@render primaryAction(task)}
                {@render redownloadAction(task)}
                <button
                  class="mobile-action-btn"
                  onclick={(e) => {
                    e.stopPropagation();
                    onOpenInfo(task);
                  }}><span class="material-icons">open_in_new</span>Info</button
                >
                <button
                  class="mobile-action-btn action-details"
                  onclick={(e) => {
                    e.stopPropagation();
                    onShowTaskDetails(task);
                  }}><span class="material-icons">info_outline</span>Details</button
                >
                <button
                  class="mobile-action-btn action-delete"
                  onclick={(e) => {
                    e.stopPropagation();
                    onShowDeleteConfirm(task);
                  }}><span class="material-icons">delete_outline</span>Delete</button
                >
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/each}

  {#each torrentRows as row (row.id)}
    {@const torrent = row.torrent}
    {@const state = torrentState(torrent)}
    {@const stateColor = getStateColor(state)}
    {@const key = torrentKey(torrent)}
    {@const downloading = state === 'DOWNLOADING' || state === 'STARTING'}
    {@const cardExpanded = expandedMobileCardId === key}
    <div class="download-card-mobile torrent-mobile-card" class:expanded={cardExpanded} role="group">
      <div
        class="card-header-mobile"
        role="presentation"
        onpointerdown={(e) => onStartTorrentLongPress(e, torrent)}
        onpointermove={onMoveLongPress}
        onpointerup={onCancelLongPress}
        onpointercancel={onCancelLongPress}
        onlostpointercapture={onCancelLongPress}
      >
        <div class="card-name">
          {torrent.name}
          {#if torrent.category}<span class="torrent-cat-badge">{torrent.category}</span>{/if}
        </div>
        <span class="status-badge-v2" style="background:{stateColor}15;color:{stateColor};border-color:{stateColor}30;"
          >{torrent.state.toLowerCase().includes('up') ? 'SEEDING' : state}</span
        >
        <button
          class="mobile-more-btn"
          aria-label={`More actions for ${torrent.name}`}
          aria-expanded={cardExpanded}
          onclick={(e) => handleMoreClick(e, key)}>More</button
        >
      </div>
      <div class="card-progress-mobile">
        <div class="progress-fill-v2" style="width:{torrent.progress * 100}%;background:{stateColor};"></div>
      </div>
      <div class="card-meta-mobile">
        <div class="meta-item">
          <span class="material-icons">storage</span><span>{formatBytes(Math.max(0, torrent.size))}</span>
        </div>
        <div class="meta-item">
          <span class="material-icons">speed</span><span style="color:{stateColor};"
            >{downloading
              ? `${formatBytes(torrent.dlspeed)}/s`
              : torrent.upspeed > 0
                ? `↑${formatBytes(torrent.upspeed)}/s`
                : '-'}</span
          >
        </div>
        <div class="meta-item"><span>{Math.round(torrent.progress * 100)}%</span></div>
        <div class="meta-item"><span class="source-badge source-badge-torrent">torrent</span></div>
      </div>
      <div class="mobile-subline">Torrent-backed queue item. Advanced controls live in Torrents.</div>
      <div class="mobile-card-actions">
        {#if cardExpanded}
          <button
            class="mobile-action-btn"
            onclick={(e) => {
              e.stopPropagation();
              onTorrentAction('copy', torrent);
            }}><span class="material-icons">content_copy</span>Copy Hash</button
          >
          <a class="mobile-action-btn action-details" href="/torrents" onclick={(e) => e.stopPropagation()}
            ><span class="material-icons">open_in_new</span>Open in Torrents</a
          >
        {/if}
      </div>
    </div>
  {/each}

  {#each downloadRows as row (row.id)}
    {@const task = row.download}
    {@const stateColor = getStateColor(task.state)}
    {@const active = isActiveState(task.state)}
    {@const cardExpanded = expandedMobileCardId === task.id}
    <div class="download-card-mobile" class:expanded={cardExpanded} role="group">
      <div
        class="card-header-mobile"
        role="presentation"
        onpointerdown={(e) => onStartCardLongPress(e, task.id)}
        onpointermove={onMoveLongPress}
        onpointerup={onCancelLongPress}
        onpointercancel={onCancelLongPress}
        onlostpointercapture={onCancelLongPress}
      >
        <div class="card-name">
          {task.filename}
          {#if task.quality}
            <Badge variant="purple" size="xs" class="ml-[6px]" title={task.quality}>
              {task.resolution || task.quality}
            </Badge>
          {/if}
        </div>
        <span class="status-badge-v2" style="background:{stateColor}15;color:{stateColor};border-color:{stateColor}30;"
          >{task.state}</span
        >
        <button
          class="mobile-more-btn"
          aria-label={`More actions for ${task.filename}`}
          aria-expanded={cardExpanded}
          onclick={(e) => handleMoreClick(e, task.id)}>More</button
        >
      </div>
      <div class="mobile-subline">{sourceSubline(taskSource(task))}</div>
      <div class="card-progress-mobile">
        <div class="progress-fill-v2" style="width:{task.progress}%;background:{stateColor};"></div>
      </div>
      <div class="card-meta-mobile">
        <div class="meta-item"><span class="material-icons">storage</span><span>{formatBytes(task.size)}</span></div>
        <div class="meta-item">
          <span class="material-icons">speed</span><span style="color:{stateColor};"
            >{active ? `${formatBytes(task.speed || 0)}/s` : '-'}</span
          >
        </div>
        <div class="meta-item"><span>{Math.round(task.progress)}%</span></div>
        <div class="meta-item"><span class="source-badge">{taskSource(task)}</span></div>
      </div>
      {#if task.error_message}
        <div class="card-error-mobile">⚠ {task.error_message}</div>
      {/if}
      <div class="mobile-card-actions">
        {@render primaryAction(task)}
        {@render redownloadAction(task)}
        <button
          class="mobile-action-btn"
          onclick={(e) => {
            e.stopPropagation();
            onCopySafeSource(task);
          }}><span class="material-icons">link</span>Copy Link</button
        >
        <button
          class="mobile-action-btn action-details"
          onclick={(e) => {
            e.stopPropagation();
            onShowTaskDetails(task);
          }}><span class="material-icons">info_outline</span>Details</button
        >
        <button
          class="mobile-action-btn action-delete"
          onclick={(e) => {
            e.stopPropagation();
            onShowDeleteConfirm(task);
          }}><span class="material-icons">delete_outline</span>Delete</button
        >
      </div>
    </div>
  {/each}

  {#if displayRows.length === 0}
    <div class="empty-state-inline" style="padding:2rem;text-align:center;">
      <span class="material-icons" style="font-size:36px;opacity:.3;">inbox</span>
      <p style="margin-top:.5rem;color:var(--cui-text-tertiary);font-size:.8rem;">
        {#if statusFilter !== 'ALL'}
          No {statusFilter.toLowerCase()} downloads. Show all to continue.
        {:else if searchQuery}
          No results for "{searchQuery}".
        {:else}
          Queue empty. Add from Search or Discover; torrent-backed tasks appear here too.
        {/if}
      </p>
    </div>
  {/if}

  <div class="mobile-bottom-bar">
    <div class="mobile-bar-actions">
      <button class="mobile-bar-btn btn-success" title="Resume" onclick={onResumeAll}
        ><span class="material-icons">play_arrow</span></button
      >
      <button class="mobile-bar-btn btn-danger" title="Pause" onclick={onPauseAll}
        ><span class="material-icons">pause</span></button
      >
      <button class="mobile-bar-btn" title="Refresh" onclick={onRefresh}><span class="material-icons">refresh</span></button>
    </div>
    <div class="mobile-bar-divider"></div>
    <div class="mobile-bar-pagination">
      <button
        class="mobile-bar-page-btn"
        aria-label="Previous page"
        disabled={currentPage === 1}
        onclick={() => onPage(currentPage - 1)}><span class="material-icons">chevron_left</span></button
      >
      <span class="mobile-bar-page-info">{currentPage} / {totalPages}</span>
      <button
        class="mobile-bar-page-btn"
        aria-label="Next page"
        disabled={currentPage === totalPages}
        onclick={() => onPage(currentPage + 1)}><span class="material-icons">chevron_right</span></button
      >
    </div>
  </div>
</div>

<style>
  .mobile-cards {
    display:none;
  }
  .status-badge-v2 {
    display:inline-flex;
    align-items:center;
    gap:5px;
    padding:2px 7px;
    font-size:.55rem;
    font-weight:800;
    border:1px solid;
    border-radius:999px;
    text-transform:uppercase;
    font-family:var(--cui-font-mono);
    letter-spacing:.07em;
  }
  .status-badge-v2:before {
    content:"";
    display:inline-block;
    width:4px;
    height:4px;
    border-radius:50%;
    background:currentColor;
    opacity:.8;
    flex-shrink:0;
  }
  .source-badge {
    display:inline-flex;
    align-items:center;
    padding:2px 7px;
    border:1px solid rgba(0,243,255,.24);
    border-radius:999px;
    background:#00f3ff14;
    color:var(--cui-secondary);
    font-size:.55rem;
    font-weight:800;
    letter-spacing:.07em;
    text-transform:uppercase;
    font-family:var(--cui-font-mono);
    white-space:nowrap;
  }
  .source-badge-torrent {
    border-color:#00ffa347;
    background:#00ffa314;
    color:var(--cui-secondary);
  }
  .torrent-cat-badge {
    margin-left:6px;
    padding:1px 5px;
    border:1px solid rgba(148,163,184,.25);
    border-radius:999px;
    color:var(--cui-text-secondary);
    font-family:var(--cui-font-mono);
    font-size:.52rem;
    text-transform:uppercase;
  }
  .progress-fill-v2 {
    height:100%;
    border-radius:2px;
    transition:width .3s ease;
  }
  .empty-state-inline {
    display:flex;
    flex-direction:column;
    align-items:center;
    justify-content:center;
    gap:.5rem;
    color:var(--cui-text-tertiary);
    font-size:.8rem;
  }
  .mobile-subline {
    margin:-.25rem 0 .55rem;
    color:var(--cui-text-tertiary);
    font-size:.66rem;
    line-height:1.4;
  }
  @media(max-width:767px),(pointer:coarse)and (max-height:500px) {
    .mobile-cards {
      display:flex;
      flex-direction:column;
      gap:.85rem;
      padding-bottom:calc(5.5rem + env(safe-area-inset-bottom,0px));
    }
    .download-card-mobile {
      background:var(--cui-glass);
      border:1px solid var(--cui-glass-border);
      border-radius:14px;
      padding:1rem;
      box-shadow:0 10px 24px #0000002e;
    }
    .download-card-mobile:focus-visible {
      outline:2px solid rgba(0,243,255,.45);
      outline-offset:2px;
    }
    .card-header-mobile {
      display:flex;
      justify-content:space-between;
      align-items:flex-start;
      gap:.5rem;
      margin-bottom:.75rem;
    }
    .card-name {
      font-size:.8rem;
      font-weight:600;
      color:var(--cui-text-primary);
      overflow:hidden;
      text-overflow:ellipsis;
      white-space:nowrap;
      flex:1;
    }
    .card-progress-mobile {
      height:4px;
      background:#ffffff0d;
      border-radius:2px;
      overflow:hidden;
      margin-bottom:.75rem;
    }
    .card-meta-mobile {
      display:grid;
      grid-template-columns:repeat(2,minmax(0,1fr));
      gap:.45rem .9rem;
      font-size:.7rem;
      color:var(--cui-text-tertiary);
    }
    .meta-item {
      display:flex;
      align-items:center;
      gap:.25rem;
    }
    .meta-item .material-icons {
      font-size:14px;
    }
    .card-error-mobile {
      margin-top:.5rem;
      font-size:.65rem;
      color:var(--cui-danger);
      padding:.5rem;
      background:#ff52521a;
      border-radius:4px;
    }
    .mobile-status-chips {
      display:flex;
      gap:.5rem;
      overflow-x:auto;
      overflow-y:hidden;
      flex-wrap:nowrap;
      padding-bottom:.25rem;
      scrollbar-width:none;
      -webkit-overflow-scrolling:touch;
    }
    .mobile-status-chips::-webkit-scrollbar {
      display:none;
    }
    .mobile-status-chip {
      flex-shrink:0;
      padding:.35rem .85rem;
      background:#ffffff0d;
      border:1px solid rgba(255,255,255,.1);
      border-radius:20px;
      color:var(--cui-text-tertiary);
      font-size:.7rem;
      font-weight:600;
      cursor:pointer;
      white-space:nowrap;
      transition:all .15s;
      min-height:44px;
      display:flex;
      align-items:center;
      gap:.3rem;
    }
    .mobile-status-chip.active {
      background:#00f3ff26;
      border-color:#00f3ff66;
      color:var(--cui-primary);
    }
    .chip-count {
      opacity:.65;
      font-size:.65rem;
    }
    .mobile-batch-card {
      background:#ffffff08;
      border:1px solid rgba(255,255,255,.08);
      border-radius:12px;
      overflow:hidden;
    }
    .mobile-batch-header {
      width:100%;
      padding:.85rem 1rem;
      cursor:pointer;
      display:flex;
      flex-direction:column;
      gap:.5rem;
      min-height:56px;
      justify-content:center;
      transition:background .15s;
      -webkit-user-select:none;
      user-select:none;
      background:none;
      border:none;
      color:inherit;
      text-align:left;
    }
    .mobile-batch-header:active {
      background:#ffffff0a;
    }
    .mobile-batch-header:focus-visible {
      outline:2px solid rgba(0,243,255,.45);
      outline-offset:-2px;
    }
    .batch-header-top {
      display:flex;
      align-items:center;
      gap:.5rem;
    }
    .batch-header-top .expand-icon {
      font-size:1.1rem;
      opacity:.6;
      flex-shrink:0;
      color:var(--cui-primary);
    }
    .batch-header-top .batch-name {
      flex:1;
      font-size:.8rem;
      font-weight:700;
      color:var(--cui-text-primary);
      overflow:hidden;
      text-overflow:ellipsis;
      white-space:nowrap;
    }
    .batch-progress-mobile {
      display:flex;
      flex-direction:column;
      gap:.3rem;
      padding-left:1.6rem;
    }
    .batch-progress-bar-mobile {
      height:3px;
      background:#ffffff12;
      border-radius:2px;
      overflow:hidden;
    }
    .batch-progress-fill-mobile {
      height:100%;
      border-radius:2px;
      transition:width .4s ease;
    }
    .batch-meta-mobile {
      display:flex;
      align-items:center;
      gap:.75rem;
    }
    .batch-pct-mobile {
      font-size:.65rem;
      font-family:var(--cui-font-mono);
      color:var(--cui-text-tertiary);
      font-weight:700;
    }
    .batch-speed-mobile {
      font-size:.65rem;
      font-family:var(--cui-font-mono);
      font-weight:700;
    }
    .batch-count-mobile {
      font-size:.65rem;
      color:var(--cui-text-tertiary);
    }
    .mobile-batch-children {
      border-top:1px solid rgba(255,255,255,.06);
      display:flex;
      flex-direction:column;
      gap:0;
    }
    .mobile-batch-children .download-card-mobile {
      border-radius:0;
      border:none;
      border-bottom:1px solid rgba(255,255,255,.05);
      padding:.75rem 1rem;
    }
    .mobile-batch-children .download-card-mobile:last-child {
      border-bottom:none;
    }
    .mobile-card-actions {
      display:none;
      flex-wrap:nowrap;
      gap:.35rem;
      padding-top:.75rem;
      margin-top:.5rem;
      border-top:1px solid rgba(255,255,255,.07);
      overflow-x:auto;
      overflow-y:hidden;
      -ms-overflow-style:none;
      scrollbar-width:none;
    }
    .mobile-card-actions::-webkit-scrollbar {
      display:none;
    }
    .download-card-mobile.expanded .mobile-card-actions {
      display:flex;
    }
    .mobile-action-btn,
    .mobile-more-btn {
      display:inline-flex;
      align-items:center;
      justify-content:center;
      gap:.3rem;
      padding:.4rem .65rem;
      background:#ffffff0f;
      border:1px solid rgba(255,255,255,.1);
      border-radius:6px;
      color:var(--cui-text-secondary);
      font-size:.68rem;
      font-weight:600;
      cursor:pointer;
      transition:all .15s;
      min-height:44px;
      white-space:nowrap;
    }
    .mobile-more-btn {
      flex:0 0 auto;
    }
    .mobile-action-btn:active {
      background:#ffffff1f;
    }
    .mobile-action-btn .material-icons {
      font-size:14px;
    }
    .mobile-action-btn.action-pause {
      color:#ff9800;
      border-color:#ff980040;
      background:#ff980014;
    }
    .mobile-action-btn.action-resume {
      color:var(--cui-secondary);
      border-color:#00ffa340;
      background:#00ffa314;
    }
    .mobile-action-btn.action-retry {
      color:var(--cui-info);
      border-color:#2196f340;
      background:#2196f314;
    }
    .mobile-action-btn.action-start {
      color:var(--cui-secondary);
      border-color:#00ffa340;
      background:#00ffa314;
    }
    .mobile-action-btn.action-details {
      color:var(--cui-primary);
      border-color:#00f3ff33;
    }
    .mobile-action-btn.action-delete {
      color:var(--cui-danger);
      border-color:#ff525233;
      background:#ff52520f;
    }
    .mobile-bottom-bar {
      display:flex;
      align-items:center;
      gap:.75rem;
      padding:.75rem 1rem calc(.75rem + env(safe-area-inset-bottom,0px));
      margin-top:1rem;
      background:var(--cui-glass);
      border:1px solid var(--cui-glass-border);
      border-radius:12px;
      overflow:hidden;
    }
    .mobile-bar-actions {
      display:flex;
      gap:.5rem;
      flex-shrink:0;
    }
    .mobile-bar-btn {
      display:flex;
      align-items:center;
      justify-content:center;
      width:44px;
      height:44px;
      border-radius:8px;
      border:1px solid rgba(255,255,255,.1);
      background:#ffffff08;
      color:var(--cui-text-primary);
      cursor:pointer;
      transition:all .2s;
    }
    .mobile-bar-btn:active {
      background:#ffffff14;
    }
    .mobile-bar-btn .material-icons {
      font-size:18px;
    }
    .mobile-bar-btn.btn-success {
      color:var(--cui-secondary);
      border-color:#00ffa333;
    }
    .mobile-bar-btn.btn-danger {
      color:#ff9800;
      border-color:#ff980033;
    }
    .mobile-bar-divider {
      width:1px;
      height:24px;
      background:#ffffff1a;
      flex-shrink:0;
    }
    .mobile-bar-pagination {
      display:flex;
      align-items:center;
      gap:.5rem;
      flex:1;
      justify-content:flex-end;
      min-width:0;
    }
    .mobile-bar-page-btn {
      display:flex;
      align-items:center;
      justify-content:center;
      width:44px;
      height:44px;
      border-radius:6px;
      border:1px solid rgba(255,255,255,.1);
      background:#ffffff08;
      color:var(--cui-text-primary);
      cursor:pointer;
      transition:all .2s;
      flex-shrink:0;
    }
    .mobile-bar-page-btn:disabled {
      opacity:.3;
      cursor:not-allowed;
    }
    .mobile-bar-page-btn:not(:disabled):active {
      background:#ffffff14;
    }
    .mobile-bar-page-btn .material-icons {
      font-size:18px;
    }
    .mobile-bar-page-info {
      font-size:.7rem;
      font-weight:700;
      color:var(--cui-primary);
      font-family:var(--cui-font-mono);
      white-space:nowrap;
      flex-shrink:0;
    }
    :global(.context-menu) {
      left:50% !important;
      top:50% !important;
      transform:translate(-50%,-50%) !important;
      min-width:260px;
      max-width:calc(100vw - 2rem);
    }
  }
</style>
