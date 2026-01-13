"""
Unit Tests for FshareClient PyLoad Integration

Tests for the V3 API and folder enumeration methods from pyLoad plugins.
"""

import pytest
import responses
from unittest.mock import patch, MagicMock

from src.flasharr.clients.fshare import FshareClient, FshareFile


@pytest.fixture
def authenticated_client():
    """Create an authenticated client for testing."""
    client = FshareClient(
        email="test@example.com",
        password="testpass123",
    )
    # Mock authentication state
    client._token = "mock_token"
    client._session_id = "mock_session_id"
    client._token_expires = __import__("datetime").datetime.now() + __import__("datetime").timedelta(hours=24)
    return client


class TestFshareClientV3API:
    """Tests for V3 API methods (from pyLoad FshareVn plugin)."""
    
    @responses.activate
    def test_get_file_info_v3_success(self, authenticated_client):
        """Test getting file info via V3 API."""
        responses.add(
            responses.GET,
            "https://www.fshare.vn/api/v3/files/folder",
            json={
                "current": {
                    "name": "Movie.2024.1080p.BluRay.mkv",
                    "size": 5368709120,
                    "linkcode": "ABC123XYZ",
                }
            },
            status=200,
        )
        
        result = authenticated_client.get_file_info_v3("ABC123XYZ")
        
        assert result is not None
        assert result.name == "Movie.2024.1080p.BluRay.mkv"
        assert result.size == 5368709120
        assert result.fcode == "ABC123XYZ"
    
    @responses.activate
    def test_get_file_info_v3_not_found(self, authenticated_client):
        """Test V3 API returns None for non-existent file."""
        responses.add(
            responses.GET,
            "https://www.fshare.vn/api/v3/files/folder",
            json={"status": 404},
            status=200,
        )
        
        result = authenticated_client.get_file_info_v3("NONEXISTENT")
        
        assert result is None
    
    @responses.activate
    def test_get_file_info_v3_error(self, authenticated_client):
        """Test V3 API handles HTTP errors gracefully."""
        responses.add(
            responses.GET,
            "https://www.fshare.vn/api/v3/files/folder",
            status=500,
        )
        
        result = authenticated_client.get_file_info_v3("ABC123")
        
        assert result is None


class TestFshareClientFolderEnumeration:
    """Tests for folder enumeration (from pyLoad FshareVnFolder plugin)."""
    
    @responses.activate
    def test_enumerate_folder_single_page(self, authenticated_client):
        """Test enumerating a folder with single page of results."""
        responses.add(
            responses.GET,
            "https://www.fshare.vn/api/v3/files/folder",
            json={
                "items": [
                    {"name": "file1.mkv", "size": 1000000, "linkcode": "FILE1", "type": 1},
                    {"name": "file2.mkv", "size": 2000000, "linkcode": "FILE2", "type": 1},
                ],
                "_links": {"last": "&page=1"},
            },
            status=200,
        )
        
        results = authenticated_client.enumerate_folder("FOLDER123")
        
        assert len(results) == 2
        assert results[0].name == "file1.mkv"
        assert results[0].fcode == "FILE1"
        assert results[1].name == "file2.mkv"
    
    @responses.activate
    def test_enumerate_folder_multiple_pages(self, authenticated_client):
        """Test enumerating a folder with multiple pages."""
        # Page 1
        responses.add(
            responses.GET,
            "https://www.fshare.vn/api/v3/files/folder",
            json={
                "items": [
                    {"name": "file1.mkv", "size": 1000000, "linkcode": "FILE1", "type": 1},
                ],
                "_links": {"last": "&page=2"},
            },
            status=200,
        )
        # Page 2
        responses.add(
            responses.GET,
            "https://www.fshare.vn/api/v3/files/folder",
            json={
                "items": [
                    {"name": "file2.mkv", "size": 2000000, "linkcode": "FILE2", "type": 1},
                ],
                "_links": {"last": "&page=2"},
            },
            status=200,
        )
        
        results = authenticated_client.enumerate_folder("FOLDER123")
        
        assert len(results) == 2
    
    @responses.activate
    def test_enumerate_folder_skips_subfolders_by_default(self, authenticated_client):
        """Test that subfolders are skipped by default."""
        responses.add(
            responses.GET,
            "https://www.fshare.vn/api/v3/files/folder",
            json={
                "items": [
                    {"name": "file1.mkv", "size": 1000000, "linkcode": "FILE1", "type": 1},
                    {"name": "subfolder", "size": 0, "linkcode": "SUBFOLDER", "type": 0},
                ],
                "_links": {"last": "&page=1"},
            },
            status=200,
        )
        
        results = authenticated_client.enumerate_folder("FOLDER123", include_subfolders=False)
        
        # Only the file should be included, not the folder
        assert len(results) == 1
        assert results[0].name == "file1.mkv"
    
    @responses.activate
    def test_enumerate_folder_empty(self, authenticated_client):
        """Test enumerating an empty folder."""
        responses.add(
            responses.GET,
            "https://www.fshare.vn/api/v3/files/folder",
            json={
                "items": [],
                "_links": {"last": "&page=1"},
            },
            status=200,
        )
        
        results = authenticated_client.enumerate_folder("EMPTY_FOLDER")
        
        assert len(results) == 0
    
    @responses.activate
    def test_enumerate_folder_error(self, authenticated_client):
        """Test folder enumeration handles errors gracefully."""
        responses.add(
            responses.GET,
            "https://www.fshare.vn/api/v3/files/folder",
            status=500,
        )
        
        results = authenticated_client.enumerate_folder("FOLDER123")
        
        # Should return empty list on error, not raise
        assert results == []


