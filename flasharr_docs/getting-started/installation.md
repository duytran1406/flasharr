# Installation Guide

This guide covers installation of Flasharr using Docker (recommended) or manual installation.

---

## Prerequisites

- **Fshare.vn VIP Account** - Required for downloads
- **Docker & Docker Compose** (recommended) - For containerized deployment
- **Python 3.9+** (manual install) - For running without Docker

---

## Docker Installation (Recommended)

### 1. Clone the Repository

```bash
git clone https://github.com/yourusername/fshare-arr-bridge.git
cd fshare-arr-bridge
```

### 2. Create Environment File

Create `/mnt/appdata/Flasharr/.env`:

```bash
# Fshare Credentials
FSHARE_EMAIL=your-email@example.com
FSHARE_PASSWORD=your-password

# Server Configuration
INDEXER_PORT=8484
HOST=0.0.0.0

# Download Configuration
DOWNLOAD_DIR=/downloads
MAX_CONCURRENT_DOWNLOADS=2
SEGMENTS_PER_DOWNLOAD=4

# Optional: Multi-Account (comma-separated)
# FSHARE_ACCOUNTS=email1:pass1,email2:pass2
```

### 3. Create Data Directory

```bash
mkdir -p /mnt/appdata/Flasharr/data
mkdir -p /data/fshare-downloader
```

### 4. Start the Container

```bash
docker-compose up -d
```

### 5. Verify Installation

```bash
# Check logs
docker-compose logs -f flasharr

# Test health endpoint
curl http://localhost:8484/health
```

You should see:
```json
{"status": "healthy", "version": "0.0.3-beta"}
```

---

## Manual Installation

### 1. Clone Repository

```bash
git clone https://github.com/yourusername/fshare-arr-bridge.git
cd fshare-arr-bridge
```

### 2. Create Virtual Environment

```bash
python3 -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
```

### 3. Install Dependencies

```bash
pip install -r requirements.txt
```

### 4. Configure Environment

Create `.env` file in project root:

```bash
FSHARE_EMAIL=your-email@example.com
FSHARE_PASSWORD=your-password
INDEXER_PORT=8484
DOWNLOAD_DIR=./downloads
```

### 5. Run the Application

```bash
python -m flasharr
```

Or use the run script:

```bash
./start.sh
```

---

## Post-Installation Setup

### Access Web Interface

Navigate to: `http://localhost:8484`

You should see the Flasharr dashboard.

### Configure *arr Integration

See the [Configuration Guide](configuration.md) for:
- Adding Flasharr to Prowlarr as an indexer
- Adding Flasharr to Radarr/Sonarr as a download client

---

## Directory Structure

After installation, your directory structure should look like:

```
/mnt/appdata/Flasharr/
├── .env                    # Environment configuration
├── data/
│   ├── accounts.json       # Multi-account data
│   ├── downloads.db        # Download queue database
│   └── cache/              # Search cache
└── logs/
    └── flasharr.log        # Application logs

/data/fshare-downloader/    # Downloaded files
├── movies/
├── tv/
└── temp/                   # Incomplete downloads
```

---

## Updating Flasharr

### Docker Update

```bash
cd /etc/pve/fshare-arr-bridge
docker-compose pull
docker-compose up -d
```

### Manual Update

```bash
cd /etc/pve/fshare-arr-bridge
git pull
source venv/bin/activate
pip install -r requirements.txt --upgrade
```

---

## Uninstallation

### Docker

```bash
docker-compose down
docker rmi flasharr:beta
rm -rf /mnt/appdata/Flasharr
```

### Manual

```bash
deactivate  # Exit virtual environment
rm -rf /etc/pve/fshare-arr-bridge
```

---

## Troubleshooting Installation

### Container Won't Start

**Check logs:**
```bash
docker-compose logs flasharr
```

**Common issues:**
- Missing `.env` file
- Invalid Fshare credentials
- Port 8484 already in use

### Permission Errors

**Fix ownership:**
```bash
sudo chown -R 1000:1000 /mnt/appdata/Flasharr
sudo chown -R 1000:1000 /data/fshare-downloader
```

### Python Version Issues

**Check version:**
```bash
python3 --version  # Should be 3.9 or higher
```

---

## Next Steps

- [Quick Start Guide](quick-start.md) - Get started with Flasharr
- [Configuration](configuration.md) - Configure Flasharr and *arr integration
- [Web Interface](../user-guide/web-interface.md) - Learn the dashboard
