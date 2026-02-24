# Download System Test Scenarios

## Test Coverage Matrix

| Scenario                           | User Journey                         | Data Flow                                         | WebSocket Events                                  | Database State                        | Expected Outcome                             |
| ---------------------------------- | ------------------------------------ | ------------------------------------------------- | ------------------------------------------------- | ------------------------------------- | -------------------------------------------- |
| **1. Single Download**             | User adds one file                   | API → Orchestrator → TaskManager → DB             | Created, StateChanged, ProgressUpdated, Completed | Task persisted                        | Download completes, UI updates in real-time  |
| **2. Batch Download (Smart Grab)** | User grabs TV season                 | Discovery → API (batch) → Orchestrator            | Multiple Created events                           | All tasks with same batch_id          | UI shows collapsible group                   |
| **3. Pause/Resume Active**         | User pauses downloading task         | API → Orchestrator → TaskManager → DB             | StateChanged (PAUSED)                             | State updated                         | Download pauses, UI shows PAUSED             |
| **4. Pause All**                   | User clicks "Pause All"              | API → Orchestrator (bulk) → DB                    | Multiple StateChanged                             | All pauseable tasks updated           | All active downloads pause                   |
| **5. Resume All**                  | User clicks "Resume All"             | API → Orchestrator (bulk) → DB                    | Multiple StateChanged                             | All paused tasks updated              | All paused downloads resume                  |
| **6. Backend Restart**             | Server restarts                      | DB → Orchestrator.load_pending_tasks()            | None (recovery)                                   | QUEUED/PAUSED loaded to TaskManager   | Pending tasks restored                       |
| **7. Pause After Restart**         | User pauses task from before restart | API → Orchestrator → DB (task not in TaskManager) | StateChanged via unified lookup                   | Task updated in DB                    | UI updates via WebSocket (no silent failure) |
| **8. Download Failure**            | Network error during download        | Worker → Orchestrator → DB                        | Failed event                                      | State = FAILED, error_history updated | UI shows error, retry available              |
| **9. Retry Failed Task**           | User retries failed download         | API → Orchestrator → TaskManager                  | StateChanged (QUEUED)                             | retry_count++, state updated          | Task re-queued                               |
| **10. Delete Completed**           | User deletes completed task          | API → Orchestrator → DB                           | Removed event                                     | Task deleted from DB                  | UI removes task                              |
| **11. Concurrent Downloads**       | Multiple downloads active            | Workers → TaskManager (parallel)                  | Multiple ProgressUpdated                          | Real-time metrics in memory           | UI shows all progress bars                   |
| **12. Rate Limit Handling**        | Fshare rate limit hit                | Worker → Orchestrator → DB                        | StateChanged (WAITING), wait_until set            | State = WAITING                       | Task waits, then auto-retries                |

---

## User Journey Flows

### Journey 1: First-Time User - Single Download

```
1. User opens Flasharr
2. Pastes Fshare URL
3. Clicks "Add Download"
4. Watches progress bar fill
5. Download completes
6. File appears in destination folder
```

**Data Flow:**

```
POST /api/downloads/add
  ↓
Orchestrator.add_download_with_metadata()
  ↓
TaskManager.add_task() + DB.save_task()
  ↓
EventBus.publish(TaskEvent::Created)
  ↓
WebSocket → Frontend (TASK_ADDED)
  ↓
Worker picks up task
  ↓
EventBus.publish(TaskEvent::ProgressUpdated) [every 500ms]
  ↓
WebSocket → Frontend (TASK_UPDATED)
  ↓
EventBus.publish(TaskEvent::Completed)
  ↓
WebSocket → Frontend (TASK_UPDATED with state=COMPLETED)
```

---

### Journey 2: Power User - Batch Download (Smart Grab)

```
1. User searches for "Breaking Bad Season 1"
2. Clicks "Smart Grab" on season
3. System finds 13 episodes
4. All episodes added with same batch_id
5. UI shows collapsible group "Breaking Bad S01"
6. User expands to see individual episodes
7. All download simultaneously (up to max_concurrent)
```

**Data Flow:**

```
POST /api/discovery/smart-grab
  ↓
Generate batch_id = UUID
Generate batch_name = "Breaking Bad S01"
  ↓
For each episode:
  POST /api/downloads/add (with batch_id, batch_name, tmdb_metadata)
    ↓
  Orchestrator.add_download_with_metadata()
    ↓
  task.batch_id = batch_id
  task.batch_name = batch_name
  task.tmdb_id = 1396
  task.media_type = "tv"
  task.season = 1
  task.episode = i
    ↓
  EventBus.publish(TaskEvent::Created)
    ↓
  WebSocket → Frontend (TASK_ADDED)
    ↓
Frontend groups by batch_id in UI
```

---

### Journey 3: Critical Bug Fix - Pause After Restart

```
1. User has 10 downloads running
2. 5 complete, 5 still downloading
3. Server crashes/restarts
4. On restart: load_pending_tasks() loads QUEUED/PAUSED into TaskManager
5. Completed tasks are ONLY in DB (not in TaskManager)
6. User clicks "Pause All"
7. System queries DB for all pauseable tasks (finds 5 active + 0 completed)
8. For each task: broadcast_task_update()
9. WebSocket handler receives StateChanged event
10. WebSocket calls get_task_unified(task_id)
    - Checks TaskManager (miss for DB-only tasks)
    - Falls back to DB (finds task)
11. WebSocket sends TASK_UPDATED to frontend
12. UI updates instantly (NO SILENT FAILURE)
```

**Before Fix:**

```
WebSocket handler:
  orchestrator.task_manager().get_task(task_id) → None
  ↓
  return None  ← SILENT FAILURE, no message sent
  ↓
  Frontend never updates
```

