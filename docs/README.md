# Flasharr Documentation

Flasharr is a modern download manager with intelligent media parsing, Fshare integration, and Arr services support.

## Quick Links

- [Architecture](ARCHITECTURE.md) - System design and component overview
- [API Reference](API.md) - REST API endpoints documentation
- [Development](DEVELOPMENT.md) - Setup and development guide

## Features

### Core

- **Smart Search** - Intelligent media file discovery with TMDB integration
- **Download Manager** - Multi-threaded downloads with resume support
- **Real-time Updates** - WebSocket-based live progress tracking

### Integrations

- **Fshare** - Premium file hosting support with VIP account management
- **Sonarr/Radarr** - Automatic media library organization
- **TMDB** - Metadata enrichment and discovery

## Tech Stack

| Component | Technology              |
| --------- | ----------------------- |
| Backend   | Rust (Axum, Tokio)      |
| Frontend  | SvelteKit 5, TypeScript |
| Database  | SQLite (rusqlite)       |
| Caching   | Moka                    |

## Project Structure

```
flasharr/
├── backend/           # Rust API server
│   └── src/
│       ├── api/       # REST endpoints
│       ├── downloader/# Download engine
│       ├── hosts/     # Fshare handler
│       └── utils/     # Smart parser
├── frontend/          # SvelteKit app
│   └── src/
│       ├── lib/       # Components & stores
│       └── routes/    # Pages
└── docs/              # Documentation
```

## Getting Started

```bash
# Development
./scripts/debug/dev.sh

# Production build
docker-compose up -d
```
