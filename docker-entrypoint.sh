#!/bin/sh
# Flasharr Container Entrypoint
# Fixes volume ownership on each startup, then runs as the flasharr user

# Fix ownership of mounted volumes (they may be owned by root after redeploy)
chown -R flasharr:flasharr /appData 2>/dev/null || true
chown -R flasharr:flasharr /downloads 2>/dev/null || true

# Only chown the top-level media dirs (not recursively — media libraries can be huge)
if [ -d "/data/media" ]; then
    for dir in /data/media/*/; do
        chown flasharr:flasharr "$dir" 2>/dev/null || true
    done
fi

# Drop to flasharr user and exec the application
exec gosu flasharr "$@"
