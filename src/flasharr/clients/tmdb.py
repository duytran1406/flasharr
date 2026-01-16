"""
TMDB Client

Client for The Movie Database API.
"""

import logging
import requests
from typing import List, Dict, Optional, Any

logger = logging.getLogger(__name__)


class TMDBClient:
    """Client for TMDB API."""
    
    BASE_URL = "https://api.themoviedb.org/3"
    IMAGE_BASE_URL = "https://image.tmdb.org/t/p"
    
    def __init__(self, api_key: str):
        self.api_key = api_key
        self.session = requests.Session()
        self.session.params = {"api_key": self.api_key, "language": "en-US"}
        
    def _get(self, endpoint: str, params: Optional[Dict] = None) -> Any:
        try:
            url = f"{self.BASE_URL}/{endpoint.lstrip('/')}"
            response = self.session.get(url, params=params, timeout=10)
            response.raise_for_status()
            return response.json()
        except requests.exceptions.RequestException as e:
            logger.error(f"Error communicating with TMDB API: {e}")
            return None

    def search_multi(self, query: str, page: int = 1) -> Dict:
        """Search for movies and TV shows."""
        return self._get("search/multi", params={"query": query, "page": page, "include_adult": False})

    def search_movie(self, query: str, page: int = 1) -> Dict:
        """Search for movies."""
        return self._get("search/movie", params={"query": query, "page": page, "include_adult": False})
    
    def search_tv(self, query: str, page: int = 1) -> Dict:
        """Search for TV shows."""
        return self._get("search/tv", params={"query": query, "page": page, "include_adult": False})

    def get_movie_details(self, tmdb_id: int) -> Dict:
        """Get movie details."""
        return self._get(f"movie/{tmdb_id}", params={"append_to_response": "credits,videos,similar,recommendations"})

    def get_tv_details(self, tmdb_id: int) -> Dict:
        """Get TV show details."""
        return self._get(f"tv/{tmdb_id}", params={"append_to_response": "credits,videos,similar,recommendations"})
        
    def get_season_details(self, tmdb_id: int, season_number: int) -> Dict:
        """Get details for a specific season."""
        return self._get(f"tv/{tmdb_id}/season/{season_number}")
        
    def get_external_ids(self, tmdb_id: int, media_type: str) -> Dict:
        """Get external IDs (TVDB, IMDB, etc)."""
        return self._get(f"{media_type}/{tmdb_id}/external_ids")
    
    def discover_movie(self, **filters) -> Dict:
        """
        Discover movies with filters.
        
        Supported filters:
        - sort_by: popularity.desc, release_date.desc, vote_average.desc, etc.
        - page: int
        - with_genres: comma-separated genre IDs
        - primary_release_year: int
        - primary_release_date.gte: YYYY-MM-DD
        - primary_release_date.lte: YYYY-MM-DD
        - vote_average.gte: float
        - vote_average.lte: float
        - with_runtime.gte: int (minutes)
        - with_runtime.lte: int (minutes)
        """
        params = {k: v for k, v in filters.items() if v}
        return self._get("discover/movie", params=params)
    
    def discover_tv(self, **filters) -> Dict:
        """
        Discover TV shows with filters.
        
        Supported filters similar to discover_movie but for TV.
        """
        params = {k: v for k, v in filters.items() if v}
        return self._get("discover/tv", params=params)
    
    def get_trending(self, media_type: str = "all", time_window: str = "day", page: int = 1) -> Dict:
        """
        Get trending items.
        
        Args:
            media_type: 'all', 'movie', 'tv', 'person'
            time_window: 'day' or 'week'
            page: page number
        """
        return self._get(f"trending/{media_type}/{time_window}", params={"page": page})
    
    def get_popular_movies(self, page: int = 1) -> Dict:
        """Get popular movies."""
        return self._get("movie/popular", params={"page": page})
    
    def get_popular_tv(self, page: int = 1) -> Dict:
        """Get popular TV shows."""
        return self._get("tv/popular", params={"page": page})
    
    def get_movie_genres(self) -> Dict:
        """Get list of movie genres."""
        return self._get("genre/movie/list")
    
    def get_tv_genres(self) -> Dict:
        """Get list of TV genres."""
        return self._get("genre/tv/list")
    
    def get_similar_movies(self, tmdb_id: int, page: int = 1) -> Dict:
        """Get similar movies."""
        return self._get(f"movie/{tmdb_id}/similar", params={"page": page})
    
    def get_similar_tv(self, tmdb_id: int, page: int = 1) -> Dict:
        """Get similar TV shows."""
        return self._get(f"tv/{tmdb_id}/similar", params={"page": page})
    
    def get_movie_recommendations(self, tmdb_id: int, page: int = 1) -> Dict:
        """Get movie recommendations."""
        return self._get(f"movie/{tmdb_id}/recommendations", params={"page": page})
    
    def get_tv_recommendations(self, tmdb_id: int, page: int = 1) -> Dict:
        """Get TV recommendations."""
        return self._get(f"tv/{tmdb_id}/recommendations", params={"page": page})
    
    def get_poster_url(self, path: str, size: str = "w342") -> Optional[str]:
        """
        Get full poster URL.
        
        Sizes: w92, w154, w185, w342, w500, w780, original
        """
        if not path:
            return None
        return f"{self.IMAGE_BASE_URL}/{size}{path}"
    
    def get_backdrop_url(self, path: str, size: str = "w780") -> Optional[str]:
        """
        Get full backdrop URL.
        
        Sizes: w300, w780, w1280, original
        """
        if not path:
            return None
        return f"{self.IMAGE_BASE_URL}/{size}{path}"

