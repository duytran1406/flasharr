"""
Unit tests for the TimFshare client.
"""

import pytest
from unittest.mock import Mock, patch

from src.flasharr.clients.timfshare import (
    TimFshareClient,
    SearchResult,
    ScoringConfig,
)
from src.flasharr.core.exceptions import APIError


class TestSearchResult:
    """Tests for SearchResult dataclass."""
    
    def test_from_api_response(self):
        item = {
            "name": "Movie.2024.1080p.mkv",
            "url": "https://www.fshare.vn/file/ABC123",
            "size": 4831838208,
        }
        
        result = SearchResult.from_api_response(item, score=50)
        
        assert result.name == "Movie.2024.1080p.mkv"
        assert result.url == "https://www.fshare.vn/file/ABC123"
        assert result.size == 4831838208
        assert result.fcode == "ABC123"
        assert result.score == 50
    
    def test_to_dict(self):
        result = SearchResult(
            name="Test.mkv",
            url="https://www.fshare.vn/file/XYZ",
            size=1024,
            fcode="XYZ",
            score=30,
        )
        
        d = result.to_dict()
        
        assert d["name"] == "Test.mkv"
        assert d["score"] == 30


class TestScoringConfig:
    """Tests for ScoringConfig dataclass."""
    
    def test_default_values(self):
        config = ScoringConfig()
        
        assert config.keyword_match_points == 10
        assert config.year_match_points == 20
        assert config.quality_bonus_points == 10
        assert config.vietnamese_bonus_points == 15
        assert "1080p" in config.quality_markers
        assert "vietsub" in config.vietnamese_markers


class TestTimFshareClient:
    """Tests for TimFshareClient."""
    
    @pytest.fixture
    def client(self):
        return TimFshareClient()
    
    def test_init_default(self, client):
        assert client.timeout == TimFshareClient.DEFAULT_TIMEOUT
        assert isinstance(client.scoring, ScoringConfig)
    
    def test_init_custom_scoring(self):
        custom_scoring = ScoringConfig(keyword_match_points=20)
        client = TimFshareClient(scoring_config=custom_scoring)
        
        assert client.scoring.keyword_match_points == 20
    
    def test_score_results_keyword_matching(self, client):
        results = [
            {"name": "Movie Title 2024 1080p.mkv", "url": "", "size": 0},
            {"name": "Other Film.mkv", "url": "", "size": 0},
        ]
        
        scored = client._score_results(results, "movie title 2024")
        
        # First result should have higher score due to more keyword matches
        assert scored[0]["score"] > scored[1]["score"]
    
    def test_score_results_year_bonus(self, client):
        results = [
            {"name": "Movie 2024 1080p.mkv", "url": "", "size": 0},
            {"name": "Movie 1080p.mkv", "url": "", "size": 0},
        ]
        
        scored = client._score_results(results, "movie 2024")
        
        # First result should have year bonus
        assert scored[0]["score"] > scored[1]["score"]
    
    def test_score_results_quality_bonus(self, client):
        results = [
            {"name": "Movie 1080p.mkv", "url": "", "size": 0},
            {"name": "Movie.mkv", "url": "", "size": 0},
        ]
        
        scored = client._score_results(results, "movie")
        
        # First result should have quality bonus
        assert scored[0]["score"] > scored[1]["score"]
    
    def test_score_results_vietnamese_bonus(self, client):
        results = [
            {"name": "Movie VietSub.mkv", "url": "", "size": 0},
            {"name": "Movie.mkv", "url": "", "size": 0},
        ]
        
        scored = client._score_results(results, "movie")
        
        # First result should have Vietnamese bonus
        assert scored[0]["score"] > scored[1]["score"]
    
    @patch("requests.Session.post")
    def test_execute_search_success(self, mock_post, client):
        mock_response = Mock()
        mock_response.status_code = 200
        mock_response.json.return_value = {
            "data": [
                {"name": "Result1.mkv", "url": "url1", "size": 100},
                {"name": "Result2.mkv", "url": "url2", "size": 200},
            ]
        }
        mock_post.return_value = mock_response
        
        results = client._execute_search("test query")
        
        assert len(results) == 2
        assert results[0]["name"] == "Result1.mkv"
    
    @patch("requests.Session.post")
    def test_execute_search_failure(self, mock_post, client):
        mock_response = Mock()
        mock_response.status_code = 500
        mock_response.text = "Server Error"
        mock_post.return_value = mock_response
        
        with pytest.raises(APIError):
            client._execute_search("test query")
    
    @patch("requests.Session.post")
    def test_search_with_extension_filter(self, mock_post, client):
        mock_response = Mock()
        mock_response.status_code = 200
        mock_response.json.return_value = {
            "data": [
                {"name": "Movie.mkv", "url": "https://www.fshare.vn/file/A", "size": 100},
                {"name": "Movie.avi", "url": "https://www.fshare.vn/file/B", "size": 100},
                {"name": "Movie.mp4", "url": "https://www.fshare.vn/file/C", "size": 100},
            ]
        }
        mock_post.return_value = mock_response
        
        results = client.search("movie", extensions=(".mkv", ".mp4"))
        
        assert len(results) == 2
        names = [r.name for r in results]
        assert "Movie.mkv" in names
        assert "Movie.mp4" in names
        assert "Movie.avi" not in names
    
    @patch("requests.Session.get")
    def test_autocomplete(self, mock_get, client):
        mock_response = Mock()
        mock_response.status_code = 200
        mock_response.json.return_value = {
            "data": [
                {"value": "suggestion 1"},
                {"value": "suggestion 2"},
            ]
        }
        mock_get.return_value = mock_response
        
        suggestions = client.autocomplete("test")
        
        assert len(suggestions) == 2
        assert "suggestion 1" in suggestions
