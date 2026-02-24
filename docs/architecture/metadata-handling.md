# Download Static Metadata - Architectural Decisions

## 🔗 Component Relationship Map

This table defines the communication boundaries between all components in the download architecture.

| From Component        | To Component                | Communication Type                    | Data Exchanged                      | Purpose                                          |
| :-------------------- | :-------------------------- | :------------------------------------ | :---------------------------------- | :----------------------------------------------- |
| **API Handler**       | **Orchestrator**            | Direct Method Call                    | Download requests, Control commands | Trigger download operations (add, pause, resume) |
| **API Handler**       | **Database**                | Direct Query                          | Pagination params, Filters          | Fetch task lists for UI display                  |
| **Orchestrator**      | **TaskManager**             | Direct Method Call                    | Task objects, State updates         | Manage in-memory task state                      |
| **Orchestrator**      | **Database**                | Async Method Call                     | Task objects, State snapshots       | Persist tasks for restart recovery               |
| **Orchestrator**      | **WebSocket (progress_tx)** | Broadcast Channel                     | `ProgressUpdate` signals            | Notify about state changes                       |
| **Orchestrator**      | **Workers**                 | Notify + Shared State                 | Task availability signal            | Wake workers for new tasks                       |
| **Orchestrator**      | **Host Registry**           | Direct Method Call                    | URLs                                | Resolve direct download links                    |
| **Workers**           | **TaskManager**             | Direct Method Call                    | Progress updates, State changes     | Update real-time metrics                         |
| **Workers**           | **Download Engine**         | Direct Method Call                    | URL, Destination, Callbacks         | Execute HTTP download                            |
| **WebSocket Handler** | **Orchestrator**            | Direct Method Call                    | Task ID lookups                     | Fetch full task for broadcasting                 |
| **WebSocket Handler** | **TaskManager**             | Direct Method Call (via Orchestrator) | Task ID                             | Fast lookup for active tasks                     |
| **WebSocket Handler** | **Database**                | Async Query (Fallback)                | Task ID                             | Lookup for DB-only tasks                         |
| **WebSocket Handler** | **Frontend**                | WebSocket Protocol                    | `WsMessage` (JSON)                  | Real-time UI updates                             |
| **Frontend Store**    | **API**                     | HTTP REST                             | Requests/Responses                  | Fetch paginated task lists                       |
| **Frontend Store**    | **WebSocket**               | WebSocket Protocol                    | Subscribe to updates                | Receive real-time state changes                  |

### Key Architectural Rules

1. **Database Access**: Only **Orchestrator** and **API Handler** may directly query the Database. Workers and WebSocket handlers must go through the Orchestrator.
2. **State Mutations**: Only **Orchestrator** (via Workers) may change task states. API handlers trigger state changes but don't modify directly.
3. **WebSocket Broadcasting**: Only **Orchestrator** sends to `progress_tx`. WebSocket handler is read-only (consumes broadcasts).
4. **TaskManager Isolation**: Only **Orchestrator** and **Workers** directly access TaskManager. All other components use Orchestrator methods.
5. **Frontend Separation**: Frontend never talks directly to Database or TaskManager—only through API (HTTP) and WebSocket.

## Metadata Summary & Clean-up

| Property         | Decision                       | Rationale                                                                                                                                          |
| :--------------- | :----------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------- |
| **fshare_code**  | **Useless / To be Deprecated** | We already save `original_url`. The code can be extracted on-the-fly via Regex whenever needed. Storing it separately adds unnecessary DB columns. |
| **url_metadata** | **Useless / To be Deprecated** | Fshare API does not return link expiration timestamps or metadata. Tracking this internally is guestwork and unreliable.                           |
| **batch_id**     | **Critical / Required**        | Required for UI grouping. Must be auto-generated during **Smart Grab** (Discovery) and **Arr Services** (Sonarr/Radarr) imports.                   |
| **batch_name**   | **Critical / Required**        | Human-readable name for the group (e.g., "Breaking Bad S01").                                                                                      |
| **category**     | **Static / Primary**           | Defines the top-level disk folder (e.g., `/downloads/movies`). If `media_type` exists, `category` should be auto-synced to match it.               |
| **media_type**   | **Static / Secondary**         | Comes from TMDB. Tells the system whether to look for Season/Episode or Collection metadata.                                                       |
| **size**         | **Static / Resolved**          | Total file size in bytes. Fetched once during creation/enrichment and saved to DB.                                                                 |

## 🔗 URL & Resolution

In this architecture, we strictly separate the **Statics** (Source) from the **Dynamics** (Temporary Link).

| Property  | Format                 | Usage / Explanation                                                                                                                                                           |
| :-------- | :--------------------- | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`url`** | `String` (Direct Link) | **Dynamic.** This is the temporary direct download link fetched from Fshare. It is NEVER the source of truth and only lives as long as the download is active or not expired. |

### Architectural Rules

