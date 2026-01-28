<script lang="ts">
  interface Props {
    mode?: "poster" | "banner";
    count?: number;
  }

  let { mode = "poster", count = 1 }: Props = $props();
</script>

{#each Array(count) as _, i}
  <div class="skeleton-card mode-{mode}" style="animation-delay: {i * 0.1}s">
    <div class="skeleton-image"></div>
    <div class="skeleton-content">
      <div class="skeleton-title"></div>
      <div class="skeleton-meta">
        <div class="skeleton-rating"></div>
        <div class="skeleton-year"></div>
      </div>
    </div>
  </div>
{/each}

<style>
  .skeleton-card {
    position: relative;
    border-radius: 16px;
    overflow: hidden;
    background: var(--surface-secondary, #1a1a2e);
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .skeleton-card.mode-poster {
    aspect-ratio: 2/3;
  }

  .skeleton-card.mode-banner {
    aspect-ratio: 16/9;
  }

  .skeleton-image {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      110deg,
      rgba(255, 255, 255, 0.02) 0%,
      rgba(255, 255, 255, 0.05) 50%,
      rgba(255, 255, 255, 0.02) 100%
    );
    background-size: 200% 100%;
    animation: shimmer 1.5s infinite;
  }

  .skeleton-content {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    padding: 1rem;
    background: linear-gradient(
      to top,
      rgba(0, 0, 0, 0.8) 0%,
      transparent 100%
    );
  }

  .skeleton-title {
    width: 80%;
    height: 16px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.1);
    margin-bottom: 0.5rem;
    animation: pulse 1.5s ease-in-out infinite;
  }

  .skeleton-meta {
    display: flex;
    gap: 0.5rem;
  }

  .skeleton-rating {
    width: 50px;
    height: 12px;
    border-radius: 3px;
    background: rgba(255, 215, 0, 0.2);
    animation: pulse 1.5s ease-in-out infinite;
    animation-delay: 0.1s;
  }

  .skeleton-year {
    width: 40px;
    height: 12px;
    border-radius: 3px;
    background: rgba(0, 243, 255, 0.15);
    animation: pulse 1.5s ease-in-out infinite;
    animation-delay: 0.2s;
  }

  @keyframes shimmer {
    0% {
      background-position: 200% 0;
    }
    100% {
      background-position: -200% 0;
    }
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 0.4;
    }
    50% {
      opacity: 0.8;
    }
  }
</style>
