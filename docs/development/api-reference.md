# Flasharr API Reference

Complete reference for Flasharr's REST API and WebSocket protocol.

## Base URL

```
http://localhost:8484/api
```

## Authentication

Currently, Flasharr does not require authentication for local deployments. For production deployments, consider placing behind a reverse proxy with authentication.

## REST API Endpoints

### Health Check

**GET** `/health`

Check if the backend is running.

**Response:**

```json
{
  "status": "ok"
}
```

---

### Downloads

#### List Downloads

**GET** `/downloads`

Get all download tasks with optional filtering and pagination.

**Query Parameters:**

- `status` (optional): Filter by status (`queued`, `downloading`, `paused`, `completed`, `failed`)
- `page` (optional): Page number (default: 1)
- `limit` (optional): Items per page (default: 50)

**Response:**

```json
{
  "tasks": [
    {
      "id": "uuid",
      "filename": "example.mkv",
      "state": "downloading",
      "progress": 45.5,
      "speed": 5242880,
      "eta": 120,
      "batch_id": "uuid",
      "batch_name": "Breaking Bad S01",
      "tmdb_title": "Breaking Bad",
      "tmdb_season": 1,
      "tmdb_episode": 1
    }
  ],
  "total": 100,
  "page": 1,
  "pages": 2
}
```

#### Add Download

**POST** `/downloads`

Add a new download task.

**Request Body:**

```json
{
  "url": "https://fshare.vn/file/...",
  "batch_id": "uuid (optional)",
  "batch_name": "string (optional)"
}
```

**Response:**

```json
{
  "id": "uuid",
  "filename": "example.mkv",
  "state": "queued"
}
```

#### Get Download

**GET** `/downloads/:id`

Get details of a specific download task.

**Response:**

```json
{
  "id": "uuid",
  "filename": "example.mkv",
  "state": "downloading",
  "progress": 45.5,
  "speed": 5242880,
  "downloaded_bytes": 524288000,
  "total_bytes": 1152921504,
  "eta": 120,
  "created_at": "2026-02-04T10:00:00Z",
  "started_at": "2026-02-04T10:01:00Z"
}
```

#### Pause Download

**POST** `/downloads/:id/pause`

Pause a download task.

**Response:**

```json
{
  "id": "uuid",
  "state": "paused"
}
```

#### Resume Download

**POST** `/downloads/:id/resume`

Resume a paused download task.

**Response:**

```json
{
  "id": "uuid",
  "state": "queued"
}
```

#### Delete Download

**DELETE** `/downloads/:id`

Delete a download task.

**Response:**

```json
{
  "success": true
}
```

---

### Batches

#### List Batches

**GET** `/batches`

Get all batch summaries.

**Response:**

```json
{
  "batches": [
    {
      "batch_id": "uuid",
      "batch_name": "Breaking Bad S01",
      "total_items": 13,
      "completed_items": 5,
      "progress": 38.5,
      "total_size": 15032385536,
      "downloaded_size": 5784576000
    }
  ]
}
```

#### Batch Operations

**POST** `/batches/:batch_id/pause`

Pause all downloads in a batch.

**POST** `/batches/:batch_id/resume`

Resume all downloads in a batch.

**DELETE** `/batches/:batch_id`

Delete all downloads in a batch.

---

### Search

#### Smart Search

**POST** `/search/smart`

Search for media using TMDB metadata.

**Request Body:**

```json
{
  "title": "Breaking Bad",
  "year": "2008",
  "type": "tv",
  "tmdb_id": "1396",
  "season": 1
}
```

**Response:**

```json
{
  "seasons": [
    {
      "season": 1,
      "episodes_grouped": [
        {
          "episode_number": 1,
          "title": "Pilot",
          "files": [
            {
              "url": "https://fshare.vn/file/...",
              "quality": "1080p",
              "size": 1152921504
            }
          ]
        }
      ]
    }
  ]
}
```

#### Smart Grab

**POST** `/search/smart-grab`

Automatically queue all episodes from a smart search.

**Request Body:**

```json
{
  "title": "Breaking Bad",
  "year": "2008",
  "type": "tv",
  "tmdb_id": "1396",
  "season": 1
}
```

