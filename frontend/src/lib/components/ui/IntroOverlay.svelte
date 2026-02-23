<script lang="ts">
  import { onMount } from "svelte";
  import anime from "animejs";
  import { ui } from "$lib/stores/ui.svelte";

  interface Props {
    checkConfig?: () => Promise<string>;
    onComplete?: (destination: string) => void;
  }
  let { checkConfig, onComplete }: Props = $props();

  let pendingDestination: string | null = null;

  /* ── DOM refs ── */
  let overlay: HTMLDivElement;
  let flashLayer: HTMLDivElement;
  let logoImg: HTMLImageElement;
  let chromaR: HTMLImageElement; // orange-red clone
  let chromaB: HTMLImageElement; // purple-blue clone
  let glowOrb: HTMLDivElement;
  let streakLine: HTMLDivElement;
  let titleWrap: HTMLDivElement;
  let titleEl: HTMLDivElement;
  let ringInner: HTMLDivElement;
  let ringOuter: HTMLDivElement;
  let scanLine: HTMLDivElement;
  let particleBox: HTMLDivElement;

  let idleAnims: ReturnType<typeof anime>[] = [];
  let idleRunning = false;

  const TITLE = "FLASHARR";
  const SUBTITLE = "Fshare × Arr Media Suite";
  const GLITCH_CHARS = "!<>-_\\/[]{}—=+*^?#_01";

  let activeIntervals: number[] = [];

  /* ── Scramble text decode ── */
  function scrambleDecode(el: HTMLElement, text: string, speed = 30) {
    let frame = 0;
    const id = setInterval(() => {
      el.textContent = text
        .split("")
        .map((ch, i) =>
          i < frame
            ? ch
            : GLITCH_CHARS[Math.floor(Math.random() * GLITCH_CHARS.length)],
        )
        .join("");
      if (frame >= text.length) {
        clearInterval(id);
        activeIntervals = activeIntervals.filter((i) => i !== (id as any));
      }
      frame += 1;
    }, speed);
    activeIntervals.push(id as any);
  }

  /* ═══════════════════════════════════════════════
     PHASE 1 — APPEAR  (Cinema-grade logo reveal)
     Total: ~2.4s
  ═══════════════════════════════════════════════ */
  function playAppear() {
    const tl = anime.timeline({ autoplay: true });

    /* ─── Act 1: Darkness & anticipation (0–800ms) ─── */

    // A thin horizontal light streak races across the screen left→right
    tl.add(
      {
        targets: streakLine,
        translateX: ["-110vw", "110vw"],
        opacity: [0, 1, 1, 0],
        scaleY: [0.5, 1, 1, 0.5],
        duration: 700,
        easing: "easeInOutQuart",
      },
      100,
    );

    // As streak passes center, a soft glow orb blooms behind where logo will be
    tl.add(
      {
        targets: glowOrb,
        scale: [0, 1.3],
        opacity: [0, 0.8],
        duration: 400,
        easing: "easeOutQuad",
      },
      300,
    );

    /* ─── Impact fires right as streak crosses center ─── */

    // Hard white flash — very brief
    tl.add(
      {
        targets: flashLayer,
        opacity: [0, 0.9, 0],
        duration: 180,
        easing: "easeOutQuad",
      },
      500,
    );

    // Inner ring expands
    tl.add(
      {
        targets: ringInner,
        scale: [0, 1],
        opacity: [0.8, 0],
        duration: 600,
        easing: "easeOutCubic",
      },
      520,
    );

    // Outer ring expands (staggered)
    tl.add(
      {
        targets: ringOuter,
        scale: [0, 1],
        opacity: [0.5, 0],
        duration: 800,
        easing: "easeOutCubic",
      },
      550,
    );

    // VIBRANT GLITCH ENTRY (Sync with Logo Slam)
    tl.add(
      {
        targets: [chromaR, chromaB],
        opacity: [0, 1, 0.4, 0.8, 0],
        translateX: () =>
          (Math.random() > 0.5 ? 1 : -1) * (Math.random() * 15 + 5),
        duration: 450,
        easing: "steps(8)",
      },
      500,
    );

    // Violent Logo Slam / White Glitch Entry (logo appears here, no fade-in)
    tl.add(
      {
        targets: logoImg,
        opacity: [0, 1],
        scale: [1.6, 1],
        filter: [
          "brightness(30) blur(20px)",
          "brightness(8) blur(10px)",
          "brightness(1) blur(0px) drop-shadow(0 0 30px rgba(0,243,255,0.8))",
        ],
        duration: 500,
        easing: "easeOutExpo",
        begin() {
          if (logoImg) logoImg.style.visibility = "visible";
        },
      },
      500,
    );

    // Subtle horizontal screen shake on impact
    tl.add(
      {
        targets: overlay,
        translateX: [
          { value: 12, duration: 30 },
          { value: -12, duration: 30 },
          { value: 8, duration: 30 },
          { value: -6, duration: 30 },
          { value: 0, duration: 30 },
        ],
        easing: "steps(1)",
      },
      500,
    );

    // Glow orb settles to idle state
    tl.add(
      {
        targets: glowOrb,
        scale: [1.3, 1],
        opacity: [0.8, 0.5],
        duration: 600,
        easing: "easeOutQuad",
      },
      600,
    );

    /* ─── Act 4: Title reveal (1800–2400ms) ─── */

    // Start continuous particle emitter after impact
    tl.add(
      {
        targets: particleBox,
        opacity: [0, 1],
        duration: 200,
        easing: "linear",
        begin() {
          startParticleEmitter();
        },
      },
      600,
    );

    // Title container slides up and fades in
    tl.add(
      {
        targets: titleWrap,
        opacity: [0, 1],
        translateY: [20, 0],
        duration: 500,
        easing: "easeOutCubic",
        begin() {
          if (titleEl) scrambleDecode(titleEl, TITLE, 40);
        },
      },
      1000,
    );

    tl.finished.then(() => playIdle());
  }

  /* ── Continuous particle emitter ── */
  function startParticleEmitter() {
    if (!particleBox) return;
    // Emit one particle every ~16ms for a dense, continuous stream (5x rate)
    const emitInterval = setInterval(() => {
      if (!particleBox) {
        clearInterval(emitInterval);
        return;
      }
      const p = document.createElement("div");
      p.className = "particle";
      const angle = Math.random() * Math.PI * 2;
      const dist = 80 + Math.random() * 300;
      const dur = 1000 + Math.random() * 800;
      p.style.setProperty("--tx", `${Math.cos(angle) * dist}px`);
      p.style.setProperty("--ty", `${Math.sin(angle) * dist}px`);
      p.style.setProperty("--delay", "0ms");
      p.style.setProperty("--dur", `${dur}ms`);
      p.style.setProperty("--size", `${1 + Math.random() * 3}px`);
      particleBox.appendChild(p);
      // Self-remove after animation completes to keep DOM clean
      setTimeout(() => p.remove(), dur + 50);
    }, 16);
    activeIntervals.push(emitInterval as any);
  }

  /* ═══════════════════════════════════════════════
     PHASE 2 — IDLE  (Subtle breathing)
  ═══════════════════════════════════════════════ */
  function playIdle() {
    idleRunning = true;

    // Gentle logo breathing glow
    idleAnims.push(
      anime({
        targets: logoImg,
        filter: [
          "drop-shadow(0 0 20px rgba(0,243,255,0.7)) drop-shadow(0 0 60px rgba(0,243,255,0.3))",
          "drop-shadow(0 0 30px rgba(0,243,255,0.9)) drop-shadow(0 0 80px rgba(0,243,255,0.5))",
        ],
        duration: 2000,
        direction: "alternate",
        loop: true,
        easing: "easeInOutSine",
      }),
    );

    // Very subtle scale breathe
    idleAnims.push(
      anime({
        targets: logoImg,
        scale: [1, 1.015],
        duration: 2500,
        direction: "alternate",
        loop: true,
        easing: "easeInOutSine",
      }),
    );

    // Background glow pulses
    idleAnims.push(
      anime({
        targets: glowOrb,
        scale: [1, 1.1],
        opacity: [0.5, 0.65],
        duration: 3000,
        direction: "alternate",
        loop: true,
        easing: "easeInOutSine",
      }),
    );

    // Periodic scan line sweep
    idleAnims.push(
      anime({
        targets: scanLine,
        top: ["-5%", "105%"],
        opacity: [0, 0.6, 0.6, 0],
        duration: 1500,
        delay: 4000,
        loop: true,
        easing: "linear",
      }),
    );

    // Particle emitter is already running continuously from Appear phase

    if (checkConfig) {
      checkConfig()
        .then((dest) => {
          pendingDestination = dest;
          if (idleRunning) playDisappear();
        })
        .catch(() => {
          pendingDestination = "/";
          if (idleRunning) playDisappear();
        });
    }
  }

  /* ═══════════════════════════════════════════════
     PHASE 3 — DISAPPEAR (Terminal Breach & Shatter)
  ═══════════════════════════════════════════════ */
  function playDisappear() {
    // 1. Kill ALL conflicting animations and intervals IMMEDIATELY
    idleRunning = false;
    idleAnims.forEach((a) => {
      try {
        a.pause();
      } catch (_) {}
    });
    idleAnims = [];
    activeIntervals.forEach((id) => clearInterval(id));
    activeIntervals = [];

    // ─── Helper: random int in range ───
    const rand = (min: number, max: number) =>
      Math.floor(Math.random() * (max - min + 1)) + min;

    // ─── 2. THE OVERLOAD (400ms of per-frame chaos) ───
    // We use a raw rAF loop so every single frame gets NEW random values.
    // anime.js function-based values only evaluate once — this is real chaos.
    const OVERLOAD_MS = 400;
    const glitchTargets = [logoImg, titleWrap, chromaR, chromaB].filter(
      Boolean,
    );
    const startTime = performance.now();

    // Build array of filter presets to cycle through randomly
    const glitchFilters = [
      "contrast(3) brightness(5) drop-shadow(-15px 0 0 #ff00ff) drop-shadow(15px 0 0 #00ffff)",
      "contrast(2) brightness(8) drop-shadow(12px 0 0 #ff00ff) drop-shadow(-12px 0 0 #00ffff)",
      "contrast(4) brightness(3) drop-shadow(-20px 0 0 #ff00ff) drop-shadow(20px 0 0 #00ffff)",
      "contrast(1.5) brightness(10) drop-shadow(8px 0 0 #ff00ff) drop-shadow(-8px 0 0 #00ffff)",
      "contrast(5) brightness(2) drop-shadow(-10px 0 0 #ff00ff) drop-shadow(25px 0 0 #00ffff)",
      "contrast(2) brightness(6) drop-shadow(18px 0 0 #ff00ff) drop-shadow(-5px 0 0 #00ffff)",
    ];

    // Make chroma clones visible immediately
    if (chromaR) chromaR.style.opacity = "1";
    if (chromaB) chromaB.style.opacity = "1";

    function glitchFrame(now: number) {
      const elapsed = now - startTime;
      if (elapsed >= OVERLOAD_MS) {
        // Clean up inline styles after overload
        glitchTargets.forEach((el) => {
          if (el) el.style.cssText = "";
        });
        return; // stop the loop
      }

      // ── Per-frame random transforms on logo ──
      if (logoImg) {
        const tx = rand(-45, 45);
        const ty = rand(-20, 20);
        const sy = (rand(70, 140) / 100).toFixed(2);
        const skx = rand(-20, 20);
        const f = glitchFilters[rand(0, glitchFilters.length - 1)];
        logoImg.style.transform = `translateX(${tx}px) translateY(${ty}px) scaleY(${sy}) skewX(${skx}deg)`;
        logoImg.style.filter = f;
      }

      // ── Per-frame random transforms on title ──
      if (titleWrap) {
        const tx = rand(-30, 30);
        const ty = rand(-15, 15);
        const skx = rand(-12, 12);
        titleWrap.style.transform = `translateX(${tx}px) translateY(${ty}px) skewX(${skx}deg)`;
        titleWrap.style.filter = `brightness(${rand(2, 8)}) contrast(${rand(1, 4)})`;
      }

      // ── Chromatic clones jitter in opposite directions ──
      if (chromaR) {
        const offset = rand(-30, -5);
        chromaR.style.transform = `translateX(${offset}px)`;
        chromaR.style.opacity = String(Math.random() > 0.3 ? 1 : 0.4);
      }
      if (chromaB) {
        const offset = rand(5, 30);
        chromaB.style.transform = `translateX(${offset}px)`;
        chromaB.style.opacity = String(Math.random() > 0.3 ? 1 : 0.4);
      }

      requestAnimationFrame(glitchFrame);
    }
    requestAnimationFrame(glitchFrame);

    // ─── 3. THE WIPE (overlay opacity stutter + cleanup) ───
    // This runs concurrently via anime.js while the rAF loop does the chaos
    const tl = anime.timeline({ autoplay: true });

    // Master container violently stutters down: 1 → 0.2 → 0.9 → 0
    tl.add(
      {
        targets: overlay,
        opacity: [1, 0.2, 0.9, 0.15, 0.8, 0],
        duration: OVERLOAD_MS,
        easing: "steps(6)",
        complete() {
          if (overlay) overlay.style.display = "none";
          document.body.style.overflow = "";
          ui.finishIntro();
          onComplete?.(pendingDestination ?? "/");
        },
      },
      0,
    );

    // Rings, scan-lines, particles pop out of existence instantly
    tl.add(
      {
        targets: [
          ringInner,
          ringOuter,
          scanLine,
          streakLine,
          glowOrb,
          particleBox,
        ],
        opacity: 0,
        scale: 1.5,
        duration: 150,
        easing: "easeOutExpo",
      },
      0,
    );
  }

  /* ═══════════════════════════════════════════════
     MOUNT
  ═══════════════════════════════════════════════ */
  onMount(() => {
    document.body.style.overflow = "hidden";

    // Set initial states
    if (logoImg) logoImg.style.visibility = "hidden";
    anime.set(logoImg, { opacity: 0, scale: 1.15 });
    anime.set(flashLayer, { opacity: 0 });
    anime.set(glowOrb, { opacity: 0, scale: 0 });
    anime.set(streakLine, { opacity: 0 });
    anime.set(titleWrap, { opacity: 0 });
    anime.set([ringInner, ringOuter], { scale: 0, opacity: 0 });
    anime.set(scanLine, { opacity: 0 });
    anime.set(particleBox, { opacity: 0 });
    anime.set([chromaR, chromaB], { opacity: 0 });

    playAppear();

    return () => {
      document.body.style.overflow = "";
      idleAnims.forEach((a) => {
        try {
          a.pause();
        } catch (_) {}
      });
    };
  });
