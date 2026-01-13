# Flasharr (Beta)

**A complete integration bridge for using Fshare.vn with Sonarr, Radarr, and Prowlarr.**

[![Version](https://img.shields.io/badge/version-0.0.3--beta-blue.svg)](./VERSION)
[![Docker](https://img.shields.io/badge/docker-ready-brightgreen.svg)](./docker-compose.yml)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](./LICENSE)

---

## 🚀 Features

- **🔍 Newznab/Torznab Indexer** - Search Fshare via Prowlarr
- **📥 SABnzbd-Compatible API** - Download client for Radarr/Sonarr
- **⚡ Built-in Download Engine** - Multi-threaded, segmented downloads
- **🔄 Multi-Account Support** - Load balancing across VIP accounts
- **📊 Priority Queue** - Prioritize important downloads
- **🎯 Real-time Updates** - WebSocket-powered dashboard
- **🌐 Web Dashboard** - Modern UI for management
- **🔧 Auto-Cleanup** - Automatic history management

---

## 📋 What's New in Beta

Flasharr beta is a **complete rewrite** with major improvements:

- ✅ **Native Download Engine** - No more PyLoad dependency
- ✅ **Multi-threaded Downloads** - Segmented downloads for maximum speed
- ✅ **Multi-Account Support** - Distribute downloads across VIP accounts
- ✅ **Priority System** - Control download order
- ✅ **Enhanced Dashboard** - Real-time statistics and control
- ✅ **Better Performance** - Optimized for speed and reliability

---

## 🏗️ Architecture

```
Prowlarr → Flasharr (Newznab API) → TimFshare.com (Search)
                ↓
Radarr/Sonarr → Flasharr (SABnzbd API) → Built-in Engine → Fshare.vn
                ↓
        Download Files
```

**No external download manager required!** Flasharr handles everything internally.

---

## 🚀 Quick Start

### Docker Installation (Recommended)

1. **Create environment file:**
   ```bash
   mkdir -p /mnt/appdata/Flasharr
   nano /mnt/appdata/Flasharr/.env
   ```

2. **Add your Fshare credentials:**
   ```bash
   FSHARE_EMAIL=your-email@example.com
   FSHARE_PASSWORD=your-password
   INDEXER_PORT=8484
   DOWNLOAD_DIR=/downloads
   MAX_CONCURRENT_DOWNLOADS=2
   SEGMENTS_PER_DOWNLOAD=4
   ```

3. **Start Flasharr:**
   ```bash
   docker-compose up -d
   ```

4. **Access the dashboard:**
   ```
   http://localhost:8484
   ```

---

## 🔧 Configuration

### Add to Prowlarr (Indexer)

1. **Settings** → **Indexers** → **Add Indexer**
2. Select **Newznab**
3. Configure:
   - **Name:** Fshare (Flasharr)
   - **URL:** `http://flasharr:8484/indexer`
   - **API Key:** (optional)
   - **Categories:** Movies (2000), TV (5000)

### Add to Radarr/Sonarr (Download Client)

1. **Settings** → **Download Clients** → **Add**
2. Select **SABnzbd**
3. Configure:
   - **Name:** Flasharr
   - **Host:** `flasharr`
   - **Port:** `8484`
   - **URL Base:** `/sabnzbd`
   - **Category:** movies (Radarr) or tv (Sonarr)

---

## 📚 Documentation

Complete documentation is available in the [`flasharr_docs/`](flasharr_docs/) directory:

- **[Getting Started](flasharr_docs/getting-started/)** - Installation, quick start, configuration
- **[User Guide](flasharr_docs/user-guide/)** - Web interface, download management, troubleshooting
- **[API Reference](flasharr_docs/api-reference/)** - Newznab, SABnzbd, Engine APIs
- **[Architecture](flasharr_docs/architecture/)** - System design and internals
- **[Development](flasharr_docs/development/)** - Contributing, testing, changelog

**Quick Links:**
- [Installation Guide](flasharr_docs/getting-started/installation.md)
- [Quick Start](flasharr_docs/getting-started/quick-start.md)
- [Troubleshooting](flasharr_docs/user-guide/troubleshooting.md)
- [Changelog](flasharr_docs/development/changelog.md)

---

## 🎯 Key Features Explained

### Multi-threaded Downloads
Download files using multiple segments simultaneously for maximum speed:
```bash
SEGMENTS_PER_DOWNLOAD=8  # More segments = faster downloads
```

### Multi-Account Support
Distribute downloads across multiple VIP accounts:
```bash
FSHARE_ACCOUNTS=email1:pass1,email2:pass2,email3:pass3
```

### Priority Queue
Control download order:
- **URGENT** - Download immediately
- **HIGH** - Before normal downloads
- **NORMAL** - Default priority
- **LOW** - Download last

### Global Speed Limiting
Limit bandwidth usage:
```bash
GLOBAL_SPEED_LIMIT_MBPS=10  # Limit to 10 MB/s
```

---

## 🔌 API Endpoints

### Indexer API (Prowlarr)
- `GET /indexer/api?t=caps` - Capabilities
- `GET /indexer/api?t=search&q={query}` - Search
- `GET /indexer/nzb/{guid}` - Get NZB

### SABnzbd API (Radarr/Sonarr)
- `POST /sabnzbd/api?mode=addurl` - Add download
- `GET /sabnzbd/api?mode=queue` - Get queue
- `GET /sabnzbd/api?mode=history` - Get history

### Engine API (Direct Control)
- `GET /api/downloads` - List downloads
- `POST /api/downloads` - Add download
- `POST /api/downloads/{id}/pause` - Pause download
- `POST /api/downloads/{id}/resume` - Resume download

---

## 🐳 Docker Compose

```yaml
services:
  flasharr:
    image: flasharr:beta
    container_name: flasharr
    restart: unless-stopped
    ports:
      - "8484:8484"
    env_file:
      - /mnt/appdata/Flasharr/.env
    volumes:
      - /mnt/appdata/Flasharr/data:/app/data
      - /data/fshare-downloader:/downloads
```

---

## 🛠️ Development

### Project Structure

```
fshare-arr-bridge/
├── src/flasharr/              # Main application code
│   ├── api/                   # API endpoints
│   ├── clients/               # Fshare & TimFshare clients
│   ├── core/                  # Core services
│   ├── downloader/            # Download engine
│   ├── services/              # Business logic
│   ├── web/                   # Web UI
│   └── websocket/             # Real-time updates
├── tests/                     # Test suite
├── flasharr_docs/             # Documentation
├── docker-compose.yml         # Docker configuration
├── Dockerfile                 # Container definition
└── README.md                  # This file
```

### Running Tests

```bash
pytest tests/ -v
```

### Building from Source

```bash
docker-compose build
```

---

## 🐛 Troubleshooting

### Container Won't Start
```bash
docker-compose logs flasharr
```

### No Search Results
```bash
# Test indexer directly
curl "http://localhost:8484/indexer/api?t=search&q=test"
```

### Downloads Not Starting
```bash
# Check queue
curl "http://localhost:8484/sabnzbd/api?mode=queue"
```

See the [Troubleshooting Guide](flasharr_docs/user-guide/troubleshooting.md) for more solutions.

---

## 📝 Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `FSHARE_EMAIL` | Fshare account email | Required |
| `FSHARE_PASSWORD` | Fshare account password | Required |
| `INDEXER_PORT` | Web server port | `8484` |
| `DOWNLOAD_DIR` | Download directory | `/downloads` |
| `MAX_CONCURRENT_DOWNLOADS` | Max simultaneous downloads | `2` |
| `SEGMENTS_PER_DOWNLOAD` | Segments per file | `4` |
| `GLOBAL_SPEED_LIMIT_MBPS` | Speed limit (0=unlimited) | `0` |

See [Configuration Guide](flasharr_docs/getting-started/configuration.md) for all options.

---

## 🤝 Contributing

Contributions are welcome! See [Contributing Guide](flasharr_docs/development/contributing.md).

---

## 📄 License

MIT License - See LICENSE file for details.

---

## 🙏 Credits

- Built for integration with the *arr media management suite
- Uses TimFshare.com for Fshare search
- Implements Newznab/Torznab and SABnzbd API standards

---

## 📞 Support

- **Documentation:** [flasharr_docs/](flasharr_docs/)
- **Issues:** [GitHub Issues](https://github.com/yourusername/fshare-arr-bridge/issues)
- **Version:** 0.0.3-beta
