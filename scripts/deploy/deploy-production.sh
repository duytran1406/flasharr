#!/bin/bash
# Deploy Flasharr to LXC112 on pve-remote (Remote Build)
# This version builds on the remote server instead of locally

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

REMOTE_HOST="pve-remote"
LXC_ID="112"
DEPLOY_DIR="/root/flasharr"

echo -e "${BLUE}=== Deploying Flasharr to LXC112 on pve-remote ===${NC}"
echo ""

# Step 1: Create deployment package
echo -e "${YELLOW}Step 1: Creating deployment package...${NC}"
COPYFILE_DISABLE=1 tar --exclude='node_modules' \
    --exclude='target' \
    --exclude='.git' \
    --exclude='appData' \
    --exclude='debug_log' \
    --exclude='*.tar.gz' \
    --exclude='.agent' \
    --exclude='.DS_Store' \
    --exclude='._*' \
    -czf /tmp/flasharr-deploy.tar.gz .
echo -e "${GREEN}✓${NC} Package created"
echo ""

# Step 2: Copy package to pve-remote
echo -e "${YELLOW}Step 2: Copying package to pve-remote...${NC}"
scp /tmp/flasharr-deploy.tar.gz ${REMOTE_HOST}:/tmp/
echo -e "${GREEN}✓${NC} Package copied"
echo ""

# Step 3: Deploy on LXC112
echo -e "${YELLOW}Step 3: Deploying on LXC112...${NC}"
ssh ${REMOTE_HOST} << 'ENDSSH'
set -e

LXC_ID="112"
DEPLOY_DIR="/root/flasharr"

# Push package to LXC
pct push ${LXC_ID} /tmp/flasharr-deploy.tar.gz /tmp/flasharr-deploy.tar.gz

# Execute deployment inside LXC
pct exec ${LXC_ID} -- bash -c "
    set -e
    
    # Create deploy directory
    mkdir -p ${DEPLOY_DIR}
    cd ${DEPLOY_DIR}
    
    # Extract package
    echo 'Extracting package...'
    tar -xzf /tmp/flasharr-deploy.tar.gz
    
    # Ensure appData directory exists
    mkdir -p appData/{config,data,downloads,logs}
    
    # Build Docker image using regular Dockerfile (builds from source)
    echo 'Building Docker image...'
    docker build -t flasharr:latest .
    
    # Stop existing container
    echo 'Stopping existing container...'
    docker-compose -f docker-compose.production.yml down || true
    
    # Start new container
    echo 'Starting new container...'
    docker-compose -f docker-compose.production.yml up -d
    
    # Cleanup
    rm /tmp/flasharr-deploy.tar.gz
    
    echo 'Deployment complete!'
"

# Cleanup on pve-remote
rm /tmp/flasharr-deploy.tar.gz

ENDSSH

echo -e "${GREEN}✓${NC} Deployed to LXC112"
echo ""

# Cleanup local temp file
rm /tmp/flasharr-deploy.tar.gz

# Step 4: Verify deployment
echo -e "${YELLOW}Step 4: Verifying deployment...${NC}"
sleep 3

ssh ${REMOTE_HOST} "pct exec ${LXC_ID} -- docker ps" | grep flasharr && \
    echo -e "${GREEN}✅ Flasharr is running on LXC112!${NC}" || \
    echo -e "${RED}❌ Deployment verification failed${NC}"

echo ""
echo -e "${BLUE}Access Flasharr at: https://fshare.blavkbeav.com/${NC}"
echo ""
echo -e "Useful commands:"
echo -e "  View logs:    ${YELLOW}ssh pve-remote 'pct exec 112 -- docker-compose -f ${DEPLOY_DIR}/docker-compose.production.yml logs -f'${NC}"
echo -e "  Restart:      ${YELLOW}ssh pve-remote 'pct exec 112 -- docker-compose -f ${DEPLOY_DIR}/docker-compose.production.yml restart'${NC}"
echo -e "  Stop:         ${YELLOW}ssh pve-remote 'pct exec 112 -- docker-compose -f ${DEPLOY_DIR}/docker-compose.production.yml down'${NC}"
