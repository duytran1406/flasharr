#!/bin/bash
# Reset Flasharr appData on LXC 112
# This clears all data including accounts, settings, and downloads

set -e

# Colors
BLUE='\033[0;34m'
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

# Configuration
LXC_ID="112"
LXC_HOST="pve-remote"
APPDATA_DIR="/mnt/appdata/flasharr"
DOWNLOAD_DIR="/data/flasharr-download"

echo ""
echo -e "${BLUE}╔═══════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       🧹 Flasharr Data Reset (LXC 112) 🧹           ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${RED}⚠️  This will delete ALL data on LXC ${LXC_ID}:${NC}"
echo -e "${YELLOW}   - FShare accounts${NC}"
echo -e "${YELLOW}   - Sonarr/Radarr settings${NC}"
echo -e "${YELLOW}   - Download history${NC}"
echo -e "${YELLOW}   - All configuration${NC}"
echo -e "${YELLOW}   - Database${NC}"
echo -e "${YELLOW}   - Downloaded files${NC}"
echo ""
echo -e "${CYAN}AppData location:${NC} ${APPDATA_DIR}"
echo -e "${CYAN}Download location:${NC} ${DOWNLOAD_DIR}"
echo ""

# Check if container is accessible
echo -e "${CYAN}🔍 Checking LXC ${LXC_ID} accessibility...${NC}"
if ! ssh root@${LXC_HOST} "pct status ${LXC_ID}" >/dev/null 2>&1; then
    echo -e "${RED}✗ Cannot access LXC ${LXC_ID}${NC}"
    exit 1
fi
echo -e "${GREEN}✓ LXC ${LXC_ID} is accessible${NC}"
echo ""

# Check if container is running
echo -e "${CYAN}📊 Checking container status...${NC}"
ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
    if docker ps | grep -q flasharr; then
        echo \"⚠️  Flasharr container is running\"
        docker ps | grep flasharr
    else
        echo \"✓ No running Flasharr container\"
    fi
'" || true
echo ""

# Show current data
echo -e "${CYAN}📁 Current data:${NC}"
ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
    if [ -d \"${APPDATA_DIR}\" ]; then
        echo \"AppData contents:\"
        du -sh ${APPDATA_DIR}/* 2>/dev/null || echo \"  (empty)\"
    else
        echo \"AppData directory does not exist\"
    fi
    echo \"\"
    if [ -d \"${DOWNLOAD_DIR}\" ]; then
        echo \"Download directory:\"
        du -sh ${DOWNLOAD_DIR} 2>/dev/null || echo \"  (empty)\"
    else
        echo \"Download directory does not exist\"
    fi
'" || true
echo ""

# Confirm
echo -e "${RED}⚠️  This action cannot be undone!${NC}"
read -p "Are you sure you want to delete ALL data? (y/N) " -n 1 -r
echo ""
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    # Stop container
    echo -e "${YELLOW}[1/4]${NC} 🛑 Stopping Flasharr container..."
    ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
        cd /opt/flasharr 2>/dev/null || true
        docker compose down 2>/dev/null || true
        docker stop flasharr 2>/dev/null || true
        docker rm flasharr 2>/dev/null || true
        echo \"Container stopped\"
    '" && echo -e "${GREEN}      ✓ Container stopped${NC}" || echo -e "${YELLOW}      ⚠️  No container to stop${NC}"
    echo ""
    
    # Clear appData
    echo -e "${YELLOW}[2/4]${NC} 🗑️  Clearing appData..."
    ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
        if [ -d \"${APPDATA_DIR}\" ]; then
            rm -rf ${APPDATA_DIR}/*
            echo \"AppData cleared\"
        else
            echo \"AppData directory does not exist\"
        fi
    '" && echo -e "${GREEN}      ✓ AppData cleared${NC}"
    echo ""
    
    # Clear downloads
    echo -e "${YELLOW}[3/4]${NC} 🗑️  Clearing downloads..."
    ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
        if [ -d \"${DOWNLOAD_DIR}\" ]; then
            rm -rf ${DOWNLOAD_DIR}/*
            echo \"Downloads cleared\"
        else
            echo \"Download directory does not exist\"
        fi
    '" && echo -e "${GREEN}      ✓ Downloads cleared${NC}"
    echo ""
    
    # Verify
    echo -e "${YELLOW}[4/4]${NC} ✅ Verifying cleanup..."
    ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
        echo \"AppData:\"
        ls -la ${APPDATA_DIR} 2>/dev/null || echo \"  (empty)\"
        echo \"\"
        echo \"Downloads:\"
        ls -la ${DOWNLOAD_DIR} 2>/dev/null || echo \"  (empty)\"
    '" || true
    echo ""
    
    echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}✅ All data cleared successfully!${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "${CYAN}Next steps:${NC}"
    echo -e "${CYAN}   1. Deploy fresh image: ./scripts/deploy/staging.sh${NC}"
    echo -e "${CYAN}   2. Setup wizard will appear on first access${NC}"
    echo -e "${CYAN}   3. Access at: http://[LXC-IP]:8484${NC}"
    echo ""
else
    echo -e "${YELLOW}❌ Cancelled.${NC}"
    exit 1
fi
