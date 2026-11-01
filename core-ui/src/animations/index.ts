/**
 * Media-set Animation System — powered by anime.js v4.
 *
 * Svelte transitions (animeFade, animeFly, animeSlide, animePop,
 * animeSlideDown), Svelte actions (stagger, countUp, revealOnScroll,
 * animateRing) and thin re-exports of the anime.js primitives so apps share a
 * single animation engine.
 *
 * RECOVERY NOTE: animeFade, animeFly, animePop, animeSlideDown, countUp,
 * revealOnScroll, animateRing and the anime re-exports are recovered from the
 * compiled bundles (2026-04-28 .. 2026-09-02). animeSlide is recovered from the
 * 2026-04-28 build only. The `stagger` action and animatePageEntrance were
 * imported (but never called) by the frontend, so they were tree-shaken: they
 * are ports of the pre-migration frontend versions (lib/animations at 8ae7c57^)
 * to the anime.js v4 API — inferred, no compiled evidence.
 */

import * as animeModule from "animejs";
import type { Action } from "svelte/action";

const anime = animeModule;

export const { animate, stagger: animeStagger, set, createTimeline } = anime;

// ============================================================================
// Svelte transitions (drop-in replacements for svelte/transition)
// ============================================================================

/**
 * Replaces `fade`.
 * Usage: <div transition:animeFade={{ duration: 300 }}>
 */
export function animeFade(
  node: HTMLElement,
  { duration = 300, delay = 0, easing = "easeOutQuad" } = {},
) {
  return {
    duration: duration + delay,
    tick(t: number, u: number) {
      // t goes 0→1 on intro, 1→0 on outro
      node.style.opacity = String(t);
    },
  };
}

/**
 * Replaces `fly`.
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
  const opacity = +getComputedStyle(node).opacity;

  return {
    duration: duration + delay,
    css(t: number) {
      const eased = easeOutCubic(
        Math.max(0, (t * (duration + delay) - delay) / duration),
      );
      return `
        opacity: ${eased * opacity};
        transform: translate(${(1 - eased) * x}px, ${(1 - eased) * y}px);
      `;
    },
  };
}

/**
 * Slide + fade with a slight scale-up.
 * Usage: <div in:animeSlide={{ y: 30, duration: 500 }}>
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
      const eased = easeOutCubic(
        Math.max(0, (t * (duration + delay) - delay) / duration),
      );
      const s = scale + (1 - scale) * eased;
      return `
        opacity: ${eased};
        transform: translate(${(1 - eased) * x}px, ${(1 - eased) * y}px) scale(${s});
      `;
    },
  };
}

/**
 * Pop-in with an elastic (back-out) overshoot.
 * Usage: <div in:animePop={{ duration: 400, scale: 0.9 }}>
 */
export function animePop(
  node: HTMLElement,
  { duration = 500, delay = 0, scale = 0.8 } = {},
) {
  return {
    duration: duration + delay,
    css(t: number) {
      const raw = Math.max(0, (t * (duration + delay) - delay) / duration);
      const eased = easeOutBack(raw);
      const s = scale + (1 - scale) * eased;
      return `
        opacity: ${Math.min(1, raw * 2)};
        transform: scale(${s});
      `;
    },
  };
}

/**
 * Replaces `slide`: animates height/padding/margin from 0 with opacity.
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
      const eased = easeOutCubic(
        Math.max(0, (t * (duration + delay) - delay) / duration),
      );
      return `
        overflow: hidden;
        opacity: ${eased * opacity};
        height: ${eased * height}px;
        padding-top: ${eased * paddingTop}px;
        padding-bottom: ${eased * paddingBottom}px;
        margin-top: ${eased * marginTop}px;
        margin-bottom: ${eased * marginBottom}px;
      `;
    },
  };
}

// ============================================================================
// Svelte actions
// ============================================================================

type StaggerParams = {
  delay?: number;
  y?: number;
  x?: number;
  duration?: number;
  scale?: number;
  opacity?: number[];
};

/**
 * Staggered entrance of all direct children once the node scrolls into view.
 * Usage: <div use:stagger={{ delay: 60, y: 20 }}>
 * (inferred, see module note)
 */
