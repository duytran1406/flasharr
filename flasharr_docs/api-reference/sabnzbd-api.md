# SABnzbd API Reference

Flasharr emulates the SABnzbd API to integrate with Radarr, Sonarr, and other *arr applications as a download client.

---

## Base URL

```
http://localhost:8484/sabnzbd
```

---

## Authentication

The API key is optional. You can use any value or leave it empty in your *arr configuration.

---

## Endpoints

### 1. Get Version (`?mode=version`)

Returns the SABnzbd version (emulated).

**Request:**
```http
GET /sabnzbd/api?mode=version
```

**Response:**
```json
{
  "version": "3.5.0"
}
```

---

### 2. Add URL (`?mode=addurl`)

Add a download from a URL (Fshare link).

**Request:**
```http
POST /sabnzbd/api?mode=addurl&name={url}&cat={category}&priority={priority}
```

**Parameters:**
| Parameter | Required | Description |
|-----------|----------|-------------|
| `mode` | Yes | Must be `addurl` |
| `name` | Yes | Fshare URL |
| `cat` | No | Category (movies, tv) |
| `priority` | No | Priority (-1=Low, 0=Normal, 1=High, 2=Force) |
| `apikey` | No | API key (optional) |

**Example:**
```bash
curl -X POST "http://localhost:8484/sabnzbd/api?mode=addurl&name=https://fshare.vn/file/ABC123&cat=movies"
```

**Response:**
```json
{
  "status": true,
  "nzo_ids": ["abc123-def456-ghi789"]
}
```

---

### 3. Add File (`?mode=addfile`)

Add a download from an NZB file upload.

**Request:**
```http
POST /sabnzbd/api?mode=addfile&cat={category}
Content-Type: multipart/form-data

nzbfile: <file data>
```

**Parameters:**
| Parameter | Required | Description |
|-----------|----------|-------------|
| `mode` | Yes | Must be `addfile` |
| `nzbfile` | Yes | NZB file (multipart upload) |
| `cat` | No | Category (movies, tv) |
| `priority` | No | Priority |

**Example:**
```bash
curl -X POST "http://localhost:8484/sabnzbd/api?mode=addfile&cat=movies" \
  -F "nzbfile=@movie.nzb"
```

**Response:**
```json
{
  "status": true,
  "nzo_ids": ["abc123-def456-ghi789"]
}
```

---

### 4. Get Queue (`?mode=queue`)

Get the current download queue.

**Request:**
```http
GET /sabnzbd/api?mode=queue&start={start}&limit={limit}
```

**Parameters:**
| Parameter | Required | Description |
|-----------|----------|-------------|
| `mode` | Yes | Must be `queue` |
| `start` | No | Start index (default: 0) |
| `limit` | No | Max items (default: 100) |

**Example:**
```bash
curl "http://localhost:8484/sabnzbd/api?mode=queue"
```

**Response:**
```json
{
  "queue": {
    "status": "Downloading",
    "speed": "10.5 MB/s",
    "size": "15.2 GB",
    "sizeleft": "5.3 GB",
    "noofslots": 3,
    "slots": [
      {
        "nzo_id": "abc123",
        "filename": "Movie.2023.1080p.mkv",
        "status": "Downloading",
        "percentage": 65,
        "mb": "2048.00",
        "mbleft": "716.80",
        "mbmissing": "0.00",
        "size": "2.0 GB",
        "sizeleft": "700 MB",
        "eta": "00:01:15",
        "priority": "Normal",
        "cat": "movies"
      }
    ]
  }
}
```

---

### 5. Get History (`?mode=history`)

Get download history.

**Request:**
```http
GET /sabnzbd/api?mode=history&start={start}&limit={limit}
```

**Parameters:**
| Parameter | Required | Description |
|-----------|----------|-------------|
| `mode` | Yes | Must be `history` |
| `start` | No | Start index (default: 0) |
| `limit` | No | Max items (default: 100) |

**Example:**
```bash
curl "http://localhost:8484/sabnzbd/api?mode=history&limit=10"
```

