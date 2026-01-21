# Flasharr Documentation

**Version:** 0.2.0-beta  
**Last Updated:** 2026-01-21

Flasharr is a bridge service that integrates Fshare.vn with the *arr media management suite (Radarr, Sonarr, Prowlarr). It provides both a Newznab/Torznab indexer interface and a SABnzbd-compatible download client.

---

## 📚 Documentation Index

### Getting Started
- **[Installation Guide](getting-started/installation.md)** - Docker installation instructions
- **[Quick Start](getting-started/quick-start.md)** - Get up and running in 5 minutes
- **[Configuration](getting-started/configuration.md)** - Environment variables and settings

### User Guide
- **[Troubleshooting](user-guide/troubleshooting.md)** - Common issues and solutions

### API Reference
- **[Newznab API](api-reference/newznab-api.md)** - Prowlarr indexer integration
- **[SABnzbd API](api-reference/sabnzbd-api.md)** - Radarr/Sonarr download client integration

### Architecture
- **[System Overview](architecture/overview.md)** - High-level architecture and design
- **[Code Structure](architecture/code-structure.md)** - Module and directory documentation

### Development
- **[Changelog](development/changelog.md)** - Version history and changes
- **[Deployment](development/deployment.md)** - Deployment guide
- **[Development Rules](development/rules.md)** - Coding guidelines

---

## 🎯 Key Features

| Feature | Description |
|---------|-------------|
| **Newznab Indexer** | Search Fshare via Prowlarr |
| **SABnzbd API** | Download client for Radarr/Sonarr |
| **Multi-threaded Downloads** | Segmented downloads for speed |
| **Multi-Account Support** | Load balancing across VIP accounts |
| **Auto-Reconnect** | Automatic session restoration |
| **WebSocket Updates** | Real-time progress notifications |
| **TMDB Integration** | Movie/TV discovery |
| **Modern UI** | Glassmorphism dashboard |

---

## 🚀 Quick Links

- **Web Dashboard:** `http://localhost:8484`
- **Health Check:** `http://localhost:8484/health`
- **Indexer API:** `http://localhost:8484/indexer/api`
- **SABnzbd API:** `http://localhost:8484/sabnzbd/api`

---

## 📖 Documentation Conventions

- **Code blocks** - Exact commands or configuration
- **Tables** - Quick reference information
- **Alerts** - Important notes and warnings
  - 💡 **Note**: Additional context
  - ⚠️ **Warning**: Potential issues
  - 🚨 **Caution**: Critical information

---

## 📄 License

MIT License - See LICENSE file for details.
