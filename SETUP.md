# Fshare-Arr Bridge - Setup Guide

## Quick Start (Docker - Recommended)

### 1. Configure Environment

```bash
cd /etc/pve/fshare-arr-bridge
cp .env.example .env
nano .env
```

Edit the following values:
```env
FSHARE_EMAIL=your-email@example.com
FSHARE_PASSWORD=your-password
PYLOAD_HOST=192.168.1.112
PYLOAD_PORT=8000
PYLOAD_USERNAME=admin
PYLOAD_PASSWORD=your-pyload-password
```

### 2. Build and Run

```bash
# Build the Docker image
docker build -t fshare-arr-bridge:latest .

# Run with docker-compose
docker-compose up -d

# Or run directly
docker run -d \
  --name fshare-arr-bridge \
  -p 8484:8484 \
  --env-file .env \
  fshare-arr-bridge:latest
```

### 3. Verify

```bash
# Check health
curl http://localhost:8484/health

# Check logs
docker logs fshare-arr-bridge
```

---

## Manual Installation (Without Docker)

### 1. Install Dependencies

```bash
cd /etc/pve/fshare-arr-bridge

# Create virtual environment
python3 -m venv venv
source venv/bin/activate

# Install requirements
pip install -r requirements.txt
```

### 2. Configure

```bash
cp .env.example .env
nano .env
```

### 3. Run

```bash
# Using the startup script
./start.sh

# Or directly
python -m app.main
```

---

## Integration with *arr Suite

### Step 1: Add Indexer to Prowlarr

1. Open Prowlarr web UI
2. Go to **Settings** → **Indexers** → **Add Indexer**
3. Select **Generic Newznab**
4. Configure:
   - **Name**: `Fshare`
   - **URL**: `http://your-server-ip:8484/indexer`
   - **API Path**: `/api`
   - **API Key**: (leave blank or use any value)
   - **Categories**: Select TV and Movies
5. Click **Test** → Should show "Success"
6. Click **Save**

### Step 2: Add Download Client to Sonarr/Radarr

1. Open Sonarr/Radarr web UI
2. Go to **Settings** → **Download Clients** → **Add**
3. Select **SABnzbd**
4. Configure:
   - **Name**: `Fshare Bridge`
   - **Host**: `your-server-ip`
   - **Port**: `8484`
   - **URL Base**: `/sabnzbd`
   - **API Key**: `fshare-bridge-api-key` (any value)
   - **Category**: `tv` (for Sonarr) or `movies` (for Radarr)
5. Click **Test** → Should show "Success"
6. Click **Save**

### Step 3: Test the Integration

1. In Sonarr, search for a series (e.g., "Ling Cage")
2. Click **Search** for an episode
3. You should see results from the Fshare indexer
4. Click **Download** on a result
5. Check the download client queue - the download should appear
6. Check pyLoad - the download should be added with the normalized filename

---

## Troubleshooting

### Issue: "Unable to identify correct episode(s)"

**Cause**: Filename normalization didn't work correctly.

**Solution**:
1. Check bridge logs: `docker logs fshare-arr-bridge`
2. Verify the normalized filename in logs
3. Ensure the filename follows pattern: `Series Name S01E01 Quality`

### Issue: Downloads not appearing in pyLoad

**Cause**: pyLoad connection failed.

**Solution**:
1. Check pyLoad is running: `curl http://pyload-host:8000`
2. Verify pyLoad credentials in `.env`
3. Check bridge logs for pyLoad connection errors
4. Ensure pyLoad is accessible from the bridge container

### Issue: No search results from Fshare

**Cause**: Fshare login failed or search query invalid.

**Solution**:
1. Check Fshare credentials in `.env`
2. Verify Fshare account is active and has VIP if required
3. Check bridge logs for Fshare API errors
4. Try searching directly on Fshare.vn to verify content exists

### Issue: Prowlarr can't connect to indexer

