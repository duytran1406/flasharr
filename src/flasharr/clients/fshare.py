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
    - File search (Deprecated)
    - Download link generation
    - File info retrieval
    - Folder enumeration
    """
    
    # API endpoints
    API_V3_BASE = "https://www.fshare.vn/api/v3"
    
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
        # Prefer web_session token validation
        if self._token == "web_session" and self._token_expires:
             if datetime.now() < self._token_expires:
                 return True
             else:
                 logger.debug("Web session token expired")
                 return False

        if self._token and self._token_expires:
            return datetime.now() < self._token_expires
        
        # Fallback: check session cookies
        if self.session.cookies.get('_identity-app') or self.session.cookies.get('session_id'):
            return True
            
        return False
    
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
            
            # Clear existing cookies to ensure a fresh session
            self.session.cookies.clear()
            self._token = None # Clear token
            
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
                    "User-Agent": self.session.headers.get("User-Agent")
                },
                timeout=self.timeout,
                allow_redirects=True
            )
            
            logger.info(f"Fshare login response status: {response.status_code}")
            
            if response.status_code not in [200, 302]:
                logger.error(f"Fshare login HTTP error: {response.status_code}")
                raise APIError(
                    f"Login request failed with status {response.status_code}",
                    status_code=response.status_code,
                    response=response.text[:200],
                )
            
            # Check if login was successful
            if '/site/logout' in response.text:
                logger.info("✅ Fshare login successful")
                self._token = "web_session"
                # Set session expiry to 7 days (assume cookies persist longer)
                self._token_expires = datetime.now() + timedelta(days=7)
                
                # Fetch account profile page to extract account information
                try:
                    self._parse_profile(self.session.get("https://www.fshare.vn/account/profile", timeout=self.timeout).text)
                except Exception as e:
                    logger.warning(f"Could not fetch/parse account profile page: {e}")
                    # Fallback to basic detection
                    self._is_premium = True  # Assume premium if logged in
                
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
                    
                raise AuthenticationError(f"Fshare login failed: {error_msg}")
        
        except requests.exceptions.RequestException as e:
            logger.error(f"❌ Fshare connection error: {e}")
            raise FshareConnectionError(f"Failed to connect to Fshare: {e}")

    def _parse_profile(self, html: str):
        """Helper to parse profile HTML."""
        # ACCOUNT_TYPE
        account_type_match = re.search(r'Loại tài khoản</a>\s*<span>(.*?)</span>', html, re.IGNORECASE | re.DOTALL)
        if account_type_match:
            self._account_type = account_type_match.group(1).strip()
            self._is_premium = 'VIP' in self._account_type.upper()
            logger.info(f"Account type: {self._account_type} (Premium: {self._is_premium})")
        else:
            self._is_premium = 'VIP' in html or 'img alt="VIP"' in html or 'level-vip' in html.lower()
        
        # VALID_UNTIL
        valid_until_match = re.search(r'Hạn dùng:</a>\s*<span[^>]*>(.*?)</span>', html, re.IGNORECASE | re.DOTALL)
        if valid_until_match:
            try:
                import time
                expiry_str = valid_until_match.group(1).strip()
                for fmt in ['%d/%m/%Y', '%d-%m-%Y', '%I:%M:%S %p %d-%m-%Y']:
                    try:
                        expiry_time = time.mktime(time.strptime(expiry_str, fmt))
                        self._premium_expiry = int(expiry_time)
                        break
                    except ValueError:
                        continue
            except: pass
        elif re.search(r'Vĩnh viễn|Lifetime|Forever', html, re.IGNORECASE):
             self._premium_expiry = -1

        # TRAFFIC
        traffic_match = re.search(r'Dung lượng tải trong ngày:\s*</a>\s*(.*?)\s*</p>', html, re.IGNORECASE | re.DOTALL)
        if traffic_match:
            raw_traffic = traffic_match.group(1).strip()
            self._traffic_left = re.sub(r'\s+', ' ', raw_traffic)

    def ensure_authenticated(self) -> bool:
        """
        Ensure we have a valid session, login if needed.
        """
        if self.is_authenticated:
            return True
            
        logger.info("Session expired or missing, logging in...")
        return self.login()
    
    def search(self, query: str, limit: int = 50) -> List[FshareFile]:
        """
        DEAD API: Use TimFshare instead.
        """
        logger.warning("FshareClient.search is deprecated (API dead).")
        return []
    
    def get_download_link(self, fcode: str) -> Optional[str]:
        """
        Get direct download link for a file.
        """
        try:
            self.ensure_authenticated()
            
            # Removed redundant token check to prevent login loops.
            # We rely on ensure_authenticated() and the redirect implementation 
            # in get_download_link_premium to handle session validity.

            url = f"https://www.fshare.vn/file/{fcode}"
            return self.get_download_link_premium(url)
            
        except requests.exceptions.RequestException as e:
            logger.error(f"Error getting download link: {e}")
            return None
    
    def get_file_info(self, url: str) -> Optional[FshareFile]:
        """
        Get file information from Fshare URL.
        """
        self.ensure_authenticated()
        
        fcode = url.split("/file/")[-1].split("?")[0]
        
        # Try V3 API (internal web API) first
        info = self.get_file_info_v3(fcode)
        if info:
            return info

        # Fallback to HTML scraping if V3 API fails
        logger.info(f"V3 API failed for {fcode}, falling back to HTML scraping...")
        return self.get_file_info_html(url)

    def get_file_info_html(self, url: str) -> Optional[FshareFile]:
        """
        Get file information by scraping the file page HTML.
        """
        try:
            response = self.session.get(url, timeout=self.timeout)
            if response.status_code != 200:
                logger.error(f"HTML scraping failed: {response.status_code}")
                return None
            
            html = response.text
            fcode = url.split("/file/")[-1].split("?")[0]
            
            # Parse name from title or h1
            # <h1 ... class="...file-name...">Filename.ext</h1>
            name = "Unknown"
            name_match = re.search(r'<h1[^>]*class="[^"]*file-name[^"]*"[^>]*>(.*?)</h1>', html, re.IGNORECASE | re.DOTALL)
            if name_match:
                name = name_match.group(1).strip()
            else:
                title_match = re.search(r'<title>(.*?)</title>', html)
                if title_match:
                     title_text = title_match.group(1)
                     if " - Fshare" in title_text:
                         name = title_text.split(" - Fshare")[0].strip()
                     else:
                         name = title_text.strip()
            
            return FshareFile(
                name=name,
                size=0, # Size parsing from raw HTML is unreliable without specific class
                fcode=fcode,
                url=url,
            )
            
        except Exception as e:
            logger.error(f"HTML scraping error: {e}")
            return None
    
    def get_file_info_v3(self, fcode: str) -> Optional[FshareFile]:
        """
        Get file information using V3 API (internal web API).
        """
        try:
            response = self.session.get(
                f"{self.API_V3_BASE}/files/folder",
                params={"linkcode": fcode},
                headers={"Accept": "application/json, text/plain, */*"},
                timeout=self.timeout,
            )
            
            if response.status_code != 200:
                logger.warning(f"V3 API returned status {response.status_code} for {fcode}")
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
        Enumerate all files in a folder.
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
                        files.extend(self.enumerate_folder(linkcode, include_subfolders))
                
                links = data.get("_links", {})
                last_link = links.get("last", "")
                
                import re
                last_page_match = re.search(r"&page=(\d+)", last_link)
                last_page = int(last_page_match.group(1)) if last_page_match else 1
                
                current_page += 1
                if current_page > last_page:
                    break
            
            return files
            
        except Exception as e:
            logger.error(f"Folder enumeration error: {e}")
            return files

    def get_daily_quota(self) -> Optional[str]:
        """
        Request Fshare profile HTML and parse Daily Quota.
        """
        try:
            self.ensure_authenticated()
            response = self.session.get("https://www.fshare.vn/account/profile", timeout=self.timeout)
            
            # Check for session expiration (redirect to login)
            if "site/login" in response.url:
                logger.warning("Session expired during quota check. Re-authenticating...")
                self._token = None
                if self.login():
                    response = self.session.get("https://www.fshare.vn/account/profile", timeout=self.timeout)
                else:
                    return None

            if response.status_code != 200:
                return None
                
            html = response.text
            traffic_match = re.search(r'Dung lượng tải trong ngày:\s*</a>\s*(.*?)\s*</p>', html, re.IGNORECASE | re.DOTALL)
            
            if traffic_match:
                raw_traffic = traffic_match.group(1).strip()
                clean_traffic = re.sub(r'\s+', ' ', raw_traffic)
                self._traffic_left = clean_traffic
                return clean_traffic
            
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
        Get direct download link using form-based download.
        """
        self.ensure_authenticated()
        
        try:
            # Step 1: Load file page
            logger.info(f"Loading file page: {url}")
            page_response = self.session.get(url, timeout=self.timeout)
            
            # CHECK FOR REDIRECT TO LOGIN (Fix for session timeout)
            if "site/login" in page_response.url:
                 logger.warning("Session expired (redirected to login) while loading file page. Re-authenticating...")
                 self._token = None
                 if self.login():
                      page_response = self.session.get(url, timeout=self.timeout)
                 else:
                      logger.error("Re-authentication failed during download page load")
                      return None

            if page_response.status_code != 200:
                logger.error(f"Failed to load file page: {page_response.status_code}")
                return None
            
            # Step 2: Check for password
            if password and 'password-form' in page_response.text:
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
            
            # Step 3: Find download form
            form_match = re.search(r'<form[^>]*id="form-download"[^>]*>(.*?)</form>', page_response.text, re.DOTALL)
            if not form_match:
                logger.error("Download form not found on page")
                return None
            
            form_html = form_match.group(0)
            action_match = re.search(r'action="([^"]+)"', form_html)
            if not action_match:
                return None
            
            form_action = action_match.group(1)
            if form_action.startswith('/'):
                form_action = f"https://www.fshare.vn{form_action}"
            
            # Get CSRF and linkcode
            csrf_token = None
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
            
            linkcode_match = re.search(r'name="linkcode" value="([^"]+)"', form_html)
            linkcode = linkcode_match.group(1) if linkcode_match else ""
            
            if not csrf_token:
                logger.error("CSRF token not found")
                return None
            
            # Submit form
            logger.info("Submitting download form...")
            download_data = {
                '_csrf-app': csrf_token,
                'linkcode': linkcode or "",
                'withFcode5': '0',
                'ushare': '',
            }
            
            headers = {
                "Content-Type": "application/x-www-form-urlencoded",
                "Referer": url,
                "X-CSRF-Token": csrf_token,
                "X-Requested-With": "XMLHttpRequest",
                "User-Agent": self.session.headers.get("User-Agent")
            }
            
            download_response = self.session.post(
                form_action,
                data=download_data,
                headers=headers,
                timeout=self.timeout
            )
            
            if download_response.status_code not in [200, 201]:
                logger.error(f"Download form submission failed: {download_response.status_code}")
                return None
            
            try:
                data = download_response.json()
            except:
                return None
            
            if 'msg' in data and data['msg']:
                logger.error(f"Download error message: {data['msg']}")
                return None
            
            return data.get('url')
            
        except Exception as e:
            logger.error(f"Form-based download error: {e}")
            return None
