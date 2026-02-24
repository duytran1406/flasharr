#!/bin/bash
# Fix staging data: update old batch progress and sync How Dare You to Sonarr
# Run from local machine (connects to pve-remote -> LXC 112 -> flasharr container)

set -e

echo "=== Fixing Staging Data ==="

# 1. Fix old batch progress: set downloaded=size for COMPLETED tasks where downloaded is 0
echo ""
echo "[1/3] Fixing batch progress for completed tasks..."
ssh root@pve-remote 'pct exec 112 -- docker exec flasharr /bin/sh -c "
  # Install sqlite3 temporarily
  apk add --no-cache sqlite >/dev/null 2>&1 || true
  
  echo \"Before fix:\"
  sqlite3 /config/flasharr.db \"SELECT filename, state, size, downloaded FROM downloads WHERE state='COMPLETED' AND (downloaded=0 OR downloaded IS NULL) LIMIT 10\"
  
  # Fix: set downloaded=size for completed tasks with 0/NULL downloaded
  sqlite3 /config/flasharr.db \"UPDATE downloads SET downloaded=size, progress=100.0 WHERE state='COMPLETED' AND (downloaded=0 OR downloaded IS NULL)\"
  
  echo \"\"
  echo \"Fixed rows: \$(sqlite3 /config/flasharr.db \"SELECT changes()\")\"
  
  echo \"\"
  echo \"Batch progress after fix:\"
  sqlite3 /config/flasharr.db \"SELECT batch_name, SUM(downloaded) as dl, SUM(size) as total, ROUND(CAST(SUM(downloaded) AS FLOAT)/CAST(SUM(size) AS FLOAT)*100, 1) as progress FROM downloads WHERE batch_id IS NOT NULL GROUP BY batch_id, batch_name\"
"'

# 2. Create hardlinks for "How Dare You" episodes to Sonarr-Import
echo ""
echo "[2/3] Creating hardlinks for 'How Dare You' episodes..."
ssh root@pve-remote 'pct exec 112 -- docker exec flasharr /bin/sh -c "
  # Create Sonarr-Import directory structure
  mkdir -p \"/downloads/Sonarr-Import/How Dare You/Season 01\"
  
  # Create hardlinks for each episode
  for f in \"/downloads/How Dare You!_/Season 01/\"*.mkv; do
    if [ -f \"\$f\" ]; then
      basename=\$(basename \"\$f\")
      # Clean filename: remove ? from name for Sonarr compatibility
      clean_name=\$(echo \"\$basename\" | sed \"s/!?/!/g\")
      target=\"/downloads/Sonarr-Import/How Dare You/Season 01/\$clean_name\"
      if [ ! -f \"\$target\" ]; then
        ln \"\$f\" \"\$target\" && echo \"  Hardlinked: \$clean_name\" || echo \"  FAILED: \$clean_name\"
      else
        echo \"  Already exists: \$clean_name\"
      fi
    fi
  done
  
  echo \"\"
  echo \"Hardlink verification:\"
  ls -la \"/downloads/Sonarr-Import/How Dare You/Season 01/\"
"'

# 3. Trigger Sonarr rescan for the series
echo ""
echo "[3/3] Triggering Sonarr rescan..."
ssh root@pve-remote 'pct exec 112 -- docker exec flasharr /bin/sh -c "
  apk add --no-cache curl >/dev/null 2>&1 || true
  
  # Get Sonarr connection info from the app
  SONARR_URL=\$(sqlite3 /config/flasharr.db \"SELECT value FROM settings WHERE key='\''sonarr_url'\''\" 2>/dev/null)
  SONARR_KEY=\$(sqlite3 /config/flasharr.db \"SELECT value FROM settings WHERE key='\''sonarr_api_key'\''\" 2>/dev/null)
  
  if [ -z \"\$SONARR_URL\" ] || [ -z \"\$SONARR_KEY\" ]; then
    echo \"Could not find Sonarr connection info in settings\"
    echo \"URL: \$SONARR_URL\"
    echo \"Key: \${SONARR_KEY:0:5}...\"
    exit 1
  fi
  
  # Strip quotes if present
  SONARR_URL=\$(echo \"\$SONARR_URL\" | tr -d '\"')
  SONARR_KEY=\$(echo \"\$SONARR_KEY\" | tr -d '\"')
  
  echo \"Sonarr URL: \$SONARR_URL\"
  
  # Trigger DownloadedEpisodesScan command for series ID 60 (How Dare You)
  echo \"Sending RescanSeries command for series ID 60...\"
  RESULT=\$(curl -s -X POST \"\${SONARR_URL}/api/v3/command\" \
    -H \"X-Api-Key: \${SONARR_KEY}\" \
    -H \"Content-Type: application/json\" \
    -d '{\"name\":\"RescanSeries\",\"seriesId\":60}')
  echo \"Sonarr response: \$RESULT\"
  
  echo \"\"
  echo \"Sending DownloadedEpisodesScan command...\"
  RESULT=\$(curl -s -X POST \"\${SONARR_URL}/api/v3/command\" \
    -H \"X-Api-Key: \${SONARR_KEY}\" \
    -H \"Content-Type: application/json\" \
    -d \"{\\\"name\\\":\\\"DownloadedEpisodesScan\\\",\\\"path\\\":\\\"/downloads/Sonarr-Import/How Dare You/Season 01\\\"}\")
  echo \"Sonarr response: \$RESULT\"
"'

echo ""
echo "=== Done ==="