**Cause**: Network connectivity or wrong URL.

**Solution**:
1. Verify bridge is running: `curl http://localhost:8484/health`
2. Check firewall rules
3. Ensure Prowlarr can reach the bridge server
4. Use correct URL: `http://server-ip:8484/indexer`

---

## Advanced Configuration

### Using with Traefik

Add labels to docker-compose.yml:

```yaml
services:
  fshare-arr-bridge:
    # ... existing config ...
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.fshare-bridge.rule=Host(`fshare-bridge.yourdomain.com`)"
      - "traefik.http.routers.fshare-bridge.entrypoints=websecure"
      - "traefik.http.routers.fshare-bridge.tls.certresolver=letsencrypt"
      - "traefik.http.services.fshare-bridge.loadbalancer.server.port=8484"
```

### Custom Port

Change in `.env`:
```env
INDEXER_PORT=9999
```

And update docker-compose.yml:
```yaml
ports:
  - "9999:9999"
environment:
  INDEXER_PORT: 9999
```

### Debug Mode

Enable detailed logging:
```env
DEBUG=true
```

---

## API Documentation

### Indexer API (Newznab-compatible)

**Base URL**: `http://localhost:8484/indexer`

#### Capabilities
```bash
curl "http://localhost:8484/indexer/api?t=caps"
```

#### Search
```bash
# General search
curl "http://localhost:8484/indexer/api?t=search&q=Ling+Cage"

# TV search with season/episode
curl "http://localhost:8484/indexer/api?t=tvsearch&q=Ling+Cage&season=1&ep=14"

# Movie search
curl "http://localhost:8484/indexer/api?t=movie&q=Avengers"
```

### SABnzbd API

**Base URL**: `http://localhost:8484/sabnzbd`

#### Add URL
```bash
curl "http://localhost:8484/sabnzbd/api?mode=addurl&name=https://fshare.vn/file/XXXXX"
```

#### Get Queue
```bash
curl "http://localhost:8484/sabnzbd/api?mode=queue&output=json"
```

#### Get History
```bash
curl "http://localhost:8484/sabnzbd/api?mode=history&output=json"
```

---

## Maintenance

### View Logs

```bash
# Docker
docker logs -f fshare-arr-bridge

# Manual
tail -f app.log
```

### Update

```bash
# Pull latest changes
git pull

# Rebuild Docker image
docker-compose down
docker-compose build
docker-compose up -d
```

### Backup

```bash
# Backup configuration
cp .env .env.backup

# Export Docker image
docker save fshare-arr-bridge:latest | gzip > fshare-arr-bridge.tar.gz
```

---

## Performance Tuning

### Increase Workers

Edit `Dockerfile`:
```dockerfile
CMD ["gunicorn", "--bind", "0.0.0.0:8484", "--workers", "4", "--timeout", "120", "app.main:create_app()"]
```

### Adjust Timeouts

For slow Fshare connections, increase timeout in `app/fshare_client.py`:
```python
response = self.session.post(..., timeout=30)  # Increase from 15 to 30
```

---

## Security Considerations

1. **API Keys**: The bridge doesn't enforce API key authentication by default. Consider adding authentication if exposing to the internet.

2. **Credentials**: Never commit `.env` to git. It's already in `.gitignore`.

3. **Network**: Run on a private network or behind a reverse proxy with authentication.

4. **Updates**: Keep dependencies updated:
   ```bash
   pip install --upgrade -r requirements.txt
   ```

---

## Support

For issues, check:
1. Bridge logs
2. Fshare API status
3. pyLoad connectivity
4. *arr suite logs

Common log locations:
- Bridge: `docker logs fshare-arr-bridge`
- Sonarr: `/config/logs/sonarr.txt`
- Radarr: `/config/logs/radarr.txt`
- Prowlarr: `/config/logs/prowlarr.txt`
- pyLoad: Check pyLoad web UI → Logs
