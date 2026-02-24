#!/bin/bash
# Reset Flasharr Downloads Database on LXC 112
# Clears only download records from the database
# Preserves: config, accounts, settings, logs, and downloaded files

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

echo ""
echo -e "${BLUE}╔═══════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   🧹 Flasharr Downloads DB Reset (LXC 112) 🧹      ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${YELLOW}⚠️  This will clear on LXC ${LXC_ID}:${NC}"
echo -e "${YELLOW}   - All download task records from the database${NC}"
echo ""
echo -e "${GREEN}✓ Preserved: config, accounts, settings, logs, files${NC}"
echo ""

# Check accessibility
echo -e "${CYAN}🔍 Checking LXC ${LXC_ID}...${NC}"
if ! ssh root@${LXC_HOST} "pct status ${LXC_ID}" >/dev/null 2>&1; then
    echo -e "${RED}✗ Cannot access LXC ${LXC_ID}${NC}"
    exit 1
fi
echo -e "${GREEN}✓ LXC ${LXC_ID} is accessible${NC}"
echo ""

# Show current download count
echo -e "${CYAN}📊 Current download records:${NC}"
ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
    DB_FILE=\$(find ${APPDATA_DIR} -name \"*.db\" -type f 2>/dev/null | head -1)
    if [ -n \"\$DB_FILE\" ]; then
        echo \"  Database: \$DB_FILE\"
        COUNT=\$(sqlite3 \"\$DB_FILE\" \"SELECT COUNT(*) FROM downloads;\" 2>/dev/null || echo \"unknown\")
        echo \"  Download records: \$COUNT\"
    else
        echo \"  No database found\"
    fi
'" || true
echo ""

# Confirm
echo -e "${RED}⚠️  Delete all download records from database?${NC}"
read -p "Are you sure? (y/N) " -n 1 -r
echo ""
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${YELLOW}[1/2]${NC} 🗑️  Clearing downloads table..."
    ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
        DB_FILE=\$(find ${APPDATA_DIR} -name \"*.db\" -type f 2>/dev/null | head -1)
        if [ -n \"\$DB_FILE\" ]; then
            sqlite3 \"\$DB_FILE\" \"DELETE FROM downloads;\"
            echo \"Downloads table cleared\"
            sqlite3 \"\$DB_FILE\" \"VACUUM;\"
            echo \"Database vacuumed\"
        else
            echo \"No database found\"
            exit 1
        fi
    '" && echo -e "${GREEN}      ✓ Downloads table cleared${NC}" || { echo -e "${RED}      ✗ Failed${NC}"; exit 1; }
    echo ""

    # Restart container to pick up clean state
    echo -e "${YELLOW}[2/2]${NC} 🔄 Restarting Flasharr container..."
    ssh root@${LXC_HOST} "pct exec ${LXC_ID} -- bash -c '
        docker restart flasharr 2>/dev/null && echo \"Container restarted\" || echo \"No container to restart\"
    '" && echo -e "${GREEN}      ✓ Container restarted${NC}"
    echo ""

    echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}✅ Download records cleared! Everything else preserved.${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
    echo ""
else
    echo -e "${YELLOW}❌ Cancelled.${NC}"
    exit 1
fi
