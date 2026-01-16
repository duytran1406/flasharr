#!/bin/bash
# Script to fix remote environment and clean up bad task
# Usage: ./fix_and_clean.sh

export TMDB_KEY="8d95150f3391194ca66fef44df497ad6"
export TARGET_ID="cc0df9df-b2c1-4414-b2a2-b5a7d9d926bb"
export LXC_ID=112

echo "=== 🔧 Fixing Remote Environment ==="
# Check if .env exists, if not create it from example or scratch
pct exec $LXC_ID -- bash -c "touch /mnt/appdata/Flasharr/.env"

# Add or Update TMDB_API_KEY
pct exec $LXC_ID -- bash -c "grep -q 'TMDB_API_KEY' /mnt/appdata/Flasharr/.env && sed -i 's/^TMDB_API_KEY=.*/TMDB_API_KEY=$TMDB_KEY/' /mnt/appdata/Flasharr/.env || echo 'TMDB_API_KEY=$TMDB_KEY' >> /mnt/appdata/Flasharr/.env"

echo "✅ TMDB Key injected."

echo "=== 🚀 Redeploying Code Fixes ==="
export SKIP_GIT=true
bash deploy.sh

echo "=== ⏳ Waiting for Server Health ==="
sleep 15
# Simple health check loop
for i in {1..10}; do
    if pct exec $LXC_ID -- curl -s http://localhost:8484/health | grep -q "status"; then
        echo "✅ Server is UP."
        break
    fi
    echo "Waiting for server... ($i/10)"
    sleep 3
done

echo "=== 🗑️ Deleting Task $TARGET_ID ==="
# Try to delete the task using the newly deployed fix
pct exec $LXC_ID -- curl -X DELETE "http://localhost:8484/api/downloads/$TARGET_ID"
echo ""

echo "=== ✨ Fix & Cleanup Complete ==="
