#!/bin/bash
set -e

# Configuration
REPO_REMOTE="lxc106"
REPO_BRANCH="v1.0.0alpha"
TARGET_LXC_ID="112"
REMOTE_PASS="123456"
TARGET_DIR="/root/fshare-arr-bridge"

echo "=== 🚀 Flasharr Release & Deploy Pipeline ==="

# 1. Calculate New Version
CURRENT_VER=$(cat VERSION)
# Python one-liner to bump patch version
NEW_VER=$(python3 -c "import re; m = re.match(r'(\d+)\.(\d+)\.(\d+)(.*)', '$CURRENT_VER'); print(f'{m.group(1)}.{m.group(2)}.{int(m.group(3))+1}{m.group(4)}') if m else print('$CURRENT_VER')")

echo "📦 Version Bump: $CURRENT_VER -> $NEW_VER"
echo $NEW_VER > VERSION
# Also update deploy.sh version display
sed -i "s/Version: .*/Version: $NEW_VER/" deploy.sh
sed -i "s/Flasharr v.* Deployment/Flasharr v$NEW_VER Deployment/" deploy.sh

# 2. Commit and Push to Central Repo (LXC 106)
echo "💾 Committing changes..."
git add .
git commit -m "Release v$NEW_VER"

echo "☁️  Pushing to Central Repo ($REPO_REMOTE)..."
# Use sshpass for password auth if keys aren't set up
if command -v sshpass &> /dev/null; then
    sshpass -p "$REMOTE_PASS" git push $REPO_REMOTE $REPO_BRANCH || echo "⚠️  Git push failed (check auth/remote)"
else
    git push $REPO_REMOTE $REPO_BRANCH || echo "⚠️  Git push failed (install sshpass for auto-auth)"
fi

# 3. Deploy to Target (LXC 112)
echo "🚚 Deploying to Target (LXC $TARGET_LXC_ID)..."

# Create transfer archive (exclude heavy/unnecessary files)
tar --exclude='.git' --exclude='__pycache__' --exclude='*.pyc' -czf /tmp/flasharr_deploy.tar.gz .

# Push archive to container
pct push $TARGET_LXC_ID /tmp/flasharr_deploy.tar.gz $TARGET_DIR/update.tar.gz

# Execute deployment inside container
# 1. Go to dir, 2. Extract, 3. Run deploy.sh with SKIP_PULL
echo "⚡ Triggering internal deployment..."
pct exec $TARGET_LXC_ID -- bash -c "cd $TARGET_DIR && tar xzf update.tar.gz && rm update.tar.gz && export SKIP_PULL=true && bash deploy.sh"

echo "✅ Release & Deployment Complete!"
