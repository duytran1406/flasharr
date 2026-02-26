<script lang="ts">
  import { onMount } from "svelte";
  import { animeFade, animeFly } from "$lib/animations";
  import {
    downloads,
    batches,
    downloadStore,
    activeDownloads,
    queuedDownloads,
    completedDownloads,
    failedDownloads,
    pausedDownloads,
    paginationState,
    engineStats,
    formatBytes,
    formatSpeed,
    formatETA,
    isLoading,
    STATUS_PRIORITY,
    type DownloadTask,
    type DownloadState,
    type BatchSummary,
  } from "$lib/stores/downloads";
  import { toasts } from "$lib/stores/toasts";
  import Modal from "$lib/components/ui/Modal.svelte";
  import Button from "$lib/components/ui/Button.svelte";

  // Filter state - V2 doesn't have filter tabs, just shows all downloads
  type FilterType =
    | "all"
    | "active"
    | "queued"
    | "completed"
    | "failed"
    | "paused";
  let currentFilter = $state<FilterType>("all");
  let searchQuery = $state("");

  // Sorting state - Now uses server-side sorting via store
  type SortColumn = "added" | "status" | "filename" | "size" | "progress";
  type SortDirection = "asc" | "desc";

  // Context menu state
  let contextMenuId = $state<string | null>(null);
  let contextMenuPos = $state({ x: 0, y: 0 });

  // V2: Delete confirmation modal state
  let deleteConfirmTask = $state<DownloadTask | null>(null);

  // Batch delete confirmation modal state
  let deleteConfirmBatch = $state<{
    batchId: string;
    batchName: string;
    itemCount: number;
  } | null>(null);

  // V2: Task details modal state
  let detailsTask = $state<DownloadTask | null>(null);

  // V6: Batch details modal state
  let detailsBatch = $state<BatchGroup | null>(null);

  // V3: Keyboard navigation - selected row
  let selectedRowId = $state<string | null>(null);

  // V3: Completion animation tracking
  let justCompletedIds = $state<Set<string>>(new Set());
  let previousStates = $state<Map<string, DownloadState>>(new Map());

  // V4: Status filter - synced with store's pagination.statusFilter
  let statusFilter = $state<string>("ALL");
  let isStatusDropdownOpen = $state(false);

  // V5: Batch hierarchy - track which batches are expanded
  let expandedBatches = $state<Set<string>>(new Set());

  // V5: Batch context menu
  let batchContextMenuId = $state<string | null>(null);
  let batchContextMenuPos = $state({ x: 0, y: 0 });

  // Mobile: tap-to-expand tracking for action buttons
  let expandedMobileCardId = $state<string | null>(null);
  let expandedMobileBatches = $state<Set<string>>(new Set());

  function toggleMobileCard(id: string) {
    expandedMobileCardId = expandedMobileCardId === id ? null : id;
  }

  async function toggleMobileBatch(batchId: string) {
    if (expandedMobileBatches.has(batchId)) {
      expandedMobileBatches = new Set(
        [...expandedMobileBatches].filter((id) => id !== batchId),
      );
    } else {
      expandedMobileBatches = new Set([...expandedMobileBatches, batchId]);
      try {
        await downloadStore.fetchBatchItems(batchId);
      } catch (err) {
        console.error("Failed to load batch items:", err);
      }
    }
  }

  async function toggleBatch(batchId: string) {
    if (expandedBatches.has(batchId)) {
      expandedBatches = new Set(
        [...expandedBatches].filter((id) => id !== batchId),
      );
    } else {
      expandedBatches = new Set([...expandedBatches, batchId]);
      // Lazy load batch items when expanding
      try {
        await downloadStore.fetchBatchItems(batchId);
      } catch (err) {
        console.error("Failed to load batch items:", err);
        toasts.error("Failed to load batch items");
      }
    }
  }

  // V5: Batch group interface for hierarchy display
  interface BatchGroup {
    batchId: string;
    batchName: string;
    downloads: DownloadTask[];
    totalItems: number; // Total count from API (not downloads.length which is lazy-loaded)
    totalSize: number;
    downloaded: number;
    progress: number;
    speed: number;
    activeCount: number;
    completedCount: number;
    failedCount: number;
    pausedCount: number;
    queuedCount: number;
    createdAt: string; // Timestamp of first download in batch
  }

  // V4: Status counts from ENGINE_STATS WebSocket message (comes from database via db_counts)
  // These are updated every 2 seconds via WebSocket and have accurate global counts
  let filterCounts = $derived.by(() => {
    const downloading =
      $engineStats.db_counts?.downloading ?? $engineStats.active_downloads;
    const queued = $engineStats.db_counts?.queued ?? $engineStats.queued;
    const completed =
      $engineStats.db_counts?.completed ?? $engineStats.completed;
    const failed = $engineStats.db_counts?.failed ?? $engineStats.failed;
    const paused = $engineStats.db_counts?.paused ?? $engineStats.paused;
    const cancelled =
      $engineStats.db_counts?.cancelled ?? $engineStats.cancelled;
    // Calculate all from sum if db_counts.all is not available
    const all =
      $engineStats.db_counts?.all ??
      downloading + queued + completed + failed + paused + cancelled;
    return { downloading, queued, completed, failed, paused, cancelled, all };
  });

  // V2 Color System
  function getStateColorV2(state: DownloadState): string {
    const isCompleted = state === "COMPLETED";
    const isError = state === "FAILED";
    const isDownloading =
      state === "DOWNLOADING" || state === "EXTRACTING" || state === "STARTING";
    const isQueued = state === "QUEUED" || state === "WAITING";

    if (isCompleted) return "#00ffa3";
    if (isError) return "#FF5252";
    if (isDownloading) return "#00f3ff";
    if (isQueued) return "#ffd700";
    return "#64748b"; // paused/other
  }

  // V2 Icon System
  function getStateIconV2(state: DownloadState): string {
    const isCompleted = state === "COMPLETED";
    const isError = state === "FAILED";
    const isDownloading =
      state === "DOWNLOADING" || state === "EXTRACTING" || state === "STARTING";
    const isQueued = state === "QUEUED" || state === "WAITING";

    if (isCompleted) return "check_circle";
    if (isError) return "report_problem";
    if (isDownloading) return "sync";
    if (isQueued) return "hourglass_bottom";
    return "pause_circle";
  }

  // Format added date - V2 feature
  function formatAddedDate(dateStr: string): string {
    if (!dateStr) return "-";
    try {
      const d = new Date(dateStr);
      return (
        d.toLocaleDateString() +
        " " +
        d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
      );
    } catch {
      return "-";
    }
  }

  // Sort icon - V2 feature (uses store state)
  function getSortIcon(col: SortColumn): string {
    if ($paginationState.sortBy !== col) return "sort";
    return $paginationState.sortDir === "asc" ? "expand_less" : "expand_more";
  }

  // Toggle sort - V2 feature (now uses server-side sorting)
  function toggleSort(col: SortColumn) {
    const currentSortBy = $paginationState.sortBy;
    const currentSortDir = $paginationState.sortDir;

    if (currentSortBy === col) {
      // Toggle direction
      downloadStore.setSort(col, currentSortDir === "asc" ? "desc" : "asc");
    } else {
      // New column, default to desc
      downloadStore.setSort(col, "desc");
    }
  }

  // Filtered downloads - now uses server-side data, only client-side filtering for tabs
  let filteredDownloads = $derived(() => {
    let list: DownloadTask[] = $downloads;

    // Apply filter (client-side filter for tabs - server returns all)
    switch (currentFilter) {
      case "active":
        list = list.filter(
          (d) => d.state === "DOWNLOADING" || d.state === "STARTING",
        );
        break;
      case "queued":
        list = list.filter((d) => d.state === "QUEUED");
        break;
      case "completed":
        list = list.filter((d) => d.state === "COMPLETED");
        break;
      case "failed":
        list = list.filter((d) => d.state === "FAILED");
        break;
      case "paused":
        list = list.filter((d) => d.state === "PAUSED");
        break;
      default:
        // "all" - no filter
        break;
    }

    // Apply search (client-side)
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      list = list.filter(
        (d) =>
          d.filename.toLowerCase().includes(q) ||
          d.id.toLowerCase().includes(q),
      );
    }

    // V3: Apply client-side priority sorting when sorting by status
    const sortBy = $paginationState.sortBy;
    const sortDir = $paginationState.sortDir;

    if (sortBy === "status") {
      // Priority-based sorting: FAILED first, COMPLETED last
      list = [...list].sort((a, b) => {
        const priorityA = STATUS_PRIORITY[a.state] ?? 99;
        const priorityB = STATUS_PRIORITY[b.state] ?? 99;
        const diff = priorityA - priorityB;

        // If same priority, sort by progress (higher progress = closer to done)
        if (diff === 0) {
          return b.progress - a.progress;
        }

        return sortDir === "asc" ? -diff : diff; // asc = errors first, desc = completed first
      });
    } else if (sortBy === "filename") {
      // Natural sort for filenames
      list = [...list].sort((a, b) => {
        const result = a.filename.localeCompare(b.filename, undefined, {
          numeric: true,
          sensitivity: "base",
        });
        return sortDir === "asc" ? result : -result;
      });
    } else if (sortBy === "progress") {
      list = [...list].sort((a, b) => {
        const diff = a.progress - b.progress;
        return sortDir === "asc" ? diff : -diff;
      });
    } else if (sortBy === "size") {
      list = [...list].sort((a, b) => {
        const diff = a.size - b.size;
        return sortDir === "asc" ? diff : -diff;
      });
    }
    // For "added", server-side sorting is already applied

    return list;
  });

  // Use server-side pagination info
  let totalPages = $derived($paginationState.totalPages || 1);
  let currentPage = $derived($paginationState.page);

  // V5: Use server-side batch summaries instead of client-side grouping
  let groupedDownloads = $derived(() => {
    // Convert BatchSummary to BatchGroup interface for compatibility
    const groups: BatchGroup[] = $batches.map((batch) => {
      // Get batch items from cache if expanded
      let batchItems = expandedBatches.has(batch.batch_id)
        ? $downloadStore.batchItems.get(batch.batch_id) || []
        : [];

      // Sort batch items to match table sorting (client-side for batch children)
      if (batchItems.length > 0) {
        batchItems = [...batchItems].sort((a, b) => {
          let comparison = 0;

          // Sort by filename (alphabetical)
          if (a.filename < b.filename) comparison = -1;
          else if (a.filename > b.filename) comparison = 1;

          return comparison;
        });
      }

      return {
        batchId: batch.batch_id,
        batchName: batch.batch_name,
        downloads: batchItems,
        totalItems: batch.total_items,
        totalSize: batch.total_size,
        downloaded: batch.downloaded_size,
        progress: batch.progress,
        speed: batch.speed,
        activeCount: batch.downloading_items,
        completedCount: batch.completed_items,
        failedCount: batch.failed_items,
        pausedCount: batch.paused_items,
        queuedCount: batch.queued_items,
        createdAt: batch.created_at,
      };
    });

    // Standalone downloads - filter out items that belong to batches
    // This prevents duplicates when batches are expanded
    const batchIds = new Set($batches.map((b) => b.batch_id));
    const standalone = $downloads.filter(
      (d) => !d.batch_id || !batchIds.has(d.batch_id),
    );

    return { groups, standalone };
  });

  // V6: Filter batches by search query and status filter
  let filteredBatches = $derived(() => {
    let batches = groupedDownloads().groups;
    const q = searchQuery.toLowerCase();

    // Apply search filter
    if (q) {
      batches = batches.filter((batch) =>
        batch.batchName.toLowerCase().includes(q),
      );
    }

    // Apply status filter
    if (statusFilter !== "ALL") {
      batches = batches.filter((batch) => {
        // Filter based on batch's aggregate state
        if (statusFilter === "DOWNLOADING") {
          return batch.activeCount > 0;
        } else if (statusFilter === "PAUSED") {
          return (
            batch.pausedCount > 0 && batch.completedCount < batch.totalItems
          );
        } else if (statusFilter === "COMPLETED") {
          return batch.completedCount === batch.totalItems;
        } else if (statusFilter === "FAILED") {
          return batch.failedCount > 0;
        } else if (statusFilter === "QUEUED") {
          return batch.queuedCount > 0;
        } else if (statusFilter === "CANCELLED") {
          // Batches don't have a cancelled count, so we'll skip this for now
          return false;
        }
        return true;
      });
    }

    return batches;
  });

  // Helper to get batch aggregate status
  function getBatchStatus(group: BatchGroup): {
    state: string;
    color: string;
    icon: string;
  } {
    // Count how many different states exist in this batch
    const stateCount = [
      group.failedCount > 0 ? 1 : 0,
      group.activeCount > 0 ? 1 : 0,
      group.pausedCount > 0 ? 1 : 0,
      group.queuedCount > 0 ? 1 : 0,
      group.completedCount > 0 ? 1 : 0,
    ].reduce((a, b) => a + b, 0);

    // If there are multiple states, show as MIXED with details
    if (stateCount > 1) {
      const parts: string[] = [];
      if (group.completedCount > 0) parts.push(`${group.completedCount} done`);
      if (group.activeCount > 0) parts.push(`${group.activeCount} active`);
      if (group.pausedCount > 0) parts.push(`${group.pausedCount} paused`);
      if (group.failedCount > 0) parts.push(`${group.failedCount} failed`);
      if (group.queuedCount > 0) parts.push(`${group.queuedCount} queued`);

      // Use the most important state's color (priority: failed > active > paused > queued > completed)
      let primaryColor = getStateColorV2("QUEUED");
      let primaryIcon = getStateIconV2("QUEUED");

      if (group.failedCount > 0) {
        primaryColor = getStateColorV2("FAILED");
        primaryIcon = getStateIconV2("FAILED");
      } else if (group.activeCount > 0) {
        primaryColor = getStateColorV2("DOWNLOADING");
        primaryIcon = getStateIconV2("DOWNLOADING");
      } else if (group.pausedCount > 0) {
        primaryColor = getStateColorV2("PAUSED");
        primaryIcon = getStateIconV2("PAUSED");
      } else if (group.queuedCount > 0) {
        primaryColor = getStateColorV2("QUEUED");
        primaryIcon = getStateIconV2("QUEUED");
      }

      return {
        state: `MIXED: ${parts.join(", ")}`,
        color: primaryColor,
        icon: primaryIcon,
      };
    }

    // Single state - use original logic
    if (group.failedCount > 0) {
      return {
        state: "FAILED",
        color: getStateColorV2("FAILED"),
        icon: getStateIconV2("FAILED"),
      };
    } else if (group.activeCount > 0) {
      return {
        state: "DOWNLOADING",
        color: getStateColorV2("DOWNLOADING"),
        icon: getStateIconV2("DOWNLOADING"),
      };
    } else if (
      group.pausedCount > 0 &&
      group.completedCount < group.totalItems
    ) {
      return {
        state: "PAUSED",
        color: getStateColorV2("PAUSED"),
        icon: getStateIconV2("PAUSED"),
      };
    } else if (group.queuedCount > 0) {
      return {
        state: "QUEUED",
        color: getStateColorV2("QUEUED"),
        icon: getStateIconV2("QUEUED"),
      };
    } else if (group.completedCount === group.totalItems) {
      return {
        state: "COMPLETED",
        color: getStateColorV2("COMPLETED"),
        icon: getStateIconV2("COMPLETED"),
      };
    }
    return {
      state: "QUEUED",
      color: getStateColorV2("QUEUED"),
      icon: getStateIconV2("QUEUED"),
    };
  }

  // Pagination info text - V2 style
  let paginationInfo = $derived(() => {
    const total = $paginationState.total;
    if (total === 0) return "Showing 0 items";
    const limit = $paginationState.limit;
    const page = $paginationState.page;
    const start = (page - 1) * limit + 1;
    const end = Math.min(page * limit, total);
    return `Showing ${start}-${end} of ${total} items`;
  });

  function setPage(p: number) {
    if (p >= 1 && p <= totalPages) {
      downloadStore.setPage(p);
    }
  }

  // Check if download is active (downloading/extracting/starting)
  function isDownloadActive(state: DownloadState): boolean {
    return (
      state === "DOWNLOADING" || state === "EXTRACTING" || state === "STARTING"
    );
  }

  // Actions
  async function pauseDownload(id: string) {
    const result = await downloadStore.pauseDownload(id);
    if (!result.success) {
      toasts.error(`Failed to pause: ${result.error}`);
    }
  }

  async function resumeDownload(id: string) {
    const result = await downloadStore.resumeDownload(id);
    if (!result.success) {
      toasts.error(`Failed to resume: ${result.error}`);
    }
  }

  async function deleteDownload(id: string) {
    if (!confirm("Are you sure you want to delete this download?")) return;

    const result = await downloadStore.deleteDownload(id);
    if (!result.success) {
      toasts.error(`Failed to delete: ${result.error}`);
    }
  }

  async function retryDownload(id: string) {
    const result = await downloadStore.retryDownload(id);
    if (!result.success) {
      toasts.error(`Failed to retry: ${result.error}`);
    }
  }

  async function pauseAll() {
    const result = await downloadStore.pauseAll();
    if (!result.success) {
      toasts.error(`Failed to pause all: ${result.error}`);
    }
  }

  async function resumeAll() {
    const result = await downloadStore.resumeAll();
    if (!result.success) {
      toasts.error(`Failed to resume all: ${result.error}`);
    }
  }

  function applyStatusFilter(filter: string) {
    statusFilter = filter;
    isStatusDropdownOpen = false;
    downloadStore.setStatusFilter(
      filter === "ALL" ? null : (filter as DownloadState),
    );
  }

  function showContextMenu(e: MouseEvent, id: string) {
    e.preventDefault();
    contextMenuId = id;
    contextMenuPos = { x: e.clientX, y: e.clientY };
  }

  function hideContextMenu() {
    contextMenuId = null;
    batchContextMenuId = null;
  }

  // Batch context menu
  function showBatchContextMenu(e: MouseEvent, batchId: string) {
    e.preventDefault();
    e.stopPropagation();
    batchContextMenuId = batchId;
    batchContextMenuPos = { x: e.clientX, y: e.clientY };
  }

  async function handleBatchAction(action: string, batchId: string) {
    try {
      let response;
      switch (action) {
        case "pause":
          response = await fetch(`/api/downloads/batch/${batchId}/pause`, {
            method: "POST",
          });
          if (response.ok) toasts.success("Batch paused");
          break;
        case "resume":
          response = await fetch(`/api/downloads/batch/${batchId}/resume`, {
            method: "POST",
          });
          if (response.ok) toasts.success("Batch resumed");
          break;
        case "delete":
          // Find batch info for confirmation modal
          const batch = filteredBatches().find((b) => b.batchId === batchId);
          if (batch) {
            showBatchDeleteConfirm(batchId, batch.batchName, batch.totalItems);
          }
          return; // Don't hide context menu yet, modal will handle it
      }
    } catch (err) {
      toasts.error(`Batch action failed: ${err}`);
    }
    hideContextMenu();
  }

  async function handleAction(action: string, id: string) {
    switch (action) {
      case "pause":
        await pauseDownload(id);
        break;
      case "resume":
        await resumeDownload(id);
        break;
      case "delete":
        await deleteDownload(id);
        break;
      case "retry":
        await retryDownload(id);
        break;
    }
    hideContextMenu();
  }

  // Copy error to clipboard
  function copyError(errorMsg: string) {
    navigator.clipboard.writeText(errorMsg);
    toasts.success("Error message copied to clipboard");
    hideContextMenu();
  }

  // V2: Delete confirmation modal functions
  function showDeleteConfirm(task: DownloadTask) {
    deleteConfirmTask = task;
    hideContextMenu();
  }

  function hideDeleteConfirm() {
    deleteConfirmTask = null;
  }

  function showBatchDeleteConfirm(
    batchId: string,
    batchName: string,
    itemCount: number,
  ) {
    deleteConfirmBatch = { batchId, batchName, itemCount };
    hideContextMenu();
  }

  function hideBatchDeleteConfirm() {
    deleteConfirmBatch = null;
  }

  async function confirmDelete() {
    if (!deleteConfirmTask) return;
    const id = deleteConfirmTask.id;
    deleteConfirmTask = null;

    const result = await downloadStore.deleteDownload(id);
    if (!result.success) {
      toasts.error(`Failed to delete: ${result.error}`);
    }
  }

  async function confirmBatchDelete() {
    if (!deleteConfirmBatch) return;
    const { batchId, batchName } = deleteConfirmBatch;
    deleteConfirmBatch = null;

    try {
      const response = await fetch(`/api/downloads/batch/${batchId}`, {
        method: "DELETE",
      });
      if (response.ok) {
        toasts.success(`Batch "${batchName}" deleted`);
      } else {
        toasts.error("Failed to delete batch");
      }
    } catch (err) {
      toasts.error(`Batch deletion failed: ${err}`);
    }
  }

  // V2: Task details modal functions
  function showTaskDetails(task: DownloadTask) {
    detailsTask = task;
    hideContextMenu();
  }

  function hideTaskDetails() {
    detailsTask = null;
  }

  // V6: Batch details modal functions
  function showBatchDetails(batch: BatchGroup) {
    detailsBatch = batch;
    hideContextMenu();
  }

  function hideBatchDetails() {
    detailsBatch = null;
  }

  // V3: Keyboard navigation handler
  function handleKeyboardNav(e: KeyboardEvent) {
    const list = filteredDownloads();
    if (list.length === 0) return;

    // Find current index
    const currentIndex = selectedRowId
      ? list.findIndex((d) => d.id === selectedRowId)
      : -1;

    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        if (currentIndex < list.length - 1) {
          selectedRowId = list[currentIndex + 1].id;
        } else if (currentIndex === -1) {
          selectedRowId = list[0].id;
        }
        break;

      case "ArrowUp":
        e.preventDefault();
        if (currentIndex > 0) {
          selectedRowId = list[currentIndex - 1].id;
        }
        break;

      case " ": // Space - Pause/Resume
        e.preventDefault();
        if (selectedRowId) {
          const task = list.find((d) => d.id === selectedRowId);
          if (task) {
            if (task.state === "DOWNLOADING" || task.state === "STARTING") {
              pauseDownload(selectedRowId);
            } else if (task.state === "PAUSED" || task.state === "QUEUED") {
              resumeDownload(selectedRowId);
            }
          }
        }
        break;

      case "Delete":
      case "Backspace":
        e.preventDefault();
        if (selectedRowId) {
          deleteDownload(selectedRowId);
        }
        break;

      case "e":
      case "E": // Jump to next error
        e.preventDefault();
        const errorTasks = list.filter((d) => d.state === "FAILED");
        if (errorTasks.length > 0) {
          const currentErrorIndex = selectedRowId
            ? errorTasks.findIndex((d) => d.id === selectedRowId)
            : -1;
          const nextIndex = (currentErrorIndex + 1) % errorTasks.length;
          selectedRowId = errorTasks[nextIndex].id;
          toasts.info(`Error ${nextIndex + 1} of ${errorTasks.length}`);
        }
        break;

      case "r":
      case "R": // Retry selected error
        e.preventDefault();
        if (selectedRowId) {
          const task = list.find((d) => d.id === selectedRowId);
          if (task?.state === "FAILED") {
            retryDownload(selectedRowId);
          }
        }
        break;

      case "b": // Toggle batch expand/collapse
        e.preventDefault();
        if (selectedRowId) {
          const task = list.find((d) => d.id === selectedRowId);
          if (task?.batch_id) {
            toggleBatch(task.batch_id);
            toasts.info(
              expandedBatches.has(task.batch_id)
                ? "Batch collapsed"
                : "Batch expanded",
            );
          }
        }
        break;

      case "B": // Pause/Resume entire batch
        e.preventDefault();
        if (selectedRowId) {
          const task = list.find((d) => d.id === selectedRowId);
          if (task?.batch_id) {
            const batch = filteredBatches().find(
              (b) => b.batchId === task.batch_id,
            );
            if (batch) {
              if (batch.pausedCount > 0) {
                downloadStore.resumeBatch(batch.batchId);
                toasts.success("Batch resumed");
              } else if (batch.activeCount > 0 || batch.queuedCount > 0) {
                downloadStore.pauseBatch(batch.batchId);
                toasts.success("Batch paused");
              }
            }
          }
        }
        break;

      case "Escape":
        selectedRowId = null;
        hideContextMenu();
        break;
    }
  }

  // V3: Track completion animations
  $effect(() => {
    const currentDownloads = $downloads;

    for (const dl of currentDownloads) {
      const prevState = previousStates.get(dl.id);

      // Detect transition to COMPLETED
      if (prevState && prevState !== "COMPLETED" && dl.state === "COMPLETED") {
        // Trigger completion animation
        justCompletedIds = new Set([...justCompletedIds, dl.id]);

        // Remove after animation duration (1.5s)
        setTimeout(() => {
          justCompletedIds = new Set(
            [...justCompletedIds].filter((id) => id !== dl.id),
          );
        }, 1500);
      }

      // Update previous state tracker
      previousStates.set(dl.id, dl.state);
    }
  });

  onMount(() => {
    // Sync local statusFilter with store's paginationState
    const storeFilter = $paginationState.statusFilter;
    statusFilter = storeFilter ? storeFilter : "ALL";

    // Set Page Header - V2 style
    const headerContainer = document.getElementById("header-dynamic-content");
    if (headerContainer) {
      headerContainer.innerHTML = `
        <div style="display: flex; align-items: center; gap: 0.75rem;">
          <span class="material-icons" style="color: var(--color-secondary); font-size: 1.5rem;">cloud_download</span>
          <h1 style="font-size: 0.9rem; font-weight: 800; letter-spacing: 0.1em; text-transform: uppercase; margin: 0; color: #fff;">Active Downloads</h1>
        </div>
      `;
    }

    // Always refresh downloads when landing on this page so active queue is current
    downloadStore
      .fetchAll()
      .catch((err) =>
        console.error("[Downloads] Failed to refresh on mount:", err),
      );
  });
