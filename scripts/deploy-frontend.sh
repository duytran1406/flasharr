#!/bin/bash
# Deploy Frontend to Backend Static Directory
# Builds the frontend and copies it to backend/static for production serving

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
FRONTEND_DIR="$SCRIPT_DIR/../frontend"
BACKEND_STATIC_DIR="$SCRIPT_DIR/../backend/static"

echo "🚀 Flasharr Frontend Deployment"
echo "================================"
echo ""
echo "Frontend: $FRONTEND_DIR"
echo "Backend Static: $BACKEND_STATIC_DIR"
echo ""

# Check if frontend directory exists
if [ ! -d "$FRONTEND_DIR" ]; then
    echo "❌ Frontend directory not found!"
    exit 1
fi

# Build frontend
echo "📦 Building frontend..."
cd "$FRONTEND_DIR"
npm run build

if [ $? -ne 0 ]; then
    echo "❌ Frontend build failed!"
    exit 1
fi

echo "✅ Frontend built successfully"
echo ""

# Clean old static files
echo "🧹 Cleaning old static files..."
rm -rf "$BACKEND_STATIC_DIR"
mkdir -p "$BACKEND_STATIC_DIR"

# Copy new build
echo "📋 Copying build to backend/static..."
cp -r "$FRONTEND_DIR/build/"* "$BACKEND_STATIC_DIR/"

if [ $? -ne 0 ]; then
    echo "❌ Failed to copy files!"
    exit 1
fi

echo "✅ Files copied successfully"
echo ""
echo "✅ Frontend deployed! Changes will be visible at http://localhost:8484"
echo ""
echo "Note: If the backend is running, you may need to hard-refresh (Ctrl+Shift+R / Cmd+Shift+R)"
