# Flasharr Design System — MASTER.md

> **Project Aurora: Cyber-Dark Elite Design Language**
>
> This document defines the complete design system for Flasharr, extracted from the production codebase. Use this as the source of truth for all UI/UX decisions.

---

## 🎨 Color System: Cyber-Dark (Flasharr Elite)

### Primary Palette

| Token               | Value               | Usage                                             |
| ------------------- | ------------------- | ------------------------------------------------- |
| `--color-primary`   | `#00f3ff` (Cyan)    | Primary actions, highlights, active states, glows |
| `--color-secondary` | `#00ffa3` (Emerald) | Success, confirmations, hover accents             |
| `--color-accent`    | `#7000ff` (Violet)  | Smart features, premium actions, gradients        |

### Background System

| Token              | Value                  | Usage                               |
| ------------------ | ---------------------- | ----------------------------------- |
| `--bg-app`         | `#010203`              | Deep space black body background    |
| `--bg-glass`       | `rgba(8, 10, 15, 0.8)` | Standard glassmorphism panels       |
| `--bg-glass-heavy` | `rgba(4, 5, 8, 0.95)`  | Modals, overlays, elevated surfaces |

### Border & Glow Effects

| Token                   | Value                             | Usage                        |
| ----------------------- | --------------------------------- | ---------------------------- |
| `--border-glass`        | `rgba(255, 255, 255, 0.08)`       | Subtle panel borders         |
| `--border-glass-active` | `rgba(0, 243, 255, 0.5)`          | Focused/active state borders |
| `--shadow-glass`        | Complex shadow                    | 3D depth for glass panels    |
| `--glow-primary`        | `0 0 20px rgba(0, 243, 255, 0.2)` | Neon glow effects            |

### Text Hierarchy

| Token              | Value     | Usage                     |
| ------------------ | --------- | ------------------------- |
| `--text-primary`   | `#e2e8f0` | Main content, headings    |
| `--text-secondary` | `#94a3b8` | Supporting text, labels   |
| `--text-muted`     | `#64748b` | De-emphasized text, hints |

### Semantic Colors

| Purpose    | Color  | Hex                   |
| ---------- | ------ | --------------------- |
| Success    | Green  | `#22c55e` / `#00ffa3` |
| Error      | Red    | `#ff5252` / `#ef4444` |
| Warning    | Amber  | `#ffd700`             |
| Processing | Purple | `#8b5cf6`             |

---

## 📐 Typography

### Font Stack

```css
--font-heading: "Outfit", sans-serif; /* Titles, section headers */
--font-body: "Inter", sans-serif; /* All body text */
--font-mono: "JetBrains Mono", monospace; /* Stats, code, HUD elements */
```

### Type Scale (Mobile-First)

| Token                     | Size            | Usage                |
| ------------------------- | --------------- | -------------------- |
| `--font-size-mobile-xs`   | 0.75rem (12px)  | Micro labels, badges |
| `--font-size-mobile-sm`   | 0.875rem (14px) | Secondary text, meta |
| `--font-size-mobile-base` | 1rem (16px)     | Body text            |
| `--font-size-mobile-lg`   | 1.125rem (18px) | Subheadings          |
| `--font-size-mobile-xl`   | 1.25rem (20px)  | Section titles       |
| `--font-size-mobile-2xl`  | 1.5rem (24px)   | Page titles          |

### Text Treatments

- **Headings**: `font-weight: 700-800`, `letter-spacing: -0.02em`
- **Labels**: `text-transform: uppercase`, `letter-spacing: 0.05-0.12em`, `font-weight: 800`
- **Monospace**: `letter-spacing: 0.05em`, use for stats/data/HUD elements
- **Glow Text**: `text-shadow: 0 0 10px rgba(0, 243, 255, 0.5)`

---

## 🪟 Glassmorphism Design

### Standard Glass Panel

```css
.glass-panel {
  background: var(--bg-glass);
  backdrop-filter: blur(24px);
  -webkit-backdrop-filter: blur(24px);
  border: 1px solid var(--border-glass);
  box-shadow: var(--shadow-glass);
  border-radius: 12px;
}
```

