#!/bin/bash
# Flasharr Deployment Script for LXC 112
# Version: 0.1.0-beta

set -e

echo "=== Flasharr v0.1.0-beta Deployment ==="
echo ""

# Navigate to project directory
cd /etc/pve/fshare-arr-bridge

# Pull latest code
echo "📥 Pulling latest code from repository..."
git fetch lxc106
git pull lxc106 v1.0.0alpha

# Show version
echo ""
echo "📌 Version: $(cat VERSION)"
echo ""

# Stop current container
echo "🛑 Stopping current container..."
docker-compose down

# Rebuild image
echo "🔨 Building new image..."
docker-compose build

# Start container
echo "🚀 Starting Flasharr..."
docker-compose up -d

# Wait for container to start
echo "⏳ Waiting for container to start..."
sleep 5

# Show logs
echo ""
echo "📋 Container logs:"
docker-compose logs --tail=30 flasharr

echo ""
echo "✅ Deployment complete!"
echo ""
echo "🌐 Access Flasharr at: http://localhost:8484"
echo "📚 Documentation: /etc/pve/fshare-arr-bridge/flasharr_docs/"
echo ""
