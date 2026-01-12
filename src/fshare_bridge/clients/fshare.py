"""
Fshare API Client

Handles authentication and file operations with Fshare.vn.
Refactored version with proper typing, error handling, and integration with core modules.
"""

import requests
import logging
import re
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
    - Folder enumeration (from pyLoad FshareVnFolder plugin)
    
    Example:
        >>> client = FshareClient.from_config()
        >>> if client.login():
        ...     results = client.search("movie 2024")
        ...     for file in results:
        ...         print(f"{file.name}: {file.size} bytes")
    """
    
    # API endpoints
    API_BASE = "https://api2.fshare.vn/api"
    API_V3_BASE = "https://www.fshare.vn/api/v3"
    API_FSHARE_BASE = "https://api.fshare.vn/api"
    
    # From pyLoad FshareVn plugin
    API_KEY = "dMnqMMZMUnN5YpvKENaEhdQQ5jxDqddt"
    API_USERAGENT = "pyLoad-B1RS5N"
    
    # Default app key (original bridge)
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

    @property
    def is_premium(self) -> bool:
        """Check if account is premium/VIP."""
        return self._is_premium
    @property
    def premium_expiry(self) -> Optional[int]:
        """Get premium account expiration timestamp (Unix timestamp), -1 for lifetime."""
        return self._premium_expiry

        self._is_premium: bool = False
    
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
        Login to Fshare using /site/login endpoint with CSRF token.
        
        Returns:
            True if login successful
            
        Raises:
            AuthenticationError: If login fails
            FshareConnectionError: If connection fails
        """
        try:
            logger.info("Logging into Fshare...")
            
           # Step 1: Get CSRF token from homepage
            homepage = self.session.get(
                "https://www.fshare.vn/",
                timeout=self.timeout
            )
            
            # Extract CSRF token
            csrf_match = re.search(r'name="_csrf-app" value="([^"]+)"', homepage.text)
            if not csrf_match:
                logger.error("Could not find CSRF token")
                raise AuthenticationError("Could not find CSRF token")
            
            csrf_token = csrf_match.group(1)
            logger.debug(f"Got CSRF token: {csrf_token[:20]}...")
            
            # Step 2: Submit login form with CSRF token
            response = self.session.post(
                "https://www.fshare.vn/site/login",
                data={
                    "_csrf-app": csrf_token,
                    "LoginForm[email]": self.email,
                    "LoginForm[password]": self.password,
                    "LoginForm[rememberMe]": 1
                },
                headers={
                    "Referer": "https://www.fshare.vn/",
                    "Content-Type": "application/x-www-form-urlencoded"
                },
                timeout=self.timeout,
                allow_redirects=True
            )
            
            logger.info(f"Fshare login response status: {response.status_code}")
            logger.debug(f"Fshare login response URL: {response.url}")
            
            if response.status_code not in [200, 302]:
                logger.error(f"Fshare login HTTP error: {response.status_code}")
                raise APIError(
                    f"Login request failed with status {response.status_code}",
                    status_code=response.status_code,
                    response=response.text[:200],
                )
            
            # Check if login was successful by looking for profile or VIP indicators
            if 'profile' in response.text.lower() or 'user__profile' in response.text:
                logger.info("✅ Fshare login successful")
                self._token = "web_session"  # Placeholder since we use cookies
                
                # Check for VIP status
                self._is_premium = 'VIP' in response.text or 'img alt="VIP"' in response.text
                logger.info(f"Premium status: {self._is_premium}")
                
                # Fetch account info to get expiration date
                try:
                    account_info = self.session.get(
                        "https://www.fshare.vn/account_info.php",
                        timeout=self.timeout
                    )
                    
                    if account_info.status_code == 200:
                        # Parse expiration date (format: HH:MM:SS AM/PM DD-MM-YYYY)
                        valid_match = re.search(r'<dt>Thời hạn dùng:</dt>\s*<dd>(.+?)</dd>', account_info.text)
                        if valid_match:
                            import time
                            try:
                                expiry_str = valid_match.group(1).strip()
                                expiry_time = time.mktime(time.strptime(expiry_str, '%I:%M:%S %p %d-%m-%Y'))
                                self._premium_expiry = int(expiry_time)
                                logger.info(f"Account expires: {expiry_str}")
                            except:
                                self._premium_expiry = None
                        
                        # Check for lifetime account
                        if re.search(r'<dt>Lần đăng nhập trước:</dt>', account_info.text):
                            self._premium_expiry = -1  # Lifetime
                            logger.info("Lifetime premium account detected")
                except Exception as e:
                    logger.warning(f"Could not fetch account expiration: {e}")
                    self._premium_expiry = None
                
                return True
            else:
                logger.error("❌ Fshare login failed: Invalid credentials or unexpected response")
                raise AuthenticationError("Fshare login failed: Invalid credentials")
        
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
    
    def get_file_info_v3(self, fcode: str) -> Optional[FshareFile]:
        """
        Get file information using V3 API (from pyLoad FshareVn plugin).
        
        Args:
            fcode: Fshare file code (linkcode)
            
        Returns:
            FshareFile object or None if failed
        """
        try:
            response = self.session.get(
                f"{self.API_V3_BASE}/files/folder",
                params={"linkcode": fcode},
                headers={"Accept": "application/json, text/plain, */*"},
                timeout=self.timeout,
            )
            
            if response.status_code != 200:
                return None
            
            data = response.json()
            
            if data.get("status") == 404:
                return None
            
            current = data.get("current", {})
            return FshareFile(
                name=current.get("name", "Unknown"),
                size=int(current.get("size", 0)),
                fcode=fcode,
                url=f"https://www.fshare.vn/file/{fcode}",
            )
            
        except Exception as e:
            logger.error(f"V3 API error: {e}")
            return None
    
    def enumerate_folder(
        self,
        folder_code: str,
        include_subfolders: bool = False,
    ) -> List[FshareFile]:
        """
        Enumerate all files in a folder (from pyLoad FshareVnFolder plugin).
        
        Args:
            folder_code: Fshare folder code
            include_subfolders: Whether to recursively include subfolders
            
        Returns:
            List of FshareFile objects
        """
        files = []
        current_page = 1
        
        try:
            while True:
                response = self.session.get(
                    f"{self.API_V3_BASE}/files/folder",
                    params={"linkcode": folder_code, "page": current_page},
                    headers={"Accept": "application/json, text/plain, */*"},
                    timeout=self.timeout,
                )
                
                if response.status_code != 200:
                    break
                
                data = response.json()
                items = data.get("items", [])
                
                for item in items:
                    item_type = int(item.get("type", 0))
                    linkcode = item.get("linkcode", "")
                    
                    if item_type == 1:
                        # File
                        files.append(FshareFile(
                            name=item.get("name", "Unknown"),
                            size=int(item.get("size", 0)),
                            fcode=linkcode,
                            url=f"https://www.fshare.vn/file/{linkcode}",
                            file_type=1,
                        ))
                    elif item_type == 0 and include_subfolders:
                        # Folder - recursively enumerate
                        files.extend(self.enumerate_folder(linkcode, include_subfolders))
                
                # Check for more pages
                links = data.get("_links", {})
                last_link = links.get("last", "")
                
                import re
                last_page_match = re.search(r"&page=(\d+)", last_link)
                last_page = int(last_page_match.group(1)) if last_page_match else 1
                
                current_page += 1
                if current_page > last_page:
                    break
            
            logger.info(f"Enumerated {len(files)} files from folder {folder_code}")
            return files
            
        except Exception as e:
            logger.error(f"Folder enumeration error: {e}")
            return files
    
    def get_download_link_premium(
        self,
        url: str,
        password: Optional[str] = None,
    ) -> Optional[str]:
        """
        Get direct download link using premium API (from pyLoad FshareVn plugin).
        
        Args:
            url: Fshare file URL
            password: Optional file password
            
        Returns:
            Direct download URL or None if failed
        """
        self.ensure_authenticated()
        
        try:
            payload = {
                "url": url,
                "token": self._token,
            }
            if password:
                payload["password"] = password
            
            response = self.session.post(
                f"{self.API_FSHARE_BASE}/session/download",
                json=payload,
                headers={
                    "User-Agent": self.API_USERAGENT,
                    "Content-Type": "application/json",
                },
                cookies={"session_id": self._session_id} if self._session_id else None,
                timeout=self.timeout,
            )
            
            if response.status_code == 403:
                if password:
                    logger.error("Wrong password")
                else:
                    logger.error("Download is password protected")
                return None
            
            if response.status_code != 200:
                logger.error(f"Premium download failed: {response.status_code}")
                return None
            
            data = response.json()
            return data.get("location")
            
        except Exception as e:
            logger.error(f"Premium download error: {e}")
            return None
