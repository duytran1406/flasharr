"""
Unit tests for the Fshare client.
"""

import pytest
from unittest.mock import Mock, patch, MagicMock
from datetime import datetime, timedelta

from src.fshare_bridge.clients.fshare import FshareClient, FshareFile
from src.fshare_bridge.core.exceptions import AuthenticationError, APIError


class TestFshareFile:
    """Tests for FshareFile dataclass."""
    
    def test_from_api_response(self):
        item = {
            "name": "Movie.2024.mkv",
            "linkcode": "ABC123",
            "size": 1073741824,
            "type": 0,
        }
        
        file = FshareFile.from_api_response(item)
        
        assert file.name == "Movie.2024.mkv"
        assert file.url == "https://www.fshare.vn/file/ABC123"
        assert file.size == 1073741824
        assert file.fcode == "ABC123"
        assert file.file_type == 0
    
    def test_to_dict(self):
        file = FshareFile(
            name="Test.mkv",
            url="https://www.fshare.vn/file/XYZ",
            size=1024,
            fcode="XYZ",
        )
        
        d = file.to_dict()
        
        assert d["name"] == "Test.mkv"
        assert d["url"] == "https://www.fshare.vn/file/XYZ"
        assert d["size"] == 1024
        assert d["fcode"] == "XYZ"


class TestFshareClient:
    """Tests for FshareClient."""
    
    @pytest.fixture
    def client(self):
        return FshareClient(
            email="test@example.com",
            password="testpass",
        )
    
    def test_init(self, client):
        assert client.email == "test@example.com"
        assert client.password == "testpass"
        assert client.app_key == FshareClient.DEFAULT_APP_KEY
        assert client._token is None
    
    def test_is_authenticated_no_token(self, client):
        assert client.is_authenticated is False
    
    def test_is_authenticated_expired_token(self, client):
        client._token = "some_token"
        client._token_expires = datetime.now() - timedelta(hours=1)
        assert client.is_authenticated is False
    
    def test_is_authenticated_valid_token(self, client):
        client._token = "some_token"
        client._token_expires = datetime.now() + timedelta(hours=1)
        assert client.is_authenticated is True
    
    @patch("requests.Session.post")
    def test_login_success(self, mock_post, client):
        mock_response = Mock()
        mock_response.status_code = 200
        mock_response.json.return_value = {
            "code": 200,
            "token": "test_token",
            "session_id": "test_session",
        }
        mock_post.return_value = mock_response
        
        result = client.login()
        
        assert result is True
        assert client._token == "test_token"
        assert client._session_id == "test_session"
        assert client._token_expires is not None
    
    @patch("requests.Session.post")
    def test_login_failure_bad_credentials(self, mock_post, client):
        mock_response = Mock()
        mock_response.status_code = 200
        mock_response.json.return_value = {
            "code": 401,
            "msg": "Invalid credentials",
        }
        mock_post.return_value = mock_response
        
        with pytest.raises(AuthenticationError) as exc_info:
            client.login()
        
        assert "Invalid credentials" in str(exc_info.value)
    
    @patch("requests.Session.post")
    def test_login_failure_http_error(self, mock_post, client):
        mock_response = Mock()
        mock_response.status_code = 500
        mock_response.text = "Server Error"
        mock_post.return_value = mock_response
        
        with pytest.raises(APIError) as exc_info:
            client.login()
        
        assert exc_info.value.status_code == 500
    
    @patch("requests.Session.post")
    def test_search_returns_results(self, mock_post, client):
        # Setup authentication
        client._token = "test_token"
        client._token_expires = datetime.now() + timedelta(hours=1)
        
        mock_response = Mock()
        mock_response.status_code = 200
        mock_response.json.return_value = {
            "code": 200,
            "items": [
                {"name": "Movie1.mkv", "linkcode": "A1", "size": 1024, "type": 0},
                {"name": "Movie2.mkv", "linkcode": "A2", "size": 2048, "type": 0},
            ],
        }
        mock_post.return_value = mock_response
        
        results = client.search("movie")
        
        assert len(results) == 2
        assert isinstance(results[0], FshareFile)
        assert results[0].name == "Movie1.mkv"
        assert results[1].fcode == "A2"