### Panel Hierarchy

| Component    | Background         | Border Radius | Notes                          |
| ------------ | ------------------ | ------------- | ------------------------------ |
| Sidebar      | `--bg-glass`       | —             | Full height, 280px width       |
| Header       | `--bg-glass`       | —             | 72px height                    |
| Cards        | `--bg-glass`       | 12px          | Interactive, can lift on hover |
| Box Sections | `var(--bg-glass)`  | 6px           | Fieldset-style with labels     |
| Modals       | `--bg-glass-heavy` | 24px          | Highest elevation              |

### Hover States

```css
.glass-panel:hover {
  border-color: var(--border-glass-active);
  box-shadow:
    0 30px 60px rgba(0, 0, 0, 0.6),
    0 0 20px rgba(0, 243, 255, 0.15);
  transform: translateY(-4px);
}
```

> **Note**: Use `.no-lift` or `.glass-panel--static` class to disable hover lift on container panels.

---

## 🔘 Button System

### Premium Sci-Fi Button (Primary)

Key characteristics:

- **Metallic gradient base**: `linear-gradient(135deg, #0a0e1a 0%, #1a2332 50%, #0a0e1a 100%)`
- **Tactical clip-path corners**: 4px corner cuts
- **Holographic scan line animation** on hover
- **Neon glow shadow system**

```css
.dl-btn-premium,
.add-btn,
.modal-btn.primary {
  background: linear-gradient(135deg, #0a0e1a 0%, #1a2332 50%, #0a0e1a 100%);
  color: var(--color-primary);
  border: 2px solid var(--color-primary);
  clip-path: polygon(
    4px 0%,
    calc(100% - 4px) 0%,
    100% 4px,
    100% calc(100% - 4px),
    calc(100% - 4px) 100%,
    4px 100%,
    0% calc(100% - 4px),
    0% 4px
  );
  font-weight: 900;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}
```

### Hover Transition

```css
.dl-btn-premium:hover {
  border-color: var(--color-secondary);
  color: var(--color-secondary);
  box-shadow:
    0 0 30px rgba(0, 255, 163, 0.6),
    0 0 60px rgba(0, 255, 163, 0.3);
  transform: translateY(-1px);
}
```

### Button Variants

| Class                  | Purpose             | Border Color                          |
| ---------------------- | ------------------- | ------------------------------------- |
| `.modal-btn.primary`   | Primary actions     | `--color-primary`                     |
| `.modal-btn.secondary` | Secondary/cancel    | `rgba(255, 255, 255, 0.3)`            |
| `.btn-success`         | Success actions     | `rgba(34, 197, 94, 0.3)`              |
| `.btn-danger`          | Destructive actions | `rgba(239, 68, 68, 0.3)`              |
| `.btn-smart-grab`      | AI features         | Gradient purple `#8b5cf6` → `#6366f1` |

### Icon Buttons

```css
.icon-btn {
  width: 44px;
  height: 44px;
  border-radius: 12px;
  border: 1px solid var(--border-glass);
  background: var(--bg-glass);
}

.icon-btn-tiny {
  width: 28px;
  height: 28px;
  border-radius: 6px;
}
```

---

## 🏗️ Layout System

### App Shell Structure

```
┌─────────────────────────────────────────────────┐
│                  .app-shell                     │
├──────────────┬──────────────────────────────────┤
│              │         .glass-header (72px)     │
│              ├──────────────────────────────────┤
│ .glass-      │                                  │
│  sidebar     │         .view-container          │
│  (280px)     │                                  │
│              │                                  │
└──────────────┴──────────────────────────────────┘
```

### Layout Variables

| Token                       | Value | Usage             |
| --------------------------- | ----- | ----------------- |
| `--sidebar-width`           | 280px | Expanded sidebar  |
| `--sidebar-collapsed-width` | 96px  | Collapsed sidebar |
| `--header-height`           | 72px  | Desktop header    |

### Responsive Breakpoints

