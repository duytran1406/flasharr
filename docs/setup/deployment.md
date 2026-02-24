# Flasharr Deployment Guide

Complete guide for deploying Flasharr to various environments.

## Deployment Options

- [Local Development](#local-development)
- [Docker Deployment](#docker-deployment)
- [Staging Environment (LXC)](#staging-environment)
- [Production Deployment](#production-deployment)

---

## Local Development

### Prerequisites

- Rust 1.70+ and Cargo
- Node.js 18+ and npm
- SQLite3

### Setup

```bash
# 1. Clone repository
git clone https://github.com/yourusername/flasharr.git
cd flasharr/Flasharr

# 2. Create appData directory
mkdir -p appData/{config,data,downloads,logs}

# 3. Run development script
./scripts/deploy/dev.sh
```

The development script will:

- Build the backend in debug mode
- Build the frontend
- Start the backend on port 8484
- Watch for file changes

### Access

- **Frontend**: http://localhost:8484
- **API**: http://localhost:8484/api
- **Health Check**: http://localhost:8484/api/health

---

## Docker Deployment

### Quick Start

```bash
# 1. Clone repository
git clone https://github.com/yourusername/flasharr.git
cd flasharr/Flasharr

# 2. Create directories
mkdir -p appData downloads

# 3. Start with Docker Compose
docker-compose up -d

# 4. View logs
docker-compose logs -f
```

### Docker Compose Configuration

```yaml
version: "3.8"

services:
  flasharr:
    image: flasharr:latest
    container_name: flasharr
    restart: unless-stopped
    ports:
      - "8484:8484"
    volumes:
      - ./appData:/appData
      - ./downloads:/downloads
    environment:
      - FLASHARR_APPDATA_DIR=/appData
      - RUST_LOG=flasharr=info,tower_http=info
      - TZ=Asia/Bangkok
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8484/api/health"]
      interval: 30s
      timeout: 3s
      retries: 3
      start_period: 10s
```

### Custom Build

```bash
# Build local image
docker build -t flasharr:local .

# Run container
docker run -d \
  --name flasharr \
  -p 8484:8484 \
  -v $(pwd)/appData:/appData \
  -v $(pwd)/downloads:/downloads \
  -e FLASHARR_APPDATA_DIR=/appData \
  flasharr:local
```

---

## Staging Environment

Deploy to LXC container on Proxmox for testing.

### Prerequisites

- Proxmox host accessible via SSH
- LXC container (ID: 112) with Docker installed
- SSH key configured for root access

### Deploy to Staging

```bash
./scripts/deploy/staging.sh
```

This script will:

1. Build Docker image locally (linux/amd64)
2. Save image to tarball
3. Transfer to Proxmox host
4. Push to LXC container
5. Load image and start container

### Access Staging

```bash
# Get LXC IP
ssh root@pve-remote "pct exec 112 -- hostname -I | awk '{print \$1}'"

# Access UI
open http://<LXC_IP>:8484

# View logs
ssh root@pve-remote "pct exec 112 -- docker logs -f flasharr"
```

---

## Production Deployment

### Option 1: GitHub Container Registry

#### Setup

1. **Configure GitHub Secrets**
   - `GHCR_TOKEN`: GitHub Personal Access Token with `write:packages` permission

2. **Push to GitHub**

   ```bash
   git push origin main
   ```

3. **Automatic Build**
   - GitHub Actions builds and pushes to `ghcr.io/yourusername/flasharr`
   - Tags: `latest`, `v{version}`, `{git-sha}`

#### Deploy

```bash
# Pull from GHCR
docker pull ghcr.io/yourusername/flasharr:latest

# Run
docker run -d \
  --name flasharr \
  -p 8484:8484 \
  -v /path/to/appData:/appData \
  -v /path/to/downloads:/downloads \
  ghcr.io/yourusername/flasharr:latest
```

### Option 2: Self-Hosted

#### Build Production Image

```bash
# Build optimized image
docker build \
  --build-arg VERSION=$(git describe --tags --always) \
  --build-arg BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ") \
  --build-arg VCS_REF=$(git rev-parse --short HEAD) \
  -t flasharr:production \
  .
```

#### Deploy with Docker Compose

```yaml
version: "3.8"

services:
  flasharr:
    image: flasharr:production
    container_name: flasharr
    restart: unless-stopped
    ports:
      - "8484:8484"
    volumes:
      - /mnt/appdata/flasharr:/appData
      - /mnt/data/downloads:/downloads
    environment:
      - FLASHARR_APPDATA_DIR=/appData
      - RUST_LOG=flasharr=info
      - TZ=Asia/Bangkok
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8484/api/health"]
      interval: 30s
      timeout: 3s
      retries: 3
      start_period: 10s
    networks:
      - media

networks:
  media:
    external: true
```

---

## Environment Variables

| Variable               | Default         | Description                |
| ---------------------- | --------------- | -------------------------- |
| `FLASHARR_APPDATA_DIR` | `/appData`      | Application data directory |
| `RUST_LOG`             | `flasharr=info` | Logging level              |
| `TZ`                   | `UTC`           | Timezone                   |
| `PORT`                 | `8484`          | HTTP server port           |

---

## Reverse Proxy Setup

### Nginx

```nginx
server {
    listen 80;
    server_name flasharr.example.com;

    location / {
        proxy_pass http://localhost:8484;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### Traefik

```yaml
labels:
  - "traefik.enable=true"
  - "traefik.http.routers.flasharr.rule=Host(`flasharr.example.com`)"
  - "traefik.http.routers.flasharr.entrypoints=websecure"
  - "traefik.http.routers.flasharr.tls.certresolver=letsencrypt"
  - "traefik.http.services.flasharr.loadbalancer.server.port=8484"
```

---

## Monitoring

### Health Check

```bash
curl http://localhost:8484/api/health
```

Expected response:

```json
{ "status": "ok" }
```

### Logs

**Docker:**

```bash
docker logs -f flasharr
```

**Local:**

```bash
tail -f appData/logs/flasharr.log
```

### Metrics

Check engine stats:

```bash
curl http://localhost:8484/api/engine/stats
```

---

## Backup & Restore

### Backup

```bash
# Stop Flasharr
docker-compose down

# Backup appData
tar -czf flasharr-backup-$(date +%Y%m%d).tar.gz appData/

# Restart
docker-compose up -d
```

### Restore

```bash
# Stop Flasharr
docker-compose down

# Restore appData
tar -xzf flasharr-backup-YYYYMMDD.tar.gz

# Restart
docker-compose up -d
```

---

## Updating

### Docker

```bash
# Pull latest image
docker-compose pull

# Restart with new image
docker-compose up -d
```

### Local

```bash
# Pull latest code
git pull origin main

# Rebuild and restart
./scripts/deploy/dev.sh
```

---

## Troubleshooting

### Container Won't Start

**Check logs:**

```bash
docker logs flasharr
```

**Common issues:**

- Port 8484 already in use
- appData directory permissions
- Missing environment variables

### Database Locked

**Symptoms:** "database is locked" errors

**Fix:**

```bash
# Stop all instances
docker-compose down

# Remove lock file
rm appData/data/flasharr.db-wal
rm appData/data/flasharr.db-shm

# Restart
docker-compose up -d
```

### High Memory Usage

**Check:**

```bash
docker stats flasharr
```

**Fix:**

- Clear completed downloads
- Reduce concurrent downloads
- Restart container

---

## Security Considerations

1. **Reverse Proxy**: Always use HTTPS in production
2. **Firewall**: Restrict port 8484 to trusted networks
3. **Updates**: Keep Docker image and dependencies updated
4. **Backups**: Regular backups of appData directory
5. **Credentials**: Never commit Fshare credentials to git

---

## Performance Tuning

### Docker Resources

```yaml
services:
  flasharr:
    # ... other config ...
    deploy:
      resources:
        limits:
          cpus: "2.0"
          memory: 2G
        reservations:
          cpus: "1.0"
          memory: 512M
```

### Concurrent Downloads

Adjust in Settings → General based on your bandwidth and system resources:

- **Low-end**: 1-2 concurrent downloads
- **Mid-range**: 3-5 concurrent downloads
- **High-end**: 5-10 concurrent downloads

---

For more deployment options, see:

- [Publishing Guide](publishing.md) - GitHub Container Registry setup
- [Versioning Guide](versioning.md) - Release management
