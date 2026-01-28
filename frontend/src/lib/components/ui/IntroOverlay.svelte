<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";

  interface Props {
    showTagline?: boolean;
  }

  let { showTagline = false }: Props = $props();
</script>

<div class="intro-screen" transition:fade={{ duration: 800 }}>
  <!-- Subtle Ambient Background -->
  <div class="ambient-bg"></div>

  <!-- Main Content -->
  <div class="intro-content">
    <!-- Logo with Premium Glow and Floating Particles -->
    <div class="logo-wrapper">
      <!-- Floating Particles around glow -->
      <div class="particle-field">
        {#each Array(20) as _, i}
          <div
            class="glow-particle"
            style="--angle: {i * 18}deg; --delay: {i * 0.15}s; --distance: {80 +
              (i % 3) * 25}px; --size: {3 + (i % 4)}px; --duration: {2 +
              (i % 3)}s;"
          ></div>
        {/each}
      </div>

      <div class="logo-glow"></div>
      <div class="logo-inner">
        <img src="/images/fshare.png" alt="Flasharr" class="intro-logo" />
        <!-- Apple-style sweeping shine -->
        <div class="logo-shine"></div>
      </div>
    </div>

    <!-- Text Section - BELOW everything -->
    <div class="text-section">
      {#if !showTagline}
        <p class="intro-text">INITIALIZING SYSTEM...</p>
      {:else}
        <p class="intro-tagline">
          <span class="typing-text">Flasharr - Fshare × Arr Suite</span>
        </p>
      {/if}

      <!-- Minimal Loading Indicator -->
      <div class="loading-dots">
        <span></span>
        <span></span>
        <span></span>
      </div>
    </div>
  </div>
</div>

<style>
  .intro-screen {
    position: fixed;
    inset: 0;
    background: radial-gradient(ellipse at center, #0a1628 0%, #020408 100%);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
    overflow: hidden;
  }

  .ambient-bg {
    position: absolute;
    inset: 0;
    background: radial-gradient(
        circle at 20% 30%,
        rgba(0, 243, 255, 0.05) 0%,
        transparent 50%
      ),
      radial-gradient(
        circle at 80% 70%,
        rgba(138, 43, 226, 0.05) 0%,
        transparent 50%
      );
    filter: blur(60px);
  }

  .intro-content {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3rem;
    z-index: 10;
  }

  /* Logo Wrapper with Premium Glow */
  .logo-wrapper {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .logo-glow {
    position: absolute;
    width: 200px;
    height: 200px;
    background: radial-gradient(
      circle,
      rgba(0, 243, 255, 0.25) 0%,
      rgba(0, 180, 255, 0.1) 40%,
      transparent 70%
    );
    filter: blur(30px);
    animation: glow-breathe 3s ease-in-out infinite;
  }

  /* Floating Particle Field */
  .particle-field {
    position: absolute;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }

  .glow-particle {
    position: absolute;
    width: var(--size);
    height: var(--size);
    background: radial-gradient(
      circle,
      rgba(0, 243, 255, 1) 0%,
      rgba(0, 243, 255, 0.5) 40%,
      transparent 70%
    );
    border-radius: 50%;
    left: 50%;
    top: 50%;
    transform: rotate(var(--angle)) translateX(var(--distance)) translateY(0);
    animation: particle-float var(--duration) ease-in-out infinite;
    animation-delay: var(--delay);
    opacity: 0;
    box-shadow:
      0 0 6px rgba(0, 243, 255, 0.8),
      0 0 12px rgba(0, 243, 255, 0.4);
  }

  @keyframes particle-float {
    0% {
      opacity: 0;
      transform: rotate(var(--angle)) translateX(calc(var(--distance) * 0.5))
        translateY(0) scale(0.5);
    }
    20% {
      opacity: 1;
    }
    80% {
      opacity: 0.8;
    }
    100% {
      opacity: 0;
      transform: rotate(var(--angle)) translateX(calc(var(--distance) * 1.5))
        translateY(-20px) scale(1);
    }
  }

  /* Logo Inner Container */
  .logo-inner {
    position: relative;
    overflow: hidden;
    border-radius: 24px;
  }

  .intro-logo {
    position: relative;
    width: 120px;
    height: 120px;
    object-fit: contain;
    border-radius: 24px;
    animation: logo-appear 0.8s cubic-bezier(0.34, 1.56, 0.64, 1) forwards;
    filter: drop-shadow(0 0 2px rgba(0, 243, 255, 0.9))
      drop-shadow(0 0 15px rgba(0, 243, 255, 0.5))
      drop-shadow(0 0 30px rgba(0, 243, 255, 0.25));
  }

  /* Apple-style Sweeping Shine */
  .logo-shine {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      105deg,
      transparent 20%,
      rgba(255, 255, 255, 0) 40%,
      rgba(255, 255, 255, 0.4) 50%,
      rgba(255, 255, 255, 0) 60%,
      transparent 80%
    );
    transform: translateX(-100%);
    animation: shine-sweep 3s ease-in-out infinite;
    animation-delay: 1s;
    pointer-events: none;
  }

  @keyframes shine-sweep {
    0% {
      transform: translateX(-100%) skewX(-15deg);
    }
    30% {
      transform: translateX(100%) skewX(-15deg);
    }
    100% {
      transform: translateX(100%) skewX(-15deg);
    }
  }

  /* Text Section - Always Below Logo */
  .text-section {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1.5rem;
    min-height: 60px;
  }

  .intro-text {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 0.8rem;
    font-weight: 500;
    letter-spacing: 0.25em;
    color: var(--color-primary);
    text-shadow: 0 0 10px rgba(0, 243, 255, 0.5);
    animation: text-pulse 2s ease-in-out infinite;
  }

  .intro-tagline {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 0.95rem;
    font-weight: 400;
    letter-spacing: 0.08em;
    color: rgba(255, 255, 255, 0.9);
    text-shadow: 0 0 20px rgba(255, 255, 255, 0.2);
    animation: tagline-slide-in 0.4s ease-out forwards;
  }

  .typing-text {
    display: inline-block;
    overflow: hidden;
    white-space: nowrap;
    border-right: 2px solid var(--color-primary);
    width: 0;
    animation:
      type-in 0.8s steps(29) 0.1s forwards,
      cursor-blink 0.7s step-end infinite;
  }

  /* Loading Dots */
  .loading-dots {
    display: flex;
    gap: 6px;
  }

  .loading-dots span {
    width: 6px;
    height: 6px;
    background: var(--color-primary);
    border-radius: 50%;
    opacity: 0.3;
    animation: dot-pulse 1.4s ease-in-out infinite;
  }

  .loading-dots span:nth-child(2) {
    animation-delay: 0.2s;
  }

  .loading-dots span:nth-child(3) {
    animation-delay: 0.4s;
  }

  /* Keyframes */
  @keyframes logo-appear {
    0% {
      transform: scale(0.9);
      opacity: 0;
    }
    100% {
      transform: scale(1);
      opacity: 1;
    }
  }

  @keyframes glow-breathe {
    0%,
    100% {
      transform: scale(1);
      opacity: 0.7;
    }
    50% {
      transform: scale(1.15);
      opacity: 1;
    }
  }

  @keyframes text-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.6;
    }
  }

  @keyframes tagline-slide-in {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @keyframes type-in {
    from {
      width: 0;
    }
    to {
      width: 100%;
    }
  }

  @keyframes cursor-blink {
    50% {
      border-color: transparent;
    }
  }

  @keyframes dot-pulse {
    0%,
    80%,
    100% {
      opacity: 0.3;
      transform: scale(1);
    }
    40% {
      opacity: 1;
      transform: scale(1.2);
    }
  }
</style>
