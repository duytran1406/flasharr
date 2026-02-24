<script lang="ts">
  export let gap: number = 24;
  export let cols: number = 4;
</script>

<div class="bento-grid" style="--gap: {gap}px; --cols: {cols};">
  <slot />
</div>

<style>
  .bento-grid {
    display: grid;
    grid-template-columns: repeat(var(--cols), 1fr);
    gap: var(--gap);
    width: 100%;
    margin: 0 auto;
    padding: 0;
  }

  @media (max-width: 1200px) {
    .bento-grid {
      grid-template-columns: repeat(2, 1fr);
    }
  }

  @media (max-width: 768px) {
    .bento-grid {
      grid-template-columns: 1fr;
      padding-bottom: 2rem;
    }
  }
</style>
