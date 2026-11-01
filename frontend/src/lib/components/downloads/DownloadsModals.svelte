<script lang="ts">
  import { Modal, Button, Badge } from '@media-set/core-ui';
  import { formatBytes, formatSpeed, formatETA, type DownloadTask } from '$lib/stores/downloads.svelte';
  import { toasts } from '$lib/stores/toasts';
  import { copyToClipboard } from '$lib/utils/clipboard';
  import {
    formatAddedDate,
    getBatchStatus,
    getStateColor,
    isActiveState,
    isTorrentTask,
    type BatchDeleteTarget,
    type BatchGroup,
  } from './downloadsHelpers';

  type BatchAction = 'pause' | 'resume' | 'retry' | 'redownload' | 'delete';

  interface Props {
    deleteConfirmTask: DownloadTask | null;
    deleteConfirmBatch: BatchDeleteTarget | null;
    detailsTask: DownloadTask | null;
    detailsBatch: BatchGroup | null;
    selectedCount?: number;
    onHideDeleteConfirm: () => void;
    onHideBatchDeleteConfirm: () => void;
    onHideTaskDetails: () => void;
    onHideBatchDetails: () => void;
    onConfirmDelete: () => void;
    onConfirmBatchDelete: () => void;
    onBatchAction: (action: BatchAction, batchId: string) => void;
    onCopySafeSource: (task: DownloadTask) => void;
  }

  let {
    deleteConfirmTask,
    deleteConfirmBatch,
    detailsTask,
    detailsBatch,
    onHideDeleteConfirm,
    onHideBatchDeleteConfirm,
    onHideTaskDetails,
    onHideBatchDetails,
    onConfirmDelete,
    onConfirmBatchDelete,
    onBatchAction,
    onCopySafeSource,
  }: Props = $props();
</script>

<!-- Delete Confirmation Modal -->
<Modal
  open={!!deleteConfirmTask}
  onClose={onHideDeleteConfirm}
  maxWidth="420px"
  accent="var(--cui-danger)"
  ariaLabel="Confirm Delete Download"