1. **No Static Data in Resolution**: Properties like `original_url`, `filename`, and `fshare_code` (regex-only) belong to **Static Metadata**. They must not be recalculated or redefined during the Resolution phase.
2. **Resolution Responsibility**: The Orchestrator is the sole coordinator for URL resolution. It uses the `original_url` to fetch a fresh `url` (direct link) only when transitioning to the `STARTING` state.
3. **Link Expiry**: Since Fshare does not provide expiry metadata, the system treats the direct `url` as disposable. If a download fails with a 403 or 410, the Orchestrator simply re-resolves using the `original_url`.

## 📊 Real-time Metrics

These properties represent the live state of an active download.

| Property         | Format    | Storage | Usage                             |
| :--------------- | :-------- | :------ | :-------------------------------- |
| **`downloaded`** | Bytes     | Memory  | Amount of data currently on disk. |
| **`progress`**   | % (0-100) | Memory  | Percentage for UI progress bars.  |
| **`speed`**      | Bytes/s   | Memory  | Current transfer rate.            |
| **`eta`**        | Seconds   | Memory  | Estimated time remaining.         |

### Architectural Rules

1. **Volatile Data**: Metrics are treated as **volatile**. They are updated in **Memory (TaskManager)** at high frequency (~1s) for WebSocket broadcasting.
2. **Database Passive Snapshotting**: To protect disk I/O, metrics are **NEVER** written to the Database during active downloading. The DB only receives a "snapshot" of these values during state transitions (e.g., `Downloading` -> `Paused`).
3. **Resumption**: On application startup, the `downloaded` value from the DB snapshot is used as the starting point for the download engine's range-request (Resume).

## 🔄 State Tracking & Retry Logic

These properties control the lifecycle and retry behavior of a download task.

| Property          | Format    | Storage     | Usage                                                                                                        |
| :---------------- | :-------- | :---------- | :----------------------------------------------------------------------------------------------------------- |
| **`state`**       | Enum      | Memory + DB | Current lifecycle stage (`QUEUED`, `STARTING`, `DOWNLOADING`, `PAUSED`, `COMPLETED`, `FAILED`, `CANCELLED`). |
| **`retry_count`** | Integer   | Memory + DB | Number of retry attempts for this task.                                                                      |
| **`wait_until`**  | Timestamp | Memory + DB | Future timestamp when the next retry is allowed (exponential backoff).                                       |

### Architectural Rules

1. **Orchestrator as State Machine**: Only the Orchestrator (via Background Workers) may transition task states. API endpoints trigger state changes but don't modify state directly.
2. **Transient State Recovery**: On startup, transient states (`STARTING`, `DOWNLOADING`) are downgraded to `QUEUED` to ensure they're re-processed.
3. **Exponential Backoff**: Failed tasks increment `retry_count` and set `wait_until` = Now + (30s × 2^retry_count). Workers skip tasks where `CurrentTime < wait_until`.
4. **Persistence**: All state properties are saved to DB on every state transition to survive restarts.

## ❌ Error Handling

Error information helps users understand why downloads fail.

| Property            | Format     | Storage     | Usage                                                    |
| :------------------ | :--------- | :---------- | :------------------------------------------------------- |
| **`error_message`** | String     | Memory + DB | The most recent error message.                           |
| **`error_history`** | JSON Array | DB Only     | Last 3 errors with timestamps. Stored as JSON in SQLite. |

### Architectural Rules

1. **Limited History**: Only the last 3 errors are retained to prevent DB bloat. Each error entry contains `{timestamp, message, retry_count}`.
2. **UI Display**: The frontend shows `error_message` prominently. The full `error_history` is available in a details/tooltip view.
3. **Error Classification**: The Orchestrator categorizes errors (network, auth, disk space) to determine retry eligibility.

## 🎛️ Runtime Control

These properties exist only in memory and control active download execution.

| Property           | Format              | Storage     | Usage                                                            |
| :----------------- | :------------------ | :---------- | :--------------------------------------------------------------- |
| **`cancel_token`** | `CancellationToken` | Memory Only | Used by workers to abort HTTP requests when user pauses/cancels. |
| **`pause_notify`** | `Arc<Notify>`       | Memory Only | Async notification channel for pause/resume coordination.        |

### Architectural Rules

1. **Non-Persistent**: These are Rust runtime objects (Arc pointers) that **cannot** be serialized to SQLite.
2. **Lifecycle**: Created when a task enters `STARTING` state, dropped when task reaches terminal state (`COMPLETED`, `FAILED`, `CANCELLED`).
3. **Thread Safety**: All runtime controls use `Arc` for safe sharing across worker threads.

## ⚙️ Logic Adjustments Needed

### 1. Batching in Discovery

When a user uses **Smart Grab** on a series or a collection, the Orchestrator must generate a unique `batch_id` and a descriptive `batch_name` to group all related episode/movie files.

### 2. Arr Integration

Ensure Sonarr and Radarr imports correctly pass or trigger the creation of batch metadata so the Downloads tab stays organized.

### 3. Fshare Link Handling

Rely solely on `original_url` as the source of truth for the resource identity. Any logic requiring the file code should perform a regex extraction from this URL.
