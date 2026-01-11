"""
Fshare API Client

Handles authentication and file operations with Fshare.vn.
Refactored version with proper typing, error handling, and integration with core modules.
"""

import requests
import logging
from typing import Optional, List, Dict, Any
from datetime import datetime, timedelta
from dataclasses import dataclass

from ..core.config import get_config, FshareConfig
from ..core.exceptions import (
    AuthenticationError,
    APIError,
    ConnectionError as FshareConnectionError,
)

logger = logging.getLogger(__name__)


@dataclass
class FshareFile:
    """Represents a file on Fshare."""
    name: str
    url: str
    size: int
    fcode: str
    file_type: int = 0  # 0 = file, 1 = folder
    
    @classmethod
    def from_api_response(cls, item: Dict[str, Any]) -> "FshareFile":
        """Create FshareFile from API response item."""
        return cls(
            name=item.get("name", "Unknown"),
            url=f"https://www.fshare.vn/file/{item.get('linkcode', '')}",
            size=int(item.get("size", 0)),
            fcode=item.get("linkcode", ""),
            file_type=int(item.get("type", 0)),
        )
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "name": self.name,
            "url": self.url,
            "size": self.size,
            "fcode": self.fcode,
            "type": self.file_type,
        }


class FshareClient:
    """
    Client for interacting with Fshare.vn API.
    
    Provides methods for:
    - Authentication (login, session management)
    - File search
    - Download link generation
    - File info retrieval
    
    Example:
        >>> client = FshareClient.from_config()
        >>> if client.login():
        ...     results = client.search("movie 2024")
        ...     for file in results:
        ...         print(f"{file.name}: {file.size} bytes")
    """
    
    API_BASE = "https://www.fshare.vn/api"
    DEFAULT_APP_KEY = "L2S7R6ZMagggC5wWkQhX2+aDi467PPuftWUMRoK"
    DEFAULT_TIMEOUT = 15
    TOKEN_LIFETIME_HOURS = 24
    
    def __init__(
        self,
        email: str,
        password: str,
        app_key: Optional[str] = None,
        timeout: int = DEFAULT_TIMEOUT,
    ):
        """
        Initialize Fshare client.
        
        Args:
            email: Fshare account email
            password: Fshare account password
            app_key: Optional Fshare API app key
            timeout: Request timeout in seconds
        """
        self.email = email
        self.password = password
        self.app_key = app_key or self.DEFAULT_APP_KEY
        self.timeout = timeout
        
        self.session = requests.Session()
        self.session.headers.update({
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Content-Type": "application/json",
        })
        
        self._token: Optional[str] = None
        self._session_id: Optional[str] = None
        self._token_expires: Optional[datetime] = None
    
    @classmethod
    def from_config(cls, config: Optional[FshareConfig] = None) -> "FshareClient":
        """
        Create client from configuration.
        
        Args:
            config: Optional FshareConfig, uses global config if not provided
            
        Returns:
            Configured FshareClient instance
        """
        if config is None:
            config = get_config().fshare
        
        return cls(
            email=config.email,
            password=config.password,
            app_key=config.app_key,
        )
    
    @property
    def is_authenticated(self) -> bool:
        """Check if client has a valid authentication token."""
        if not self._token or not self._token_expires:
            return False
        return datetime.now() < self._token_expires
    
    def login(self) -> bool:
        """
        Login to Fshare and obtain session token.
        
        Returns:
            True if login successful
            
        Raises:
            AuthenticationError: If login fails
            FshareConnectionError: If connection fails
        """
        try:
            logger.info("Logging into Fshare...")
            
            response = self.session.post(
                f"{self.API_BASE}/user/login",
                json={
                    "user_email": self.email,
                    "password": self.password,
                    "app_key": self.app_key,
                },
                timeout=self.timeout,
            )
            
            if response.status_code != 200:
                raise APIError(
                    f"Login request failed",
                    status_code=response.status_code,
                    response=response.text,
                )
            
            data = response.json()
            
            if data.get("code") != 200:
                error_msg = data.get("msg", "Unknown error")
                logger.error(f"❌ Fshare login failed: {error_msg}")
                raise AuthenticationError(f"Fshare login failed: {error_msg}")
            
            self._token = data.get("token")
            self._session_id = data.get("session_id")
            
            # Update session cookie
            self.session.headers.update({
                "Cookie": f"session_id={self._session_id}"
            })
            
            # Set token expiration
            self._token_expires = datetime.now() + timedelta(hours=self.TOKEN_LIFETIME_HOURS)
            
            logger.info("✅ Fshare login successful")
            return True
            
        except requests.exceptions.RequestException as e:
            logger.error(f"❌ Fshare connection error: {e}")
            raise FshareConnectionError(f"Failed to connect to Fshare: {e}")
    
    def ensure_authenticated(self) -> bool:
        """
        Ensure we have a valid session, login if needed.
        
        Returns:
            True if authenticated
        """
        if not self.is_authenticated:
            return self.login()
        return True
    
    def search(self, query: str, limit: int = 50) -> List[FshareFile]:
        """
        Search for files on Fshare.
        
        Args:
            query: Search query string
            limit: Maximum number of results
            
        Returns:
            List of FshareFile objects
            
        Raises:
            AuthenticationError: If not authenticated
            APIError: If search fails
        """
        self.ensure_authenticated()
        
        try:
            logger.info(f"Searching Fshare for: {query}")
            
            response = self.session.post(
                f"{self.API_BASE}/fileops/search",
                json={
                    "search": query,
                    "limit": limit,
                    "token": self._token,
                },
                timeout=self.timeout,
            )
            
            if response.status_code != 200:
                raise APIError(
                    "Search request failed",
                    status_code=response.status_code,
                    response=response.text,
                )
            
            data = response.json()
            
            if data.get("code") != 200:
                logger.warning(f"Search returned error: {data.get('msg', 'Unknown')}")
                return []
            
            items = data.get("items", [])
            results = [FshareFile.from_api_response(item) for item in items]
            
            logger.info(f"✅ Found {len(results)} results")
            return results
            
        except requests.exceptions.RequestException as e:
            logger.error(f"Search error: {e}")
            raise FshareConnectionError(f"Search failed: {e}")
    
    def get_download_link(self, fcode: str) -> Optional[str]:
        """
        Get direct download link for a file.
        
        Args:
            fcode: Fshare file code
            
        Returns:
            Direct download URL or None if failed
        """
        self.ensure_authenticated()
        
        try:
            logger.info(f"Getting download link for: {fcode}")
            
            response = self.session.post(
                f"{self.API_BASE}/session/download",
                json={
                    "url": f"https://www.fshare.vn/file/{fcode}",
                    "token": self._token,
                    "password": "",
                },
                timeout=self.timeout,
            )
            
            if response.status_code != 200:
                logger.error(f"Download link request failed: {response.status_code}")
                return None
            
            data = response.json()
            
            if data.get("code") != 200:
                logger.error(f"Failed to get download link: {data.get('msg', 'Unknown')}")
                return None
            
            download_url = data.get("location")
            logger.info("✅ Got download link")
            return download_url
            
        except requests.exceptions.RequestException as e:
            logger.error(f"Error getting download link: {e}")
            return None
    
    def get_file_info(self, url: str) -> Optional[FshareFile]:
        """
        Get file information from Fshare URL.
        
        Args:
            url: Fshare file URL
            
        Returns:
            FshareFile object or None if failed
        """
        self.ensure_authenticated()
        
        try:
            # Extract fcode from URL
            fcode = url.split("/file/")[-1].split("?")[0]
            
            response = self.session.post(
                f"{self.API_BASE}/fileops/get",
                json={
                    "url": url,
                    "token": self._token,
                },
                timeout=self.timeout,
            )
            
            if response.status_code != 200:
                logger.error(f"File info request failed: {response.status_code}")
                return None
            
            data = response.json()
            
            if data.get("code") != 200:
                logger.error(f"Failed to get file info: {data.get('msg', 'Unknown')}")
                return None
            
            item = data.get("item", {})
            return FshareFile(
                name=item.get("name", "Unknown"),
                size=int(item.get("size", 0)),
                fcode=fcode,
                url=url,
            )
            
        except requests.exceptions.RequestException as e:
            logger.error(f"Error getting file info: {e}")
            return None
