# Quick Start Guide

Get Flasharr up and running with Prowlarr, Radarr, and Sonarr in 5 minutes.

---

## Step 1: Start Flasharr

```bash
cd /etc/pve/fshare-arr-bridge
docker-compose up -d
```

Verify it's running:
```bash
curl http://localhost:8484/health
```

---

## Step 2: Add to Prowlarr (Indexer)

1. Open Prowlarr → **Settings** → **Indexers** → **Add Indexer**
2. Search for "**Newznab**" and select it
3. Configure:
   - **Name:** Fshare (Flasharr)
   - **URL:** `http://localhost:8484/indexer`
   - **API Key:** (leave empty or use any value)
   - **Categories:** Movies (2000), TV (5000)
4. **Test** → **Save**

---

## Step 3: Add to Radarr (Download Client)

1. Open Radarr → **Settings** → **Download Clients** → **Add**
2. Select "**SABnzbd**"
3. Configure:
   - **Name:** Flasharr
   - **Host:** `localhost`
   - **Port:** `8484`
   - **URL Base:** `/sabnzbd`
   - **API Key:** (leave empty or use any value)
   - **Category:** movies
4. **Test** → **Save**

---

## Step 4: Add to Sonarr (Download Client)

1. Open Sonarr → **Settings** → **Download Clients** → **Add**
2. Select "**SABnzbd**"
3. Configure:
   - **Name:** Flasharr
   - **Host:** `localhost`
   - **Port:** `8484`
   - **URL Base:** `/sabnzbd`
   - **API Key:** (leave empty or use any value)
   - **Category:** tv
4. **Test** → **Save**

---

## Step 5: Test the Integration

### Test Search (via Prowlarr)

1. Go to Prowlarr → **Search**
2. Enter a movie or TV show name
3. You should see results from "Fshare (Flasharr)"

### Test Download (via Radarr/Sonarr)

1. In Radarr, search for a movie
2. Click **Manual Search**
3. Select a result from Flasharr indexer
4. Click **Download**
5. Check Flasharr dashboard: `http://localhost:8484`

You should see the download in progress!

---

## Step 6: Monitor Downloads

Access the Flasharr web interface:

```
http://localhost:8484
```

Features:
- **Dashboard** - Real-time download statistics
- **Downloads** - Active and completed downloads
- **Search** - Direct Fshare search
- **Settings** - Configuration management

---

## Common First-Time Issues

### "Connection Failed" in Prowlarr/Radarr

**Check Flasharr is running:**
```bash
docker-compose ps
curl http://localhost:8484/health
```

### No Search Results

**Verify Fshare credentials:**
```bash
docker-compose logs flasharr | grep -i "login"
```

Should show: `Fshare login successful`

### Downloads Not Starting

**Check download directory permissions:**
```bash
ls -la /data/fshare-downloader
```

Fix if needed:
```bash
sudo chown -R 1000:1000 /data/fshare-downloader
```

---

## Next Steps

- **[Configuration Guide](configuration.md)** - Advanced settings
- **[Multi-Account Setup](../user-guide/multi-account.md)** - Use multiple VIP accounts
- **[Priority System](../user-guide/priority-system.md)** - Prioritize downloads
- **[Troubleshooting](../user-guide/troubleshooting.md)** - Solve common issues

---

## Quick Reference

| Service | URL | Purpose |
|---------|-----|---------|
| Web UI | `http://localhost:8484` | Dashboard |
| Health Check | `http://localhost:8484/health` | Status |
| Indexer API | `http://localhost:8484/indexer/api` | Prowlarr |
| SABnzbd API | `http://localhost:8484/sabnzbd/api` | Radarr/Sonarr |
| Engine API | `http://localhost:8484/api` | Direct API |

---

**You're all set!** Flasharr is now integrated with your *arr suite. 🎉