**After Fix:**

```
WebSocket handler:
  orchestrator.get_task_unified(task_id)
    ↓
  TaskManager.get_task(task_id) → None
    ↓
  DB.get_task_by_id(task_id) → Some(task)
    ↓
  return Some(task)
    ↓
  WebSocket sends TASK_UPDATED
    ↓
  Frontend updates ✅
```

---

### Journey 4: Error Handling - Network Failure

```
1. Download starts successfully
2. Network drops mid-download
3. Worker detects error
4. Orchestrator classifies error (transient)
5. retry_count++
6. error_history.push(ErrorEntry)
7. If retry_count < 3:
   - State = WAITING
   - wait_until = now + exponential_backoff(retry_count)
8. EventBus.publish(TaskEvent::Failed)
9. WebSocket → Frontend shows error message
10. After wait_until expires:
    - Orchestrator auto-retries
    - EventBus.publish(TaskEvent::StateChanged to QUEUED)
```

---

## Test Data Requirements

### Minimal Test Dataset

```json
{
  "single_download": {
    "url": "https://www.fshare.vn/file/TESTCODE123",
    "filename": "test-file.mkv",
    "size": 1073741824,
    "category": "movies"
  },
  "batch_download": {
    "batch_id": "550e8400-e29b-41d4-a716-446655440000",
    "batch_name": "Breaking Bad S01",
    "episodes": [
      {
        "url": "https://www.fshare.vn/file/EP01CODE",
        "filename": "Breaking.Bad.S01E01.mkv",
        "tmdb_id": 1396,
        "media_type": "tv",
        "season": 1,
        "episode": 1
      }
      // ... 12 more episodes
    ]
  },
  "error_scenarios": [
    {
      "type": "network_timeout",
      "error_message": "Connection timeout after 30s",
      "category": "transient",
      "should_retry": true
    },
    {
      "type": "rate_limit",
      "error_message": "Fshare rate limit exceeded",
      "category": "rate_limit",
      "wait_seconds": 3600
    },
    {
      "type": "file_not_found",
      "error_message": "File has been deleted",
      "category": "permanent",
      "should_retry": false
    }
  ]
}
```

---

## WebSocket Event Assertions

### Expected Event Sequence for Single Download

```javascript
[
  { type: "TASK_ADDED", task: { id, state: "QUEUED" } },
  { type: "TASK_UPDATED", task: { id, state: "STARTING" } },
  { type: "TASK_UPDATED", task: { id, state: "DOWNLOADING", progress: 0 } },
  { type: "TASK_UPDATED", task: { id, state: "DOWNLOADING", progress: 25 } },
  { type: "TASK_UPDATED", task: { id, state: "DOWNLOADING", progress: 50 } },
  { type: "TASK_UPDATED", task: { id, state: "DOWNLOADING", progress: 75 } },
  { type: "TASK_UPDATED", task: { id, state: "DOWNLOADING", progress: 100 } },
  { type: "TASK_UPDATED", task: { id, state: "COMPLETED" } },
];
```

### Expected Event Sequence for Pause All

```javascript
[
  { type: "TASK_UPDATED", task: { id: "task1", state: "PAUSED" } },
  { type: "TASK_UPDATED", task: { id: "task2", state: "PAUSED" } },
  { type: "TASK_UPDATED", task: { id: "task3", state: "PAUSED" } },
  // ... all pauseable tasks
];
```

---

## Database State Verification

### After Batch Download

```sql
SELECT batch_id, batch_name, COUNT(*) as episode_count
FROM tasks
WHERE batch_id = '550e8400-e29b-41d4-a716-446655440000'
GROUP BY batch_id, batch_name;

-- Expected: 1 row with episode_count = 13
```

### After Error with Retry

```sql
SELECT id, state, retry_count, error_message,
       json_array_length(error_history) as error_count
FROM tasks
WHERE id = 'failed-task-id';

-- Expected: state = 'WAITING', retry_count = 1, error_count = 1
```

### After Backend Restart

```sql
-- Before restart: 10 tasks (5 DOWNLOADING, 5 COMPLETED)
-- After restart load_pending_tasks():
SELECT state, COUNT(*) FROM tasks GROUP BY state;

-- Expected in TaskManager: 0 tasks (DOWNLOADING are transient)
-- Expected in DB: 5 QUEUED (reset from DOWNLOADING), 5 COMPLETED
```

---

## Performance Benchmarks

| Metric                         | Target          | Critical Threshold |
| ------------------------------ | --------------- | ------------------ |
| WebSocket latency (event → UI) | < 50ms          | < 100ms            |
| get_task_unified() cache hit   | > 95%           | > 90%              |
| get_task_unified() DB fallback | < 10ms          | < 50ms             |
| Batch add (13 episodes)        | < 500ms         | < 1s               |
| Pause All (100 tasks)          | < 200ms         | < 500ms            |
| EventBus throughput            | > 1000 events/s | > 500 events/s     |

---

## Edge Cases to Test

1. **Empty Database**: First download after fresh install
2. **Corrupted Task**: Task in DB with invalid state
3. **Duplicate Detection**: Adding same Fshare URL twice
4. **Concurrent Pause/Resume**: User rapidly clicking pause/resume
5. **WebSocket Reconnect**: Client disconnects and reconnects mid-download
6. **Database Lock**: SQLite write contention under load
7. **EventBus Lag**: Slow subscriber causing channel to lag
8. **Memory Pressure**: 1000+ tasks in TaskManager
9. **Orphaned Tasks**: Tasks in DB but never loaded to TaskManager
10. **Race Condition**: Task completes while user clicks pause