</script>

<!-- ═══════════════════════════════════════
     MARKUP — Simple, logo-centric layers
═══════════════════════════════════════ -->
<div class="intro-overlay" bind:this={overlay}>
  <!-- Deep space background with subtle grid -->
  <div class="bg-void"></div>

  <!-- Glow orb sits behind the logo -->
  <div class="glow-orb" bind:this={glowOrb}></div>

  <!-- Expanding impact rings -->
  <div class="ring ring-inner" bind:this={ringInner}></div>
  <div class="ring ring-outer" bind:this={ringOuter}></div>

  <!-- Horizontal light streak -->
  <div class="streak-line" bind:this={streakLine}></div>

  <!-- Scan line for idle -->
  <div class="scan-line" bind:this={scanLine}></div>

  <!-- Particle container -->
  <div class="particle-box" bind:this={particleBox}></div>

  <!-- ★ THE HERO: Logo ★ -->
  <div class="logo-wrap">
    <!-- Chromatic aberration clones (behind main logo) -->
    <img
      src="/images/flasharr_logo.png"
      alt=""
      class="chroma-clone chroma-r"
      bind:this={chromaR}
    />
    <img
      src="/images/flasharr_logo.png"
      alt=""
      class="chroma-clone chroma-b"
      bind:this={chromaB}
    />
    <!-- Main logo -->
    <img
      src="/images/flasharr_logo.png"
      alt="Flasharr"
      class="hero-logo"
      bind:this={logoImg}
    />
  </div>

  <!-- Title -->
  <div class="title-wrap" bind:this={titleWrap}>
    <div class="title-main" bind:this={titleEl}></div>
    <div class="title-sub">{SUBTITLE}</div>
    <div class="title-rule"></div>
  </div>

  <!-- Flash overlay -->
  <div class="flash-layer" bind:this={flashLayer}></div>
