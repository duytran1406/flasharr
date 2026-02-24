<script lang="ts">
  import { toasts } from "$lib/stores/toasts";
  import { animePop, animeFade } from "$lib/animations";

  // Subscribe to toasts store
  let toastList = $derived($toasts);
</script>

<div class="toasts-container">
  {#each toastList as toast (toast.id)}
    <div
      class="toast-terminal toast-{toast.type}"
      in:animePop={{ duration: 400, scale: 0.9 }}
      out:animeFade={{ duration: 200 }}
    >
      <div class="toast-header">
        <div class="toast-icon">
          {#if toast.type === "success"}
            <span class="material-icons">check_circle</span>
          {:else if toast.type === "error"}
            <span class="material-icons">error</span>
          {:else if toast.type === "warning"}
            <span class="material-icons">warning</span>
          {:else}
            <span class="material-icons">info</span>
          {/if}
        </div>
        <span class="toast-timestamp">[{toast.timestamp}]</span>
        <span class="toast-level">[{toast.type.toUpperCase()}]</span>
        <button
          class="toast-close"
          onclick={() => toasts.remove(toast.id)}
          aria-label="Close notification"
        >
          <span class="material-icons">close</span>
        </button>
      </div>
      <div class="toast-message">{toast.message}</div>
    </div>
  {/each}
</div>

<style>
  .toasts-container {
    position: fixed;
    top: 80px;
    right: 20px;
    z-index: 1000000;
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 450px;
    pointer-events: none;
  }

  .toast-terminal {
    background: rgba(13, 17, 23, 0.95);
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-left: 3px solid;
    border-radius: 8px;
    overflow: hidden;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    pointer-events: auto;
    font-family: "JetBrains Mono", "Fira Code", monospace;
    min-width: 350px;
    position: relative;
  }

  /* Halftone dot bleed from left border */
  .toast-terminal::before {
    content: "";
    position: absolute;
    top: -5px;
    bottom: -5px;
    left: -20px;
    width: 80px;
    background-image: radial-gradient(
      circle,
      var(--toast-color, #00d4ff) 0.8px,
      transparent 0.8px
    );
    background-size: 5px 5px;
    opacity: 0.12;
    pointer-events: none;
    z-index: 0;
    mask-image: radial-gradient(
      ellipse at 0% 50%,
      black 0%,
      rgba(0, 0, 0, 0.3) 20%,
      transparent 60%
    );
    -webkit-mask-image: radial-gradient(
      ellipse at 0% 50%,
      black 0%,
      rgba(0, 0, 0, 0.3) 20%,
      transparent 60%
    );
  }

  .toast-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    background: rgba(255, 255, 255, 0.03);
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    font-size: 0.75rem;
  }

  .toast-icon {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .toast-icon .material-icons {
    font-size: 1.125rem;
  }

  .toast-timestamp {
    color: #7d8590;
    font-size: 0.7rem;
  }

  .toast-level {
    font-weight: 700;
    font-size: 0.7rem;
    letter-spacing: 0.5px;
  }

  .toast-close {
    margin-left: auto;
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.5);
    cursor: pointer;
    padding: 2px;
    border-radius: 4px;
    transition: all 0.2s;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .toast-close:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }

  .toast-close .material-icons {
    font-size: 1rem;
  }

  .toast-message {
    padding: 0.875rem 1rem;
    color: #c9d1d9;
    font-size: 0.8rem;
    line-height: 1.5;
    word-break: break-word;
  }

  /* Toast Type Specific Styles */
  .toast-success {
    border-left-color: #00ffa3;
    --toast-color: #00ffa3;
  }

  .toast-success .toast-icon .material-icons {
    color: #00ffa3;
  }

  .toast-success .toast-level {
    color: #00ffa3;
  }

  .toast-error {
    border-left-color: #ff5252;
    --toast-color: #ff5252;
  }

  .toast-error .toast-icon .material-icons {
    color: #ff5252;
  }

  .toast-error .toast-level {
    color: #ff5252;
  }

  .toast-warning {
    border-left-color: #ffd700;
    --toast-color: #ffd700;
  }

  .toast-warning .toast-icon .material-icons {
    color: #ffd700;
  }

  .toast-warning .toast-level {
    color: #ffd700;
  }

  .toast-info {
    border-left-color: #00d4ff;
    --toast-color: #00d4ff;
  }

  .toast-info .toast-icon .material-icons {
    color: #00d4ff;
  }

  .toast-info .toast-level {
    color: #00d4ff;
  }

  /* Mobile Responsive */
  @media (max-width: 768px) {
    .toasts-container {
      top: 70px;
      right: 10px;
      left: 10px;
      max-width: none;
    }

    .toast-terminal {
      min-width: auto;
    }

    .toast-header {
      padding: 0.625rem 0.875rem;
      font-size: 0.7rem;
    }

    .toast-message {
      padding: 0.75rem 0.875rem;
      font-size: 0.75rem;
    }

    .toast-timestamp {
      display: none;
    }
  }
</style>
