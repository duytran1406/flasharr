#!/bin/bash
set -e

# Files to sync
FILES=(
    "src/fshare_bridge/templates/settings.html"
    "src/fshare_bridge/app.py"
    "src/fshare_bridge/static/js/app.js"
    "src/fshare_bridge/templates/base.html"
    "src/fshare_bridge/templates/downloads.html"
    "src/fshare_bridge/templates/index.html"
    "src/fshare_bridge/factory.py"
    "src/fshare_bridge/clients/fshare.py"
    "src/fshare_bridge/core/account_manager.py"
    "src/fshare_bridge/templates/search.html"
    "src/fshare_bridge/static/css/style.css"
    "src/fshare_bridge/services/sabnzbd.py"
    "src/fshare_bridge/downloader/builtin_client.py"
    "VERSION"
)

# Base dir on containers
BASE_DIR="/root/fshare-arr-bridge"

echo "Syncing to LXC 106..."
for file in "${FILES[@]}"; do
    pct push 106 "/etc/pve/fshare-arr-bridge/$file" "$BASE_DIR/$file"
done

echo "Committing on LXC 106..."
pct exec 106 -- bash -c "cd $BASE_DIR && git add . && git commit -m 'Fix JS errors, slider length, and toggle alignment' || echo 'Nothing to commit'"

echo "Syncing to LXC 112..."
for file in "${FILES[@]}"; do
    pct push 112 "/etc/pve/fshare-arr-bridge/$file" "$BASE_DIR/$file"
done

echo "Rebuilding on LXC 112..."
# Navigate to dir and rebuild
pct exec 112 -- bash -c "cd $BASE_DIR && docker compose up -d --build fshare-arr-bridge"

echo "Done!"
