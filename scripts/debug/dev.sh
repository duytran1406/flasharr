#!/bin/bash
# Flasharr V3 Hot-Reload Development Script
# Runs backend and frontend concurrently for instant adjustments

BLUE='\033[0;34m'
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

# Get project root (2 levels up from scripts/debug/)
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/../.."

echo -e "${BLUE}=== 🛠️ Flasharr V3 DEBUG MODE ===${NC}"
echo -e "Frontend: ${YELLOW}http://localhost:5173 (Hot-Reload Enabled)${NC}"
echo -e "Backend:  ${YELLOW}http://localhost:8484 (API Only)${NC}"
echo ""

# 1. Kill existing processes
echo -e "${CYAN}[1/3]${NC} Cleaning up previous sessions..."
pkill -f "target/debug/flasharr" 2>/dev/null || true
pkill -f "vite" 2>/dev/null || true
sleep 1

# 2. Start Backend in background
echo -e "${CYAN}[2/3]${NC} Starting Rust Backend (Debug Mode)..."
cd "$PROJECT_ROOT/backend"
cargo run > "$PROJECT_ROOT/debug_log/run.log" 2>&1 &
BACKEND_PID=$!
echo -e "${GREEN}Backend started (PID: $BACKEND_PID)${NC}"

# Wait for backend to start
sleep 3

# 3. Start Frontend
echo -e "${CYAN}[3/3]${NC} Launching Frontend Dev Server..."
cd "$PROJECT_ROOT/frontend"
npm run dev

# Cleanup on exit
trap "kill $BACKEND_PID 2>/dev/null" EXIT
