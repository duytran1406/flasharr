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
    
    API_USERAGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
    
    DEFAULT_TIMEOUT = 15
    TOKEN_LIFETIME_HOURS = 24
    
    def __init__(
        self,
        email: str,
        password: str,
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
        self.timeout = timeout
        
        self.session = requests.Session()
        self.session.headers.update({
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Content-Type": "application/json",
        })
        
        self._token: Optional[str] = None
        self._session_id: Optional[str] = None
        self._token_expires: Optional[datetime] = None
        
        self._is_premium: bool = False
        self._premium_expiry: Optional[int] = None
        self._traffic_left: Optional[str] = None
        self._account_type: Optional[str] = None

    @property
    def is_premium(self) -> bool:
        """Check if account is premium/VIP."""
        return self._is_premium
    
    @property
    def premium_expiry(self) -> Optional[int]:
        """Get premium account expiration timestamp (Unix timestamp), -1 for lifetime."""
        return self._premium_expiry
        
    @property
    def traffic_left(self) -> Optional[str]:
        """Get traffic left details."""
        return self._traffic_left
        
    @property
    def account_type(self) -> Optional[str]:
        """Get account type string."""
        return self._account_type
        
    def get_cookies(self) -> Dict[str, str]:
        """Get current session cookies."""
        return self.session.cookies.get_dict()
        
    def set_cookies(self, cookies: Dict[str, str]):
        """Set session cookies."""
        if cookies:
            self.session.cookies.update(cookies)
    
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
        )
    
    @property
    def is_authenticated(self) -> bool:
        """Check if client has a valid authentication token or session cookies."""
        if self._token and self._token_expires:
            return datetime.now() < self._token_expires
        
        # Fallback: check session cookies
        if self.session.cookies.get('_identity-app') or self.session.cookies.get('session_id'):
            return True
            
        return False
    
    def login(self) -> bool:
        """
        Login to Fshare using /site/login endpoint with CSRF token.
        Parse account information using patterns from pyLoad FshareVn plugin.
        
        Returns:
            True if login successful
            
        Raises:
            AuthenticationError: If login fails
            FshareConnectionError: If connection fails
        """
        try:
            logger.info("Logging into Fshare...")
            
            # Clear existing cookies to ensure a fresh session
            self.session.cookies.clear()
            
            # Step 1: Get CSRF token from homepage
            homepage = self.session.get(
                "https://www.fshare.vn/",
                timeout=self.timeout
            )
            
            # Extract CSRF token (try both input and meta)
            csrf_token = None
            csrf_match = re.search(r'name="_csrf-app" value="([^"]+)"', homepage.text)
            if csrf_match:
                csrf_token = csrf_match.group(1)
            else:
                csrf_match = re.search(r'content="([^"]+)" name="csrf-token"', homepage.text)
                if csrf_match:
                    csrf_token = csrf_match.group(1)
            
            logger.debug(f"Cookies after homepage GET: {self.session.cookies.get_dict()}")
            
            if not csrf_token:
                logger.error("Could not find CSRF token")
                logger.debug(f"HTML snippet: {homepage.text[:500]}")
                raise AuthenticationError("Could not find CSRF token")
            
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
                    "Content-Type": "application/x-www-form-urlencoded",
                    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
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
            
            # Check if login was successful
            # pyLoad plugin checks for presence of logout link
            # We strictly check for /site/logout which indicates an active session
            if '/site/logout' in response.text:
                logger.info("✅ Fshare login successful")
                self._token = "web_session"
                # Set session expiry to 7 days (assume cookies persist longer)
                self._token_expires = datetime.now() + timedelta(days=7)
                logger.debug(f"Session valid until: {self._token_expires}")
                logger.debug(f"Cookies after login: {self.session.cookies.get_dict()}")
                
                # Fetch account profile page to extract account information
                try:
                    profile_page = self.session.get("https://www.fshare.vn/account/profile", timeout=self.timeout)
                    html = profile_page.text
                    
                    # Updated regex patterns based on user feedback
                    
                    # ACCOUNT_TYPE: <a href="/account/profile">Loại tài khoản</a>                    <span>Vip</span>
                    account_type_match = re.search(r'Loại tài khoản</a>\s*<span>(.*?)</span>', html, re.IGNORECASE | re.DOTALL)
                    if account_type_match:
                        self._account_type = account_type_match.group(1).strip()
                        self._is_premium = 'VIP' in self._account_type.upper()
                        logger.info(f"Account type: {self._account_type} (Premium: {self._is_premium})")
                    else:
                        logger.warning("Could not parse account type, checking for fallback indicators")
                        self._is_premium = 'VIP' in html or 'img alt="VIP"' in html or 'level-vip' in html.lower()
                    
                    # VALID_UNTIL: <a href="/account/profile">Hạn dùng:</a>                    <span ...>31/01/2026</span>
                    valid_until_match = re.search(r'Hạn dùng:</a>\s*<span[^>]*>(.*?)</span>', html, re.IGNORECASE | re.DOTALL)
                    if valid_until_match:
                        try:
                            import time
                            expiry_str = valid_until_match.group(1).strip()
                            # Try multiple date formats
                            for fmt in ['%d/%m/%Y', '%d-%m-%Y', '%I:%M:%S %p %d-%m-%Y']:
                                try:
                                    expiry_time = time.mktime(time.strptime(expiry_str, fmt))
                                    self._premium_expiry = int(expiry_time)
                                    logger.info(f"Account expires: {expiry_str}")
                                    break
                                except ValueError:
                                    continue
                            else:
                                logger.warning(f"Could not parse expiry date format: {expiry_str}")
                                self._premium_expiry = None
                        except Exception as e:
                            logger.warning(f"Error parsing expiry date: {e}")
                            self._premium_expiry = None
                    else:
                        # Check for lifetime
                        if re.search(r'Vĩnh viễn|Lifetime|Forever', html, re.IGNORECASE):
                             self._premium_expiry = -1
                        else:
                             self._premium_expiry = None

                    # TRAFFIC: <a href="...">Dung lượng tải trong ngày: </a>                            0 Bytes /                             150 GB                        </p>
                    # We will capture the whole string "0 Bytes / 150 GB"
                    traffic_match = re.search(r'Dung lượng tải trong ngày:\s*</a>\s*(.*?)\s*</p>', html, re.IGNORECASE | re.DOTALL)
                    if traffic_match:
                        raw_traffic = traffic_match.group(1).strip()
                        # Clean up multiple spaces
                        raw_traffic = re.sub(r'\s+', ' ', raw_traffic)
                        self._traffic_left = raw_traffic
                        logger.info(f"Daily traffic: {self._traffic_left}")
                    else:
                        logger.debug("Could not find daily traffic information")
                        self._traffic_left = None
                    
                except Exception as e:
                    logger.warning(f"Could not fetch account profile page: {e}")
                    # Fallback to basic detection
                    self._is_premium = True  # Assume premium if logged in
                    self._premium_expiry = None
                    self._traffic_left = None
                
                return True
            else:
                # Try to find specific error message
                error_msg = "Invalid credentials or unexpected response"
                if "Email hoặc mật khẩu không đúng" in response.text:
                    error_msg = "Incorrect email or password"
                elif "Tài khoản đang bị khóa" in response.text:
                    error_msg = "Account is locked"
                elif "recaptcha" in response.text.lower():
                    error_msg = "CAPTCHA required by Fshare. Please log in through the website first."
                else:
                     logger.error(f"❌ Unknown login failure. URL: {response.url}. Status: {response.status_code}")
                     logger.debug(f"HTML snippet: {response.text[:2000]}")
                    
                logger.debug(f"Login failed HTML preview: {response.text[:1000]}")
                logger.debug(f"Cookies on failure: {self.session.cookies.get_dict()}")
                logger.error(f"❌ Fshare login failed: {error_msg}")
                logger.error(f"Full Login Page (first 5000 chars): {response.text[:5000]}")
                raise AuthenticationError(f"Fshare login failed: {error_msg}")
        
        except requests.exceptions.RequestException as e:
            logger.error(f"❌ Fshare connection error: {e}")
            raise FshareConnectionError(f"Failed to connect to Fshare: {e}")


    def ensure_authenticated(self) -> bool:
        """
        Ensure we have a valid session, login if needed.
        
        Returns:
            True if authenticated
        """
        if self.is_authenticated:
            return True
            
        logger.info("Session expired or missing, logging in...")
        logger.debug(f"Current token: {self._token}")
        logger.debug(f"Token expires: {self._token_expires}")
        logger.debug(f"Cookies: {self.session.cookies.get_dict()}")
        
        return self.login()
    
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
        try:
            # Enforce authentication (which should set _token="web_session")
            self.ensure_authenticated()
            
            # Additional check: If login didn't result in a web session, we can't proceed
            if self._token != "web_session":
                logger.error("No valid web session found. Attempting to re-login...")
                if self.login() and self._token == "web_session":
                     pass # Continue to download
                else:
                    logger.error("Could not obtain web session. Cannot generate premium download link.")
                    return None

            url = f"https://www.fshare.vn/file/{fcode}"
            return self.get_download_link_premium(url)
            
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
        
        # Extract fcode from URL
        fcode = url.split("/file/")[-1].split("?")[0]
        
        # Strictly use V3 API (web session)
        info = self.get_file_info_v3(fcode)
        if info:
            return info
            
        logger.error(f"Failed to get file info for {fcode}. Ensure valid web session.")
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

    def get_daily_quota(self) -> Optional[str]:
        """
        Request Fshare profile HTML and parse Daily Quota information.
        This sends a request to /account/profile and extracts the usage stats.

        Returns:
            String containing daily quota info (e.g. "5.2 GB / 100 GB") or None.
        """
        try:
            # Ensure we have a valid session
            self.ensure_authenticated()
            
            logger.debug("Fetching profile page for daily quota...")
            response = self.session.get("https://www.fshare.vn/account/profile", timeout=self.timeout)
            
            # Check for session expiration (redirect to login)
            if "site/login" in response.url:
                logger.warning("Session expired (redirected to login). Attempting re-authentication...")
                self._token = None # Invalidate local token
                if self.login():
                    # Retry fetch after successful login
                    response = self.session.get("https://www.fshare.vn/account/profile", timeout=self.timeout)
                else:
                    logger.error("Re-authentication failed during daily quota check")
                    return None

            if response.status_code != 200:
                logger.error(f"Failed to get profile page. Status: {response.status_code}")
                return None
                
            html = response.text
            
            # Parse traffic/daily quota
            # Pattern: <a href="...">Dung lượng tải trong ngày: </a> 0 Bytes / 150 GB </p>
            traffic_match = re.search(r'Dung lượng tải trong ngày:\s*</a>\s*(.*?)\s*</p>', html, re.IGNORECASE | re.DOTALL)
            
            if traffic_match:
                raw_traffic = traffic_match.group(1).strip()
                # Clean up multiple spaces and newlines
                clean_traffic = re.sub(r'\s+', ' ', raw_traffic)
                logger.info(f"Daily quota parsed: {clean_traffic}")
                
                # Update internal state as side effect
                self._traffic_left = clean_traffic
                
                return clean_traffic
            
            logger.warning("Could not find daily quota pattern in profile page")
            return None
            
        except Exception as e:
            logger.error(f"Error getting daily quota: {e}")
            return None
    
    def get_download_link_premium(
        self,
        url: str,
        password: Optional[str] = None,
    ) -> Optional[str]:
        """
        Get direct download link using form-based download (pyLoad method).
        This works with web session cookies.
        
        Args:
            url: Fshare file URL
            password: Optional file password
            
        Returns:
            Direct download URL or None if failed
        """
        self.ensure_authenticated()
        
        try:
            # Step 1: Load the file page to get the download form
            logger.info(f"Loading file page: {url}")
            page_response = self.session.get(url, timeout=self.timeout)
            
            if page_response.status_code != 200:
                logger.error(f"Failed to load file page: {page_response.status_code}")
                return None
            
            # Step 2: Check for password protection
            if password and 'password-form' in page_response.text:
                # Handle password-protected files
                csrf_match = re.search(r'name="_csrf-app"\s+value="([^"]+)"', page_response.text)
                if not csrf_match:
                    csrf_match = re.search(r'value="([^"]+)"\s+name="_csrf-app"', page_response.text)
                
                if csrf_match:
                    csrf_token = csrf_match.group(1)
                    password_data = {
                        '_csrf-app': csrf_token,
                        'DownloadPasswordForm[password]': password,
                    }
                    page_response = self.session.post(url, data=password_data, timeout=self.timeout)
                    
                    if 'Sai mật khẩu' in page_response.text or 'Wrong password' in page_response.text:
                        logger.error("Wrong password")
                        return None
            
            # Step 3: Find and submit the download form
            # Look for the form-download form and extract action URL
            form_match = re.search(r'<form[^>]*id="form-download"[^>]*>(.*?)</form>', page_response.text, re.DOTALL)
            if not form_match:
                logger.error("Download form not found on page")
                return None
            
            form_html = form_match.group(0)
            
            # Extract form action
            action_match = re.search(r'action="([^"]+)"', form_html)
            if not action_match:
                logger.error("Form action not found")
                return None
            
            form_action = action_match.group(1)
            # Convert relative URL to absolute
            if form_action.startswith('/'):
                form_action = f"https://www.fshare.vn{form_action}"
            
            logger.info(f"Form action: {form_action}")
            
            # Extract CSRF token from form or meta tags (more reliable)
            csrf_token = None
            
            # Try multiple patterns for meta tags and hidden inputs
            patterns = [
                r'name="_csrf-app"\s+value="([^"]+)"',
                r'value="([^"]+)"\s+name="_csrf-app"',
                r'name="csrf-token"\s+content="([^"]+)"',
                r'content="([^"]+)"\s+name="csrf-token"',
            ]
            
            for pattern in patterns:
                match = re.search(pattern, page_response.text)
                if match:
                    csrf_token = match.group(1)
                    break
            
            if not csrf_token:
                logger.error("CSRF token not found on page")
                return None
            
            logger.debug(f"Using CSRF token: {csrf_token[:20]}...")

            # Extract linkcode (fcode) from form
            linkcode_match = re.search(r'name="linkcode" value="([^"]+)"', form_html)
            linkcode = linkcode_match.group(1) if linkcode_match else ""
            
            # Prepare form data
            download_data = {
                '_csrf-app': csrf_token,
                'linkcode': linkcode or "",
                'withFcode5': '0',
                'ushare': '',
            }
            
            # Post to the form action URL
            logger.info("Submitting download form...")
            
            headers = {
                "Content-Type": "application/x-www-form-urlencoded",
                "Referer": url,
                "X-CSRF-Token": csrf_token,
                "X-Requested-With": "XMLHttpRequest",
                "User-Agent": self.session.headers.get("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            }
            
            logger.debug(f"Cookies before post: {self.session.cookies.get_dict()}")
            
            download_response = self.session.post(
                form_action,
                data=download_data,
                headers=headers,
                timeout=self.timeout
            )
            
            if download_response.status_code == 400:
                logger.error("Download form submission failed with 400 (Bad Request).")
                logger.error(f"Response Body: {download_response.text[:2000]}")
                # We stop here to avoid account lockout from repeated login attempts.
                return None
                
            if download_response.status_code not in [200, 201]:
                logger.error(f"Download form submission failed: {download_response.status_code}")
                logger.error(f"Form HTML (Full): {form_html}") 
                logger.error(f"Data sent: {download_data}")
                logger.error(f"Headers sent: {headers}")
                logger.error(f"Cookies sent: {self.session.cookies.get_dict()}")
                logger.error(f"Response: {download_response.text[:1000]}...")
                return None
            
            # Step 4: Parse JSON response
            try:
                data = download_response.json()
            except:
                logger.error("Response is not valid JSON")
                logger.debug(f"Response: {download_response.text[:500]}")
                return None
            
            # Check for error message
            if 'msg' in data and data['msg']:
                logger.error(f"Download error: {data['msg']}")
                return None
            
            # Get the download URL
            download_url = data.get('url')
            if not download_url:
                logger.error("No download URL in response")
                logger.debug(f"Response data: {data}")
                return None
            
            # Note: pyLoad waits for wait_time, but we'll return immediately
            # The download engine can handle any wait time if needed
            wait_time = data.get('wait_time', 0)
            if wait_time:
                logger.info(f"Server requested wait time: {wait_time}s (ignoring for now)")
            
            logger.info("✅ Got download link via form submission")
            return download_url
            
        except Exception as e:
            logger.error(f"Form-based download error: {e}")
            import traceback
            traceback.print_exc()
            return None
