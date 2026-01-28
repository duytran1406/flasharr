# Flasharr Versioning System - Complete Setup

## 📋 Overview

Your Flasharr project now has a complete, professional versioning and release system.

## 🏷️ Available Docker Tags

| Tag       | Updates            | Use Case                   |
| --------- | ------------------ | -------------------------- |
| `stable`  | On releases        | Production (recommended)   |
| `latest`  | On main commits    | Production (bleeding edge) |
| `nightly` | Daily at 2 AM UTC  | Testing                    |
| `develop` | On develop commits | Development                |
| `v2.0.0`  | Never (immutable)  | Pinned production          |
| `v2.0`    | On v2.0.x releases | Auto-patch updates         |
| `v2`      | On v2.x.x releases | Auto-minor updates         |

## 🚀 Release Workflow

### For You (Maintainer):

```bash
# 1. Make your changes and commit
git add .
git commit -m "Add awesome feature"

# 2. Run the release script
./scripts/release.sh

# 3. Enter new version (e.g., 2.1.0)
# Script will:
#   - Update Cargo.toml and package.json
#   - Update CHANGELOG.md
#   - Create git tag
#   - Optionally push to GitHub

# 4. GitHub Actions automatically:
#   - Builds Docker images (AMD64 + ARM64)
#   - Publishes to ghcr.io
#   - Creates GitHub release
```

### For Users:

```bash
# Install
curl -sSL https://raw.githubusercontent.com/duytran1406/flasharr/main/install.sh | bash

# Update
docker compose pull && docker compose up -d
```

## 📁 Files Created

### Documentation

- ✅ `README.md` - Project overview
- ✅ `LICENSE` - MIT License
- ✅ `CONTRIBUTING.md` - Contributor guidelines
- ✅ `CHANGELOG.md` - Version history
- ✅ `SECURITY.md` - Security policy
- ✅ `docs/INSTALLATION.md` - Installation guide
- ✅ `docs/CONFIGURATION.md` - Configuration guide
- ✅ `docs/TROUBLESHOOTING.md` - Troubleshooting guide
- ✅ `docs/API.md` - API documentation
- ✅ `docs/DOCKER_TAGS.md` - Docker tags explained
- ✅ `docs/RELEASE_PROCESS.md` - Release process for maintainers

### Scripts

- ✅ `install.sh` - One-line installer
- ✅ `scripts/release.sh` - Automated release helper
- ✅ `scripts/check-version.sh` - Version checker
- ✅ `scripts/build-local.sh` - Local test builds

### Docker

- ✅ `docker-compose.production.yml` - User-friendly compose file
- ✅ `docker-compose.auto-update.yml` - With Watchtower
- ✅ `Dockerfile` - Updated with version metadata

### CI/CD

- ✅ `.github/workflows/docker-publish.yml` - Automated builds
- ✅ `.github/workflows/docker-hub-publish.yml` - Docker Hub alternative

## 🎯 What Happens When...

### You push to `main` branch:

```
ghcr.io/duytran1406/flasharr:latest   ← Updated
ghcr.io/duytran1406/flasharr:nightly  ← Updated
ghcr.io/duytran1406/flasharr:main-abc1234 ← Created
```

### You create tag `v2.1.3`:

```
ghcr.io/duytran1406/flasharr:v2.1.3   ← Created (immutable)
ghcr.io/duytran1406/flasharr:v2.1     ← Updated
ghcr.io/duytran1406/flasharr:v2       ← Updated
ghcr.io/duytran1406/flasharr:stable   ← Updated
ghcr.io/duytran1406/flasharr:latest   ← Updated
```

### Every night at 2 AM UTC:

```
ghcr.io/duytran1406/flasharr:nightly  ← Rebuilt
```

## 🔧 Before Publishing

### 1. Replace Placeholders

Search and replace in all files:

- `duytran1406` → your GitHub username
- `flasharr-community` → your Discord invite (optional)
- Update email in `SECURITY.md`

### 2. Test Locally

```bash
# Build and test
./scripts/build-local.sh
docker run -p 8484:8484 -v ./appData:/appData flasharr:local

# Test version checking
./scripts/check-version.sh
```

### 3. Prepare Repository

```bash
# Initialize git (if not already)
git init
git add .
git commit -m "Initial commit"

# Create main branch
git branch -M main

# Add remote
git remote add origin https://github.com/duytran1406/flasharr.git
```

### 4. First Release

```bash
# Run release script
./scripts/release.sh
# Enter: 2.0.0

# Push to GitHub
git push -u origin main --tags
```

### 5. Enable GitHub Packages

1. Go to repository Settings
2. Enable GitHub Packages
3. Set package visibility to Public

## 📊 Monitoring

### Check Build Status

- GitHub → Actions tab
- View workflow runs
- Check build logs

### Check Published Images

```bash
docker pull ghcr.io/duytran1406/flasharr:latest
docker inspect ghcr.io/duytran1406/flasharr:latest | grep -A 10 Labels
```

### Monitor Downloads

- GitHub → Insights → Traffic
- GitHub Packages → Package insights

## 🎉 Launch Checklist

- [ ] Replace all placeholders
- [ ] Test local build
- [ ] Push to GitHub
- [ ] Create first release (v2.0.0)
- [ ] Verify Docker images published
- [ ] Test installation with `install.sh`
- [ ] Create screenshots for README
- [ ] Write announcement post
- [ ] Share on:
  - [ ] Reddit (r/selfhosted, r/homelab)
  - [ ] Discord servers
  - [ ] Twitter/X
  - [ ] Hacker News (Show HN)

## 📚 User Documentation

Users will find:

- **Quick Start**: One-line install in README
- **Full Guide**: docs/INSTALLATION.md
- **Configuration**: docs/CONFIGURATION.md
- **Troubleshooting**: docs/TROUBLESHOOTING.md
- **API Docs**: docs/API.md
- **Docker Tags**: docs/DOCKER_TAGS.md

## 🔄 Ongoing Maintenance

### Regular Releases

```bash
./scripts/release.sh
```

### Hotfixes

```bash
git checkout -b hotfix/2.0.1 v2.0.0
# Fix the bug
./scripts/release.sh
# Enter: 2.0.1
```

### Nightly Builds

Automatic - no action needed!

## 🎓 Next Steps

1. **Test everything locally**
2. **Push to GitHub**
3. **Create your first release**
4. **Announce to the world!**

---

**You're ready to publish! 🚀**

All the infrastructure is in place for a professional, maintainable open-source project.
