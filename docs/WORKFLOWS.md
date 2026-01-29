# 🚀 Flasharr Deployment Workflows - Complete Guide

This document provides a complete overview of all deployment workflows for Flasharr.

## 📋 Quick Reference

| Workflow         | Purpose             | Command              | Environment     |
| ---------------- | ------------------- | -------------------- | --------------- |
| **dev**          | Local development   | `/deploy-local`      | Mac (localhost) |
| **staging**      | Test on LXC 112     | `/deploy-staging`    | LXC 112         |
| **production**   | Deploy from GHCR    | `/deploy-production` | LXC 112         |
| **publish-ghcr** | Publish to registry | `/publish-ghcr`      | GHCR            |

---

## 🎯 Deployment Workflows

### 1️⃣ Development (`/deploy-local`)

**Purpose**: Local debugging with hot-reload

**Script**: `./scripts/deploy/dev.sh`

**What it does**:

- ✅ Builds backend in debug mode (fast compilation)
- ✅ Runs backend with `cargo run`
- ✅ Starts frontend with Vite hot-reload
- ✅ Detailed logging for debugging

**Access**:

- Frontend: http://localhost:5173
- Backend: http://localhost:8484

**When to use**:

- Daily development work
- Testing new features locally
- Debugging issues
- Quick iterations

---

### 2️⃣ Staging (`/deploy-staging`)

**Purpose**: Test production builds on LXC 112

**Script**: `./scripts/deploy/staging.sh`

**What it does**:

- ✅ Builds production Docker image on Mac (faster than Proxmox)
- ✅ Saves and transfers image to LXC 112
- ✅ Loads image and starts container
- ✅ Verifies deployment with health checks

**Access**:

- Application: http://[LXC-IP]:8484

**When to use**:

- Testing production builds before release
- Verifying Docker configuration
- Integration testing
- Pre-release validation

**Why build on Mac?**

- 3-5x faster than building on Proxmox
- Better Docker layer caching
- Transfer time < build time savings

---

### 3️⃣ Production (`/deploy-production`)

**Purpose**: Deploy official releases from GHCR

**Script**: `./scripts/deploy/production.sh`

**What it does**:

- ✅ Pulls latest image from GHCR
- ✅ Stops old containers gracefully
- ✅ Cleans up old images
- ✅ Starts new container
- ✅ Verifies health checks

**Access**:

- Application: http://[LXC-IP]:8484
- Public: https://fshare.blavkbeav.com

**When to use**:

- Deploying official releases
- Production updates
- Rollbacks to previous versions

**Prerequisites**:

- Image must be published to GHCR first

---

### 4️⃣ Publish to GHCR (`/publish-ghcr`)

**Purpose**: Build and publish Docker images to GitHub Container Registry

**Script**: `./scripts/deploy/publish-ghcr.sh`

**What it does**:

- ✅ Authenticates with GHCR
- ✅ Detects version from git tags
- ✅ Builds Docker image with metadata
- ✅ Pushes with appropriate tags
- ✅ Verifies publication

**Tags created**:

**For releases (vX.Y.Z)**:

- `ghcr.io/duytran1406/flasharr:v2.1.0` (exact)
- `ghcr.io/duytran1406/flasharr:v2.1` (minor)
- `ghcr.io/duytran1406/flasharr:v2` (major)
- `ghcr.io/duytran1406/flasharr:stable`
- `ghcr.io/duytran1406/flasharr:latest`

**For development**:

- `ghcr.io/duytran1406/flasharr:nightly`
- `ghcr.io/duytran1406/flasharr:[commit-hash]`

**When to use**:

- After creating a release with `./scripts/release.sh`
- Publishing nightly builds
- Making images available for production deployment

---

## 🔄 Complete Release Workflow

Here's the typical workflow from development to production:

```
┌─────────────────────────────────────────────────────────────┐
│                    DEVELOPMENT PHASE                        │
└─────────────────────────────────────────────────────────────┘
                            ↓
                   /deploy-local
                   (Local testing)
                            ↓
                  Make changes, commit
                            ↓
┌─────────────────────────────────────────────────────────────┐
│                     STAGING PHASE                           │
└─────────────────────────────────────────────────────────────┘
                            ↓
                   /deploy-staging
              (Test on LXC 112)
                            ↓
                  Verify functionality
                            ↓
┌─────────────────────────────────────────────────────────────┐
│                     RELEASE PHASE                           │
└─────────────────────────────────────────────────────────────┘
                            ↓
                  ./scripts/release.sh
              (Bump version, tag)
                            ↓
                   /publish-ghcr
              (Publish to GHCR)
                            ↓
┌─────────────────────────────────────────────────────────────┐
│                   PRODUCTION PHASE                          │
└─────────────────────────────────────────────────────────────┘
                            ↓
                  /deploy-production
              (Deploy from GHCR)
                            ↓
                  Monitor & verify
```

---

## 📂 File Structure

