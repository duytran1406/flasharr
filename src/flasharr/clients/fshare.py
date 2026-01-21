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
        # Use iteration to avoid CookieConflictError if multiple matching cookies exist
        has_identity = False
        has_session = False
        
        for cookie in self.session.cookies:
            if cookie.name == '_identity-app':
                has_identity = True
            elif cookie.name == 'session_id':
                has_session = True
                
        if has_identity or has_session:
            return True
            
        return False
    
    def validate_session(self) -> bool:
        """
        Validate session by calling Fshare account profile.
        
        This is the ONLY reliable way to check if session is valid on server.
        Unlike is_authenticated which only checks local cookies, this method
        actually verifies the session works on Fshare's servers.
        
        Also updates account info (traffic, premium status, etc.) as a side effect.
        
        Returns:
            True if session is valid, False otherwise
        """
        try:
            response = self.session.get(
                "https://www.fshare.vn/account/profile",
                timeout=self.timeout
            )
            
            # Check if redirected to login (session expired)
            if "site/login" in response.url:
                logger.debug("Session validation failed: redirected to login")
                return False
            
            # Check if we got profile page
            if "account/profile" not in response.url:
                logger.debug(f"Session validation failed: unexpected URL {response.url}")
                return False
            
            # Check status code
            if response.status_code != 200:
                logger.debug(f"Session validation failed: HTTP {response.status_code}")
                return False
            
            # Session is valid! Parse account info as side effect
            try:
                self._parse_profile(response.text)
                logger.debug(f"Session validated: {self._account_type}, Traffic: {self._traffic_left}")
            except Exception as e:
                logger.warning(f"Failed to parse profile during validation: {e}")
                # Still return True since session is valid, just couldn't parse
            
            return True
            
        except Exception as e:
            logger.warning(f"Session validation failed: {e}")
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
            
            # Don't clear cookies - preserve fshare-app session cookie for login continuity
            self._token = None # Clear token
            
            # Step 1: Get CSRF token from login page
            homepage = self.session.get(
                "https://www.fshare.vn/site/login",
                timeout=self.timeout
            )
            
            # Extract CSRF token
            csrf_token = self._extract_csrf_from_html(homepage.text)
            
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
                # Set token optimistically
                self._token = "web_session"
                self._token_expires = datetime.now() + timedelta(days=7)
                
                # Verify login by fetching user info
                # If this succeeds, we know the session is valid
                if self._fetch_user_info():
                    logger.info("✅ Fshare login successful")
                        
                    if self._on_session_update:
                        try:
                            self._on_session_update(self)
                        except Exception as e:
                            logger.error(f"Failed to trigger session update callback: {e}")

                    return True
                else:
                    # Login appeared successful but couldn't fetch user info
                    logger.error("❌ Login verification failed - could not fetch user info")
                    self._token = None
                    raise AuthenticationError(
                        "Unable to login. Please check your email and password. "
                        "If credentials are correct, your account may be blocked or suspended."
                    )
            
            logger.error(f"❌ Login failed. Status: {response.status_code}. Response snippet: {response.text[:500]}")
            raise AuthenticationError(
                "Login failed. Please verify your email and password are correct."
            )
        except requests.exceptions.RequestException as e:
            logger.error(f"❌ Fshare connection error: {e}")
            raise FshareConnectionError(f"Failed to connect to Fshare: {e}")
    
    def _fetch_user_info(self) -> bool:
        """
        Fetch and parse user account information from profile page.
        
        Returns:
            True if successful, False otherwise
        """
        try:
            profile_response = self.session.get(
                "https://www.fshare.vn/account/profile",
                timeout=self.timeout
            )
            
            # Check if redirected to login (session expired/invalid)
            if "site/login" in profile_response.url:
                logger.warning("Failed to fetch profile: redirected to login")
                return False
            
            # Check if we got profile page
            if "account/profile" not in profile_response.url:
                logger.warning(f"Failed to fetch profile: unexpected URL {profile_response.url}")
                return False
            
            if profile_response.status_code != 200:
                logger.warning(f"Failed to fetch profile: HTTP {profile_response.status_code}")
                return False
            
            self._parse_profile(profile_response.text)
            logger.info(f"✅ User info fetched: {self._account_type}, Traffic: {self._traffic_left}")
            return True
            
        except Exception as e:
            logger.warning(f"Failed to fetch user info: {e}")
            return False

    def _extract_csrf_from_html(self, html: str) -> Optional[str]:
        """
        Extract CSRF token from HTML.
        
        Tries multiple patterns to find _csrf-app or csrf-token.
        
        Returns:
            CSRF token string, or None if not found
        """
        patterns = [
            r'name="_csrf-app"\s+value="([^"]+)"',
            r'value="([^"]+)"\s+name="_csrf-app"',
            r'name="csrf-token"\s+content="([^"]+)"',
            r'content="([^"]+)"\s+name="csrf-token"',
        ]
        
        for pattern in patterns:
            match = re.search(pattern, html)
            if match:
                return match.group(1)
        
        return None
    
    def _handle_session_expiry(self, response: requests.Response, original_url: str) -> Optional[requests.Response]:
        """
        Check if response indicates session expiry and re-authenticate.
        
        Args:
            response: The response to check
            original_url: URL to retry after re-authentication
            
        Returns:
            New response after re-auth, or None if re-auth failed
        """
        if "site/login" not in response.url:
            return response
        
        logger.warning("Session expired (redirected to login). Re-authenticating...")
        self._token = None
        
        if not self.ensure_authenticated():
            logger.error("Re-authentication failed")
            return None
        
        # Retry the original request
        logger.info(f"Retrying request to {original_url}")
        return self.session.get(original_url, timeout=self.timeout)

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
            r'(?:Dung lượng tải|Tải trong ngày|Daily download|Traffic left).*?</a>\s*(.*?)\s*</p>',
            r'(?:Dung lượng tải|Tải trong ngày|Daily download|Traffic left).*?<(?:span|div|a|b|strong)[^>]*>(.*?)</(?:span|div|a|b|strong)>',
            r'(?:Dung lượng tải|Tải trong ngày|Daily download|Traffic left)[^:]*:\s*(?:<[^>]+>)*\s*([\d.]+\s*[KMGT]?B)',
            r'(?:Dung lượng tải|Tải trong ngày|Daily download|Traffic left).*?:\s*(?:</a>\s*)?(.*?)\s*(?:</p>|<|$)'
        ]
        
        for pattern in traffic_patterns:
            traffic_match = re.search(pattern, html, re.IGNORECASE | re.DOTALL)
            if traffic_match:
                raw_traffic = traffic_match.group(1).strip()
                # Clean HTML tags recursively
                raw_traffic = re.sub(r'<[^>]+>', '', raw_traffic).strip()
                # Clean extra whitespace
                self._traffic_left = re.sub(r'\s+', ' ', raw_traffic)
                
                # Validation: must look like a size
                if re.match(r'^[\d.]+\s*[KMGT]?B$', self._traffic_left, re.IGNORECASE):
                     break
                elif len(self._traffic_left) < 50:
                     break
                else:
                    self._traffic_left = None
    

    
    def check_quota_available(self, file_size_bytes: int = 0) -> tuple[bool, str]:
        """
        Check if account has enough quota remaining for download.
        
        Applies 10% safety margin since quota is approximate.
        
        Args:
            file_size_bytes: Size of file to download in bytes (0 = just check if any quota left)
            
        Returns:
            Tuple of (has_quota, message)
        """
        # VIP accounts also have quotas! Check them too.
        
        # Parse traffic_left string (format: "5 GB / 10 GB" or "5.2 GB / 10 GB")
        if not self._traffic_left:
            return (False, "Unable to check quota. Please ensure you're logged in and try again.")
        
        try:
            # Extract used and total from "X GB / Y GB" or "0 Bytes / 150 GB" format
            match = re.search(r'([\d.]+)\s*(Bytes|[KMGT]?B)\s*/\s*([\d.]+)\s*(Bytes|[KMGT]?B)', self._traffic_left, re.IGNORECASE)
            if not match:
                # If can't parse, assume quota available (fail open)
                logger.warning(f"Could not parse traffic_left: {self._traffic_left}")
                return (True, f"Quota information available but format unexpected: {self._traffic_left}")
            
            used_val = float(match.group(1))
            used_unit = match.group(2).upper()
            total_val = float(match.group(3))
            total_unit = match.group(4).upper()
            
            # Convert to bytes
            def to_bytes(val: float, unit: str) -> int:
                units = {'B': 1, 'BYTES': 1, 'KB': 1024, 'MB': 1024**2, 'GB': 1024**3, 'TB': 1024**4}
                return int(val * units.get(unit, 1))
            
            used_bytes = to_bytes(used_val, used_unit)
            total_bytes = to_bytes(total_val, total_unit)
            remaining_bytes = total_bytes - used_bytes
            
            # Apply 10% safety margin (quota is approximate)
            safe_remaining = int(remaining_bytes * 0.9)
            
            # Check if enough quota (with 10% safety margin)
            if file_size_bytes > 0:
                if safe_remaining >= file_size_bytes:
                    remaining_gb = safe_remaining / (1024**3)
                    needed_gb = file_size_bytes / (1024**3)
                    return (True, f"Quota OK: {remaining_gb:.2f} GB available (need {needed_gb:.2f} GB, 10% margin)")
                else:
                    remaining_gb = safe_remaining / (1024**3)
                    needed_gb = file_size_bytes / (1024**3)
                    return (False, f"Insufficient quota: {remaining_gb:.2f} GB available (need {needed_gb:.2f} GB, 10% margin)")
            else:
                # Just check if any quota left
                if safe_remaining > 0:
                    remaining_gb = safe_remaining / (1024**3)
                    return (True, f"Quota available: {remaining_gb:.2f} GB (10% margin)")
                else:
                    return (False, f"Daily quota exceeded. Current usage: {self._traffic_left}. Quota resets at midnight (GMT+7).")
                    
        except Exception as e:
            logger.warning(f"Error checking quota: {e}")
            return (True, f"Quota check failed, proceeding anyway: {self._traffic_left}")

    def ensure_authenticated(self, force_login: bool = True) -> bool:
        """
        Ensure we have a valid session, login if needed.
        
        Uses server-side validation via validate_session() to ensure
        the session is actually valid on Fshare's servers.
        """
        # Always validate against server (not just local cookies)
        if self.validate_session():
            return True
            
        if not force_login:
            return False

        # Lock to ensure only one thread triggers a login
        # and others wait for it to finish then reuse the result
        with self._login_lock:
            # Re-check after acquiring lock in case another thread logged in
            if self.validate_session():
                return True
                
            logger.info("Session invalid, logging in...")
            success = self.login()
            return success
    
    def search(self, query: str, limit: int = 50) -> List[FshareFile]:
        """
        DEAD API: Use TimFshare instead.
        """
        logger.warning("FshareClient.search is deprecated (API dead).")
        return []
    
    def _normalize_file_url(self, fcode_or_url: str) -> str:
        """
        Convert fcode to full URL if needed.
        
        Args:
            fcode_or_url: Either a file code or full URL
            
        Returns:
            Full Fshare file URL
        """
        if "/file/" not in fcode_or_url:
            return f"https://www.fshare.vn/file/{fcode_or_url}"
        return fcode_or_url
    
    def _submit_password_form(self, response: requests.Response, url: str, password: str) -> Optional[requests.Response]:
        """
        Submit password for password-protected file.
        
        Args:
            response: Response containing password form
            url: File URL
            password: File password
            
        Returns:
            Response after password submission, or None if failed
        """
        if 'password-form' not in response.text:
            return response
        
        csrf_token = self._extract_csrf_from_html(response.text)
        if not csrf_token:
            logger.error("Password form CSRF token not found")
            return None
        
        password_data = {
            '_csrf-app': csrf_token,
            'DownloadPasswordForm[password]': password,
        }
        
        return self.session.post(url, data=password_data, timeout=self.timeout)
    
    def _extract_download_form_data(self, html: str) -> Optional[Dict[str, str]]:
        """
        Extract download form action, CSRF token, and linkcode from HTML.
        
        Args:
            html: Page HTML containing download form
            
        Returns:
            Dict with 'action', 'csrf_token', 'linkcode', or None if form not found
        """
        # Find download form
        form_match = re.search(r'<form[^>]*id="form-download"[^>]*>(.*?)</form>', html, re.DOTALL)
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
        if form_action.startswith('/'):
            form_action = f"https://www.fshare.vn{form_action}"
        
        # Extract CSRF token
        csrf_token = self._extract_csrf_from_html(html)
        if not csrf_token:
            logger.error("Download CSRF token not found")
            return None
        
        # Extract linkcode
        linkcode_match = re.search(r'name="linkcode" value="([^"]+)"', form_html)
        linkcode = linkcode_match.group(1) if linkcode_match else ""
        
        return {
            'action': form_action,
            'csrf_token': csrf_token,
            'linkcode': linkcode
        }
    
    def get_download_link(self, fcode_or_url: str, password: Optional[str] = None) -> Optional[str]:
        """
        Get direct download link for a file.
        Accepts either a link code (linkcode) or a full Fshare URL.
        
        Note: Authentication is checked automatically when needed.
        If session expires, it will re-authenticate and retry.
        """
        url = self._normalize_file_url(fcode_or_url)

        try:
            # Step 1: Load file page
            logger.info(f"Loading file page: {url}")
            page_response = self.session.get(url, timeout=self.timeout)
            
            # Handle session expiry
            page_response = self._handle_session_expiry(page_response, url)
            if not page_response:
                return None

            if page_response.status_code != 200:
                logger.error(f"Failed to load file page: {page_response.status_code}")
                return None
            
            # Step 2: Handle password if needed
            if password:
                page_response = self._submit_password_form(page_response, url, password)
                if not page_response:
                    return None
            
            # Step 3: Extract download form data
            form_data = self._extract_download_form_data(page_response.text)
            if not form_data:
                return None
            
            # Step 4: Submit download form
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
            
            # Get CSRF token
            csrf_token = self._extract_csrf_from_html(page_response.text)
            
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
                "X-CSRF-Token": form_data['csrf_token'],
                "X-Requested-With": "XMLHttpRequest",
                "User-Agent": self.session.headers.get("User-Agent")
            }
            
            response = self.session.post(form_data['action'], data=download_data, headers=headers, timeout=self.timeout)
            
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
        
        Note: File pages are public, no authentication required.
        """
        try:
            response = self.session.get(url, timeout=self.timeout)
            
            if response.status_code != 200:
                logger.error(f"HTML scraping failed: HTTP {response.status_code}")
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
        
        Automatically handles session expiry and re-authentication.
        """
        try:
            response = self.session.get(
                "https://www.fshare.vn/account/profile",
                timeout=self.timeout
            )
            
            # Handle session expiry (auto-login if needed)
            response = self._handle_session_expiry(
                response,
                "https://www.fshare.vn/account/profile"
            )
            
            if not response:
                logger.warning("Failed to get profile page after re-authentication")
                return None
            
            # If profile page returns nothing or very short, something is wrong
            if not response.text or len(response.text) < 100:
                logger.warning("Fshare profile returned no data")
                return None

            if response.status_code != 200:
                logger.error(f"Profile check failed with status {response.status_code}")
                return None
                
            html = response.text
            
            # Parse full profile to update all account fields
            self._parse_profile(html)
            
            # Return traffic quota (or "Unlimited" for VIP)
            if self._account_type:
                return self._traffic_left or "Unlimited"
                
            return self._traffic_left
            
        except Exception as e:
            logger.error(f"Error getting daily quota: {e}")
            return None
    
