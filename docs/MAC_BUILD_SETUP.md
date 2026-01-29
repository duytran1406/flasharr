# 🍎 Mac Build Environment Setup

Complete guide to set up your Mac for building Flasharr Docker images.

## ✅ Current Status

Your Mac already has:

- ✅ Docker Desktop installed (v28.5.1)
- ✅ Docker CLI available at `/usr/local/bin/docker`
- ✅ Docker.app in Applications folder

## 🚀 Quick Start

### 1. Start Docker Desktop

Docker Desktop has been started automatically. You can also start it manually:

```bash
# Start Docker Desktop
open -a Docker

# Or from Applications folder
# Click: Applications > Docker.app
```

### 2. Verify Docker is Running

```bash
# Check Docker status
docker info

# Should show:
# - Server Version
# - Operating System
# - CPUs and Memory available
```

### 3. Test Docker Build

```bash
# Simple test
docker run hello-world

# Should download and run successfully
```

## 🔧 Complete Environment Checklist

### Required Tools

- [x] **Docker Desktop** - Installed ✅
- [x] **Git** - For version control
- [ ] **SSH Access** - To pve-remote (for deployment)

### Verify All Tools

```bash
# Docker
docker --version
# Expected: Docker version 28.5.1, build e180ab8

# Git
git --version
# Expected: git version 2.x.x

# SSH to pve-remote
ssh root@pve-remote "echo 'Connected'"
# Expected: Connected
```

## 🐳 Docker Desktop Configuration

### Recommended Settings

1. **Resources** (Docker Desktop > Settings > Resources)
   - **CPUs**: 4-6 (for faster builds)
   - **Memory**: 8GB minimum, 12GB recommended
   - **Disk**: 60GB minimum

2. **Docker Engine** (Docker Desktop > Settings > Docker Engine)

   ```json
   {
     "builder": {
       "gc": {
         "enabled": true,
         "defaultKeepStorage": "20GB"
       }
     },
     "experimental": false,
     "features": {
       "buildkit": true
     }
   }
   ```

3. **Enable BuildKit** (faster builds)
   ```bash
   # Add to ~/.zshrc or ~/.bash_profile
   export DOCKER_BUILDKIT=1
   ```

### Apply Settings

```bash
# Reload shell configuration
source ~/.zshrc

# Restart Docker Desktop
osascript -e 'quit app "Docker"'
sleep 2
open -a Docker
```

## 📦 Building Flasharr Images

### Method 1: Using Staging Script (Recommended)

```bash
cd /Users/blavkbeav/Documents/Workspace/Flasharr/Flasharr/scripts/deploy
./staging.sh
```

This will:

1. Build Docker image on Mac
2. Save as tarball
3. Transfer to LXC 112
4. Deploy and verify

### Method 2: Manual Build

```bash
cd /Users/blavkbeav/Documents/Workspace/Flasharr/Flasharr

# Build with version info
VERSION=$(git describe --tags --always)
BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
VCS_REF=$(git rev-parse --short HEAD)

docker build \
  --build-arg VERSION="${VERSION}" \
  --build-arg BUILD_DATE="${BUILD_DATE}" \
  --build-arg VCS_REF="${VCS_REF}" \
  -t flasharr:local \
  -f Dockerfile \
  .
```

### Method 3: Publish to GHCR

```bash
cd /Users/blavkbeav/Documents/Workspace/Flasharr/Flasharr/scripts/deploy
./publish-ghcr.sh
```

## 🔍 Troubleshooting

### Docker Desktop Won't Start

```bash
# Check if already running
pgrep -f "Docker Desktop"

# Kill existing processes
pkill -f "Docker Desktop"

# Remove lock files
rm -f ~/Library/Group\ Containers/group.com.docker/docker.sock.lock

# Start fresh
open -a Docker
```

### Docker Build Fails

```bash
# Clean up old images
docker system prune -a

# Check disk space
docker system df

# Increase Docker Desktop resources
# Settings > Resources > Increase Memory/Disk
```

### "Cannot connect to Docker daemon"

```bash
# Wait for Docker to fully start
until docker info >/dev/null 2>&1; do
    echo "Waiting for Docker..."
    sleep 2
done
echo "Docker is ready!"

# Check Docker socket
ls -la ~/.docker/run/docker.sock
```

### Build is Slow

```bash
# Enable BuildKit
export DOCKER_BUILDKIT=1

# Use build cache
docker build --cache-from flasharr:latest .

# Increase Docker resources
# Settings > Resources > Increase CPUs/Memory
```

