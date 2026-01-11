"""
Pytest configuration and shared fixtures.
"""

import pytest
import os
from unittest.mock import patch


@pytest.fixture
def mock_env_vars():
    """Fixture to set up mock environment variables."""
    env_vars = {
        "FSHARE_EMAIL": "test@example.com",
        "FSHARE_PASSWORD": "testpassword",
        "PYLOAD_HOST": "localhost",
        "PYLOAD_PORT": "8000",
        "PYLOAD_USER": "admin",
        "PYLOAD_PASS": "admin",
        "INDEXER_PORT": "8484",
        "DOWNLOAD_DIR": "/tmp/downloads",
        "MAX_CONCURRENT_DOWNLOADS": "5",
    }
    with patch.dict(os.environ, env_vars, clear=False):
        yield env_vars


@pytest.fixture
def sample_download():
    """Fixture providing a sample download data structure."""
    return {
        "id": "test-123",
        "url": "https://www.fshare.vn/file/ABC123XYZ",
        "filename": "Movie.2024.1080p.BluRay.x264.mkv",
        "size_bytes": 4831838208,  # ~4.5 GB
        "downloaded_bytes": 2415919104,  # ~2.25 GB (50%)
        "status": "running",
        "speed_bytes": 10485760,  # 10 MB/s
        "eta_seconds": 230,
    }


@pytest.fixture
def sample_search_result():
    """Fixture providing a sample search result."""
    return {
        "title": "Movie Title 2024",
        "filename": "Movie.Title.2024.1080p.BluRay.x264-GROUP.mkv",
        "size_bytes": 4831838208,
        "url": "https://www.fshare.vn/file/ABC123XYZ",
        "quality": "1080p",
        "source": "BluRay",
        "codec": "x264",
    }
