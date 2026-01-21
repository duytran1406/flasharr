# Flasharr Deployment Guide

**Version:** 0.0.3-beta  
**Last Updated:** 2026-01-14

This guide covers deploying Flasharr to production environments.

---

## Prerequisites

- Docker and Docker Compose installed
- Fshare.vn VIP account(s)
- Network access to Fshare.vn and TimFshare.com
- Storage for downloads

---

## Deployment Methods

### Method 1: Docker Compose (Recommended)

#### 1. Prepare Environment

```bash
# Create application directory
mkdir -p /mnt/appdata/Flasharr/data
mkdir -p /data/fshare-downloader

# Set permissions
chown -R 1000:1000 /mnt/appdata/Flasharr
chown -R 1000:1000 /data/fshare-downloader
```

#### 2. Create Environment File

Create `/mnt/appdata/Flasharr/.env`:

```bash
# Fshare Credentials (Required)
FSHARE_EMAIL=your-email@example.com
FSHARE_PASSWORD=your-secure-password

# Server Configuration
INDEXER_PORT=8484
HOST=0.0.0.0

# Download Configuration
DOWNLOAD_DIR=/downloads
MAX_CONCURRENT_DOWNLOADS=2
SEGMENTS_PER_DOWNLOAD=4

# Optional: Multi-Account Support
# FSHARE_ACCOUNTS=email1:pass1,email2:pass2

# Optional: Speed Limiting
GLOBAL_SPEED_LIMIT_MBPS=0

# Optional: Auto-Cleanup
ENABLE_AUTO_CLEANUP=true
RETENTION_DAYS_SUCCESS=7
RETENTION_DAYS_FAILED=30

# Logging
LOG_LEVEL=INFO
```

#### 3. Deploy Container

```bash
cd /etc/pve/fshare-arr-bridge
docker-compose up -d
```

#### 4. Verify Deployment

```bash
# Check container status
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

### Method 2: Manual Deployment

#### 1. Clone Repository

```bash
git clone https://github.com/yourusername/fshare-arr-bridge.git
cd fshare-arr-bridge
```

#### 2. Install Dependencies

```bash
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
```

#### 3. Configure

```bash
cp .env.example .env
nano .env  # Edit with your settings
```

#### 4. Run Application

```bash
python -m flasharr
```

Or use the start script:
```bash
./start.sh
```

---

## Production Deployment Checklist

### Pre-Deployment

- [ ] Fshare VIP account(s) verified and active
- [ ] Environment variables configured
- [ ] Storage directories created with correct permissions
- [ ] Network connectivity to Fshare.vn tested
- [ ] Docker/Docker Compose installed and updated
- [ ] Backup of previous version (if upgrading)

### Deployment

- [ ] Pull latest code/image
- [ ] Review changelog for breaking changes
- [ ] Update environment variables if needed
- [ ] Start container/application
- [ ] Verify health endpoint responds
- [ ] Check logs for errors

### Post-Deployment

- [ ] Test web interface access
- [ ] Test Prowlarr indexer integration
- [ ] Test Radarr/Sonarr download client
- [ ] Verify downloads start successfully
- [ ] Monitor logs for 10-15 minutes
- [ ] Update documentation if needed

---

## Updating Flasharr

### Docker Update

```bash
cd /etc/pve/fshare-arr-bridge

# Pull latest code
git pull origin main

# Rebuild image
docker-compose build

# Stop current container
docker-compose down

# Start with new image
docker-compose up -d

# Verify
docker-compose logs -f flasharr
```

### Manual Update

```bash
cd /etc/pve/fshare-arr-bridge

# Pull latest code
git pull origin main

# Activate virtual environment
source venv/bin/activate

# Update dependencies
pip install -r requirements.txt --upgrade

# Restart application
./start.sh
```

---

## Rollback Procedure

If deployment fails or issues arise:

```bash
# Stop current version
docker-compose down

# Checkout previous version
git checkout v0.0.2-beta  # or specific commit

# Rebuild
docker-compose build

# Start
docker-compose up -d
```

---

## Multi-Instance Deployment

To run multiple Flasharr instances:

### Instance 1 (Primary)

```yaml
# docker-compose.yml
services:
  flasharr-primary:
    image: flasharr:beta
    container_name: flasharr-primary
    ports:
      - "8484:8484"
    env_file:
      - /mnt/appdata/Flasharr-Primary/.env
    volumes:
      - /mnt/appdata/Flasharr-Primary/data:/app/data
      - /data/fshare-downloader-1:/downloads
