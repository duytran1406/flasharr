# Troubleshooting Guide

Common issues and solutions for Flasharr.

---

## Installation Issues

### Container Won't Start

**Symptoms:**
- Container exits immediately
- `docker-compose ps` shows container as exited

**Solutions:**

1. **Check logs:**
   ```bash
   docker-compose logs flasharr
   ```

2. **Verify environment file exists:**
   ```bash
   ls -la /mnt/appdata/Flasharr/.env
   ```

3. **Check Fshare credentials:**
   ```bash
   docker-compose exec flasharr env | grep FSHARE
   ```

4. **Verify port availability:**
   ```bash
   netstat -tulpn | grep 8484
   ```

---

### Permission Denied Errors

**Symptoms:**
- Cannot write to download directory
- Database errors

**Solutions:**

```bash
# Fix ownership
sudo chown -R 1000:1000 /mnt/appdata/Flasharr
sudo chown -R 1000:1000 /data/fshare-downloader

# Fix permissions
sudo chmod -R 755 /data/fshare-downloader
```

---

## Authentication Issues

### Fshare Login Failed

**Symptoms:**
- "Authentication failed" in logs
- Downloads stuck in queue

**Solutions:**

1. **Verify credentials:**
   ```bash
   docker-compose logs flasharr | grep -i "login\|auth"
   ```

2. **Check account status:**
   - Login to Fshare.vn manually
   - Verify VIP account is active
   - Check for password changes

3. **Update credentials:**
   ```bash
   # Edit .env file
   nano /mnt/appdata/Flasharr/.env
   
   # Restart container
   docker-compose restart flasharr
   ```

---

### Session Expired

**Symptoms:**
- Downloads fail mid-progress
- "Session expired" errors

**Solutions:**

Flasharr automatically re-authenticates. If issues persist:

```bash
# Restart container to force re-login
docker-compose restart flasharr
```

---

## Search Issues

### No Search Results in Prowlarr

**Symptoms:**
- Prowlarr shows 0 results from Flasharr
- Other indexers work fine

**Solutions:**

1. **Test indexer directly:**
   ```bash
   curl "http://localhost:8484/indexer/api?t=search&q=test"
   ```

2. **Check TimFshare availability:**
   ```bash
   curl -I https://timfshare.com
   ```

3. **Verify Prowlarr configuration:**
   - URL: `http://flasharr:8484/indexer`
   - API Path: `/api`
   - Test connection in Prowlarr

4. **Check logs:**
   ```bash
   docker-compose logs flasharr | grep -i "search\|indexer"
   ```

---

### Search Returns Irrelevant Results

**Symptoms:**
- Results don't match search query
- Too many unrelated files

**Solutions:**

1. **Use more specific search terms:**
   - Include year: "Inception 2010"
   - Include quality: "Inception 1080p"
   - Include format: "Inception BluRay"

2. **Filter in Prowlarr:**
   - Set minimum size
   - Use preferred words
   - Configure quality profiles

---

## Download Issues

### Downloads Stuck in Queue

**Symptoms:**
- Downloads show "Queued" status
- Never start downloading

**Solutions:**

1. **Check concurrent download limit:**
   ```bash
   # View current config
   curl http://localhost:8484/api/engine/stats
   ```

2. **Check for errors:**
   ```bash
   docker-compose logs flasharr | tail -50
   ```

3. **Manually start download:**
   ```bash
   # Via API
   curl -X POST http://localhost:8484/api/downloads/{id}/resume
   ```

4. **Restart engine:**
   ```bash
   docker-compose restart flasharr
   ```

---

### Downloads Fail Immediately

**Symptoms:**
- Download status changes to "Failed"
- Error message in logs

**Common Errors:**

**"Invalid Fshare URL"**
```bash
# Verify URL format
# Should be: https://fshare.vn/file/XXXXXX
```

**"File not found"**
- File was deleted from Fshare
- Link expired
- Try searching again

**"Quota exceeded"**
- VIP account reached download limit
- Add more accounts or wait 24 hours

**"Download directory not writable"**
```bash
# Fix permissions
sudo chown -R 1000:1000 /data/fshare-downloader
```

---

### Slow Download Speeds

**Symptoms:**
- Downloads slower than expected
- Speed below VIP limits

**Solutions:**

1. **Check speed limit:**
   ```bash
   curl http://localhost:8484/api/engine/stats | grep speed_limit
   ```

2. **Increase segments:**
   ```bash
   # Edit .env
   SEGMENTS_PER_DOWNLOAD=8
   MAX_SEGMENTS=8
   
   # Restart
   docker-compose restart flasharr
   ```

3. **Check network:**
   ```bash
   # Test download speed
   docker-compose exec flasharr curl -o /dev/null https://speed.cloudflare.com/__down?bytes=100000000
   ```

4. **Disable speed limit:**
   ```bash
   curl -X POST http://localhost:8484/api/engine/speed-limit \
     -H "Content-Type: application/json" \
     -d '{"limit_mbps": 0}'
   ```

---

### Downloads Stop Mid-Progress

**Symptoms:**
- Download pauses unexpectedly
- No error message

**Solutions:**

1. **Check for quota limits:**
   ```bash
   docker-compose logs flasharr | grep -i "quota"
   ```

