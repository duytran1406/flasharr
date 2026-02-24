/**
 * Flasharr Animation System — Powered by anime.js
 *
 * Provides Svelte actions, transition replacements, and utilities
 * for a unified animation layer across the entire application.
 */

import anime from "animejs";
import type { Action } from "svelte/action";

// Re-export anime for direct use where needed
export { anime };

// ============================================================================
// Svelte Transition Replacements (drop-in for svelte/transition)
// ============================================================================

/**
 * Custom Svelte transition using anime.js — replaces `fade`
 * Usage: <div transition:animeFade={{ duration: 300 }}>
 */
export function animeFade(
  node: HTMLElement,
  { duration = 300, delay = 0, easing = "easeOutQuad" } = {},
) {
  return {
    duration: duration + delay,
    tick(t: number, u: number) {
      // t goes 0→1 on intro, u goes 1→0 on intro
      node.style.opacity = String(t);
    },
  };
}

/**
 * Custom Svelte transition using anime.js — replaces `fly`
 * Usage: <div transition:animeFly={{ y: 20, duration: 400 }}>
 */
export function animeFly(
  node: HTMLElement,
  {
    y = 0,
    x = 0,
    duration = 400,
    delay = 0,
    easing = "easeOutCubic",
  }: {
    y?: number;
    x?: number;
    duration?: number;
    delay?: number;
    easing?: string;
  } = {},
) {
  const style = getComputedStyle(node);
  const opacity = +style.opacity;

  return {
    duration: duration + delay,
    css(t: number) {
      const easedT = easeOutCubic(Math.max(0, (t * (duration + delay) - delay) / duration));
      return `
        opacity: ${easedT * opacity};
        transform: translate(${(1 - easedT) * x}px, ${(1 - easedT) * y}px);
      `;
    },
  };
}

/**
 * Custom Svelte transition — slide + fade with spring feel
 * Usage: <div transition:animeSlide={{ y: 30, duration: 500 }}>
 */
export function animeSlide(
  node: HTMLElement,
  {
    y = 30,
    x = 0,
    duration = 500,
    delay = 0,
    scale = 0.97,
  }: {
    y?: number;
    x?: number;
    duration?: number;
    delay?: number;
    scale?: number;
  } = {},
) {
  return {
    duration: duration + delay,
    css(t: number) {
      const easedT = easeOutCubic(Math.max(0, (t * (duration + delay) - delay) / duration));
      const s = scale + (1 - scale) * easedT;
      return `
        opacity: ${easedT};
        transform: translate(${(1 - easedT) * x}px, ${(1 - easedT) * y}px) scale(${s});
      `;
    },
  };
}

/**
 * Pop-in transition with elastic feel
 */
export function animePop(
  node: HTMLElement,
  { duration = 500, delay = 0, scale = 0.8 } = {},
) {
  return {
    duration: duration + delay,
    css(t: number) {
      const raw = Math.max(0, (t * (duration + delay) - delay) / duration);
      const easedT = easeOutBack(raw);
      const s = scale + (1 - scale) * easedT;
      return `
        opacity: ${Math.min(1, raw * 2)};
        transform: scale(${s});
      `;
    },
  };
}

/**
 * Slide down/up transition — replaces Svelte's `slide`
 * Animates height from 0 to natural height with opacity.
 * Usage: <div transition:animeSlideDown={{ duration: 300 }}>
 */
export function animeSlideDown(
  node: HTMLElement,
  { duration = 300, delay = 0 }: { duration?: number; delay?: number } = {},
) {
  const style = getComputedStyle(node);
  const opacity = +style.opacity;
  const height = parseFloat(style.height);
  const paddingTop = parseFloat(style.paddingTop);
  const paddingBottom = parseFloat(style.paddingBottom);
  const marginTop = parseFloat(style.marginTop);
  const marginBottom = parseFloat(style.marginBottom);

  return {
    duration: duration + delay,
    css(t: number) {
      const easedT = easeOutCubic(Math.max(0, (t * (duration + delay) - delay) / duration));
      return `
        overflow: hidden;
        opacity: ${easedT * opacity};
        height: ${easedT * height}px;
        padding-top: ${easedT * paddingTop}px;
        padding-bottom: ${easedT * paddingBottom}px;
        margin-top: ${easedT * marginTop}px;
        margin-bottom: ${easedT * marginBottom}px;
      `;
    },
  };
}

// ============================================================================
// Svelte Actions — use:directive for imperative animations
// ============================================================================

/**
 * Staggered entrance animation for list/grid children
 * Usage: <div use:stagger={{ delay: 60, y: 20 }}>
 *
 * Animates all direct children with staggered timing.
 */
export const stagger: Action<
  HTMLElement,
  { delay?: number; y?: number; x?: number; duration?: number; scale?: number; opacity?: number[] }
