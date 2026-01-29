# Flasharr \*arr Integration Guide

Complete guide for integrating Flasharr with Sonarr and Radarr as both a Newznab indexer and SABnzbd download client.

---

## 📋 Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Getting Your API Key](#getting-your-api-key)
4. [Sonarr Integration](#sonarr-integration)
5. [Radarr Integration](#radarr-integration)
6. [Remote Path Mappings](#remote-path-mappings)
7. [Testing](#testing)
8. [Troubleshooting](#troubleshooting)

---

## Overview

Flasharr provides two integration points for Sonarr/Radarr:

1. **Newznab Indexer** - Search for Vietnamese content on Fshare
2. **SABnzbd Download Client** - Download files from Fshare

Both use the same API key for authentication.

---

## Prerequisites

- ✅ Flasharr installed and running
- ✅ Sonarr v3+ or Radarr v3+ installed
- ✅ Network connectivity between \*arr apps and Flasharr
- ✅ Valid Fshare account configured in Flasharr

---

## Getting Your API Key

### Option 1: From Flasharr Settings UI

1. Open Flasharr web interface: `http://your-flasharr-ip:8484`
2. Navigate to **Settings** → **Indexer** tab
3. Your API key is displayed in the **Newznab Indexer** section
4. Default key: `flasharr-default-key`

### Option 2: From Configuration File

```bash
# View your API key
cat /path/to/flasharr/config.toml | grep api_key
```

### Option 3: Custom API Key

Edit `config.toml`:

```toml
[indexer]
api_key = "your-custom-secure-key-here"
```

Then restart Flasharr.

---

## Sonarr Integration

### Step 1: Add Newznab Indexer

1. In Sonarr, go to **Settings** → **Indexers**
2. Click **+** → Select **Newznab**
3. Configure as follows:

| Field                         | Value                                       |
| ----------------------------- | ------------------------------------------- |
| **Name**                      | `Flasharr`                                  |
| **Enable RSS**                | ✅ (Optional)                               |
| **Enable Automatic Search**   | ✅                                          |
| **Enable Interactive Search** | ✅                                          |
| **URL**                       | `http://your-flasharr-ip:8484/newznab`      |
| **API Path**                  | `/api`                                      |
| **API Key**                   | `flasharr-default-key` (or your custom key) |
| **Categories**                | `5000,5040,5045` (TV categories)            |

4. Click **Test** - You should see "Test was successful!"
5. Click **Save**

> **Note:** If you see "Invalid API Key" but the test passes, you can safely ignore it and save. This is a known Sonarr UI quirk.

---

### Step 2: Add SABnzbd Download Client

1. In Sonarr, go to **Settings** → **Download Clients**
2. Click **+** → Select **SABnzbd**
3. Configure as follows:

| Field        | Value                                       |
| ------------ | ------------------------------------------- |
| **Name**     | `Flasharr`                                  |
| **Enable**   | ✅                                          |
| **Host**     | `your-flasharr-ip`                          |
| **Port**     | `8484`                                      |
| **URL Base** | `/sabnzbd`                                  |
| **API Key**  | `flasharr-default-key` (or your custom key) |
| **Username** | (leave empty)                               |
| **Password** | (leave empty)                               |
| **Category** | `tv` (optional)                             |

4. Click **Test** - You should see "Test was successful!"
5. Click **Save**

---

## Radarr Integration

### Step 1: Add Newznab Indexer

1. In Radarr, go to **Settings** → **Indexers**
2. Click **+** → Select **Newznab**
3. Configure as follows:

| Field                         | Value                                       |
| ----------------------------- | ------------------------------------------- |
| **Name**                      | `Flasharr`                                  |
| **Enable RSS**                | ✅ (Optional)                               |
| **Enable Automatic Search**   | ✅                                          |
| **Enable Interactive Search** | ✅                                          |
| **URL**                       | `http://your-flasharr-ip:8484/newznab`      |
| **API Path**                  | `/api`                                      |
| **API Key**                   | `flasharr-default-key` (or your custom key) |
| **Categories**                | `2000,2040,2045` (Movie categories)         |

4. Click **Test** - You should see "Test was successful!"
5. Click **Save**

---

### Step 2: Add SABnzbd Download Client

1. In Radarr, go to **Settings** → **Download Clients**
2. Click **+** → Select **SABnzbd**
3. Configure as follows:

| Field        | Value                                       |
| ------------ | ------------------------------------------- |
| **Name**     | `Flasharr`                                  |
| **Enable**   | ✅                                          |
| **Host**     | `your-flasharr-ip`                          |
| **Port**     | `8484`                                      |
| **URL Base** | `/sabnzbd`                                  |
| **API Key**  | `flasharr-default-key` (or your custom key) |
| **Username** | (leave empty)                               |
| **Password** | (leave empty)                               |
| **Category** | `movies` (optional)                         |

4. Click **Test** - You should see "Test was successful!"
5. Click **Save**

---

## Remote Path Mappings

If Sonarr/Radarr and Flasharr are on different systems, you need to configure Remote Path Mappings.

### For Sonarr:

1. Go to **Settings** → **Download Clients**
2. Scroll to **Remote Path Mappings**
3. Click **+** and configure:

| Field           | Value                             |
| --------------- | --------------------------------- |
| **Host**        | `your-flasharr-ip`                |
| **Remote Path** | `/appData/downloads/`             |
| **Local Path**  | `/path/to/your/shared/downloads/` |

### For Radarr:

Same as Sonarr - configure the path where Radarr can access Flasharr's downloads.

### Example Scenarios:

**Scenario 1: Docker Compose (Same Network)**

```yaml
# Both containers share the same volume
volumes:
  - /mnt/downloads:/downloads

# Remote Path Mapping:
# Remote: /appData/downloads/
# Local: /downloads/
```

**Scenario 2: Separate Machines (NFS/SMB)**

```
# Flasharr downloads to: /appData/downloads/
# Mounted on Sonarr at: /mnt/flasharr-downloads/

# Remote Path Mapping:
# Remote: /appData/downloads/
# Local: /mnt/flasharr-downloads/
```

---

## Testing

### Test Newznab Indexer

#### In Sonarr:

1. Go to **Series** → Select any series
2. Click **Search** → **Manual Search**
3. You should see results from Flasharr with Vietnamese titles
4. Example: "Breaking Bad" → "Tội Phạm Hoàn Lương"

#### In Radarr:

1. Go to **Movies** → Select any movie
2. Click **Search** → **Manual Search**
3. You should see results from Flasharr

### Test Download Client

1. In Sonarr/Radarr, manually grab a release from Flasharr
2. Check **Activity** → **Queue**
3. The download should appear and progress
4. Monitor Flasharr's **Downloads** tab for progress

---

## Troubleshooting

### Issue: "Invalid API Key" Error

**Symptoms:** Test shows error but connection works

**Solution:**

- This is often a false positive in Sonarr/Radarr UI
- If the test passes, save anyway
- Check Flasharr logs for actual API key validation:
  ```bash
  tail -f /tmp/flasharr.log | grep "API key"
  ```

---

### Issue: "Connection Refused"

**Symptoms:** Cannot connect to Flasharr

**Solutions:**

1. **Check Flasharr is running:**

   ```bash
   curl http://localhost:8484/health
   ```

2. **Check firewall:**

   ```bash
   # Allow port 8484
   sudo ufw allow 8484/tcp
   ```

3. **Verify URL format:**
   - ✅ Correct: `http://192.168.1.100:8484/newznab`
   - ❌ Wrong: `http://192.168.1.100:8484/newznab/`
   - ❌ Wrong: `http://192.168.1.100:8484/newznab/api`

4. **Check network connectivity:**
   ```bash
   # From Sonarr/Radarr host
   curl http://flasharr-ip:8484/newznab/api?t=caps
   ```

---

### Issue: No Search Results

**Symptoms:** Searches return empty

**Solutions:**

1. **Check Fshare credentials in Flasharr:**
   - Go to Settings → Fshare Account
   - Verify credentials are correct
   - Test connection

2. **Check search logs:**

   ```bash
   tail -f /tmp/flasharr.log | grep "Search"
   ```

3. **Try manual search in Flasharr:**
   - Go to Discover tab
   - Search for the same content
   - If results appear, integration is working

4. **Verify TMDB API:**
   - Flasharr uses TMDB for Vietnamese title resolution
   - Check logs for TMDB errors

---

### Issue: Downloads Not Importing

**Symptoms:** Downloads complete but don't import to Sonarr/Radarr

**Solutions:**

1. **Check Remote Path Mappings:**
   - Ensure paths are correctly mapped
   - Verify both systems can access the download location

2. **Check file permissions:**

   ```bash
   # Ensure Sonarr/Radarr can read downloaded files
   ls -la /path/to/downloads/
   ```

3. **Check download location:**
   - In Flasharr Settings, verify download path
   - Ensure it matches Remote Path Mapping

4. **Manual import:**
   - Go to Sonarr/Radarr → Activity → Queue
   - Click "Manual Import" if needed

---

### Issue: Slow Searches

**Symptoms:** Searches take a long time

**Solutions:**

1. **Check TMDB API rate limits:**
   - Flasharr caches TMDB data
   - First search may be slower

2. **Check Fshare response time:**
   - Fshare API may be slow during peak hours

3. **Enable caching:**
   - Flasharr automatically caches search results
   - Subsequent searches will be faster

---

## Advanced Configuration

### Custom Categories

Edit Flasharr's `config.toml`:

```toml
[indexer]
api_key = "your-key"

# Custom category mappings (optional)
# tv_categories = [5000, 5040, 5045]
# movie_categories = [2000, 2040, 2045]
```

### Multiple Instances

You can add Flasharr to multiple Sonarr/Radarr instances:

- Each instance uses the same API key
- Configure Remote Path Mappings per instance
- No additional Flasharr configuration needed

---

## API Endpoints Reference

### Newznab Indexer

| Endpoint                                 | Description             |
| ---------------------------------------- | ----------------------- |
| `GET /newznab/api?t=caps`                | Get capabilities        |
| `GET /newznab/api?t=search&q=...`        | General search          |
| `GET /newznab/api?t=tvsearch&tvdbid=...` | TV search by TVDB ID    |
| `GET /newznab/api?t=movie&imdbid=...`    | Movie search by IMDB ID |

### SABnzbd Download Client

| Endpoint                    | Description          |
| --------------------------- | -------------------- |
| `GET /sabnzbd?mode=queue`   | Get download queue   |
| `GET /sabnzbd?mode=history` | Get download history |
| `GET /sabnzbd?mode=addurl`  | Add download         |
| `GET /sabnzbd?mode=pause`   | Pause download       |
| `GET /sabnzbd?mode=resume`  | Resume download      |

---

## Support

If you encounter issues:

1. **Check Flasharr logs:**

   ```bash
   tail -f /tmp/flasharr.log
   ```

2. **Check Sonarr/Radarr logs:**
   - Settings → System → Logs

3. **Test API directly:**

   ```bash
   # Test Newznab
   curl "http://localhost:8484/newznab/api?t=caps"

   # Test SABnzbd
   curl "http://localhost:8484/sabnzbd?mode=version&apikey=flasharr-default-key"
   ```

4. **Report issues:**
   - Include logs from both Flasharr and Sonarr/Radarr
   - Include your configuration (remove sensitive data)

---

## Tips for Best Results

1. **Use TVDB/IMDB IDs:** Sonarr/Radarr automatically send these for accurate Vietnamese title matching

2. **Enable Interactive Search:** Allows manual selection of quality/release

3. **Configure Quality Profiles:** Set preferred qualities in Sonarr/Radarr

4. **Monitor First Downloads:** Verify Remote Path Mappings work correctly

5. **Check Download Progress:** Use Flasharr's Downloads tab for detailed progress

---

## What Makes Flasharr Different

Traditional Newznab indexers search by English titles, which often miss Vietnamese releases on Fshare.

**Flasharr's Smart Search:**

1. Receives TVDB/IMDB ID from Sonarr/Radarr
2. Converts to TMDB ID
3. Fetches Vietnamese alternative titles from TMDB
4. Searches Fshare with both English and Vietnamese titles
5. Returns accurate Vietnamese releases

**Result:** Better matches for Vietnamese content! 🎯

---

**Happy downloading!** 🚀
