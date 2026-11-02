<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    title?: string;
    message?: string;
    /** Show the "Try Again" button (only rendered when onRetry is set) */
    showRetry?: boolean;
    onRetry?: () => void;
    /** Custom icon; defaults to the Material "error_outline" glyph */
    icon?: Snippet;
  }

  let {
    title = "Something went wrong",
    message = "We couldn't load the content. Please try again.",
    showRetry = true,
    onRetry,
    icon,
  }: Props = $props();
</script>

<div class="error-state" role="alert">
  <div class="error-icon">
    {#if icon}
      {@render icon()}
    {:else}
      <span class="material-icons">error_outline</span>
    {/if}
  </div>

  <h3 class="error-title">{title}</h3>
  <p class="error-message">{message}</p>

  {#if showRetry && onRetry}
    <button type="button" class="retry-btn" onclick={onRetry}>
      <span class="material-icons">refresh</span>
      Try Again
    </button>
  {/if}
</div>

<style>
  .error-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: var(--cui-space-xxl) var(--cui-space-xl);
    text-align: center;
    background: color-mix(in srgb, var(--cui-danger) 5%, transparent);
    border: 1px solid color-mix(in srgb, var(--cui-danger) 20%, transparent);
    clip-path: polygon(
      8px 0,
      100% 0,
      100% calc(100% - 8px),
      calc(100% - 8px) 100%,
      0 100%,
      0 8px
    );
    min-height: 200px;
  }

  .error-icon {
    width: 64px;
    height: 64px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--cui-danger) 10%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 1rem;
  }

  .error-icon .material-icons {
    font-size: 32px;
    color: var(--cui-danger);
  }

  .error-title {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--cui-text-primary);
    margin-bottom: 0.5rem;
  }

  .error-message {
    margin: 0;
    font-size: 0.9rem;
    color: var(--cui-text-secondary);
    max-width: 300px;
    line-height: 1.5;
  }

  .retry-btn {
    margin-top: 1.5rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 1.5rem;
    background: var(--cui-surface-active);
    border: 1px solid var(--cui-glass-border-active);
    color: var(--cui-primary);
    border-radius: var(--cui-radius-sm);
    font-size: var(--cui-text-sm);
    font-weight: 600;
    cursor: pointer;
    transition:
      background var(--cui-duration-base) var(--cui-ease-standard),
      border-color var(--cui-duration-base) var(--cui-ease-standard),
      box-shadow var(--cui-duration-base) var(--cui-ease-standard);
  }

  .retry-btn:hover {
    background: var(--cui-surface-focus);
    border-color: var(--cui-glass-border-active);
    box-shadow: var(--cui-glow-primary);
  }

  .retry-btn .material-icons {
    font-size: 18px;
  }
</style>
