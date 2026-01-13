# Flasharr Documentation

**Version:** 0.0.3-beta  
**Last Updated:** 2026-01-14

Flasharr is a bridge service that integrates Fshare.vn with the *arr media management suite (Radarr, Sonarr, Prowlarr). It provides both a Newznab/Torznab indexer interface and a SABnzbd-compatible download client.

---

## 📚 Documentation Index

### Getting Started
- **[Installation Guide](getting-started/installation.md)** - Docker and manual installation instructions
- **[Quick Start](getting-started/quick-start.md)** - Get up and running in 5 minutes
- **[Configuration](getting-started/configuration.md)** - Environment variables and settings

### User Guide
- **[Web Interface](user-guide/web-interface.md)** - Using the Flasharr dashboard
- **[Download Management](user-guide/download-management.md)** - Managing downloads, priorities, and queues
- **[Multi-Account Support](user-guide/multi-account.md)** - Using multiple Fshare VIP accounts
- **[Priority System](user-guide/priority-system.md)** - Download prioritization and scheduling
- **[Troubleshooting](user-guide/troubleshooting.md)** - Common issues and solutions

### API Reference
- **[Newznab API](api-reference/newznab-api.md)** - Prowlarr indexer integration
- **[SABnzbd API](api-reference/sabnzbd-api.md)** - Radarr/Sonarr download client integration
- **[Engine API](api-reference/engine-api.md)** - Direct download engine API
- **[WebSocket API](api-reference/websocket-api.md)** - Real-time updates and notifications

### Architecture
- **[System Overview](architecture/overview.md)** - High-level architecture and design
- **[Download Engine](architecture/download-engine.md)** - Multi-threaded download system
- **[Account Manager](architecture/account-manager.md)** - Multi-account load balancing
- **[WebSocket System](architecture/websocket-system.md)** - Real-time communication
- **[Integration Flow](architecture/integration-flow.md)** - How Flasharr integrates with *arr apps

### Development
- **[Development Setup](development/setup.md)** - Setting up a development environment
- **[Testing](development/testing.md)** - Running tests and test coverage
- **[Contributing](development/contributing.md)** - Contribution guidelines
- **[Changelog](development/changelog.md)** - Version history and changes

### Migration
- **[From Legacy Version](migration/from-legacy.md)** - Migrating from old fshare-arr-bridge
- **[Version History](migration/version-history.md)** - Evolution of the project

---

## 🚀 Quick Links

- **GitHub Repository:** [fshare-arr-bridge](https://github.com/yourusername/fshare-arr-bridge)
- **Docker Hub:** [flasharr](https://hub.docker.com/r/yourusername/flasharr)
- **Issue Tracker:** [GitHub Issues](https://github.com/yourusername/fshare-arr-bridge/issues)

---

## 🎯 Key Features

- **Newznab/Torznab Indexer** - Search Fshare via Prowlarr
- **SABnzbd Compatibility** - Download client for Radarr/Sonarr
- **Multi-threaded Downloads** - Segmented downloads for maximum speed
- **Multi-Account Support** - Load balancing across VIP accounts
- **Priority Queue** - Prioritize important downloads
- **WebSocket Updates** - Real-time progress notifications
- **Web Dashboard** - Modern UI for management
- **Auto-Cleanup** - Automatic history management

---

## 📖 Documentation Conventions

Throughout this documentation:

- **Code blocks** show exact commands or configuration
- **Mermaid diagrams** illustrate architecture and flows
- **Alerts** highlight important information:
  - 💡 **Note**: Additional context
  - ⚡ **Tip**: Best practices
  - ⚠️ **Warning**: Potential issues
  - 🚨 **Caution**: Critical information

---

## 🆘 Getting Help

1. Check the [Troubleshooting Guide](user-guide/troubleshooting.md)
2. Search [existing issues](https://github.com/yourusername/fshare-arr-bridge/issues)
3. Join our [Discord community](#) (if available)
4. Create a [new issue](https://github.com/yourusername/fshare-arr-bridge/issues/new)

---

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.
