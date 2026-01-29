# Flasharr Deployment Guide

This guide covers all deployment methods for Flasharr across different environments.

## 🎯 Quick Reference

| Environment     | Purpose                         | Command                          | Access                       |
| --------------- | ------------------------------- | -------------------------------- | ---------------------------- |
| **Development** | Local debugging with hot-reload | `./scripts/deploy/dev.sh`        | http://localhost:5173        |
| **Staging**     | Testing on LXC 112              | `./scripts/deploy/staging.sh`    | http://[LXC-IP]:8484         |
| **Production**  | Production deployment from GHCR | `./scripts/deploy/production.sh` | https://fshare.blavkbeav.com |

---

## 🛠️ Development Deployment

**Use Case**: Local development with hot-reload for rapid iteration

### Features

- ✅ Debug mode (faster compilation)
- ✅ Hot-reload for frontend (Vite)
- ✅ Detailed logging
- ✅ No Docker required

### Prerequisites

- Rust toolchain installed
- Node.js/npm installed

### Usage

```bash
cd /Users/blavkbeav/Documents/Workspace/Flasharr/Flasharr/scripts/deploy
./dev.sh
```

Or use the workflow:

```bash
/deploy-local
```

### What It Does

1. 🧹 Cleans up previous sessions
2. 🦀 Builds backend in debug mode
3. 🚀 Starts backend server (cargo run)
4. 🎨 Launches frontend dev server (Vite)

### Access Points

- **Frontend**: http://localhost:5173 (Vite dev server)
- **Backend**: http://localhost:8484 (API)

### Logs

- Backend build: `debug_log/backend-build.log`
- Backend runtime: `debug_log/run.log`

### Stop Services

Press `Ctrl+C` or:

```bash
pkill -f "target/debug/flasharr" && pkill -f "vite"
```

---

## 🧪 Staging Deployment

**Use Case**: Testing production builds on LXC 112 before going live

### Features

- ✅ Production Docker build on Mac (faster than Proxmox)
- ✅ Automatic image transfer to LXC 112
- ✅ Version metadata included
- ✅ Health checks

### Prerequisites

- Docker Desktop installed on Mac
- SSH access to `pve-remote` configured
- LXC 112 running with Docker installed

### Usage

```bash
cd /Users/blavkbeav/Documents/Workspace/Flasharr/Flasharr/scripts/deploy
./staging.sh
```

Or use the workflow:

```bash
/deploy-staging
```

### What It Does

1. 🏗️ Builds Docker image on Mac with version metadata
2. 💾 Saves image as tarball (~200-300MB)
3. 📤 Transfers to LXC 112 via SCP
4. 📥 Loads image on LXC
5. 🛑 Stops old container
6. 🚀 Starts new container
7. ✅ Verifies health check

### Configuration

- **LXC ID**: 112
- **Host**: pve-remote
- **Image**: flasharr:staging
- **Port**: 8484
- **AppData**: `/mnt/appdata/flasharr`
- **Downloads**: `/data/flasharr-download`

### Logs

- Local build: `debug_log/staging-docker-build.log`
- Container logs:
  ```bash
  ssh root@pve-remote "pct exec 112 -- docker logs -f flasharr"
  ```

### Why Build on Mac?

Building Docker images on your Mac is **significantly faster** than building on Proxmox because:

- Mac has more CPU/RAM resources
- Better Docker layer caching
- Faster filesystem I/O
- Transfer time (~1-2 min) is less than build time difference

---

## 🌟 Production Deployment

**Use Case**: Deploy published releases from GitHub Container Registry

### Features

- ✅ Pulls from GHCR (official releases)
- ✅ Automatic cleanup of old images
- ✅ Health verification
- ✅ Zero-downtime deployment

### Prerequisites

- Published image on GHCR: `ghcr.io/duytran1406/flasharr:latest`
- SSH access to `pve-remote` configured
- LXC 112 running with Docker installed

### Usage

```bash
cd /Users/blavkbeav/Documents/Workspace/Flasharr/Flasharr/scripts/deploy
./production.sh
```

Or use the workflow:

```bash
/deploy-production
```

### What It Does

1. 🛑 Stops old containers gracefully
2. 🧹 Cleans up old images (keeps GHCR images)
3. 📥 Pulls latest from GHCR
4. 📝 Creates docker-compose.yml
5. 🚀 Starts container
6. ⏳ Waits for health check (up to 60s)
7. 📊 Displays deployment status

### Configuration

- **LXC ID**: 112
- **Host**: pve-remote
- **Image**: ghcr.io/duytran1406/flasharr:latest
- **Port**: 8484
- **AppData**: `/mnt/appdata/flasharr` (preserved)
- **Downloads**: `/data/flasharr-download` (preserved)

### Access Points

- **Application**: http://[LXC-IP]:8484
- **Public URL**: https://fshare.blavkbeav.com/

### Monitoring

```bash
# View logs
ssh root@pve-remote "pct exec 112 -- docker logs -f flasharr"

# Check status
ssh root@pve-remote "pct exec 112 -- docker ps"

# Restart container
ssh root@pve-remote "pct exec 112 -- docker restart flasharr"
```