**Response:**
```json
{
  "history": {
    "total_size": "45.6 GB",
    "noofslots": 15,
    "slots": [
      {
        "nzo_id": "xyz789",
        "name": "TV.Show.S01E01.1080p.mkv",
        "status": "Completed",
        "bytes": "1073741824",
        "category": "tv",
        "download_time": 450,
        "completed": 1705276800,
        "fail_message": "",
        "storage": "/downloads/tv/TV.Show.S01E01.1080p.mkv"
      }
    ]
  }
}
```

---

### 6. Pause Queue (`?mode=pause`)

Pause all downloads.

**Request:**
```http
POST /sabnzbd/api?mode=pause
```

**Response:**
```json
{
  "status": true
}
```

---

### 7. Resume Queue (`?mode=resume`)

Resume all downloads.

**Request:**
```http
POST /sabnzbd/api?mode=resume
```

**Response:**
```json
{
  "status": true
}
```

---

### 8. Pause Item (`?mode=queue&name=pause&value={nzo_id}`)

Pause a specific download.

**Request:**
```http
POST /sabnzbd/api?mode=queue&name=pause&value={nzo_id}
```

**Response:**
```json
{
  "status": true
}
```

---

### 9. Resume Item (`?mode=queue&name=resume&value={nzo_id}`)

Resume a specific download.

**Request:**
```http
POST /sabnzbd/api?mode=queue&name=resume&value={nzo_id}
```

**Response:**
```json
{
  "status": true
}
```

---

### 10. Delete Item (`?mode=queue&name=delete&value={nzo_id}`)

Remove a download from the queue.

**Request:**
```http
POST /sabnzbd/api?mode=queue&name=delete&value={nzo_id}&del_files=1
```

**Parameters:**
| Parameter | Description |
|-----------|-------------|
| `del_files` | 1 = delete files, 0 = keep files |

**Response:**
```json
{
  "status": true
}
```

---

## Download States

| State | Description |
|-------|-------------|
| `Queued` | Waiting to start |
| `Downloading` | Currently downloading |
| `Paused` | Paused by user |
| `Completed` | Successfully completed |
| `Failed` | Download failed |

---

## Priority Levels

| Value | Priority | Description |
|-------|----------|-------------|
| -1 | Low | Download last |
| 0 | Normal | Default priority |
| 1 | High | Download before normal |
| 2 | Force | Download immediately |

---

## Integration Examples

### Radarr Configuration

1. **Settings** → **Download Clients** → **Add** → **SABnzbd**
2. Configure:
   ```
   Name: Flasharr
   Host: flasharr (or localhost)
   Port: 8484
   URL Base: /sabnzbd
   API Key: (any value or empty)
   Category: movies
   Priority: Normal
   ```

### Sonarr Configuration

1. **Settings** → **Download Clients** → **Add** → **SABnzbd**
2. Configure:
   ```
   Name: Flasharr
   Host: flasharr (or localhost)
   Port: 8484
   URL Base: /sabnzbd
   API Key: (any value or empty)
   Category: tv
   Priority: Normal
   ```

---

## Manual Testing

```bash
# Add download
curl -X POST "http://localhost:8484/sabnzbd/api?mode=addurl&name=https://fshare.vn/file/ABC123&cat=movies"

# Check queue
curl "http://localhost:8484/sabnzbd/api?mode=queue"

# Check history
curl "http://localhost:8484/sabnzbd/api?mode=history&limit=5"

# Pause all
curl -X POST "http://localhost:8484/sabnzbd/api?mode=pause"

# Resume all
curl -X POST "http://localhost:8484/sabnzbd/api?mode=resume"
```

---

## Error Responses

### Invalid Mode

```json
{
  "error": "Invalid mode"
}
```

### Missing Parameters

```json
{
  "error": "Missing required parameter: name"
}
```

### Download Failed

```json
{
  "status": false,
  "error": "Failed to add download: Invalid Fshare URL"
}
```

---

## Notes

> [!NOTE]
> Flasharr emulates SABnzbd 3.5.0 for maximum compatibility with *arr applications.

> [!TIP]
> Use categories to organize downloads. Radarr typically uses `movies`, Sonarr uses `tv`.

> [!WARNING]
> The `del_files` parameter actually deletes files from disk. Use with caution.

---

## Next Steps

- [Newznab API](newznab-api.md) - Indexer integration
- [Engine API](engine-api.md) - Direct download management
- [Download Management](../user-guide/download-management.md) - Managing downloads via UI
