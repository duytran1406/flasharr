# 🚀 Flasharr Quick Start Guide

Get Flasharr up and running in minutes!

## Prerequisites

- **Docker** (recommended) or Rust toolchain
- **Port 8484** available
- Modern web browser

## Installation Methods

### Option 1: Docker (Recommended)

```bash
# 1. Clone the repository
git clone https://github.com/yourusername/flasharr.git
cd flasharr/Flasharr

# 2. Create appData directory
mkdir -p appData/{config,data,downloads,logs}

# 3. Start with Docker Compose
docker-compose up -d

# 4. Access Flasharr
open http://localhost:8484
```

### Option 2: Local Development

```bash
# 1. Clone the repository
git clone https://github.com/yourusername/flasharr.git
cd flasharr/Flasharr

# 2. Run development script
./scripts/deploy/dev.sh

# 3. Access Flasharr
open http://localhost:8484
```

## First-Time Setup

### 1. Initial Configuration

When you first access Flasharr, you'll be guided through the setup wizard:

1. **Fshare Credentials** - Enter your Fshare account details
2. **TMDB API Key** - Add your TMDB API key for metadata
3. **Download Settings** - Configure concurrent downloads (1-10)
4. **Indexer Setup** (Optional) - Configure Newznab indexers

### 2. Configure Download Location

By default, downloads are saved to:

- **Docker**: `/downloads` (mapped to your host directory)
- **Local**: `./appData/downloads`

Update in Settings → General if needed.

### 3. Integrate with Sonarr/Radarr (Optional)

See [Sonarr/Radarr Integration Guide](../integration/sonarr-radarr.md) for detailed setup.

## Basic Usage

### Adding Downloads

**Method 1: Smart Search**

1. Navigate to **Discover** page
2. Search for a movie or TV show
3. Click **Smart Grab** to automatically find and queue episodes

**Method 2: Manual URL**

1. Go to **Downloads** page
2. Click **Add Download**
3. Paste Fshare URL
4. Click **Add**

### Managing Downloads

**Individual Actions:**

- **Pause** - Temporarily stop a download
- **Resume** - Continue a paused download
- **Delete** - Remove from queue

**Batch Actions:**

- Select multiple downloads or entire batches
- Apply pause/resume/delete to all selected

### Monitoring Progress

The Downloads page shows:

- **Real-time progress** - Updated via WebSocket
- **Download speed** - Current transfer rate
- **ETA** - Estimated time remaining
- **Status** - Queued, Downloading, Paused, Completed, Failed

## Key Features

### 🎯 Smart Grab

Automatically searches for and queues all episodes of a TV season with intelligent batch grouping.

### 📦 Batch Management

TV show episodes are automatically grouped by season for easy bulk operations.

### 🔄 Real-Time Updates

WebSocket-powered live updates without manual refresh.

### 🎬 TMDB Integration

Automatic metadata fetching for proper file organization and Sonarr/Radarr compatibility.

### 🔌 Newznab Bridge

Act as a Newznab indexer for Sonarr/Radarr integration.

## Common Tasks

### Pause All Downloads

```
Downloads → Select All → Pause
```

### Resume Failed Downloads

```
Downloads → Filter: Failed → Select All → Resume
```

### Clear Completed Downloads

```
Downloads → Filter: Completed → Select All → Delete
```

### Search for Content

```
Discover → Search → Browse Results → Smart Grab
```

## Troubleshooting

### Downloads Not Starting

**Check:**

1. Fshare credentials are valid (Settings → Fshare)
2. Concurrent download limit not reached (Settings → General)
3. Backend is running (check Docker logs or terminal)

**Fix:**

```bash
# Docker
docker-compose logs -f flasharr

# Local
# Check terminal running dev.sh
```

### WebSocket Not Connecting

**Symptoms:** UI doesn't update in real-time

**Fix:**

1. Hard refresh browser (Cmd+Shift+R / Ctrl+Shift+R)
2. Check browser console for errors
3. Verify backend is accessible at `http://localhost:8484/api/health`

### Metadata Not Loading

**Check:**

1. TMDB API key is configured (Settings → TMDB)
2. Internet connection is active
3. TMDB service is online

## Performance Tips

### Optimize Download Speed

- Increase concurrent downloads (Settings → General)
- Use wired connection instead of WiFi
- Close bandwidth-heavy applications

### Reduce Memory Usage

- Clear completed downloads regularly
- Limit concurrent downloads to 3-5
- Restart backend periodically for long-running instances

### Improve Search Results

- Use specific search terms
- Include year for movies
- Use original titles for foreign content

## Next Steps

- **[Sonarr/Radarr Integration](../integration/sonarr-radarr.md)** - Auto-import to media servers
- **[Publishing Guide](publishing.md)** - Deploy to production
- **[Architecture Docs](../architecture/)** - Understand how Flasharr works

## Getting Help

- **Logs**: Check Docker logs or backend terminal output
- **Health Check**: Visit `http://localhost:8484/api/health`
- **Documentation**: Browse other guides in `docs/`
- **Issues**: Report bugs on GitHub

---

**You're all set!** Start downloading and enjoy Flasharr! 🎉
