---
name: DevOps & Deployment
description: Skills for deploying and managing Flasharr infrastructure
---

# DevOps & Deployment Skill

## Overview

This skill provides guidance for deploying and managing Flasharr across local development, staging, and production environments.

## Deployment Environments

### 1. Local Development

```bash
cd /Users/blavkbeav/Documents/Workspace/Flasharr/Flasharr
./scripts/debug/dev.sh   # Hot reload
```

### 2. Local Staging (Production Mode)

```bash
cd backend && cargo build --release
cd frontend && npm run build && npm run preview
```

### 3. Production (pve-remote LXC112)

```bash
# SSH connection
ssh pve-remote
pct enter 112

# Or direct via expect script
./scripts/deploy/auth_keys.expect
```

## Deployment Workflows

### /deploy-local

- Runs `scripts/debug/dev.sh`
- Hot reload enabled
- Debug logging

### /deploy-staging

- Builds release binary
- Production frontend build
- Tests locally before production

### /deploy-production

- Build → Package → Transfer → Deploy
- Target: pve-remote LXC112
- Uses SSH + tar archive

## Production Deployment Steps

```bash
# 1. Build release
cargo build --release --manifest-path backend/Cargo.toml
cd frontend && npm run build

# 2. Create package
tar -czvf deploy-package.tar.gz \
    backend/target/release/flasharr-backend \
    frontend/build \
    .env.production

# 3. Transfer
scp deploy-package.tar.gz pve-remote:/tmp/

# 4. Deploy on LXC
ssh pve-remote "pct push 112 /tmp/deploy-package.tar.gz /opt/flasharr/"
ssh pve-remote "pct exec 112 -- bash -c 'cd /opt/flasharr && tar -xzvf deploy-package.tar.gz && systemctl restart flasharr'"

# 5. Verify
ssh pve-remote "pct exec 112 -- systemctl status flasharr"
```

## Health Checks

```bash
# Local
curl http://localhost:8080/health

# Production
ssh pve-remote "pct exec 112 -- curl -s http://localhost:8080/health"
```

## Logs

| Environment | Location                    |
| ----------- | --------------------------- |
| Local Dev   | `debug_log/run.log`         |
| Production  | `journalctl -u flasharr -f` |

## Rollback Procedure

```bash
ssh pve-remote "pct exec 112 -- bash -c 'cd /opt/flasharr && mv deploy-package.tar.gz deploy-package.new.tar.gz && tar -xzvf deploy-package.backup.tar.gz && systemctl restart flasharr'"
```

## Environment Variables

```bash
DATABASE_URL=sqlite:flasharr.db
FSHARE_EMAIL=<email>
FSHARE_PASSWORD=<password>
TMDB_API_KEY=<key>
RUST_LOG=info   # or debug
```