| Token             | Value  | Target               |
| ----------------- | ------ | -------------------- |
| `--bp-mobile-sm`  | 320px  | iPhone SE            |
| `--bp-mobile`     | 375px  | iPhone 12/13         |
| `--bp-mobile-lg`  | 428px  | iPhone 14/15 Pro Max |
| `--bp-tablet`     | 768px  | iPad Portrait        |
| `--bp-tablet-lg`  | 1024px | iPad Landscape       |
| `--bp-desktop`    | 1025px | Desktop              |
| `--bp-desktop-lg` | 1440px | Large Desktop        |

---

## 📱 Mobile Design System

### Mobile Variables

```css
--mobile-header-h: 52px;
--mobile-bottom-nav-h: 60px;
--mobile-safe-bottom: env(safe-area-inset-bottom, 0px);
--mobile-radius: 12px;
--mobile-gap: 10px;
--mobile-pad: 12px;
--touch-min: 44px; /* Minimum touch target */
```

### Spacing Scale (8px Base)

| Token                  | Value |
| ---------------------- | ----- |
| `--spacing-mobile-xs`  | 8px   |
| `--spacing-mobile-sm`  | 12px  |
| `--spacing-mobile-md`  | 16px  |
| `--spacing-mobile-lg`  | 24px  |
| `--spacing-mobile-xl`  | 32px  |
| `--spacing-mobile-xxl` | 48px  |

### Touch Targets

- Minimum: `44px × 44px`
- Comfortable: `48px × 48px`
- Minimum spacing between targets: `8px`

### Z-Index Scale

| Token                   | Value | Usage             |
| ----------------------- | ----- | ----------------- |
| `--z-mobile-modal`      | 1100  | Modals            |
| `--z-mobile-drawer`     | 1000  | Side drawer       |
| `--z-mobile-overlay`    | 999   | Drawer overlay    |
| `--z-mobile-bottom-nav` | 900   | Bottom navigation |
| `--z-mobile-header`     | 800   | Mobile header     |

---

## ⚡ Animation System

### Timing Variables

```css
--duration-fast: 150ms;
--duration-normal: 250ms;
--duration-slow: 350ms;
```

### Easing Functions

```css
--spring: cubic-bezier(0.34, 1.56, 0.64, 1); /* Bouncy spring */
--ease-out: cubic-bezier(0.16, 1, 0.3, 1); /* Smooth deceleration */
```

### Key Animations

| Animation              | Duration | Usage                   |
| ---------------------- | -------- | ----------------------- |
| `aurora-drift`         | 20s      | Background ambient glow |
| `holographic-scan`     | 3s       | Button hover scanline   |
| `neural-spin-forward`  | 2s       | Loader outer ring       |
| `neural-spin-backward` | 1.2s     | Loader inner ring       |
| `slideInRight`         | —        | Toast entrance          |
| `haptic-pulse`         | fast     | Touch feedback          |

### Aurora Drift (Background)

```css
@keyframes aurora-drift {
  0%,
  100% {
    transform: scale(1);
    opacity: 0.8;
  }
  50% {
    transform: scale(1.1);
    opacity: 1;
  }
}
```

---

## 🎴 Card Patterns

### Poster Card (Media)

- **Aspect Ratio**: 2:3
- **Border Radius**: 12px
- **Hover Effect**: `translateY(-5px)` + cyan glow
- **Overlay**: Gradient from bottom (transparent → black)

### Data Shard Card (Downloads)

- **Side accent bar**: 3px colored bar indicating status
- **Compact info layout**: filename, badges, meta
- **Status glow on hover**

### Box Section (Dashboard Widgets)

```css
.box-section {
  position: relative;
  border: 1px solid var(--border-glass);
  border-radius: 6px;
  padding: 1.25rem 0.5rem 0.5rem 0.5rem;
  background: var(--bg-glass);
}

.box-label {
  position: absolute;
  top: -0.6rem;
  left: 0.8rem;
  background: var(--bg-app); /* Masks border */
  padding: 0 0.4rem;
  font-size: 0.75rem;
  text-transform: uppercase;
  font-weight: 800;
}
```

