#!/bin/bash
# Flasharr Unified Deployment Script
# Handles both Release Management (Host) and Container Deployment (Target)

set -e

# --- Configuration ---
TARGET_LXC_ID="112"
TARGET_DIR="/root/fshare-arr-bridge"
REPO_REMOTE="lxc106"
REPO_BRANCH="v1.0.0alpha"
REMOTE_PASS="123456"

# --- Mode Detection ---
if command -v pct &> /dev/null; then
    MODE="MASTER"
else
    MODE="WORKER"
fi

# Override mode if flag provided
if [ "$1" == "--worker" ]; then
    MODE="WORKER"
fi

# ==========================================
# MASTER MODE: Version Bump, Push, Trigger
# ==========================================
if [ "$MODE" == "MASTER" ]; then
    echo "=== 🚀 Flasharr Release Manager (Host) ==="
    
    # 1. Auto-Versioning
    if [ ! -f "VERSION" ]; then echo "0.1.0" > VERSION; fi
    CURRENT_VER=$(cat VERSION)
    
    # Python script to increment patch version
    NEW_VER=$(python3 -c "import re; m = re.match(r'(\d+)\.(\d+)\.(\d+)(.*)', '$CURRENT_VER'); print(f'{m.group(1)}.{m.group(2)}.{int(m.group(3))+1}{m.group(4)}') if m else print('$CURRENT_VER')")
    
    if [ "$CURRENT_VER" == "$NEW_VER" ]; then
        echo "⚠️  Could not increment version automatically. Check VERSION file format."
    else
        echo "📦 Version Bump: $CURRENT_VER -> $NEW_VER"
        echo $NEW_VER > VERSION
    fi

    # 2. Commit & Push
    echo "💾 Committing changes..."
    if [[ -n $(git status -s) ]] && [ "$SKIP_GIT" != "true" ]; then
        git add .
        git commit -m "Release v$NEW_VER"
        
        echo "☁️  Pushing to Central Repo ($REPO_REMOTE)..."
        export GIT_SSH_COMMAND="ssh -o StrictHostKeyChecking=no"
        if command -v sshpass &> /dev/null; then
            sshpass -p "$REMOTE_PASS" git push $REPO_REMOTE $REPO_BRANCH || echo "⚠️  Git push failed (auth error?)"
        else
             # Fallback if sshpass missing (but user asked for auto-versioning via this script)
            echo "⚠️  sshpass not found. Attempting standard git push..."
            git push $REPO_REMOTE $REPO_BRANCH || echo "⚠️  Git push failed"
        fi
    else
        echo "⏭️  No changes to commit."
    fi

    # 3. Deploy to Target
    echo "🚚 Deploying to Target (LXC $TARGET_LXC_ID)..."
    
    # Create transfer archive
    tar --exclude='.git' --exclude='__pycache__' --exclude='*.pyc' -czf /tmp/flasharr_deploy.tar.gz .
    
    # Push and Trigger
    pct exec $TARGET_LXC_ID -- mkdir -p $TARGET_DIR
    pct push $TARGET_LXC_ID /tmp/flasharr_deploy.tar.gz $TARGET_DIR/update.tar.gz
    
    echo "⚡ Triggering internal deployment..."
    # We pass --worker flag explicitly to ensure correct mode on target
    pct exec $TARGET_LXC_ID -- bash -c "cd $TARGET_DIR && tar xzf update.tar.gz && rm update.tar.gz && export SKIP_PULL=true && bash deploy.sh --worker"
    
    echo "✅ Release & Deployment Complete!"
    exit 0
fi

# ==========================================
# WORKER MODE: Docker Operations
# ==========================================
if [ "$MODE" == "WORKER" ]; then
    VERSION=$(cat VERSION 2>/dev/null || echo "Unknown")
    echo "=== Flasharr Worker Deployment (v$VERSION) ==="
    
    SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
    cd "$SCRIPT_DIR"

    # Pull if not skipped (Standard mode when run manually on container)
    if [ "$SKIP_PULL" != "true" ]; then
        echo "📥 Pulling latest code..."
        git fetch lxc106 || echo "⚠️  Git fetch failed"
        # We try to pull, but if conflict or detached head, we continue
        git pull lxc106 v1.0.0alpha || echo "⚠️  Git pull failed or skipped"
    else
        echo "⏭️  Skipping git pull (Push Deployment)"
    fi

    echo "🛑 Restarting Container..."
    docker compose down || true
    docker rm -f flasharr || true
    
    echo "🔨 Rebuilding..."
    docker compose build

    echo "🚀 Starting..."
    docker compose up -d
    
    echo "⏳ Waiting for health check..."
    sleep 5
    
    echo "✅ Deployment successful!"
    echo "🌐 Access: http://localhost:8484"
fi
