#!/bin/bash
# Flasharr Staging Deployment - Build on Mac, Deploy to LXC 112
# Builds Docker image locally and deploys to staging environment

set -e

# Colors and Icons
BLUE='\033[0;34m'
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

# Configuration
LXC_ID="112"
LXC_HOST="pve-remote"
IMAGE_NAME="flasharr:staging"
IMAGE_TAR="flasharr-staging.tar"
DEPLOY_DIR="/opt/flasharr"
APPDATA_DIR="/mnt/appdata/flasharr"
DOWNLOAD_DIR="/data/downloads"

# Get project root
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/../.."

# Header
echo ""
echo -e "${BLUE}╔═══════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║         🚀 Flasharr Staging Deployment 🚀            ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${CYAN}📍 Target:${NC}    LXC ${LXC_ID} on ${LXC_HOST}"
echo -e "${CYAN}🐳 Image:${NC}     ${IMAGE_NAME}"
echo -e "${CYAN}📦 Build:${NC}     Local (Mac)"
echo ""

# Step 0: Check Docker Daemon
echo -e "${YELLOW}[0/6]${NC} 🐳 Checking Docker Desktop..."
if ! docker info >/dev/null 2>&1; then
    echo -e "${YELLOW}      ⚠️  Docker Desktop is not running${NC}"
    echo -e "${CYAN}      🚀 Starting Docker Desktop...${NC}"
    open -a Docker
    
    # Wait for Docker to start (max 60 seconds)
    echo -e "${CYAN}      ⏳ Waiting for Docker to start...${NC}"
    for i in {1..30}; do
        if docker info >/dev/null 2>&1; then
            echo -e "${GREEN}      ✓ Docker Desktop is ready${NC}"
            break
        fi
        if [ $i -eq 30 ]; then
            echo -e "${RED}      ✗ Docker failed to start after 60 seconds${NC}"
            echo -e "${YELLOW}      Please start Docker Desktop manually and try again${NC}"
            exit 1
        fi
        sleep 2
    done
else
    echo -e "${GREEN}      ✓ Docker Desktop is running${NC}"
fi
echo ""

# Step 1: Build Docker Image
echo -e "${YELLOW}[1/6]${NC} 🏗️  Building Docker image on Mac..."
cd "$PROJECT_ROOT"
mkdir -p debug_log

# Get version info
VERSION=$(git describe --tags --always 2>/dev/null || echo "dev")
BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
VCS_REF=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")

echo -e "${CYAN}      📌 Version: ${VERSION}${NC}"
echo -e "${CYAN}      📅 Build Date: ${BUILD_DATE}${NC}"
echo -e "${CYAN}      🔖 Git Commit: ${VCS_REF}${NC}"
echo -e "${CYAN}      🖥️  Platform: linux/amd64 (Proxmox)${NC}"

# Create a clean build context in /tmp so macOS-locked files (.env, etc.)
# never reach Docker's context sender (they cause 'lstat: operation not permitted').
CLEAN_CTX="/tmp/flasharr-build-ctx-$$"
rm -rf "$CLEAN_CTX"
echo -e "${CYAN}      📋 Syncing build context to ${CLEAN_CTX}...${NC}"
rsync -a --quiet --ignore-errors \
    --exclude='.env' --exclude='.env.*' \
    --exclude='.git' \
    --exclude='backend/target' \
    --exclude='backend/node_modules' \
    --exclude='frontend/node_modules' \
    --exclude='frontend/build' \
    --exclude='frontend/.svelte-kit' \
    --exclude='backend/static' \
    --exclude='appData' \
    --exclude='.DS_Store' \
    --exclude='*.log' \
    --exclude='debug_log' \
    --exclude='.agent' \
    . "$CLEAN_CTX/" || true

# Work around macOS com.apple.provenance lock on ~/.docker/buildx/activity
BUILDX_TMP_CONFIG="/tmp/buildx-config-$$"
rm -rf "$BUILDX_TMP_CONFIG"
mkdir -p "$BUILDX_TMP_CONFIG/activity"

BUILDX_CONFIG="$BUILDX_TMP_CONFIG" docker buildx build \
    --platform linux/amd64 \
    --provenance=false \
    --build-arg VERSION="${VERSION}" \
    --build-arg BUILD_DATE="${BUILD_DATE}" \
    --build-arg VCS_REF="${VCS_REF}" \
    -t "${IMAGE_NAME}" \
    -f "${CLEAN_CTX}/Dockerfile" \
    --load \
    "$CLEAN_CTX" 2>&1 | tee /tmp/staging-docker-build.log
BUILD_EXIT=${PIPESTATUS[0]}

# Cleanup temp context and buildx config dir
rm -rf "$CLEAN_CTX" "$BUILDX_TMP_CONFIG"

if [ "$BUILD_EXIT" -eq 0 ]; then
    echo -e "${GREEN}      ✓ Docker image built successfully${NC}"
    # Prune dangling <none> images left by previous buildx runs
    docker image prune -f --filter "dangling=true" > /dev/null 2>&1 || true
