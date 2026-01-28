#!/bin/bash
# Reset Flasharr appData to force setup wizard
# This clears all data including accounts, settings, and downloads

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
APPDATA_DIR="$SCRIPT_DIR/../backend/appData"
DB_FILE="$SCRIPT_DIR/../backend/flasharr.db"

echo "🧹 Flasharr appData Reset"
echo "========================="
echo ""
echo "⚠️  This will delete ALL data:"
echo "   - FShare accounts"
echo "   - Sonarr/Radarr settings"
echo "   - Download history"
echo "   - All configuration"
echo "   - Database (flasharr.db)"
echo ""
echo "AppData location: $APPDATA_DIR"
echo "Database location: $DB_FILE"
echo ""

# Check if backend is running
BACKEND_PID=$(pgrep -f "target/release/flasharr" || true)
if [ -n "$BACKEND_PID" ]; then
    echo "⚠️  Backend is currently running (PID: $BACKEND_PID)"
    echo "   It will be stopped before cleaning."
    echo ""
fi

# Check if appData exists
if [ ! -d "$APPDATA_DIR" ] && [ ! -f "$DB_FILE" ]; then
    echo "✅ No appData or database found - already clean!"
    exit 0
fi

# Show what will be deleted
if [ -d "$APPDATA_DIR" ]; then
    echo "AppData contents:"
    ls -la "$APPDATA_DIR" 2>/dev/null || echo "  (empty)"
fi

if [ -f "$DB_FILE" ]; then
    echo ""
    echo "Database file:"
    ls -lh "$DB_FILE"
fi
echo ""

# Confirm
read -p "Are you sure? (y/N) " -n 1 -r
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    # Stop backend if running
    if [ -n "$BACKEND_PID" ]; then
        echo "🛑 Stopping backend..."
        pkill -f "target/release/flasharr" || true
        sleep 1
        echo "✅ Backend stopped"
    fi
    
    # Delete appData
    if [ -d "$APPDATA_DIR" ]; then
        rm -rf "$APPDATA_DIR"/*
        echo "✅ appData cleared"
    fi
    
    # Delete database
    if [ -f "$DB_FILE" ]; then
        rm -f "$DB_FILE"
        echo "✅ Database deleted"
    fi
    
    echo ""
    echo "✅ All data cleared! Setup wizard will appear on next start."
    echo ""
    echo "To restart the backend, run:"
    echo "  cd $SCRIPT_DIR/../backend && ./target/release/flasharr"
else
    echo "❌ Cancelled."
    exit 1
fi
