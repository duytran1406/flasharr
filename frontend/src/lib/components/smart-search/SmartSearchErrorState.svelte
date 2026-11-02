<script lang="ts">
  import { Button } from "@media-set/core-ui";

  interface Props {
    error?: string | null;
    onRetry?: () => void;
  }

  let { error = "Something went wrong.", onRetry }: Props = $props();
</script>

<div class="error-state" role="alert">
  <span class="material-icons">error_outline</span>
  <p>{error}</p>
  {#if onRetry}
    <Button size="sm" icon="refresh" onclick={onRetry}>Retry</Button>
  {/if}
</div>

<style>
  .error-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 300px;
    gap: var(--cui-space-md);
    color: var(--cui-text-secondary);
    text-align: center;
  }

  p {
    margin: 0;
    max-width: 42ch;
  }
</style>