</div>

<style>
  /* ════════════ Base ════════════ */
  .intro-overlay {
    position: fixed;
    inset: 0;
    z-index: 9999;
    overflow: hidden;
    background: #020509;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  /* Subtle depth grid */
  .bg-void {
    position: absolute;
    inset: 0;
    background: radial-gradient(
        ellipse at 50% 50%,
        rgba(0, 40, 60, 0.25) 0%,
        transparent 70%
      ),
      linear-gradient(rgba(0, 243, 255, 0.015) 1px, transparent 1px),
      linear-gradient(90deg, rgba(0, 243, 255, 0.015) 1px, transparent 1px);
    background-size:
      100% 100%,
      60px 60px,
      60px 60px;
  }

  /* ════════════ Glow Orb ════════════ */
  .glow-orb {
    position: absolute;
    inset: 0;
    margin: auto;
    width: min(80vmin, 700px);
    height: min(80vmin, 700px);
    border-radius: 50%;
    background: radial-gradient(
      circle,
      rgba(0, 243, 255, 0.25) 0%,
      rgba(0, 150, 200, 0.08) 40%,
      transparent 70%
    );
    filter: blur(40px);
    pointer-events: none;
  }

  /* ════════════ Impact Rings ════════════ */
  .ring {
    position: absolute;
    inset: 0;
    margin: auto;
    border-radius: 50%;
    border: 2px solid rgba(0, 243, 255, 0.6);
    pointer-events: none;
  }
  .ring-inner {
    width: min(60vmin, 500px);
    height: min(60vmin, 500px);
    border-width: 2px;
    box-shadow:
      0 0 20px rgba(0, 243, 255, 0.4),
      inset 0 0 20px rgba(0, 243, 255, 0.1);
  }
  .ring-outer {
    width: min(90vmin, 800px);
    height: min(90vmin, 800px);
    border-width: 1px;
    border-color: rgba(0, 243, 255, 0.3);
    box-shadow: 0 0 30px rgba(0, 243, 255, 0.2);
  }

  /* ════════════ Light Streak ════════════ */
  .streak-line {
    position: absolute;
    top: 50%;
    left: 0;
    width: 40vw;
    height: 2px;
    transform: translateY(-50%);
    background: linear-gradient(
      90deg,
      transparent,
      rgba(0, 243, 255, 0.3),
      rgba(0, 243, 255, 0.8),
      #fff,
      rgba(0, 243, 255, 0.8),
      rgba(0, 243, 255, 0.3),
      transparent
    );
    box-shadow:
      0 0 15px rgba(0, 243, 255, 0.6),
      0 0 40px rgba(0, 243, 255, 0.3);
    pointer-events: none;
  }

  /* ════════════ Scan Line (idle) ════════════ */
  .scan-line {
    position: absolute;
    left: 30%;
    right: 30%;
    height: 1px;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(0, 243, 255, 0.5),
      transparent
    );
    box-shadow: 0 0 8px rgba(0, 243, 255, 0.4);
    pointer-events: none;
    opacity: 0;
  }

  /* ════════════ Particles ════════════ */
  .particle-box {
    position: absolute;
    inset: 0;
    margin: auto;
    width: 0;
    height: 0;
    pointer-events: none;
  }
  .particle-box :global(.particle) {
    position: absolute;
    width: var(--size, 2px);
    height: var(--size, 2px);
    border-radius: 50%;
    background: #00f3ff;
    box-shadow:
      0 0 6px #00f3ff,
      0 0 12px rgba(0, 243, 255, 0.4);
    animation: particleFly var(--dur, 1s) var(--delay, 0ms) ease-out forwards;
  }
  @keyframes particleFly {
    0% {
      transform: translate(0, 0) scale(1);
      opacity: 1;
    }
    70% {
      opacity: 0.6;
    }
    100% {
      transform: translate(var(--tx), var(--ty)) scale(0.3);
      opacity: 0;
    }
  }

  /* ════════════ HERO LOGO ════════════ */
  .logo-wrap {
    position: absolute;
    inset: 0;
    margin: auto;
    z-index: 10;
    width: min(55vmin, 420px);
    height: min(55vmin, 420px);
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }
  .hero-logo {
    position: relative;
    z-index: 2;
    width: 100%;
    height: 100%;
    object-fit: contain;
    will-change: transform, filter, opacity;
  }
  .chroma-clone {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    opacity: 0;
    pointer-events: none;
    z-index: 3;
    mix-blend-mode: screen;
    will-change: transform, opacity;
  }
  .chroma-r {
    filter: brightness(2) sepia(1) hue-rotate(280deg) saturate(10);
    /* Neon Pink-Purple */
    mix-blend-mode: screen;
  }
  .chroma-b {
    filter: brightness(1.8) sepia(1) hue-rotate(180deg) saturate(8);
    /* Neon Cyan */
    mix-blend-mode: screen;
  }

  /* ════════════ Title ════════════ */
  .title-wrap {
    position: absolute;
    top: calc(50% + min(30vmin, 240px));
    left: 0;
    right: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    z-index: 10;
    text-align: center;
  }
  .title-main {
    font-family: "JetBrains Mono", "SF Mono", "Fira Code", monospace;
    font-size: clamp(1.2rem, 3vw, 2rem);
    font-weight: 800;
    letter-spacing: 0.35em;
    margin-right: -0.35em; /* CANCELS the trailing letter-spacing gap for visual centering */
    color: #fff;
    text-align: center;
    text-shadow:
      0 0 20px rgba(0, 243, 255, 0.8),
      0 0 40px rgba(0, 243, 255, 0.4);
  }
  .title-sub {
    font-family: "Inter", "Segoe UI", sans-serif;
    font-size: clamp(0.6rem, 1.2vw, 0.85rem);
    font-weight: 400;
    letter-spacing: 0.25em;
    margin-right: -0.25em; /* CANCELS the trailing letter-spacing */
    color: rgba(225, 225, 230, 0.6);
    text-transform: uppercase;
    text-align: center;
  }
  .title-rule {
    width: min(250px, 50vw);
    height: 1px;
    margin-top: 4px;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(0, 243, 255, 0.5),
      transparent
    );
  }

  /* ════════════ Flash ════════════ */
  .flash-layer {
    position: absolute;
    inset: 0;
    background: #fff;
    z-index: 100;
    pointer-events: none;
  }
</style>
