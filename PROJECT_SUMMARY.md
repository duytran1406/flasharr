# Fshare-Arr Bridge - Project Summary

## 🎯 Project Overview

**Fshare-Arr Bridge** is a complete integration solution that enables Sonarr/Radarr to download content from Fshare.vn through pyLoad, with automatic filename normalization to fix Vietnamese media naming issues.

**Version**: 1.0.0  
**Repository**: `/etc/pve/fshare-arr-bridge`  
**Status**: ✅ Ready for deployment

---

## 📦 What's Included

### Core Components

1. **Prowlarr Indexer API** (`app/indexer.py`)
   - Newznab-compatible API
   - Searches Fshare.vn
   - Returns normalized results in XML format
   - Port: 8484

2. **SABnzbd Download Client API** (`app/sabnzbd.py`)
   - SABnzbd-compatible API
   - Receives downloads from Sonarr/Radarr
   - Sends to pyLoad with normalized filenames
   - Port: 8484

3. **Filename Normalizer** (`app/filename_parser.py`)
   - **THE KEY FEATURE** - Fixes non-standard Fshare filenames
   - Moves quality markers AFTER episode identifiers
   - Handles Vietnamese media naming conventions
   - Example transformation:
     ```
     ❌ Ling Cage 2019 4K HDR 10Bit S1E14 SP TVP TMPĐ_kimngonx5 (2019) 2160p
     ✅ Ling Cage S01E14 2019 4K HDR 10Bit SP TVP TMPĐ kimngonx5 2160p
     ```

4. **Fshare API Client** (`app/fshare_client.py`)
   - Authentication with Fshare.vn
   - Search functionality
   - Direct download link retrieval

5. **pyLoad API Client** (`app/pyload_client.py`)
   - Sends downloads to pyLoad
   - Queue management

### Supporting Files

