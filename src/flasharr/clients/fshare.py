"""
Fshare API Client

Handles authentication and file operations with Fshare.vn.
Refactored version with proper typing, error handling, and integration with core modules.
"""

import requests
import logging
import re
import threading
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
        
        # Callback for when session/cookies change (to persist back to storage)
        self._on_session_update = None
        
        # Lock for preventing concurrent login attempts (Promise-like behavior)
        self._login_lock = threading.Lock()

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
        if not self.session.cookies:
            return {}
        return requests.utils.dict_from_cookiejar(self.session.cookies)
        
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
            import traceback
            stack = traceback.extract_stack()
            # Get last 5 frames for context
            trace = " -> ".join([f"{f.name}:{f.lineno}" for f in stack[-5:-1]])
            
            logger.info(f"Logging into Fshare... (Instance: {id(self)}, Path: {trace})")
            
            # Clear existing cookies to ensure a fresh session
            self.session.cookies.clear()
            self._token = None # Clear token
            
            # Step 1: Get CSRF token from login page
            homepage = self.session.get(
                "https://www.fshare.vn/site/login",
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
            
            logger.info(f"Got CSRF token: {csrf_token}")
            logger.info(f"Using cookies: {self.session.cookies.get_dict()}")
            
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
                    "Referer": "https://www.fshare.vn/site/login",
                    "Content-Type": "application/x-www-form-urlencoded",
                    "User-Agent": self.session.headers.get("User-Agent")
                },
                timeout=self.timeout,
                allow_redirects=True
            )
            
            if response.status_code in [200, 302]:
                # If we got a 302, we are almost certainly logged in
                # If we got a 200, check if we are on a page that isn't the login page
                is_logged_in = (response.status_code == 302) or ('/site/logout' in response.text)
                
                if is_logged_in:
                    logger.info("✅ Fshare login successful")
                    self._token = "web_session"
                    self._token_expires = datetime.now() + timedelta(days=7)
                    
                    # Fetch account profile page to extract account information
                    try:
                        profile_response = self.session.get("https://www.fshare.vn/account/profile", timeout=self.timeout)
                        self._parse_profile(profile_response.text)
                    except Exception as e:
                        logger.warning(f"Failed to parse profile after login: {e}")
                        
                    if self._on_session_update:
                        try:
                            self._on_session_update(self)
                        except Exception as e:
                            logger.error(f"Failed to trigger session update callback: {e}")

                    return True
            
            logger.error(f"❌ Login failed. Status: {response.status_code}. Response snippet: {response.text[:500]}")
            raise AuthenticationError(f"Fshare login failed: Invalid credentials or unexpected response")
        except requests.exceptions.RequestException as e:
            logger.error(f"❌ Fshare connection error: {e}")
            raise FshareConnectionError(f"Failed to connect to Fshare: {e}")

    def _parse_profile(self, html: str):
        """Helper to parse profile HTML."""
        # ACCOUNT_TYPE
        # Vietnamese: Loại tài khoản, English: Account type
        account_type_match = re.search(r'(?:Loại tài khoản|Account type).*?<(?:span|div)[^>]*>(.*?)</(?:span|div)>', html, re.IGNORECASE | re.DOTALL)
        if not account_type_match:
            account_type_match = re.search(r'(?:Loại tài khoản|Account type).*?>\s*(.*?)\s*<', html, re.IGNORECASE | re.DOTALL)
            
        if account_type_match:
            self._account_type = account_type_match.group(1).strip()
            self._account_type = re.sub(r'<[^>]+>', '', self._account_type).strip() # Clean HTML tags
            self._is_premium = any(x in self._account_type.upper() for x in ['VIP', 'PREMIUM'])
            logger.info(f"Account type detected: {self._account_type} (Premium: {self._is_premium})")
        else:
            # Fallback checks for VIP indicators
            self._is_premium = any(x in html for x in ['VIP', 'level-vip', 'img alt="VIP"'])
            if self._is_premium:
                self._account_type = "VIP"
            else:
                self._account_type = "Free"
        
        # VALID_UNTIL
        # Vietnamese: Hạn dùng, English: Expiry / Valid until
        valid_until_match = re.search(r'(?:Hạn dùng|Expiry|Valid until).*?<(?:span|div)[^>]*>(.*?)</(?:span|div)>', html, re.IGNORECASE | re.DOTALL)
        if not valid_until_match:
            valid_until_match = re.search(r'(?:Hạn dùng|Expiry|Valid until).*?>\s*(.*?)\s*<', html, re.IGNORECASE | re.DOTALL)
            
        if valid_until_match:
            try:
                import time
                expiry_str = valid_until_match.group(1).strip()
                # Clean up string from tags if any
                expiry_str = re.sub(r'<[^>]+>', '', expiry_str).strip()
                
                # Handle "Lifetime" / "Vĩnh viễn"
                if re.search(r'Vĩnh viễn|Lifetime|Forever', expiry_str, re.IGNORECASE):
                    self._premium_expiry = -1
                    self._is_premium = True
                else:
                    for fmt in ['%d/%m/%Y', '%d-%m-%Y', '%I:%M:%S %p %d-%m-%Y', '%Y-%m-%d %H:%M:%S']:
                        try:
                            expiry_time = time.mktime(time.strptime(expiry_str, fmt))
                            self._premium_expiry = int(expiry_time)
                            break
                        except ValueError:
                            continue
            except Exception as e:
                logger.debug(f"Failed to parse expiry date string '{valid_until_match.group(1)}': {e}")
        elif re.search(r'Vĩnh viễn|Lifetime|Forever', html, re.IGNORECASE):
             self._premium_expiry = -1
             self._is_premium = True
             self._account_type = "VIP (Lifetime)"

        # TRAFFIC
        # Vietnamese: Dung lượng tải | Tải trong ngày, English: Daily download | Traffic left
        traffic_patterns = [
            r'(?:Dung lượng tải|Tải trong ngày|Daily download|Traffic left).*?<(?:span|div|a)[^>]*>(.*?)</(?:span|div|a)>',
            r'(?:Dung lượng tải|Tải trong ngày|Daily download|Traffic left).*?:\s*(?:</a>\s*)?(.*?)\s*(?:</p>|<|$)'
        ]
        
        for pattern in traffic_patterns:
            traffic_match = re.search(pattern, html, re.IGNORECASE | re.DOTALL)
            if traffic_match:
                raw_traffic = traffic_match.group(1).strip()
                raw_traffic = re.sub(r'<[^>]+>', '', raw_traffic).strip()
                self._traffic_left = re.sub(r'\s+', ' ', raw_traffic)
                if self._traffic_left and len(self._traffic_left) < 50: # Sanity check for too large matches
                    break
                else:
                    self._traffic_left = None

    def ensure_authenticated(self, force_login: bool = True) -> bool:
        """
        Ensure we have a valid session, login if needed.
        Wait for any in-progress login to finish (promise-like).
        """
        if self.is_authenticated:
            return True
            
        if not force_login:
            return False

        # Lock to ensure only one thread triggers a login
        # and others wait for it to finish then reuse the result
        with self._login_lock:
            # Re-check after acquiring lock in case another thread logged in
            if self.is_authenticated:
                return True
                
            logger.info("Session expired or missing, logging in...")
            success = self.login()
            return success
    
    def search(self, query: str, limit: int = 50) -> List[FshareFile]:
        """
        DEAD API: Use TimFshare instead.
        """
        logger.warning("FshareClient.search is deprecated (API dead).")
        return []
    
    def get_download_link(self, fcode_or_url: str, password: Optional[str] = None) -> Optional[str]:
        """
        Get direct download link for a file.
        Accepts either a link code (linkcode) or a full Fshare URL.
        """
        self.ensure_authenticated()
        
        url = fcode_or_url
        if "/file/" not in url:
            url = f"https://www.fshare.vn/file/{fcode_or_url}"

        try:
            # Step 1: Load file page
            logger.info(f"Loading file page: {url}")
            page_response = self.session.get(url, timeout=self.timeout)
            
            # CHECK FOR REDIRECT TO LOGIN (Fix for session timeout)
            if "site/login" in page_response.url:
                 logger.warning("Session expired (redirected to login) while loading file page. Re-authenticating...")
                 self._token = None # Clear local token to force ensure_authenticated to re-login
                 if self.ensure_authenticated():
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
            
            if not csrf_token:
                logger.error("Download CSRF token not found")
                return None

            linkcode_match = re.search(r'name="linkcode" value="([^"]+)"', form_html)
            linkcode = linkcode_match.group(1) if linkcode_match else ""
            
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
            
            response = self.session.post(form_action, data=download_data, headers=headers, timeout=self.timeout)
            
            if response.status_code == 200:
                try:
                    res_data = response.json()
                    # The direct link is usually in the 'url' field of the JSON response
                    direct_url = res_data.get("url")
                    if direct_url:
                        logger.info(f"✅ Successfully generated direct link")
                        return direct_url
                    else:
                        logger.error(f"Generate link failed: {res_data.get('msg')}")
                except Exception as e:
                    logger.error(f"Failed to parse download response: {e}")
            else:
                logger.error(f"Download form submission failed with status {response.status_code}")
                
            return None
            
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
            logger.info(f"✅ Successfully retrieved file info via V3 API for {fcode}")
            return info

        # Fallback to HTML scraping if V3 API fails
        logger.warning(f"⚠️  V3 API failed for {fcode}, falling back to HTML scraping...")
        return self.get_file_info_html(url)

    def get_file_info_html(self, url: str) -> Optional[FshareFile]:
        """
        Get file information by scraping the file page HTML.
        """
        try:
            response = self.session.get(url, timeout=self.timeout)
            
            # Check for session expiration (redirect to login)
            if "site/login" in response.url:
                 logger.warning("Session expired (redirected to login) while scraping file info. Re-authenticating...")
                 self._token = None # Clear local token
                 if self.ensure_authenticated():
                      response = self.session.get(url, timeout=self.timeout)
                 else:
                      return None

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
            # Check if we have cookies/session first without forcing a POST login
            self.ensure_authenticated(force_login=False)
            
            response = self.session.get("https://www.fshare.vn/account/profile", timeout=self.timeout)
            
            # If profile page returns nothing or very short, something is wrong, don't try login
            if not response.text or len(response.text) < 100:
                logger.warning("Fshare profile returned no data, skipping login retry")
                return None

            # Check for session expiration (redirect to login) or restricted access
            # If the URL is no longer the profile page, the session is likely dead or restricted.
            if "account/profile" not in response.url or "site/login" in response.url:
                logger.debug("Likely session expiration (redirected to login), returning None.")
                return None

            if response.status_code != 200:
                return None
                
            html = response.text
            # Use same fallback logic as _parse_profile
            traffic_match = re.search(r'(?:Dung lượng tải|Tải trong ngày).*?<\w+[^>]*>(.*?)</\w+>', html, re.IGNORECASE | re.DOTALL)
            if not traffic_match:
                traffic_match = re.search(r'(?:Dung lượng tải|Tải trong ngày).*?:\s*</a>\s*(.*?)\s*</p>', html, re.IGNORECASE | re.DOTALL)
            if not traffic_match:
                traffic_match = re.search(r'(?:Dung lượng tải|Tải trong ngày).*?:\s*(.*?)\s*(?:<|$)', html, re.IGNORECASE | re.DOTALL)
            
            if traffic_match:
                raw_traffic = traffic_match.group(1).strip()
                raw_traffic = re.sub(r'<[^>]+>', '', raw_traffic).strip()
                self._traffic_left = re.sub(r'\s+', ' ', raw_traffic)
                return self._traffic_left
            
            # Fallback: full parse might find it via other patterns
            self._parse_profile(html)
            return self._traffic_left
            
        except Exception as e:
            logger.error(f"Error getting daily quota: {e}")
            return None
    
