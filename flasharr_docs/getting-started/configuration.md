# Configuration Guide

Complete reference for configuring Flasharr.

---

## Environment Variables

All configuration is done via environment variables in `/mnt/appdata/Flasharr/.env`.

### Fshare Credentials

```bash
# Single Account
FSHARE_EMAIL=your-email@example.com
FSHARE_PASSWORD=your-password

# Multi-Account (comma-separated)
FSHARE_ACCOUNTS=email1:pass1,email2:pass2,email3:pass3
```

> [!TIP]
> Use multi-account mode to distribute downloads and avoid quota limits.

---

### Server Configuration

```bash
# Web Server
INDEXER_PORT=8484          # Port for web interface and APIs
HOST=0.0.0.0               # Bind address (0.0.0.0 for all interfaces)

# Logging
LOG_LEVEL=INFO             # DEBUG, INFO, WARNING, ERROR
LOG_FILE=/app/data/flasharr.log
```

---

### Download Configuration

```bash
# Download Engine
DOWNLOAD_DIR=/downloads                # Base download directory
MAX_CONCURRENT_DOWNLOADS=2             # Max simultaneous downloads
SEGMENTS_PER_DOWNLOAD=4                # Segments per file (multi-threading)

# Dynamic Scaling
ENABLE_DYNAMIC_SCALING=true            # Auto-adjust segments based on file size
MIN_SEGMENTS=2                         # Minimum segments
MAX_SEGMENTS=8                         # Maximum segments

# Speed Limiting
GLOBAL_SPEED_LIMIT_MBPS=0              # 0 = unlimited, or set MB/s limit
```

> [!NOTE]
> More segments = faster downloads but higher CPU/memory usage.

---

### Priority Configuration

```bash
# Auto-Prioritization
AUTO_PRIORITIZE_SMALL_FILES=true       # Files < 100MB get HIGH priority
SMALL_FILE_THRESHOLD_MB=100            # Threshold for auto-prioritization
```

---

### Cleanup Configuration

```bash
# Auto-Cleanup
ENABLE_AUTO_CLEANUP=true               # Enable automatic history cleanup
RETENTION_DAYS_SUCCESS=7               # Keep successful downloads for 7 days
RETENTION_DAYS_FAILED=30               # Keep failed downloads for 30 days
CLEANUP_INTERVAL_HOURS=24              # Run cleanup every 24 hours
```

---

### Advanced Configuration

```bash
# Link Checking
ENABLE_LINK_CHECK=true                 # Pre-check links before download
LINK_CHECK_CACHE_TTL=300               # Cache results for 5 minutes

# WebSocket
ENABLE_WEBSOCKET=true                  # Enable real-time updates
WEBSOCKET_PING_INTERVAL=30             # Ping interval in seconds

# Database
DB_PATH=/app/data/downloads.db         # SQLite database path
```

---

## Configuration Examples

### Home Network (Bandwidth Conscious)

```bash
# Limit speed during peak hours
GLOBAL_SPEED_LIMIT_MBPS=5
MAX_CONCURRENT_DOWNLOADS=1
SEGMENTS_PER_DOWNLOAD=2
```

### Server (Maximum Performance)

```bash
# Unlimited speed, max concurrency
GLOBAL_SPEED_LIMIT_MBPS=0
MAX_CONCURRENT_DOWNLOADS=4
SEGMENTS_PER_DOWNLOAD=8
ENABLE_DYNAMIC_SCALING=true
```

### Multi-Account Load Balancing

```bash
# Use 3 VIP accounts
FSHARE_ACCOUNTS=vip1@email.com:pass1,vip2@email.com:pass2,vip3@email.com:pass3
MAX_CONCURRENT_DOWNLOADS=3  # One per account
```

---

## *arr Integration Configuration

### Prowlarr Indexer Settings

**Indexer Configuration:**
- **Type:** Newznab
- **URL:** `http://flasharr:8484/indexer`
- **API Path:** `/api`
- **API Key:** (optional, any value)
- **Categories:** 
  - Movies: 2000
  - TV: 5000
- **Capabilities:** Automatic

**Search Settings:**
- **Minimum Seeders:** 0 (not applicable)
- **Required Flags:** None

---

### Radarr Download Client Settings

**SABnzbd Configuration:**
- **Type:** SABnzbd
- **Host:** `flasharr` (or `localhost`)
- **Port:** `8484`
- **URL Base:** `/sabnzbd`
- **API Key:** (optional)
- **Category:** `movies`
- **Priority:** Normal

**Advanced:**
- **Remove Completed:** Yes
- **Remove Failed:** No (for retry)

---

### Sonarr Download Client Settings

**SABnzbd Configuration:**
- **Type:** SABnzbd
- **Host:** `flasharr` (or `localhost`)
- **Port:** `8484`
- **URL Base:** `/sabnzbd`
- **API Key:** (optional)
- **Category:** `tv`
- **Priority:** Normal

**Advanced:**
- **Remove Completed:** Yes
- **Remove Failed:** No (for retry)

---

## Docker Compose Configuration

### Basic Setup

```yaml
services:
  flasharr:
    image: flasharr:beta
    container_name: flasharr
    restart: unless-stopped
    ports:
      - "8484:8484"
    env_file:
      - /mnt/appdata/Flasharr/.env
    volumes:
      - /mnt/appdata/Flasharr/data:/app/data
      - /data/fshare-downloader:/downloads
```

### With Custom Network

```yaml
services:
  flasharr:
    image: flasharr:beta
    container_name: flasharr
    restart: unless-stopped
    networks:
      - arr-network
    ports:
      - "8484:8484"
    env_file:
      - /mnt/appdata/Flasharr/.env
    volumes:
      - /mnt/appdata/Flasharr/data:/app/data
      - /data/fshare-downloader:/downloads

networks:
  arr-network:
    external: true
```

---

## Runtime Configuration

Some settings can be changed at runtime via the web UI or API.

### Via Web UI

1. Navigate to `http://localhost:8484/settings`
2. Modify settings
3. Click **Save**

### Via API

```bash
# Set global speed limit
curl -X POST http://localhost:8484/api/engine/speed-limit \
  -H "Content-Type: application/json" \
  -d '{"limit_mbps": 10}'

# Update max concurrent downloads
curl -X POST http://localhost:8484/api/engine/config \
  -H "Content-Type: application/json" \
  -d '{"max_concurrent": 3}'
```

---

## Validation

### Test Configuration

```bash
# Check environment variables
docker-compose exec flasharr env | grep FSHARE

# Test Fshare login
docker-compose logs flasharr | grep "login"

# Verify API endpoints
curl http://localhost:8484/api/version
curl http://localhost:8484/indexer/api?t=caps
```

---

## Troubleshooting Configuration

### Changes Not Applied

**Restart container:**
```bash
docker-compose restart flasharr
```

### Invalid Credentials

**Check logs:**
```bash
docker-compose logs flasharr | grep -i "auth\|login"
```

### Port Conflicts

**Change port in docker-compose.yml:**
```yaml
ports:
  - "8585:8484"  # Use 8585 instead
```

---

## Next Steps

- [Web Interface Guide](../user-guide/web-interface.md)
- [Multi-Account Setup](../user-guide/multi-account.md)
- [Troubleshooting](../user-guide/troubleshooting.md)
