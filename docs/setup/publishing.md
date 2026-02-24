# Autonomous Publishing Workflow Setup

This document lists everything needed to set up autonomous publishing for Flasharr.

## 📋 Required Information

### 1. GitHub Repository Details

```yaml
Repository Information:
  - GitHub Username: _____________
  - Repository Name: _____________
  - Repository URL: https://github.com/___________/___________
  - Default Branch: main (or specify: _____________)
```

### 2. Container Registry Choice

**Option A: GitHub Container Registry (GHCR)** ✅ Recommended

- ✅ Free for public repos
- ✅ No additional setup needed
- ✅ Integrated with GitHub
- **Required**: Nothing! Uses GITHUB_TOKEN automatically

**Option B: Docker Hub**

- ✅ Better discoverability
- ✅ More familiar to users
- **Required**:
  ```yaml
  Docker Hub:
    - Username: _____________
    - Access Token: _____________ (create at hub.docker.com/settings/security)
  ```

**Option C: Both** (Recommended for maximum reach)

- Publish to both GHCR and Docker Hub
- **Required**: Docker Hub credentials above

### 3. Project Metadata

```yaml
Project Details:
  - Project Name: Flasharr
  - Short Description: _____________________________________________
  - License: MIT (or specify: _____________)
  - Author/Team Name: _____________
  - Contact Email (for security): _____________
  - Discord Server (optional): _____________
  - Website (optional): _____________
```

### 4. Version Information

```yaml
Initial Release:
  - Starting Version: 2.0.0 (or specify: _____________)
  - Current Development Branch: main
  - Use Nightly Builds: Yes/No
  - Nightly Build Schedule: 2 AM UTC (or specify: _____________)
```

### 5. Build Configuration

```yaml
Build Settings:
  - Target Platforms:
    - [x] linux/amd64 (Intel/AMD)
    - [x] linux/arm64 (ARM/Apple Silicon/Raspberry Pi)
    - [ ] linux/arm/v7 (Older Raspberry Pi)

  - Build Cache: Yes (recommended)
  - Multi-stage Build: Yes (already configured)
```

## 🔐 GitHub Secrets Setup

### For GitHub Container Registry (GHCR)

**No secrets needed!** GITHUB_TOKEN is automatic.

### For Docker Hub

Add these secrets in GitHub:

1. Go to: `https://github.com/YOUR_USERNAME/YOUR_REPO/settings/secrets/actions`
2. Click "New repository secret"
3. Add:
   - Name: `DOCKERHUB_USERNAME`
   - Value: Your Docker Hub username
4. Add:
   - Name: `DOCKERHUB_TOKEN`
   - Value: Your Docker Hub access token

## 📝 Files to Update

Once you provide the information above, update these files:

### 1. Replace Placeholders

**Search for:** `duytran1406`
**Replace with:** Your GitHub username

**Files to update:**

- [ ] `README.md`
- [ ] `.github/workflows/docker-publish.yml`
- [ ] `.github/workflows/docker-hub-publish.yml`
- [ ] `install.sh`
- [ ] `docs/INSTALLATION.md`
- [ ] `docs/CONFIGURATION.md`
- [ ] `docs/TROUBLESHOOTING.md`
- [ ] `docs/API.md`
- [ ] `docs/DOCKER_TAGS.md`
- [ ] `docker-compose.production.yml`
- [ ] `docker-compose.auto-update.yml`
- [ ] `Dockerfile`

### 2. Update Contact Information

**In `SECURITY.md`:**

- Replace `security@flasharr.example.com` with your email

**In `README.md`:**

- Add your Discord invite (if applicable)
- Add your website (if applicable)

### 3. Update Image Names

**If using Docker Hub:**

- Replace `ghcr.io/duytran1406/flasharr` with `duytran1406/flasharr`

## 🚀 Step-by-Step Setup

### Step 1: Create GitHub Repository

```bash
# On GitHub.com:
1. Click "New Repository"
2. Name: flasharr (or your choice)
3. Description: Multi-host download manager with *arr integration
4. Public repository
5. Don't initialize with README (you already have one)
6. Create repository
```

### Step 2: Push Your Code

```bash
# In your local Flasharr directory:
git init
git add .
git commit -m "Initial commit"
git branch -M main
git remote add origin https://github.com/YOUR_USERNAME/flasharr.git
git push -u origin main
```

### Step 3: Enable GitHub Packages

```bash
# On GitHub.com:
1. Go to repository Settings
2. Scroll to "Features"
3. Enable "Packages"
4. Set package visibility to "Public"
```

### Step 4: (Optional) Add Docker Hub Secrets

```bash
# If using Docker Hub:
1. Go to: https://github.com/YOUR_USERNAME/flasharr/settings/secrets/actions
2. Add DOCKERHUB_USERNAME
3. Add DOCKERHUB_TOKEN
```

### Step 5: Create First Release

```bash
# In your local directory:
./scripts/release.sh
# Enter: 2.0.0
# Push when prompted
```

### Step 6: Verify

```bash
# Check GitHub Actions:
https://github.com/YOUR_USERNAME/flasharr/actions

# After build completes, test:
docker pull ghcr.io/YOUR_USERNAME/flasharr:latest
docker run -p 8484:8484 ghcr.io/YOUR_USERNAME/flasharr:latest
```

## 📋 Pre-Launch Checklist

- [ ] GitHub repository created
- [ ] Code pushed to GitHub
- [ ] GitHub Packages enabled
- [ ] Placeholders replaced
- [ ] Contact info updated
- [ ] (Optional) Docker Hub secrets added
- [ ] Local build tested
- [ ] Screenshots added to `docs/images/`
- [ ] CHANGELOG.md updated
- [ ] First release created
- [ ] Docker images verified
- [ ] Installation tested with `install.sh`

## 🎯 Quick Start Template

Fill this out and I'll help you set everything up:

```yaml
# FLASHARR PUBLISHING CONFIGURATION

GitHub:
  username: _______________
  repo_name: flasharr

Container Registry:
  use_ghcr: true
  use_dockerhub: false # Set to true if you want Docker Hub too
  dockerhub_username: _______________ (if using Docker Hub)

Contact:
  security_email: _______________
  discord_invite: _______________ (optional)
  website: _______________ (optional)

Release:
  initial_version: 2.0.0
  enable_nightly: true
  nightly_time: "2 AM UTC"

Build:
  platforms:
    - linux/amd64
    - linux/arm64
```

## 🤖 What Happens Automatically

Once set up, the workflow will automatically:

### On Every Push to Main:

- ✅ Build Docker images
- ✅ Run tests (if configured)
- ✅ Update `latest` tag
- ✅ Update `nightly` tag
- ✅ Create commit-specific tag

### On Every Release Tag (v\*):

- ✅ Build Docker images
- ✅ Create version tags (v2.0.0, v2.0, v2)
- ✅ Update `stable` tag
- ✅ Create GitHub release
- ✅ Generate release notes

### Daily at 2 AM UTC:

- ✅ Rebuild `nightly` from latest main

### On Pull Requests:

- ✅ Build (but don't publish)
- ✅ Run tests
- ✅ Verify Docker build works

## 📞 Need Help?

Provide the filled template above and I'll:

1. Update all files with your information
2. Generate the exact commands to run
3. Create a custom setup guide for your configuration

---

**Ready to publish?** Fill out the template above! 🚀