class TestFshareClientPremiumDownload:
    """Tests for premium download link (from pyLoad FshareVn plugin)."""
    
    @responses.activate
    def test_get_download_link_premium_success(self, authenticated_client):
        """Test getting premium download link."""
        responses.add(
            responses.POST,
            "https://api.fshare.vn/api/session/download",
            json={"location": "https://download.fshare.vn/premium/file.mkv"},
            status=200,
        )
        
        result = authenticated_client.get_download_link_premium(
            "https://www.fshare.vn/file/ABC123"
        )
        
        assert result == "https://download.fshare.vn/premium/file.mkv"
    
    @responses.activate
    def test_get_download_link_premium_with_password(self, authenticated_client):
        """Test premium download with password-protected file."""
        responses.add(
            responses.POST,
            "https://api.fshare.vn/api/session/download",
            json={"location": "https://download.fshare.vn/premium/protected.mkv"},
            status=200,
        )
        
        result = authenticated_client.get_download_link_premium(
            "https://www.fshare.vn/file/ABC123",
            password="secret123",
        )
        
        assert result == "https://download.fshare.vn/premium/protected.mkv"
    
    @responses.activate
    def test_get_download_link_premium_wrong_password(self, authenticated_client):
        """Test premium download with wrong password returns None."""
        responses.add(
            responses.POST,
            "https://api.fshare.vn/api/session/download",
            status=403,
        )
        
        result = authenticated_client.get_download_link_premium(
            "https://www.fshare.vn/file/ABC123",
            password="wrong_password",
        )
        
        assert result is None
    
    @responses.activate
    def test_get_download_link_premium_password_required(self, authenticated_client):
        """Test premium download when password is required but not provided."""
        responses.add(
            responses.POST,
            "https://api.fshare.vn/api/session/download",
            status=403,
        )
        
        result = authenticated_client.get_download_link_premium(
            "https://www.fshare.vn/file/ABC123"
        )
        
        assert result is None
    
    @responses.activate
    def test_get_download_link_premium_error(self, authenticated_client):
        """Test premium download handles server errors."""
        responses.add(
            responses.POST,
            "https://api.fshare.vn/api/session/download",
            status=500,
        )
        
        result = authenticated_client.get_download_link_premium(
            "https://www.fshare.vn/file/ABC123"
        )
        
        assert result is None


class TestFshareClientConstants:
    """Tests for pyLoad plugin constants."""
    
    def test_api_key_present(self):
        """Test that pyLoad API key is defined."""
        assert FshareClient.API_KEY == "dMnqMMZMUnN5YpvKENaEhdQQ5jxDqddt"
    
    def test_api_useragent_present(self):
        """Test that pyLoad user agent is defined."""
        assert FshareClient.API_USERAGENT == "pyLoad-B1RS5N"
    
    def test_api_v3_base_url(self):
        """Test V3 API base URL."""
        assert FshareClient.API_V3_BASE == "https://www.fshare.vn/api/v3"
    
    def test_api_fshare_base_url(self):
        """Test Fshare API base URL."""
        assert FshareClient.API_FSHARE_BASE == "https://api.fshare.vn/api"
