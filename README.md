<p align="center">
  <img src="https://img.shields.io/badge/version-3.0.0-blue?style=for-the-badge" alt="Version">
  <img src="https://img.shields.io/badge/rust-1.75+-orange?style=for-the-badge&logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/svelte-5-ff3e00?style=for-the-badge&logo=svelte" alt="Svelte">
  <img src="https://img.shields.io/badge/docker-ready-2496ED?style=for-the-badge&logo=docker" alt="Docker">
</p>

# 🚀 Flasharr

**Flasharr** is a lightweight, high-performance download manager that bridges **Fshare** (Vietnamese file hosting service) with the **\*arr ecosystem** (Sonarr & Radarr). It enables automated downloading and importing of media files directly into your Plex/Jellyfin library.

Built with **Rust** and **SvelteKit**, Flasharr is optimized for low-resource hardware like mini PCs (Intel N100, 4GB RAM) while delivering blazing-fast download speeds.

---

## ✨ Key Features

- 🎬 **\*arr Integration** — Seamlessly works with Sonarr and Radarr for automated media management
- 🔍 **Smart Search** — Search Fshare with TMDB-enriched metadata (posters, ratings, descriptions)
- ⬇️ **Intelligent Downloads** — State machine with smart error handling and automatic retries
- ⚡ **High Performance** — Download speeds up to 300 MB/s with minimal resource usage
- 🔄 **Real-time Updates** — WebSocket-based progress tracking
- 📱 **Modern UI** — Beautiful, responsive web interface
- 🐳 **Docker Ready** — One-command deployment

---

## 📋 Requirements

- **Docker** and **Docker Compose** (recommended)
- **Fshare VIP account** (required for premium download speeds)
- **TMDB API key** (free, for media metadata)
- Optionally: **Sonarr** and/or **Radarr** for automation

---

## 🚀 Quick Start

### Step 1: Create Project Directory

```bash
mkdir flasharr && cd flasharr
```

### Step 2: Create Configuration Files

Create a `docker-compose.yml` file:

```yaml
version: "3.8"

services:
  flasharr:
    image: flasharr:latest
    container_name: flasharr
    restart: unless-stopped
    ports:
      - "8484:8484"
    volumes:
      # AppData volume for configuration and database
      - ./appData:/appData

      # Mount your media library (adjust paths as needed)
      # - /path/to/downloads:/appData/downloads
    environment:
      - FLASHARR_APPDATA_DIR=/appData
      - RUST_LOG=flasharr=info,tower_http=info
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8484/health"]
      interval: 30s
      timeout: 3s
      retries: 3
      start_period: 10s
```

Create a `.env` file in the same directory:

```bash
# Fshare Credentials (REQUIRED)
FSHARE_EMAIL=your-email@example.com
FSHARE_PASSWORD=your-fshare-password

# TMDB API Key (REQUIRED for media search)
# Get your free API key at: https://www.themoviedb.org/settings/api
TMDB_API_KEY=your-tmdb-api-key

# Indexer API Key (for Sonarr/Radarr authentication)
# Generate a secure key: openssl rand -hex 32
API_KEY=your-generated-api-key

# Server Configuration
FLASHARR_PORT=8484
DEBUG=false
```

### Step 3: Start Flasharr

```bash
docker-compose up -d
```

### Step 4: Access the Web UI

Open your browser and navigate to:

```
http://localhost:8484
```

---

## ⚙️ Configuration

### Getting Your API Keys

#### TMDB API Key (Free)

