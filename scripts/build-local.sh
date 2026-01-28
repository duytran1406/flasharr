#!/bin/bash
# Local test build script - for testing only, not for publishing
# This builds the image locally to verify it works before pushing to GitHub

set -e

echo "🔨 Building Flasharr Docker image locally..."
echo "⚠️  This is for testing only. Production images are built by GitHub Actions."
echo ""

# Build the image
docker build -t flasharr:local -f Dockerfile .

echo ""
echo "✅ Build complete!"
echo ""
echo "Test the image with:"
echo "  docker run -p 8484:8484 -v ./appData:/appData flasharr:local"
echo ""
echo "When ready to publish:"
echo "  1. Commit your changes"
echo "  2. Create a git tag: git tag v2.0.0"
echo "  3. Push with tags: git push origin main --tags"
echo "  4. GitHub Actions will build and publish automatically"