```
Flasharr/
├── scripts/
│   ├── deploy/
│   │   ├── dev.sh              # Development deployment
│   │   ├── staging.sh          # Staging deployment
│   │   ├── production.sh       # Production deployment
│   │   └── publish-ghcr.sh     # Publish to GHCR
│   └── release.sh              # Create new release
│
├── .agent/workflows/
│   ├── deploy-local.md         # Dev workflow docs
│   ├── deploy-staging.md       # Staging workflow docs
│   ├── deploy-production.md    # Production workflow docs
│   └── publish-ghcr.md         # Publish workflow docs
│
└── docs/
    └── DEPLOYMENT.md           # Complete deployment guide
```

---

## 🎨 Script Features

All deployment scripts include:

- ✅ **Colored output** with emojis for readability
- ✅ **Progress indicators** (e.g., [1/5], [2/5])
- ✅ **Error handling** with clear messages
- ✅ **Status verification** after deployment
- ✅ **Helpful commands** displayed at completion
- ✅ **Detailed logging** to `debug_log/` directory

---

## 🔧 Common Commands

### Start Development

```bash
cd /Users/blavkbeav/Documents/Workspace/Flasharr/Flasharr/scripts/deploy
./dev.sh
```

### Deploy to Staging

```bash
cd /Users/blavkbeav/Documents/Workspace/Flasharr/Flasharr/scripts/deploy
./staging.sh
```

### Create and Publish Release

```bash
# 1. Create release
cd /Users/blavkbeav/Documents/Workspace/Flasharr/Flasharr
./scripts/release.sh

# 2. Publish to GHCR
./scripts/deploy/publish-ghcr.sh

# 3. Deploy to production
./scripts/deploy/production.sh
```

### View Logs

```bash
# Local development
tail -f debug_log/run.log

# Staging/Production
ssh root@pve-remote "pct exec 112 -- docker logs -f flasharr"
```

---

## 📊 Comparison Matrix

| Feature           | Dev    | Staging  | Production | Publish  |
| ----------------- | ------ | -------- | ---------- | -------- |
| **Build Time**    | ~30s   | ~5-10min | ~1-2min    | ~5-10min |
| **Hot Reload**    | ✅ Yes | ❌ No    | ❌ No      | N/A      |
| **Docker**        | ❌ No  | ✅ Yes   | ✅ Yes     | ✅ Yes   |
| **Optimized**     | ❌ No  | ✅ Yes   | ✅ Yes     | ✅ Yes   |
| **Versioning**    | ❌ No  | ✅ Yes   | ✅ Yes     | ✅ Yes   |
| **Health Check**  | ❌ No  | ✅ Yes   | ✅ Yes     | ✅ Yes   |
| **Persistence**   | ❌ No  | ✅ Yes   | ✅ Yes     | N/A      |
| **Public Access** | ❌ No  | ⚠️ LXC   | ✅ Yes     | N/A      |

---

## 🛡️ Best Practices

1. **Always test in dev first** before staging
2. **Always test in staging** before production
3. **Tag releases** with semantic versioning (v1.2.3)
4. **Update CHANGELOG.md** for every release
5. **Monitor logs** after deployment
6. **Keep AppData backed up** before major updates
7. **Use specific version tags** for production rollbacks
8. **Document breaking changes** in release notes

---

## 🔍 Troubleshooting

### Development Issues

```bash
# Check backend logs
tail -f debug_log/run.log

# Restart development environment
pkill -f "flasharr" && pkill -f "vite" && ./scripts/deploy/dev.sh
```

### Staging Issues

```bash
# Check build logs
cat debug_log/staging-docker-build.log

# Check container logs
ssh root@pve-remote "pct exec 112 -- docker logs flasharr"
```

### Production Issues

```bash
# Check if image exists
docker manifest inspect ghcr.io/duytran1406/flasharr:latest

# Rollback to previous version
ssh root@pve-remote "pct exec 112 -- bash -c 'cd /opt/flasharr && sed -i \"s/:latest/:v1.2.3/\" docker-compose.yml && docker compose pull && docker compose up -d'"
```

### Publish Issues

```bash
# Re-authenticate
docker logout ghcr.io
echo $GITHUB_TOKEN | docker login ghcr.io -u USERNAME --password-stdin

# Check build locally
docker build -t test .
```

---

## 📝 Environment Variables

### Development

- `RUST_LOG`: Set in backend code
- No `.env` file needed

### Staging/Production

- `FLASHARR_APPDATA_DIR`: `/appData`
- `RUST_LOG`: `flasharr=info,tower_http=info`
- `TZ`: `Asia/Bangkok`

---

## 🎯 Summary

You now have **4 streamlined deployment workflows**:

1. **`/deploy-local`** - Fast local development with hot-reload
2. **`/deploy-staging`** - Test production builds on LXC 112
3. **`/deploy-production`** - Deploy official releases from GHCR
4. **`/publish-ghcr`** - Publish Docker images to registry

All scripts feature:

- 🎨 Beautiful colored output with icons
- 📊 Clear progress indicators
- ✅ Error handling and verification
- 📋 Helpful status information
- 📝 Detailed logging

**Next Steps**:

- Use `/deploy-local` for daily development
- Use `/deploy-staging` to test before releases
- Use `./scripts/release.sh` to create releases
- Use `/publish-ghcr` to publish to registry
- Use `/deploy-production` for production deployments
