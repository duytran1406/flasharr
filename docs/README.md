# Flasharr Documentation

Welcome to the Flasharr documentation! This directory contains all the essential guides and references for using, deploying, and developing Flasharr.

## 📁 Directory Structure

```
docs/
├── setup/              # Getting started and deployment
├── architecture/       # System design and technical details
├── integration/        # External service integration guides
└── development/        # Developer resources
```

## 🚀 Quick Start

New to Flasharr? Start here:

1. [Quick Start Guide](setup/quick-start.md) - Get up and running in minutes
2. [Publishing Guide](setup/publishing.md) - Deploy to production

## 📚 Documentation Index

### Setup & Deployment

- **[Quick Start](setup/quick-start.md)** - Installation and initial setup
- **[Deployment Guide](setup/deployment.md)** - Complete deployment guide (local, Docker, staging, production)
- **[Publishing](setup/publishing.md)** - GitHub Container Registry setup
- **[Versioning](setup/versioning.md)** - Version management and releases
- **[Publishing Checklist](setup/publishing-checklist.md)** - Pre-deployment checklist

### Architecture

- **[Batch System](architecture/batch-system.md)** - Batch download management and UI design
- **[Metadata Handling](architecture/metadata-handling.md)** - TMDB metadata and file organization
- **[WebSocket Protocol](architecture/websocket-protocol.md)** - Real-time updates and message types

### Integration

- **[Sonarr/Radarr](integration/sonarr-radarr.md)** - Integration with \*arr services
- **[TMDB](integration/tmdb.md)** - TMDB metadata integration

### Development

- **[API Reference](development/api-reference.md)** - Complete REST API and WebSocket documentation
- **[Testing](development/testing.md)** - Test scenarios and validation
- **[Troubleshooting](development/troubleshooting.md)** - Common issues and solutions

## 🗂️ Archive

Historical documentation and completed work summaries are archived in `.archive/legacy-docs/` for reference.

## 📝 Contributing

When adding new documentation:

1. Place files in the appropriate category directory
2. Use lowercase with hyphens for filenames (e.g., `my-guide.md`)
3. Update this README with a link to your new doc
4. Keep documentation concise and focused

## 🔗 External Resources

- [GitHub Repository](https://github.com/yourusername/flasharr)
- [Issue Tracker](https://github.com/yourusername/flasharr/issues)
