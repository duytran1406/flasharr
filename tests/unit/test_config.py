"""
Unit tests for the configuration module.
"""

import pytest
import os
from unittest.mock import patch

from src.fshare_bridge.core.config import (
    FshareConfig,
    PyLoadConfig,
    ServerConfig,
    DownloadConfig,
    AppConfig,
    get_config,
    reload_config,
)


class TestFshareConfig:
    """Tests for FshareConfig."""
    
    def test_default_values(self):
        config = FshareConfig()
        assert config.email == ""
        assert config.password == ""
        assert config.app_key == "L2S7R6ZMagggC5wWkQhX2+aDi467PPuftWUMRoK"
    
    def test_from_env(self, mock_env_vars):
        config = FshareConfig.from_env()
        assert config.email == "test@example.com"
        assert config.password == "testpassword"


class TestPyLoadConfig:
    """Tests for PyLoadConfig."""
    
    def test_default_values(self):
        config = PyLoadConfig()
        assert config.host == "localhost"
        assert config.port == 8000
        assert config.username == "admin"
        assert config.password == "admin"
    
    def test_from_env(self, mock_env_vars):
        config = PyLoadConfig.from_env()
        assert config.host == "localhost"
        assert config.port == 8000


class TestServerConfig:
    """Tests for ServerConfig."""
    
    def test_default_values(self):
        config = ServerConfig()
        assert config.host == "0.0.0.0"
        assert config.port == 8484
        assert config.debug is False
    
    def test_from_env(self, mock_env_vars):
        config = ServerConfig.from_env()
        assert config.port == 8484


class TestDownloadConfig:
    """Tests for DownloadConfig."""
    
    def test_default_values(self):
        config = DownloadConfig()
        assert config.download_dir == "/downloads"
        assert config.max_concurrent == 3
        assert config.chunk_size == 8192
        assert config.retry_attempts == 3
        assert config.retry_delay == 5.0
    
    def test_from_env(self, mock_env_vars):
        config = DownloadConfig.from_env()
        assert config.download_dir == "/tmp/downloads"
        assert config.max_concurrent == 5


class TestAppConfig:
    """Tests for AppConfig."""
    
    def test_default_values(self):
        config = AppConfig()
        assert isinstance(config.fshare, FshareConfig)
        assert isinstance(config.pyload, PyLoadConfig)
        assert isinstance(config.server, ServerConfig)
        assert isinstance(config.download, DownloadConfig)
    
    def test_from_env(self, mock_env_vars):
        config = AppConfig.from_env()
        assert config.fshare.email == "test@example.com"
        assert config.download.max_concurrent == 5


class TestGetConfig:
    """Tests for get_config and reload_config functions."""
    
    def test_get_config_returns_instance(self, mock_env_vars):
        # Force reload to clear any cached config
        reload_config()
        config = get_config()
        assert isinstance(config, AppConfig)
    
    def test_get_config_singleton(self, mock_env_vars):
        reload_config()
        config1 = get_config()
        config2 = get_config()
        assert config1 is config2
    
    def test_reload_config(self, mock_env_vars):
        config1 = get_config()
        config2 = reload_config()
        # After reload, should be a new instance
        assert config2 is not config1
