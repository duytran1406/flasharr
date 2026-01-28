#!/bin/bash
# Flasharr Docker Build and Deploy Script

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Flasharr Docker Deployment ===${NC}"
echo ""

# Check if appData exists
if [ ! -d "appData" ]; then
    echo -e "${YELLOW}Creating appData directory structure...${NC}"
    mkdir -p appData/{config,data,downloads,logs}
    echo -e "${GREEN}✓${NC} appData created"
    echo -e "${YELLOW}Note: Configure Fshare credentials and API key through the web UI after startup${NC}"
    echo ""
fi

# Build Docker image
echo -e "${YELLOW}Building Docker image...${NC}"
docker-compose build

echo -e "${GREEN}✓${NC} Build complete"
echo ""

# Start containers
echo -e "${YELLOW}Starting Flasharr...${NC}"
docker-compose up -d

echo -e "${GREEN}✓${NC} Flasharr started"
echo ""

# Wait for health check
echo -e "${YELLOW}Waiting for health check...${NC}"
sleep 5

# Check if container is running
if docker-compose ps | grep -q "Up"; then
    echo -e "${GREEN}✅ Flasharr is running!${NC}"
    echo ""
    echo -e "Access Flasharr at: ${BLUE}http://localhost:8484${NC}"
    echo ""
    echo -e "Useful commands:"
    echo -e "  View logs:    ${YELLOW}docker-compose logs -f${NC}"
    echo -e "  Stop:         ${YELLOW}docker-compose down${NC}"
    echo -e "  Restart:      ${YELLOW}docker-compose restart${NC}"
    echo -e "  Rebuild:      ${YELLOW}docker-compose up -d --build${NC}"
else
    echo -e "${RED}❌ Failed to start Flasharr${NC}"
    echo -e "Check logs: ${YELLOW}docker-compose logs${NC}"
    exit 1
fi
