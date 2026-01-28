# Configuration Guide

## Environment Variables

### Required

None - Flasharr works out of the box with sensible defaults.

### Optional

| Variable               | Default         | Description                                      |
| ---------------------- | --------------- | ------------------------------------------------ |
| `TZ`                   | `UTC`           | Timezone for logs and scheduling                 |
| `RUST_LOG`             | `flasharr=info` | Logging level (`debug`, `info`, `warn`, `error`) |
| `FLASHARR_APPDATA_DIR` | `/appData`      | Data directory path (internal use)               |

## Volume Mounts

### Required Volumes

| Host Path   | Container Path | Description                                  |
| ----------- | -------------- | -------------------------------------------- |
| `./appData` | `/appData`     | Configuration, database, logs, and downloads |

### Optional Volumes

| Host Path           | Container Path       | Description              |
| ------------------- | -------------------- | ------------------------ |
| `/custom/downloads` | `/appData/downloads` | Custom download location |

## Port Configuration

| Port   | Protocol | Description    |
| ------ | -------- | -------------- |
| `8484` | HTTP     | Web UI and API |

## First-Time Setup

1. **Access the Setup Wizard**: Navigate to `http://localhost:8484/setup`

2. **Configure FShare Credentials** (if using FShare):
   - Enter your FShare email and password
   - Credentials are stored securely in the database

3. **Set Download Directory**:
   - Default: `/appData/downloads`
   - For custom location, mount a volume to `/appData/downloads`

4. **Configure Indexer** (optional):
   - Generate an API key for Sonarr/Radarr integration
   - The indexer will be available at `http://localhost:8484/api/indexer`

5. **Connect Arr Services** (optional):
   - Add Sonarr/Radarr URLs and API keys
   - Enable cloud integration for automatic downloads

## Advanced Configuration

### Using Custom Download Paths

Edit your `docker-compose.yml`:

```yaml
volumes:
  - ./appData:/appData
  - /mnt/media/downloads:/appData/downloads # Custom path
```

### Behind a Reverse Proxy

#### Nginx

```nginx
location /flasharr {
    proxy_pass http://localhost:8484;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;

    # WebSocket support
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
}
```

#### Traefik

```yaml
labels:
  - "traefik.enable=true"
  - "traefik.http.routers.flasharr.rule=Host(`flasharr.yourdomain.com`)"
  - "traefik.http.routers.flasharr.entrypoints=websecure"
  - "traefik.http.routers.flasharr.tls.certresolver=letsencrypt"
  - "traefik.http.services.flasharr.loadbalancer.server.port=8484"
```

### Resource Limits

Add resource limits to `docker-compose.yml`:

```yaml
deploy:
  resources:
    limits:
      cpus: "2"
      memory: 1G
    reservations:
      cpus: "0.5"
      memory: 256M
```

### Logging Configuration

Adjust log levels via environment variable:

```yaml
environment:
  - RUST_LOG=flasharr=debug,tower_http=debug # Verbose logging
  - RUST_LOG=flasharr=warn # Minimal logging
```

## Database Management

### Backup

```bash
docker compose exec flasharr cp /appData/data/flasharr.db /appData/data/flasharr.db.backup
```

### Restore

```bash
docker compose down
cp appData/data/flasharr.db.backup appData/data/flasharr.db
docker compose up -d
```

## Security Best Practices

1. **Change Default Ports**: Map to a different host port if needed
2. **Use Reverse Proxy**: Add authentication layer with Nginx/Traefik
3. **Firewall Rules**: Restrict access to trusted networks
4. **Regular Backups**: Backup your appData directory regularly
5. **Keep Updated**: Use Watchtower or manual updates regularly

## Performance Tuning

### For High-Volume Downloads

```yaml
environment:
  - RUST_LOG=flasharr=info # Reduce logging overhead
```

### For Low-Resource Systems

```yaml
deploy:
  resources:
    limits:
      memory: 512M
```

## Troubleshooting Configuration Issues

See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for common configuration problems and solutions.