```

### Instance 2 (Secondary)

```yaml
# docker-compose-secondary.yml
services:
  flasharr-secondary:
    image: flasharr:beta
    container_name: flasharr-secondary
    ports:
      - "8485:8484"
    env_file:
      - /mnt/appdata/Flasharr-Secondary/.env
    volumes:
      - /mnt/appdata/Flasharr-Secondary/data:/app/data
      - /data/fshare-downloader-2:/downloads
```

---

## Monitoring

### Health Checks

```bash
# HTTP health check
curl http://localhost:8484/health

# Container health
docker inspect flasharr --format='{{.State.Health.Status}}'
```

### Log Monitoring

```bash
# Follow logs
docker-compose logs -f flasharr

# Last 100 lines
docker-compose logs --tail=100 flasharr

# Errors only
docker-compose logs flasharr | grep -i error
```

### Resource Monitoring

```bash
# Container stats
docker stats flasharr

# Disk usage
du -sh /mnt/appdata/Flasharr
du -sh /data/fshare-downloader
```

---

## Backup & Restore

### Backup

```bash
#!/bin/bash
# backup-flasharr.sh

BACKUP_DIR="/backup/flasharr"
DATE=$(date +%Y%m%d_%H%M%S)

# Create backup directory
mkdir -p $BACKUP_DIR

# Backup environment file
cp /mnt/appdata/Flasharr/.env $BACKUP_DIR/.env.$DATE

# Backup database
cp /mnt/appdata/Flasharr/data/downloads.db $BACKUP_DIR/downloads.db.$DATE

# Backup accounts
cp /mnt/appdata/Flasharr/data/accounts.json $BACKUP_DIR/accounts.json.$DATE

echo "Backup completed: $BACKUP_DIR/*.$DATE"
```

### Restore

```bash
#!/bin/bash
# restore-flasharr.sh

BACKUP_FILE=$1

# Stop container
docker-compose down

# Restore files
cp $BACKUP_FILE.env /mnt/appdata/Flasharr/.env
cp $BACKUP_FILE.db /mnt/appdata/Flasharr/data/downloads.db
cp $BACKUP_FILE.json /mnt/appdata/Flasharr/data/accounts.json

# Start container
docker-compose up -d
```

---

## Security Considerations

### Environment Variables

- Never commit `.env` file to version control
- Use strong passwords for Fshare accounts
- Rotate credentials periodically

### Network Security

- Use reverse proxy (Traefik, Nginx) for HTTPS
- Restrict access with firewall rules
- Consider VPN for remote access

### Container Security

```yaml
# docker-compose.yml security enhancements
services:
  flasharr:
    # ... other config ...
    security_opt:
      - no-new-privileges:true
    read_only: true
    tmpfs:
      - /tmp
    user: "1000:1000"
```

---

## Troubleshooting Deployment

### Container Won't Start

```bash
# Check logs
docker-compose logs flasharr

# Common issues:
# - Missing .env file
# - Invalid credentials
# - Port already in use
# - Permission errors
```

### Port Conflicts

```bash
# Check what's using port 8484
netstat -tulpn | grep 8484

# Change port in docker-compose.yml
ports:
  - "8585:8484"  # External:Internal
```

### Permission Errors

```bash
# Fix ownership
chown -R 1000:1000 /mnt/appdata/Flasharr
chown -R 1000:1000 /data/fshare-downloader

# Fix permissions
chmod -R 755 /data/fshare-downloader
```

---

## Performance Tuning

### For High-Volume Downloads

```bash
# .env
MAX_CONCURRENT_DOWNLOADS=4
SEGMENTS_PER_DOWNLOAD=8
ENABLE_DYNAMIC_SCALING=true
```

### For Limited Resources

```bash
# .env
MAX_CONCURRENT_DOWNLOADS=1
SEGMENTS_PER_DOWNLOAD=2
GLOBAL_SPEED_LIMIT_MBPS=5
```

### Database Optimization

```bash
# Vacuum database periodically
docker-compose exec flasharr sqlite3 /app/data/downloads.db "VACUUM;"
```

---

## Next Steps

- [Configuration Guide](flasharr_docs/getting-started/configuration.md)
- [Troubleshooting](flasharr_docs/user-guide/troubleshooting.md)
- [API Reference](flasharr_docs/api-reference/)