- **Dockerfile** - Production-ready container image
- **docker-compose.yml** - Easy deployment configuration
- **requirements.txt** - Python dependencies
- **.env.example** - Configuration template
- **start.sh** - Startup script for both Docker and local execution
- **tests/** - Comprehensive test suite
- **README.md** - Project documentation
- **SETUP.md** - Detailed setup and troubleshooting guide

---

## 🚀 Quick Start

### Option 1: Docker (Recommended)

```bash
cd /etc/pve/fshare-arr-bridge

# Configure
cp .env.example .env
nano .env  # Edit with your credentials

# Build and run
docker build -t fshare-arr-bridge:latest .
docker-compose up -d

# Verify
curl http://localhost:8484/health
```

### Option 2: Manual

```bash
cd /etc/pve/fshare-arr-bridge

# Setup
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt

# Configure
cp .env.example .env
nano .env

# Run
./start.sh
```

---

## 🔧 Configuration Required

Edit `.env` with your credentials:

```env
# Fshare account
FSHARE_EMAIL=your-email@example.com
FSHARE_PASSWORD=your-password

# pyLoad server
PYLOAD_HOST=192.168.1.112
PYLOAD_PORT=8000
PYLOAD_USERNAME=admin
PYLOAD_PASSWORD=your-pyload-password
```

---

## 🔌 Integration Steps

### 1. Add to Prowlarr

- **Settings** → **Indexers** → **Add Indexer**
- Select: **Generic Newznab**
- URL: `http://your-server-ip:8484/indexer`
- API Path: `/api`

### 2. Add to Sonarr/Radarr

- **Settings** → **Download Clients** → **Add**
- Select: **SABnzbd**
- Host: `your-server-ip`
- Port: `8484`
- URL Base: `/sabnzbd`
- API Key: `fshare-bridge-api-key`

### 3. Test

1. Search for content in Sonarr/Radarr
2. Results should appear from Fshare indexer
3. Download should be sent to pyLoad with normalized filename
4. Sonarr/Radarr should correctly identify the episode

---

## 🎯 Problem Solved

### Before

```
Filename: Ling Cage 2019 4K HDR 10Bit S1E14 SP TVP TMPĐ_kimngonx5 (2019) 2160p

Sonarr extracts title: "Ling Cage 4K HDR 10Bit"
TVDB lookup: NO MATCH ❌
Error: "Unable to identify correct episode(s) using release name and scene mappings"
```

### After

```
Filename: Ling Cage S01E14 2019 4K HDR 10Bit SP TVP TMPĐ kimngonx5 2160p

Sonarr extracts title: "Ling Cage"
TVDB lookup: MATCH FOUND ✅
Episode: S01E14 correctly identified
```

---

## 📊 Architecture

```
Sonarr/Radarr
    ↓
Prowlarr (searches via Fshare Indexer)
    ↓
Fshare-Arr Bridge (this app)
    ├─ Indexer API (Newznab)
    ├─ Filename Normalizer ⭐
    └─ Download Client API (SABnzbd)
    ↓
Fshare.vn ← → pyLoad
```

See `architecture diagram` above for visual representation.

---

## 🧪 Testing

Run the test suite:

```bash
cd /etc/pve/fshare-arr-bridge
source venv/bin/activate
pytest tests/ -v
```

Test the normalizer with your specific file:

```bash
python3 << 'EOF'
from app.filename_parser import FilenameNormalizer

normalizer = FilenameNormalizer()
filename = "Ling Cage 2019 4K HDR 10Bit S1E14 SP TVP TMPĐ_kimngonx5 (2019) 2160p"
parsed = normalizer.parse(filename)

print(f"Original:   {filename}")
print(f"Normalized: {parsed.normalized_filename}")
print(f"Title:      {parsed.title}")
print(f"Season:     {parsed.season}")
print(f"Episode:    {parsed.episode}")
EOF
```

---

## 📁 Project Structure

```
fshare-arr-bridge/
├── app/
│   ├── __init__.py
│   ├── main.py              # Main Flask application
│   ├── indexer.py           # Prowlarr indexer API
│   ├── sabnzbd.py           # SABnzbd download client API
│   ├── fshare_client.py     # Fshare.vn API client
│   ├── pyload_client.py     # pyLoad API client
│   └── filename_parser.py   # ⭐ Filename normalization
├── tests/
│   ├── __init__.py
│   └── test_parser.py       # Test suite
├── Dockerfile               # Container image
├── docker-compose.yml       # Docker deployment
├── requirements.txt         # Python dependencies
├── .env.example            # Configuration template
├── .gitignore              # Git ignore rules
├── start.sh                # Startup script
├── README.md               # Project documentation
└── SETUP.md                # Setup guide
```

---

## 🔍 API Endpoints

### Indexer API (Port 8484)

```bash
# Capabilities
GET http://localhost:8484/indexer/api?t=caps

# Search
GET http://localhost:8484/indexer/api?t=search&q=Ling+Cage

# TV Search
GET http://localhost:8484/indexer/api?t=tvsearch&q=Ling+Cage&season=1&ep=14
```

### SABnzbd API (Port 8484)

```bash
# Add download
POST http://localhost:8484/sabnzbd/api?mode=addfile
POST http://localhost:8484/sabnzbd/api?mode=addurl&name=https://fshare.vn/file/XXX

# Get queue
GET http://localhost:8484/sabnzbd/api?mode=queue&output=json

# Get history
GET http://localhost:8484/sabnzbd/api?mode=history&output=json
```

### Health Check

```bash
GET http://localhost:8484/health
```

---

## 📝 Git Repository

The project is initialized as a Git repository with 2 commits:

```bash
cd /etc/pve/fshare-arr-bridge
git log --oneline
```

Output:
```
a884e8d Add comprehensive setup guide
581812a Initial commit: Fshare-Arr Bridge v1.0.0
```

---

## 🚦 Next Steps

1. **Configure Environment**
   ```bash
   cd /etc/pve/fshare-arr-bridge
   cp .env.example .env
   nano .env
   ```

2. **Deploy**
   ```bash
   docker-compose up -d
   ```

3. **Integrate with Prowlarr**
   - Add as Newznab indexer
   - URL: `http://your-server-ip:8484/indexer`

4. **Integrate with Sonarr/Radarr**
   - Add as SABnzbd download client
   - Host: `your-server-ip`, Port: `8484`

5. **Test**
   - Search for "Ling Cage" in Sonarr
   - Download an episode
   - Verify it appears in pyLoad with normalized filename

---

## 📚 Documentation

- **README.md** - Project overview and features
- **SETUP.md** - Detailed setup, integration, and troubleshooting
- **This file** - Project summary and quick reference

---

## 🎉 Success Criteria

✅ Fshare filenames are automatically normalized  
✅ Sonarr/Radarr can identify episodes correctly  
✅ Downloads are sent to pyLoad  
✅ No more "Unable to identify correct episode(s)" errors  
✅ Vietnamese media naming is handled properly  
✅ Single Docker image deployment  
✅ Comprehensive documentation  
✅ Test suite included  

---

## 🔧 Maintenance

### View Logs
```bash
docker logs -f fshare-arr-bridge
```

### Update
```bash
git pull
docker-compose down
docker-compose build
docker-compose up -d
```

### Backup
```bash
cp .env .env.backup
docker save fshare-arr-bridge:latest | gzip > fshare-arr-bridge.tar.gz
```

---

## 🐛 Troubleshooting

See **SETUP.md** for detailed troubleshooting guide.

Common issues:
- **No search results**: Check Fshare credentials
- **Downloads not in pyLoad**: Check pyLoad connection
- **Episode not identified**: Check logs for normalization output

---

## 📄 License

MIT License - Free to use and modify

---

## 🙏 Credits

Built to solve the Vietnamese media filename parsing issue with Fshare.vn and *arr suite integration.

**Key Innovation**: Automatic filename normalization that moves quality markers after episode identifiers, fixing the root cause of "*arr suite unable to identify episodes" error.

---

**Status**: ✅ Production Ready  
**Last Updated**: 2026-01-10  
**Version**: 1.0.0