2. **Resume download:**
   ```bash
   curl -X POST http://localhost:8484/api/downloads/{id}/resume
   ```

3. **Check disk space:**
   ```bash
   df -h /data/fshare-downloader
   ```

---

## Integration Issues

### Radarr/Sonarr Can't Connect

**Symptoms:**
- "Unable to connect to SABnzbd" error
- Test connection fails

**Solutions:**

1. **Verify Flasharr is running:**
   ```bash
   curl http://localhost:8484/health
   ```

2. **Check SABnzbd API:**
   ```bash
   curl "http://localhost:8484/sabnzbd/api?mode=version"
   ```

3. **Verify configuration:**
   - Host: `flasharr` (Docker) or `localhost`
   - Port: `8484`
   - URL Base: `/sabnzbd`

4. **Check network:**
   ```bash
   # From Radarr/Sonarr container
   docker exec radarr ping flasharr
   ```

---

### Downloads Not Importing

**Symptoms:**
- Download completes in Flasharr
- Radarr/Sonarr doesn't import file

**Solutions:**

1. **Check download path:**
   ```bash
   # Verify Radarr/Sonarr can access download directory
   docker exec radarr ls -la /downloads
   ```

2. **Verify category mapping:**
   - Radarr category: `movies`
   - Sonarr category: `tv`

3. **Check file permissions:**
   ```bash
   ls -la /data/fshare-downloader/
   ```

4. **Manual import:**
   - Radarr/Sonarr → Activity → Queue
   - Click "Manual Import"

---

## Web Interface Issues

### Dashboard Not Loading

**Symptoms:**
- Blank page
- 404 error
- Connection refused

**Solutions:**

1. **Verify container is running:**
   ```bash
   docker-compose ps
   ```

2. **Check port mapping:**
   ```bash
   docker-compose port flasharr 8484
   ```

3. **Check browser console:**
   - Open Developer Tools (F12)
   - Look for JavaScript errors

4. **Clear browser cache:**
   - Ctrl+Shift+R (hard refresh)

---

### Real-time Updates Not Working

**Symptoms:**
- Download progress doesn't update
- Must refresh page manually

**Solutions:**

1. **Check WebSocket connection:**
   - Open browser Developer Tools → Network → WS
   - Should see WebSocket connection

2. **Verify WebSocket is enabled:**
   ```bash
   docker-compose logs flasharr | grep -i websocket
   ```

3. **Check firewall:**
   - Ensure WebSocket traffic allowed
   - Port 8484 must allow WebSocket upgrade

---

## Performance Issues

### High CPU Usage

**Symptoms:**
- Container using excessive CPU
- System slowdown

**Solutions:**

1. **Reduce concurrent downloads:**
   ```bash
   # Edit .env
   MAX_CONCURRENT_DOWNLOADS=1
   SEGMENTS_PER_DOWNLOAD=2
   ```

2. **Check for stuck downloads:**
   ```bash
   curl http://localhost:8484/api/downloads | grep -i "downloading"
   ```

3. **Restart container:**
   ```bash
   docker-compose restart flasharr
   ```

---

### High Memory Usage

**Symptoms:**
- Container using excessive RAM
- OOM errors

**Solutions:**

1. **Reduce segments:**
   ```bash
   # Edit .env
   SEGMENTS_PER_DOWNLOAD=2
   MAX_SEGMENTS=4
   ```

2. **Set memory limit:**
   ```yaml
   # docker-compose.yml
   services:
     flasharr:
       mem_limit: 512m
   ```

---

## Database Issues

### Database Locked

**Symptoms:**
- "Database is locked" errors
- Queue operations fail

**Solutions:**

```bash
# Stop container
docker-compose down

# Backup database
cp /mnt/appdata/Flasharr/data/downloads.db /mnt/appdata/Flasharr/data/downloads.db.bak

# Start container
docker-compose up -d
```

---

### Corrupt Database

**Symptoms:**
- SQLite errors in logs
- Queue not loading

**Solutions:**

```bash
# Stop container
docker-compose down

# Check database integrity
sqlite3 /mnt/appdata/Flasharr/data/downloads.db "PRAGMA integrity_check;"

# If corrupt, restore from backup or reset
rm /mnt/appdata/Flasharr/data/downloads.db

# Start container (will create new database)
docker-compose up -d
```

---

## Getting More Help

### Enable Debug Logging

```bash
# Edit .env
LOG_LEVEL=DEBUG

# Restart
docker-compose restart flasharr

# View logs
docker-compose logs -f flasharr
```

### Collect Diagnostic Information

```bash
# System info
docker-compose version
docker version

# Container status
docker-compose ps
docker-compose logs flasharr | tail -100

# Configuration
docker-compose exec flasharr env | grep -v PASSWORD

# API health
curl http://localhost:8484/health
curl http://localhost:8484/api/version
```

### Report an Issue

Include the following when reporting issues:

1. Flasharr version
2. Docker/Docker Compose version
3. Relevant log excerpts
4. Steps to reproduce
5. Expected vs actual behavior

---

## Next Steps

- [Configuration Guide](../getting-started/configuration.md)
- [Web Interface](web-interface.md)
- [Download Management](download-management.md)
