# WebSocket Protocol

## Overview

Flasharr uses WebSocket for real-time updates between the backend and frontend. This ensures that download progress, state changes, and batch updates are reflected immediately in the UI without requiring manual refresh.

## Connection

- **Endpoint**: `ws://localhost:8484/api/ws` (or `wss://` for HTTPS)
- **Auto-reconnect**: Enabled with exponential backoff
- **Max reconnect attempts**: 10

## Message Types

### SYNC_ALL

Full synchronization of all download tasks.

**Direction**: Backend → Frontend  
**Frequency**: On initial connection

```json
{
  "type": "SYNC_ALL",
  "tasks": [
    /* array of DownloadTask */
  ]
}
```

### TASK_ADDED

New download task created.

**Direction**: Backend → Frontend  
**Frequency**: Immediate (when task is added)

```json
{
  "type": "TASK_ADDED",
  "task": {
    /* DownloadTask object */
  }
}
```

### TASK_UPDATED

Download task state or progress changed.

**Direction**: Backend → Frontend  
**Frequency**: Real-time (as changes occur)

```json
{
  "type": "TASK_UPDATED",
  "task": {
    /* Partial DownloadTask with updated fields */
  }
}
```

### TASK_BATCH_UPDATE

Batch of task updates (optimized for performance).

**Direction**: Backend → Frontend  
**Frequency**: Every 500ms (for active downloads)

```json
{
  "type": "TASK_BATCH_UPDATE",
  "tasks": [
    /* array of partial DownloadTask updates */
  ]
}
```

### TASK_REMOVED

Download task deleted.

**Direction**: Backend → Frontend  
**Frequency**: Immediate (when task is deleted)

```json
{
  "type": "TASK_REMOVED",
  "task_id": "uuid-string"
}
```

### ENGINE_STATS

Engine statistics and status counts.

**Direction**: Backend → Frontend  
**Frequency**: Every 2 seconds

```json
{
  "type": "ENGINE_STATS",
  "stats": {
    "active_downloads": 3,
    "queued": 5,
    "completed": 10,
    "failed": 1,
    "paused": 2,
    "total_speed": 5242880,
    "db_counts": {
      /* database status counts */
    }
  }
}
```

## Frontend Implementation

The frontend uses a WebSocket client (`websocket.ts`) that:

1. Automatically connects on page load
2. Registers message handlers for each message type
3. Updates Svelte stores in real-time
4. Handles reconnection on disconnect

### Batch Updates

When tasks are added/removed from batches:

- Frontend refetches batch summaries to update grouping
- Batch items cache is updated in real-time
- UI reflects changes immediately without manual refresh

## Performance Optimizations

1. **Batched Updates**: Progress updates are sent in batches every 500ms instead of individually
2. **Delta Updates**: Only changed fields are sent, not entire task objects
3. **Selective Refetch**: Batch summaries are only refetched when structure changes (add/remove)

## Troubleshooting

### Connection Issues

- Check backend is running on correct port (8484)
- Verify WebSocket endpoint is accessible
- Check browser console for connection errors

### Missing Updates

- Verify WebSocket connection status in UI
- Check backend logs for broadcast errors
- Ensure message handlers are registered

### Performance Issues

- Monitor batch update frequency (should be ~500ms)
- Check for excessive refetch calls
- Verify delta updates are working correctly
