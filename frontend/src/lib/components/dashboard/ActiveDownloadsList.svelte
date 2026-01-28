<script lang="ts">
  interface DownloadItem {
    id: string;
    filename: string;
    state: string;
    progress: number;
  }

  interface Stats {
    active: number;
    queued: number;
    completed: number;
    failed: number;
  }

  interface Props {
    downloads: DownloadItem[];
    stats: Stats;
  }

  let { downloads, stats }: Props = $props();
</script>

<section class="box-section downloads-section">
  <div class="box-label">
    <span class="material-icons">list_alt</span>
    ACTIVE & QUEUED TASKS
  </div>
  <div class="toolbar-actions-dock">
    <div class="header-stats-row">
      <div class="h-stat">
        <span class="l">ACTIVE</span>
        <span class="v color-secondary">{stats.active}</span>
      </div>
      <div class="h-stat">
        <span class="l">QUEUED</span>
        <span class="v color-warning">{stats.queued}</span>
      </div>
      <div class="h-stat">
        <span class="l">COMPLETED</span>
        <span class="v color-success">{stats.completed}</span>
      </div>
    </div>
    <a href="/downloads" class="expand-btn">EXPAND LIST</a>
  </div>

  <div class="mini-queue">
    {#each downloads as dl}
      <div class="data-shard-card {dl.state}">
        <div class="shard-side-accent"></div>
        <div class="shard-main">
          <div class="shard-top">
            <span class="shard-filename" title={dl.filename}>{dl.filename}</span
            >
            <div class="shard-badge {dl.state}">
              <span class="dot"></span>
              {dl.state}
            </div>
          </div>

          <div class="shard-progress-block">
            <div class="shard-track">
              <div class="shard-fill" style="width: {dl.progress}%">
                <div class="fill-glow"></div>
              </div>
            </div>
            <div class="shard-meta">
              <span class="pct">{dl.progress}%</span>
              <span class="shard-id">NODE-{dl.id.substring(0, 6)}/A</span>
            </div>
          </div>
        </div>
      </div>
    {:else}
      <div class="empty-shard-placeholder">
        <span class="material-icons">radar</span>
        <p>NO ACTIVE DATA STREAMS DETECTED</p>
      </div>
    {/each}
  </div>
</section>

<style>
  .box-section {
    background: rgba(10, 15, 25, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 0;
    position: relative;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .box-section::before {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    width: 2px;
    height: 100%;
    background: var(--color-secondary);
    opacity: 0.5;
  }

  .box-label {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    font-size: 0.65rem;
    font-weight: 900;
    letter-spacing: 0.2rem;
    text-transform: uppercase;
    margin-bottom: 0.75rem;
    color: var(--color-secondary);
    font-family: var(--font-mono, monospace);
    padding: 0 0.5rem;
    flex-shrink: 0;
  }

  .box-label .material-icons {
    font-size: 1.1rem;
    opacity: 0.8;
  }

  .downloads-section {
    display: flex;
    flex-direction: column;
    height: 100%;
    position: relative;
  }

  .toolbar-actions-dock {
    position: absolute;
    top: 1rem;
    right: 1.5rem;
  }

  .header-stats-row {
    display: none;
  }

  .expand-btn {
    font-size: 0.6rem;
    font-weight: 800;
    color: var(--text-muted);
    text-decoration: none;
    border: 1px solid rgba(255, 255, 255, 0.1);
    padding: 3px 8px;
    border-radius: 4px;
    transition: all 0.2s;
  }

  .expand-btn:hover {
    color: var(--color-primary);
    border-color: var(--color-primary);
    background: rgba(0, 243, 255, 0.05);
  }

  .mini-queue {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    margin-top: 0.5rem;
    flex: 1;
    overflow-y: auto;
    padding-right: 0.5rem;
    min-height: 0;
  }

  .mini-queue::-webkit-scrollbar {
    width: 4px;
  }
  .mini-queue::-webkit-scrollbar-track {
    background: transparent;
  }
  .mini-queue::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 2px;
  }
  .mini-queue::-webkit-scrollbar-thumb:hover {
    background: var(--color-primary);
  }

  .data-shard-card {
    background: linear-gradient(
      135deg,
      rgba(255, 255, 255, 0.02) 0%,
      rgba(255, 255, 255, 0.04) 100%
    );
    border: 1px solid rgba(255, 255, 255, 0.03);
    border-radius: 4px;
    display: flex;
    position: relative;
    overflow: hidden;
    transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .data-shard-card:hover {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(0, 243, 255, 0.2);
    transform: translateY(-2px);
  }

  .shard-side-accent {
    width: 3px;
    background: var(--color-primary);
    opacity: 0.3;
    transition: opacity 0.3s;
  }

  .data-shard-card:hover .shard-side-accent {
    opacity: 1;
  }

  .shard-main {
    flex: 1;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .shard-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .shard-filename {
    font-size: 0.75rem;
    font-weight: 700;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 70%;
  }

  .shard-badge {
    font-size: 0.55rem;
    font-weight: 900;
    text-transform: uppercase;
    background: rgba(255, 255, 255, 0.05);
    padding: 2px 8px;
    border-radius: 20px;
    display: flex;
    align-items: center;
    gap: 4px;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .shard-badge .dot {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: #fff;
  }

  .shard-badge.downloading {
    color: var(--color-primary);
    border-color: rgba(0, 243, 255, 0.3);
  }
  .shard-badge.downloading .dot {
    background: var(--color-primary);
    box-shadow: 0 0 5px var(--color-primary);
  }

  .shard-badge.queued {
    color: #ffcc00;
    border-color: rgba(255, 204, 0, 0.3);
  }
  .shard-badge.queued .dot {
    background: #ffcc00;
  }

  .shard-badge.completed {
    color: var(--color-secondary);
    border-color: rgba(0, 255, 136, 0.3);
  }
  .shard-badge.completed .dot {
    background: var(--color-secondary);
  }

  .shard-progress-block {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .shard-track {
    height: 4px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 2px;
    overflow: hidden;
  }

  .shard-fill {
    height: 100%;
    background: linear-gradient(
      90deg,
      var(--color-primary),
      var(--color-secondary)
    );
    border-radius: 2px;
    position: relative;
    transition: width 0.3s ease;
  }

  .fill-glow {
    position: absolute;
    right: 0;
    top: 50%;
    transform: translateY(-50%);
    width: 20px;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.5));
    animation: pulse-glow 1.5s ease-in-out infinite;
  }

  @keyframes pulse-glow {
    0%,
    100% {
      opacity: 0.3;
    }
    50% {
      opacity: 1;
    }
  }

  .shard-meta {
    display: flex;
    justify-content: space-between;
    font-size: 0.6rem;
    font-family: var(--font-mono, monospace);
  }

  .shard-meta .pct {
    color: var(--color-primary);
    font-weight: 700;
  }

  .shard-meta .shard-id {
    color: var(--text-muted);
    opacity: 0.6;
  }

  .empty-shard-placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 3rem 1rem;
    color: var(--text-muted);
    opacity: 0.5;
    text-align: center;
  }

  .empty-shard-placeholder .material-icons {
    font-size: 48px;
    margin-bottom: 1rem;
    opacity: 0.3;
  }

  .empty-shard-placeholder p {
    font-size: 0.7rem;
    font-weight: 800;
    letter-spacing: 0.1em;
    font-family: var(--font-mono, monospace);
  }

  .h-stat {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }

  .h-stat .l {
    font-size: 0.5rem;
    font-weight: 800;
    color: var(--text-muted);
    letter-spacing: 0.1em;
  }

  .h-stat .v {
    font-size: 1rem;
    font-weight: 900;
    font-family: var(--font-mono, monospace);
  }

  .color-secondary {
    color: var(--color-secondary);
  }
  .color-warning {
    color: #ffcc00;
  }
  .color-success {
    color: var(--color-primary);
  }
</style>
