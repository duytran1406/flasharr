<script lang="ts">
  import { Badge } from '@media-set/core-ui';
  import {
    downloadStore,
    formatBytes,
    formatSpeed,
    formatETA,
    type DownloadTask,
    type StatusCounts,
  } from '$lib/stores/downloads.svelte';
  import type { UnifiedTorrentRow } from '$lib/stores/qbitReducer';
  import { episodeCoordinates } from './mediaQueue';
  import {
    formatAddedDate,
    getBatchStatus,
    getStateColor,
    isActiveState,
    normalizeSource,
    taskSource,
    torrentKey,
    torrentState,
    type DisplayRow,
    type SortColumn,
    type SourceCounts,
    type SourceFilter,
    type StatusFilter,
  } from './downloadsHelpers';

  interface Props {
    displayRows: DisplayRow[];
    visibleColumns: Set<string>;
    expandedBatches: Set<string>;
    selectedRowId: string | null;
    justCompletedIds: Set<string>;
    statusFilter: StatusFilter;
    sourceFilter?: SourceFilter;
    searchQuery: string;
    filterCounts: StatusCounts;
    sourceCounts: SourceCounts;
    isStatusDropdownOpen: boolean;
    isSourceDropdownOpen: boolean;
    loading: boolean;
    error?: string | null;
    currentPage: number;
    totalPages: number;
    onSort: (column: SortColumn) => void;
    onSourceFilter: (source: SourceFilter) => void;
    onSourceDropdownToggle: (open: boolean) => void;
    onStatusFilter: (status: string) => void;
    onStatusDropdownToggle: (open: boolean) => void;
    onRowSelect: (id: string) => void;
    onClearStatus?: () => void;
    onClearSearch?: () => void;
    onPage: (page: number) => void;
    onToggleBatch: (batchId: string) => void;
    onBatchContextMenu: (e: MouseEvent, batchId: string) => void;
    onDownloadContextMenu: (e: MouseEvent, id: string) => void;
    onTorrentContextMenu: (e: MouseEvent, torrent: UnifiedTorrentRow) => void;
  }

  let {
    displayRows,
    visibleColumns,
    expandedBatches,
    selectedRowId,
    justCompletedIds,
    statusFilter,
    sourceFilter = 'ALL',
    searchQuery,
    filterCounts,
    sourceCounts,
    isStatusDropdownOpen,
    isSourceDropdownOpen,
    loading,
    error = null,
    currentPage,
    totalPages,
    onSort,
    onSourceFilter,
    onSourceDropdownToggle,
    onStatusFilter,
    onStatusDropdownToggle,
    onRowSelect,
    onClearStatus,
    onClearSearch,
    onPage,
    onToggleBatch,
    onBatchContextMenu,
    onDownloadContextMenu,
    onTorrentContextMenu,
  }: Props = $props();

  /** Expanded season keys: `<batchId>:<season>` */
  let expandedSeasons = $state(new Set<string>());

  function seasonOf(task: DownloadTask): number {
    return episodeCoordinates(task).season ?? -1;
  }

  function seasonsOf(tasks: DownloadTask[]): number[] {
    return [...new Set(tasks.map(seasonOf))].sort((a, b) => a - b);
  }

  function seasonLabel(season: number): string {
    if (season === 0) return 'Specials';
    if (season > 0) return `Season ${season}`;
    return 'Unknown season';
  }

  function toggleSeason(batchId: string, season: number) {
    const key = `${batchId}:${season}`;
    const next = new Set(expandedSeasons);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    expandedSeasons = next;
  }

  function episodeLabel(task: DownloadTask): string {
    const { season, episode } = episodeCoordinates(task);
    if (season != null && episode != null) {
      return `S${String(season).padStart(2, '0')}E${String(episode).padStart(2, '0')} · Episode ${episode}`;
    }
    return task.filename;
  }

  /** CSS modifier for the source label (`source-label--fshare|torrent|torbox`). */
  function sourceLabelClass(source: string | null | undefined): string {
    const s = normalizeSource(source);
    if (s === 'torrent') return 'torrent';
    if (s === 'debrid') return 'torbox';
    return 'fshare';
  }

  function stateBadgeVariant(state: string) {
    if (state === 'COMPLETED' || state === 'Seeding') return 'success';
    if (state === 'FAILED') return 'danger';
    if (state === 'PAUSED') return 'warning';
    if (state === 'MIXED') return 'info';
    return 'primary';
  }

  function getSortIcon(column: SortColumn): string {
    if (downloadStore.pagination.sortBy !== column) return 'sort';
    return downloadStore.pagination.sortDir === 'asc' ? 'expand_less' : 'expand_more';
  }

  function getAriaSort(column: SortColumn): 'ascending' | 'descending' | 'none' {
    if (downloadStore.pagination.sortBy !== column) return 'none';
    return downloadStore.pagination.sortDir === 'asc' ? 'ascending' : 'descending';
  }

  let paginationInfo = $derived(() => {
    const total = downloadStore.pagination.total;
    if (total === 0) return 'Showing 0 media records';
    const limit = downloadStore.pagination.limit;
    const page = downloadStore.pagination.page;
    const start = (page - 1) * limit + 1;
    const end = Math.min(page * limit, total);
    return `Showing ${start}–${end} of ${total} media records`;
  });

  function statusBadgeStyle(status: string): string {
    const styles: Record<string, { bg: string; color: string; border: string }> = {
      ALL: { bg: 'rgba(100,116,139,.2)', color: 'var(--cui-text-secondary)', border: 'rgba(148,163,184,.3)' },
      DOWNLOADING: { bg: 'rgba(0,243,255,.2)', color: 'var(--cui-primary)', border: 'rgba(0,243,255,.3)' },
      QUEUED: { bg: 'rgba(255,215,0,.2)', color: 'var(--cui-warning)', border: 'rgba(255,215,0,.3)' },
      PAUSED: { bg: 'rgba(100,116,139,.2)', color: 'var(--cui-text-tertiary)', border: 'rgba(100,116,139,.3)' },
      COMPLETED: { bg: 'rgba(0,255,163,.2)', color: 'var(--cui-secondary)', border: 'rgba(0,255,163,.3)' },
      FAILED: { bg: 'rgba(255,82,82,.2)', color: 'var(--cui-danger)', border: 'rgba(255,82,82,.3)' },
    };
    const s = styles[status] || { bg: 'rgba(255,165,0,.2)', color: '#ffa500', border: 'rgba(255,165,0,.3)' };
    return `background:${s.bg};color:${s.color};border-color:${s.border};`;
  }

  function statusCount(status: string): number {
    const counts: Record<string, number> = {
      ALL: filterCounts.all,
      DOWNLOADING: filterCounts.downloading,
      QUEUED: filterCounts.queued,
      PAUSED: filterCounts.paused,
      COMPLETED: filterCounts.completed,
      FAILED: filterCounts.failed,
      CANCELLED: filterCounts.cancelled,
    };
    return counts[status] ?? 0;
  }

  const SOURCE_OPTIONS: SourceFilter[] = ['ALL', 'fshare', 'torrent', 'torbox'];

  function sourceCount(source: SourceFilter): number {
    if (source === 'ALL') return sourceCounts.all;
    return sourceCounts[source as keyof SourceCounts] ?? 0;
  }

  function sourceOptionLabel(source: SourceFilter): string {
    if (source === 'ALL') return 'All';
    if (source === 'torbox') return 'Debrid';
    return source[0].toUpperCase() + source.slice(1);
  }

  /** Shift+F10 opens the context menu at the row's top-left corner. */
  function handleRowKeydown(e: KeyboardEvent, id: string) {
    if (e.shiftKey && e.key === 'F10') {
      e.preventDefault();
      const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
      onDownloadContextMenu(new MouseEvent('contextmenu', { clientX: rect.left + 24, clientY: rect.top + 24 }), id);
    }
  }
