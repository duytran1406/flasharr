#!/bin/bash
# Release helper script
# Creates a new release with proper versioning

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔═══════════════════════════════════════╗${NC}"
echo -e "${BLUE}║     Flasharr Release Helper          ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════╝${NC}"
echo ""

# Check if on main branch
CURRENT_BRANCH=$(git branch --show-current)
if [ "$CURRENT_BRANCH" != "main" ]; then
    echo -e "${RED}❌ You must be on the main branch to create a release${NC}"
    echo -e "${YELLOW}Current branch: $CURRENT_BRANCH${NC}"
    exit 1
fi

# Check for uncommitted changes
if ! git diff-index --quiet HEAD --; then
    echo -e "${RED}❌ You have uncommitted changes${NC}"
    echo -e "${YELLOW}Please commit or stash your changes first${NC}"
    exit 1
fi

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep '^version = ' backend/Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
echo -e "${BLUE}Current version: ${YELLOW}v${CURRENT_VERSION}${NC}"
echo ""

# Ask for new version
echo -e "${YELLOW}Enter new version (without 'v' prefix):${NC}"
read -p "> " NEW_VERSION

if [ -z "$NEW_VERSION" ]; then
    echo -e "${RED}❌ Version cannot be empty${NC}"
    exit 1
fi

# Validate version format (basic semver check)
if ! [[ $NEW_VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
    echo -e "${RED}❌ Invalid version format${NC}"
    echo -e "${YELLOW}Expected format: X.Y.Z or X.Y.Z-beta.1${NC}"
    exit 1
fi

echo ""
echo -e "${YELLOW}📝 Release Summary:${NC}"
echo -e "   Old version: ${RED}v${CURRENT_VERSION}${NC}"
echo -e "   New version: ${GREEN}v${NEW_VERSION}${NC}"
echo ""
read -p "Continue? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${YELLOW}Cancelled${NC}"
    exit 1
fi

echo ""
echo -e "${BLUE}🔄 Updating version numbers...${NC}"

# Update backend version
sed -i.bak "s/^version = \".*\"/version = \"$NEW_VERSION\"/" backend/Cargo.toml
rm backend/Cargo.toml.bak

# Update frontend version
sed -i.bak "s/\"version\": \".*\"/\"version\": \"$NEW_VERSION\"/" frontend/package.json
rm frontend/package.json.bak

echo -e "${GREEN}✓${NC} Updated backend/Cargo.toml"
echo -e "${GREEN}✓${NC} Updated frontend/package.json"

# Update CHANGELOG.md
TODAY=$(date +%Y-%m-%d)
echo ""
echo -e "${BLUE}📝 Updating CHANGELOG.md...${NC}"

# Create a temporary file with the new entry
cat > /tmp/changelog_entry << EOF
## [${NEW_VERSION}] - ${TODAY}

### Added
- 

### Changed
- 

### Fixed
- 

EOF

# Insert after the [Unreleased] section
sed -i.bak "/## \[Unreleased\]/r /tmp/changelog_entry" CHANGELOG.md
rm CHANGELOG.md.bak

echo -e "${GREEN}✓${NC} Updated CHANGELOG.md"
echo -e "${YELLOW}⚠️  Please edit CHANGELOG.md to add release notes${NC}"
echo ""

# Open CHANGELOG in editor
if command -v code &> /dev/null; then
    code CHANGELOG.md
elif command -v vim &> /dev/null; then
    vim CHANGELOG.md
elif command -v nano &> /dev/null; then
    nano CHANGELOG.md
else
    echo -e "${YELLOW}Please manually edit CHANGELOG.md${NC}"
fi

echo ""
read -p "Press Enter after editing CHANGELOG.md..."

# Commit changes
echo ""
echo -e "${BLUE}📦 Committing changes...${NC}"
git add backend/Cargo.toml frontend/package.json CHANGELOG.md
git commit -m "Release v${NEW_VERSION}"
echo -e "${GREEN}✓${NC} Committed version bump"

# Create tag
echo -e "${BLUE}🏷️  Creating git tag...${NC}"
git tag -a "v${NEW_VERSION}" -m "Release v${NEW_VERSION}"
echo -e "${GREEN}✓${NC} Created tag v${NEW_VERSION}"

echo ""
echo -e "${GREEN}✅ Release prepared successfully!${NC}"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo -e "  1. Review the changes: ${BLUE}git show${NC}"
echo -e "  2. Push to GitHub: ${BLUE}git push origin main --tags${NC}"
echo -e "  3. GitHub Actions will automatically build and publish Docker images"
echo ""
echo -e "${YELLOW}Docker images that will be created:${NC}"
echo -e "  - ghcr.io/duytran1406/flasharr:v${NEW_VERSION}"
echo -e "  - ghcr.io/duytran1406/flasharr:v$(echo $NEW_VERSION | cut -d. -f1-2)"
echo -e "  - ghcr.io/duytran1406/flasharr:v$(echo $NEW_VERSION | cut -d. -f1)"
echo -e "  - ghcr.io/duytran1406/flasharr:stable"
echo -e "  - ghcr.io/duytran1406/flasharr:latest"
echo ""

read -p "Push now? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${BLUE}🚀 Pushing to GitHub...${NC}"
    git push origin main --tags
    echo ""
    echo -e "${GREEN}✅ Release published!${NC}"
    echo -e "${BLUE}View release: https://github.com/duytran1406/flasharr/releases/tag/v${NEW_VERSION}${NC}"
else
    echo -e "${YELLOW}Remember to push manually:${NC}"
    echo -e "  ${BLUE}git push origin main --tags${NC}"
fi
