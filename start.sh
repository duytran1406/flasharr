#!/bin/bash
# Fshare-Arr Bridge Startup Script

set -e

echo "========================================="
echo "Fshare-Arr Bridge"
echo "========================================="

# Check if .env exists
if [ ! -f .env ]; then
    echo "⚠️  No .env file found!"
    echo "Creating from .env.example..."
    cp .env.example .env
    echo ""
    echo "❗ Please edit .env with your credentials:"
    echo "   - FSHARE_EMAIL"
    echo "   - FSHARE_PASSWORD"
    echo "   - PYLOAD_HOST"
    echo "   - PYLOAD_PASSWORD"
    echo ""
    echo "Then run this script again."
    exit 1
fi

# Load environment variables
source .env

# Check required variables
if [ -z "$FSHARE_EMAIL" ] || [ -z "$FSHARE_PASSWORD" ]; then
    echo "❌ Error: FSHARE_EMAIL and FSHARE_PASSWORD must be set in .env"
    exit 1
fi

if [ -z "$PYLOAD_PASSWORD" ]; then
    echo "❌ Error: PYLOAD_PASSWORD must be set in .env"
    exit 1
fi

echo "✅ Configuration loaded"
echo ""

# Check if running in Docker
if [ -f /.dockerenv ]; then
    echo "🐳 Running in Docker container"
    exec gunicorn --bind 0.0.0.0:${INDEXER_PORT:-8484} --workers 2 --timeout 120 "app.main:create_app()"
else
    echo "💻 Running locally"
    
    # Check if virtual environment exists
    if [ ! -d "venv" ]; then
        echo "Creating virtual environment..."
        python3 -m venv venv
    fi
    
    # Activate virtual environment
    source venv/bin/activate
    
    # Install dependencies
    echo "Installing dependencies..."
    pip install -q -r requirements.txt
    
    echo ""
    echo "========================================="
    echo "Starting Fshare-Arr Bridge"
    echo "========================================="
    echo "Indexer API: http://localhost:${INDEXER_PORT:-8484}/indexer/api"
    echo "SABnzbd API: http://localhost:${INDEXER_PORT:-8484}/sabnzbd/api"
    echo "Health Check: http://localhost:${INDEXER_PORT:-8484}/health"
    echo "========================================="
    echo ""
    
    # Run application
    python -m app.main
fi
