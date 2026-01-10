#!/bin/bash
# Setup script for Fshare-Arr Bridge appdata initialization

set -e

APPDATA_DIR="/bulk-storage/appdata/fshare-arr-bridge"

echo "🚀 Initializing Fshare-Arr Bridge appdata..."

# Create appdata directories
mkdir -p "$APPDATA_DIR"
mkdir -p "$APPDATA_DIR/pyload/config"
mkdir -p "$APPDATA_DIR/pyload/downloads"
mkdir -p "$APPDATA_DIR/pyload/plugins"

# Copy .env.example to appdata if .env doesn't exist
if [ ! -f "$APPDATA_DIR/.env" ]; then
    echo "📝 Creating .env file from template..."
    cp .env.example "$APPDATA_DIR/.env"
    echo "⚠️  Please edit $APPDATA_DIR/.env with your credentials!"
else
    echo "✅ .env file already exists"
fi

# Set proper permissions
chmod 600 "$APPDATA_DIR/.env"
chown -R 1000:1000 "$APPDATA_DIR/pyload"

echo "✅ Appdata initialization complete!"
echo ""
echo "Next steps:"
echo "1. Edit $APPDATA_DIR/.env with your Fshare credentials"
echo "2. Run: docker compose up -d"
