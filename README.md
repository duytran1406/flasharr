# Fshare-Arr Bridge

A complete integration solution for using Fshare.vn with Sonarr/Radarr and pyLoad.

## Features

- 🔍 **TimFshare Search Integration**: Search Fshare content using TimFshare.com API
- 📥 **SABnzbd-Compatible API**: Acts as a download client for *arr suite
- 🔄 **Automatic Filename Normalization**: Fixes non-standard Fshare filenames for *arr compatibility
- 🎯 **pyLoad with Fshare Plugins**: Integrated pyLoad with FshareVn and FshareVnFolder plugins
- 🌏 **Vietnamese Media Support**: Handles Vietnamese naming conventions and special characters
- 🐳 **Complete Docker Solution**: Single-command deployment with all services included

## Architecture

```
Sonarr/Radarr
    ↓
Prowlarr (searches via Fshare Indexer)
    ↓
Fshare-Arr Bridge
    ├─ TimFshare API (search)
    ├─ Filename Normalizer
    └─ SABnzbd API (download client)
    ↓
pyLoad (with Fshare plugins)
    ↓
Fshare.vn (download)
```

## What's Included

- **Fshare-Arr Bridge**: Prowlarr indexer + SABnzbd download client API
- **pyLoad**: Download manager with FshareVn and FshareVnFolder plugins pre-configured
- **TimFshare Integration**: Uses timfshare.com API for searching (no Fshare API key needed for search)
- **Automatic Setup**: One-command deployment with all dependencies

## The Problem This Solves

Fshare filenames often have quality markers BEFORE episode identifiers:
```
❌ Ling Cage 2019 4K HDR 10Bit S1E14 SP TVP TMPĐ_kimngonx5 (2019) 2160p
   └─────────┬─────────┘     └─┬─┘
   Included in title        Episode
   
Result: Sonarr searches for "Ling Cage 4K HDR 10Bit" → No match
```

This bridge normalizes filenames to standard format:
```
✅ Ling Cage S01E14 2019 4K HDR 10Bit SP TVP TMPĐ kimngonx5 2160p
   └───┬───┘ └──┬──┘ └──────────────┬──────────────────┘
    Title   Episode        Quality markers
    
Result: Sonarr searches for "Ling Cage" → Match found!
```

## Quick Start

### Using Docker Compose

```yaml
version: '3.8'

services:
  fshare-arr-bridge:
    image: fshare-arr-bridge:latest
    container_name: fshare-arr-bridge
    ports:
      - "8484:8484"  # Indexer API
      - "8585:8585"  # SABnzbd API
    environment:
      # Fshare credentials
      FSHARE_EMAIL: your-email@example.com
      FSHARE_PASSWORD: your-password
      
      # pyLoad connection
      PYLOAD_HOST: 192.168.1.112
      PYLOAD_PORT: 8000
      PYLOAD_USERNAME: admin
      PYLOAD_PASSWORD: password
      
      # Optional: Enable debug logging
      DEBUG: "false"
    restart: unless-stopped
```

### Manual Installation

```bash
# Clone the repository
git clone /path/to/fshare-arr-bridge
cd fshare-arr-bridge

# Install dependencies
pip install -r requirements.txt

# Configure environment
cp .env.example .env
nano .env

# Run the bridge
python main.py
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `FSHARE_EMAIL` | Your Fshare account email | Required |
| `FSHARE_PASSWORD` | Your Fshare account password | Required |
| `PYLOAD_HOST` | pyLoad server hostname/IP | `localhost` |
| `PYLOAD_PORT` | pyLoad server port | `8000` |
| `PYLOAD_USERNAME` | pyLoad username | `admin` |
| `PYLOAD_PASSWORD` | pyLoad password | Required |
| `INDEXER_PORT` | Indexer API port | `8484` |
| `SABNZBD_PORT` | SABnzbd API port | `8585` |
| `DEBUG` | Enable debug logging | `false` |

### Prowlarr Configuration

1. **Add Indexer**:
   - Go to Prowlarr → Settings → Indexers → Add
   - Select "Generic Newznab"
   - Name: `Fshare`
   - URL: `http://your-bridge-ip:8484`
   - API Key: (leave blank or use any value)
   - Categories: TV, Movies

2. **Test**: Click "Test" to verify connection

### Sonarr/Radarr Configuration

1. **Add Download Client**:
   - Go to Settings → Download Clients → Add
   - Select "SABnzbd"
   - Name: `Fshare Bridge`
   - Host: `your-bridge-ip`
   - Port: `8585`
   - API Key: `fshare-bridge-api-key`

2. **Test**: Click "Test" to verify connection

## API Endpoints

### Indexer API (Port 8484)

- `GET /api?t=search&q={query}` - Search for content
- `GET /api?t=caps` - Get indexer capabilities

### SABnzbd API (Port 8585)

- `POST /api?mode=addfile` - Add download from NZB
- `GET /api?mode=queue` - Get download queue
- `GET /api?mode=history` - Get download history

## Development

### Project Structure

```
fshare-arr-bridge/
├── app/
│   ├── __init__.py
│   ├── main.py              # Main application
│   ├── indexer.py           # Prowlarr indexer API
│   ├── sabnzbd.py           # SABnzbd-compatible API
│   ├── fshare_client.py     # Fshare API client
│   ├── pyload_client.py     # pyLoad API client
│   ├── filename_parser.py   # Filename normalization
│   └── models.py            # Data models
├── tests/
│   ├── test_parser.py
│   ├── test_indexer.py
│   └── test_integration.py
├── Dockerfile
├── docker-compose.yml
├── requirements.txt
├── .env.example
└── README.md
```

### Running Tests

```bash
pytest tests/
```

### Building Docker Image

```bash
docker build -t fshare-arr-bridge:latest .
```

## Troubleshooting

### "Unable to identify correct episode(s)"

This means the filename normalization didn't work. Check:
1. The bridge is receiving the download request
2. Logs show filename normalization happening
3. The normalized filename follows pattern: `Series S01E01 Quality`

### Downloads not appearing in pyLoad

Check:
1. pyLoad credentials are correct
2. pyLoad is accessible from the bridge container
3. Bridge logs for pyLoad connection errors

### Indexer not showing results in Prowlarr

Check:
1. Fshare credentials are correct
2. Bridge is accessible from Prowlarr
3. Search query is valid

## License

MIT License - See LICENSE file for details

## Credits

- Built for integration with Sonarr/Radarr
- Uses pyLoad for Fshare downloads
- Filename parsing based on scene naming standards
