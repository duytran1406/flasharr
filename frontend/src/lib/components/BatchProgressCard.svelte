<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  interface BatchProgress {
    batch_id: string;
    batch_name: string;
    total_tasks: number;
    completed_tasks: number;
    overall_progress: number;
    combined_speed: number;
    estimated_time_remaining: number;
  }

  let { batchId }: { batchId: string } = $props();
  let progress = $state<BatchProgress | null>(null);
  let interval: ReturnType<typeof setInterval> | null = null;

  async function fetchProgress() {
    try {
      const res = await fetch(`/api/downloads/batch/${batchId}/progress`);
      if (res.ok) progress = await res.json();
    } catch (err) {
      console.error("Failed to fetch batch progress:", err);
    }
  }

  async function pauseBatch() {
    await fetch(`/api/downloads/batch/${batchId}/pause`, { method: "POST" });
  }

  async function resumeBatch() {
    await fetch(`/api/downloads/batch/${batchId}/resume`, { method: "POST" });
  }

  async function deleteBatch() {
    if (confirm(`Delete batch?`)) {
      await fetch(`/api/downloads/batch/${batchId}`, { method: "DELETE" });
    }
  }

  function formatSpeed(b: number): string {
    return b ? `${(b / 1024 / 1024).toFixed(1)} MB/s` : "0 B/s";
  }

  function formatETA(s: number): string {
    if (!s || s === Infinity) return "∞";
    const m = Math.floor(s / 60);
    return m > 60 ? `${Math.floor(m / 60)}h ${m % 60}m` : `${m}m`;
  }

  onMount(() => {
    fetchProgress();
    interval = setInterval(fetchProgress, 2000);
  });

  onDestroy(() => {
    if (interval) clearInterval(interval);
  });
</script>

{#if progress}
  <div class="batch-inline">
    <div class="progress-bar-wrap">
      <div
        class="progress-bar"
        style:width="{progress.overall_progress}%"
      ></div>
    </div>
    <div class="stats">
      <span class="stat"><b>{progress.overall_progress.toFixed(1)}%</b></span>
      <span class="stat"
        >{progress.completed_tasks}/{progress.total_tasks} files</span
      >
      <span class="stat speed">{formatSpeed(progress.combined_speed)}</span>
      <span class="stat">{formatETA(progress.estimated_time_remaining)}</span>
      <div class="actions">
        <button onclick={resumeBatch} title="Resume"
          ><i class="material-icons">play_arrow</i></button
        >
        <button onclick={pauseBatch} title="Pause"
          ><i class="material-icons">pause</i></button
        >
        <button onclick={deleteBatch} title="Delete" class="danger"
          ><i class="material-icons">delete</i></button
        >
      </div>
    </div>
  </div>
{/if}

<style>
  .batch-inline {
    background: rgba(10, 14, 26, 0.4);
    border: 1px solid rgba(0, 243, 255, 0.15);
    border-radius: 6px;
    padding: 8px 12px;
    margin: 6px 0;
  }

  .progress-bar-wrap {
    width: 100%;
    height: 3px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 2px;
    overflow: hidden;
    margin-bottom: 8px;
  }

  .progress-bar {
    height: 100%;
    background: linear-gradient(90deg, #00f3ff, #00ffa3);
    transition: width 0.3s ease;
    box-shadow: 0 0 6px #00f3ff;
  }

  .stats {
    display: flex;
    align-items: center;
    gap: 16px;
    font-size: 0.75rem;
  }

  .stat {
    color: rgba(255, 255, 255, 0.7);
  }

  .stat b {
    color: #00ffa3;
  }

  .stat.speed {
    color: #00f3ff;
    font-weight: 600;
  }

  .actions {
    display: flex;
    gap: 6px;
    margin-left: auto;
  }

  button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    background: rgba(0, 243, 255, 0.1);
    border: 1px solid rgba(0, 243, 255, 0.2);
    border-radius: 4px;
    color: #00f3ff;
    cursor: pointer;
    transition: all 0.2s;
  }

  button i {
    font-size: 16px;
  }

  button:hover {
    background: rgba(0, 243, 255, 0.2);
    box-shadow: 0 0 8px rgba(0, 243, 255, 0.3);
  }

  button.danger {
    border-color: rgba(255, 68, 68, 0.2);
    color: #ff4444;
  }

  button.danger:hover {
    background: rgba(255, 68, 68, 0.2);
    box-shadow: 0 0 8px rgba(255, 68, 68, 0.3);
  }
</style>
