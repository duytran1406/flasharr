#!/bin/sh
# Flasharr Container Entrypoint
# Fixes volume ownership on startup, then runs as the configured app user.

set -eu

PUID="${PUID:-911}"
PGID="${PGID:-911}"
UMASK="${UMASK:-002}"
RUN_AS_ROOT="${FLASHARR_RUN_AS_ROOT:-false}"

umask "$UMASK"

if [ "$(id -u)" = "0" ]; then
    if getent group flasharr >/dev/null 2>&1; then
        groupmod -o -g "$PGID" flasharr 2>/dev/null || true
    fi

    if id flasharr >/dev/null 2>&1; then
        usermod -o -u "$PUID" -g "$PGID" flasharr 2>/dev/null || true
    fi

    mkdir -p /appData/config /appData/data /appData/downloads /appData/logs
    chown -R "$PUID:$PGID" /appData 2>/dev/null || true
    chmod -R ug+rwX /appData 2>/dev/null || true

    # Keep shared download aliases writable when they are mounted. This is shallow
    # on purpose: media libraries can be large and should be owned by Sonarr/Radarr.
    for dir in /downloads /data/flasharr-download /appData/downloads; do
        if [ -d "$dir" ]; then
            chown "$PUID:$PGID" "$dir" 2>/dev/null || true
            chmod ug+rwX "$dir" 2>/dev/null || true
        fi
    done

    if [ "$RUN_AS_ROOT" = "true" ]; then
        echo "FLASHARR_RUN_AS_ROOT=true set; starting Flasharr as root"
        exec "$@"
    fi

    exec gosu flasharr "$@"
fi

exec "$@"
