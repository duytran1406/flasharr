"""
Integration Service

Manages connections and operations with external *Arr applications (Sonarr, Radarr).
"""

import logging
from typing import Dict, List, Optional, Tuple

from ..core.settings_store import get_settings_store
from ..clients.arr import SonarrClient, RadarrClient

logger = logging.getLogger(__name__)


class IntegrationService:
    """Service to handle Sonarr/Radarr integrations."""
    
    def __init__(self):
        self.store = get_settings_store()
        self._sonarr: Optional[SonarrClient] = None
        self._radarr: Optional[RadarrClient] = None
        
        # Initialize clients 
        self._init_clients()
        
    def _init_clients(self):
        """Initialize clients from settings store."""
        settings = self.store.get_app_settings()
        
        # Sonarr
        if settings.sonarr and settings.sonarr.get("url") and settings.sonarr.get("api_key"):
            self._sonarr = SonarrClient(settings.sonarr["url"], settings.sonarr["api_key"])
            
        # Radarr
        if settings.radarr and settings.radarr.get("url") and settings.radarr.get("api_key"):
            self._radarr = RadarrClient(settings.radarr["url"], settings.radarr["api_key"])
            
    def reload_config(self):
        """Reload configuration and re-initialize clients."""
        logger.info("Reloading integration configuration...")
        self._init_clients()

    @property
    def sonarr(self) -> Optional[SonarrClient]:
        return self._sonarr
        
    @property
    def radarr(self) -> Optional[RadarrClient]:
        return self._radarr

    def get_status(self) -> Dict[str, bool]:
        """Check connection status for integrations."""
        status = {
            "sonarr": False,
            "radarr": False
        }
        
        if self.sonarr:
            status["sonarr"] = self.sonarr.get_system_status()
            
        if self.radarr:
            status["radarr"] = self.radarr.get_system_status()
            
        return status

    # --- Sonarr Operations ---
    
    def get_sonarr_profiles(self) -> List[Dict]:
        """Get Sonarr quality profiles."""
        if not self.sonarr:
            return []
        return self.sonarr.get_quality_profiles()
        
    def get_sonarr_folders(self) -> List[Dict]:
        """Get Sonarr root folders."""
        if not self.sonarr:
            return []
        return self.sonarr.get_root_folders()
        
    def add_series(self, 
                   tvdb_id: int, 
                   quality_profile_id: int = None, 
                   root_folder_path: str = None,
                   search: bool = True) -> Tuple[bool, str]:
        """Add series to Sonarr."""
        if not self.sonarr:
            return False, "Sonarr not configured"
            
        # Use first available profile/folder if not specified
        if not quality_profile_id:
            profiles = self.get_sonarr_profiles()
            if not profiles:
                return False, "No quality profiles found in Sonarr"
            quality_profile_id = profiles[0]['id']
            
        if not root_folder_path:
            folders = self.get_sonarr_folders()
            if not folders:
                return False, "No root folders found in Sonarr"
            root_folder_path = folders[0]['path']
            
        result = self.sonarr.add_series(
            tvdb_id=tvdb_id,
            title="", # Will be fetched by lookup
            quality_profile_id=quality_profile_id,
            root_folder_path=root_folder_path,
            search_now=search
        )
        
        if result:
            return True, "Series added successfully"
        return False, "Failed to add series"

    # --- Radarr Operations ---

    def get_radarr_profiles(self) -> List[Dict]:
        """Get Radarr quality profiles."""
        if not self.radarr:
            return []
        return self.radarr.get_quality_profiles()
        
    def get_radarr_folders(self) -> List[Dict]:
        """Get Radarr root folders."""
        if not self.radarr:
            return []
        return self.radarr.get_root_folders()
        
    def add_movie(self, 
                  tmdb_id: int, 
                  quality_profile_id: int = None, 
                  root_folder_path: str = None,
                  search: bool = True) -> Tuple[bool, str]:
        """Add movie to Radarr."""
        if not self.radarr:
            return False, "Radarr not configured"
            
        # Use first available profile/folder if not specified
        if not quality_profile_id:
            profiles = self.get_radarr_profiles()
            if not profiles:
                return False, "No quality profiles found in Radarr"
            quality_profile_id = profiles[0]['id']
            
        if not root_folder_path:
            folders = self.get_radarr_folders()
            if not folders:
                return False, "No root folders found in Radarr"
            root_folder_path = folders[0]['path']
            
        result = self.radarr.add_movie(
            tmdb_id=tmdb_id,
            title="", # Will be fetched by lookup
            quality_profile_id=quality_profile_id,
            root_folder_path=root_folder_path,
            search_now=search
        )
        
        if result:
            return True, "Movie added successfully"
        return False, "Failed to add movie"
        
        
# Lazy loader
_integration_service = None

def get_integration_service() -> IntegrationService:
    global _integration_service
    if not _integration_service:
        _integration_service = IntegrationService()
    return _integration_service
