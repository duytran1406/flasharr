#!/bin/bash
# Flasharr AppData Migration Script
# Migrates existing data to new appData structure

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Flasharr AppData Migration ===${NC}"
echo ""

# Get script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
cd "$SCRIPT_DIR"

# Define paths
APPDATA_DIR="./appData"
BACKEND_DIR="./backend"

echo -e "${YELLOW}Creating appData directory structure...${NC}"
mkdir -p "$APPDATA_DIR/config"
mkdir -p "$APPDATA_DIR/data"
mkdir -p "$APPDATA_DIR/downloads"
mkdir -p "$APPDATA_DIR/logs"

echo -e "${GREEN}✓${NC} Directories created"

# Migrate config.toml
if [ -f "$BACKEND_DIR/config.toml" ]; then
    echo -e "${YELLOW}Migrating config.toml...${NC}"
    cp "$BACKEND_DIR/config.toml" "$APPDATA_DIR/config/config.toml"
    echo -e "${GREEN}✓${NC} config.toml migrated"
else
    echo -e "${YELLOW}⚠${NC}  No config.toml found in backend/"
fi

# Migrate .env
if [ -f "./.env" ]; then
    echo -e "${YELLOW}Migrating .env...${NC}"
    cp "./.env" "$APPDATA_DIR/config/.env"
    echo -e "${GREEN}✓${NC} .env migrated"
else
    echo -e "${YELLOW}⚠${NC}  No .env found"
fi

# Migrate database
if [ -f "$BACKEND_DIR/flasharr.db" ]; then
    echo -e "${YELLOW}Migrating flasharr.db...${NC}"
    cp "$BACKEND_DIR/flasharr.db" "$APPDATA_DIR/data/flasharr.db"
    echo -e "${GREEN}✓${NC} flasharr.db migrated"
else
    echo -e "${YELLOW}⚠${NC}  No flasharr.db found"
fi

# Create backup of old files
echo -e "${YELLOW}Creating backup of old files...${NC}"
BACKUP_DIR="./backup_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$BACKUP_DIR"

[ -f "$BACKEND_DIR/config.toml" ] && cp "$BACKEND_DIR/config.toml" "$BACKUP_DIR/"
[ -f "./.env" ] && cp "./.env" "$BACKUP_DIR/"
[ -f "$BACKEND_DIR/flasharr.db" ] && cp "$BACKEND_DIR/flasharr.db" "$BACKUP_DIR/"

echo -e "${GREEN}✓${NC} Backup created at $BACKUP_DIR"

echo ""
echo -e "${GREEN}=== Migration Complete! ===${NC}"
echo ""
echo -e "AppData structure:"
echo -e "  ${BLUE}$APPDATA_DIR/${NC}"
echo -e "  ├── config/"
echo -e "  │   ├── config.toml"
echo -e "  │   └── .env"
echo -e "  ├── data/"
echo -e "  │   └── flasharr.db"
echo -e "  ├── downloads/"
echo -e "  └── logs/"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo -e "1. Verify appData structure: ls -la $APPDATA_DIR"
echo -e "2. Start Flasharr (it will automatically use appData)"
echo -e "3. Old files backed up to: $BACKUP_DIR"
echo ""
