# System Architecture

## Overview

Flasharr uses a client-server architecture with real-time WebSocket communication.

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   SvelteKit     │────▶│   Axum Server   │────▶│     SQLite      │
│   Frontend      │◀────│   (REST + WS)   │◀────│    Database     │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                               │
                        ┌──────┴──────┐
                        ▼             ▼
                  ┌──────────┐  ┌──────────┐
                  │  Fshare  │  │   TMDB   │
                  │   API    │  │   API    │
                  └──────────┘  └──────────┘
```

## Backend Components

### API Layer (`/src/api/`)

- `downloads.rs` - Download task CRUD operations
- `search.rs` - Smart search with TMDB integration
- `tmdb.rs` - TMDB proxy for discover/details
- `settings.rs` - Configuration management

### Download Engine (`/src/downloader/`)

- `orchestrator.rs` - Task queue management
- `engine_simple.rs` - Single-stream download with resume
- `task.rs` - Download task state machine

### Host Handlers (`/src/hosts/`)

- `fshare.rs` - Fshare VIP link resolution
- `registry.rs` - Host handler registry

### Utilities (`/src/utils/`)

- `parser.rs` - Smart media filename parser
- `smart_tokenizer.rs` - Vietnamese/English tokenization
- `title_matcher.rs` - Fuzzy title matching

## Frontend Components

### Stores (`/lib/stores/`)

- `downloads.ts` - Download state management
- `websocket.ts` - Real-time updates
- `settings.ts` - App configuration

### Key Components (`/lib/components/`)

- `SmartSearchModal` - Media search interface
- `MediaCard` - Poster/banner display
- `SpeedGraph` - Real-time speed visualization

## Data Flow

### Download Flow

1. User initiates download via Smart Search
2. Backend resolves Fshare link
3. Download task created in SQLite
4. Orchestrator assigns to download engine
5. Progress broadcast via WebSocket
6. Frontend updates in real-time

### Search Flow

1. User enters query in Smart Search
2. Backend parses filename for metadata
3. TMDB lookup for matching content
4. Results cached and returned
5. User selects file for download