else
    echo -e "${RED}      ✗ Docker build failed - check /tmp/staging-docker-build.log${NC}"
    tail -50 /tmp/staging-docker-build.log
    exit 1
fi
echo ""

# Step 2: Save Docker Image
echo -e "${YELLOW}[2/6]${NC} 💾 Saving Docker image to tarball..."
if docker save "${IMAGE_NAME}" -o "/tmp/${IMAGE_TAR}"; then
    IMAGE_SIZE=$(du -h "/tmp/${IMAGE_TAR}" | cut -f1)
    echo -e "${GREEN}      ✓ Image saved (${IMAGE_SIZE})${NC}"
else
    echo -e "${RED}      ✗ Failed to save image${NC}"
    exit 1
fi
echo ""

# Step 3: Transfer to Proxmox Host
echo -e "${YELLOW}[3/6]${NC} 📤 Transferring image to Proxmox host..."
if scp -q "/tmp/${IMAGE_TAR}" "root@${LXC_HOST}:/tmp/${IMAGE_TAR}"; then
    echo -e "${GREEN}      ✓ Transfer to host complete${NC}"
else
    echo -e "${RED}      ✗ Transfer failed${NC}"
    exit 1
fi
echo ""

# Step 3.5: Push to LXC Container
echo -e "${CYAN}      📦 Pushing to LXC ${LXC_ID}...${NC}"
if ssh root@${LXC_HOST} "pct push ${LXC_ID} /tmp/${IMAGE_TAR} /tmp/${IMAGE_TAR}"; then
    echo -e "${GREEN}      ✓ Image pushed to LXC${NC}"
    # Cleanup on Proxmox host
    ssh root@${LXC_HOST} "rm -f /tmp/${IMAGE_TAR}"
else
    echo -e "${RED}      ✗ Failed to push image${NC}"
    exit 1
fi

# Cleanup local tarball
rm -f "/tmp/${IMAGE_TAR}"
echo ""

# Step 4: Load Image on LXC
echo -e "${YELLOW}[4/6]${NC} 📥 Loading image on LXC ${LXC_ID}..."
ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
    echo \"Loading Docker image...\"
    if docker load -i /tmp/${IMAGE_TAR}; then
        echo \"Image loaded successfully\"
        rm -f /tmp/${IMAGE_TAR}
        exit 0
    else
        echo \"Failed to load image\"
        exit 1
    fi
'" && echo -e "${GREEN}      ✓ Image loaded${NC}" || { echo -e "${RED}      ✗ Failed to load image${NC}"; exit 1; }
echo ""

# Step 5: Stop Old Container
echo -e "${YELLOW}[5/6]${NC} 🛑 Stopping old container..."
ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
    cd ${DEPLOY_DIR} 2>/dev/null || true
    docker compose down 2>/dev/null || true
    docker stop flasharr 2>/dev/null || true
    docker rm flasharr 2>/dev/null || true
    echo \"Old container stopped\"
'" && echo -e "${GREEN}      ✓ Old container removed${NC}"
echo ""

# Step 6: Create docker-compose.yml and Start
echo -e "${YELLOW}[6/6]${NC} 🚀 Starting new container..."
ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
    mkdir -p ${DEPLOY_DIR}
    cat > ${DEPLOY_DIR}/docker-compose.yml <<EOF
version: \"3.8\"

services:
  flasharr:
    image: ${IMAGE_NAME}
    container_name: flasharr
    restart: unless-stopped
    ports:
      - \"8484:8484\"
    volumes:
      - ${APPDATA_DIR}:/appData
      - ${DOWNLOAD_DIR}:/downloads
      - /data/media:/data/media
    environment:
      - FLASHARR_APPDATA_DIR=/appData
      - RUST_LOG=flasharr=debug,tower_http=debug
      - TZ=Asia/Bangkok
    healthcheck:
      test: [\"CMD\", \"curl\", \"-f\", \"http://localhost:8484/api/health\"]
      interval: 30s
      timeout: 3s
      retries: 3
      start_period: 10s
EOF
    
    cd ${DEPLOY_DIR}
    docker compose up -d
    echo \"Container started\"
'" && echo -e "${GREEN}      ✓ Container started${NC}" || { echo -e "${RED}      ✗ Failed to start container${NC}"; exit 1; }
echo ""

# Wait and verify
echo -e "${CYAN}⏳ Waiting for health check...${NC}"
sleep 10

# Get container status
echo ""
echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}✓ Staging deployment complete!${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
echo ""

# Show status
ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
    echo \"Container Status:\"
    docker ps -a | grep flasharr
    echo \"\"
    echo \"Image Info:\"
    docker images | grep flasharr
'" || true

echo ""
echo -e "${CYAN}🌐 Access:${NC}    http://\$(ssh root@${LXC_HOST} \"pct exec ${LXC_ID} -- hostname -I | awk '{print \$1}'\"):8484"
echo -e "${CYAN}📋 Logs:${NC}     ssh root@${LXC_HOST} \"pct exec ${LXC_ID} -- docker logs -f flasharr\""
echo -e "${CYAN}🔍 Status:${NC}   ssh root@${LXC_HOST} \"pct exec ${LXC_ID} -- docker ps\""
echo ""