export const stagger: Action<HTMLElement, StaggerParams | undefined> = (
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

  const children = Array.from(node.children) as HTMLElement[];
  if (children.length === 0) return;

  for (const child of children) {
    child.style.opacity = String(opacity[0]);
    child.style.transform = `translate(${x}px, ${y}px) scale(${scale})`;
  }

  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          animate(children, {
            opacity: opacity[1],
            translateY: [y, 0],
            translateX: [x, 0],
            scale: [scale, 1],
            duration,
            delay: animeStagger(delay),
            ease: "outQuad",
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

type CountUpParams =
  | number
  | { value: number; duration?: number; decimals?: number };

/**
 * Count up to a number when the node scrolls into view.
 * Usage: <span use:countUp={1234}>0</span>
 */
export const countUp: Action<HTMLElement, CountUpParams> = (node, params) => {
  const { value, duration, decimals } =
    typeof params === "number"
      ? { value: params, duration: 1200, decimals: 0 }
      : {
          value: params?.value ?? 0,
          duration: params?.duration ?? 1200,
          decimals: params?.decimals ?? 0,
        };

  const counter = { val: 0 };

  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          animate(counter, {
            val: value,
            duration,
            ease: "outExpo",
            onUpdate: () => {
              node.textContent =
                decimals > 0
                  ? counter.val.toFixed(decimals)
                  : Math.round(counter.val).toString();
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
      const newValue =
        typeof newParams === "number" ? newParams : (newParams?.value ?? 0);
      const newDecimals =
        typeof newParams === "number" ? 0 : (newParams?.decimals ?? 0);
      animate(counter, {
        val: newValue,
        duration: 600,
        ease: "outQuad",
        onUpdate: () => {
          node.textContent =
            newDecimals > 0
              ? counter.val.toFixed(newDecimals)
              : Math.round(counter.val).toString();
        },
      });
    },
    destroy() {
      observer.disconnect();
    },
  };
};

/**
 * Fade/slide an element in when it scrolls into view.
 * Usage: <div use:revealOnScroll={{ y: 30 }}>
 */
export const revealOnScroll: Action<
  HTMLElement,
  { y?: number; x?: number; duration?: number; delay?: number; scale?: number } | undefined
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
          animate(node, {
            opacity: [0, 1],
            translateY: [y, 0],
            translateX: [x, 0],
            scale: [scale, 1],
            duration,
            delay,
            ease: "outCubic",
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
 * Animate an SVG progress ring via stroke-dashoffset.
 * Usage: <circle r="20" use:animateRing={{ progress: 75 }}>
 */
export const animateRing: Action<
  SVGCircleElement,
  { progress: number; duration?: number }
> = (node, params) => {
  const circumference = 2 * Math.PI * parseFloat(node.getAttribute("r") || "20");
  node.style.strokeDasharray = String(circumference);
  node.style.strokeDashoffset = String(circumference);

  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          const target =
            circumference - (circumference * (params?.progress ?? 0)) / 100;
          animate(node, {
            strokeDashoffset: [circumference, target],
            duration: params?.duration ?? 1500,
            ease: "inOutQuart",
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
      const target =
        circumference - (circumference * (newParams?.progress ?? 0)) / 100;
      animate(node, { strokeDashoffset: target, duration: 800, ease: "outQuad" });
    },
    destroy() {
      observer.disconnect();
    },
  };
};

// ============================================================================
// Page helpers
// ============================================================================

/**
 * Staggered entrance for the common section elements inside a container
 * (call from onMount). (inferred, see module note)
 */
export function animatePageEntrance(
  container: HTMLElement,
  opts?: { delay?: number; staggerDelay?: number },
) {
  const delay = opts?.delay ?? 0;
  const staggerDelay = opts?.staggerDelay ?? 80;

  const sections = Array.from(
    container.querySelectorAll<HTMLElement>(
      "section, .card, .widget, .status-card-v5, .bento-item, [data-animate]",
    ),
  );
  if (sections.length === 0) return;

  for (const el of sections) {
    el.style.opacity = "0";
    el.style.transform = "translateY(20px)";
  }

  animate(sections, {
    opacity: [0, 1],
    translateY: [20, 0],
    duration: 500,
    delay: animeStagger(staggerDelay, { start: delay }),
    ease: "outQuad",
  });
}

// ============================================================================
// Easing helpers (for CSS-based transitions)
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
