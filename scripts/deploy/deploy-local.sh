#!/bin/bash
# Flasharr V3 Local Development Script
# Builds and runs the Rust backend and Svelte frontend locally

set -e

# Colors
BLUE='\033[0;34m'
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Configuration
BACKEND_PORT=8484
FRONTEND_PORT=5173
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"

# Helper for progress
progress() {
    local P=$1
    local MSG=$2
    local BAR_LEN=30
    local FILLED=$(($P * $BAR_LEN / 100))
    local EMPTY=$(($BAR_LEN - $FILLED))
    local BAR=$(printf "%${FILLED}s" | tr ' ' '#')
    local GAP=$(printf "%${EMPTY}s" | tr ' ' '-')
    echo -ne "\r\033[K${CYAN}[${BAR}${GAP}] ${P}%${NC} - ${MSG}"
    if [ "$P" -eq 100 ]; then echo ""; fi
}

echo -e "${BLUE}=== 🚀 Flasharr V3 Local Deploy ===${NC}"
echo -e "Backend: ${YELLOW}localhost:$BACKEND_PORT${NC}"
echo -e "Frontend: ${YELLOW}localhost:$FRONTEND_PORT${NC}"
echo ""

cd "$SCRIPT_DIR"

# 1. Stop existing processes
progress 5 "Stopping existing processes..."
pkill -f "target/release/flasharr" 2>/dev/null || true
sleep 1

# 2. Build Frontend
progress 15 "Building Svelte frontend..."
cd "$SCRIPT_DIR/frontend"
if [ ! -d "node_modules" ]; then
    npm install > /tmp/npm_install.log 2>&1
fi
npm run build > /tmp/frontend_build.log 2>&1

# 3. Prepare Static Directory for Backend
progress 40 "Preparing static assets..."
mkdir -p "$SCRIPT_DIR/backend/static"
rm -rf "$SCRIPT_DIR/backend/static/*"
cp -r build/* "$SCRIPT_DIR/backend/static/"

# 4. Build Backend
progress 55 "Building Rust backend..."
cd "$SCRIPT_DIR/backend"
cargo build --release > /tmp/cargo_build.log 2>&1

# 5. Start Unified App
progress 80 "Starting Flasharr V3..."
./target/release/flasharr > /tmp/flasharr_backend.log 2>&1 &
BACKEND_PID=$!
sleep 3

# 6. Health Check
progress 95 "Verifying health..."
if curl -s "http://localhost:8484/health" | grep -q "ok"; then
    progress 100 "Flasharr V3 is LIVE!"
else
    echo -e "\n${RED}❌ Backend failed to start. Check /tmp/flasharr_backend.log${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}✅ Flasharr V3 Unified Build is running!${NC}"
echo -e "   URL: ${CYAN}http://localhost:8484${NC}"
echo -e "   PID: ${CYAN}$BACKEND_PID${NC}"
echo ""
echo -e "🛑 To stop: ${YELLOW}kill $BACKEND_PID${NC}"

# Open browser (macOS)
if command -v open &> /dev/null; then
    open "http://localhost:$FRONTEND_PORT"
fi
