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
    app_key: str = "L2S7R6ZMagggC5wWkQhX2+aDi467PPuftWUMRoK"
    
    @classmethod
    def from_env(cls) -> "FshareConfig":
        return cls(
            email=os.getenv("FSHARE_EMAIL", ""),
            password=os.getenv("FSHARE_PASSWORD", ""),
            app_key=os.getenv("FSHARE_APP_KEY", cls.app_key),
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
    
    @classmethod
    def from_env(cls) -> "ServerConfig":
        return cls(
            host=os.getenv("SERVER_HOST", "0.0.0.0"),
            port=int(os.getenv("INDEXER_PORT", "8484")),
            debug=os.getenv("DEBUG", "false").lower() == "true",
        )


@dataclass
class DownloadConfig:
    """Download engine configuration."""
    download_dir: str = "/downloads"
    max_concurrent: int = 3
    chunk_size: int = 8192
    retry_attempts: int = 3
    retry_delay: float = 5.0
    
    @classmethod
    def from_env(cls) -> "DownloadConfig":
        return cls(
            download_dir=os.getenv("DOWNLOAD_DIR", "/downloads"),
            max_concurrent=int(os.getenv("MAX_CONCURRENT_DOWNLOADS", "3")),
            chunk_size=int(os.getenv("CHUNK_SIZE", "8192")),
            retry_attempts=int(os.getenv("RETRY_ATTEMPTS", "3")),
            retry_delay=float(os.getenv("RETRY_DELAY", "5.0")),
        )


@dataclass
class AppConfig:
    """Main application configuration container."""
    fshare: FshareConfig = field(default_factory=FshareConfig)
    pyload: PyLoadConfig = field(default_factory=PyLoadConfig)
    server: ServerConfig = field(default_factory=ServerConfig)
    download: DownloadConfig = field(default_factory=DownloadConfig)
    
    @classmethod
    def from_env(cls) -> "AppConfig":
        """Load all configuration from environment variables."""
        return cls(
            fshare=FshareConfig.from_env(),
            pyload=PyLoadConfig.from_env(),
            server=ServerConfig.from_env(),
            download=DownloadConfig.from_env(),
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
