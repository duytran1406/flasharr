<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import {
    downloads,
    downloadStore,
    activeDownloads,
    queuedDownloads,
    completedDownloads,
    failedDownloads,
    pausedDownloads,
    formatBytes,
    formatSpeed,
    formatETA,
    isLoading,
    type DownloadTask,
    type DownloadState,
  } from "$lib/stores/downloads";
  import { toasts } from "$lib/stores/toasts";

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

  // Sorting state - V2 feature
  type SortColumn =
    | "filename"
    | "state"
    | "size"
    | "progress"
    | "speed"
    | "eta"
    | "added";
  type SortDirection = "asc" | "desc";
  let sortColumn = $state<SortColumn>("added");
  let sortDirection = $state<SortDirection>("desc");

  // Pagination state - V2 uses 12 items per page
  let currentPage = $state(1);
  const itemsPerPage = 12;

  // Context menu state
  let contextMenuId = $state<string | null>(null);
  let contextMenuPos = $state({ x: 0, y: 0 });

  // V2: Delete confirmation modal state
  let deleteConfirmTask = $state<DownloadTask | null>(null);

  // V2: Task details modal state
  let detailsTask = $state<DownloadTask | null>(null);

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

  // Sort icon - V2 feature
  function getSortIcon(col: SortColumn): string {
    if (sortColumn !== col) return "sort";
    return sortDirection === "asc" ? "expand_less" : "expand_more";
  }

  // Toggle sort - V2 feature
  function toggleSort(col: SortColumn) {
    if (sortColumn === col) {
      sortDirection = sortDirection === "asc" ? "desc" : "asc";
    } else {
      sortColumn = col;
      sortDirection = "desc";
    }
  }

  // Filtered and sorted downloads
  let filteredDownloads = $derived(() => {
    let list: DownloadTask[] = [];

    // Apply filter
    switch (currentFilter) {
      case "active":
        list = $activeDownloads;
        break;
      case "queued":
        list = $queuedDownloads;
        break;
      case "completed":
        list = $completedDownloads;
        break;
      case "failed":
        list = $failedDownloads;
        break;
      case "paused":
        list = $pausedDownloads;
        break;
      default:
        list = $downloads;
    }

    // Apply search - V2 feature (omniSearchQuery)
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      list = list.filter(
        (d) =>
          d.filename.toLowerCase().includes(q) ||
          d.id.toLowerCase().includes(q),
      );
    }

    // Apply sorting - V2 feature
    list = [...list].sort((a, b) => {
      const dir = sortDirection === "asc" ? 1 : -1;
      let valA: any;
      let valB: any;

      switch (sortColumn) {
        case "filename":
          valA = a.filename || "";
          valB = b.filename || "";
          return valA.localeCompare(valB) * dir;
        case "state":
          valA = a.state || "";
          valB = b.state || "";
          return valA.localeCompare(valB) * dir;
        case "size":
          valA = a.size || 0;
          valB = b.size || 0;
          break;
        case "progress":
          valA = a.progress || 0;
          valB = b.progress || 0;
          break;
        case "speed":
          valA = a.speed || 0;
          valB = b.speed || 0;
          break;
        case "eta":
          valA = a.eta || 999999;
          valB = b.eta || 999999;
          break;
        case "added":
          valA = new Date(a.created_at || 0).getTime();
          valB = new Date(b.created_at || 0).getTime();
          break;
        default:
          return 0;
      }
      return (valA - valB) * dir;
    });

    return list;
  });

  // Filter counts
  let filterCounts = $derived({
    all: $downloads.length,
    active: $activeDownloads.length,
    queued: $queuedDownloads.length,
    completed: $completedDownloads.length,
    failed: $failedDownloads.length,
    paused: $pausedDownloads.length,
  });

  let totalPages = $derived(
    Math.ceil(filteredDownloads().length / itemsPerPage) || 1,
  );

  let paginatedDownloads = $derived(() => {
    const list = filteredDownloads();
    const start = (currentPage - 1) * itemsPerPage;
    return list.slice(start, start + itemsPerPage);
  });

  // Pagination info text - V2 style
  let paginationInfo = $derived(() => {
    const total = filteredDownloads().length;
    if (total === 0) return "Showing 0 items";
    const start = (currentPage - 1) * itemsPerPage + 1;
    const end = Math.min(currentPage * itemsPerPage, total);
    return `Showing ${start}-${end} of ${total} items`;
  });

  function setPage(p: number) {
    if (p >= 1 && p <= totalPages) {
      currentPage = p;
      const container = document.querySelector(".downloads-container");
      if (container) container.scrollTop = 0;
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

  function showContextMenu(e: MouseEvent, id: string) {
    e.preventDefault();
    contextMenuId = id;
    contextMenuPos = { x: e.clientX, y: e.clientY };
  }

  function hideContextMenu() {
    contextMenuId = null;
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

  async function confirmDelete() {
    if (!deleteConfirmTask) return;
    const id = deleteConfirmTask.id;
    deleteConfirmTask = null;

    const result = await downloadStore.deleteDownload(id);
    if (!result.success) {
      toasts.error(`Failed to delete: ${result.error}`);
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

  onMount(() => {
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
  });
</script>

<svelte:head>
  <title>Downloads - Flasharr</title>
</svelte:head>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="downloads-container"
  onclick={hideContextMenu}
  onkeydown={(e) => e.key === "Escape" && hideContextMenu()}
  role="presentation"
>
  <!-- V2 Style Toolbar -->
  <div class="section-box">
    <div class="toolbar">
      <h2 class="glow-text">Active Downloads</h2>
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

    <!-- Filter Tabs (V3 addition kept) -->
    <div class="filter-tabs">
      <button
        class="filter-tab"
        class:active={currentFilter === "all"}
        onclick={() => {
          currentFilter = "all";
          currentPage = 1;
        }}
      >
        <span>All</span>
        <span class="count">{filterCounts.all}</span>
      </button>
      <button
        class="filter-tab"
        class:active={currentFilter === "active"}
        onclick={() => {
          currentFilter = "active";
          currentPage = 1;
        }}
      >
        <span>Active</span>
        <span class="count active-count">{filterCounts.active}</span>
      </button>
      <button
        class="filter-tab"
        class:active={currentFilter === "queued"}
        onclick={() => {
          currentFilter = "queued";
          currentPage = 1;
        }}
      >
        <span>Queued</span>
        <span class="count queued-count">{filterCounts.queued}</span>
      </button>
      <button
        class="filter-tab"
        class:active={currentFilter === "completed"}
        onclick={() => {
          currentFilter = "completed";
          currentPage = 1;
        }}
      >
        <span>Completed</span>
        <span class="count completed-count">{filterCounts.completed}</span>
      </button>
      <button
        class="filter-tab"
        class:active={currentFilter === "failed"}
        onclick={() => {
          currentFilter = "failed";
          currentPage = 1;
        }}
      >
        <span>Failed</span>
        <span class="count failed-count">{filterCounts.failed}</span>
      </button>
      <button
        class="filter-tab"
        class:active={currentFilter === "paused"}
        onclick={() => {
          currentFilter = "paused";
          currentPage = 1;
        }}
      >
        <span>Paused</span>
        <span class="count paused-count">{filterCounts.paused}</span>
      </button>

      <!-- Search Box -->
      <div class="search-box">
        <span class="material-icons">search</span>
        <input
          type="text"
          placeholder="Search downloads..."
          bind:value={searchQuery}
        />
      </div>
    </div>

    <!-- Downloads Content -->
    <div class="downloads-content">
      {#if $isLoading && $downloads.length === 0}
        <div class="loading-state">
          <div class="spinner-ring"></div>
          <p>Synchronizing download tasks...</p>
        </div>
      {:else if filteredDownloads().length === 0}
        <div class="empty-state-premium">
          <div class="empty-icon-ring">
            <span class="material-icons">cloud_done</span>
          </div>
          <h3>Queue is Clear</h3>
          <p>No active downloads in queue</p>
          {#if searchQuery}
            <button class="v3-btn-outline" onclick={() => (searchQuery = "")}>
              Clear search filter
            </button>
          {:else}
            <a href="/search" class="v3-btn-primary">GO TO SEARCH</a>
          {/if}
        </div>
      {:else}
        <!-- V2 Style Table -->
        <div class="table-wrapper">
          <table class="data-table">
            <thead>
              <tr>
                <th style="width: 30%;" onclick={() => toggleSort("filename")}>
                  <div class="th-content">
                    Filename
                    <span
                      class="material-icons sort-icon"
                      class:active={sortColumn === "filename"}
                      >{getSortIcon("filename")}</span
                    >
                  </div>
                </th>
                <th style="width: 10%;" onclick={() => toggleSort("state")}>
                  <div class="th-content">
                    Status
                    <span
                      class="material-icons sort-icon"
                      class:active={sortColumn === "state"}
                      >{getSortIcon("state")}</span
                    >
                  </div>
                </th>
                <th style="width: 10%;" onclick={() => toggleSort("size")}>
                  <div class="th-content">
                    Size
                    <span
                      class="material-icons sort-icon"
                      class:active={sortColumn === "size"}
                      >{getSortIcon("size")}</span
                    >
                  </div>
                </th>
                <th style="width: 14%;" onclick={() => toggleSort("progress")}>
                  <div class="th-content">
                    Progress
                    <span
                      class="material-icons sort-icon"
                      class:active={sortColumn === "progress"}
                      >{getSortIcon("progress")}</span
                    >
                  </div>
                </th>
                <th style="width: 8%;" onclick={() => toggleSort("speed")}>
                  <div class="th-content">
                    Speed
                    <span
                      class="material-icons sort-icon"
                      class:active={sortColumn === "speed"}
                      >{getSortIcon("speed")}</span
                    >
                  </div>
                </th>
                <th style="width: 8%;" onclick={() => toggleSort("eta")}>
                  <div class="th-content">
                    ETA
                    <span
                      class="material-icons sort-icon"
                      class:active={sortColumn === "eta"}
                      >{getSortIcon("eta")}</span
                    >
                  </div>
                </th>
                <th style="width: 14%;" onclick={() => toggleSort("added")}>
                  <div class="th-content">
                    Added
                    <span
                      class="material-icons sort-icon"
                      class:active={sortColumn === "added"}
                      >{getSortIcon("added")}</span
                    >
                  </div>
                </th>
                <th style="width: 6%; text-align: right;">...</th>
              </tr>
            </thead>
            <tbody>
              {#each paginatedDownloads() as download (download.id)}
                {@const color = getStateColorV2(download.state)}
                {@const icon = getStateIconV2(download.state)}
                {@const isActive = isDownloadActive(download.state)}
                {@const isError = download.state === "FAILED"}
                <tr
                  class="transfer-row"
                  data-id={download.id}
                  oncontextmenu={(e) => showContextMenu(e, download.id)}
                >
                  <td class="filename-cell" title={download.filename}>
                    {download.filename}
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
                        ⚠ {download.error_message}
                      </div>
                    {/if}
                  </td>
                  <td class="size-cell">{formatBytes(download.size)}</td>
                  <td>
                    <div class="progress-cell">
                      <div class="progress-header">
                        <span>{Math.round(download.progress)}%</span>
                      </div>
                      <div class="progress-bar-v2">
                        <div
                          class="progress-fill-v2"
                          style="width: {download.progress}%; background: {color}; box-shadow: 0 0 6px {color}80;"
                        ></div>
                      </div>
                    </div>
                  </td>
                  <td class="speed-cell" style="color: {color};">
                    {isActive ? formatSpeed(download.speed || 0) : "-"}
                  </td>
                  <td class="eta-cell">
                    {isActive ? formatETA(download.eta) : "-"}
                  </td>
                  <td class="added-cell">
                    {formatAddedDate(download.created_at)}
                  </td>
                  <td class="actions-cell">
                    <button
                      class="icon-btn-tiny"
                      onclick={(e) => {
                        e.stopPropagation();
                        showContextMenu(e, download.id);
                      }}
                      title="More options"
                    >
                      <span class="material-icons">more_vert</span>
                    </button>
                  </td>
                </tr>
              {/each}
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
      {/if}
    </div>
  </div>

  <!-- Mobile Card View -->
  <div class="mobile-cards">
    {#each paginatedDownloads() as download (download.id)}
      {@const color = getStateColorV2(download.state)}
      {@const isActive = isDownloadActive(download.state)}
      <div
        class="download-card-mobile"
        oncontextmenu={(e) => showContextMenu(e, download.id)}
      >
        <div class="card-header-mobile">
          <div class="card-name">{download.filename}</div>
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
      </div>
    {/each}
  </div>
</div>

<!-- V2 Style Context Menu -->
{#if contextMenuId}
  {#each $downloads.filter((d) => d.id === contextMenuId) as download}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="context-menu"
      style="left: {contextMenuPos.x}px; top: {contextMenuPos.y}px;"
      onclick={(e) => e.stopPropagation()}
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

<!-- V2 Style Delete Confirmation Modal -->
{#if deleteConfirmTask}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="delete-modal-overlay" onclick={hideDeleteConfirm}>
    <div class="delete-modal" onclick={(e) => e.stopPropagation()}>
      <div class="delete-modal-header">
        <div class="delete-icon-box">
          <span class="material-icons">warning</span>
        </div>
        <div>
          <h3>Delete Download</h3>
          <p>This action cannot be undone</p>
        </div>
      </div>

      <p class="delete-modal-text">
        Are you sure you want to delete this download?
      </p>

      <div class="delete-filename-box">
        {deleteConfirmTask.filename.length > 50
          ? deleteConfirmTask.filename.substring(0, 50) + "..."
          : deleteConfirmTask.filename}
      </div>

      <div class="delete-warning-box">
        <span class="material-icons">folder_delete</span>
        <p>
          <strong>Warning:</strong> The downloaded file will also be deleted from
          the disk.
        </p>
      </div>

      <div class="delete-modal-actions">
        <button class="delete-cancel-btn" onclick={hideDeleteConfirm}
          >Cancel</button
        >
        <button class="delete-confirm-btn" onclick={confirmDelete}
          >Delete Permanently</button
        >
      </div>
    </div>
  </div>
{/if}

<!-- V2 Style Task Details Modal -->
{#if detailsTask}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="details-modal-overlay"
    onclick={hideTaskDetails}
    transition:fade={{ duration: 200 }}
  >
    <div
      class="details-modal glass-panel-premium"
      onclick={(e) => e.stopPropagation()}
      in:fly={{ y: 20, duration: 400 }}
    >
      <!-- Header with Status -->
      <div class="details-header-premium">
        <div class="header-main">
          <div
            class="id-badge"
            onclick={() => {
              navigator.clipboard.writeText(detailsTask!.id);
              toasts.success("ID copied");
            }}
          >
            <span class="material-icons">fingerprint</span>
            <span>{detailsTask.id.substring(0, 8)}...</span>
          </div>
          <h3 class="filename-title" title={detailsTask.filename}>
            {detailsTask.filename}
          </h3>
        </div>
        <button class="close-btn-premium" onclick={hideTaskDetails}>
          <span class="material-icons">close</span>
        </button>
      </div>

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
        <button class="premium-btn-secondary" onclick={hideTaskDetails}
          >CLOSE TERMINAL</button
        >
      </div>
    </div>
  </div>
{/if}

<style>
  .downloads-container {
    padding: 1.5rem;
    height: 100%;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
  }

  .section-box {
    background: var(--bg-glass);
    border: 1px solid var(--border-glass);
    border-radius: 12px;
    padding: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    flex: 1;
  }

  /* V2 Toolbar */
  .toolbar {
    padding: 1rem 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .toolbar h2 {
    font-size: 1rem;
    text-transform: uppercase;
    margin: 0;
  }

  .glow-text {
    background: linear-gradient(135deg, #fff 0%, var(--color-primary) 100%);
    -webkit-background-clip: text;
    background-clip: text;
    -webkit-text-fill-color: transparent;
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

  /* Filter Tabs */
  .filter-tabs {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    align-items: center;
    padding: 0.75rem 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
    background: rgba(0, 0, 0, 0.15);
  }

  .filter-tab {
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    padding: 0.4rem 0.75rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    transition: all 0.2s;
    font-size: 0.7rem;
    font-weight: 700;
    color: var(--text-muted);
  }

  .filter-tab:hover {
    background: rgba(255, 255, 255, 0.03);
    color: var(--text-secondary);
  }

  .filter-tab.active {
    background: rgba(0, 243, 255, 0.1);
    border-color: var(--color-primary);
    color: var(--color-primary);
  }

  .count {
    font-family: var(--font-mono);
    font-size: 0.6rem;
    padding: 2px 5px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-muted);
  }

  .active-count {
    background: rgba(0, 255, 128, 0.1);
    color: #00ff80;
  }
  .queued-count {
    background: rgba(255, 204, 0, 0.1);
    color: #ffd700;
  }
  .completed-count {
    background: rgba(0, 255, 163, 0.1);
    color: #00ffa3;
  }
  .failed-count {
    background: rgba(255, 82, 82, 0.1);
    color: #ff5252;
  }
  .paused-count {
    background: rgba(100, 116, 139, 0.1);
    color: #64748b;
  }

  /* Search Box */
  .search-box {
    margin-left: auto;
    min-width: 200px;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid var(--border-glass);
    border-radius: 6px;
    padding: 0.4rem 0.75rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .search-box .material-icons {
    font-size: 16px;
    color: var(--text-muted);
  }

  .search-box input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: 0.7rem;
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
    padding: 0.6rem 1rem;
    text-align: left;
    font-size: 0.62rem;
    font-weight: 800;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
    cursor: pointer;
    user-select: none;
  }

  .data-table th:hover {
    color: var(--text-secondary);
  }

  .th-content {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .sort-icon {
    font-size: 12px !important;
    opacity: 0.3;
    transition: all 0.2s;
  }

  .sort-icon.active {
    opacity: 1;
    color: var(--color-primary);
  }

  .data-table td {
    padding: 0.5rem 1rem;
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
    gap: 3px;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 0.55rem;
    font-weight: 800;
    border: 1px solid;
    text-transform: uppercase;
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
  .progress-cell {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .progress-header {
    font-size: 0.55rem;
    font-weight: 700;
    opacity: 0.8;
  }

  .progress-bar-v2 {
    height: 3px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-fill-v2 {
    height: 100%;
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

  .added-cell {
    color: var(--text-muted);
    font-size: 0.65rem;
  }

  .actions-cell {
    text-align: right;
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

  /* Empty State */
  .empty-state-premium {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    text-align: center;
    gap: 1rem;
    padding: 3rem;
  }

  .empty-icon-ring {
    width: 56px;
    height: 56px;
    border-radius: 50%;
    background: rgba(0, 243, 255, 0.1);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .empty-icon-ring .material-icons {
    font-size: 28px;
    color: var(--color-primary);
    opacity: 0.6;
  }

  .empty-state-premium h3 {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text-secondary);
    margin: 0;
  }

  .empty-state-premium p {
    font-size: 0.75rem;
    margin: 0;
  }

  .v3-btn-primary {
    background: var(--color-primary);
    color: #000;
    padding: 0.6rem 1.25rem;
    border-radius: 6px;
    font-weight: 800;
    font-size: 0.75rem;
    letter-spacing: 0.05em;
    text-decoration: none;
    transition: all 0.2s;
  }

  .v3-btn-outline {
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #fff;
    padding: 0.5rem 1rem;
    border-radius: 6px;
    font-weight: 700;
    font-size: 0.7rem;
    cursor: pointer;
    transition: all 0.2s;
  }

  /* Loading State */
  .loading-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1.5rem;
    color: var(--text-muted);
  }

  .spinner-ring {
    width: 48px;
    height: 48px;
    border: 3px solid rgba(0, 243, 255, 0.1);
    border-top-color: var(--color-primary);
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
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

  /* V2: Delete Confirmation Modal */
  .delete-modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.85);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10000;
    backdrop-filter: blur(8px);
    animation: fadeIn 0.2s ease;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .delete-modal {
    background: linear-gradient(
      145deg,
      rgba(30, 30, 40, 0.98),
      rgba(20, 20, 30, 0.98)
    );
    border: 1px solid rgba(255, 82, 82, 0.3);
    border-radius: 16px;
    padding: 2rem;
    max-width: 420px;
    width: 90%;
    box-shadow:
      0 25px 60px rgba(0, 0, 0, 0.7),
      0 0 30px rgba(255, 82, 82, 0.15);
  }

  .delete-modal-header {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 1.5rem;
  }

  .delete-icon-box {
    width: 48px;
    height: 48px;
    border-radius: 12px;
    background: rgba(255, 82, 82, 0.15);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .delete-icon-box .material-icons {
    font-size: 28px;
    color: #ff5252;
  }

  .delete-modal-header h3 {
    margin: 0;
    font-size: 1.1rem;
    color: #ff5252;
    font-weight: 700;
  }

  .delete-modal-header p {
    margin: 0.25rem 0 0;
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .delete-modal-text {
    color: var(--text-secondary);
    font-size: 0.85rem;
    line-height: 1.6;
    margin-bottom: 1rem;
  }

  .delete-filename-box {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    padding: 0.75rem 1rem;
    margin-bottom: 1.5rem;
    font-size: 0.8rem;
    color: var(--text-primary);
    font-weight: 600;
    word-break: break-all;
  }

  .delete-warning-box {
    background: rgba(255, 82, 82, 0.1);
    border: 1px solid rgba(255, 82, 82, 0.2);
    border-radius: 8px;
    padding: 0.75rem 1rem;
    margin-bottom: 1.5rem;
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .delete-warning-box .material-icons {
    font-size: 18px;
    color: #ff5252;
  }

  .delete-warning-box p {
    margin: 0;
    font-size: 0.75rem;
    color: #ff8a80;
  }

  .delete-modal-actions {
    display: flex;
    gap: 0.75rem;
    justify-content: flex-end;
  }

  .delete-cancel-btn {
    padding: 0.75rem 1.5rem;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-primary);
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .delete-cancel-btn:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .delete-confirm-btn {
    padding: 0.75rem 1.5rem;
    border-radius: 8px;
    border: none;
    background: linear-gradient(135deg, #ff5252, #ff1744);
    color: white;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
    box-shadow: 0 4px 15px rgba(255, 82, 82, 0.3);
  }

  .delete-confirm-btn:hover {
    transform: translateY(-2px);
  }

  /* V2: Task Details Modal Redesign */
  .details-modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(1, 2, 4, 0.85);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10000;
    backdrop-filter: blur(16px) saturate(180%);
    animation: fadeIn 0.3s ease;
  }

  .glass-panel-premium {
    background: linear-gradient(
      145deg,
      rgba(15, 23, 42, 0.9),
      rgba(2, 4, 8, 0.95)
    );
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 20px;
    width: 600px;
    max-width: 95%;
    max-height: 90vh;
    box-shadow:
      0 30px 60px -12px rgba(0, 0, 0, 0.8),
      0 0 40px rgba(0, 243, 255, 0.05);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    position: relative;
  }

  .details-header-premium {
    padding: 1.5rem 2rem;
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(255, 255, 255, 0.02);
  }

  .header-main {
    flex: 1;
    min-width: 0;
  }

  .id-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.25rem 0.6rem;
    background: rgba(0, 243, 255, 0.1);
    border: 1px solid rgba(0, 243, 255, 0.2);
    border-radius: 6px;
    font-family: var(--font-mono);
    font-size: 0.65rem;
    color: var(--color-primary);
    margin-bottom: 0.75rem;
    cursor: pointer;
    transition: all 0.2s;
  }

  .id-badge:hover {
    background: rgba(0, 243, 255, 0.2);
    border-color: var(--color-primary);
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

  .close-btn-premium {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: var(--text-muted);
    width: 36px;
    height: 36px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.2s;
    margin-left: 1.5rem;
  }

  .close-btn-premium:hover {
    background: rgba(255, 82, 82, 0.1);
    color: #ff5252;
    border-color: rgba(255, 82, 82, 0.3);
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
</style>
