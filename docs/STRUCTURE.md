# Fshare-Arr Bridge - Code Structure

## Directory Structure

```
fshare-arr-bridge/
├── src/
│   └── fshare_bridge/
│       ├── __init__.py              # Package root (version info)
│       ├── __main__.py              # CLI entry point
│       │
│       ├── core/                    # Core infrastructure
│       │   ├── __init__.py
│       │   ├── config.py            # Dataclass-based configuration
│       │   └── exceptions.py        # Custom exception hierarchy
│       │
│       ├── clients/                 # External API clients
│       │   ├── __init__.py
│       │   ├── fshare.py            # Fshare.vn API client
│       │   └── timfshare.py         # TimFshare.com search client
│       │
│       ├── services/                # Business logic services
│       │   ├── __init__.py
│       │   ├── indexer.py           # Torznab/Newznab implementation
│       │   └── sabnzbd.py           # SABnzbd API emulation
│       │
│       ├── downloader/              # Native download engine
│       │   ├── __init__.py
│       │   ├── engine.py            # Async download engine
│       │   ├── queue.py             # SQLite queue manager
│       │   └── fshare_handler.py    # Fshare URL handler
│       │
│       ├── utils/                   # Utility functions
│       │   ├── __init__.py
│       │   ├── filename_parser.py   # Media filename parser
│       │   └── formatters.py        # Size/speed/time formatters
│       │
│       └── web/                     # Flask web application
│           ├── __init__.py
│           ├── app.py               # Flask app factory
│           ├── api.py               # REST API endpoints
│           ├── routes.py            # Page routes
│           ├── indexer_routes.py    # Torznab API routes
│           └── sabnzbd_routes.py    # SABnzbd API routes
│
├── app/                             # Legacy app (templates/static)
│   ├── templates/                   # Jinja2 templates
│   │   ├── base.html
│   │   ├── index.html
│   │   ├── downloads.html
│   │   ├── search.html
│   │   ├── settings.html
│   │   └── tutorial.html
│   └── static/
│       ├── css/
│       └── js/
│           └── app.js               # Frontend JavaScript
│
├── tests/                           # Test suite
│   ├── conftest.py                  # Pytest fixtures
│   └── unit/
│       ├── test_config.py
│       ├── test_formatters.py
│       ├── test_fshare_client.py
│       └── test_timfshare_client.py
│
├── docs/                            # Documentation
│   ├── ARCHITECTURE.md              # High-level architecture
│   └── STRUCTURE.md                 # This file
│
├── pyproject.toml                   # Modern Python packaging
├── requirements.txt                 # Production dependencies
├── requirements-dev.txt             # Development dependencies
├── Dockerfile                       # Container definition
├── docker-compose.yml               # Container orchestration
├── README.md                        # Project documentation
└── VERSION                          # Version file
```

## Module Details

### Core (`src/fshare_bridge/core/`)

#### `config.py`
```python
@dataclass
class FshareConfig:     # Fshare credentials
class PyLoadConfig:     # Legacy PyLoad config (transition)
class ServerConfig:     # Web server settings
class DownloadConfig:   # Download engine settings
class AppConfig:        # Container for all configs

def get_config() -> AppConfig      # Singleton accessor
def reload_config() -> AppConfig   # Force reload
```

#### `exceptions.py`
```python
class FshareBridgeError(Exception)    # Base exception
class ClientError                      # ├── AuthenticationError
                                       # ├── APIError
                                       # └── ConnectionError
class DownloadError                    # ├── DownloadNotFoundError
                                       # ├── DownloadFailedError
                                       # └── InvalidURLError
class IndexerError                     # ├── SearchError
                                       # └── ParseError
class ConfigurationError               # Configuration issues
```

---

### Clients (`src/fshare_bridge/clients/`)

#### `fshare.py`
```python
@dataclass
class FshareFile:
    name: str
    url: str
    size: int
    fcode: str

class FshareClient:
    def login() -> bool
    def search(query, limit) -> List[FshareFile]
    def get_download_link(fcode) -> Optional[str]
    def get_file_info(url) -> Optional[FshareFile]
```

