---
description: Deploy local code to remote LXC 112 for dev check
---

# Remote Deployment (Dev Check)

This workflow deploys your current local code to the remote LXC 112 via `pve-remote`. Use this to verify your changes in a live-like environment.

## Steps

1. **Deploy to Remote**
   Run the deployment script which handles version bumping, packaging, uploading, and restarting the container.

   // turbo

   ```bash
   ./deploy-remote.sh
   ```

2. **Verify Deployment**
   Check the deployment output for the success message and version number.
   Access the application at: [https://fshare.blavkbeav.com/](https://fshare.blavkbeav.com/)
