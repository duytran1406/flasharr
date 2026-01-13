"""
Settings Store

SQLite-based persistent settings storage.
"""

import sqlite3
import json
import logging
from pathlib import Path
from typing import Any, Dict, Optional, TypeVar, Generic
from contextlib import contextmanager
from dataclasses import dataclass, asdict

logger = logging.getLogger(__name__)

T = TypeVar("T")


@dataclass
class AppSettings:
    """Application settings with defaults."""
    
    # Account
    fshare_email: str = ""
    fshare_password: str = ""
    
    # Downloads
    download_path: str = "/downloads"
    category_paths: Dict[str, str] = None
    max_concurrent_downloads: int = 3
    speed_limit_mbps: int = 0  # 0 = unlimited
    auto_resume: bool = True
    
    # Integration
    indexer_api_key: str = ""
    sabnzbd_api_key: str = ""
    base_url: str = "http://localhost:8484"
    enable_indexer: bool = True
    enable_sabnzbd: bool = True
    
    # Appearance
    theme: str = "dark"  # dark, light, system
    language: str = "en"  # en, vi
    refresh_interval: int = 3000  # ms
    
    # Advanced
    debug_mode: bool = False
    
    def __post_init__(self):
        if self.category_paths is None:
            self.category_paths = {
                "radarr": "movies",
                "sonarr": "tv",
                "lidarr": "music",
            }
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return asdict(self)
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "AppSettings":
        """Create from dictionary."""
        return cls(**{k: v for k, v in data.items() if hasattr(cls, k)})


class SettingsStore:
    """
    SQLite-backed settings persistence.
    
    Example:
        >>> store = SettingsStore("/app/data/settings.db")
        >>> store.set("theme", "dark")
        >>> theme = store.get("theme", "light")
    """
    
    SCHEMA = """
    CREATE TABLE IF NOT EXISTS settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    """
    
    def __init__(self, db_path: str = "/app/data/settings.db"):
        """
        Initialize settings store.
        
        Args:
            db_path: Path to SQLite database
        """
        self.db_path = Path(db_path)
        self.db_path.parent.mkdir(parents=True, exist_ok=True)
        self._init_db()
        self._cache: Dict[str, str] = {}
        self._load_cache()
    
    def _init_db(self) -> None:
        """Initialize database schema."""
        with self._get_connection() as conn:
            conn.executescript(self.SCHEMA)
    
    @contextmanager
    def _get_connection(self):
        """Get database connection."""
        conn = sqlite3.connect(str(self.db_path))
        try:
            yield conn
            conn.commit()
        finally:
            conn.close()
    
    def _load_cache(self) -> None:
        """Load all settings into cache."""
        with self._get_connection() as conn:
            rows = conn.execute("SELECT key, value FROM settings").fetchall()
            self._cache = {row[0]: row[1] for row in rows}
    
    def get(self, key: str, default: Any = None) -> Any:
        """
        Get a setting value.
        
        Args:
            key: Setting key
            default: Default value if not found
            
        Returns:
            Setting value or default
        """
        value = self._cache.get(key)
        if value is None:
            return default
        
        # Try to deserialize JSON
        try:
            return json.loads(value)
        except (json.JSONDecodeError, TypeError):
            return value
    
    def set(self, key: str, value: Any) -> None:
        """
        Set a setting value.
        
        Args:
            key: Setting key
            value: Value to store
        """
        # Serialize to JSON for complex types
        if isinstance(value, (dict, list, bool)):
            stored_value = json.dumps(value)
        else:
            stored_value = str(value)
        
        with self._get_connection() as conn:
            conn.execute(
                """
                INSERT OR REPLACE INTO settings (key, value, updated_at)
                VALUES (?, ?, CURRENT_TIMESTAMP)
                """,
                (key, stored_value),
            )
        
        self._cache[key] = stored_value
    
    def delete(self, key: str) -> bool:
        """Delete a setting."""
        with self._get_connection() as conn:
            cursor = conn.execute("DELETE FROM settings WHERE key = ?", (key,))
            if key in self._cache:
                del self._cache[key]
            return cursor.rowcount > 0
    
    def get_all(self) -> Dict[str, Any]:
        """Get all settings as dictionary."""
        result = {}
        for key, value in self._cache.items():
            try:
                result[key] = json.loads(value)
            except (json.JSONDecodeError, TypeError):
                result[key] = value
        return result
    
    def set_many(self, settings: Dict[str, Any]) -> None:
        """Set multiple settings at once."""
        for key, value in settings.items():
            self.set(key, value)
    
    def get_app_settings(self) -> AppSettings:
        """Get all settings as AppSettings dataclass."""
        stored = self.get_all()
        defaults = AppSettings()
        
        # Merge stored values with defaults
        merged = defaults.to_dict()
        merged.update(stored)
        
        return AppSettings.from_dict(merged)
    
    def save_app_settings(self, settings: AppSettings) -> None:
        """Save AppSettings to store."""
        self.set_many(settings.to_dict())
    
    def export_json(self) -> str:
        """Export all settings as JSON string."""
        return json.dumps(self.get_all(), indent=2)
    
    def import_json(self, json_str: str) -> bool:
        """
        Import settings from JSON string.
        
        Args:
            json_str: JSON string with settings
            
        Returns:
            True if successful
        """
        try:
            data = json.loads(json_str)
            if not isinstance(data, dict):
                return False
            self.set_many(data)
            return True
        except json.JSONDecodeError:
            return False


# Singleton instance
_store: Optional[SettingsStore] = None


def get_settings_store() -> SettingsStore:
    """Get or create settings store singleton."""
    global _store
    if _store is None:
        _store = SettingsStore()
    return _store