**Response:**

```json
{
  "batch_id": "uuid",
  "batch_name": "Breaking Bad S01",
  "tasks_created": 13
}
```

---

### Settings

#### Get Settings

**GET** `/settings`

Get current application settings.

**Response:**

```json
{
  "max_concurrent_downloads": 5,
  "download_directory": "/downloads",
  "fshare_configured": true,
  "tmdb_configured": true
}
```

#### Update Settings

**PUT** `/settings`

Update application settings.

**Request Body:**

```json
{
  "max_concurrent_downloads": 3
}
```

**Response:**

```json
{
  "success": true
}
```

---

### Engine Stats

**GET** `/engine/stats`

Get download engine statistics.

**Response:**

```json
{
  "active_downloads": 3,
  "queued": 5,
  "completed": 10,
  "failed": 1,
  "paused": 2,
  "total_speed": 15728640,
  "db_counts": {
    "queued": 5,
    "downloading": 3,
    "completed": 10,
    "failed": 1,
    "paused": 2
  }
}
```

---

## WebSocket API

### Connection

**Endpoint:** `ws://localhost:8484/api/ws`

### Message Types

All messages follow this format:

```json
{
  "type": "MESSAGE_TYPE",
  "...additional fields"
}
```

#### SYNC_ALL

Full synchronization of all tasks (sent on connection).

```json
{
  "type": "SYNC_ALL",
  "tasks": [
    /* array of DownloadTask */
  ]
}
```

#### TASK_ADDED

New task created.

```json
{
  "type": "TASK_ADDED",
  "task": {
    /* DownloadTask object */
  }
}
```

#### TASK_UPDATED

Task state or progress changed.

```json
{
  "type": "TASK_UPDATED",
  "task": {
    "id": "uuid",
    "state": "downloading",
    "progress": 45.5,
    "speed": 5242880
  }
}
```

#### TASK_BATCH_UPDATE

Batch of task updates (sent every 500ms for active downloads).

```json
{
  "type": "TASK_BATCH_UPDATE",
  "tasks": [
    /* array of partial DownloadTask updates */
  ]
}
```

#### TASK_REMOVED

Task deleted.

```json
{
  "type": "TASK_REMOVED",
  "task_id": "uuid"
}
```

#### ENGINE_STATS

Engine statistics (sent every 2 seconds).

```json
{
  "type": "ENGINE_STATS",
  "stats": {
    "active_downloads": 3,
    "queued": 5,
    "total_speed": 15728640
  }
}
```

---

## Error Responses

All endpoints return errors in this format:

```json
{
  "error": "Error message",
  "code": "ERROR_CODE"
}
```

### Common Error Codes

- `INVALID_REQUEST` - Malformed request body
- `NOT_FOUND` - Resource not found
- `FSHARE_ERROR` - Fshare API error
- `DOWNLOAD_ERROR` - Download failed
- `DATABASE_ERROR` - Database operation failed

### HTTP Status Codes

- `200` - Success
- `201` - Created
- `400` - Bad Request
- `404` - Not Found
- `500` - Internal Server Error

---

## Rate Limiting

Currently, no rate limiting is enforced. For production deployments, consider implementing rate limiting at the reverse proxy level.

## CORS

CORS is enabled for all origins in development. For production, configure appropriate CORS settings.

---

## Examples

### cURL Examples

**Add Download:**

```bash
curl -X POST http://localhost:8484/api/downloads \
  -H "Content-Type: application/json" \
  -d '{"url": "https://fshare.vn/file/..."}'
```

**Pause Download:**

```bash
curl -X POST http://localhost:8484/api/downloads/{id}/pause
```

**Get Stats:**

```bash
curl http://localhost:8484/api/engine/stats
```

### JavaScript Examples

**Add Download:**

```javascript
const response = await fetch("http://localhost:8484/api/downloads", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ url: "https://fshare.vn/file/..." }),
});
const task = await response.json();
```

**WebSocket Connection:**

```javascript
const ws = new WebSocket("ws://localhost:8484/api/ws");

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  console.log("Received:", message.type);
};
```

---

For more details on WebSocket protocol, see [WebSocket Protocol Documentation](../architecture/websocket-protocol.md).