> = (
  node,
  params = {},
) => {
  const {
    delay = 60,
    y = 20,
    x = 0,
    duration = 500,
    scale = 1,
    opacity = [0, 1],
  } = params ?? {};

  const children = node.children;
  if (children.length === 0) return;

  // Set initial state
  for (const child of children) {
    (child as HTMLElement).style.opacity = String(opacity[0]);
    (child as HTMLElement).style.transform = `translate(${x}px, ${y}px) scale(${scale})`;
  }

  // Use IntersectionObserver for viewport-aware triggering
  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          anime({
            targets: children,
            opacity: opacity[1],
            translateY: [y, 0],
            translateX: [x, 0],
            scale: [scale, 1],
            duration,
            delay: anime.stagger(delay),
            easing: "easeOutQuad",
          });
          observer.disconnect();
        }
      }
    },
    { threshold: 0.1 },
  );

  observer.observe(node);

  return {
    destroy() {
      observer.disconnect();
    },
  };
};

/**
 * Count-up animation for numeric values
 * Usage: <span use:countUp={1234}>0</span>
 */
export const countUp: Action<HTMLElement, number | { value: number; duration?: number; decimals?: number }> = (
  node,
  params,
) => {
  const { value, duration, decimals } =
    typeof params === "number"
      ? { value: params, duration: 1200, decimals: 0 }
      : { value: params?.value ?? 0, duration: params?.duration ?? 1200, decimals: params?.decimals ?? 0 };

  const obj = { val: 0 };

  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          anime({
            targets: obj,
            val: value,
            round: decimals === 0 ? 1 : false,
            duration,
            easing: "easeOutExpo",
            update() {
              node.textContent = decimals > 0 ? obj.val.toFixed(decimals) : Math.round(obj.val).toString();
            },
          });
          observer.disconnect();
        }
      }
    },
    { threshold: 0.1 },
  );

  observer.observe(node);

  return {
    update(newParams) {
      const newVal = typeof newParams === "number" ? newParams : newParams?.value ?? 0;
      const newDec = typeof newParams === "number" ? 0 : newParams?.decimals ?? 0;
      anime({
        targets: obj,
        val: newVal,
        round: newDec === 0 ? 1 : false,
        duration: 600,
        easing: "easeOutQuad",
        update() {
          node.textContent = newDec > 0 ? obj.val.toFixed(newDec) : Math.round(obj.val).toString();
        },
      });
    },
    destroy() {
      observer.disconnect();
    },
  };
};

/**
 * Morph a displayed value smoothly (for speeds, percentages, etc.)
 * Usage: <span use:morphValue={{ value: speed, suffix: ' MB/s' }}>0</span>
 */
export const morphValue: Action<
  HTMLElement,
  { value: number; suffix?: string; decimals?: number; duration?: number }
> = (node, params) => {
  const obj = { val: params?.value ?? 0 };
  const suffix = params?.suffix ?? "";
  const decimals = params?.decimals ?? 1;
  node.textContent = obj.val.toFixed(decimals) + suffix;

  return {
    update(newParams) {
      const target = newParams?.value ?? 0;
      const newSuffix = newParams?.suffix ?? "";
      const newDec = newParams?.decimals ?? 1;
      anime({
        targets: obj,
        val: target,
        duration: newParams?.duration ?? 400,
        easing: "easeOutQuad",
        update() {
          node.textContent = obj.val.toFixed(newDec) + newSuffix;
        },
      });
    },
  };
};

/**
 * Pulse glow effect for status indicators
 * Usage: <div use:pulseGlow={{ color: '#00ffa3' }}>
 */
export const pulseGlow: Action<HTMLElement, { color?: string; duration?: number; loop?: boolean }> = (
  node,
  params,
) => {
  const color = params?.color ?? "#a78bfa";
  const duration = params?.duration ?? 2000;

  const a = anime({
    targets: node,
    boxShadow: [
      `0 0 0px ${color}00`,
      `0 0 12px ${color}66`,
      `0 0 0px ${color}00`,
    ],
    duration,
    loop: params?.loop !== false,
    easing: "easeInOutSine",
  });

  return {
    update(newParams) {
      const newColor = newParams?.color ?? "#a78bfa";
      a.pause();
      anime({
        targets: node,
        boxShadow: [
          `0 0 0px ${newColor}00`,
          `0 0 12px ${newColor}66`,
          `0 0 0px ${newColor}00`,
        ],
        duration: newParams?.duration ?? 2000,
        loop: newParams?.loop !== false,
        easing: "easeInOutSine",
      });
    },
    destroy() {
      a.pause();
    },
  };
};

/**
 * Animate element entrance when it scrolls into view
 * Usage: <div use:revealOnScroll={{ y: 30 }}>
 */
export const revealOnScroll: Action<
  HTMLElement,
  { y?: number; x?: number; duration?: number; delay?: number; scale?: number }
