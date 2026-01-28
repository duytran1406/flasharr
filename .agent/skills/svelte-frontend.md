---
name: Svelte Frontend Development
description: Skills for developing the Flasharr Svelte frontend
---

# Svelte Frontend Development Skill

## Overview

This skill provides guidance for working with the Flasharr Svelte frontend, including component development, state management, and styling.

## Key Directories

- `/frontend/src/` - Main source code
- `/frontend/src/routes/` - SvelteKit routes
- `/frontend/src/lib/` - Shared components and utilities
- `/frontend/src/lib/components/` - Reusable UI components

## Common Tasks

### Development Server

```bash
cd frontend && npm run dev
```

### Production Build

```bash
cd frontend && npm run build
```

### Type Checking

```bash
cd frontend && npm run check
```

## Component Structure

```svelte
<script lang="ts">
  // TypeScript logic
</script>

<div class="component">
  <!-- HTML template -->
</div>

<style>
  /* Scoped CSS */
</style>
```

## Best Practices

1. Use TypeScript for type safety
2. Keep components small and focused
3. Use stores for shared state
4. Implement proper loading states
5. Handle errors gracefully

## API Integration

- Use `fetch` or custom API client in `/frontend/src/lib/api/`
- Backend API at `http://localhost:3000`

## Debug Tips

- Check browser DevTools console
- Use Svelte DevTools extension
- Check `debug_log/frontend-dev.log` for build errors
