"""
Unit Tests for Settings Store

Tests for the SQLite-backed settings persistence.
"""

import pytest
import tempfile
import os
from pathlib import Path

from src.fshare_bridge.core.settings_store import (
    SettingsStore,
    AppSettings,
    get_settings_store,
)


@pytest.fixture
def temp_db():
    """Create a temporary database file."""
    fd, path = tempfile.mkstemp(suffix=".db")
    os.close(fd)
    yield path
    if os.path.exists(path):
        os.unlink(path)


@pytest.fixture
def store(temp_db):
    """Create a settings store with temp database."""
    return SettingsStore(temp_db)


class TestSettingsStore:
    """Tests for SettingsStore class."""
    
    def test_init_creates_db(self, temp_db):
        """Test that initialization creates the database."""
        store = SettingsStore(temp_db)
        assert Path(temp_db).exists()
    
    def test_set_and_get_string(self, store):
        """Test setting and getting a string value."""
        store.set("test_key", "test_value")
        assert store.get("test_key") == "test_value"
    
    def test_set_and_get_int(self, store):
        """Test setting and getting an integer value."""
        store.set("max_downloads", 5)
        assert store.get("max_downloads") == 5
    
    def test_set_and_get_bool(self, store):
        """Test setting and getting a boolean value."""
        store.set("enabled", True)
        assert store.get("enabled") is True
        
        store.set("disabled", False)
        assert store.get("disabled") is False
    
    def test_set_and_get_dict(self, store):
        """Test setting and getting a dictionary value."""
        data = {"radarr": "movies", "sonarr": "tv"}
        store.set("category_paths", data)
        assert store.get("category_paths") == data
    
    def test_set_and_get_list(self, store):
        """Test setting and getting a list value."""
        data = ["item1", "item2", "item3"]
        store.set("items", data)
        assert store.get("items") == data
    
    def test_get_default(self, store):
        """Test getting a non-existent key returns default."""
        assert store.get("nonexistent") is None
        assert store.get("nonexistent", "default") == "default"
    
    def test_delete(self, store):
        """Test deleting a setting."""
        store.set("to_delete", "value")
        assert store.get("to_delete") == "value"
        
        result = store.delete("to_delete")
        assert result is True
        assert store.get("to_delete") is None
    
    def test_delete_nonexistent(self, store):
        """Test deleting a non-existent key returns False."""
        result = store.delete("nonexistent")
        assert result is False
    
    def test_get_all(self, store):
        """Test getting all settings."""
        store.set("key1", "value1")
        store.set("key2", 42)
        store.set("key3", True)
        
        all_settings = store.get_all()
        assert all_settings["key1"] == "value1"
        assert all_settings["key2"] == 42
        assert all_settings["key3"] is True
    
    def test_set_many(self, store):
        """Test setting multiple values at once."""
        settings = {
            "email": "test@example.com",
            "max_concurrent": 3,
            "auto_resume": True,
        }
        store.set_many(settings)
        
        assert store.get("email") == "test@example.com"
        assert store.get("max_concurrent") == 3
        assert store.get("auto_resume") is True
    
    def test_export_json(self, store):
        """Test exporting settings as JSON."""
        store.set("key1", "value1")
        store.set("key2", {"nested": True})
        
        json_str = store.export_json()
        assert "key1" in json_str
        assert "value1" in json_str
        assert "nested" in json_str
    
    def test_import_json(self, store):
        """Test importing settings from JSON."""
        json_str = '{"imported_key": "imported_value", "count": 10}'
        result = store.import_json(json_str)
        
        assert result is True
        assert store.get("imported_key") == "imported_value"
        assert store.get("count") == 10
    
    def test_import_invalid_json(self, store):
        """Test importing invalid JSON returns False."""
        result = store.import_json("not valid json")
        assert result is False
    
    def test_overwrite_value(self, store):
        """Test overwriting an existing value."""
        store.set("key", "original")
        assert store.get("key") == "original"
        
        store.set("key", "updated")
        assert store.get("key") == "updated"


class TestAppSettings:
    """Tests for AppSettings dataclass."""
    
    def test_default_values(self):
        """Test that AppSettings has sensible defaults."""
        settings = AppSettings()
        
        assert settings.fshare_email == ""
        assert settings.download_path == "/downloads"
        assert settings.max_concurrent_downloads == 3
        assert settings.theme == "dark"
        assert settings.auto_resume is True
    
    def test_category_paths_default(self):
        """Test that category_paths has defaults after post_init."""
        settings = AppSettings()
        
        assert settings.category_paths is not None
        assert settings.category_paths["radarr"] == "movies"
        assert settings.category_paths["sonarr"] == "tv"
    
    def test_to_dict(self):
        """Test converting to dictionary."""
        settings = AppSettings(fshare_email="test@example.com")
        data = settings.to_dict()
        
        assert isinstance(data, dict)
        assert data["fshare_email"] == "test@example.com"
    
    def test_from_dict(self):
        """Test creating from dictionary."""
        data = {
            "fshare_email": "test@example.com",
            "max_concurrent_downloads": 5,
            "theme": "light",
        }
        settings = AppSettings.from_dict(data)
        
        assert settings.fshare_email == "test@example.com"
        assert settings.max_concurrent_downloads == 5
        assert settings.theme == "light"
    
    def test_from_dict_ignores_unknown_keys(self):
        """Test that from_dict ignores unknown keys."""
        data = {
            "fshare_email": "test@example.com",
            "unknown_key": "should be ignored",
        }
        settings = AppSettings.from_dict(data)
        
        assert settings.fshare_email == "test@example.com"
        assert not hasattr(settings, "unknown_key")


class TestAppSettingsPersistence:
    """Tests for AppSettings integration with SettingsStore."""
    
    def test_save_and_load_app_settings(self, store):
        """Test saving and loading AppSettings."""
        original = AppSettings(
            fshare_email="user@example.com",
            download_path="/custom/path",
            max_concurrent_downloads=5,
            theme="light",
        )
        
        store.save_app_settings(original)
        loaded = store.get_app_settings()
        
        assert loaded.fshare_email == "user@example.com"
        assert loaded.download_path == "/custom/path"
        assert loaded.max_concurrent_downloads == 5
        assert loaded.theme == "light"
    
    def test_get_app_settings_with_defaults(self, store):
        """Test that get_app_settings returns defaults for missing values."""
        # Only set some values
        store.set("fshare_email", "partial@example.com")
        
        settings = store.get_app_settings()
        
        # Set value should be present
        assert settings.fshare_email == "partial@example.com"
        # Unset values should have defaults
        assert settings.download_path == "/downloads"
        assert settings.theme == "dark"
