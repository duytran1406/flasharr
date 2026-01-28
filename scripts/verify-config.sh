#!/bin/bash
# Verification script - checks if everything is configured correctly

set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}╔═══════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   Flasharr Configuration Checker     ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════╝${NC}"
echo ""

ERRORS=0
WARNINGS=0

# Check GitHub username
echo -n "Checking GitHub configuration... "
if grep -r "yourusername" . --exclude-dir={node_modules,.git,target,.svelte-kit} >/dev/null 2>&1; then
    echo -e "${RED}✗${NC}"
    echo -e "${RED}  Found unreplaced 'yourusername' placeholders${NC}"
    ERRORS=$((ERRORS + 1))
else
    echo -e "${GREEN}✓${NC}"
fi

# Check for duytran1406
echo -n "Checking repository name... "
if grep -q "duytran1406/flasharr" README.md; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${RED}✗${NC}"
    echo -e "${RED}  Repository name not found in README.md${NC}"
    ERRORS=$((ERRORS + 1))
fi

# Check Docker image names
echo -n "Checking Docker image configuration... "
if grep -q "ghcr.io/duytran1406/flasharr" docker-compose.production.yml; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${RED}✗${NC}"
    echo -e "${RED}  Docker image name not configured${NC}"
    ERRORS=$((ERRORS + 1))
fi

# Check install script
echo -n "Checking install script... "
if grep -q "duytran1406/flasharr" install.sh; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${RED}✗${NC}"
    echo -e "${RED}  Install script not configured${NC}"
    ERRORS=$((ERRORS + 1))
fi

# Check GitHub Actions
echo -n "Checking GitHub Actions workflow... "
if [ -f "../.github/workflows/docker-publish.yml" ]; then
    if grep -q "duytran1406" ../.github/workflows/docker-publish.yml; then
        echo -e "${GREEN}✓${NC}"
    else
        echo -e "${YELLOW}⚠${NC}"
        echo -e "${YELLOW}  GitHub Actions may need manual review${NC}"
        WARNINGS=$((WARNINGS + 1))
    fi
else
    echo -e "${RED}✗${NC}"
    echo -e "${RED}  GitHub Actions workflow not found${NC}"
    ERRORS=$((ERRORS + 1))
fi

# Check Dockerfile
echo -n "Checking Dockerfile... "
if grep -q "duytran1406" Dockerfile; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${YELLOW}⚠${NC}"
    echo -e "${YELLOW}  Dockerfile labels may need review${NC}"
    WARNINGS=$((WARNINGS + 1))
fi

# Check version files
echo -n "Checking version consistency... "
BACKEND_VERSION=$(grep '^version = ' backend/Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
FRONTEND_VERSION=$(grep '"version":' frontend/package.json | head -1 | sed 's/.*"version": "\(.*\)".*/\1/')

if [ "$BACKEND_VERSION" = "$FRONTEND_VERSION" ]; then
    echo -e "${GREEN}✓${NC} (v$BACKEND_VERSION)"
else
    echo -e "${RED}✗${NC}"
    echo -e "${RED}  Version mismatch: backend=$BACKEND_VERSION, frontend=$FRONTEND_VERSION${NC}"
    ERRORS=$((ERRORS + 1))
fi

# Check for required scripts
echo -n "Checking release scripts... "
if [ -x "scripts/release.sh" ] && [ -x "scripts/check-version.sh" ]; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${RED}✗${NC}"
    echo -e "${RED}  Release scripts not found or not executable${NC}"
    ERRORS=$((ERRORS + 1))
fi

# Check documentation
echo -n "Checking documentation... "
REQUIRED_DOCS=("docs/INSTALLATION.md" "docs/CONFIGURATION.md" "docs/TROUBLESHOOTING.md" "docs/API.md")
MISSING_DOCS=()

for doc in "${REQUIRED_DOCS[@]}"; do
    if [ ! -f "$doc" ]; then
        MISSING_DOCS+=("$doc")
    fi
done

if [ ${#MISSING_DOCS[@]} -eq 0 ]; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${YELLOW}⚠${NC}"
    echo -e "${YELLOW}  Missing documentation: ${MISSING_DOCS[*]}${NC}"
    WARNINGS=$((WARNINGS + 1))
fi

# Summary
echo ""
echo -e "${BLUE}═══════════════════════════════════════${NC}"
if [ $ERRORS -eq 0 ] && [ $WARNINGS -eq 0 ]; then
    echo -e "${GREEN}✅ All checks passed!${NC}"
    echo ""
    echo -e "${GREEN}Your Flasharr project is ready to publish!${NC}"
    echo ""
    echo -e "Next steps:"
    echo -e "  1. ${BLUE}git push origin main${NC}"
    echo -e "  2. ${BLUE}./scripts/release.sh${NC}"
    echo -e "  3. Enable GitHub Packages"
    echo ""
elif [ $ERRORS -eq 0 ]; then
    echo -e "${YELLOW}⚠ Configuration complete with $WARNINGS warning(s)${NC}"
    echo ""
    echo -e "You can proceed, but review the warnings above."
else
    echo -e "${RED}❌ Found $ERRORS error(s) and $WARNINGS warning(s)${NC}"
    echo ""
    echo -e "Please fix the errors above before publishing."
    exit 1
fi
