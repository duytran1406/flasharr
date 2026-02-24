# 🎨 Batch Progress UI Design - Project Aurora

## Component: BatchProgressCard

### Visual Design

```
┌─────────────────────────────────────────────────────────┐
│  Breaking Bad S01                              [⋮]      │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  │
│                                                          │
│     ╭───────────╮                                       │
│     │    45%    │  ← Circular progress ring            │
│     │           │     (cyan glow, animated)             │
│     ╰───────────╯                                       │
│                                                          │
│  5/13 episodes  •  5.2 MB/s  •  2 min remaining        │
│                                                          │
│  [▶ Resume] [⏸ Pause] [🗑 Delete]                      │
└─────────────────────────────────────────────────────────┘
```

### Color Palette (Project Aurora)

- **Primary**: `#00f3ff` (Cyan) - Progress ring, borders
- **Secondary**: `#00ffa3` (Emerald) - Speed indicator
- **Accent**: `#7000ff` (Purple) - Hover states
- **Background**: `#010203` → `#1a2332` gradient
- **Glass**: `backdrop-filter: blur(24px)`

### Typography

- **Title**: Inter 1.25rem, weight 600, cyan glow
- **Stats**: Inter 0.875rem, weight 500
- **Labels**: Inter 0.75rem, weight 400, 60% opacity

### Effects

