<div align="center">

# ⚡ Flasharr

### Multi-Host Download Manager with \*arr Integration

[![Docker Pulls](https://img.shields.io/docker/pulls/duytran1406/flasharr?style=for-the-badge&logo=docker)](https://hub.docker.com/r/duytran1406/flasharr)
[![GitHub Release](https://img.shields.io/github/v/release/duytran1406/flasharr?style=for-the-badge&logo=github)](https://github.com/duytran1406/flasharr/releases)
[![License](https://img.shields.io/github/license/duytran1406/flasharr?style=for-the-badge)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/duytran1406/flasharr?style=for-the-badge&logo=github)](https://github.com/duytran1406/flasharr/stargazers)

**Blazing-fast download manager built with Rust + SvelteKit**  
Seamlessly integrates with Sonarr & Radarr for automated media management

[Quick Start](#-quick-start) • [Features](#-features) • [Documentation](#-documentation) • [Screenshots](#-screenshots)

</div>

---

## 🎬 Demo

<!-- TODO: Add animated GIF of intro animation -->
<!-- ![Flasharr Demo](docs/images/demo.gif) -->

<div align="center">
  <img src="docs/images/dashboard.png" alt="Dashboard" width="45%">
  <img src="docs/images/downloads.png" alt="Downloads" width="45%">
</div>

---

## ✨ Features

<table>
<tr>
<td width="50%">

### 🚀 **Performance**

- ⚡ **300 MB/s** download speeds
- 🪶 **~30 MB** RAM usage (idle)
- 🔥 **Rust-powered** backend
- 📦 **Multi-arch** Docker images

</td>
<td width="50%">

### 🎯 **Integration**

- 🎬 **Sonarr & Radarr** compatible
- 🔍 **TMDB** metadata enrichment
- 📡 **Newznab API** indexer
- 🔄 **Real-time** WebSocket updates

</td>
</tr>
<tr>
<td width="50%">

### 🎨 **User Experience**

- 💎 **Modern** SvelteKit UI
- 📱 **Responsive** design
- 🌙 **Dark mode** ready
- ⚙️ **Easy** configuration

</td>
<td width="50%">

### 🛡️ **Reliability**

- 🔄 **Auto-retry** on failures
- 💾 **SQLite** database
- 🐳 **Docker** deployment
- 📊 **Health** monitoring

</td>
</tr>
</table>

---

## 🚀 Quick Start

### One-Line Installation

```bash
curl -sSL https://raw.githubusercontent.com/duytran1406/flasharr/main/install.sh | bash
```

### Docker Compose (Recommended)

```bash
# Download docker-compose.yml
curl -O https://raw.githubusercontent.com/duytran1406/flasharr/main/docker-compose.production.yml
mv docker-compose.production.yml docker-compose.yml

# Start Flasharr
docker compose up -d

# Access at http://localhost:8484
```

### Docker Run

```bash
docker run -d \
  --name flasharr \
  -p 8484:8484 \
  -v ./appData:/appData \
  --restart unless-stopped \
  ghcr.io/duytran1406/flasharr:latest
```

---

## 📦 Installation Methods

<details>
<summary><b>🐳 Docker Compose (Recommended)</b></summary>

Create `docker-compose.yml`:

```yaml
version: "3.8"

services:
  flasharr:
    image: ghcr.io/duytran1406/flasharr:latest
    container_name: flasharr
    restart: unless-stopped
    ports:
      - "8484:8484"
    volumes:
      - ./appData:/appData
      # Optional: Mount your media library
      # - /path/to/downloads:/appData/downloads
    environment:
      - FLASHARR_APPDATA_DIR=/appData
      - RUST_LOG=flasharr=info,tower_http=info
      - TZ=Asia/Bangkok
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8484/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
```

Start:

```bash
docker compose up -d
```

</details>

<details>
<summary><b>🔄 Auto-Update with Watchtower</b></summary>

Add Watchtower to your `docker-compose.yml`:

```yaml
services:
  flasharr:
    # ... your flasharr config
    labels:
      - "com.centurylinklabs.watchtower.enable=true"

  watchtower:
    image: containrrr/watchtower
    container_name: watchtower
    restart: unless-stopped
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
    environment:
      - WATCHTOWER_CLEANUP=true
      - WATCHTOWER_POLL_INTERVAL=86400 # Check daily
      - WATCHTOWER_LABEL_ENABLE=true
```

</details>

<details>
<summary><b>🏗️ Build from Source</b></summary>

```bash
# Clone repository
git clone https://github.com/duytran1406/flasharr.git
cd flasharr

# Build with Docker
docker compose up -d --build
```

</details>

---

## ⚙️ Configuration

### First-Time Setup

1. **Access Web UI**: Navigate to `http://localhost:8484`
2. **Complete Setup Wizard**:
   - Enter Fshare credentials (VIP account required)
   - Add TMDB API key ([Get free key](https://www.themoviedb.org/settings/api))
   - Configure download paths

### Environment Variables

| Variable               | Description    | Default         |
| ---------------------- | -------------- | --------------- |
| `FLASHARR_APPDATA_DIR` | Data directory | `/appData`      |
| `RUST_LOG`             | Log level      | `flasharr=info` |
| `TZ`                   | Timezone       | `UTC`           |

### Volume Mounts

| Host Path            | Container Path       | Purpose           |
| -------------------- | -------------------- | ----------------- |
| `./appData`          | `/appData`           | Database & config |
| `/path/to/downloads` | `/appData/downloads` | Downloaded files  |

---

## 🔗 \*arr Integration

### Add to Sonarr/Radarr

#### As Indexer (Search)

1. **Settings** → **Indexers** → **Add** → **Newznab**
2. Configure:
   - **Name**: `Flasharr`
   - **URL**: `http://flasharr:8484/api/indexer`
   - **API Key**: (from Flasharr settings)
3. **Test** → **Save**

#### As Download Client

1. **Settings** → **Download Clients** → **Add** → **SABnzbd**
2. Configure:
   - **Name**: `Flasharr`
   - **Host**: `flasharr`
   - **Port**: `8484`
   - **API Key**: (from Flasharr settings)
3. **Test** → **Save**

---

## 📊 Performance

| Metric              | Value          |
| ------------------- | -------------- |
| **Memory (Idle)**   | ~30 MB         |
| **Memory (Active)** | ~100 MB        |
| **CPU (Idle)**      | < 0.5%         |
| **Download Speed**  | Up to 300 MB/s |
| **Startup Time**    | ~0.2s          |

---

## 🐳 Docker Tags

| Tag       | Description              | Update Frequency  |
| --------- | ------------------------ | ----------------- |
| `latest`  | Latest stable release    | On new releases   |
| `stable`  | Production recommended   | On new releases   |
| `nightly` | Latest development build | Daily at 2 AM UTC |
| `v2.0.0`  | Specific version         | Never             |
| `v2.0`    | Auto-patch updates       | On patch releases |
| `v2`      | Auto-minor updates       | On minor releases |

**Recommended for production**: `stable` or specific version tags

---

## 📚 Documentation

- 📖 [Installation Guide](docs/INSTALLATION.md)
- ⚙️ [Configuration](docs/CONFIGURATION.md)
- 🔧 [Troubleshooting](docs/TROUBLESHOOTING.md)
- 🔌 [API Documentation](docs/API.md)
- 🏷️ [Docker Tags Guide](docs/DOCKER_TAGS.md)

---

## 🆘 Troubleshooting

### Common Issues

<details>
<summary><b>Container won't start</b></summary>

```bash
# Check logs
docker logs flasharr

# Verify ports aren't in use
netstat -tulpn | grep 8484

# Check volume permissions
ls -la ./appData
```

</details>

<details>
<summary><b>Fshare login fails</b></summary>

- Verify VIP account is active
- Check credentials in settings
- Ensure IP isn't blocked by Fshare
- Try manual login at fshare.vn

</details>

<details>
<summary><b>Sonarr/Radarr can't connect</b></summary>

```bash
# Verify containers are on same network
docker network inspect bridge

# Test connectivity
docker exec sonarr ping flasharr

# Check API key matches
```

</details>

### View Logs

```bash
# Live logs
docker logs -f flasharr

# Last 100 lines
docker logs --tail 100 flasharr

# Save logs to file
docker logs flasharr > flasharr.log 2>&1
```

---

## 🔄 Updating

### Docker Compose

```bash
docker compose pull
docker compose up -d
```

### Docker Run

```bash
docker pull ghcr.io/duytran1406/flasharr:latest
docker stop flasharr
docker rm flasharr
# Run with same command as before
```

---

## 🛠️ Development

### Prerequisites

- Rust 1.75+
- Node.js 20+
- Docker (optional)

### Local Development

```bash
# Clone repository
git clone https://github.com/duytran1406/flasharr.git
cd flasharr

# Start backend
cd backend
cargo run

# Start frontend (new terminal)
cd frontend
npm install
npm run dev
```

### Build

```bash
# Backend
cd backend
cargo build --release

# Frontend
cd frontend
npm run build
```

---

## 🤝 Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- [Sonarr](https://sonarr.tv/) & [Radarr](https://radarr.video/) - The amazing \*arr ecosystem
- [TMDB](https://www.themoviedb.org/) - Media metadata API
- [Fshare](https://www.fshare.vn/) - Vietnamese file hosting service

---

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/duytran1406/flasharr/issues)
- **Discussions**: [GitHub Discussions](https://github.com/duytran1406/flasharr/discussions)

---

<div align="center">

**⭐ Star this repo if you find it useful!**

Made with ❤️ for the home media enthusiast community

[⬆ Back to Top](#-flasharr)

</div>
