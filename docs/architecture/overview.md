# Flasharr - Architecture Document

## High-Level Architecture

```mermaid
graph TB
    subgraph "External Services"
        PROWLARR[Prowlarr]
        RADARR[Radarr]
        SONARR[Sonarr]
        FSHARE[Fshare.vn API]
        TIMFSHARE[TimFshare.com API]
    end
    
    subgraph "Flasharr"
        subgraph "Web Layer"
            FLASK[Flask App]
            API[REST API]
            INDEXER_EP[Indexer Endpoint<br>/indexer/api]
            SAB_EP[SABnzbd Endpoint<br>/sabnzbd/api]
        end
        
        subgraph "Service Layer"
            INDEXER_SVC[IndexerService]
            SAB_SVC[SABnzbdEmulator]
        end
        
        subgraph "Client Layer"
            FSHARE_CLIENT[FshareClient]
            TIMFSHARE_CLIENT[TimFshareClient]
        end
        
        subgraph "Download Layer"
            ENGINE[DownloadEngine]
            QUEUE[DownloadQueue<br>SQLite]
            HANDLER[FshareDownloadHandler]
        end
        
        subgraph "Core"
            CONFIG[AppConfig]
            EXCEPTIONS[Custom Exceptions]
        end
        
        subgraph "Utils"
            PARSER[FilenameParser]
            FORMATTERS[Formatters]
        end
    end
    
    subgraph "Storage"
        DOWNLOADS[(Download Files)]
        DB[(SQLite DB)]
    end
    
    PROWLARR -->|Torznab API| INDEXER_EP
    RADARR -->|SABnzbd API| SAB_EP
    SONARR -->|SABnzbd API| SAB_EP
    
    INDEXER_EP --> INDEXER_SVC
    SAB_EP --> SAB_SVC
    API --> ENGINE
    
    INDEXER_SVC --> TIMFSHARE_CLIENT
    INDEXER_SVC --> PARSER
    SAB_SVC --> FSHARE_CLIENT
    SAB_SVC --> ENGINE
    
    TIMFSHARE_CLIENT --> TIMFSHARE
    FSHARE_CLIENT --> FSHARE
    
    ENGINE --> QUEUE
    ENGINE --> HANDLER
    ENGINE --> DOWNLOADS
    QUEUE --> DB
    HANDLER --> FSHARE_CLIENT
```

## Data Flow

### Search Flow (Prowlarr → Indexer)
1. Prowlarr sends Torznab search request to `/indexer/api?t=search&q=...`
2. `IndexerService` receives request and builds search query
3. `TimFshareClient` queries TimFshare.com API
4. Results are scored, filtered, and normalized using `FilenameParser`
5. XML response returned in Torznab format

### Download Flow (*arr → SABnzbd → Engine)
1. Radarr/Sonarr sends download request to `/sabnzbd/api?mode=addurl&name=...`
2. `SABnzbdEmulator` receives Fshare URL
3. `FshareClient` resolves direct download link
4. `DownloadEngine` queues the download
5. Async download worker processes the queue
6. Progress updates stored in SQLite via `DownloadQueue`

## Component Responsibilities

| Component | Responsibility |
|-----------|----------------|
| `FshareClient` | Fshare authentication, file info, direct link resolution |
| `TimFshareClient` | Search TimFshare.com with relevance scoring |
| `IndexerService` | Torznab/Newznab API implementation |
| `SABnzbdEmulator` | SABnzbd API compatibility layer |
| `DownloadEngine` | Async concurrent download management |
| `DownloadQueue` | SQLite-based task persistence |
| `FshareDownloadHandler` | Fshare URL validation and resolution |
| `FilenameParser` | Media filename normalization for *arr |
| `AppConfig` | Centralized configuration management |

## Technology Stack

- **Backend**: Python 3.9+, Flask, aiohttp, SQLite
- **API Standards**: Torznab, Newznab, SABnzbd
- **Container**: Docker (single unified image)
- **Storage**: SQLite for queue, filesystem for downloads
