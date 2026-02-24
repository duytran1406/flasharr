<script lang="ts">
  import Button from "$lib/components/ui/Button.svelte";

  interface Props {
    title?: string;
    message?: string;
    showRetry?: boolean;
    onRetry?: () => void;
  }

  let {
    title = "Something went wrong",
    message = "We couldn't load the content. Please try again.",
    showRetry = true,
    onRetry,
  }: Props = $props();
</script>

<div class="error-state">
  <div class="error-icon">
    <span class="material-icons">error_outline</span>
  </div>
  <h3 class="error-title">{title}</h3>
  <p class="error-message">{message}</p>
  {#if showRetry && onRetry}
    <Button icon="refresh" size="md" onclick={onRetry}>Try Again</Button>
  {/if}
</div>

<style>
  .error-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 3rem 2rem;
    text-align: center;
    background: rgba(255, 100, 100, 0.05);
    border: 1px solid rgba(255, 100, 100, 0.2);
    border-radius: 16px;
    min-height: 200px;
  }

  .error-icon {
    width: 64px;
    height: 64px;
    border-radius: 50%;
    background: rgba(255, 100, 100, 0.1);
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 1rem;
  }

  .error-icon .material-icons {
    font-size: 32px;
    color: #ff6464;
  }

  .error-title {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 700;
    color: #fff;
    margin-bottom: 0.5rem;
  }

  .error-message {
    margin: 0;
    font-size: 0.9rem;
    color: rgba(255, 255, 255, 0.6);
    max-width: 300px;
    line-height: 1.5;
  }

  .retry-btn {
    margin-top: 1.5rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 1.5rem;
    background: rgba(0, 243, 255, 0.1);
    border: 1px solid rgba(0, 243, 255, 0.3);
    color: var(--color-primary, #00f3ff);
    border-radius: 8px;
    font-size: 0.9rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.3s ease;
  }

  .retry-btn:hover {
    background: rgba(0, 243, 255, 0.2);
    border-color: rgba(0, 243, 255, 0.5);
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.2);
  }

  .retry-btn .material-icons {
    font-size: 18px;
  }
</style>