</script>

<div class="table-wrapper">
  <table
    class="data-table"
    class:hide-source={!visibleColumns.has('Source')}
    class:hide-status={!visibleColumns.has('Status')}
    class:hide-size={!visibleColumns.has('Size')}
    class:hide-progress={!visibleColumns.has('Progress')}
    class:hide-speed={!visibleColumns.has('Speed')}
    class:hide-eta={!visibleColumns.has('ETA')}
    class:hide-added={!visibleColumns.has('Date added')}
    aria-label="Unified media download queue"
  >
    <thead>
      <tr>
        <th style="width:29%;" aria-sort={getAriaSort('filename')}
          ><button class="sort-button" onclick={() => onSort('filename')}
            ><span class="th-label">Media / File</span><span
              class="material-icons sort-icon"
              class:active={downloadStore.pagination.sortBy === 'filename'}
              aria-hidden="true">{getSortIcon('filename')}</span
            ></button
          ></th
        >
        <th style="width:16%;" class="status-filter-th" aria-label="Source">
          <div class="th-content status-filter-header">
            <div class="status-dropdown-wrapper">
              <button
                type="button"
                class="status-dropdown-trigger source-dropdown-trigger"
                class:open={isSourceDropdownOpen}
                aria-label="Filter downloads by source"
                aria-haspopup="menu"
                aria-expanded={isSourceDropdownOpen}
                onclick={(e) => {
                  e.stopPropagation();
                  onSourceDropdownToggle(!isSourceDropdownOpen);
                }}
                ><span class="filter-label">Source</span><span class="status-badge-v2"
                  >{sourceOptionLabel(sourceFilter)}</span
                ><span class="dropdown-count">({sourceCount(sourceFilter)})</span><span
                  class="material-icons dropdown-arrow"
                  class:open={isSourceDropdownOpen}>expand_more</span
                ></button
              >
              {#if isSourceDropdownOpen}
                <button class="dropdown-backdrop" aria-label="Close source filter" onclick={() => onSourceDropdownToggle(false)}
                ></button><div class="status-dropdown-menu" role="menu" aria-label="Source filter">
                  {#each SOURCE_OPTIONS as source}
                    <button
                      role="menuitem"
                      class="status-option"
                      class:selected={sourceFilter === source}
                      onclick={() => {
                        onSourceFilter(source);
                        onSourceDropdownToggle(false);
                      }}
                      ><span class="status-badge-v2">{sourceOptionLabel(source)}</span><span class="count"
                        >({sourceCount(source)})</span
                      >{#if sourceFilter === source}<span class="material-icons check-icon">check</span>{/if}</button
                    >
                  {/each}
                </div>
              {/if}
            </div>
          </div>
        </th>
        <th style="width:16%;" class="status-filter-th" aria-label="Status">
          <div class="th-content status-filter-header">
            <div class="status-dropdown-wrapper">
              <button
                type="button"
                class="status-dropdown-trigger"
                class:open={isStatusDropdownOpen}
                aria-label="Filter downloads by status"
                aria-haspopup="menu"
                aria-expanded={isStatusDropdownOpen}
                onclick={(e) => {
                  e.stopPropagation();
                  onStatusDropdownToggle(!isStatusDropdownOpen);
                }}
              >
                <span class="filter-label">Status</span>
                <span class="status-badge-v2" style={statusBadgeStyle(statusFilter)}
                  >{statusFilter === 'ALL' ? 'All' : statusFilter}</span
                >
                <span class="dropdown-count">({statusCount(statusFilter)})</span>
                <span class="material-icons dropdown-arrow" class:open={isStatusDropdownOpen}>expand_more</span>
              </button>
              {#if isStatusDropdownOpen}
                <button class="dropdown-backdrop" aria-label="Close status filter" onclick={() => onStatusDropdownToggle(false)}
                ></button>
                <div class="status-dropdown-menu" role="menu" aria-label="Status filter">
                  {#each ['ALL', 'DOWNLOADING', 'QUEUED', 'PAUSED', 'COMPLETED', 'FAILED', 'CANCELLED'] as status}
                    <button
                      role="menuitemradio"
                      class="status-option"
                      class:selected={statusFilter === status}
                      aria-checked={statusFilter === status}
                      onclick={() => {
                        onStatusFilter(status);
                        onStatusDropdownToggle(false);
                      }}
                    >
                      <span class="status-badge-v2 {status.toLowerCase()}">{status === 'ALL' ? 'All' : status}</span>
                      <span class="count">({statusCount(status)})</span>
                      {#if statusFilter === status}<span class="material-icons check-icon">check</span>{/if}
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
        </th>
        <th style="width:10%;" aria-sort={getAriaSort('size')}
          ><button class="sort-button" onclick={() => onSort('size')}
            ><span class="th-label">Size</span><span
              class="material-icons sort-icon"
              class:active={downloadStore.pagination.sortBy === 'size'}
              aria-hidden="true">{getSortIcon('size')}</span
            ></button
          ></th
        >
        <th style="width:14%;" aria-sort={getAriaSort('progress')}
          ><button class="sort-button" onclick={() => onSort('progress')}
            ><span class="th-label">Progress</span><span
              class="material-icons sort-icon"
              class:active={downloadStore.pagination.sortBy === 'progress'}
              aria-hidden="true">{getSortIcon('progress')}</span
            ></button
          ></th
        >
        <th style="width:10%;"><div class="th-content"><span class="th-label">Speed</span></div></th>
        <th style="width:8%;"><div class="th-content"><span class="th-label">ETA</span></div></th>
        <th style="width:13%;" aria-sort={getAriaSort('added')}
          ><button class="sort-button" onclick={() => onSort('added')}
            ><span class="th-label">Added</span><span
              class="material-icons sort-icon"
              class:active={downloadStore.pagination.sortBy === 'added'}
              aria-hidden="true">{getSortIcon('added')}</span
            ></button
          ></th
        >
      </tr>
    </thead>
    <tbody>
      {#if loading && displayRows.length === 0}
        {#each Array(5) as _}
          <tr class="skeleton-row"
            ><td class="filename-cell"><div class="skeleton-box" style="width:65%;"></div></td><td
              ><div class="skeleton-box" style="width:55px;"></div></td
            ><td><div class="skeleton-box" style="width:70px;"></div></td><td class="size-cell"
              ><div class="skeleton-box" style="width:80px;"></div></td
            ><td><div class="skeleton-box progress-skeleton" style="width:100%;"></div></td><td class="speed-cell"
              ><div class="skeleton-box" style="width:70px;"></div></td
            ><td class="eta-cell"><div class="skeleton-box" style="width:60px;"></div></td><td class="date-cell"
              ><div class="skeleton-box" style="width:100px;"></div></td
            ></tr
          >
        {/each}
      {:else if error}
        <tr
          ><td colspan="8" class="table-empty-cell"
            ><div class="empty-state-inline" role="alert" aria-live="polite">
              <span class="material-icons">error_outline</span>
              <span>{error}</span>
            </div></td
          ></tr
        >
      {:else if displayRows.length === 0}
        <tr
          ><td colspan="8" class="table-empty-cell"
            ><div class="empty-state-inline" role="status" aria-live="polite">
              <span class="material-icons">inbox</span>
              <span
                >{#if statusFilter !== 'ALL'}
                  No {statusFilter.toLowerCase()} downloads. <button class="inline-link" onclick={onClearStatus}
                    >Show all</button
                  >
                {:else if searchQuery}
                  No results for "{searchQuery}". <button class="inline-link" onclick={onClearSearch}>Clear search</button>
                {:else if sourceFilter === 'torrent'}
                  No torrent-backed queue items in Flasharr. Advanced qBittorrent controls stay in <a
                    class="inline-link inline-anchor"
                    href="/torrents">Torrents</a
                  >.
                {:else}
                  Queue empty. Add from Search or Discover; torrent-backed tasks appear here too.
                {/if}</span
              >
            </div></td
          ></tr
        >
      {:else}
        {#each displayRows as row (row.id)}
          {#if row.kind === 'batch'}
            {@const group = row.group}
            {@const batchStatus = getBatchStatus(group)}
            {@const expanded = expandedBatches.has(group.batchId)}
            <tr
              class="batch-header-row"
              class:batch-expanded={expanded}
              tabindex="0"
              role="button"
              aria-expanded={expanded}
              aria-label={`Toggle batch ${group.batchName}`}
              onclick={() => onToggleBatch(group.batchId)}
              onkeydown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  onToggleBatch(group.batchId);
                }
              }}
              oncontextmenu={(e) => onBatchContextMenu(e, group.batchId)}
              ><td class="filename-cell batch-name-cell"
                ><span class="expand-icon material-icons">{expanded ? 'expand_less' : 'expand_more'}</span>
                <span class="batch-name">{group.batchName}</span>
                <span class="batch-count">({group.totalItems} {group.totalItems === 1 ? 'episode' : 'episodes'})</span></td
              ><td class="source-cell"
                ><Badge variant="success" size="sm"
                  ><span class="source-label source-label--{sourceLabelClass(group.source)}"
                    >{normalizeSource(group.source)}</span
                  ></Badge
                ></td
              ><td
                ><Badge
                  variant={stateBadgeVariant(batchStatus.state)}
                  size="sm"
                  text={batchStatus.state === 'MIXED' ? 'Mixed' : batchStatus.state}
                />
                <div class="row-subline">{group.completedCount}/{group.totalItems} complete</div></td
              ><td class="size-cell">{formatBytes(group.totalSize)}</td><td
                ><div class="progress-cell-v2">
                  <div class="progress-bar-v2">
                    <div
                      class="progress-fill-v2"
                      style="--progress:{group.progress}%;--bar-color:{batchStatus.color};"
                    ></div>
                  </div>
                  <span class="progress-text-v2">{group.progress.toFixed(1)}%</span>
                </div></td
              ><td class="speed-cell">{group.speed > 0 ? formatSpeed(group.speed) : '—'}</td><td class="eta-cell"
                >{#if group.speed > 0 && group.totalSize > group.downloaded}{formatETA(
                    Math.round((group.totalSize - group.downloaded) / group.speed),
                  )}{:else}—{/if}</td
              ><td class="date-cell">{formatAddedDate(group.createdAt)}</td></tr
            >
            {#if expanded}
              {#each seasonsOf(group.downloads) as season}
                {@const seasonKey = `${group.batchId}:${season}`}
                {@const episodes = group.downloads.filter((task) => seasonOf(task) === season)}
                {@const seasonExpanded = expandedSeasons.has(seasonKey)}
                <tr class="season-row"
                  ><td colspan="8"
                    ><button
                      class="filename-cell season-name"
                      aria-expanded={seasonExpanded}
                      onclick={() => toggleSeason(group.batchId, season)}
                      ><span class="material-icons" aria-hidden="true"
                        >{seasonExpanded ? 'expand_more' : 'chevron_right'}</span
                      ><strong>{seasonLabel(season)}</strong><span>{episodes.length} episodes</span></button
                    ></td
                  ></tr
                >
                {#if seasonExpanded}
                  {#each episodes as task (task.id)}
                    {@const stateColor = getStateColor(task.state)}
                    {@const active = isActiveState(task.state)}
                    {@const selected = selectedRowId === task.id}
                    {@const justCompleted = justCompletedIds.has(task.id)}
                    <tr
                      class="transfer-row batch-child-row"
                      class:error-row={task.state === 'FAILED'}
                      class:selected-row={selected}
                      class:just-completed={justCompleted}
                      data-id={task.id}
                      data-download-id={task.id}
                      tabindex="0"
                      onclick={() => onRowSelect(task.id)}
                      onkeydown={(e) => handleRowKeydown(e, task.id)}
                      oncontextmenu={(e) => onDownloadContextMenu(e, task.id)}
                      ><td class="filename-cell batch-child-filename" title={task.filename}
                        ><span class="filename-text episode-title">{episodeLabel(task)}</span>
                        <span class="episode-filename">{task.filename}</span>
                        {#if task.quality}
                          <Badge variant="purple" size="xs" class="ml-[6px]" title={task.quality}>
                            {task.resolution || task.quality}
                          </Badge>
                        {/if}</td
                      ><td class="source-cell"
                        ><Badge variant="success" size="sm"
                          ><span class="source-label source-label--{sourceLabelClass(taskSource(task))}"
                            >{taskSource(task)}</span
                          ></Badge
                        ></td
                      ><td
                        ><Badge
                          variant={stateBadgeVariant(task.state)}
                          size="sm"
                          text={task.state}
                          title={task.error_message || ''}
                        />
                        {#if task.error_message}
                          <div class="error-preview" title={task.error_message}>{task.error_message}</div>
                        {/if}</td
                      ><td class="size-cell">{formatBytes(task.size)}</td><td
                        ><div class="progress-cell-v2">
                          <div class="progress-bar-v2">
                            <div class="progress-fill-v2" style="--progress:{task.progress}%;--bar-color:{stateColor};"></div>
                          </div>
                          <span class="progress-text-v2">{task.progress.toFixed(1)}%</span>
                        </div></td
                      ><td class="speed-cell">{active && task.speed ? formatSpeed(task.speed) : '—'}</td><td
                        class="eta-cell">{active && task.eta ? formatETA(task.eta) : '—'}</td
                      ><td class="date-cell">{formatAddedDate(task.created_at)}</td></tr
                    >
                  {/each}
                {/if}
              {/each}
            {/if}
          {:else if row.kind === 'torrent'}
            {@const torrent = row.torrent}
            {@const state = torrentState(torrent)}
            {@const stateColor = getStateColor(state)}
            {@const downloading = state === 'DOWNLOADING' || state === 'STARTING'}
            {@const seeding = torrent.state.toLowerCase().includes('up')}
            {@const selected = selectedRowId === torrentKey(torrent)}
            <tr
              class="transfer-row torrent-row"
              class:error-row={state === 'FAILED'}
              class:selected-row={selected}
              class:seeding-row={seeding}
              data-id={torrentKey(torrent)}
              tabindex="0"
              onclick={() => onRowSelect(torrentKey(torrent))}
              oncontextmenu={(e) => onTorrentContextMenu(e, torrent)}
              ><td class="filename-cell" title={torrent.name}
                ><span class="filename-text">{torrent.name}</span>
                {#if torrent.category}<span class="torrent-cat-badge">{torrent.category}</span>{/if}</td
              ><td class="source-cell"
                ><Badge variant="success" size="sm"><span class="source-label source-label--torrent">torrent</span></Badge
                ></td
              ><td
                ><Badge
                  variant={stateBadgeVariant(seeding ? 'Seeding' : state)}
                  size="sm"
                  text={seeding ? 'Seeding' : state}
                  title={torrent.state}
                />
                <div class="row-subline">Torrent-backed queue item. Advanced controls in Torrents.</div></td
              ><td class="size-cell">{formatBytes(Math.max(0, torrent.size))}</td><td
                ><div class="progress-cell-v2">
                  <div class="progress-bar-v2">
                    <div
                      class="progress-fill-v2"
                      style="--progress:{(torrent.progress * 100).toFixed(1)}%;--bar-color:{seeding
                        ? 'var(--cui-secondary)'
                        : stateColor};"
                    ></div>
                  </div>
                  <span class="progress-text-v2">{(torrent.progress * 100).toFixed(1)}%</span>
                </div></td
              ><td class="speed-cell"
                >{#if downloading && torrent.dlspeed > 0}{formatSpeed(torrent.dlspeed)}{:else}—{/if}</td
              ><td class="eta-cell"
                >{#if downloading && torrent.eta > 0 && torrent.eta < 864000}{formatETA(torrent.eta)}{:else}—{/if}</td
              ><td class="date-cell"
                ><a class="torrent-linkout" href="/torrents" onclick={(e) => e.stopPropagation()}>→ Torrents</a></td
              ></tr
            >
          {:else}
            {@const task = row.download}
            {@const stateColor = getStateColor(task.state)}
            {@const active = isActiveState(task.state)}
            {@const selected = selectedRowId === task.id}
            {@const justCompleted = justCompletedIds.has(task.id)}
            <tr
              class="transfer-row"
              class:error-row={task.state === 'FAILED'}
              class:selected-row={selected}
              class:just-completed={justCompleted}
              data-id={task.id}
              data-download-id={task.id}
              tabindex="0"
              onclick={() => onRowSelect(task.id)}
              onkeydown={(e) => handleRowKeydown(e, task.id)}
              oncontextmenu={(e) => onDownloadContextMenu(e, task.id)}
              ><td class="filename-cell" title={task.filename}
                ><span class="filename-text">{task.filename}</span>
                {#if task.quality}
                  <Badge variant="purple" size="xs" class="ml-[6px]" title={task.quality}>
                    {task.resolution || task.quality}
                  </Badge>
                {/if}</td
              ><td class="source-cell"
                ><Badge variant="success" size="sm"
                  ><span class="source-label source-label--{sourceLabelClass(taskSource(task))}">{taskSource(task)}</span
                  ></Badge
                ></td
              ><td
                ><Badge variant={stateBadgeVariant(task.state)} size="sm" text={task.state} title={task.error_message || ''} />
                {#if task.error_message}
                  <div class="error-preview" title={task.error_message}>{task.error_message.slice(0, 40)}...</div>
                {/if}</td
              ><td class="size-cell">{formatBytes(task.size)}</td><td
                ><div class="progress-cell-v2">
                  <div class="progress-bar-v2">
                    <div class="progress-fill-v2" style="--progress:{task.progress}%;--bar-color:{stateColor};"></div>
                  </div>
                  <span class="progress-text-v2">{task.progress.toFixed(1)}%</span>
                </div></td
              ><td class="speed-cell">{active && task.speed ? formatSpeed(task.speed) : '—'}</td><td class="eta-cell"
                >{active && task.eta ? formatETA(task.eta) : '—'}</td
              ><td class="date-cell">{formatAddedDate(task.created_at)}</td></tr
            >
          {/if}
        {/each}
      {/if}
    </tbody>
  </table>
</div>

<div class="pagination-footer-v2">
  <span class="page-info-v2">{paginationInfo()}</span>
  <div class="page-controls">
    <button class="icon-btn-tiny" disabled={currentPage === 1} onclick={() => onPage(currentPage - 1)}
      ><span class="material-icons">chevron_left</span></button
    >
    <div class="page-indicator">{currentPage} / {totalPages}</div>
    <button class="icon-btn-tiny" disabled={currentPage === totalPages} onclick={() => onPage(currentPage + 1)}
      ><span class="material-icons">chevron_right</span></button
    >
  </div>
</div>

<style>
  .table-wrapper {
    flex:1;
    min-height:0;
    overflow-y:auto;
    scroll-padding-top:49px;
    scrollbar-color:rgba(0,243,255,.35) transparent;
    scrollbar-width:thin;
  }
  .data-table {
    width:100%;
    border-collapse:collapse;
    table-layout:fixed;
  }
  .data-table.hide-source :is(th,td):nth-child(2),
  .data-table.hide-status :is(th,td):nth-child(3),
  .data-table.hide-size :is(th,td):nth-child(4),
  .data-table.hide-progress :is(th,td):nth-child(5),
  .data-table.hide-speed :is(th,td):nth-child(6),
  .data-table.hide-eta :is(th,td):nth-child(7),
  .data-table.hide-added :is(th,td):nth-child(8) {
    display:none;
  }
  .data-table.hide-source.hide-status.hide-size.hide-progress.hide-speed.hide-eta.hide-added :is(th,td):first-child {
    width:100%!important;
  }
  .data-table thead {
    position:sticky;
    top:0;
    background:#0f172af2;
    z-index:10;
    -webkit-backdrop-filter:blur(10px);
    backdrop-filter:blur(10px);
  }
  .data-table th {
    padding:.75rem 1rem;
    text-align:left;
    font-size:.7rem;
    font-weight:800;
    color:var(--cui-text-secondary);
    text-transform:uppercase;
    letter-spacing:.08em;
    border-bottom:2px solid rgba(255,255,255,.08);
    -webkit-user-select:none;
    user-select:none;
    transition:all .2s;
  }
  .data-table th:hover {
    color:var(--cui-text-primary);
    background:#ffffff05;
  }
  .sort-button {
    display:flex;
    min-height:44px;
    align-items:center;
    gap:6px;
    margin:-.75rem -1rem;
    padding:.75rem 1rem;
    border:0;
    background:transparent;
    color:inherit;
    font:inherit;
    text-transform:inherit;
    letter-spacing:inherit;
    cursor:pointer;
  }
  .sort-button:focus-visible {
    outline:2px solid var(--cui-primary);
    outline-offset:-2px;
  }
  .th-content {
    display:flex;
    align-items:center;
    gap:6px;
  }
  .th-label {
    font-weight:900;
    letter-spacing:.1em;
  }
  .sort-icon {
    font-size:14px!important;
    opacity:.4;
    transition:all .2s;
  }
  .sort-icon.active {
    opacity:1;
    color:var(--cui-primary);
  }
  .status-filter-th {
    padding:0!important;
  }
  .status-filter-header {
    position:relative;
    padding:.5rem;
    gap:.45rem;
  }
  .status-dropdown-wrapper {
    position:relative;
    display:inline-block;
  }
  .status-dropdown-trigger {
    display:flex;
    align-items:center;
    gap:.35rem;
    padding:.3rem .5rem;
    background:#0f172acc;
    border:1px solid rgba(255,255,255,.1);
    border-radius:6px;
    cursor:pointer;
    transition:all .2s;
    width:160px;
  }
  .status-dropdown-trigger:hover {
    border-color:#00f3ff4d;
    background:#0f172af2;
  }
  .status-dropdown-trigger.open {
    border-color:#ffffff1a;
    border-bottom-color:transparent;
    border-radius:6px 6px 0 0;
    background:#0a0f1ef2;
  }
  .status-dropdown-trigger .status-badge-v2 {
    font-size:.5rem;
    padding:2px 5px;
  }
  .dropdown-count {
    font-size:.6rem;
    color:var(--cui-text-tertiary);
    font-weight:500;
  }
  .dropdown-arrow {
    font-size:14px!important;
    color:var(--cui-text-tertiary);
    transition:transform .2s;
    margin-left:auto;
  }
  .dropdown-arrow.open {
    transform:rotate(180deg);
  }
  .dropdown-backdrop {
    position:fixed;
    inset:0;
    z-index:999;
  }
  .status-dropdown-menu {
    position:absolute;
    top:100%;
    left:0;
    background:#0a0f1ef2;
    -webkit-backdrop-filter:blur(12px);
    backdrop-filter:blur(12px);
    border:1px solid rgba(255,255,255,.1);
    border-top:none;
    border-radius:0 0 10px 10px;
    padding:.4rem;
    width:160px;
    box-sizing:border-box;
    box-shadow:0 10px 40px #00000080;
    z-index:1000;
  }
  .status-option {
    width:100%;
    display:flex;
    align-items:center;
    gap:.5rem;
    padding:.5rem .6rem;
    border:none;
    background:transparent;
    cursor:pointer;
    border-radius:6px;
    transition:all .15s;
    text-align:left;
  }
  .status-option:hover {
    background:#ffffff0d;
  }
  .status-option.selected {
    background:#00f3ff14;
  }
  .status-option .status-badge-v2 {
    font-size:.5rem;
    padding:2px 5px;
  }
  .status-option .count {
    font-size:.6rem;
    color:var(--cui-text-tertiary);
    font-weight:500;
  }
  .check-icon {
    font-size:14px!important;
    color:var(--cui-primary);
    margin-left:auto;
  }
  .data-table td {
    padding:.42rem 1rem;
    font-size:.72rem;
    color:var(--cui-text-secondary);
    border-bottom:1px solid rgba(255,255,255,.02);
    vertical-align:middle;
  }
  .data-table tr.transfer-row {
    transition:background .2s;
    cursor:pointer;
  }
  .data-table tr.transfer-row:hover {
    background:#ffffff05;
  }
  .data-table tr.error-row {
    background:#ff52520d;
    border-left:3px solid var(--cui-danger);
    box-shadow:inset 0 0 20px #ff525214;
  }
  .data-table tr.error-row:hover {
    background:#ff52521a;
  }
  .data-table tr.selected-row {
    background:#00f3ff14;
    border-left:3px solid var(--cui-primary);
    box-shadow:inset 0 0 15px #00f3ff1a;
  }
  .data-table tr.selected-row.error-row {
    border-left:3px solid var(--cui-danger);
  }
  .data-table tr.just-completed {
    animation:completion-celebrate 1.5s ease-out;
  }
  .batch-header-row {
    background:#00f3ff0a;
    cursor:pointer;
    transition:background .2s;
    border-left:3px solid transparent;
  }
  .batch-header-row:hover {
    background:#00f3ff14;
  }
  .batch-header-row.batch-expanded {
    background:#00f3ff0f;
    border-left:3px solid var(--cui-primary);
  }
  .batch-name-cell {
    display:flex;
    align-items:center;
    gap:.5rem;
  }
  .expand-icon {
    font-size:1.2rem;
    color:var(--cui-text-tertiary);
    transition:transform .2s,color .2s;
  }
  .batch-expanded .expand-icon {
    color:var(--cui-primary);
  }
  .batch-name {
    font-weight:500;
    color:var(--cui-text-primary);
  }
  .batch-count {
    color:#fff6;
    font-size:.75rem;
    font-weight:400;
  }
  .row-subline {
    margin-top:4px;
    font-size:.63rem;
    color:var(--cui-text-tertiary);
    line-height:1.35;
  }
  .desktop-inline {
    display:block;
  }
  .batch-child-row {
    background:#00000026;
  }
  .batch-child-row:hover {
    background:#ffffff08!important;
  }
  .batch-child-filename {
    padding-left:2.5rem!important;
  }
  .season-row {
    height:38px;
    background:#00f3ff06;
  }
  .season-row:hover {
    background:#00f3ff0f;
  }
  .season-row td {
    padding:0!important;
  }
  .season-name {
    width:100%;
    height:38px;
    padding-left:2rem!important;
    gap:.5rem;
    border:0;
    background:transparent;
    color:inherit;
    text-align:left;
    cursor:pointer;
  }
  .season-name .material-icons {
    font-size:16px;
    color:var(--cui-primary);
  }
  .season-name strong {
    font-size:.68rem;
    text-transform:uppercase;
    letter-spacing:.08em;
  }
  .season-name span:last-child {
    font-size:.62rem;
    color:var(--cui-text-tertiary);
  }
  .episode-title {
    font-weight:700;
    color:var(--cui-text-primary);
  }
  .episode-filename {
    display:block;
    flex-basis:100%;
    font-size:.62rem;
    color:var(--cui-text-tertiary);
    overflow:hidden;
    text-overflow:ellipsis;
    white-space:nowrap;
  }
  .filename-cell {
    font-weight:600;
    overflow:hidden;
    text-overflow:ellipsis;
    white-space:nowrap;
    color:var(--cui-text-primary);
    display:flex;
    align-items:center;
    flex-wrap:wrap;
    gap:.35rem .5rem;
  }
  .filename-text {
    overflow:hidden;
    text-overflow:ellipsis;
    white-space:nowrap;
    min-width:0;
    max-width:100%;
  }
  .source-cell {
    overflow:hidden;
    text-overflow:ellipsis;
  }
  .source-label--fshare {
    color:var(--cui-danger);
  }
  .source-label--torrent {
    color:var(--cui-warning);
  }
  .source-label--torbox {
    color:var(--cui-success);
  }
  .torrent-row {
    --torrent-accent:rgba(0,255,163,.45);
  }
  .torrent-row.seeding-row td:first-child {
    box-shadow:inset 2px 0 #00ffa3b3;
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
  .status-badge-v2.all {
    background:#ffffff1a;
    color:var(--cui-text-primary);
    border-color:#fff3;
  }
  .status-badge-v2.downloading {
    background:#00f3ff26;
    color:var(--cui-primary);
    border-color:#00f3ff4d;
  }
  .status-badge-v2.queued {
    background:#2196f326;
    color:var(--cui-info);
    border-color:#2196f34d;
  }
  .status-badge-v2.paused {
    background:#ff980026;
    color:#ff9800;
    border-color:#ff98004d;
  }
  .status-badge-v2.completed {
    background:#4caf5026;
    color:var(--cui-success);
    border-color:#4caf504d;
  }
  .status-badge-v2.failed {
    background:#f4433626;
    color:#f44336;
    border-color:#f443364d;
  }
  .status-badge-v2.cancelled {
    background:#9e9e9e26;
    color:#9e9e9e;
    border-color:#9e9e9e4d;
  }
  .progress-bar-v2 {
    height:3px;
    background:#ffffff0d;
    border-radius:2px;
    overflow:hidden;
  }
  .progress-fill-v2 {
    height:100%;
    width:var(--progress,0%);
    background:var(--bar-color,var(--cui-primary));
    border-radius:2px;
    transition:width .3s ease;
  }
  .progress-cell-v2 {
    display:flex;
    flex-direction:column;
    gap:2px;
  }
  .progress-text-v2 {
    font-size:.65rem;
    color:var(--cui-text-tertiary);
    font-family:var(--cui-font-mono);
    font-weight:600;
  }
  .error-preview {
    margin-top:4px;
    font-size:.6rem;
    color:var(--cui-danger);
    max-width:200px;
    overflow:hidden;
    text-overflow:ellipsis;
    white-space:nowrap;
  }
  .size-cell {
    color:var(--cui-text-secondary);
    font-size:.7rem;
  }
  .speed-cell {
    font-family:var(--cui-font-mono);
    font-size:.7rem;
    font-weight:700;
  }
  .eta-cell {
    color:var(--cui-text-secondary);
    font-size:.7rem;
  }
  .date-cell {
    color:var(--cui-text-secondary);
    font-size:.7rem;
    white-space:nowrap;
  }
  .table-empty-cell {
    text-align:center;
    padding:3rem 1rem!important;
    color:var(--cui-text-tertiary);
  }
  .empty-state-inline {
    display:flex;
    align-items:center;
    justify-content:center;
    gap:.5rem;
    color:var(--cui-text-tertiary);
    font-size:.8rem;
  }
  .empty-state-inline .material-icons {
    font-size:18px;
    opacity:.5;
  }
  .inline-link {
    background:transparent;
    border:none;
    color:var(--cui-primary);
    cursor:pointer;
    font-size:.8rem;
    text-decoration:underline;
    padding:0;
    margin-left:.5rem;
  }
  .inline-anchor {
    display:inline-block;
  }
  .torrent-linkout {
    color:var(--cui-primary);
    font-size:.7rem;
    text-decoration:none;
    font-weight:700;
  }
  @media(prefers-reduced-motion:reduce) {
    .data-table tr.transfer-row,
    .progress-fill-v2,
    .sort-icon,
    .expand-icon,
    .icon-btn-tiny,
    .status-dropdown-trigger,
    .dropdown-arrow {
      transition:none!important;
    }
    .data-table tr.just-completed {
      animation:none!important;
    }
  }
  .pagination-footer-v2 {
    padding:.5rem 1.25rem;
    display:flex;
    justify-content:flex-end;
    align-items:center;
    gap:1.5rem;
    border-top:1px solid rgba(255,255,255,.03);
    background:#00000026;
  }
  .page-info-v2 {
    font-size:.7rem;
    color:var(--cui-text-tertiary);
    letter-spacing:.05em;
    font-weight:700;
  }
  .page-controls {
    display:flex;
    gap:.5rem;
    align-items:center;
    background:#ffffff08;
    padding:4px;
    border-radius:8px;
    border:1px solid rgba(255,255,255,.05);
  }
  .page-indicator {
    padding:0 .5rem;
    font-weight:800;
    font-size:.7rem;
    color:var(--cui-primary);
    font-family:var(--cui-font-mono);
  }
  .icon-btn-tiny {
    width:44px;
    height:44px;
    background:transparent;
    border:none;
    border-radius:4px;
    color:var(--cui-text-tertiary);
    cursor:pointer;
    display:flex;
    align-items:center;
    justify-content:center;
    transition:all .2s;
  }
  .icon-btn-tiny:hover {
    background:#ffffff1a;
    color:var(--cui-text-primary);
  }
  .icon-btn-tiny:disabled {
    opacity:.3;
    cursor:not-allowed;
  }
  .icon-btn-tiny .material-icons {
    font-size:14px;
  }
  .skeleton-row {
    opacity:.6;
  }
  .skeleton-box {
    height:16px;
    background:linear-gradient(90deg,#ffffff08 25%,#ffffff14,#ffffff08 75%);
    background-size:200% 100%;
    animation:shimmer 1.5s infinite;
    border-radius:4px;
  }
  .progress-skeleton {
    height:8px!important;
  }
  @keyframes completion-celebrate {
    0% {
      background:#00ffa300;
    }
    15% {
      background:#00ffa340;
      box-shadow:0 0 30px #00ffa366;
    }
    40% {
      background:#00ffa31a;
    }
    to {
      background:#00ffa308;
    }
  }
  @keyframes shimmer {
    0% {
      background-position:-200% 0;
    }
    to {
      background-position:200% 0;
    }
  }
</style>
