"""
Arr Clients

Implements clients for interacting with Sonarr and Radarr APIs.
"""

import logging
import requests
from typing import List, Dict, Optional, Any, Union
from urllib.parse import urljoin

logger = logging.getLogger(__name__)


class ArrClient:
    """Base client for *Arr suite applications."""
    
    def __init__(self, url: str, api_key: str):
        self.url = url.rstrip("/")
        self.api_key = api_key
        self.session = requests.Session()
        self.session.headers.update({"X-Api-Key": api_key})
        
    def _get(self, endpoint: str, params: Optional[Dict] = None) -> Any:
        try:
            url = f"{self.url}/api/v3/{endpoint.lstrip('/')}"
            response = self.session.get(url, params=params, timeout=10)
            response.raise_for_status()
            return response.json()
        except requests.exceptions.RequestException as e:
            logger.error(f"Error communicating with Arr API ({self.url}): {e}")
            return None

    def _post(self, endpoint: str, data: Dict) -> Any:
        try:
            url = f"{self.url}/api/v3/{endpoint.lstrip('/')}"
            response = self.session.post(url, json=data, timeout=10)
            response.raise_for_status()
            return response.json()
        except requests.exceptions.RequestException as e:
            logger.error(f"Error posting to Arr API ({self.url}): {e}")
            return None

    def get_system_status(self) -> bool:
        """Check if service is reachable."""
        try:
            # lightweight check usually 'system/status' or just root
            return self._get("system/status") is not None
        except:
            return False

    def get_root_folders(self) -> List[Dict]:
        """Get configured root folders."""
        return self._get("rootfolder") or []

    def get_quality_profiles(self) -> List[Dict]:
        """Get configured quality profiles."""
        return self._get("qualityprofile") or []
        
    def command_search(self, item_ids: List[int]) -> bool:
        """Trigger a search for specific items."""
        # Generic command - specific implementations might vary slightly but usually 'SeriesSearch' or 'MovieSearch'
        # Leaving for subclasses to implement specific commands
        pass


class SonarrClient(ArrClient):
    """Client for Sonarr V3 API."""
    
    def lookup_series(self, term: str) -> List[Dict]:
        """Lookup series by term (name or tvdb:id)."""
        return self._get("series/lookup", params={"term": term}) or []
        
    def add_series(self, 
                   tvdb_id: int, 
                   title: str, 
                   quality_profile_id: int, 
                   root_folder_path: str,
                   monitored: bool = True,
                   search_now: bool = False) -> Optional[Dict]:
        """Add a new series to Sonarr."""
        
        # First lookup to get correct metadata structure
        lookup = self.lookup_series(f"tvdb:{tvdb_id}")
        if not lookup:
            logger.error(f"Could not find series with tvdb_id {tvdb_id} in Sonarr lookup")
            return None
            
        series_data = lookup[0]
        
        payload = {
            "title": series_data.get("title", title),
            "qualityProfileId": quality_profile_id,
            "titleSlug": series_data.get("titleSlug"),
            "images": series_data.get("images", []),
            "tvdbId": tvdb_id,
            "rootFolderPath": root_folder_path,
            "monitored": monitored,
            "addOptions": {
                "searchForMissingEpisodes": search_now
            },
            "seasons": series_data.get("seasons", [])
        }
        
        return self._post("series", payload)

    def trigger_search(self, series_id: int) -> bool:
        """Trigger search for a series."""
        payload = {
            "name": "SeriesSearch",
            "seriesIds": [series_id]
        }
        return self._post("command", payload) is not None


class RadarrClient(ArrClient):
    """Client for Radarr V3 API."""
    
    def lookup_movie(self, term: str) -> List[Dict]:
        """Lookup movie by term (name or tmdb:id)."""
        return self._get("movie/lookup", params={"term": term}) or []
        
    def add_movie(self, 
                  tmdb_id: int, 
                  title: str, 
                  quality_profile_id: int, 
                  root_folder_path: str,
                  monitored: bool = True,
                  search_now: bool = False) -> Optional[Dict]:
        """Add a new movie to Radarr."""
        
        # First lookup to get correct metadata structure
        lookup = self.lookup_movie(f"tmdb:{tmdb_id}")
        if not lookup:
            logger.error(f"Could not find movie with tmdb_id {tmdb_id} in Radarr lookup")
            return None
            
        movie_data = lookup[0]
        
        payload = {
            "title": movie_data.get("title", title),
            "qualityProfileId": quality_profile_id,
            "titleSlug": movie_data.get("titleSlug"),
            "images": movie_data.get("images", []),
            "tmdbId": tmdb_id,
            "rootFolderPath": root_folder_path,
            "monitored": monitored,
            "addOptions": {
                "searchForMovie": search_now
            },
            "year": movie_data.get("year")
        }
        
        return self._post("movie", payload)
        
    def trigger_search(self, movie_ids: List[int]) -> bool:
        """Trigger search for movies."""
        payload = {
            "name": "MoviesSearch",
            "movieIds": movie_ids
        }
        return self._post("command", payload) is not None
