# API Documentation

Flasharr provides a RESTful API and WebSocket interface for programmatic access.

## Base URL

```
http://localhost:8484/api
```

## Authentication

Most endpoints require an API key. Generate one in Settings → Indexer.

Include the API key in requests:

- **Query Parameter**: `?apikey=YOUR_API_KEY`
- **Header**: `X-Api-Key: YOUR_API_KEY`

## Endpoints

### Health Check

**GET** `/health`

Check if the service is running.

**Response**:

```json
{
  "status": "healthy",
  "version": "2.0.0"
}
```

### Search

**GET** `/api/search`

Search for media across configured hosts.

**Parameters**:

- `q` (string, required): Search query
- `type` (string, optional): `movie` or `tv`

**Example**:

```bash
curl "http://localhost:8484/api/search?q=Inception&type=movie"
```

**Response**:

```json
{
  "results": [
    {
      "id": "550",
      "title": "Inception",
      "year": 2010,
      "type": "movie",
      "poster_path": "/path/to/poster.jpg"
    }
  ]
}
```

### Downloads

#### List Downloads

**GET** `/api/downloads`

Get all active and queued downloads.

**Response**:

```json
{
  "downloads": [
    {
      "id": "uuid",
      "filename": "movie.mkv",
      "status": "downloading",
      "progress": 45.5,
      "speed": "5.2 MB/s",
      "eta": "2m 30s"
    }
  ]
}
```

#### Start Download

**POST** `/api/downloads`

Start a new download.

**Body**:

```json
{
  "url": "https://fshare.vn/file/...",
  "filename": "movie.mkv"
}
```

**Response**:

```json
{
  "id": "uuid",
  "status": "queued"
}
```

#### Pause Download

**POST** `/api/downloads/{id}/pause`

Pause an active download.

#### Resume Download

**POST** `/api/downloads/{id}/resume`

Resume a paused download.

#### Cancel Download

**DELETE** `/api/downloads/{id}`

Cancel and remove a download.

### Indexer (Newznab API)

**GET** `/api/indexer/caps`

Get indexer capabilities (for Sonarr/Radarr).

**Parameters**:

- `apikey` (required): Your API key

**GET** `/api/indexer/api`

Search via Newznab protocol.

**Parameters**:

- `apikey` (required): Your API key
- `t` (required): Function (`search`, `tvsearch`, `movie`)
- `q`: Search query
- `imdbid`: IMDB ID
- `tmdbid`: TMDB ID
- `season`: Season number (for TV)
- `ep`: Episode number (for TV)

**Example**:

```bash
curl "http://localhost:8484/api/indexer/api?apikey=KEY&t=movie&tmdbid=550"
```

### Settings

#### Get Settings

**GET** `/api/settings`

Get current application settings.

**Response**:

```json
{
  "download_directory": "/appData/downloads",
  "max_concurrent_downloads": 3,
  "fshare_configured": true
}
```

#### Update Settings

**POST** `/api/settings`

Update application settings.

**Body**:

```json
{
  "max_concurrent_downloads": 5
}
```

### Accounts

#### Check FShare Status

**GET** `/api/accounts/fshare/status`

Get FShare account information.

**Response**:

```json
{
  "email": "user@example.com",
  "account_type": "VIP",
  "is_premium": true,
  "expiry_date": "2025-12-31"
}
```

## WebSocket API

Connect to real-time updates:

```
ws://localhost:8484/ws
```

### Events

#### Download Progress

```json
{
  "type": "download_progress",
  "data": {
    "id": "uuid",
    "progress": 50.5,
    "speed": "6.2 MB/s",
    "eta": "1m 45s"
  }
}
```

#### Download Complete

```json
{
  "type": "download_complete",
  "data": {
    "id": "uuid",
    "filename": "movie.mkv",
    "path": "/appData/downloads/movie.mkv"
  }
}
```

#### Download Error

```json
{
  "type": "download_error",
  "data": {
    "id": "uuid",
    "error": "Connection timeout"
  }
}
```

## Error Responses

All errors follow this format:

```json
{
  "error": "Error message",
  "code": "ERROR_CODE"
}
```

### Common Error Codes

| Code              | Description                |
| ----------------- | -------------------------- |
| `UNAUTHORIZED`    | Invalid or missing API key |
| `NOT_FOUND`       | Resource not found         |
| `INVALID_REQUEST` | Invalid request parameters |
| `INTERNAL_ERROR`  | Server error               |
| `RATE_LIMITED`    | Too many requests          |

## Rate Limiting

API requests are limited to:

- **100 requests per minute** per IP
- **1000 requests per hour** per API key

## Examples

### Python

```python
import requests

API_URL = "http://localhost:8484/api"
API_KEY = "your-api-key"

# Search for a movie
response = requests.get(
    f"{API_URL}/search",
    params={"q": "Inception", "type": "movie"}
)
results = response.json()

# Start a download
response = requests.post(
    f"{API_URL}/downloads",
    json={
        "url": "https://fshare.vn/file/...",
        "filename": "movie.mkv"
    },
    headers={"X-Api-Key": API_KEY}
)
download = response.json()
```

### JavaScript

```javascript
const API_URL = "http://localhost:8484/api";
const API_KEY = "your-api-key";

// Search for a movie
const search = await fetch(`${API_URL}/search?q=Inception&type=movie`);
const results = await search.json();

// WebSocket connection
const ws = new WebSocket("ws://localhost:8484/ws");
ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log("Download update:", data);
};
```

### cURL

```bash
# Search
curl "http://localhost:8484/api/search?q=Inception&type=movie"

# Start download
curl -X POST http://localhost:8484/api/downloads \
  -H "X-Api-Key: your-api-key" \
  -H "Content-Type: application/json" \
  -d '{"url":"https://fshare.vn/file/...","filename":"movie.mkv"}'

# Get downloads
curl -H "X-Api-Key: your-api-key" \
  http://localhost:8484/api/downloads
```