1. Create an account at [themoviedb.org](https://www.themoviedb.org/signup)
2. Go to **Settings** → **API**
3. Request an API key (choose "Developer" for personal use)
4. Copy your **API Key (v3 auth)**

#### Fshare VIP Account

A Fshare VIP account is required for high-speed downloads. Visit [fshare.vn](https://www.fshare.vn) to purchase a subscription.

---

## 🔗 Sonarr & Radarr Integration

Flasharr acts as both an **Indexer** (search) and **Download Client** (SABnzbd-compatible) for the \*arr suite.

### Adding Flasharr to Sonarr/Radarr

#### Step 1: Add as Indexer

1. Open **Sonarr/Radarr** → **Settings** → **Indexers**
2. Click **+ Add Indexer** → Choose **Newznab** (Generic)
3. Configure:
   - **Name:** `Flasharr`
   - **URL:** `http://flasharr:8484/api/indexer` (or use your Flasharr IP)
   - **API Key:** Your `API_KEY` from `.env`
4. Click **Test** then **Save**

#### Step 2: Add as Download Client

1. Open **Sonarr/Radarr** → **Settings** → **Download Clients**
2. Click **+ Add** → Choose **SABnzbd**
3. Configure:
   - **Name:** `Flasharr`
   - **Host:** `flasharr` (or your Flasharr IP)
   - **Port:** `8484`
   - **API Key:** Your `API_KEY` from `.env`
   - **Category (TV):** `tv` (for Sonarr)
   - **Category (Movies):** `movies` (for Radarr)
4. Click **Test** then **Save**

### Docker Network Setup

If running all services in Docker, ensure they share a network:

```yaml
version: "3.8"

services:
  flasharr:
    image: flasharr:latest
    container_name: flasharr
    networks:
      - arr-network
    # ... other config

  sonarr:
    image: linuxserver/sonarr
    container_name: sonarr
    networks:
      - arr-network
    # ... other config

  radarr:
    image: linuxserver/radarr
    container_name: radarr
    networks:
      - arr-network
    # ... other config

networks:
  arr-network:
    driver: bridge
```

---

## 📁 Directory Structure

After starting Flasharr, the following structure is created:

```
./appData/
├── config/         # Configuration files
│   └── config.json # Main config (auto-generated)
├── data/           # SQLite database
│   └── flasharr.db
├── downloads/      # Downloaded files
│   ├── tv/         # TV shows (for Sonarr)
│   └── movies/     # Movies (for Radarr)
└── logs/           # Application logs
```

### Custom Download Path

To use a different download location, mount it in your `docker-compose.yml`:

```yaml
volumes:
  - ./appData:/appData
  - /mnt/media/downloads:/appData/downloads
```

---

## 🔧 Advanced Configuration

### Environment Variables

| Variable               | Description                   | Default         |
| ---------------------- | ----------------------------- | --------------- |
| `FLASHARR_APPDATA_DIR` | Data directory path           | `/appData`      |
| `FSHARE_EMAIL`         | Fshare login email            | -               |
| `FSHARE_PASSWORD`      | Fshare login password         | -               |
| `TMDB_API_KEY`         | TMDB API key for metadata     | -               |
| `API_KEY`              | API key for \*arr integration | -               |
| `FLASHARR_PORT`        | Server port                   | `8484`          |
| `RUST_LOG`             | Log level                     | `flasharr=info` |
| `DEBUG`                | Enable debug mode             | `false`         |

### Resource Limits (Optional)

For low-resource systems, add resource limits:

```yaml
services:
  flasharr:
    # ... other config
    deploy:
      resources:
        limits:
          cpus: "2"
          memory: 512M
        reservations:
          cpus: "0.5"
          memory: 128M
```

---

## 📊 Performance

Flasharr is optimized for minimal resource usage:

| Metric          | Flasharr V3    |
| --------------- | -------------- |
| Memory (Idle)   | ~30 MB         |
| Memory (Active) | ~100 MB        |
| CPU (Idle)      | < 0.5%         |
| Download Speed  | Up to 300 MB/s |
| Startup Time    | ~0.2s          |

---

## 🆘 Troubleshooting

### Common Issues

#### Fshare Login Failed

- Verify your credentials in `.env`
- Check if your VIP subscription is active
- Ensure your IP isn't blocked by Fshare

#### Sonarr/Radarr Connection Failed

- Verify the Flasharr container is running: `docker ps`
- Check the API key matches in both services
- Ensure containers are on the same Docker network

#### Downloads Not Starting

- Check Fshare account status in Settings
- Verify disk space is available
- Check logs: `docker logs flasharr`

### View Logs

```bash
# Live logs
docker logs -f flasharr

# Last 100 lines
docker logs --tail 100 flasharr
```

---

## 🔄 Updating

To update to the latest version:

```bash
docker-compose pull
docker-compose up -d
```

---

## 📜 License

This project is for personal use only. Flasharr is not affiliated with Fshare, Sonarr, Radarr, or any other mentioned services.

---

## 🙏 Acknowledgments

- [Sonarr](https://sonarr.tv/) & [Radarr](https://radarr.video/) — The amazing \*arr ecosystem
- [TMDB](https://www.themoviedb.org/) — Media metadata API
- [Fshare](https://www.fshare.vn/) — Vietnamese file hosting service

---

<p align="center">
  Made with ❤️ for the home media enthusiast community
</p>