>
  {#snippet header()}
    <div class="del-modal-title">
      <span class="del-modal-icon material-icons">delete_forever</span>
      <div>
        <div class="del-modal-heading">Delete Download</div>
        <div class="del-modal-sub">This action cannot be undone</div>
      </div>
    </div>
    <button class="close-btn" onclick={onHideDeleteConfirm} aria-label="Close">
      <span class="material-icons">close</span>
    </button>
  {/snippet}

  {#snippet children()}
    <p class="del-confirm-text">Are you sure you want to delete this download?</p>
    <div class="del-filename-pill">
      <span class="material-icons">insert_drive_file</span>
      <span class="del-filename-text">
        {deleteConfirmTask
          ? deleteConfirmTask.filename.length > 48
            ? deleteConfirmTask.filename.substring(0, 48) + '…'
            : deleteConfirmTask.filename
          : ''}
      </span>
    </div>
    <div class="del-warning-row">
      <span class="material-icons del-warn-icon">folder_delete</span>
      <p class="del-warn-text">
        <strong>Warning:</strong> The downloaded file will also be permanently deleted from disk.
      </p>
    </div>
  {/snippet}

  {#snippet footer()}
    <div class="del-modal-actions">
      <Button variant="ghost" size="md" onclick={onHideDeleteConfirm}>Cancel</Button>
      <Button variant="danger" size="md" icon="delete_forever" onclick={onConfirmDelete}>Delete Permanently</Button>
    </div>
  {/snippet}
</Modal>

<!-- Batch Delete Confirmation Modal -->
<Modal
  open={!!deleteConfirmBatch}
  onClose={onHideBatchDeleteConfirm}
  maxWidth="420px"
  accent="var(--cui-danger)"
  ariaLabel="Confirm Delete Batch"
>
  {#snippet header()}
    <div class="del-modal-title">
      <span class="del-modal-icon material-icons">delete_sweep</span>
      <div>
        <div class="del-modal-heading">Delete Batch</div>
        <div class="del-modal-sub">This action cannot be undone</div>
      </div>
    </div>
    <button class="close-btn" onclick={onHideBatchDeleteConfirm} aria-label="Close">
      <span class="material-icons">close</span>
    </button>
  {/snippet}

  {#snippet children()}
    <p class="del-confirm-text">Are you sure you want to delete this entire batch?</p>
    <div class="del-filename-pill">
      <span class="material-icons">folder_zip</span>
      <span class="del-filename-text">
        {deleteConfirmBatch
          ? deleteConfirmBatch.batchName.length > 48
            ? deleteConfirmBatch.batchName.substring(0, 48) + '…'
            : deleteConfirmBatch.batchName
          : ''}
      </span>
    </div>
    <div class="del-warning-row">
      <span class="material-icons del-warn-icon">folder_delete</span>
      <p class="del-warn-text">
        <strong>Warning:</strong> Deleting this batch will permanently remove
        {#if deleteConfirmBatch}
          <strong>{deleteConfirmBatch.itemCount} {deleteConfirmBatch.itemCount === 1 ? 'item' : 'items'}</strong>
        {/if}
        and all associated files from disk.
      </p>
    </div>
  {/snippet}

  {#snippet footer()}
    <div class="del-modal-actions">
      <Button variant="ghost" size="md" onclick={onHideBatchDeleteConfirm}>Cancel</Button>
      <Button variant="danger" size="md" icon="delete_sweep" onclick={onConfirmBatchDelete}>Delete Batch</Button>
    </div>
  {/snippet}
</Modal>

<!-- Task Details Modal -->
<Modal
  open={!!detailsTask}
  onClose={onHideTaskDetails}
  maxWidth="600px"
  accent="var(--color-primary, var(--cui-primary))"
  ariaLabel="Download Details"
>
  {#snippet header()}
    <div class="detail-header-main">
      {#if detailsTask}
        <!-- Left: big fingerprint icon, UUID overlaid at bottom -->
        <button
          class="id-badge-pill"
          title="Click to copy full ID"
          onclick={() =>
            copyToClipboard(detailsTask.id).then((ok) =>
              ok ? toasts.success('ID copied') : toasts.error('Failed to copy'),
            )}
        >
          <span class="material-icons id-big-icon">fingerprint</span>
          <span class="id-overlay-text">{detailsTask.id.substring(0, 8)}</span>
        </button>
        <!-- Right: filename row + quality badge stacked -->
        <div class="detail-name-block">
          <h3 class="filename-title" title={detailsTask.filename}>{detailsTask.filename}</h3>
          {#if detailsTask.quality}
            <Badge variant="purple" size="md" class="mt-[4px]" title={detailsTask.quality}>{detailsTask.quality}</Badge>
          {/if}
        </div>
      {/if}
    </div>
    <button class="close-btn" onclick={onHideTaskDetails}>
      <span class="material-icons">close</span>
    </button>
  {/snippet}

  {#snippet children()}
    {#if detailsTask}
      {@const torrent = isTorrentTask(detailsTask)}
      <!-- Main Progress & State -->
      <div class="details-hero-section">
        <div class="hero-stats">
          <div class="stat-block">
            <span class="label">STATUS</span
            ><span class="value state-text" style="color:{getStateColor(detailsTask.state)};">{detailsTask.state}</span>
          </div>
          <div class="stat-block">
            <span class="label">PROGRESS</span
            ><span class="value progress-text">{Math.round(detailsTask.progress)}%</span>
          </div>
          <div class="stat-block">
            <span class="label">SPEED</span
            ><span class="value speed-text"
              >{isActiveState(detailsTask.state) ? formatSpeed(detailsTask.speed || 0) : '0 B/s'}</span
            >
          </div>
        </div>

        <div class="hero-progress-container">
          <div class="progress-track">
            <div
              class="progress-bar-glow"
              style="width:{detailsTask.progress}%;background:{getStateColor(detailsTask.state)};"
            ></div>
          </div>
          <div class="progress-meta">
            <span>{formatBytes(detailsTask.downloaded || 0)}</span
            ><span>OF</span
            ><span>{formatBytes(detailsTask.size)}</span>
          </div>
        </div>
      </div>

      <div class="details-grid-premium">
        <!-- Technical Details Section -->
        <div class="details-section">
          <div class="section-label">TECHNICAL LOGS</div>

          <div class="data-row-premium">
            <div class="data-item">
              <span class="material-icons">calendar_today</span
              ><div class="item-content">
                <span class="l">Created At</span
                ><span class="v">{formatAddedDate(detailsTask.created_at)}</span>
              </div>
            </div>
            {#if detailsTask.started_at}
              <div class="data-item">
                <span class="material-icons">play_circle</span
                ><div class="item-content">
                  <span class="l">Started At</span
                  ><span class="v">{formatAddedDate(detailsTask.started_at)}</span>
                </div>
              </div>
            {/if}
          </div>

          <div class="data-row-premium">
            <div class="data-item">
              <span class="material-icons">timer</span
              ><div class="item-content">
                <span class="l">{detailsTask.state === 'COMPLETED' ? 'Completed At' : 'Estimated Time'}</span
                ><span class="v">
                  {detailsTask.state === 'COMPLETED'
                    ? detailsTask.completed_at
                      ? formatAddedDate(detailsTask.completed_at)
                      : '--'
                    : isActiveState(detailsTask.state)
                      ? formatETA(detailsTask.eta)
                      : '--'}
                </span>
              </div>
            </div>
            <div class="data-item">
              <span class="material-icons">category</span
              ><div class="item-content">
                <span class="l">Data Category</span
                ><span class="v" style="text-transform:uppercase;">{detailsTask.category || 'General'}</span>
              </div>
            </div>
          </div>
        </div>

        <!-- Links Section -->
        <div class="details-section">
          <div class="section-label">ACCESS NODES</div>

          <div class="link-item-premium">
            <div class="link-info">
              <span class="material-icons link-icon">link</span
              ><div class="link-text">
                <span class="l">{torrent ? 'Torrent Hash' : 'Premium Decoded URL'}</span
                ><span class="v"
                  >{torrent ? detailsTask.external_id || 'Resolving...' : detailsTask.url || 'Resolving...'}</span
                >
              </div>
            </div>
            <button class="copy-tiny" onclick={() => onCopySafeSource(detailsTask)}>
              <span class="material-icons">content_copy</span>
            </button>
          </div>

          <div class="link-item-premium">
            <div class="link-info">
              <span class="material-icons link-icon">cloud</span
              ><div class="link-text">
                <span class="l">{torrent ? 'Torrent Provider' : 'Fshare Source Link'}</span
                ><span class="v">{torrent ? detailsTask.external_provider || 'qBittorrent' : detailsTask.original_url}</span>
              </div>
            </div>
            <button class="copy-tiny" onclick={() => onCopySafeSource(detailsTask)}>
              <span class="material-icons">content_copy</span>
            </button>
          </div>
        </div>

        <!-- Error Handling -->
        {#if detailsTask.error_message}
          <div class="details-section error">
            <div class="section-label danger">INCIDENT REPORT</div>
            <div class="error-box-premium">
              <span class="material-icons">error_outline</span
              ><p>{detailsTask.error_message}</p>
            </div>
          </div>
        {/if}
      </div>

      <div class="details-footer-premium">
        <Button variant="ghost" size="md" onclick={onHideTaskDetails}>Close Terminal</Button>
      </div>
    {/if}
  {/snippet}
</Modal>

<!-- Batch Detail Modal -->
<Modal
  open={!!detailsBatch}
  onClose={onHideBatchDetails}
  maxWidth="680px"
  accent="var(--cui-secondary)"
  ariaLabel="Batch Download Details"
>
  {#snippet header()}
    <div class="detail-header-main">
      {#if detailsBatch}
        <!-- Left: big icon, UUID overlaid at bottom -->
        <button
          class="id-badge-pill"
          title="Click to copy full batch ID"
          onclick={() =>
            copyToClipboard(detailsBatch.batchId).then((ok) =>
              ok ? toasts.success('Batch ID copied') : toasts.error('Failed to copy'),
            )}
        >
          <span class="material-icons id-big-icon">folder_special</span>
          <span class="id-overlay-text">{detailsBatch.batchId.substring(0, 8)}</span>
        </button>
        <!-- Right: batch name stacked -->
        <div class="detail-name-block">
          <h3 class="filename-title" title={detailsBatch.batchName}>{detailsBatch.batchName}</h3>
        </div>
      {/if}
    </div>
    <button class="close-btn" onclick={onHideBatchDetails}>
      <span class="material-icons">close</span>
    </button>
  {/snippet}

  {#snippet children()}
    {#if detailsBatch}
      {@const batchStatus = getBatchStatus(detailsBatch)}
      {@const estimatedTime =
        detailsBatch.speed > 0 ? Math.ceil((detailsBatch.totalSize - detailsBatch.downloaded) / detailsBatch.speed) : 0}

      <!-- Hero Section: Stats & Progress -->
      <div class="details-hero-section">
        <div class="hero-stats">
          <div class="stat-block">
            <span class="label">STATUS</span
            ><span class="value state-text" style="color:{batchStatus.color};">
              {detailsBatch.completedCount === detailsBatch.totalItems
                ? 'All Done'
                : detailsBatch.activeCount > 0
                  ? `Mixed (${detailsBatch.completedCount}/${detailsBatch.totalItems})`
                  : `Paused (${detailsBatch.completedCount}/${detailsBatch.totalItems})`}
            </span>
          </div>
          <div class="stat-block">
            <span class="label">PROGRESS</span
            ><span class="value progress-text">{detailsBatch.progress.toFixed(1)}%</span>
          </div>
          <div class="stat-block">
            <span class="label">SPEED</span
            ><span class="value speed-text">{detailsBatch.speed > 0 ? formatSpeed(detailsBatch.speed) : '0 B/s'}</span>
          </div>
        </div>

        <div class="hero-progress-container">
          <div class="progress-track">
            <div class="progress-bar-glow" style="width:{detailsBatch.progress}%;background:{batchStatus.color};"></div>
          </div>
          <div class="progress-meta">
            <span>{formatBytes(detailsBatch.downloaded)}</span
            ><span>OF</span
            ><span>{formatBytes(detailsBatch.totalSize)}</span>
          </div>
        </div>
      </div>

      <!-- Statistics Grid -->
      <div class="details-grid-premium batch-stats-grid">
        <!-- Batch Overview -->
        <div class="details-section">
          <div class="section-label">BATCH OVERVIEW</div>

          <div class="batch-stats-compact">
            <div class="stat-row">
              <span class="stat-label-compact">Total Files:</span
              ><span class="stat-value-compact">{detailsBatch.totalItems}</span>
            </div>
            <div class="stat-row">
              <span class="stat-label-compact"><span class="material-icons stat-icon">pause_circle</span> Paused:</span
              ><span class="stat-value-compact" style="color:var(--cui-text-tertiary);">{detailsBatch.pausedCount}</span>
            </div>
            <div class="stat-row">
              <span class="stat-label-compact"
                ><span class="material-icons stat-icon" style="color:var(--cui-secondary)">check_circle</span> Completed:</span
              ><span class="stat-value-compact" style="color:var(--cui-secondary);">{detailsBatch.completedCount}</span>
            </div>
            <div class="stat-row">
              <span class="stat-label-compact"
                ><span class="material-icons stat-icon" style="color:var(--cui-warning)">schedule</span> Queued:</span
              ><span class="stat-value-compact" style="color:var(--cui-warning);">{detailsBatch.queuedCount}</span>
            </div>
            <div class="stat-row">
              <span class="stat-label-compact"
                ><span class="material-icons stat-icon" style="color:var(--cui-primary)">download</span> Downloading:</span
              ><span class="stat-value-compact" style="color:var(--cui-primary);">{detailsBatch.activeCount}</span>
            </div>
            <div class="stat-row">
              <span class="stat-label-compact"
                ><span class="material-icons stat-icon" style="color:var(--cui-danger)">error</span> Failed:</span
              ><span class="stat-value-compact" style="color:var(--cui-danger);">{detailsBatch.failedCount}</span>
            </div>
          </div>
        </div>

        <!-- Timing Info -->
        <div class="details-section">
          <div class="section-label">TIMING INFO</div>

          <div class="batch-stats-compact">
            <div class="stat-row">
              <span class="stat-label-compact"><span class="material-icons stat-icon">event</span> Created:</span
              ><span class="stat-value-compact">{formatAddedDate(detailsBatch.createdAt)}</span>
            </div>
            <div class="stat-row">
              <span class="stat-label-compact"><span class="material-icons stat-icon">timer</span> Est. Complete:</span
              ><span class="stat-value-compact">
                {estimatedTime > 0 ? formatETA(estimatedTime) : '~15 minutes'}
              </span>
            </div>
          </div>
        </div>
      </div>

      <!-- Batch Actions Footer -->
      <div class="details-footer-premium batch-actions-footer">
        <div class="batch-action-buttons">
          <Button
            size="md"
            icon="play_arrow"
            onclick={() => onBatchAction('resume', detailsBatch.batchId)}
            disabled={detailsBatch.pausedCount === 0 && detailsBatch.queuedCount === 0}>Resume All</Button
          >
          <Button
            size="md"
            icon="pause"
            onclick={() => onBatchAction('pause', detailsBatch.batchId)}
            disabled={detailsBatch.activeCount === 0}>Pause All</Button
          >
          <Button
            variant="danger"
            size="md"
            icon="refresh"
            onclick={() => onBatchAction('retry', detailsBatch.batchId)}
            disabled={detailsBatch.failedCount === 0}>Retry Failed</Button
          >
        </div>
        <Button variant="ghost" size="md" onclick={onHideBatchDetails}>Close</Button>
      </div>
    {/if}
  {/snippet}
</Modal>

<style>
  .close-btn {
    background:none;
    border:0;
    color:var(--cui-text-secondary);
    cursor:pointer;
    padding:0;
    line-height:1;
    display:inline-flex;
    align-items:center;
    border-radius:8px;
  }
  .del-modal-title {
    display:flex;
    align-items:center;
    gap:.75rem;
    flex:1;
    min-width:0;
  }
  .del-modal-icon {
    font-size:1.6rem;
    color:var(--cui-danger);
    flex-shrink:0;
  }
  .del-modal-heading {
    font-size:1rem;
    font-weight:800;
    color:var(--cui-danger);
    font-family:var(--cui-font-mono);
    letter-spacing:.04em;
  }
  .del-modal-sub {
    font-size:.72rem;
    color:#ffffff59;
    font-family:var(--cui-font-mono);
    margin-top:2px;
  }
  .del-confirm-text {
    font-size:.88rem;
    color:#ffffffa6;
    margin:0 0 1.1rem;
    line-height:1.5;
  }
  .del-filename-pill {
    display:flex;
    align-items:center;
    gap:.5rem;
    background:#ffffff0a;
    border:1px solid rgba(255,255,255,.08);
    border-radius:8px;
    padding:.6rem .85rem;
    margin-bottom:1rem;
    font-size:.78rem;
    font-family:var(--cui-font-mono);
    color:#ffffffd9;
    word-break:break-all;
  }
  .del-filename-pill .material-icons {
    font-size:1rem;
    color:#ffffff4d;
    flex-shrink:0;
  }
  .del-filename-text {
    flex:1;
    min-width:0;
  }
  .del-warning-row {
    display:flex;
    align-items:flex-start;
    gap:.65rem;
    background:#ff525212;
    border:1px solid rgba(255,82,82,.2);
    border-radius:8px;
    padding:.7rem .85rem;
  }
  .del-warn-icon {
    font-size:1.1rem;
    color:var(--cui-danger);
    flex-shrink:0;
    margin-top:1px;
  }
  .del-warn-text {
    margin:0;
    font-size:.78rem;
    color:#ff9682d9;
    line-height:1.5;
  }
  .del-warn-text strong {
    color:#ff7878;
  }
  .del-modal-actions {
    display:flex;
    flex-direction:row;
    align-items:center;
    justify-content:flex-end;
    gap:.65rem;
    margin-inline:-1rem;
  }
  .detail-header-main {
    flex:1;
    min-width:0;
    display:flex;
    flex-direction:row;
    align-items:stretch;
    gap:.75rem;
  }
  .id-badge-pill {
    position:relative;
    align-self:stretch;
    flex-shrink:0;
    background:none;
    border:none;
    border-radius:0;
    min-width:3rem;
    padding:.15rem .35rem 0;
    cursor:pointer;
    overflow:hidden;
    transition:opacity .18s;
  }
  .id-big-icon {
    font-size:3rem!important;
    color:var(--cui-primary);
    opacity:.45;
    display:block;
    line-height:1;
    transition:opacity .18s;
  }
  .id-badge-pill:hover .id-big-icon {
    opacity:.85;
  }
  .id-overlay-text {
    position:absolute;
    bottom:0;
    left:0;
    right:0;
    padding:.1rem .2rem;
    background:#0009;
    -webkit-backdrop-filter:blur(4px);
    backdrop-filter:blur(4px);
    font-family:var(--cui-font-mono);
    font-size:.48rem;
    font-weight:700;
    letter-spacing:.06em;
    color:var(--cui-primary);
    text-align:center;
    line-height:1.4;
    pointer-events:none;
  }
  .detail-name-block {
    flex:1;
    min-width:0;
    display:flex;
    flex-direction:column;
    justify-content:center;
    gap:.3rem;
  }
  .filename-title {
    margin:0;
    font-size:1.1rem;
    font-weight:800;
    color:var(--cui-text-primary);
    white-space:nowrap;
    overflow:hidden;
    text-overflow:ellipsis;
    letter-spacing:-.01em;
  }
  .details-hero-section {
    padding:2rem;
    background:#ffffff04;
  }
  .hero-stats {
    display:flex;
    justify-content:space-between;
    margin-bottom:1.5rem;
  }
  .stat-block {
    display:flex;
    flex-direction:column;
    gap:.4rem;
  }
  .stat-block .label {
    font-size:.6rem;
    font-weight:900;
    color:var(--cui-text-tertiary);
    letter-spacing:.15em;
  }
  .stat-block .value {
    font-family:var(--cui-font-mono);
    font-size:1.1rem;
    font-weight:700;
    color:var(--cui-text-primary);
  }
  .hero-progress-container {
    margin-top:1rem;
  }
  .progress-track {
    height:8px;
    background:#0000004d;
    border-radius:4px;
    overflow:hidden;
    position:relative;
  }
  .progress-bar-glow {
    height:100%;
    transition:width .2s ease;
    position:relative;
  }
  .progress-meta {
    display:flex;
    justify-content:center;
    gap:.5rem;
    margin-top:.75rem;
    font-family:var(--cui-font-mono);
    font-size:.7rem;
    font-weight:700;
    color:var(--cui-text-tertiary);
  }
  .progress-meta span:nth-child(2) {
    opacity:.4;
    font-size:.6rem;
  }
  .details-grid-premium {
    padding:0 2rem 2rem;
    display:flex;
    flex-direction:column;
    gap:1.5rem;
    flex:1;
    overflow-y:auto;
  }
  .details-section {
    display:flex;
    flex-direction:column;
    gap:1rem;
  }
  .section-label {
    font-size:.65rem;
    font-weight:900;
    color:var(--cui-primary);
    letter-spacing:.1em;
    display:flex;
    align-items:center;
    gap:.75rem;
  }
  .section-label:after {
    content:"";
    flex:1;
    height:1px;
    background:linear-gradient(90deg,rgba(0,243,255,.2),transparent);
  }
  .section-label.danger {
    color:var(--cui-danger);
  }
  .section-label.danger:after {
    background:linear-gradient(90deg,rgba(255,82,82,.2),transparent);
  }
  .data-row-premium {
    display:grid;
    grid-template-columns:1fr 1fr;
    gap:1.5rem;
  }
  .data-item {
    display:flex;
    align-items:center;
    gap:.85rem;
  }
  .data-item .material-icons {
    font-size:1.25rem;
    color:var(--cui-text-tertiary);
  }
  .item-content {
    display:flex;
    flex-direction:column;
    gap:.15rem;
  }
  .item-content .l {
    font-size:.65rem;
    color:var(--cui-text-tertiary);
    font-weight:600;
  }
  .item-content .v {
    font-size:.85rem;
    color:var(--cui-text-primary);
    font-weight:600;
  }
  .link-item-premium {
    display:flex;
    align-items:center;
    justify-content:space-between;
    padding:.85rem 1rem;
    background:#ffffff05;
    border:1px solid rgba(255,255,255,.05);
    border-radius:12px;
    gap:1rem;
    transition:all .2s;
  }
  .link-item-premium:hover {
    background:#ffffff0a;
    border-color:#ffffff1a;
  }
  .link-info {
    display:flex;
    align-items:center;
    gap:1rem;
    min-width:0;
  }
  .link-icon {
    font-size:1.25rem;
    color:var(--cui-primary);
  }
  .link-text {
    display:flex;
    flex-direction:column;
    gap:.15rem;
    min-width:0;
  }
  .link-text .l {
    font-size:.6rem;
    font-weight:800;
    color:var(--cui-text-tertiary);
    text-transform:uppercase;
  }
  .link-text .v {
    font-size:.75rem;
    font-family:var(--cui-font-mono);
    color:var(--cui-text-secondary);
    white-space:nowrap;
    overflow:hidden;
    text-overflow:ellipsis;
  }
  .copy-tiny {
    background:transparent;
    border:none;
    color:var(--cui-text-tertiary);
    cursor:pointer;
    transition:all .2s;
    padding:.4rem;
    border-radius:6px;
  }
  .copy-tiny:hover {
    color:var(--cui-primary);
    background:#00f3ff1a;
  }
  .error-box-premium {
    display:flex;
    gap:1rem;
    padding:1.25rem;
    background:#ff52520d;
    border:1px solid rgba(255,82,82,.15);
    border-radius:12px;
    align-items:flex-start;
  }
  .error-box-premium .material-icons {
    color:var(--cui-danger);
    font-size:1.5rem;
  }
  .error-box-premium p {
    margin:0;
    font-size:.8rem;
    color:#ff8a80;
    line-height:1.5;
  }
  .details-footer-premium {
    padding:1.5rem 2rem;
    border-top:1px solid rgba(255,255,255,.05);
    display:flex;
    justify-content:flex-end;
    background:#0000001a;
  }
  .batch-stats-grid {
    grid-template-columns:1fr 1fr;
  }
  .batch-stats-compact {
    display:grid;
    grid-template-columns:1fr 1fr;
    gap:.5rem 1rem;
    margin-top:.75rem;
  }
  .stat-row {
    display:flex;
    justify-content:space-between;
    align-items:center;
    padding:.5rem 0;
  }
  .stat-label-compact {
    font-size:.875rem;
    color:var(--cui-text-secondary);
    display:inline-flex;
    align-items:center;
    gap:4px;
  }
  .stat-icon {
    font-size:.9rem!important;
    color:#fff6;
    vertical-align:middle;
  }
  .stat-value-compact {
    font-size:.875rem;
    font-weight:700;
    color:var(--cui-text-primary);
  }
  .batch-actions-footer {
    display:flex;
    justify-content:space-between;
    align-items:center;
    gap:1rem;
  }
  .batch-action-buttons {
    display:flex;
    gap:.75rem;
  }
  @media(max-width:767px) {
    .details-hero-section {
      padding:1rem;
    }
    .hero-stats {
      gap:.75rem;
      flex-wrap:wrap;
    }
    .details-grid-premium {
      padding:0 1rem 1rem;
      gap:1rem;
    }
    .data-row-premium,
    .batch-stats-grid,
    .batch-stats-compact {
      grid-template-columns:1fr;
    }
    .batch-actions-footer,
    .del-modal-actions {
      flex-direction:column;
      align-items:stretch;
    }
    .batch-action-buttons {
      flex-direction:column;
    }
    .filename-title {
      white-space:normal;
    }
    .details-footer-premium {
      padding:1rem;
    }
  }
  @media(prefers-reduced-motion:reduce) {
    .progress-bar-glow,
    .link-item-premium,
    .copy-tiny,
    .id-big-icon,
    .id-badge-pill {
      transition:none!important;
    }
  }
</style>
