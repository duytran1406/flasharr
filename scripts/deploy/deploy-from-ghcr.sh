#!/bin/bash
# Deploy Flasharr from GHCR to LXC 112
# This script removes the old installation and deploys from the published image

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}╔═══════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   Flasharr GHCR Deployment to LXC    ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════╝${NC}"
echo ""

# Configuration
LXC_ID="112"
LXC_HOST="pve-remote"
GHCR_IMAGE="ghcr.io/duytran1406/flasharr:latest"
DEPLOY_DIR="/opt/flasharr"
APPDATA_DIR="/mnt/appdata/flasharr"
DOWNLOAD_DIR="/data/flasharr-download"

echo -e "${YELLOW}→ Connecting to LXC ${LXC_ID} on ${LXC_HOST}...${NC}"

# Step 1: Stop and remove old containers
echo -e "${YELLOW}→ Step 1: Removing old Flasharr installation...${NC}"
ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
    cd /opt/flasharr 2>/dev/null || true
    docker compose down 2>/dev/null || true
    docker stop flasharr 2>/dev/null || true
    docker rm flasharr 2>/dev/null || true
    echo \"Old containers removed\"
'"

# Step 2: Clean up old images (optional - keeps data)
echo -e "${YELLOW}→ Step 2: Cleaning up old Docker images...${NC}"
ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
    docker images | grep flasharr | awk \"{print \\\$3}\" | xargs -r docker rmi -f 2>/dev/null || true
    echo \"Old images cleaned\"
'"

# Step 3: Pull new image from GHCR
echo -e "${YELLOW}→ Step 3: Pulling image from GHCR...${NC}"
ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
    docker pull ${GHCR_IMAGE}
    echo \"Image pulled successfully\"
'"

# Step 4: Create new docker-compose.yml
echo -e "${YELLOW}→ Step 4: Creating docker-compose.yml...${NC}"
ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
    mkdir -p ${DEPLOY_DIR}
    cat > ${DEPLOY_DIR}/docker-compose.yml <<EOF
version: \"3.8\"

services:
  flasharr:
    image: ${GHCR_IMAGE}
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
    echo \"docker-compose.yml created\"
'"

# Step 5: Start Flasharr
echo -e "${YELLOW}→ Step 5: Starting Flasharr...${NC}"
ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
    cd ${DEPLOY_DIR}
    docker compose up -d
    echo \"Flasharr started\"
'"

# Step 6: Wait for health check
echo -e "${YELLOW}→ Step 6: Waiting for Flasharr to be healthy...${NC}"
sleep 10

ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
    for i in {1..30}; do
        if docker exec flasharr curl -f http://localhost:8484/health 2>/dev/null; then
            echo \"Flasharr is healthy!\"
            exit 0
        fi
        echo \"Waiting... (\$i/30)\"
        sleep 2
    done
    echo \"Health check timeout\"
    exit 1
'"

# Step 7: Show status
echo ""
echo -e "${GREEN}✓ Deployment complete!${NC}"
echo ""
echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo -e "${GREEN}Flasharr Status:${NC}"
ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- docker ps -a | grep flasharr"
echo ""
echo -e "${GREEN}Image Info:${NC}"
ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- docker images | grep flasharr"
echo ""
echo -e "${GREEN}Access Flasharr at:${NC}"
echo -e "${BLUE}http://$(ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- hostname -I | awk '{print \$1}'"):8484${NC}"
echo ""
echo -e "${GREEN}Logs:${NC} ssh root@${LXC_HOST} \"pct exec ${LXC_ID} -- docker logs -f flasharr\""
echo -e "${BLUE}═══════════════════════════════════════${NC}"
