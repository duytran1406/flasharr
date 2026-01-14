#!/bin/bash
# Flasharr Deployment Script for LXC 112
# Version: 0.1.8-beta

set -e

echo "=== Flasharr v0.1.8-beta Deployment ==="
echo ""

# Navigate to project directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
cd "$SCRIPT_DIR"

# Pull latest code
if [ "$SKIP_PULL" != "true" ]; then
    echo "📥 Pulling latest code from repository..."
    git fetch lxc106 || echo "⚠️  Git fetch failed"
    git pull lxc106 v1.0.0alpha || echo "⚠️  Git pull failed"
else
    echo "⏭️  Skipping git pull as requested"
fi

# Show version
echo ""
echo "📌 Version: $(cat VERSION)"
echo ""

# Stop current container
echo "🛑 Stopping current container..."
docker compose down || true
docker rm -f flasharr || true

# Rebuild image
echo "🔨 Building new image..."
docker compose build

# Start container
echo "🚀 Starting Flasharr..."
docker compose up -d

# Wait for container to start
echo "⏳ Waiting for container to start..."
sleep 5

# Show logs
echo ""
echo "📋 Container logs:"
docker compose logs --tail=30 flasharr

echo ""
echo "✅ Deployment complete!"
echo ""
echo "🌐 Access Flasharr at: http://localhost:8484"
echo "📚 Documentation: $SCRIPT_DIR/flasharr_docs/"
echo ""