#### `timfshare.py`
```python
@dataclass
class SearchResult:
    name: str
    url: str
    size: int
    score: int

@dataclass
class ScoringConfig:
    keyword_match_points: int
    year_match_points: int
    quality_bonus_points: int
    vietnamese_bonus_points: int

class TimFshareClient:
    def search(query, limit, extensions) -> List[SearchResult]
    def autocomplete(query) -> List[str]
```

---

### Services (`src/fshare_bridge/services/`)

#### `indexer.py`
```python
@dataclass
class IndexerConfig:
    title: str
    base_url: str
    video_extensions: tuple

@dataclass
class TorznabResponse:
    xml: str
    items_count: int

class IndexerService:
    def get_capabilities() -> TorznabResponse
    def search(query, season, episode) -> TorznabResponse
    def get_nzb(guid) -> Optional[str]
```

#### `sabnzbd.py`
```python
class DownloadStatus(Enum):
    QUEUED, DOWNLOADING, PAUSED, COMPLETED, FAILED

@dataclass
class QueueItem:
    nzo_id: str
    filename: str
    status: DownloadStatus
    progress: float

class SABnzbdEmulator:
    def add_file(nzb_data, filename, category) -> Optional[str]
    def add_url(url, category) -> Optional[str]
    def get_queue() -> Dict
    def get_history(limit) -> Dict
    def pause_queue() -> bool
    def resume_queue() -> bool
```

---

### Downloader (`src/fshare_bridge/downloader/`)

#### `engine.py`
```python
class DownloadState(Enum):
    QUEUED, STARTING, DOWNLOADING, PAUSED, COMPLETED, FAILED

@dataclass
class DownloadProgress:
    downloaded_bytes: int
    total_bytes: int
    speed_bytes_per_sec: float
    percentage: float

@dataclass
class DownloadTask:
    id: str
    url: str
    filename: str
    destination: Path
    state: DownloadState
    progress: DownloadProgress

class DownloadEngine:
    async def start()
    async def stop()
    async def add_download(url, filename, destination) -> DownloadTask
    def pause_task(task_id) -> bool
    def resume_task(task_id) -> bool
    def cancel_task(task_id) -> bool
```

#### `queue.py`
```python
class DownloadQueue:  # SQLite-backed
    def add_task(task) -> bool
    def update_task(task) -> bool
    def get_task(task_id) -> Optional[Dict]
    def get_pending_tasks(limit) -> List[Dict]
    def get_active_tasks() -> List[Dict]
    def get_history(limit) -> List[Dict]
    def get_statistics() -> Dict
```

---

### Utils (`src/fshare_bridge/utils/`)

#### `filename_parser.py`
```python
@dataclass
class ParsedFilename:
    original_filename: str
    normalized_filename: str
    title: str
    season: Optional[int]
    episode: Optional[int]
    year: Optional[int]
    is_series: bool

class FilenameParser:
    def parse(filename) -> ParsedFilename
```

#### `formatters.py`
```python
def format_size(bytes) -> str       # "1.5 GB"
def format_speed(bps) -> str        # "10.5 MB/s"
def format_duration(seconds) -> str # "1h 23m 45s"
def format_eta(seconds) -> str      # "01:23:45"
```

---

### Web (`src/fshare_bridge/web/`)

#### API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/stats` | GET | System statistics |
| `/api/downloads` | GET | List all downloads |
| `/api/downloads` | POST | Add new download |
| `/api/downloads/<id>` | DELETE | Remove download |
| `/api/downloads/<id>/pause` | POST | Pause download |
| `/api/downloads/<id>/resume` | POST | Resume download |
| `/api/config` | GET | Get configuration |
| `/api/version` | GET | Get version |
| `/indexer/api?t=caps` | GET | Indexer capabilities |
| `/indexer/api?t=search` | GET | Search files |
| `/indexer/nzb/<guid>` | GET | Get NZB file |
| `/sabnzbd/api?mode=*` | GET/POST | SABnzbd API |