### Rollback

To rollback to a specific version:

```bash
ssh root@pve-remote "pct exec 112 -- bash -c 'cd /opt/flasharr && sed -i \"s/:latest/:v1.2.3/\" docker-compose.yml && docker compose pull && docker compose up -d'"
```

---

## 📋 Deployment Workflow

### Typical Development Cycle

```
1. Development
   └─> ./scripts/deploy/dev.sh
       └─> Make changes, test locally
           └─> Commit changes

2. Staging
   └─> ./scripts/deploy/staging.sh
       └─> Test on LXC 112
           └─> Verify functionality
               └─> Tag release

3. Production
   └─> Push to GHCR (CI/CD or manual)
       └─> ./scripts/deploy/production.sh
           └─> Monitor deployment
```

### Publishing to GHCR

Before production deployment, publish the image:

```bash
# Build with version tag
VERSION=$(git describe --tags --always)
docker build \
  --build-arg VERSION="${VERSION}" \
  --build-arg BUILD_DATE="$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  --build-arg VCS_REF="$(git rev-parse --short HEAD)" \
  -t ghcr.io/duytran1406/flasharr:latest \
  -t ghcr.io/duytran1406/flasharr:${VERSION} \
  .

# Push to GHCR
docker push ghcr.io/duytran1406/flasharr:latest
docker push ghcr.io/duytran1406/flasharr:${VERSION}
```

---

## 🎨 Script Features

All deployment scripts include:

- ✅ **Colored output** with icons for better readability
- ✅ **Progress indicators** for each step
- ✅ **Error handling** with clear error messages
- ✅ **Status verification** after deployment
- ✅ **Helpful commands** displayed at the end
- ✅ **Detailed logging** to debug_log directory

### Example Output

```
╔═══════════════════════════════════════════════════════╗
║         🚀 Flasharr Staging Deployment 🚀            ║
╚═══════════════════════════════════════════════════════╝

📍 Target:    LXC 112 on pve-remote
🐳 Image:     flasharr:staging
📦 Build:     Local (Mac)

[1/6] 🏗️  Building Docker image on Mac...
      📌 Version: v1.2.3
      📅 Build Date: 2026-01-29T01:46:33Z
      🔖 Git Commit: abc1234
      ✓ Docker image built successfully

[2/6] 💾 Saving Docker image to tarball...
      ✓ Image saved (245M)

...
```

---

## 🔧 Troubleshooting

### Development Issues

**Backend won't start**

```bash
# Check logs
tail -f debug_log/run.log

# Verify Rust installation
cargo --version
```

**Frontend won't start**

```bash
# Check Node version (need 18+)
node --version

# Reinstall dependencies
cd frontend && rm -rf node_modules && npm install
```

### Staging Issues

**Docker build fails on Mac**

```bash
# Check Docker Desktop is running
docker ps

# Check disk space
docker system df

# Clean up old images
docker system prune -a
```

**Transfer to LXC fails**

```bash
# Test SSH connection
ssh root@pve-remote "pct exec 112 -- echo 'Connected'"

# Check LXC disk space
ssh root@pve-remote "pct exec 112 -- df -h"
```

### Production Issues

**GHCR pull fails**

```bash
# Check if image exists
docker manifest inspect ghcr.io/duytran1406/flasharr:latest

# Login to GHCR (if private)
echo $GITHUB_TOKEN | docker login ghcr.io -u USERNAME --password-stdin
```

**Health check timeout**

```bash
# Check container logs
ssh root@pve-remote "pct exec 112 -- docker logs flasharr"

# Check if port is accessible
ssh root@pve-remote "pct exec 112 -- curl -v http://localhost:8484/health"
```

---

## 📊 Comparison Matrix

| Feature          | Development | Staging   | Production     |
| ---------------- | ----------- | --------- | -------------- |
| Build Time       | ~30s        | ~5-10min  | ~1-2min (pull) |
| Hot Reload       | ✅ Yes      | ❌ No     | ❌ No          |
| Docker Required  | ❌ No       | ✅ Yes    | ✅ Yes         |
| Optimized Build  | ❌ No       | ✅ Yes    | ✅ Yes         |
| Version Metadata | ❌ No       | ✅ Yes    | ✅ Yes         |
| Health Checks    | ❌ No       | ✅ Yes    | ✅ Yes         |
| Data Persistence | ❌ No       | ✅ Yes    | ✅ Yes         |
| Public Access    | ❌ No       | ⚠️ LXC IP | ✅ Yes         |

---

## 🎯 Best Practices

1. **Always test in staging** before production deployment
2. **Tag releases** with semantic versioning (v1.2.3)
3. **Monitor logs** after deployment for errors
4. **Keep AppData backed up** before major updates
5. **Use specific version tags** for production rollbacks
6. **Document breaking changes** in release notes

---

## 📝 Notes

- All scripts are located in `scripts/deploy/`
- All workflows are in `.agent/workflows/deploy-*.md`
- Logs are saved to `debug_log/` directory
- AppData is preserved across deployments
- Old containers are automatically cleaned up
