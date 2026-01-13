#!/bin/bash
# Setup script for Flasharr with pyLoad and Fshare plugins

set -e

echo "========================================="
echo "Flasharr Setup"
echo "========================================="
echo ""

# Create directory structure
echo "📁 Creating directory structure..."
mkdir -p pyload/config
mkdir -p pyload/downloads
mkdir -p pyload/plugins/hooks
mkdir -p pyload/plugins/accounts
mkdir -p pyload/plugins/crypter

# Download Fshare plugins for pyLoad
echo ""
echo "📥 Downloading Fshare plugins for pyLoad..."

# FshareVn plugin
echo "  - Downloading FshareVn.py..."
curl -sL "https://raw.githubusercontent.com/pyload/pyload/main/src/pyload/plugins/downloaders/FshareVn.py" \
  -o pyload/plugins/FshareVn.py 2>/dev/null || \
echo "⚠️  Could not download FshareVn.py - you may need to add it manually"

# FshareVnFolder plugin  
echo "  - Downloading FshareVnFolder.py..."
curl -sL "https://raw.githubusercontent.com/pyload/pyload/main/src/pyload/plugins/crypter/FshareVnFolder.py" \
  -o pyload/plugins/crypter/FshareVnFolder.py 2>/dev/null || \
echo "⚠️  Could not download FshareVnFolder.py - you may need to add it manually"

# Create pyLoad config template
echo ""
echo "📝 Creating pyLoad configuration..."
cat > pyload/config/pyload.conf << 'EOF'
# pyLoad Configuration
# This will be auto-generated on first run

[webui]
port = 8000
host = 0.0.0.0

[download]
max_downloads = 3
chunks = 1

[fshare]
# Fshare credentials will be configured via web UI
enabled = True
EOF

echo "✅ Directory structure created"
echo ""

# Check if .env exists
if [ ! -f .env ]; then
    echo "⚠️  No .env file found!"
    echo "Creating from .env.example..."
    cp .env.example .env
    echo ""
    echo "❗ Please edit .env with your credentials:"
    echo "   - FSHARE_EMAIL"
    echo "   - FSHARE_PASSWORD"
    echo "   - PYLOAD_PASSWORD"
    echo ""
    echo "Then run: docker-compose up -d"
    exit 0
fi

echo "✅ Configuration loaded"
echo ""

# Build and start services
echo "========================================="
echo "Building Docker images..."
echo "========================================="
docker-compose build

echo ""
echo "========================================="
echo "Starting services..."
echo "========================================="
docker-compose up -d

echo ""
echo "========================================="
echo "✅ Setup Complete!"
echo "========================================="
echo ""
echo "Services:"
echo "  - Flasharr: http://localhost:8484"
echo "  - pyLoad Web UI:     http://localhost:8100"
echo ""
echo "Next steps:"
echo "  1. Configure Fshare credentials in pyLoad:"
echo "     - Go to http://localhost:8100"
echo "     - Login with username: admin, password: (from .env)"
echo "     - Go to Settings → Plugins → FshareVn"
echo "     - Enter your Fshare email and password"
echo ""
echo "  2. Add to Prowlarr:"
echo "     - URL: http://your-server-ip:8484/indexer"
echo "     - Type: Generic Newznab"
echo ""
echo "  3. Add to Sonarr/Radarr:"
echo "     - Type: SABnzbd"
echo "     - Host: your-server-ip"
echo "     - Port: 8484"
echo "     - URL Base: /sabnzbd"
echo ""
echo "========================================="
