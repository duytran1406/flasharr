# 🔍 GitHub Actions Build Troubleshooting

## Current Status

All GitHub Actions builds are failing. Let me help you debug this.

## Quick Check

1. **View latest build logs**:

   ```bash
   # Go to: https://github.com/duytran1406/flasharr/actions
   # Click on the latest failed run
   # Check the "Build and push Docker image" step
   ```

2. **Common Issues**:
   - Missing files in build context
   - Dockerfile syntax errors
   - Build dependencies not available
   - Permission issues with GHCR

## What I've Done

✅ Removed Docker Hub workflow (you only need GHCR)
✅ Verified Dockerfile exists
✅ Checked workflow configuration

## Next Steps

Please check the GitHub Actions logs and share the error message. The most common issues are:

### Issue 1: Build Context

The workflow might not find required files. Check if `.dockerignore` is excluding too much.

### Issue 2: Rust/Node Dependencies

The build might fail during `cargo build` or `npm install`.

### Issue 3: GHCR Permissions

GitHub might need package permissions enabled.

## How to Fix

Once you share the error from the Actions log, I can:

1. Fix the Dockerfile
2. Update the workflow
3. Adjust build arguments
4. Fix any missing dependencies

**Please share the error message from the latest failed build.**
