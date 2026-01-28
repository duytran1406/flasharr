---
name: UI/UX Design System
description: Design guidelines and patterns for Flasharr
---

# UI/UX Design System Skill

## Overview

This skill provides design guidelines, patterns, and best practices for creating consistent and beautiful UI in Flasharr.

## Design Principles

1. **Premium Feel** - Use modern aesthetics, subtle animations, glassmorphism
2. **Dark Mode First** - Design for dark theme as primary
3. **Responsive** - Mobile-first approach
4. **Accessible** - WCAG 2.1 AA compliance

## Color Palette

```css
:root {
  /* Primary Colors */
  --color-primary: #6366f1; /* Indigo */
  --color-primary-dark: #4f46e5;
  --color-primary-light: #818cf8;

  /* Background */
  --bg-primary: #0f0f0f;
  --bg-secondary: #1a1a1a;
  --bg-card: #242424;

  /* Text */
  --text-primary: #ffffff;
  --text-secondary: #a1a1aa;
  --text-muted: #71717a;

  /* Accent */
  --color-success: #22c55e;
  --color-warning: #eab308;
  --color-error: #ef4444;
}
```

## Typography

- **Headings**: Inter, 600-700 weight
- **Body**: Inter, 400-500 weight
- **Monospace**: JetBrains Mono

## Component Patterns

### Cards

- Rounded corners (12px)
- Subtle border or shadow
- Hover lift effect
- Glassmorphism optional

### Buttons

- Primary: Solid background with gradient
- Secondary: Outlined with hover fill
- Ghost: Transparent with hover background

### Animations

- Duration: 150-300ms
- Easing: ease-out for entrances, ease-in for exits
- Micro-interactions for feedback

## Spacing Scale

```
4px, 8px, 12px, 16px, 24px, 32px, 48px, 64px
```

## Icons

- Use Lucide icons for consistency
- 20-24px for standard icons
- 16px for inline icons