---

## 🔤 Form Elements

### Input Styling

```css
.modal-input,
.filter-input-date {
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid var(--border-glass);
  border-radius: 12px;
  color: var(--text-primary);
  padding: 1rem;
}

.modal-input:focus {
  border-color: var(--color-primary);
  box-shadow: var(--glow-primary);
}
```

### Select Styling

```css
.sort-select-premium {
  background: rgba(15, 23, 42, 0.8);
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 12px;
  padding: 0.6rem 2.5rem 0.6rem 1rem;
  /* Custom dropdown arrow via background-image */
}
```

### Chips & Pills

```css
.filter-chip {
  padding: 0.5rem 1rem;
  border-radius: 20px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid var(--border-glass);
}

.filter-chip.active {
  background: var(--color-primary);
  color: #000;
  font-weight: 700;
  box-shadow: 0 0 15px rgba(0, 243, 255, 0.3);
}
```

### Tabs

```css
.tab-container {
  background: rgba(0, 0, 0, 0.2);
  padding: 4px;
  border-radius: 12px;
}

.tab-btn {
  border-radius: 8px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.tab-btn.active {
  background: var(--color-primary);
  color: #000;
  box-shadow: 0 4px 12px rgba(0, 243, 255, 0.3);
}
```

---

## 🌙 Light Theme (Arctic)

When `[data-theme="light"]` or `body.theme-arctic`:

| Token               | Dark Value             | Light Value                 |
| ------------------- | ---------------------- | --------------------------- |
| `--color-primary`   | `#00f3ff`              | `#0066cc` or `#ff3d00`      |
| `--color-secondary` | `#00ffa3`              | `#00a86b` or `#2979ff`      |
| `--bg-app`          | `#010203`              | `#f8fafc`                   |
| `--bg-glass`        | `rgba(8, 10, 15, 0.8)` | `rgba(255, 255, 255, 0.85)` |
| `--text-primary`    | `#e2e8f0`              | `#1e293b`                   |
| `--text-secondary`  | `#94a3b8`              | `#475569`                   |

---

## 📏 Scrollbar Styling

```css
::-webkit-scrollbar {
  width: 4px;
  height: 4px;
}

::-webkit-scrollbar-track {
  background: #020205;
}

::-webkit-scrollbar-thumb {
  background: var(--color-primary);
  border-radius: 2px;
  box-shadow: 0 0 10px var(--color-primary);
}
```

---

## ✅ Pre-Delivery Checklist

### Visual Quality

- [ ] No emojis as UI icons (use Material Icons)
- [ ] Consistent icon sizing (Material Icons 20-24px)
- [ ] Hover states don't cause layout shift
- [ ] Always use CSS variables, not hardcoded values

### Interaction

- [ ] All clickable elements have `cursor: pointer`
- [ ] Hover feedback visible (color, glow, transform)
- [ ] Transitions are 150-300ms
- [ ] Touch targets minimum 44px

### Dark/Light Mode

- [ ] Test both themes
- [ ] Glass panels visible in light mode (higher opacity)
- [ ] Borders visible in both modes
- [ ] Contrast ratio 4.5:1 minimum

### Mobile

- [ ] Bottom nav visible and functional
- [ ] Safe area insets respected
- [ ] No horizontal scroll
- [ ] Grid collapses to 2 columns on mobile

---

## 🚫 Anti-Patterns to Avoid

| ❌ Don't                              | ✅ Do                                      |
| ------------------------------------- | ------------------------------------------ |
| Use emojis as icons                   | Use Material Icons or Lucide               |
| Hardcode colors                       | Use CSS variables                          |
| Use `scale()` on hover (causes shift) | Use `translateY()` for lift                |
| Mix border-radius values              | Use 12px (panels), 6px (boxes), 8px (tabs) |
| Forget touch targets                  | Minimum 44px touch area                    |
| Skip transitions                      | Use 150-300ms for all interactive elements |
| Use light text in light mode          | Follow contrast guidelines                 |

---

_Document generated from Flasharr v3 codebase — Project Aurora_
