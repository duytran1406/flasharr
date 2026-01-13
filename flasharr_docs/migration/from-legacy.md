# Migrating from Legacy Version

This guide helps you migrate from the old fshare-arr-bridge (with PyLoad) to Flasharr beta (built-in engine).

---

## Key Differences

| Feature | Legacy | Flasharr Beta |
|---------|--------|---------------|
| **Download Manager** | PyLoad (external) | Built-in engine |
| **Multi-threading** | No | Yes (segmented downloads) |
| **Multi-account** | No | Yes (load balancing) |
| **Priority Queue** | No | Yes (4 levels) |
| **Speed Limiting** | No | Yes (global limit) |
| **WebSocket** | No | Yes (real-time updates) |
| **Dashboard** | Basic | Enhanced with stats |

---

## Breaking Changes

### 1. PyLoad Removed

**Legacy:**
```bash
PYLOAD_HOST=192.168.1.112
PYLOAD_PORT=8000
PYLOAD_USERNAME=admin
PYLOAD_PASSWORD=password
```

**Flasharr Beta:**
```bash
# PyLoad variables removed - not needed!
# Downloads handled internally
```

---

### 2. Port Consolidation

**Legacy:**
- Indexer: Port 8484
- SABnzbd: Port 8585

**Flasharr Beta:**
- Everything: Port 8484 (single port)

**Update your *arr apps:**
```
Old: http://flasharr:8585/sabnzbd/api
New: http://flasharr:8484/sabnzbd/api
```

---

### 3. Environment Variables

**Removed:**
- `PYLOAD_HOST`
- `PYLOAD_PORT`
- `PYLOAD_USERNAME`
- `PYLOAD_PASSWORD`
- `SABNZBD_PORT`

**Added:**
- `MAX_CONCURRENT_DOWNLOADS`
- `SEGMENTS_PER_DOWNLOAD`
- `GLOBAL_SPEED_LIMIT_MBPS`
- `FSHARE_ACCOUNTS` (multi-account)
- `ENABLE_DYNAMIC_SCALING`

---

## Migration Steps

### Step 1: Backup Current Setup

```bash
# Backup environment file
cp /mnt/appdata/Flasharr/.env /mnt/appdata/Flasharr/.env.legacy

# Backup data (if any)
cp -r /mnt/appdata/Flasharr/data /mnt/appdata/Flasharr/data.legacy
```

---

### Step 2: Stop Legacy Version

```bash
cd /etc/pve/fshare-arr-bridge
docker-compose down
```

---

### Step 3: Update Environment File

Edit `/mnt/appdata/Flasharr/.env`:

```bash
# Remove PyLoad configuration
# PYLOAD_HOST=...  ← DELETE
# PYLOAD_PORT=...  ← DELETE
# PYLOAD_USERNAME=...  ← DELETE
# PYLOAD_PASSWORD=...  ← DELETE

# Keep Fshare credentials
FSHARE_EMAIL=your-email@example.com
FSHARE_PASSWORD=your-password

# Update port (single port now)
INDEXER_PORT=8484

# Add new download engine settings
DOWNLOAD_DIR=/downloads
MAX_CONCURRENT_DOWNLOADS=2
SEGMENTS_PER_DOWNLOAD=4
GLOBAL_SPEED_LIMIT_MBPS=0

# Optional: Multi-account
# FSHARE_ACCOUNTS=email1:pass1,email2:pass2
```

---

### Step 4: Pull Latest Code

```bash
cd /etc/pve/fshare-arr-bridge
git pull origin main
```

---

### Step 5: Start Flasharr Beta

```bash
docker-compose up -d
```

---

### Step 6: Verify Installation

```bash
# Check container is running
docker-compose ps

# Check logs
docker-compose logs -f flasharr

# Test health endpoint
curl http://localhost:8484/health
```

Expected response:
```json
{"status": "healthy", "version": "0.0.3-beta"}
```

---

### Step 7: Update Radarr/Sonarr

**Update download client configuration:**

1. Go to **Settings** → **Download Clients**
2. Edit Flasharr download client
3. Change:
   - **Port:** `8484` (was 8585)
   - **URL Base:** `/sabnzbd` (unchanged)
4. **Test** → **Save**

---

### Step 8: Test Integration

1. **Test search in Prowlarr:**
   - Should still work (no changes needed)

2. **Test download in Radarr/Sonarr:**
   - Search for a movie/show
   - Click download
   - Check Flasharr dashboard: `http://localhost:8484`

3. **Verify download starts:**
   - Should see progress in dashboard
   - No PyLoad needed!

---

## Troubleshooting Migration

### "Connection refused" from Radarr/Sonarr

**Issue:** Port changed from 8585 to 8484

**Fix:**
```bash
# Update download client port in Radarr/Sonarr
Port: 8484
```

---

### Downloads not starting

**Issue:** PyLoad is no longer used

**Fix:**
```bash
# Check Flasharr logs
docker-compose logs flasharr | tail -50

# Verify Fshare credentials
docker-compose logs flasharr | grep -i "login"
```

---

### Missing download directory

**Issue:** Download path changed

**Fix:**
```bash
# Create download directory
mkdir -p /data/fshare-downloader

# Fix permissions
sudo chown -R 1000:1000 /data/fshare-downloader
```

---

## Data Migration

### Download Queue

**Legacy queue is NOT migrated automatically.**

**Options:**

1. **Let old downloads finish** in legacy version before migrating
2. **Re-add downloads** manually in Flasharr beta
3. **Use Radarr/Sonarr** to re-trigger downloads

---

### Configuration

**Most configuration is compatible:**
- Fshare credentials: Same
- Prowlarr indexer: No changes needed
- Download paths: Update if changed

---

## Rollback (If Needed)

If you need to rollback to legacy version:

```bash
# Stop beta
docker-compose down

# Restore legacy environment
cp /mnt/appdata/Flasharr/.env.legacy /mnt/appdata/Flasharr/.env

# Checkout legacy version
git checkout legacy-branch  # or specific commit

# Start legacy
docker-compose up -d

# Update Radarr/Sonarr port back to 8585
```

---

## Benefits of Migration

✅ **Faster Downloads** - Multi-threaded segmented downloads  
✅ **No External Dependencies** - No PyLoad setup needed  
✅ **Better Control** - Priority queue, speed limits  
✅ **Multi-Account** - Load balance across VIP accounts  
✅ **Real-time Updates** - WebSocket dashboard  
✅ **Simpler Setup** - One container, one port  

---

## Next Steps

- [Quick Start Guide](../getting-started/quick-start.md)
- [Configuration Guide](../getting-started/configuration.md)
- [Multi-Account Setup](../user-guide/multi-account.md)
- [Troubleshooting](../user-guide/troubleshooting.md)
