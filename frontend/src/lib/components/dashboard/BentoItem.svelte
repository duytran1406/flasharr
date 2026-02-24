<script lang="ts">
  export let spanCols: number = 1;
  export let spanRows: number = 1;
  export let title: string = "";
  export let icon: string = "";
  export let href: string = "";
</script>

<svelte:element
  this={href ? "a" : "div"}
  class="bento-item"
  style="grid-column: span {spanCols}; grid-row: span {spanRows};"
  href={href || null}
>
  {#if title}
    <div class="bento-header">
      <div class="bento-title-group">
        {#if icon}
          <span class="material-icons icon">{icon}</span>
        {/if}
        <h3>{title}</h3>
      </div>
      {#if href}
        <span class="material-icons link-indicator">arrow_forward</span>
      {/if}
    </div>
  {/if}
  <div class="bento-content">
    <slot />
  </div>
</svelte:element>

<style>
  .bento-item {
    background: rgba(20, 20, 20, 0.4);
    backdrop-filter: blur(24px);
    -webkit-backdrop-filter: blur(24px);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 24px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    transition:
      transform 0.3s cubic-bezier(0.25, 0.8, 0.25, 1),
      border-color 0.3s ease,
      box-shadow 0.3s ease;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.2);
    /* Ensure anchor tag behaves like the div */
    text-decoration: none;
    color: inherit;
    box-sizing: border-box;
  }

  /* Link specific styles */
  a.bento-item {
    cursor: pointer;
  }

  .bento-item:hover {
    border-color: rgba(255, 255, 255, 0.15);
    transform: translateY(-4px);
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.3);
    background: rgba(30, 30, 30, 0.5);
  }

  .bento-header {
    padding: 1.25rem 1.5rem 0.5rem;
    display: flex;
    align-items: center;
    justify-content: space-between; /* Separate title group from link arrow */
  }

  .bento-title-group {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .bento-header .icon {
    font-size: 1.1rem;
    color: var(--color-primary);
  }

  .bento-header h3 {
    margin: 0;
    font-size: 0.85rem;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.6);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .link-indicator {
    font-size: 1rem;
    color: rgba(255, 255, 255, 0.3);
    transition:
      color 0.3s,
      transform 0.3s;
  }

  .bento-item:hover .link-indicator {
    color: var(--color-primary);
    transform: translateX(2px);
  }

  .bento-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    position: relative;
    overflow: hidden;
  }

  @media (max-width: 768px) {
    .bento-item {
      grid-column: span 1 !important;
      grid-row: auto !important;
      min-height: 200px;
    }
  }
</style>
