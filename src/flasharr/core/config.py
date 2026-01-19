"""
Centralized Configuration Module

Uses Pydantic BaseSettings for type-safe configuration with environment variable support.
"""

import os
from typing import Optional
from dataclasses import dataclass, field


@dataclass
class FshareConfig:
    """Fshare account configuration."""
    email: str = ""
    password: str = ""
    
    @classmethod
    def from_env(cls) -> "FshareConfig":
        return cls(
            email=os.getenv("FSHARE_EMAIL", ""),
            password=os.getenv("FSHARE_PASSWORD", ""),
        )


@dataclass
class PyLoadConfig:
    """PyLoad connection configuration (legacy, for transition period)."""
    host: str = "localhost"
    port: int = 8000
    username: str = "admin"
    password: str = "admin"
    
    @classmethod
    def from_env(cls) -> "PyLoadConfig":
        return cls(
            host=os.getenv("PYLOAD_HOST", "fshare-pyload"),
            port=int(os.getenv("PYLOAD_PORT", "8000")),
            username=os.getenv("PYLOAD_USER", "admin"),
            password=os.getenv("PYLOAD_PASS", "admin"),
        )


@dataclass
class ServerConfig:
    """Web server configuration."""
    host: str = "0.0.0.0"
    port: int = 8484
    debug: bool = False
    api_key: Optional[str] = None
    
    @classmethod
    def from_env(cls) -> "ServerConfig":
        return cls(
            host=os.getenv("SERVER_HOST", "0.0.0.0"),
            port=int(os.getenv("INDEXER_PORT", "8484")),
            debug=os.getenv("DEBUG", "false").lower() == "true",
            api_key=os.getenv("API_KEY"),
        )


@dataclass
class TMDBConfig:
    """TMDB API Configuration."""
    api_key: str = ""
    
    @classmethod
    def from_env(cls) -> "TMDBConfig":
        return cls(
            api_key=os.getenv("TMDB_API_KEY", ""),
        )


@dataclass
class SonarrConfig:
    """Sonarr Connection Configuration."""
    url: str = "http://localhost:8989"
    api_key: str = ""
    
    @classmethod
    def from_env(cls) -> "SonarrConfig":
        return cls(
            url=os.getenv("SONARR_URL", "http://localhost:8989"),
            api_key=os.getenv("SONARR_API_KEY", ""),
        )


@dataclass
class RadarrConfig:
    """Radarr Connection Configuration."""
    url: str = "http://localhost:7878"
    api_key: str = ""
    
    @classmethod
    def from_env(cls) -> "RadarrConfig":
        return cls(
            url=os.getenv("RADARR_URL", "http://localhost:7878"),
            api_key=os.getenv("RADARR_API_KEY", ""),
        )


@dataclass
class DownloadConfig:
    """Download engine configuration."""
    download_dir: str = "/downloads"
    max_concurrent: int = 3
    worker_threads: int = 4
    chunk_size: int = 128 * 1024  # 128KB chunks for better performance
    retry_attempts: int = 3
    retry_delay: float = 5.0
    retry_backoff_multiplier: int = 60  # Seconds to multiply by retry count
    retry_max_wait: int = 600  # Maximum wait time in seconds (10 minutes)
    
    speed_limit: Optional[int] = None  # Bytes/sec, None = unlimited
    
    @classmethod
    def from_env(cls) -> "DownloadConfig":
        limit_str = os.getenv("DOWNLOAD_SPEED_LIMIT", "").strip()
        speed_limit = int(limit_str) if limit_str.isdigit() else None
        
        return cls(
            download_dir=os.getenv("DOWNLOAD_DIR", "/downloads"),
            max_concurrent=int(os.getenv("MAX_CONCURRENT_DOWNLOADS", "3")),
            worker_threads=int(os.getenv("WORKER_THREADS", "4")),
            chunk_size=int(os.getenv("CHUNK_SIZE", "1048576")),
            retry_attempts=int(os.getenv("RETRY_ATTEMPTS", "3")),
            retry_delay=float(os.getenv("RETRY_DELAY", "5.0")),
            retry_backoff_multiplier=int(os.getenv("RETRY_BACKOFF_MULTIPLIER", "60")),
            retry_max_wait=int(os.getenv("RETRY_MAX_WAIT", "600")),
            speed_limit=speed_limit,
        )


@dataclass
class AppConfig:
    """Main application configuration container."""
    fshare: FshareConfig = field(default_factory=FshareConfig)
    pyload: PyLoadConfig = field(default_factory=PyLoadConfig)
    server: ServerConfig = field(default_factory=ServerConfig)
    download: DownloadConfig = field(default_factory=DownloadConfig)
    tmdb: TMDBConfig = field(default_factory=TMDBConfig)
    sonarr: SonarrConfig = field(default_factory=SonarrConfig)
    radarr: RadarrConfig = field(default_factory=RadarrConfig)
    data_dir: str = "./data"
    
    @classmethod
    def from_env(cls) -> "AppConfig":
        """Load all configuration from environment variables."""
        return cls(
            fshare=FshareConfig.from_env(),
            pyload=PyLoadConfig.from_env(),
            server=ServerConfig.from_env(),
            download=DownloadConfig.from_env(),
            tmdb=TMDBConfig.from_env(),
            sonarr=SonarrConfig.from_env(),
            radarr=RadarrConfig.from_env(),
            data_dir=os.getenv("DATA_DIR", "./data"),
        )


# Global configuration instance (lazy-loaded)
_config: Optional[AppConfig] = None


def get_config() -> AppConfig:
    """Get the global configuration instance."""
    global _config
    if _config is None:
        _config = AppConfig.from_env()
    return _config


def reload_config() -> AppConfig:
    """Reload configuration from environment."""
    global _config
    _config = AppConfig.from_env()
    return _config
