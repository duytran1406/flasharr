# Flasharr \*arr Integration Guide

## Overview

Flasharr integrates with Sonarr and Radarr by **simulating two different APIs**:

1. **Newznab API** - For media search/indexing
2. **SABnzbd API** - For download client functionality

```
┌─────────────────────────────────────────────────────────────┐
│                    Sonarr / Radarr                          │
│                                                             │
│  ┌──────────────┐              ┌──────────────┐            │
│  │   Indexer    │              │   Download   │            │
│  │   (Search)   │              │    Client    │            │
│  └──────┬───────┘              └──────┬───────┘            │
└─────────┼──────────────────────────────┼──────────────────┘
          │                              │
          │ Newznab API                  │ SABnzbd API
          │ /api/indexer                 │ /sabnzbd/api
          │                              │
┌─────────▼──────────────────────────────▼──────────────────┐
│                      Flasharr                              │
│                                                            │
│  ┌──────────────┐              ┌──────────────┐           │
│  │   Newznab    │              │   SABnzbd    │           │
│  │  Endpoints   │              │  Endpoints   │           │
│  └──────┬───────┘              └──────┬───────┘           │
│         │                             │                   │
│         └─────────────┬───────────────┘                   │
│                       │                                   │
│              ┌────────▼────────┐                          │
│              │  Download       │                          │
│              │  Orchestrator   │                          │
│              └────────┬────────┘                          │
│                       │                                   │
│              ┌────────▼────────┐                          │
│              │  Fshare Host    │                          │
│              │  Integration    │                          │
│              └─────────────────┘                          │
└────────────────────────────────────────────────────────────┘
```

## API Endpoints

### Newznab API (`/api/indexer`)

**Purpose**: Allow Sonarr/Radarr to search for media

**Endpoints**:

- `GET /api/indexer?t=caps` - Get indexer capabilities
- `GET /api/indexer?t=search&q=...` - Search for media
- `GET /api/indexer?t=tvsearch&tvdbid=...` - Search TV shows
- `GET /api/indexer?t=movie&imdbid=...` - Search movies

### SABnzbd API (`/sabnzbd/api`)

**Purpose**: Allow Sonarr/Radarr to manage downloads

**Supported Modes**:

- `mode=addurl` - Add download to queue
- `mode=queue` - Get current download queue
- `mode=history` - Get download history
- `mode=pause` - Pause a download
- `mode=resume` - Resume a download
- `mode=delete` - Delete a download
- `mode=version` - Get SABnzbd version (returns "3.0.0")

## Configuration Steps

### Step 1: Get API Key from Flasharr

1. Open Flasharr web UI: `http://localhost:8484`
2. Navigate to **Settings** → **Services** tab
3. Find the **Newznab** section
4. Copy the **API Key** (or generate a new one)

### Step 2: Configure Sonarr/Radarr as Indexer

**In Sonarr/Radarr UI:**

1. Go to **Settings** → **Indexers**
2. Click **Add Indexer** → **Newznab**
3. Fill in:
   - **Name**: `Flasharr Indexer`
   - **URL**: `http://flasharr:8484/api/indexer`
   - **API Key**: `<paste-your-api-key>`
   - **Categories**: Leave default
4. Click **Test** (should show green checkmark)
5. Click **Save**

### Step 3: Configure Sonarr/Radarr as Download Client

**In Sonarr/Radarr UI:**

1. Go to **Settings** → **Download Clients**
2. Click **Add Download Client** → **SABnzbd**
3. Fill in:
   - **Name**: `Flasharr`
   - **Enable**: ✓ (checked)
   - **Host**: `flasharr` (or `localhost` if not using Docker)
   - **Port**: `8484`
   - **API Key**: `<paste-your-api-key>`
   - **URL Base**: `/sabnzbd`
   - **Use SSL**: ☐ (unchecked, unless using HTTPS)
   - **Category**: `tv` (for Sonarr) or `movies` (for Radarr)
4. Click **Test** (should show green checkmark)
5. Click **Save**

## How It Works

### Workflow: Automatic Download via Sonarr

```
1. User adds TV show to Sonarr
   ↓
2. Sonarr searches indexers (including Flasharr)
   → GET /api/indexer?t=tvsearch&tvdbid=12345
   ↓
3. Flasharr returns available episodes
   ← XML response with download links
   ↓
4. Sonarr selects best match and sends to download client
   → POST /sabnzbd/api?mode=addurl&name=<fshare-url>&nzbname=...
   ↓
5. Flasharr adds to download queue
   ← Returns task ID
   ↓
6. Sonarr monitors download progress
   → GET /sabnzbd/api?mode=queue
   ↓
7. Flasharr downloads file from Fshare
   ↓
8. When complete, Sonarr imports the file
   → GET /sabnzbd/api?mode=history
```

## Docker Network Setup

### Same Network (Recommended)

