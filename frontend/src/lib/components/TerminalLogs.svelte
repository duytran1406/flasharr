<script>
  import { onMount } from "svelte";

  /** @type {any[]} */
  export let logs = [];

  /** @type {HTMLElement} */
  let terminalContainer;

  onMount(() => {
    if (terminalContainer) {
      terminalContainer.scrollTop = terminalContainer.scrollHeight;
    }
  });

  function clearLogs() {
    logs = [];
  }
</script>

<div class="terminal-monitor-v2">
  <div class="terminal-header">
    <div class="header-left">
      <span class="material-icons">terminal</span>
      <h2>System Logs</h2>
    </div>
    <button class="clear-btn" on:click={clearLogs} title="Clear Logs">
      <span class="material-icons">clear_all</span>
    </button>
  </div>

  <div class="terminal-body" bind:this={terminalContainer}>
    {#each logs as log (log.id)}
      <div class="log-entry">
        <span class="log-time">[{log.timestamp}]</span>
        <span
          class="log-level"
          class:error={log.level === "ERROR"}
          class:info={log.level === "INFO"}
        >
          [{log.level}]
        </span>
        <span class="log-message">{log.message}</span>
      </div>
    {/each}
  </div>
</div>

<style>
  .terminal-monitor-v2 {
    background: rgba(13, 17, 23, 0.7);
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    margin-top: 2rem;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: slideUp 0.3s ease-out;
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(20px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .terminal-header {
    padding: 0.75rem 1.25rem;
    background: rgba(255, 255, 255, 0.03);
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .header-left .material-icons {
    color: #00d4ff;
    font-size: 1.25rem;
  }

  .header-left h2 {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 600;
    color: #e6edf3;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .clear-btn {
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.5);
    cursor: pointer;
    padding: 4px;
    border-radius: 4px;
    transition: all 0.2s;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .clear-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #ff5252;
  }

  .terminal-body {
    padding: 1rem;
    height: 250px;
    overflow-y: auto;
    font-family: "JetBrains Mono", "Fira Code", monospace;
    font-size: 0.8rem;
    line-height: 1.5;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.2) transparent;
  }

  .log-entry {
    margin-bottom: 0.25rem;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .log-time {
    color: #7d8590;
    margin-right: 0.5rem;
  }

  .log-level {
    font-weight: 600;
    margin-right: 0.5rem;
  }

  .log-level.info {
    color: #00d4ff;
  }

  .log-level.error {
    color: #ff5252;
  }

  .log-message {
    color: #c9d1d9;
  }

  /* Scrollbar Styling */
  .terminal-body::-webkit-scrollbar {
    width: 6px;
  }

  .terminal-body::-webkit-scrollbar-track {
    background: transparent;
  }

  .terminal-body::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 3px;
  }

  .terminal-body::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.2);
  }
</style>
