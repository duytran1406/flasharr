#!/bin/bash
# Flasharr Quick Install Script
# Usage: curl -sSL https://raw.githubusercontent.com/duytran1406/flasharr/main/install.sh | bash

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔═══════════════════════════════════════╗${NC}"
echo -e "${BLUE}║     Flasharr Installation Script     ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════╝${NC}"
echo ""

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo -e "${RED}❌ Docker is not installed${NC}"
    echo -e "${YELLOW}Please install Docker first: https://docs.docker.com/get-docker/${NC}"
    exit 1
fi

# Check if Docker Compose is available
if ! docker compose version &> /dev/null; then
    echo -e "${RED}❌ Docker Compose is not available${NC}"
    echo -e "${YELLOW}Please install Docker Compose: https://docs.docker.com/compose/install/${NC}"
    exit 1
fi

echo -e "${GREEN}✓${NC} Docker is installed"
echo -e "${GREEN}✓${NC} Docker Compose is available"
echo ""

# Ask for installation directory
read -p "Installation directory [./flasharr]: " INSTALL_DIR
INSTALL_DIR=${INSTALL_DIR:-./flasharr}

# Create installation directory
mkdir -p "$INSTALL_DIR"
cd "$INSTALL_DIR"

echo -e "${YELLOW}📥 Downloading docker-compose.yml...${NC}"

# Download docker-compose.yml
curl -sSL https://raw.githubusercontent.com/duytran1406/flasharr/main/docker-compose.production.yml -o docker-compose.yml

echo -e "${GREEN}✓${NC} Downloaded docker-compose.yml"
echo ""

# Create appData directory
mkdir -p appData

echo -e "${YELLOW}🚀 Starting Flasharr...${NC}"

# Pull and start
docker compose pull
docker compose up -d

echo ""
echo -e "${GREEN}✅ Flasharr is now running!${NC}"
echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "  Access Flasharr at: ${GREEN}http://localhost:8484${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo -e "Useful commands:"
echo -e "  ${YELLOW}View logs:${NC}    docker compose logs -f"
echo -e "  ${YELLOW}Stop:${NC}         docker compose down"
echo -e "  ${YELLOW}Restart:${NC}      docker compose restart"
echo -e "  ${YELLOW}Update:${NC}       docker compose pull && docker compose up -d"
echo ""