```yaml
version: "3.8"

services:
  flasharr:
    image: ghcr.io/duytran1406/flasharr:latest
    container_name: flasharr
    ports:
      - "8484:8484"
    networks:
      - media
    volumes:
      - ./flasharr/appData:/appData

  sonarr:
    image: linuxserver/sonarr:latest
    container_name: sonarr
    ports:
      - "8989:8989"
    networks:
      - media
    volumes:
      - ./sonarr/config:/config
      - ./flasharr/appData/downloads:/downloads

  radarr:
    image: linuxserver/radarr:latest
    container_name: radarr
    ports:
      - "7878:7878"
    networks:
      - media
    volumes:
      - ./radarr/config:/config
      - ./flasharr/appData/downloads:/downloads

networks:
  media:
    driver: bridge
```

**Key Points**:

- All containers on same `media` network
- Use container names as hostnames (`flasharr`, `sonarr`, `radarr`)
- Share download directory between Flasharr and \*arr apps

## Testing the Integration

### Test 1: Network Connectivity

```bash
# From Sonarr container
docker exec sonarr ping -c 3 flasharr

# From Radarr container
docker exec radarr ping -c 3 flasharr
```

**Expected**: Successful ping responses

### Test 2: API Accessibility

```bash
# Test Newznab API
curl "http://flasharr:8484/api/indexer?t=caps"

# Test SABnzbd API
curl "http://flasharr:8484/sabnzbd/api?mode=version"
```

**Expected**: XML/JSON responses

### Test 3: Manual Download

**In Sonarr/Radarr**:

1. Search for a TV show/movie
2. Click **Manual Search**
3. Select a result from Flasharr indexer
4. Click download icon
5. Check Flasharr UI → Downloads tab

**Expected**: Download appears in Flasharr queue

## Troubleshooting

### Issue: "Connection refused"

**Cause**: Containers not on same network or wrong hostname

**Solution**:

```bash
# Check network
docker network inspect bridge

# Verify containers are listed
docker ps

# Test connectivity
docker exec sonarr ping flasharr
```

### Issue: "Invalid API key"

**Cause**: API key mismatch

**Solution**:

1. Copy exact API key from Flasharr Settings
2. Paste into Sonarr/Radarr (no extra spaces)
3. If still failing, regenerate API key in Flasharr

### Issue: "Test failed" in Sonarr/Radarr

**Cause**: Wrong URL or port

**Solution**:

- Verify URL: `http://flasharr:8484/api/indexer` (indexer)
- Verify URL base: `/sabnzbd` (download client)
- Check port: `8484`
- Ensure Flasharr is running: `docker ps | grep flasharr`

### Issue: Downloads not starting

**Cause**: Fshare credentials not configured

**Solution**:

1. Open Flasharr → Settings → Account
2. Enter Fshare email and password
3. Click **Test Connection**
4. Ensure VIP account is active

## Advanced Configuration

### Custom Categories

Configure different download paths per category:

**In Flasharr** (`config.toml`):

```toml
[download]
base_dir = "/appData/downloads"

[download.categories]
tv = "TV Shows"
movies = "Movies"
anime = "Anime"
```

**In Sonarr/Radarr**:

- Set **Category** to `tv`, `movies`, or `anime`
- Files will be organized accordingly

### Auto-Import Configuration

**In Flasharr Settings → Services**:

1. Enable **Auto-Import** for Sonarr/Radarr
2. Enter Sonarr/Radarr URL and API key
3. When download completes, Flasharr notifies \*arr to import

**Benefits**:

- Faster import (no polling needed)
- Immediate library updates
- Better error handling

## API Reference

### SABnzbd Compatibility

Flasharr implements these SABnzbd API endpoints:

| Endpoint                    | Method   | Description      |
| --------------------------- | -------- | ---------------- |
| `/sabnzbd/api?mode=addurl`  | GET/POST | Add download     |
| `/sabnzbd/api?mode=queue`   | GET      | Get queue status |
| `/sabnzbd/api?mode=history` | GET      | Get history      |
| `/sabnzbd/api?mode=pause`   | GET/POST | Pause download   |
| `/sabnzbd/api?mode=resume`  | GET/POST | Resume download  |
| `/sabnzbd/api?mode=delete`  | GET/POST | Delete download  |
| `/sabnzbd/api?mode=version` | GET      | Get version      |

### Response Format

**Queue Response**:

```json
{
  "status": true,
  "queue": {
    "slots": [
      {
        "nzo_id": "uuid",
        "filename": "Show.S01E01.mkv",
        "percentage": "45.2",
        "mb": "1024.00",
        "mbleft": "560.64",
        "status": "Downloading",
        "timeleft": "120s"
      }
    ],
    "speed": "8.50 MB/s",
    "size": "1024.00 MB",
    "sizeleft": "560.64 MB"
  }
}
```

## Summary

✅ **Flasharr simulates SABnzbd** to integrate with Sonarr/Radarr  
✅ **No actual SABnzbd installation needed**  
✅ **Same API key for both indexer and download client**  
✅ **Works seamlessly in Docker networks**  
✅ **Supports all standard \*arr operations**

For more help, check the [main README](../README.md) or open an issue on GitHub.
