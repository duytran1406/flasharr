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
DOWNLOAD_DIR="/data/flasharr-download"

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

# Step 1: Build Docker Image
echo -e "${YELLOW}[1/6]${NC} 🏗️  Building Docker image on Mac..."
cd "$PROJECT_ROOT"

# Get version info
VERSION=$(git describe --tags --always 2>/dev/null || echo "dev")
BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
VCS_REF=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")

echo -e "${CYAN}      📌 Version: ${VERSION}${NC}"
echo -e "${CYAN}      📅 Build Date: ${BUILD_DATE}${NC}"
echo -e "${CYAN}      🔖 Git Commit: ${VCS_REF}${NC}"

if docker build \
    --build-arg VERSION="${VERSION}" \
    --build-arg BUILD_DATE="${BUILD_DATE}" \
    --build-arg VCS_REF="${VCS_REF}" \
    -t "${IMAGE_NAME}" \
    -f Dockerfile \
    . 2>&1 | tee debug_log/staging-docker-build.log | grep -E "Step|Successfully|ERROR"; then
    echo -e "${GREEN}      ✓ Docker image built successfully${NC}"
else
    echo -e "${RED}      ✗ Docker build failed - check debug_log/staging-docker-build.log${NC}"
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

# Step 3: Transfer to LXC
echo -e "${YELLOW}[3/6]${NC} 📤 Transferring image to LXC ${LXC_ID}..."
if scp -q "/tmp/${IMAGE_TAR}" "root@${LXC_HOST}:/tmp/${IMAGE_TAR}"; then
    echo -e "${GREEN}      ✓ Transfer complete${NC}"
else
    echo -e "${RED}      ✗ Transfer failed${NC}"
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
      - ${DOWNLOAD_DIR}:/appData/downloads
    environment:
      - FLASHARR_APPDATA_DIR=/appData
      - RUST_LOG=flasharr=info,tower_http=info
      - TZ=Asia/Bangkok
    healthcheck:
      test: [\"CMD\", \"curl\", \"-f\", \"http://localhost:8484/health\"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
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