> = (node, params) => {
  const y = params?.y ?? 30;
  const x = params?.x ?? 0;
  const duration = params?.duration ?? 600;
  const delay = params?.delay ?? 0;
  const scale = params?.scale ?? 1;

  node.style.opacity = "0";
  node.style.transform = `translate(${x}px, ${y}px) scale(${scale})`;

  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          anime({
            targets: node,
            opacity: [0, 1],
            translateY: [y, 0],
            translateX: [x, 0],
            scale: [scale, 1],
            duration,
            delay,
            easing: "easeOutCubic",
          });
          observer.disconnect();
        }
      }
    },
    { threshold: 0.1 },
  );

  observer.observe(node);

  return {
    destroy() {
      observer.disconnect();
    },
  };
};

/**
 * Animate SVG stroke-dashoffset (for progress rings)
 * Usage: <circle use:animateRing={{ progress: 75 }}>
 */
export const animateRing: Action<SVGCircleElement, { progress: number; duration?: number }> = (
  node,
  params,
) => {
  const circumference = 2 * Math.PI * (parseFloat(node.getAttribute("r") || "20"));
  node.style.strokeDasharray = String(circumference);
  node.style.strokeDashoffset = String(circumference);

  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          const target = circumference - (circumference * (params?.progress ?? 0)) / 100;
          anime({
            targets: node,
            strokeDashoffset: [circumference, target],
            duration: params?.duration ?? 1500,
            easing: "easeInOutQuart",
          });
          observer.disconnect();
        }
      }
    },
    { threshold: 0.1 },
  );

  observer.observe(node);

  return {
    update(newParams) {
      const target = circumference - (circumference * (newParams?.progress ?? 0)) / 100;
      anime({
        targets: node,
        strokeDashoffset: target,
        duration: 800,
        easing: "easeOutQuad",
      });
    },
    destroy() {
      observer.disconnect();
    },
  };
};

/**
 * Shimmer loading effect
 * Usage: <div use:shimmer>Loading...</div>
 */
export const shimmer: Action<HTMLElement, { duration?: number }> = (node, params) => {
  const duration = params?.duration ?? 1500;
  node.style.position = "relative";
  node.style.overflow = "hidden";

  const overlay = document.createElement("div");
  overlay.style.cssText = `
    position: absolute; inset: 0;
    background: linear-gradient(90deg, transparent 0%, rgba(255,255,255,0.04) 50%, transparent 100%);
    transform: translateX(-100%);
  `;
  node.appendChild(overlay);

  const a = anime({
    targets: overlay,
    translateX: ["-100%", "100%"],
    duration,
    loop: true,
    easing: "linear",
  });

  return {
    destroy() {
      a.pause();
      overlay.remove();
    },
  };
};

// ============================================================================
// Utility: Page Transition Helper
// ============================================================================

/**
 * Animate page content entrance (call from onMount)
 * Targets common section elements within a container.
 */
export function animatePageEntrance(container: HTMLElement, opts?: { delay?: number; staggerDelay?: number }) {
  const delay = opts?.delay ?? 0;
  const staggerDelay = opts?.staggerDelay ?? 80;

  const sections = container.querySelectorAll(
    "section, .card, .widget, .status-card-v5, .bento-item, [data-animate]",
  );

  if (sections.length === 0) return;

  // Set initial state
  sections.forEach((el) => {
    (el as HTMLElement).style.opacity = "0";
    (el as HTMLElement).style.transform = "translateY(20px)";
  });

  anime({
    targets: sections,
    opacity: [0, 1],
    translateY: [20, 0],
    duration: 500,
    delay: anime.stagger(staggerDelay, { start: delay }),
    easing: "easeOutQuad",
  });
}

/**
 * Animate a list of items with stagger
 */
export function animateList(
  targets: HTMLElement | NodeListOf<Element> | Element[],
  opts?: { delay?: number; stagger?: number; y?: number; duration?: number },
) {
  const y = opts?.y ?? 15;
  anime({
    targets,
    opacity: [0, 1],
    translateY: [y, 0],
    duration: opts?.duration ?? 400,
    delay: anime.stagger(opts?.stagger ?? 50, { start: opts?.delay ?? 0 }),
    easing: "easeOutQuad",
  });
}

// ============================================================================
// Easing helpers (used for CSS-based transitions)
// ============================================================================

function easeOutCubic(t: number): number {
  return 1 - Math.pow(1 - Math.min(1, Math.max(0, t)), 3);
}

function easeOutBack(t: number): number {
  const c1 = 1.70158;
  const c3 = c1 + 1;
  const clamped = Math.min(1, Math.max(0, t));
  return 1 + c3 * Math.pow(clamped - 1, 3) + c1 * Math.pow(clamped - 1, 2);
}