</script>

<svelte:head>
  <title>Downloads - Flasharr</title>
</svelte:head>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="downloads-container"
  onclick={hideContextMenu}
  onkeydown={(e) => {
    handleKeyboardNav(e);
    if (e.key === "Escape") hideContextMenu();
  }}
  role="application"
>
  <!-- V2 Style Toolbar -->
  <div class="section-box">
    <div class="toolbar">
      <!-- Search Box (left, prominent) -->
      <div class="search-bar-prominent">
        <span class="material-icons">search</span>
        <input
          type="text"
          placeholder="Search downloads..."
          bind:value={searchQuery}
        />
      </div>

      <!-- Action Buttons (right) -->
      <div class="toolbar-actions">
        <button
          class="btn-tiny btn-success"
          onclick={resumeAll}
          title="Start/Resume All"
        >
          <span class="material-icons">play_arrow</span>
        </button>
        <button
          class="btn-tiny btn-danger"
          onclick={pauseAll}
          title="Pause/Stop All"
        >
          <span class="material-icons">pause</span>
        </button>
        <div class="divider"></div>
        <button
          class="btn-tiny"
          onclick={() => downloadStore.fetchAll()}
          title="Refresh"
        >
          <span class="material-icons">refresh</span>
        </button>
      </div>
    </div>

    <!-- Downloads Content -->
    <div class="downloads-content">
      <!-- V2 Style Table - Header always visible -->
      <div class="table-wrapper">
        <table class="data-table">
          <thead>
            <tr>
              <th style="width: 35%;" onclick={() => toggleSort("filename")}>
                <div class="th-content">
                  <span class="th-label">Filename</span>
                  <span
                    class="material-icons sort-icon"
                    class:active={$paginationState.sortBy === "filename"}
                    >{getSortIcon("filename")}</span
                  >
                </div>
              </th>
              <th style="width: 18%;" class="status-filter-th">
                <div class="th-content status-filter-header">
                  <!-- Dropdown wrapper for proper positioning -->
                  <div class="status-dropdown-wrapper">
                    <!-- Custom glassmorphism dropdown trigger -->
                    <button
                      type="button"
                      class="status-dropdown-trigger"
                      class:open={isStatusDropdownOpen}
                      onclick={(e) => {
                        e.stopPropagation();
                        isStatusDropdownOpen = !isStatusDropdownOpen;
                      }}
                    >
                      <span
                        class="status-badge-v2"
                        style="background: {statusFilter === 'ALL'
                          ? 'rgba(100, 116, 139, 0.2)'
                          : statusFilter === 'DOWNLOADING'
                            ? 'rgba(0, 243, 255, 0.2)'
                            : statusFilter === 'QUEUED'
                              ? 'rgba(255, 215, 0, 0.2)'
                              : statusFilter === 'PAUSED'
                                ? 'rgba(100, 116, 139, 0.2)'
                                : statusFilter === 'COMPLETED'
                                  ? 'rgba(0, 255, 163, 0.2)'
                                  : statusFilter === 'FAILED'
                                    ? 'rgba(255, 82, 82, 0.2)'
                                    : 'rgba(255, 165, 0, 0.2)'}; 
                          color: {statusFilter === 'ALL'
                          ? '#94a3b8'
                          : statusFilter === 'DOWNLOADING'
                            ? '#00f3ff'
                            : statusFilter === 'QUEUED'
                              ? '#ffd700'
                              : statusFilter === 'PAUSED'
                                ? '#64748b'
                                : statusFilter === 'COMPLETED'
                                  ? '#00ffa3'
                                  : statusFilter === 'FAILED'
                                    ? '#ff5252'
                                    : '#ffa500'}; 
                          border-color: {statusFilter === 'ALL'
                          ? 'rgba(148, 163, 184, 0.3)'
                          : statusFilter === 'DOWNLOADING'
                            ? 'rgba(0, 243, 255, 0.3)'
                            : statusFilter === 'QUEUED'
                              ? 'rgba(255, 215, 0, 0.3)'
                              : statusFilter === 'PAUSED'
                                ? 'rgba(100, 116, 139, 0.3)'
                                : statusFilter === 'COMPLETED'
                                  ? 'rgba(0, 255, 163, 0.3)'
                                  : statusFilter === 'FAILED'
                                    ? 'rgba(255, 82, 82, 0.3)'
                                    : 'rgba(255, 165, 0, 0.3)'};"
                      >
                        {statusFilter === "ALL" ? "All" : statusFilter}
                      </span>
                      <span class="dropdown-count"
                        >({statusFilter === "ALL"
                          ? filterCounts.all
                          : statusFilter === "DOWNLOADING"
                            ? filterCounts.downloading
                            : statusFilter === "QUEUED"
                              ? filterCounts.queued
                              : statusFilter === "PAUSED"
                                ? filterCounts.paused
                                : statusFilter === "COMPLETED"
                                  ? filterCounts.completed
                                  : statusFilter === "FAILED"
                                    ? filterCounts.failed
                                    : filterCounts.cancelled})</span
                      >
                      <span
                        class="material-icons dropdown-arrow"
                        class:open={isStatusDropdownOpen}>expand_more</span
                      >
                    </button>

                    <!-- Glassmorphism dropdown menu -->
                    {#if isStatusDropdownOpen}
                      <!-- Backdrop to close on click outside -->
                      <div
                        class="dropdown-backdrop"
                        onclick={() => (isStatusDropdownOpen = false)}
                      ></div>
                      <div
                        class="status-dropdown-menu"
                        transition:animeFly={{ y: -5, duration: 150 }}
                      >
                        <button
                          class="status-option"
                          class:selected={statusFilter === "ALL"}
                          onclick={() => applyStatusFilter("ALL")}
                        >
                          <span class="status-badge-v2 all">All</span>
                          <span class="count">({filterCounts.all})</span>
                          {#if statusFilter === "ALL"}
                            <span class="material-icons check-icon">check</span>
                          {/if}
                        </button>
                        <button
                          class="status-option"
                          class:selected={statusFilter === "DOWNLOADING"}
                          onclick={() => applyStatusFilter("DOWNLOADING")}
                        >
                          <span class="status-badge-v2 downloading"
                            >Downloading</span
                          >
                          <span class="count">({filterCounts.downloading})</span
                          >
                          {#if statusFilter === "DOWNLOADING"}
                            <span class="material-icons check-icon">check</span>
                          {/if}
                        </button>
                        <button
                          class="status-option"
                          class:selected={statusFilter === "QUEUED"}
                          onclick={() => applyStatusFilter("QUEUED")}
                        >
                          <span class="status-badge-v2 queued">Queued</span>
                          <span class="count">({filterCounts.queued})</span>
                          {#if statusFilter === "QUEUED"}
                            <span class="material-icons check-icon">check</span>
                          {/if}
                        </button>
                        <button
                          class="status-option"
                          class:selected={statusFilter === "PAUSED"}
                          onclick={() => applyStatusFilter("PAUSED")}
                        >
                          <span class="status-badge-v2 paused">Paused</span>
                          <span class="count">({filterCounts.paused})</span>
                          {#if statusFilter === "PAUSED"}
                            <span class="material-icons check-icon">check</span>
                          {/if}
                        </button>
                        <button
                          class="status-option"
                          class:selected={statusFilter === "COMPLETED"}
                          onclick={() => applyStatusFilter("COMPLETED")}
                        >
                          <span class="status-badge-v2 completed"
                            >Completed</span
                          >
                          <span class="count">({filterCounts.completed})</span>
                          {#if statusFilter === "COMPLETED"}
                            <span class="material-icons check-icon">check</span>
                          {/if}
                        </button>
                        <button
                          class="status-option"
                          class:selected={statusFilter === "FAILED"}
                          onclick={() => applyStatusFilter("FAILED")}
                        >
                          <span class="status-badge-v2 failed">Failed</span>
                          <span class="count">({filterCounts.failed})</span>
                          {#if statusFilter === "FAILED"}
                            <span class="material-icons check-icon">check</span>
                          {/if}
                        </button>
                        <button
                          class="status-option"
                          class:selected={statusFilter === "CANCELLED"}
                          onclick={() => applyStatusFilter("CANCELLED")}
                        >
                          <span class="status-badge-v2 cancelled"
                            >Cancelled</span
                          >
                          <span class="count">({filterCounts.cancelled})</span>
                          {#if statusFilter === "CANCELLED"}
                            <span class="material-icons check-icon">check</span>
                          {/if}
                        </button>
                      </div>
                    {/if}
                  </div>
                </div>
              </th>
              <th style="width: 12%;" onclick={() => toggleSort("size")}>
                <div class="th-content">
                  <span class="th-label">Size</span>
                  <span
                    class="material-icons sort-icon"
                    class:active={$paginationState.sortBy === "size"}
                    >{getSortIcon("size")}</span
                  >
                </div>
              </th>
              <th style="width: 15%;" onclick={() => toggleSort("progress")}>
                <div class="th-content">
                  <span class="th-label">Progress</span>
                  <span
                    class="material-icons sort-icon"
                    class:active={$paginationState.sortBy === "progress"}
                    >{getSortIcon("progress")}</span
                  >
                </div>
              </th>
              <th style="width: 10%;">
                <div class="th-content">
                  <span class="th-label">Speed</span>
                </div>
              </th>
              <th style="width: 8%;">
                <div class="th-content"><span class="th-label">ETA</span></div>
              </th>
              <th style="width: 13%;" onclick={() => toggleSort("added")}>
                <div class="th-content">
                  <span class="th-label">Added</span>
                  <span
                    class="material-icons sort-icon"
                    class:active={$paginationState.sortBy === "added"}
                    >{getSortIcon("added")}</span
                  >
                </div>
              </th>
            </tr>
          </thead>
          <tbody>
            {#if $isLoading && $downloads.length === 0 && $batches.length === 0}
              <!-- Skeleton loader rows -->
              {#each Array(5) as _, i}
                <tr class="skeleton-row">
                  <td class="filename-cell">
                    <div class="skeleton-box" style="width: 65%;"></div>
                  </td>
                  <td>
                    <div class="skeleton-box" style="width: 70px;"></div>
                  </td>
                  <td class="size-cell">
                    <div class="skeleton-box" style="width: 80px;"></div>
                  </td>
                  <td>
                    <div
                      class="skeleton-box progress-skeleton"
                      style="width: 100%;"
                    ></div>
                  </td>
                  <td class="speed-cell">
                    <div class="skeleton-box" style="width: 70px;"></div>
                  </td>
                  <td class="eta-cell">
                    <div class="skeleton-box" style="width: 60px;"></div>
                  </td>
                  <td class="date-cell">
                    <div class="skeleton-box" style="width: 100px;"></div>
                  </td>
                </tr>
              {/each}
            {:else if filteredBatches().length === 0 && filteredDownloads().length === 0}
              <!-- Empty state row -->
              <tr>
                <td colspan="8" class="table-empty-cell">
                  <div class="empty-state-inline">
                    <span class="material-icons">inbox</span>
                    <span>
                      {#if statusFilter !== "ALL"}
                        No {statusFilter.toLowerCase()} downloads
                        <button
                          class="inline-link"
                          onclick={() => {
                            statusFilter = "ALL";
                            downloadStore.setStatusFilter(null);
                          }}
                        >
                          Show all
                        </button>
                      {:else if searchQuery}
                        No results for "{searchQuery}"
                        <button
                          class="inline-link"
                          onclick={() => (searchQuery = "")}
                        >
                          Clear search
                        </button>
                      {:else}
                        Queue is empty
                      {/if}
                    </span>
                  </div>
                </td>
              </tr>
            {:else}
              <!-- V5: Render batch groups first -->
              {#each filteredBatches() as group (group.batchId)}
                {@const batchStatus = getBatchStatus(group)}
                {@const isExpanded = expandedBatches.has(group.batchId)}

                <!-- Batch header row (collapsible with context menu) -->
                <tr
                  class="batch-header-row"
                  class:batch-expanded={isExpanded}
                  onclick={() => toggleBatch(group.batchId)}
                  oncontextmenu={(e) => showBatchContextMenu(e, group.batchId)}
                >
                  <td class="filename-cell batch-name-cell">
                    <span class="expand-icon material-icons">
                      {isExpanded ? "expand_less" : "expand_more"}
                    </span>
                    <span class="batch-name">{group.batchName}</span>
                    <span class="batch-count">({group.totalItems} files)</span>
                  </td>
                  <td>
                    <span
                      class="status-badge-v2"
                      style="background: {batchStatus.color}15; color: {batchStatus.color}; border-color: {batchStatus.color}30;"
                    >
                      <span class="material-icons" style="font-size: 9px;"
                        >{batchStatus.icon}</span
                      >
                      {group.completedCount}/{group.totalItems}
                    </span>
                  </td>
                  <td class="size-cell">{formatBytes(group.totalSize)}</td>
                  <td>
                    <div class="progress-cell-v2">
                      <div
                        class="progress-bar-v2"
                        style="--progress: {group.progress}%; --bar-color: {batchStatus.color};"
                      >
                        <div class="progress-fill-v2"></div>
                      </div>
                      <span class="progress-text-v2"
                        >{group.progress.toFixed(1)}%</span
                      >
                    </div>
                  </td>
                  <td class="speed-cell">
                    {#if group.speed > 0}
                      {formatSpeed(group.speed)}
                    {:else}
                      —
                    {/if}
                  </td>
                  <td class="eta-cell">
                    {#if group.speed > 0 && group.totalSize > group.downloaded}
                      {@const remainingBytes =
                        group.totalSize - group.downloaded}
                      {@const etaSec = Math.round(remainingBytes / group.speed)}
                      {formatETA(etaSec)}
                    {:else}
                      —
                    {/if}
                  </td>
                  <td class="date-cell">{formatAddedDate(group.createdAt)}</td>
                </tr>

                <!-- Child rows (only if expanded) -->
                {#if isExpanded}
                  {#each group.downloads as download (download.id)}
                    {@const color = getStateColorV2(download.state)}
                    {@const icon = getStateIconV2(download.state)}
                    {@const isActive = isDownloadActive(download.state)}
                    {@const isError = download.state === "FAILED"}
                    {@const isSelected = selectedRowId === download.id}
                    {@const isJustCompleted = justCompletedIds.has(download.id)}
                    <tr
                      class="transfer-row batch-child-row"
                      class:error-row={isError}
                      class:selected-row={isSelected}
                      class:just-completed={isJustCompleted}
                      data-id={download.id}
                      onclick={() => (selectedRowId = download.id)}
                      oncontextmenu={(e) => showContextMenu(e, download.id)}
                    >
                      <td
                        class="filename-cell batch-child-filename"
                        title={download.filename}
                      >
                        <span class="filename-text">{download.filename}</span>
                        {#if download.quality}
                          <span class="quality-badge" title={download.quality}
                            >{download.resolution || download.quality}</span
                          >
                        {/if}
                      </td>
                      <td>
                        <span
                          class="status-badge-v2"
                          style="background: {color}15; color: {color}; border-color: {color}30;"
                          title={download.error_message || ""}
                        >
                          <span class="material-icons" style="font-size: 9px;"
                            >{icon}</span
                          >
                          {download.state}
                        </span>
                        {#if download.error_message}
                          <div
                            class="error-preview"
                            title={download.error_message}
                          >
                            {download.error_message.slice(0, 40)}...
                          </div>
                        {/if}
                      </td>
                      <td class="size-cell">{formatBytes(download.size)}</td>
                      <td>
                        <div class="progress-cell-v2">
                          <div
                            class="progress-bar-v2"
                            style="--progress: {download.progress}%; --bar-color: {color};"
                          >
                            <div class="progress-fill-v2"></div>
                          </div>
                          <span class="progress-text-v2"
                            >{download.progress.toFixed(1)}%</span
                          >
                        </div>
                      </td>
                      <td class="speed-cell">
                        {#if isActive && download.speed}
                          {formatSpeed(download.speed)}
                        {:else}
                          —
                        {/if}
                      </td>
                      <td class="eta-cell">
                        {#if isActive && download.eta}
                          {formatETA(download.eta)}
                        {:else}
                          —
                        {/if}
                      </td>
                      <td class="date-cell">
                        {formatAddedDate(download.created_at)}
                      </td>
                    </tr>
                  {/each}
                {/if}
              {/each}

              <!-- Standalone downloads (no batch) -->
              {#each groupedDownloads().standalone as download (download.id)}
                {@const color = getStateColorV2(download.state)}
                {@const icon = getStateIconV2(download.state)}
                {@const isActive = isDownloadActive(download.state)}
                {@const isError = download.state === "FAILED"}
                {@const isSelected = selectedRowId === download.id}
                {@const isJustCompleted = justCompletedIds.has(download.id)}
                <tr
                  class="transfer-row"
                  class:error-row={isError}
                  class:selected-row={isSelected}
                  class:just-completed={isJustCompleted}
                  data-id={download.id}
                  onclick={() => (selectedRowId = download.id)}
                  oncontextmenu={(e) => showContextMenu(e, download.id)}
                >
                  <td class="filename-cell" title={download.filename}>
                    <span class="filename-text">{download.filename}</span>
                    {#if download.quality}
                      <span class="quality-badge" title={download.quality}
                        >{download.resolution || download.quality}</span
                      >
                    {/if}
                  </td>
                  <td>
                    <span
                      class="status-badge-v2"
                      style="background: {color}15; color: {color}; border-color: {color}30;"
                      title={download.error_message || ""}
                    >
                      <span class="material-icons" style="font-size: 9px;"
                        >{icon}</span
                      >
                      {download.state}
                    </span>
                    {#if download.error_message}
                      <div class="error-preview" title={download.error_message}>
                        {download.error_message.slice(0, 40)}...
                      </div>
                    {/if}
                  </td>
                  <td class="size-cell">{formatBytes(download.size)}</td>
                  <td>
                    <div class="progress-cell-v2">
                      <div
                        class="progress-bar-v2"
                        style="--progress: {download.progress}%; --bar-color: {color};"
                      >
                        <div class="progress-fill-v2"></div>
                      </div>
                      <span class="progress-text-v2"
                        >{download.progress.toFixed(1)}%</span
                      >
                    </div>
                  </td>
                  <td class="speed-cell">
                    {#if isActive && download.speed}
                      {formatSpeed(download.speed)}
                    {:else}
                      —
                    {/if}
                  </td>
                  <td class="eta-cell">
                    {#if isActive && download.eta}
                      {formatETA(download.eta)}
                    {:else}
                      —
                    {/if}
                  </td>
                  <td class="date-cell">
                    {formatAddedDate(download.created_at)}
                  </td>
                </tr>
              {/each}
            {/if}
          </tbody>
        </table>
      </div>

      <!-- V2 Style Pagination -->
      <div class="pagination-footer-v2">
        <span class="page-info-v2">{paginationInfo()}</span>
        <div class="page-controls">
          <button
            class="icon-btn-tiny"
            onclick={() => setPage(currentPage - 1)}
            disabled={currentPage === 1}
          >
            <span class="material-icons">chevron_left</span>
          </button>
          <div class="page-indicator">{currentPage} / {totalPages}</div>
          <button
            class="icon-btn-tiny"
            onclick={() => setPage(currentPage + 1)}
            disabled={currentPage === totalPages}
          >
            <span class="material-icons">chevron_right</span>
          </button>
        </div>
      </div>
    </div>
  </div>

  <!-- Mobile Card View -->
  <div class="mobile-cards">
    <!-- Mobile Status Filter Chips -->
    <div class="mobile-status-chips">
      <button
        class="mobile-status-chip"
        class:active={statusFilter === "ALL"}
        onclick={() => {
          statusFilter = "ALL";
          downloadStore.setStatusFilter(null);
        }}
      >
        All <span class="chip-count">({filterCounts.all})</span>
      </button>
      <button
        class="mobile-status-chip"
        class:active={statusFilter === "DOWNLOADING"}
        onclick={() => {
          statusFilter = "DOWNLOADING";
          downloadStore.setStatusFilter("DOWNLOADING");
        }}
      >
        Active <span class="chip-count">({filterCounts.downloading})</span>
      </button>
      <button
        class="mobile-status-chip"
        class:active={statusFilter === "QUEUED"}
        onclick={() => {
          statusFilter = "QUEUED";
          downloadStore.setStatusFilter("QUEUED");
        }}
      >
        Queued <span class="chip-count">({filterCounts.queued})</span>
      </button>
      <button
        class="mobile-status-chip"
        class:active={statusFilter === "PAUSED"}
        onclick={() => {
          statusFilter = "PAUSED";
          downloadStore.setStatusFilter("PAUSED");
        }}
      >
        Paused <span class="chip-count">({filterCounts.paused})</span>
      </button>
      <button
        class="mobile-status-chip"
        class:active={statusFilter === "COMPLETED"}
        onclick={() => {
          statusFilter = "COMPLETED";
          downloadStore.setStatusFilter("COMPLETED");
        }}
      >
        Done <span class="chip-count">({filterCounts.completed})</span>
      </button>
      <button
        class="mobile-status-chip"
        class:active={statusFilter === "FAILED"}
        onclick={() => {
          statusFilter = "FAILED";
          downloadStore.setStatusFilter("FAILED");
        }}
      >
        Failed <span class="chip-count">({filterCounts.failed})</span>
      </button>
    </div>

    <!-- Mobile Batch Groups -->
    {#each filteredBatches() as group (group.batchId)}
      {@const batchStatus = getBatchStatus(group)}
      {@const isBatchExpanded = expandedMobileBatches.has(group.batchId)}
      <div class="mobile-batch-card">
        <div
          class="mobile-batch-header"
          onclick={() => toggleMobileBatch(group.batchId)}
        >
          <span class="material-icons expand-icon">
            {isBatchExpanded ? "expand_less" : "expand_more"}
          </span>
          <span class="batch-name">{group.batchName}</span>
          <span class="batch-count">({group.totalItems} files)</span>
          <span
            class="status-badge-v2"
            style="background: {batchStatus.color}15; color: {batchStatus.color}; border-color: {batchStatus.color}30;"
          >
            {group.completedCount}/{group.totalItems}
          </span>
        </div>
        {#if isBatchExpanded}
          <div class="mobile-batch-children">
            {#each group.downloads as download (download.id)}
              {@const color = getStateColorV2(download.state)}
              {@const isActive = isDownloadActive(download.state)}
              {@const isExpanded = expandedMobileCardId === download.id}
              <div
                class="download-card-mobile"
                class:expanded={isExpanded}
                onclick={() => toggleMobileCard(download.id)}
              >
                <div class="card-header-mobile">
                  <div class="card-name">
                    {download.filename}
                    {#if download.quality}
                      <span class="quality-badge" title={download.quality}
                        >{download.resolution || download.quality}</span
                      >
                    {/if}
                  </div>
                  <span
                    class="status-badge-v2"
                    style="background: {color}15; color: {color}; border-color: {color}30;"
                  >
                    {download.state}
                  </span>
                </div>
                <div class="card-progress-mobile">
                  <div
                    class="progress-fill-v2"
                    style="width: {download.progress}%; background: {color};"
                  ></div>
                </div>
                <div class="card-meta-mobile">
                  <div class="meta-item">
                    <span class="material-icons">storage</span>
                    <span>{formatBytes(download.size)}</span>
                  </div>
                  <div class="meta-item">
                    <span class="material-icons">speed</span>
                    <span style="color: {color};"
                      >{isActive ? formatSpeed(download.speed || 0) : "-"}</span
                    >
                  </div>
                  <div class="meta-item">
                    <span>{Math.round(download.progress)}%</span>
                  </div>
                </div>
                {#if download.error_message}
                  <div class="card-error-mobile">
                    ⚠ {download.error_message}
                  </div>
                {/if}
                <!-- Tap-to-expand action bar -->
                <div class="mobile-card-actions">
                  {#if download.state === "DOWNLOADING" || download.state === "STARTING"}
                    <button
                      class="mobile-action-btn action-pause"
                      onclick={(e) => {
                        e.stopPropagation();
                        handleAction("pause", download.id);
                      }}
                    >
                      <span class="material-icons">pause</span>
                      Pause
                    </button>
                  {:else if download.state === "PAUSED"}
                    <button
                      class="mobile-action-btn action-resume"
                      onclick={(e) => {
                        e.stopPropagation();
                        handleAction("resume", download.id);
                      }}
                    >
                      <span class="material-icons">play_arrow</span>
                      Resume
                    </button>
                  {:else if download.state === "FAILED"}
                    <button
                      class="mobile-action-btn action-retry"
                      onclick={(e) => {
                        e.stopPropagation();
                        handleAction("retry", download.id);
                      }}
                    >
                      <span class="material-icons">refresh</span>
                      Retry
                    </button>
                  {:else if download.state === "QUEUED" || download.state === "WAITING"}
                    <button
                      class="mobile-action-btn action-start"
                      onclick={(e) => {
                        e.stopPropagation();
                        handleAction("resume", download.id);
                      }}
                    >
                      <span class="material-icons">play_arrow</span>
                      Start
                    </button>
                  {/if}
                  <button
                    class="mobile-action-btn action-details"
                    onclick={(e) => {
                      e.stopPropagation();
                      showTaskDetails(download);
                    }}
                  >
                    <span class="material-icons">info_outline</span>
                    Details
                  </button>
                  <button
                    class="mobile-action-btn action-delete"
                    onclick={(e) => {
                      e.stopPropagation();
                      deleteConfirmTask = download;
                      expandedMobileCardId = null;
                    }}
                  >
                    <span class="material-icons">delete_outline</span>
                    Delete
                  </button>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/each}

    <!-- Mobile Individual Downloads -->
    {#each filteredDownloads() as download (download.id)}
      {@const color = getStateColorV2(download.state)}
      {@const isActive = isDownloadActive(download.state)}
      {@const isExpanded = expandedMobileCardId === download.id}
      <div
        class="download-card-mobile"
        class:expanded={isExpanded}
        onclick={() => toggleMobileCard(download.id)}
      >
        <div class="card-header-mobile">
          <div class="card-name">
            {download.filename}
            {#if download.quality}
              <span class="quality-badge" title={download.quality}
                >{download.resolution || download.quality}</span
              >
            {/if}
          </div>
          <span
            class="status-badge-v2"
            style="background: {color}15; color: {color}; border-color: {color}30;"
          >
            {download.state}
          </span>
        </div>
        <div class="card-progress-mobile">
          <div
            class="progress-fill-v2"
            style="width: {download.progress}%; background: {color};"
          ></div>
        </div>
        <div class="card-meta-mobile">
          <div class="meta-item">
            <span class="material-icons">storage</span>
            <span>{formatBytes(download.size)}</span>
          </div>
          <div class="meta-item">
            <span class="material-icons">speed</span>
            <span style="color: {color};"
              >{isActive ? formatSpeed(download.speed || 0) : "-"}</span
            >
          </div>
          <div class="meta-item">
            <span>{Math.round(download.progress)}%</span>
          </div>
        </div>
        {#if download.error_message}
          <div class="card-error-mobile">
            ⚠ {download.error_message}
          </div>
        {/if}
        <!-- Tap-to-expand action bar -->
        <div class="mobile-card-actions">
          {#if download.state === "DOWNLOADING" || download.state === "STARTING"}
            <button
              class="mobile-action-btn action-pause"
              onclick={(e) => {
                e.stopPropagation();
                handleAction("pause", download.id);
              }}
            >
              <span class="material-icons">pause</span>
              Pause
            </button>
          {:else if download.state === "PAUSED"}
            <button
              class="mobile-action-btn action-resume"
              onclick={(e) => {
                e.stopPropagation();
                handleAction("resume", download.id);
              }}
            >
              <span class="material-icons">play_arrow</span>
              Resume
            </button>
          {:else if download.state === "FAILED"}
            <button
              class="mobile-action-btn action-retry"
              onclick={(e) => {
                e.stopPropagation();
                handleAction("retry", download.id);
              }}
            >
              <span class="material-icons">refresh</span>
              Retry
            </button>
          {:else if download.state === "QUEUED" || download.state === "WAITING"}
            <button
              class="mobile-action-btn action-start"
              onclick={(e) => {
                e.stopPropagation();
                handleAction("resume", download.id);
              }}
            >
              <span class="material-icons">play_arrow</span>
              Start
            </button>
          {/if}
          <button
            class="mobile-action-btn action-details"
            onclick={(e) => {
              e.stopPropagation();
              showTaskDetails(download);
            }}
          >
            <span class="material-icons">info_outline</span>
            Details
          </button>
          <button
            class="mobile-action-btn action-delete"
            onclick={(e) => {
              e.stopPropagation();
              deleteConfirmTask = download;
              expandedMobileCardId = null;
            }}
          >
            <span class="material-icons">delete_outline</span>
            Delete
          </button>
        </div>
      </div>
    {/each}

    <!-- Empty state for mobile -->
    {#if filteredBatches().length === 0 && filteredDownloads().length === 0}
      <div
        class="empty-state-inline"
        style="padding: 2rem; text-align: center;"
      >
        <span class="material-icons" style="font-size: 36px; opacity: 0.3;"
          >inbox</span
        >
        <p
          style="margin-top: 0.5rem; color: var(--text-muted); font-size: 0.8rem;"
        >
          {#if statusFilter !== "ALL"}
            No {statusFilter.toLowerCase()} downloads
          {:else if searchQuery}
            No results for "{searchQuery}"
          {:else}
            Queue is empty
          {/if}
        </p>
      </div>
    {/if}
  </div>
</div>

<!-- V2 Style Context Menu -->
{#if contextMenuId}
  {@const allDownloads = [
    ...$downloads,
    ...Array.from($downloadStore.batchItems.values()).flat(),
  ]}
  {#each allDownloads.filter((d) => d.id === contextMenuId) as download}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
      class="context-menu"
      style="left: {contextMenuPos.x}px; top: {contextMenuPos.y}px;"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => {
        if (e.key === "Escape") hideContextMenu();
      }}
      role="menu"
      tabindex="-1"
    >
      <!-- V2: Header with filename -->
      <div class="context-header">
        {download.filename.length > 40
          ? download.filename.substring(0, 40) + "..."
          : download.filename}
      </div>

      <!-- State-based actions -->
      {#if download.state === "DOWNLOADING" || download.state === "STARTING"}
        <button
          class="context-item"
          onclick={() => handleAction("pause", contextMenuId!)}
        >
          <span class="material-icons">pause</span>
          Pause Download
        </button>
      {:else if download.state === "PAUSED"}
        <button
          class="context-item"
          onclick={() => handleAction("resume", contextMenuId!)}
        >
          <span class="material-icons">play_arrow</span>
          Resume Download
        </button>
      {:else if download.state === "FAILED"}
        <button
          class="context-item"
          onclick={() => handleAction("retry", contextMenuId!)}
        >
          <span class="material-icons">refresh</span>
          Retry Download
        </button>
      {:else if download.state === "QUEUED" || download.state === "WAITING"}
        <button
          class="context-item"
          onclick={() => handleAction("resume", contextMenuId!)}
        >
          <span class="material-icons">play_arrow</span>
          Start Now
        </button>
      {/if}

      <!-- V2: View Details -->
      <button class="context-item" onclick={() => showTaskDetails(download)}>
        <span class="material-icons">info_outline</span>
        View Details
      </button>

      <div class="context-divider"></div>

      <button
        class="context-item"
        onclick={() => {
          navigator.clipboard.writeText(download.url);
          toasts.success("Premium link copied to clipboard!");
          hideContextMenu();
        }}
      >
        <span class="material-icons">link</span>
        Copy Premium Link
      </button>

      <button
        class="context-item"
        onclick={() => {
          navigator.clipboard.writeText(download.original_url);
          toasts.success("Fshare link copied to clipboard!");
          hideContextMenu();
        }}
      >
        <span class="material-icons">content_copy</span>
        Copy Fshare Link
      </button>

      {#if download.error_message}
        <button
          class="context-item"
          onclick={() => copyError(download.error_message!)}
        >
          <span class="material-icons">bug_report</span>
          Copy Error Message
        </button>
      {/if}

      <div class="context-divider"></div>

      <!-- V2: Delete with confirmation -->
      <button
        class="context-item danger"
        onclick={() => showDeleteConfirm(download)}
      >
        <span class="material-icons">delete</span>
        Delete Task
      </button>
    </div>
  {/each}
{/if}

<!-- Batch Context Menu -->
{#if batchContextMenuId}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="context-menu"
    style="left: {batchContextMenuPos.x}px; top: {batchContextMenuPos.y}px;"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => {
      if (e.key === "Escape") hideContextMenu();
    }}
    role="menu"
    tabindex="-1"
  >
    <div class="context-header">Batch Actions</div>

    <button
      class="context-item"
      onclick={() => handleBatchAction("resume", batchContextMenuId!)}
    >
      <span class="material-icons">play_arrow</span>
      Resume All
    </button>

    <button
      class="context-item"
      onclick={() => handleBatchAction("pause", batchContextMenuId!)}
    >
      <span class="material-icons">pause</span>
      Pause All
    </button>

    <button
      class="context-item"
      onclick={() => {
        const batch = filteredBatches().find(
          (b) => b.batchId === batchContextMenuId,
        );
        if (batch) showBatchDetails(batch);
      }}
    >
      <span class="material-icons">info_outline</span>
      View Details
    </button>

    <div class="context-divider"></div>

    <button
      class="context-item danger"
      onclick={() => handleBatchAction("delete", batchContextMenuId!)}
    >
      <span class="material-icons">delete</span>
      Delete Batch
    </button>
  </div>
{/if}

<!-- Delete Download Confirmation Modal -->
<Modal
  open={!!deleteConfirmTask}
  onClose={hideDeleteConfirm}
  maxWidth="420px"
  accent="#ff4444"
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
    <button class="close-btn" onclick={hideDeleteConfirm} aria-label="Close">
      <span class="material-icons">close</span>
    </button>
  {/snippet}

  {#snippet children()}
    <p class="del-confirm-text">
      Are you sure you want to delete this download?
    </p>
    <div class="del-filename-pill">
      <span class="material-icons">insert_drive_file</span>
      <span class="del-filename-text">
        {deleteConfirmTask
          ? deleteConfirmTask.filename.length > 48
            ? deleteConfirmTask.filename.substring(0, 48) + "…"
            : deleteConfirmTask.filename
          : ""}
      </span>
    </div>
    <div class="del-warning-row">
      <span class="material-icons del-warn-icon">folder_delete</span>
      <p class="del-warn-text">
        <strong>Warning:</strong> The downloaded file will also be permanently deleted
        from disk.
      </p>
    </div>
  {/snippet}

  {#snippet footer()}
    <div class="del-modal-actions">
      <Button variant="ghost" size="md" onclick={hideDeleteConfirm}
        >Cancel</Button
      >
      <Button
        variant="danger"
        size="md"
        icon="delete_forever"
        onclick={confirmDelete}>Delete Permanently</Button
      >
    </div>
  {/snippet}
</Modal>

<!-- Batch Delete Confirmation Modal -->
<Modal
  open={!!deleteConfirmBatch}
  onClose={hideBatchDeleteConfirm}
  maxWidth="420px"
  accent="#ff4444"
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
    <button
      class="close-btn"
      onclick={hideBatchDeleteConfirm}
      aria-label="Close"
    >
      <span class="material-icons">close</span>
    </button>
  {/snippet}

  {#snippet children()}
    <p class="del-confirm-text">
      Are you sure you want to delete this entire batch?
    </p>
    <div class="del-filename-pill">
      <span class="material-icons">folder_zip</span>
      <span class="del-filename-text">
        {deleteConfirmBatch
          ? deleteConfirmBatch.batchName.length > 48
            ? deleteConfirmBatch.batchName.substring(0, 48) + "…"
            : deleteConfirmBatch.batchName
          : ""}
      </span>
    </div>
    <div class="del-warning-row">
      <span class="material-icons del-warn-icon">folder_delete</span>
      <p class="del-warn-text">
        <strong>Warning:</strong> Deleting this batch will permanently remove
        {#if deleteConfirmBatch}
          <strong
            >{deleteConfirmBatch.itemCount}
            {deleteConfirmBatch.itemCount === 1 ? "item" : "items"}</strong
          >
        {/if}
        and all associated files from disk.
      </p>
    </div>
  {/snippet}

  {#snippet footer()}
    <div class="del-modal-actions">
      <Button variant="ghost" size="md" onclick={hideBatchDeleteConfirm}
        >Cancel</Button
      >
      <Button
        variant="danger"
        size="md"
        icon="delete_sweep"
        onclick={confirmBatchDelete}>Delete Batch</Button
      >
    </div>
  {/snippet}
</Modal>

<!-- V2 Style Task Details Modal -->
<Modal
  open={!!detailsTask}
  onClose={hideTaskDetails}
  maxWidth="600px"
  accent="var(--color-primary, #00f3ff)"
  ariaLabel="Download Details"
>
  {#snippet header()}
    <div class="detail-header-main">
      {#if detailsTask}
        <!-- Left: big fingerprint icon, UUID overlaid at bottom -->
        <button
          class="id-badge-pill"
          onclick={() => {
            navigator.clipboard.writeText(detailsTask!.id);
            toasts.success("ID copied");
          }}
          title="Click to copy full ID"
        >
          <span class="material-icons id-big-icon">fingerprint</span>
          <span class="id-overlay-text">{detailsTask.id.substring(0, 8)}</span>
        </button>
        <!-- Right: filename row + quality badge stacked -->
        <div class="detail-name-block">
          <h3 class="filename-title" title={detailsTask.filename}>
            {detailsTask.filename}
          </h3>
          {#if detailsTask.quality}
            <span
              class="quality-badge quality-badge-lg"
              title={detailsTask.quality}
            >
              {detailsTask.quality}
            </span>
          {/if}
        </div>
      {/if}
    </div>
    <button class="close-btn" onclick={hideTaskDetails}>
      <span class="material-icons">close</span>
    </button>
  {/snippet}

  {#snippet children()}
    {#if detailsTask}
      <!-- Main Progress & State -->
      <div class="details-hero-section">
        <div class="hero-stats">
          <div class="stat-block">
            <span class="label">STATUS</span>
            <span
              class="value state-text"
              style="color: {getStateColorV2(detailsTask.state)};"
            >
              {detailsTask.state}
            </span>
          </div>
          <div class="stat-block">
            <span class="label">PROGRESS</span>
            <span class="value progress-text"
              >{Math.round(detailsTask.progress)}%</span
            >
          </div>
          <div class="stat-block">
            <span class="label">SPEED</span>
            <span class="value speed-text"
              >{isDownloadActive(detailsTask.state)
                ? formatSpeed(detailsTask.speed || 0)
                : "0 B/s"}</span
            >
          </div>
        </div>

        <div class="hero-progress-container">
          <div class="progress-track">
            <div
              class="progress-bar-glow"
              style="width: {detailsTask.progress}%; background: {getStateColorV2(
                detailsTask.state,
              )}; box-shadow: 0 0 15px {getStateColorV2(detailsTask.state)}80;"
            ></div>
          </div>
          <div class="progress-meta">
            <span>{formatBytes(detailsTask.downloaded || 0)}</span>
            <span>OF</span>
            <span>{formatBytes(detailsTask.size)}</span>
          </div>
        </div>
      </div>

      <div class="details-grid-premium">
        <!-- Technical Details Section -->
        <div class="details-section">
          <div class="section-label">TECHNICAL LOGS</div>

          <div class="data-row-premium">
            <div class="data-item">
              <span class="material-icons">calendar_today</span>
              <div class="item-content">
                <span class="l">Created At</span>
                <span class="v">{formatAddedDate(detailsTask.created_at)}</span>
              </div>
            </div>
            {#if detailsTask.started_at}
              <div class="data-item">
                <span class="material-icons">play_circle</span>
                <div class="item-content">
                  <span class="l">Started At</span>
                  <span class="v"
                    >{formatAddedDate(detailsTask.started_at)}</span
                  >
                </div>
              </div>
            {/if}
          </div>

          <div class="data-row-premium">
            <div class="data-item">
              <span class="material-icons">timer</span>
              <div class="item-content">
                <span class="l"
                  >{detailsTask.state === "COMPLETED"
                    ? "Completed At"
                    : "Estimated Time"}</span
                >
                <span class="v">
                  {detailsTask.state === "COMPLETED"
                    ? detailsTask.completed_at
                      ? formatAddedDate(detailsTask.completed_at)
                      : "--"
                    : isDownloadActive(detailsTask.state)
                      ? formatETA(detailsTask.eta)
                      : "--"}
                </span>
              </div>
            </div>
            <div class="data-item">
              <span class="material-icons">category</span>
              <div class="item-content">
                <span class="l">Data Category</span>
                <span class="v" style="text-transform: uppercase;">
                  {detailsTask.category || "General"}
                </span>
              </div>
            </div>
          </div>
        </div>

        <!-- Links Section -->
        <div class="details-section">
          <div class="section-label">ACCESS NODES</div>

          <div class="link-item-premium">
            <div class="link-info">
              <span class="material-icons link-icon">link</span>
              <div class="link-text">
                <span class="l">Premium Decoded URL</span>
                <span class="v">{detailsTask.url || "Resolving..."}</span>
              </div>
            </div>
            <button
              class="copy-tiny"
              onclick={() => {
                navigator.clipboard.writeText(detailsTask!.url);
                toasts.success("Premium link copied");
              }}
            >
              <span class="material-icons">content_copy</span>
            </button>
          </div>

          <div class="link-item-premium">
            <div class="link-info">
              <span class="material-icons link-icon">cloud</span>
              <div class="link-text">
                <span class="l">Fshare Source Link</span>
                <span class="v">{detailsTask.original_url}</span>
              </div>
            </div>
            <button
              class="copy-tiny"
              onclick={() => {
                navigator.clipboard.writeText(detailsTask!.original_url);
                toasts.success("Fshare link copied");
              }}
            >
              <span class="material-icons">content_copy</span>
            </button>
          </div>
        </div>

        <!-- Error Handling -->
        {#if detailsTask.error_message}
          <div class="details-section error">
            <div class="section-label danger">INCIDENT REPORT</div>
            <div class="error-box-premium">
              <span class="material-icons">error_outline</span>
              <p>{detailsTask.error_message}</p>
            </div>
          </div>
        {/if}
      </div>

      <div class="details-footer-premium">
        <Button variant="ghost" size="md" onclick={hideTaskDetails}
          >Close Terminal</Button
        >
      </div>
    {/if}
  {/snippet}
</Modal>

<!-- V6: Batch Detail Modal -->
{#if detailsBatch}
  {@const batchStatus = getBatchStatus(detailsBatch)}
  {@const estimatedTime =
    detailsBatch.speed > 0
      ? Math.ceil(
          (detailsBatch.totalSize - detailsBatch.downloaded) /
            detailsBatch.speed,
        )
      : 0}
{/if}
<Modal
  open={!!detailsBatch}
  onClose={hideBatchDetails}
  maxWidth="680px"
  accent="#00ffa3"
  ariaLabel="Batch Download Details"
>
  {#snippet header()}
    <div class="detail-header-main">
      {#if detailsBatch}
        <!-- Left: big icon, UUID overlaid at bottom -->
        <button
          class="id-badge-pill"
          onclick={() => {
            navigator.clipboard.writeText(detailsBatch!.batchId);
            toasts.success("Batch ID copied");
          }}
          title="Click to copy full batch ID"
        >
          <span class="material-icons id-big-icon">folder_special</span>
          <span class="id-overlay-text"
            >{detailsBatch.batchId.substring(0, 8)}</span
          >
        </button>
        <!-- Right: batch name stacked -->
        <div class="detail-name-block">
          <h3 class="filename-title" title={detailsBatch.batchName}>
            {detailsBatch.batchName}
          </h3>
        </div>
      {/if}
    </div>
    <button class="close-btn" onclick={hideBatchDetails}>
      <span class="material-icons">close</span>
    </button>
  {/snippet}

  {#snippet children()}
    {#if detailsBatch}
      {@const batchStatus = getBatchStatus(detailsBatch)}
      {@const estimatedTime =
        detailsBatch.speed > 0
          ? Math.ceil(
              (detailsBatch.totalSize - detailsBatch.downloaded) /
                detailsBatch.speed,
            )
          : 0}

      <!-- Hero Section: Stats & Progress -->
      <div class="details-hero-section">
        <div class="hero-stats">
          <div class="stat-block">
            <span class="label">STATUS</span>
            <span class="value state-text" style="color: {batchStatus.color};">
              {detailsBatch.completedCount === detailsBatch.totalItems
                ? "All Done"
                : detailsBatch.activeCount > 0
                  ? `Mixed (${detailsBatch.completedCount}/${detailsBatch.totalItems})`
                  : `Paused (${detailsBatch.completedCount}/${detailsBatch.totalItems})`}
            </span>
          </div>
          <div class="stat-block">
            <span class="label">PROGRESS</span>
            <span class="value progress-text"
              >{detailsBatch.progress.toFixed(1)}%</span
            >
          </div>
          <div class="stat-block">
            <span class="label">SPEED</span>
            <span class="value speed-text"
              >{detailsBatch.speed > 0
                ? formatSpeed(detailsBatch.speed)
                : "0 B/s"}</span
            >
          </div>
        </div>

        <div class="hero-progress-container">
          <div class="progress-track">
            <div
              class="progress-bar-glow"
              style="width: {detailsBatch.progress}%; background: {batchStatus.color}; box-shadow: 0 0 15px {batchStatus.color}80;"
            ></div>
          </div>
          <div class="progress-meta">
            <span>{formatBytes(detailsBatch.downloaded)}</span>
            <span>OF</span>
            <span>{formatBytes(detailsBatch.totalSize)}</span>
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
              <span class="stat-label-compact">Total Files:</span>
              <span class="stat-value-compact">{detailsBatch.totalItems}</span>
            </div>
            <div class="stat-row">
              <span class="stat-label-compact"
                ><span class="material-icons stat-icon">pause_circle</span> Paused:</span
              >
              <span class="stat-value-compact" style="color: #64748b;"
                >{detailsBatch.pausedCount}</span
              >
            </div>
            <div class="stat-row">
              <span class="stat-label-compact"
                ><span class="material-icons stat-icon" style="color:#00ffa3"
                  >check_circle</span
                > Completed:</span
              >
              <span class="stat-value-compact" style="color: #00ffa3;"
                >{detailsBatch.completedCount}</span
              >
            </div>
            <div class="stat-row">
              <span class="stat-label-compact"
                ><span class="material-icons stat-icon" style="color:#ffd700"
                  >schedule</span
                > Queued:</span
              >
              <span class="stat-value-compact" style="color: #ffd700;"
                >{detailsBatch.queuedCount}</span
              >
            </div>
            <div class="stat-row">
              <span class="stat-label-compact"
                ><span class="material-icons stat-icon" style="color:#00f3ff"
                  >download</span
                > Downloading:</span
              >
              <span class="stat-value-compact" style="color: #00f3ff;"
                >{detailsBatch.activeCount}</span
              >
            </div>
            <div class="stat-row">
              <span class="stat-label-compact"
                ><span class="material-icons stat-icon" style="color:#ff5252"
                  >error</span
                > Failed:</span
              >
              <span class="stat-value-compact" style="color: #ff5252;"
                >{detailsBatch.failedCount}</span
              >
            </div>
          </div>
        </div>

        <!-- Timing Info -->
        <div class="details-section">
          <div class="section-label">TIMING INFO</div>

          <div class="batch-stats-compact">
            <div class="stat-row">
              <span class="stat-label-compact"
                ><span class="material-icons stat-icon">event</span> Created:</span
              >
              <span class="stat-value-compact"
                >{formatAddedDate(detailsBatch.createdAt)}</span
              >
            </div>
            <div class="stat-row">
              <span class="stat-label-compact"
                ><span class="material-icons stat-icon">timer</span> Est. Complete:</span
              >
              <span class="stat-value-compact">
                {estimatedTime > 0 ? formatETA(estimatedTime) : "~15 minutes"}
              </span>
            </div>
            <div class="stat-row" style="grid-column: 1 / -1;">
              <span class="stat-label-compact"
                ><span class="material-icons stat-icon">folder</span> Category:</span
              >
              <span class="stat-value-compact">TV Series</span>
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
            onclick={() => handleBatchAction("resume", detailsBatch!.batchId)}
            disabled={detailsBatch.pausedCount === 0 &&
              detailsBatch.queuedCount === 0}>Resume All</Button
          >
          <Button
            size="md"
            icon="pause"
            onclick={() => handleBatchAction("pause", detailsBatch!.batchId)}
            disabled={detailsBatch.activeCount === 0}>Pause All</Button
          >
          <Button
            variant="danger"
            size="md"
            icon="refresh"
            onclick={() => handleBatchAction("retry", detailsBatch!.batchId)}
            disabled={detailsBatch.failedCount === 0}>Retry Failed</Button
          >
        </div>
        <Button variant="ghost" size="md" onclick={hideBatchDetails}
          >Close</Button
        >
      </div>
    {/if}
  {/snippet}
</Modal>

<style>
  .downloads-container {
    padding: 1.5rem;
    height: 100%;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
  }

  .section-box {
    background: linear-gradient(
      160deg,
      rgba(0, 243, 255, 0.015) 0%,
      var(--bg-glass) 40%
    );
    border: 1px solid var(--border-glass);
    border-top: 1px solid rgba(0, 243, 255, 0.4);
    border-radius: 12px;
    padding: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    flex: 1;
    position: relative;
  }

  /* Novu halftone dissolution top-glow */
  .section-box::after {
    content: "";
    position: absolute;
    top: -60px;
    left: -20px;
    right: -20px;
    height: 300px;
    background-image: radial-gradient(circle, #00f3ff 1.2px, transparent 1.2px);
    background-size: 8px 8px;
    opacity: 0.2;
    pointer-events: none;
    z-index: 0;
    mask-image: radial-gradient(
      ellipse at 50% 0%,
      black 0%,
      rgba(0, 0, 0, 0.5) 8%,
      rgba(0, 0, 0, 0.12) 25%,
      rgba(0, 0, 0, 0.03) 45%,
      transparent 60%
    );
    -webkit-mask-image: radial-gradient(
      ellipse at 50% 0%,
      black 0%,
      rgba(0, 0, 0, 0.5) 8%,
      rgba(0, 0, 0, 0.12) 25%,
      rgba(0, 0, 0, 0.03) 45%,
      transparent 60%
    );
  }

  /* V2 Toolbar */
  .toolbar {
    padding: 1rem 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .toolbar-actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  .btn-tiny {
    width: 32px;
    height: 32px;
    border-radius: 6px;
    border: 1px solid var(--border-glass);
    background: var(--bg-glass);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s;
  }

  .btn-tiny .material-icons {
    font-size: 18px;
    color: var(--text-muted);
  }

  .btn-tiny:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .btn-tiny.btn-success {
    border-color: #00ff80;
  }
  .btn-tiny.btn-success .material-icons {
    color: #00ff80;
  }

  .btn-tiny.btn-danger {
    border-color: #ff5252;
  }
  .btn-tiny.btn-danger .material-icons {
    color: #ff5252;
  }

  .divider {
    width: 1px;
    height: 16px;
    background: rgba(255, 255, 255, 0.1);
    margin: 0 0.25rem;
  }

  /* Prominent Search Bar (matching Search/Discover tabs) */
  .search-bar-prominent {
    flex: 1;
    max-width: 500px;
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    padding: 0.75rem 1.25rem;
    transition: all 0.3s;
  }

  .search-bar-prominent:focus-within {
    border-color: var(--color-primary);
    box-shadow: 0 0 15px rgba(0, 243, 255, 0.2);
    background: rgba(0, 0, 0, 0.4);
  }

  .search-bar-prominent .material-icons {
    font-size: 20px;
    color: var(--text-muted);
    transition: color 0.3s;
  }

  .search-bar-prominent:focus-within .material-icons {
    color: var(--color-primary);
  }

  .search-bar-prominent input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: 0.875rem;
    font-weight: 500;
  }

  .search-bar-prominent input::placeholder {
    color: var(--text-muted);
    opacity: 0.6;
  }

  /* Downloads Content */
  .downloads-content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  /* Table Wrapper */
  .table-wrapper {
    flex: 1;
    overflow-y: auto;
  }

  /* V2 Style Data Table */
  .data-table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }

  .data-table thead {
    position: sticky;
    top: 0;
    background: rgba(15, 23, 42, 0.95);
    z-index: 10;
    backdrop-filter: blur(10px);
  }

  .data-table th {
    padding: 0.75rem 1rem;
    text-align: left;
    font-size: 0.7rem;
    font-weight: 800;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    border-bottom: 2px solid rgba(255, 255, 255, 0.08);
    cursor: pointer;
    user-select: none;
    transition: all 0.2s;
  }

  .data-table th:hover {
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.02);
  }

  .th-content {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .th-label {
    font-weight: 900;
    letter-spacing: 0.1em;
  }

  .sort-icon {
    font-size: 14px !important;
    opacity: 0.4;
    transition: all 0.2s;
  }

  .sort-icon.active {
    opacity: 1;
    color: var(--color-primary);
  }

  /* V4: Inline status filter dropdown in column header */
  .status-filter-th {
    padding: 0 !important;
  }

  .status-filter-header {
    position: relative;
    padding: 0.5rem 0.5rem;
  }

  .status-dropdown-wrapper {
    position: relative;
    display: inline-block;
  }

  /* Custom Status Dropdown - Glassmorphism style */
  .status-dropdown-trigger {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.3rem 0.5rem;
    background: rgba(15, 23, 42, 0.8);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s;
    width: 160px;
  }

  .status-dropdown-trigger:hover {
    border-color: rgba(0, 243, 255, 0.3);
    background: rgba(15, 23, 42, 0.95);
  }

  .status-dropdown-trigger.open {
    border-color: rgba(255, 255, 255, 0.1);
    border-bottom-color: transparent;
    border-radius: 6px 6px 0 0;
    background: rgba(10, 15, 30, 0.95);
  }

  .status-dropdown-trigger .status-badge-v2 {
    font-size: 0.5rem;
    padding: 2px 5px;
  }

  .dropdown-count {
    font-size: 0.6rem;
    color: var(--text-muted);
    font-weight: 500;
  }

  .dropdown-arrow {
    font-size: 14px !important;
    color: var(--text-muted);
    transition: transform 0.2s;
    margin-left: auto;
  }

  .dropdown-arrow.open {
    transform: rotate(180deg);
  }

  .dropdown-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 999;
  }

  .status-dropdown-menu {
    position: absolute;
    top: 100%;
    left: 0;
    background: rgba(10, 15, 30, 0.95);
    backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-top: none;
    border-radius: 0 0 10px 10px;
    padding: 0.4rem;
    width: 160px;
    box-sizing: border-box;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
    z-index: 1000;
  }

  .status-option {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.6rem;
    border: none;
    background: transparent;
    cursor: pointer;
    border-radius: 6px;
    transition: all 0.15s;
    text-align: left;
  }

  .status-option:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .status-option.selected {
    background: rgba(0, 243, 255, 0.08);
  }

  .status-option .status-badge-v2 {
    font-size: 0.5rem;
    padding: 2px 5px;
  }

  .status-option .count {
    font-size: 0.6rem;
    color: var(--text-muted);
    font-weight: 500;
  }

  .status-option .check-icon {
    font-size: 14px !important;
    color: var(--color-primary);
    margin-left: auto;
  }

  .data-table td {
    padding: 0.55rem 1rem;
    font-size: 0.75rem;
    color: var(--text-secondary);
    border-bottom: 1px solid rgba(255, 255, 255, 0.02);
    vertical-align: middle;
  }

  .data-table tr.transfer-row {
    transition: background 0.2s;
    cursor: pointer;
  }

  .data-table tr.transfer-row:hover {
    background: rgba(255, 255, 255, 0.02);
  }

  /* V3: Error row prominence */
  .data-table tr.error-row {
    background: rgba(255, 82, 82, 0.05);
    border-left: 3px solid #ff5252;
    box-shadow: inset 0 0 20px rgba(255, 82, 82, 0.08);
  }

  .data-table tr.error-row:hover {
    background: rgba(255, 82, 82, 0.1);
  }

  /* V3: Selected row highlighting */
  .data-table tr.selected-row {
    background: rgba(0, 243, 255, 0.08);
    border-left: 3px solid var(--color-primary);
    box-shadow: inset 0 0 15px rgba(0, 243, 255, 0.1);
  }

  .data-table tr.selected-row.error-row {
    border-left: 3px solid #ff5252;
  }

  /* V3: Completion animation */
  .data-table tr.just-completed {
    animation: completion-celebrate 1.5s ease-out;
  }

  @keyframes completion-celebrate {
    0% {
      background: rgba(0, 255, 163, 0);
    }
    15% {
      background: rgba(0, 255, 163, 0.25);
      box-shadow: 0 0 30px rgba(0, 255, 163, 0.4);
    }
    40% {
      background: rgba(0, 255, 163, 0.1);
    }
    100% {
      background: rgba(0, 255, 163, 0.03);
    }
  }

  /* V3: Shimmer sweep overlay for completion */
  .data-table tr.just-completed::after {
    content: "";
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(
      90deg,
      transparent 0%,
      rgba(255, 255, 255, 0.15) 50%,
      transparent 100%
    );
    animation: shimmer-sweep 0.8s ease-out 0.2s;
    pointer-events: none;
  }

  @keyframes shimmer-sweep {
    0% {
      left: -100%;
    }
    100% {
      left: 100%;
    }
  }

  /* V5: Batch hierarchy styles */
  .batch-header-row {
    background: rgba(0, 243, 255, 0.04);
    cursor: pointer;
    transition: background 0.2s;
    border-left: 3px solid transparent;
  }

  .batch-header-row:hover {
    background: rgba(0, 243, 255, 0.08);
  }

  .batch-header-row.batch-expanded {
    background: rgba(0, 243, 255, 0.06);
    border-left: 3px solid var(--color-primary);
  }

  .batch-name-cell {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .expand-icon {
    font-size: 1.2rem;
    color: var(--text-muted);
    transition:
      transform 0.2s,
      color 0.2s;
  }

  .batch-expanded .expand-icon {
    color: var(--color-primary);
  }

  .batch-name {
    font-weight: 500;
    color: var(--text-primary);
  }

  .batch-count {
    color: rgba(255, 255, 255, 0.4);
    font-size: 0.75rem;
    font-weight: 400;
  }

  .batch-child-row {
    background: rgba(0, 0, 0, 0.15);
  }

  .batch-child-row:hover {
    background: rgba(255, 255, 255, 0.03) !important;
  }

  .batch-child-filename {
    padding-left: 2.5rem !important;
  }

  .batch-child-row .filename-cell::before {
    content: "";
    display: inline-block;
    width: 1rem;
    height: 1px;
    background: rgba(255, 255, 255, 0.15);
    margin-right: 0.5rem;
    vertical-align: middle;
  }

  /* V3: Quick actions */

  .filename-cell {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-primary);
  }

  /* V2 Status Badge */
  .status-badge-v2 {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 7px;
    font-size: 0.55rem;
    font-weight: 800;
    border: 1px solid;
    text-transform: uppercase;
    font-family: var(--font-mono, monospace);
    letter-spacing: 0.07em;
    clip-path: polygon(
      3px 0%,
      calc(100% - 3px) 0%,
      100% 3px,
      100% calc(100% - 3px),
      calc(100% - 3px) 100%,
      3px 100%,
      0% calc(100% - 3px),
      0% 3px
    );
  }
  .status-badge-v2::before {
    content: "";
    display: inline-block;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: currentColor;
    opacity: 0.8;
    flex-shrink: 0;
  }

  /* Quality Badge */
  .quality-badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 6px;
    font-size: 0.52rem;
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    font-family: var(--font-mono, monospace);
    background: rgba(168, 85, 247, 0.12);
    color: #c084fc;
    border: 1px solid rgba(168, 85, 247, 0.25);
    margin-left: 6px;
    white-space: nowrap;
    flex-shrink: 0;
    cursor: default;
    clip-path: polygon(
      3px 0%,
      calc(100% - 3px) 0%,
      100% 3px,
      100% calc(100% - 3px),
      calc(100% - 3px) 100%,
      3px 100%,
      0% calc(100% - 3px),
      0% 3px
    );
  }
  .quality-badge::before {
    content: "";
    display: inline-block;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: #c084fc;
    opacity: 0.8;
    flex-shrink: 0;
  }

  .quality-badge-lg {
    font-size: 0.65rem;
    padding: 2px 8px;
    margin-left: 0;
    margin-top: 4px;
  }

  /* Status Badge Color Variants */
  .status-badge-v2.all {
    background: rgba(255, 255, 255, 0.1);
    color: #ffffff;
    border-color: rgba(255, 255, 255, 0.2);
  }

  .status-badge-v2.downloading {
    background: rgba(0, 243, 255, 0.15);
    color: #00f3ff;
    border-color: rgba(0, 243, 255, 0.3);
  }

  .status-badge-v2.queued {
    background: rgba(33, 150, 243, 0.15);
    color: #2196f3;
    border-color: rgba(33, 150, 243, 0.3);
  }

  .status-badge-v2.paused {
    background: rgba(255, 152, 0, 0.15);
    color: #ff9800;
    border-color: rgba(255, 152, 0, 0.3);
  }

  .status-badge-v2.completed {
    background: rgba(76, 175, 80, 0.15);
    color: #4caf50;
    border-color: rgba(76, 175, 80, 0.3);
  }

  .status-badge-v2.failed {
    background: rgba(244, 67, 54, 0.15);
    color: #f44336;
    border-color: rgba(244, 67, 54, 0.3);
  }

  .status-badge-v2.cancelled {
    background: rgba(158, 158, 158, 0.15);
    color: #9e9e9e;
    border-color: rgba(158, 158, 158, 0.3);
  }

  .error-preview {
    margin-top: 4px;
    font-size: 0.6rem;
    color: #ff5252;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .size-cell {
    color: var(--text-secondary);
    font-size: 0.7rem;
  }

  /* V2 Progress Cell */

  .progress-bar-v2 {
    height: 3px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-fill-v2 {
    height: 100%;
    width: var(--progress, 0%);
    background: var(--bar-color, #00f3ff);
    border-radius: 2px;
    transition: width 0.3s ease;
  }

  .speed-cell {
    font-family: var(--font-mono);
    font-size: 0.7rem;
    font-weight: 700;
  }

  .eta-cell {
    color: var(--text-secondary);
    font-size: 0.7rem;
  }

  .icon-btn-tiny {
    width: 28px;
    height: 28px;
    background: transparent;
    border: none;
    border-radius: 4px;
    color: var(--text-muted);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s;
  }

  .icon-btn-tiny:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }

  .icon-btn-tiny:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .icon-btn-tiny .material-icons {
    font-size: 14px;
  }

  /* V2 Pagination Footer */
  .pagination-footer-v2 {
    padding: 0.5rem 1.25rem;
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 1.5rem;
    border-top: 1px solid rgba(255, 255, 255, 0.03);
    background: rgba(0, 0, 0, 0.15);
  }

  .page-info-v2 {
    font-size: 0.7rem;
    color: var(--text-muted);
    letter-spacing: 0.05em;
    font-weight: 700;
  }

  .page-controls {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    background: rgba(255, 255, 255, 0.03);
    padding: 4px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .page-indicator {
    padding: 0 0.5rem;
    font-weight: 800;
    font-size: 0.7rem;
    color: var(--color-primary);
    font-family: var(--font-mono);
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Inline Loading/Empty States (inside table) */
  .table-empty-cell {
    text-align: center;
    padding: 3rem 1rem !important;
    color: var(--text-muted);
  }

  .loading-state-inline {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    color: var(--text-muted);
    font-size: 0.8rem;
  }

  .spinner-ring-small {
    width: 20px;
    height: 20px;
    border: 2px solid rgba(0, 243, 255, 0.1);
    border-top-color: var(--color-primary);
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  .empty-state-inline {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    color: var(--text-muted);
    font-size: 0.8rem;
  }

  .empty-state-inline .material-icons {
    font-size: 18px;
    opacity: 0.5;
  }

  .inline-link {
    background: transparent;
    border: none;
    color: var(--color-primary);
    cursor: pointer;
    font-size: 0.8rem;
    text-decoration: underline;
    padding: 0;
    margin-left: 0.5rem;
  }

  .inline-link:hover {
    color: var(--color-secondary);
  }

  /* Context Menu */
  .context-menu {
    position: fixed;
    background: rgba(10, 15, 30, 0.95);
    backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    padding: 0.5rem;
    min-width: 220px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
    z-index: 1000;
    animation: menu-pop 0.2s cubic-bezier(0.175, 0.885, 0.32, 1.275);
  }

  @keyframes menu-pop {
    from {
      opacity: 0;
      transform: scale(0.95);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  .context-item {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    border: none;
    background: transparent;
    color: #e6edf3;
    font-size: 0.8rem;
    font-weight: 500;
    cursor: pointer;
    border-radius: 8px;
    text-align: left;
    transition: all 0.2s;
  }

  .context-item:hover {
    background: rgba(255, 255, 255, 0.05);
    color: #fff;
  }

  .context-item .material-icons {
    font-size: 18px;
    color: var(--text-muted);
  }

  .context-item.danger:hover {
    background: rgba(255, 82, 82, 0.1);
    color: #ff5252;
  }

  .context-item.danger:hover .material-icons {
    color: #ff5252;
  }

  .context-divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.05);
    margin: 0.5rem 0;
  }

  /* V2: Context Menu Header */
  .context-header {
    padding: 0.75rem 1rem;
    font-size: 0.65rem;
    font-weight: 800;
    color: var(--text-muted);
    text-transform: uppercase;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    margin-bottom: 0.5rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* ── Delete modal content (inside universal Modal component) ── */
  .del-modal-title {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex: 1;
    min-width: 0;
  }
  .del-modal-icon {
    font-size: 1.6rem;
    color: #ff5252;
    flex-shrink: 0;
  }
  .del-modal-heading {
    font-size: 1rem;
    font-weight: 800;
    color: #ff5252;
    font-family: var(--font-mono, monospace);
    letter-spacing: 0.04em;
  }
  .del-modal-sub {
    font-size: 0.72rem;
    color: rgba(255, 255, 255, 0.35);
    font-family: var(--font-mono, monospace);
    margin-top: 2px;
  }
  .del-confirm-text {
    font-size: 0.88rem;
    color: rgba(255, 255, 255, 0.65);
    margin: 0 0 1.1rem;
    line-height: 1.5;
  }
  .del-filename-pill {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 0.6rem 0.85rem;
    margin-bottom: 1rem;
    font-size: 0.78rem;
    font-family: var(--font-mono, monospace);
    color: rgba(255, 255, 255, 0.85);
    word-break: break-all;
  }
  .del-filename-pill .material-icons {
    font-size: 1rem;
    color: rgba(255, 255, 255, 0.3);
    flex-shrink: 0;
  }
  .del-filename-text {
    flex: 1;
    min-width: 0;
  }
  .del-warning-row {
    display: flex;
    align-items: flex-start;
    gap: 0.65rem;
    background: rgba(255, 82, 82, 0.07);
    border: 1px solid rgba(255, 82, 82, 0.2);
    border-radius: 8px;
    padding: 0.7rem 0.85rem;
  }
  .del-warn-icon {
    font-size: 1.1rem;
    color: #ff5252;
    flex-shrink: 0;
    margin-top: 1px;
  }
  .del-warn-text {
    margin: 0;
    font-size: 0.78rem;
    color: rgba(255, 150, 130, 0.85);
    line-height: 1.5;
  }
  .del-warn-text strong {
    color: #ff7878;
  }
  .del-modal-actions {
    display: flex;
    flex-direction: row;
    align-items: center;
    justify-content: flex-end;
    gap: 0.65rem;
    /* Pulls the row out of the default 1.75rem footer padding so buttons
       sit exactly 0.75rem from the modal left/right edges */
    margin-inline: calc(-1.75rem + 0.75rem);
  }
  /* Buttons keep their natural content width — no forced sizing */

  /* Detail modal header — horizontal: [id pill | name block] */
  .detail-header-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: row;
    align-items: stretch;
    gap: 0.75rem;
  }

  /* ── ID badge pill: transparent, big icon + UUID overlay ── */
  .id-badge-pill {
    position: relative;
    align-self: stretch;
    flex-shrink: 0;
    background: none;
    border: none;
    border-radius: 0;
    min-width: 3rem;
    padding: 0.15rem 0.35rem 0;
    cursor: pointer;
    overflow: hidden;
    transition: opacity 0.18s;
  }
  .id-badge-pill:hover {
    opacity: 1;
  }

  /* Large icon — scaled up to fill the pill */
  .id-big-icon {
    font-size: 3rem !important;
    color: var(--color-primary);
    opacity: 0.45;
    display: block;
    line-height: 1;
    transition: opacity 0.18s;
  }
  .id-badge-pill:hover .id-big-icon {
    opacity: 0.85;
  }

  /* UUID text — overlaid at the bottom of the icon */
  .id-overlay-text {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    padding: 0.1rem 0.2rem;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    font-family: var(--font-mono);
    font-size: 0.48rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: var(--color-primary);
    text-align: center;
    line-height: 1.4;
    pointer-events: none;
  }

  /* Name block: filename + quality badge stacked vertically */
  .detail-name-block {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.3rem;
  }

  .filename-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .filename-cell {
    display: flex;
    align-items: center;
  }

  .filename-title {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 800;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    letter-spacing: -0.01em;
  }

  .details-hero-section {
    padding: 2rem;
    background: radial-gradient(
      circle at top right,
      rgba(0, 243, 255, 0.03),
      transparent 70%
    );
  }

  .hero-stats {
    display: flex;
    justify-content: space-between;
    margin-bottom: 1.5rem;
  }

  .stat-block {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .stat-block .label {
    font-size: 0.6rem;
    font-weight: 900;
    color: var(--text-muted);
    letter-spacing: 0.15em;
  }

  .stat-block .value {
    font-family: var(--font-mono);
    font-size: 1.1rem;
    font-weight: 700;
    color: #fff;
  }

  .hero-progress-container {
    margin-top: 1rem;
  }

  .progress-track {
    height: 8px;
    background: rgba(0, 0, 0, 0.3);
    border-radius: 4px;
    overflow: hidden;
    position: relative;
  }

  .progress-bar-glow {
    height: 100%;
    transition: width 0.5s cubic-bezier(0.4, 0, 0.2, 1);
    position: relative;
  }

  .progress-bar-glow::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(255, 255, 255, 0.2),
      transparent
    );
    animation: progress-slide 2s infinite;
  }

  @keyframes progress-slide {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(100%);
    }
  }

  .progress-meta {
    display: flex;
    justify-content: center;
    gap: 0.5rem;
    margin-top: 0.75rem;
    font-family: var(--font-mono);
    font-size: 0.7rem;
    font-weight: 700;
    color: var(--text-muted);
  }

  .progress-meta span:nth-child(2) {
    opacity: 0.4;
    font-size: 0.6rem;
  }

  .details-grid-premium {
    padding: 0 2rem 2rem;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    flex: 1;
    overflow-y: auto;
  }

  .details-section {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .section-label {
    font-size: 0.65rem;
    font-weight: 900;
    color: var(--color-primary);
    letter-spacing: 0.1em;
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .section-label::after {
    content: "";
    flex: 1;
    height: 1px;
    background: linear-gradient(90deg, rgba(0, 243, 255, 0.2), transparent);
  }

  .section-label.danger {
    color: #ff5252;
  }

  .section-label.danger::after {
    background: linear-gradient(90deg, rgba(255, 82, 82, 0.2), transparent);
  }

  .data-row-premium {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1.5rem;
  }

  .data-item {
    display: flex;
    align-items: center;
    gap: 0.85rem;
  }

  .data-item .material-icons {
    font-size: 1.25rem;
    color: var(--text-muted);
  }

  .item-content {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .item-content .l {
    font-size: 0.65rem;
    color: var(--text-muted);
    font-weight: 600;
  }

  .item-content .v {
    font-size: 0.85rem;
    color: var(--text-primary);
    font-weight: 600;
  }

  .link-item-premium {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.85rem 1rem;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    gap: 1rem;
    transition: all 0.2s;
  }

  .link-item-premium:hover {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.1);
  }

  .link-info {
    display: flex;
    align-items: center;
    gap: 1rem;
    min-width: 0;
  }

  .link-icon {
    font-size: 1.25rem;
    color: var(--color-primary);
  }

  .link-text {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }

  .link-text .l {
    font-size: 0.6rem;
    font-weight: 800;
    color: var(--text-muted);
    text-transform: uppercase;
  }

  .link-text .v {
    font-size: 0.75rem;
    font-family: var(--font-mono);
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .copy-tiny {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    transition: all 0.2s;
    padding: 0.4rem;
    border-radius: 6px;
  }

  .copy-tiny:hover {
    color: var(--color-primary);
    background: rgba(0, 243, 255, 0.1);
  }

  .error-box-premium {
    display: flex;
    gap: 1rem;
    padding: 1.25rem;
    background: rgba(255, 82, 82, 0.05);
    border: 1px solid rgba(255, 82, 82, 0.15);
    border-radius: 12px;
    align-items: flex-start;
  }

  .error-box-premium .material-icons {
    color: #ff5252;
    font-size: 1.5rem;
  }

  .error-box-premium p {
    margin: 0;
    font-size: 0.8rem;
    color: #ff8a80;
    line-height: 1.5;
  }

  .details-footer-premium {
    padding: 1.5rem 2rem;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    display: flex;
    justify-content: flex-end;
    background: rgba(0, 0, 0, 0.1);
  }

  .premium-btn-secondary {
    padding: 0.75rem 1.5rem;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    color: var(--text-secondary);
    font-size: 0.75rem;
    font-weight: 800;
    letter-spacing: 0.05em;
    cursor: pointer;
    transition: all 0.2s;
  }

  .premium-btn-secondary:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
    border-color: rgba(255, 255, 255, 0.2);
  }

  /* V6: Batch Detail Modal Styles */
  .batch-stats-grid {
    grid-template-columns: 1fr 1fr;
  }

  .batch-stats-compact {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5rem 1rem;
    margin-top: 0.75rem;
  }

  .stat-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0;
  }

  .stat-label-compact {
    font-size: 0.875rem;
    color: var(--text-secondary);
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .stat-icon {
    font-size: 0.9rem !important;
    color: rgba(255, 255, 255, 0.4);
    vertical-align: middle;
  }

  .stat-value-compact {
    font-size: 0.875rem;
    font-weight: 700;
    color: #fff;
  }

  .batch-actions-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
  }

  .batch-action-buttons {
    display: flex;
    gap: 0.75rem;
  }

  .premium-btn-action {
    padding: 0.75rem 1.25rem;
    background: rgba(0, 243, 255, 0.1);
    border: 1px solid rgba(0, 243, 255, 0.2);
    border-radius: 10px;
    color: #00f3ff;
    font-size: 0.75rem;
    font-weight: 800;
    letter-spacing: 0.05em;
    cursor: pointer;
    transition: all 0.2s;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .premium-btn-action .material-icons {
    font-size: 1rem;
  }

  .premium-btn-action:hover:not(:disabled) {
    background: rgba(0, 243, 255, 0.2);
    border-color: rgba(0, 243, 255, 0.4);
    transform: translateY(-1px);
  }

  .premium-btn-action.danger {
    background: rgba(255, 82, 82, 0.1);
    border-color: rgba(255, 82, 82, 0.2);
    color: #ff5252;
  }

  .premium-btn-action.danger:hover:not(:disabled) {
    background: rgba(255, 82, 82, 0.2);
    border-color: rgba(255, 82, 82, 0.4);
  }

  .premium-btn-action:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  /* Mobile Cards - Hidden on Desktop */
  .mobile-cards {
    display: none;
  }

  /* Mobile Styles */
  @media (max-width: 1024px) {
    .section-box {
      display: none;
    }

    .mobile-cards {
      display: flex;
      flex-direction: column;
      gap: 0.75rem;
    }

    .download-card-mobile {
      background: var(--bg-glass);
      border: 1px solid var(--border-glass);
      border-radius: 12px;
      padding: 1rem;
    }

    .card-header-mobile {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: 0.5rem;
      margin-bottom: 0.75rem;
    }

    .card-name {
      font-size: 0.8rem;
      font-weight: 600;
      color: var(--text-primary);
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      flex: 1;
    }

    .card-progress-mobile {
      height: 4px;
      background: rgba(255, 255, 255, 0.05);
      border-radius: 2px;
      overflow: hidden;
      margin-bottom: 0.75rem;
    }

    .card-meta-mobile {
      display: flex;
      gap: 1rem;
      font-size: 0.7rem;
      color: var(--text-muted);
    }

    .meta-item {
      display: flex;
      align-items: center;
      gap: 0.25rem;
    }

    .meta-item .material-icons {
      font-size: 14px;
    }

    .card-error-mobile {
      margin-top: 0.5rem;
      font-size: 0.65rem;
      color: #ff5252;
      padding: 0.5rem;
      background: rgba(255, 82, 82, 0.1);
      border-radius: 4px;
    }
  }

  /* Skeleton Loaders */
  .skeleton-row {
    opacity: 0.6;
  }

  .skeleton-box {
    height: 16px;
    background: linear-gradient(
      90deg,
      rgba(255, 255, 255, 0.03) 25%,
      rgba(255, 255, 255, 0.08) 50%,
      rgba(255, 255, 255, 0.03) 75%
    );
    background-size: 200% 100%;
    animation: shimmer 1.5s infinite;
    border-radius: 4px;
  }

  .progress-skeleton {
    height: 8px !important;
  }

  /* Batch Actions */
  .batch-actions {
    display: flex;
    gap: 0.25rem;
    align-items: center;
    justify-content: flex-end;
  }

  .batch-actions .icon-btn-tiny {
    opacity: 0.7;
    transition: opacity 0.2s;
  }

  .batch-actions .icon-btn-tiny:hover {
    opacity: 1;
  }

  .batch-actions .icon-btn-tiny.danger:hover {
    background: rgba(255, 82, 82, 0.1);
    border-color: rgba(255, 82, 82, 0.3);
    color: #ff5252;
  }

  @keyframes shimmer {
    0% {
      background-position: -200% 0;
    }
    100% {
      background-position: 200% 0;
    }
  }
</style>