### Out of Disk Space

```bash
# Check usage
docker system df

# Clean up
docker system prune -a --volumes

# Remove unused images
docker image prune -a

# Remove build cache
docker builder prune -a
```

## 🎯 Build Performance Tips

### 1. Enable BuildKit

```bash
# Add to ~/.zshrc
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1
```

### 2. Use Multi-stage Builds

Our Dockerfile already uses multi-stage builds:

- Stage 1: Build Rust backend
- Stage 2: Build Node.js frontend
- Stage 3: Combine into minimal runtime image

### 3. Layer Caching

```bash
# Build with cache
docker build --cache-from flasharr:latest -t flasharr:new .

# Save cache
docker build --cache-to type=local,dest=/tmp/cache .
```

### 4. Parallel Builds

```bash
# BuildKit automatically parallelizes stages
# No additional configuration needed
```

## 📊 Expected Build Times

On a typical Mac (M1/M2 or Intel i7+):

| Stage          | Time         | Notes               |
| -------------- | ------------ | ------------------- |
| Backend Build  | 3-5 min      | Rust compilation    |
| Frontend Build | 2-3 min      | npm install + build |
| Image Creation | 30-60s       | Combining layers    |
| **Total**      | **6-10 min** | First build         |
| **Cached**     | **1-2 min**  | Subsequent builds   |

## 🔐 GHCR Authentication (Optional)

For publishing to GitHub Container Registry:

```bash
# Create Personal Access Token
# https://github.com/settings/tokens/new
# Scope: write:packages

# Login to GHCR
echo $GITHUB_TOKEN | docker login ghcr.io -u YOUR_USERNAME --password-stdin

# Verify login
docker info | grep ghcr.io
```

## 🚀 SSH Setup for Deployment

### Configure SSH Access

```bash
# Test connection
ssh root@pve-remote "echo 'Connected'"

# If not configured, add to ~/.ssh/config
cat >> ~/.ssh/config <<EOF

Host pve-remote
    HostName YOUR_PROXMOX_IP
    User root
    IdentityFile ~/.ssh/id_rsa
    StrictHostKeyChecking no
EOF

# Set permissions
chmod 600 ~/.ssh/config
```

### Test LXC Access

```bash
# Test LXC 112 access
ssh root@pve-remote "pct exec 112 -- echo 'LXC 112 accessible'"

# Check Docker in LXC
ssh root@pve-remote "pct exec 112 -- docker --version"
```

## ✅ Final Verification

Run this complete verification script:

```bash
#!/bin/bash
echo "🔍 Verifying Mac Build Environment..."
echo ""

# Docker
echo "1. Docker Desktop:"
if docker info >/dev/null 2>&1; then
    echo "   ✅ Running"
    docker --version
else
    echo "   ❌ Not running"
fi
echo ""

# Git
echo "2. Git:"
git --version
echo ""

# SSH
echo "3. SSH to pve-remote:"
if ssh root@pve-remote "echo 'Connected'" 2>/dev/null; then
    echo "   ✅ Connected"
else
    echo "   ⚠️  Not configured"
fi
echo ""

# LXC 112
echo "4. LXC 112 Access:"
if ssh root@pve-remote "pct exec 112 -- echo 'Accessible'" 2>/dev/null; then
    echo "   ✅ Accessible"
else
    echo "   ⚠️  Not accessible"
fi
echo ""

# Disk Space
echo "5. Disk Space:"
df -h / | tail -1 | awk '{print "   Available: " $4}'
echo ""

# Docker Resources
echo "6. Docker Resources:"
docker info 2>/dev/null | grep -E "CPUs|Total Memory" | sed 's/^/   /'
echo ""

echo "✅ Verification complete!"
```

## 📚 Next Steps

1. **Verify Docker is running** (should be started now)
2. **Test build** with `/deploy-staging`
3. **Configure SSH** if needed for deployment
4. **Set up GHCR** authentication for publishing

## 🎉 You're Ready!

Your Mac is now configured to build Flasharr Docker images. Use these commands:

```bash
# Development (no Docker needed)
/deploy-local

# Staging (build on Mac, deploy to LXC)
/deploy-staging

# Publish to GHCR
/publish-ghcr

# Production (deploy from GHCR)
/deploy-production
```

## 📞 Support

If you encounter issues:

1. Check Docker Desktop is running (menu bar icon)
2. Verify resources in Settings > Resources
3. Check logs: `debug_log/staging-docker-build.log`
4. Clean up: `docker system prune -a`
5. Restart Docker Desktop