1. **Glassmorphism**: Frosted glass background
2. **Neon Glow**: Box-shadow with cyan (#00f3ff) at 40% opacity
3. **Tactical Corners**: Clip-path cut corners (4px)
4. **Hover Animation**: Scale 1.02, glow intensifies to 60%
5. **Progress Ring**: Animated stroke-dashoffset, 300ms ease

### Spacing

- **Padding**: 24px
- **Gap**: 16px between elements
- **Border**: 2px solid rgba(0, 243, 255, 0.3)
- **Border Radius**: 16px

---

## Component Structure

```svelte
<script lang="ts">
  interface BatchProgress {
    batch_id: string;
    batch_name: string;
    total_tasks: number;
    completed_tasks: number;
    downloading_tasks: number;
    overall_progress: number;
    combined_speed: number;
    estimated_time_remaining: number;
  }

  let { batchId }: { batchId: string } = $props();
  let progress = $state<BatchProgress | null>(null);

  async function fetchProgress() {
    const res = await fetch(`/api/downloads/batch/${batchId}/progress`);
    progress = await res.json();
  }

  onMount(() => {
    fetchProgress();
    const interval = setInterval(fetchProgress, 2000);
    return () => clearInterval(interval);
  });
</script>

<div class="batch-card">
  <div class="header">
    <h3>{progress?.batch_name}</h3>
    <button class="menu-btn">⋮</button>
  </div>

  <div class="progress-ring">
    <svg viewBox="0 0 100 100">
      <circle class="ring-bg" cx="50" cy="50" r="45" />
      <circle class="ring-progress"
        cx="50" cy="50" r="45"
        style:stroke-dashoffset={283 - (283 * (progress?.overall_progress || 0) / 100)} />
    </svg>
    <span class="percentage">{progress?.overall_progress.toFixed(1)}%</span>
  </div>

  <div class="stats">
    <div class="stat">
      <span class="label">Episodes</span>
      <span class="value">{progress?.completed_tasks}/{progress?.total_tasks}</span>
    </div>
    <div class="stat">
      <span class="label">Speed</span>
      <span class="value">{formatSpeed(progress?.combined_speed)}</span>
    </div>
    <div class="stat">
      <span class="label">ETA</span>
      <span class="value">{formatETA(progress?.estimated_time_remaining)}</span>
    </div>
  </div>

  <div class="actions">
    <button class="modal-btn" onclick={resumeBatch}>▶ Resume</button>
    <button class="modal-btn" onclick={pauseBatch}>⏸ Pause</button>
    <button class="modal-btn danger" onclick={deleteBatch}>🗑 Delete</button>
  </div>
</div>

<style>
  .batch-card {
    background: linear-gradient(135deg, #0a0e1a 0%, #1a2332 50%, #0a0e1a 100%);
    border: 2px solid rgba(0, 243, 255, 0.3);
    border-radius: 16px;
    padding: 24px;
    backdrop-filter: blur(24px);
    box-shadow: 0 0 40px rgba(0, 243, 255, 0.2);
    transition: all 0.3s ease;
    clip-path: polygon(
      8px 0%, calc(100% - 8px) 0%,
      100% 8px, 100% calc(100% - 8px),
      calc(100% - 8px) 100%, 8px 100%,
      0% calc(100% - 8px), 0% 8px
    );
  }

  .batch-card:hover {
    transform: scale(1.02);
    box-shadow: 0 0 60px rgba(0, 243, 255, 0.4);
    border-color: rgba(0, 255, 163, 0.5);
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 20px;
  }

  h3 {
    color: var(--color-primary);
    font-size: 1.25rem;
    font-weight: 600;
    margin: 0;
    text-shadow: 0 0 10px rgba(0, 243, 255, 0.5);
  }

  .progress-ring {
    position: relative;
    width: 120px;
    height: 120px;
    margin: 0 auto 20px;
  }

  .ring-bg {
    fill: none;
    stroke: rgba(255, 255, 255, 0.1);
    stroke-width: 8;
  }

  .ring-progress {
    fill: none;
    stroke: var(--color-primary);
    stroke-width: 8;
    stroke-linecap: round;
    stroke-dasharray: 283;
    transform: rotate(-90deg);
    transform-origin: 50% 50%;
    filter: drop-shadow(0 0 8px var(--color-primary));
    transition: stroke-dashoffset 0.3s ease;
  }

  .percentage {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    font-size: 1.5rem;
    font-weight: 700;
    color: var(--color-primary);
    text-shadow: 0 0 10px rgba(0, 243, 255, 0.6);
  }

  .stats {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 16px;
    margin-bottom: 20px;
  }

  .stat {
    text-align: center;
  }

  .label {
    display: block;
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.6);
    margin-bottom: 4px;
  }

  .value {
    display: block;
    font-size: 1rem;
    font-weight: 600;
    color: var(--color-secondary);
    text-shadow: 0 0 8px rgba(0, 255, 163, 0.4);
  }

  .actions {
    display: flex;
    gap: 12px;
  }

  .modal-btn {
    flex: 1;
    background: linear-gradient(135deg, #0a0e1a 0%, #1a2332 50%, #0a0e1a 100%);
    border: 2px solid var(--color-primary);
    color: var(--color-primary);
    padding: 10px 16px;
    border-radius: 8px;
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s ease;
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.3);
  }

  .modal-btn:hover {
    border-color: var(--color-secondary);
    box-shadow: 0 0 30px rgba(0, 255, 163, 0.5);
    transform: translateY(-2px);
  }

  .modal-btn:active {
    transform: scale(0.98);
  }

  .modal-btn.danger {
    border-color: #ff4444;
    color: #ff4444;
  }

  .modal-btn.danger:hover {
    box-shadow: 0 0 30px rgba(255, 68, 68, 0.5);
  }
</style>
```

---

## Integration Points

### 1. API Endpoint

```typescript
GET /api/downloads/batch/:batch_id/progress
```

### 2. Store Integration

```typescript
// downloads.ts
export const batchProgress = writable<Map<string, BatchProgress>>(new Map());

export async function fetchBatchProgress(batchId: string) {
  const res = await fetch(`/api/downloads/batch/${batchId}/progress`);
  const data = await res.json();
  batchProgress.update((map) => {
    map.set(batchId, data);
    return map;
  });
}
```

### 3. Downloads Page Update

```svelte
<!-- Group tasks by batch_id -->
{#each Object.entries(groupedTasks) as [batchId, tasks]}
  {#if batchId !== 'null'}
    <BatchProgressCard {batchId} />
  {/if}

  <!-- Individual task rows -->
  {#each tasks as task}
    <DownloadRow {task} />
  {/each}
{/each}
```

---

## Animations

### Progress Ring Animation

```css
@keyframes pulse-glow {
  0%,
  100% {
    filter: drop-shadow(0 0 8px var(--color-primary));
  }
  50% {
    filter: drop-shadow(0 0 16px var(--color-primary));
  }
}

.ring-progress {
  animation: pulse-glow 2s ease-in-out infinite;
}
```

### Card Entrance

```css
@keyframes slide-in {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.batch-card {
  animation: slide-in 0.4s ease-out;
}
```

---

## Responsive Design

```css
@media (max-width: 768px) {
  .batch-card {
    padding: 16px;
  }

  .stats {
    grid-template-columns: 1fr;
    gap: 12px;
  }

  .actions {
    flex-direction: column;
  }

  .progress-ring {
    width: 100px;
    height: 100px;
  }
}
```

---

## Accessibility

- **ARIA labels** on all buttons
- **Keyboard navigation** support
- **Screen reader** friendly stats
- **High contrast** mode support

---

## Performance

- **Polling**: 2 seconds (configurable)
- **Debouncing**: Action buttons debounced 300ms
- **Lazy loading**: Only fetch visible batches
- **Memoization**: Cache formatted strings

---

**READY FOR FRONTEND IMPLEMENTATION** 🚀
