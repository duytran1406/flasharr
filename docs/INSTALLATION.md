# 🚀 Flasharr Installation Guide

Flasharr is a multi-host download manager with \*arr integration.

## Quick Start

### Option 1: One-Line Install (Recommended)

```bash
curl -sSL https://raw.githubusercontent.com/duytran1406/flasharr/main/install.sh | bash
```

### Option 2: Docker Compose

1. **Download the docker-compose file:**

   ```bash
   mkdir flasharr && cd flasharr
   curl -O https://raw.githubusercontent.com/duytran1406/flasharr/main/docker-compose.production.yml
   mv docker-compose.production.yml docker-compose.yml
   ```

2. **Start Flasharr:**

   ```bash
   docker compose up -d
   ```

3. **Access the web UI:**
   Open `http://localhost:8484` in your browser

### Option 3: Docker Run

```bash
docker run -d \
  --name flasharr \
  -p 8484:8484 \
  -v ./appData:/appData \
  -e TZ=Asia/Bangkok \
  --restart unless-stopped \
  ghcr.io/duytran1406/flasharr:latest
```

## Configuration

### Environment Variables

| Variable               | Default         | Description                      |
| ---------------------- | --------------- | -------------------------------- |
| `TZ`                   | `UTC`           | Timezone for logs and scheduling |
| `RUST_LOG`             | `flasharr=info` | Logging level                    |
| `FLASHARR_APPDATA_DIR` | `/appData`      | Data directory (don't change)    |

### Volume Mounts

| Host Path            | Container Path       | Description                         |
| -------------------- | -------------------- | ----------------------------------- |
| `./appData`          | `/appData`           | Configuration, database, and logs   |
| `/path/to/downloads` | `/appData/downloads` | (Optional) Custom download location |

## Updating

### Docker Compose

```bash
docker compose pull
docker compose up -d
```

### Docker Run

```bash
docker stop flasharr
docker rm flasharr
docker pull ghcr.io/duytran1406/flasharr:latest
# Run the docker run command again
```

## Advanced Configuration

### Using ZFS Storage (Recommended for Production)

If you're using ZFS, mount your datasets and update the docker-compose.yml:

```yaml
volumes:
  - /mnt/appdata/flasharr:/appData
  - /data/flasharr-download:/appData/downloads
```

### Behind a Reverse Proxy

Example Nginx configuration:

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

## Troubleshooting

### View Logs

```bash
docker compose logs -f flasharr
```

### Permission Issues

Ensure the appData directory has correct permissions:

```bash
chown -R 1000:1000 ./appData
```

### Reset Configuration

```bash
docker compose down
rm -rf appData
docker compose up -d
```

## Support

- **Documentation**: [https://github.com/duytran1406/flasharr/wiki](https://github.com/duytran1406/flasharr/wiki)
- **Issues**: [https://github.com/duytran1406/flasharr/issues](https://github.com/duytran1406/flasharr/issues)
- **Discord**: [Join our community](https://discord.gg/flasharr-community)
