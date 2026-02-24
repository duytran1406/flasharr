# Batch Detail Modal Design

## Overview

A premium glassmorphism modal showing comprehensive batch statistics and child downloads, matching the style of the existing Item Detail Modal.

## Data Available (BatchGroup Interface)

- `batchId`: Unique identifier
- `batchName`: Display name
- `downloads`: Array of child DownloadTask items
- `totalItems`: Total file count
- `totalSize`: Combined size of all files
- `downloaded`: Bytes downloaded so far
- `progress`: Overall completion percentage
- `speed`: Current download speed
- `activeCount`, `completedCount`, `failedCount`, `pausedCount`, `queuedCount`: Status breakdowns
- `createdAt`: Timestamp

## Layout Structure

### 1. Header Section

```
┌─────────────────────────────────────────────────┐
│ 🔖 Batch ID Badge (clickable to copy)          │
│ 📦 Batch Name                              [X] │
└─────────────────────────────────────────────────┘
```

- **Batch ID Badge**: Clickable fingerprint icon + truncated ID (like item modal)
- **Batch Name**: Large title
- **Close Button**: Top-right X button

### 2. Hero Stats Section

```
┌─────────────────────────────────────────────────┐
│  STATUS        PROGRESS       SPEED             │
│  Mixed (4/10)  65.3%          2.5 MB/s          │
│                                                  │
│  ████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │
│  6.5 GB OF 10 GB                                │
└─────────────────────────────────────────────────┘
```

- **STATUS**: Show completion ratio (e.g., "4/10 Completed", "Mixed", "All Done")
- **PROGRESS**: Overall batch percentage
- **SPEED**: Combined download speed
- **Progress Bar**: Glowing bar with aggregated progress
- **Size Info**: Downloaded / Total size

### 3. Batch Statistics Grid

```
┌──────────────────────┬──────────────────────┐
│ 📊 BATCH OVERVIEW    │ ⏱️ TIMING INFO       │
│                      │                      │
│ 📁 Total Files: 10   │ 📅 Created:          │
│ ✅ Completed: 4      │    Feb 5, 16:30      │
│ ⬇️ Downloading: 2    │ 🏁 Est. Complete:    │
│ ⏸️ Paused: 1         │    ~15 minutes       │
│ ⏳ Queued: 2         │                      │
│ ❌ Failed: 1         │                      │
└──────────────────────┴──────────────────────┘
```

### 4. Child Downloads List

```
┌─────────────────────────────────────────────────┐
│ 📋 FILES IN BATCH (10)                          │
├─────────────────────────────────────────────────┤
│ ✅ file1.mkv                    100%  2.1 GB    │
│ ⬇️ file2.mkv                     45%  1.5 GB    │
│ ⏸️ file3.mkv                     12%  800 MB    │
│ ⏳ file4.mkv                      0%  1.2 GB    │
│ ❌ file5.mkv                      0%  950 MB    │
│ ... (scrollable)                                │
└─────────────────────────────────────────────────┘
```

- **Scrollable list** of all child downloads
- Each row shows: Status icon, filename, progress, size
- Click on a file row to open its individual detail modal

### 5. Batch Actions Footer

```
┌─────────────────────────────────────────────────┐
│  [RESUME ALL]  [PAUSE ALL]  [RETRY FAILED]     │
│                                    [CLOSE]      │
└─────────────────────────────────────────────────┘
```

## Visual Style (Matching Item Modal)

### Colors & Effects

- **Background**: `glass-panel-premium` (glassmorphism)
- **Progress Bar**: Glowing effect with dynamic color based on status
- **Badges**: Same style as item modal (fingerprint icon + ID)
- **Status Icons**: Material icons with color coding
  - ✅ Green (#00ffa3) - Completed
  - ⬇️ Cyan (#00f3ff) - Downloading
  - ⏸️ Gray (#64748b) - Paused
  - ⏳ Yellow (#ffd700) - Queued
  - ❌ Red (#ff5252) - Failed

### Typography

- **Section Labels**: Uppercase, small, gray
- **Values**: Larger, white/colored
- **Batch Name**: Bold, 1.5rem

### Spacing

- Consistent padding: 1.5rem
- Grid gap: 1rem
- Section spacing: 1.5rem

## Interaction Behavior

1. **Opening**: Click batch row → Fade in overlay + Slide up modal
2. **Batch ID Copy**: Click ID badge → Copy full batch ID to clipboard
3. **Child File Click**: Click file in list → Close batch modal → Open file detail modal
4. **Actions**:
   - Resume All: Resume all paused/queued downloads
   - Pause All: Pause all active downloads
   - Retry Failed: Retry all failed downloads
5. **Closing**: Click overlay, X button, or ESC key

## Implementation Notes

- Reuse existing modal styles (`details-modal-overlay`, `glass-panel-premium`)
- Use same animation transitions (fade, fly)
- Calculate aggregate stats from `downloads` array
- Show "Mixed" status when files have different states
- Estimated completion time = (totalSize - downloaded) / speed
- Child list should be virtualized if > 50 items

## Responsive Behavior

- **Desktop**: Full modal (max-width: 900px)
- **Mobile**: Full screen modal with adjusted layout
- Child list becomes more compact on mobile
