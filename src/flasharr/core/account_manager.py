"""
Account Manager for Multiple Fshare Accounts

Manages multiple Fshare account credentials with primary account selection.
"""

import json
import logging
from pathlib import Path
from typing import List, Dict, Optional
from datetime import datetime

from ..clients.fshare import FshareClient
from ..clients.fshare import FshareClient
from .config import FshareConfig
from ..core.exceptions import AuthenticationError

logger = logging.getLogger(__name__)


class AccountManager:
    """Manage multiple Fshare accounts with persistent storage."""
    
    def __init__(self, storage_path: Optional[Path] = None):
        """
        Initialize account manager.
        
        Args:
            storage_path: Path to JSON file for storing accounts
        """
        if storage_path is None:
            # Use data_dir from config if available, otherwise fallback to default logic
            try:
                from .config import get_config
                config = get_config()
                storage_path = Path(config.data_dir) / "accounts.json"
            except ImportError:
                 # Fallback if config not ready (should rarely happen in this structure)
                 storage_path = Path(__file__).parent.parent.parent.parent / "data" / "accounts.json"
        
        self.storage_path = Path(storage_path)
        self.storage_path.parent.mkdir(parents=True, exist_ok=True)
        
        self.accounts: List[Dict] = []
        self.primary_email: Optional[str] = None
        self._client_cache: Dict[str, FshareClient] = {} # Cache client instances by email
        self._last_quota_sync: float = 0
        
        self._load()
    
    def _load(self):
        """Load accounts from storage."""
        if not self.storage_path.exists():
            logger.info("No existing accounts file, starting fresh")
            return
        
        try:
            with open(self.storage_path, 'r') as f:
                data = json.load(f)
                self.accounts = data.get('accounts', [])
                self.primary_email = data.get('primary_email')
            logger.info(f"Loaded {len(self.accounts)} accounts")
        except Exception as e:
            logger.error(f"Error loading accounts: {e}")
            self.accounts = []
            self.primary_email = None
    
    def _save(self):
        """Save accounts to storage."""
        try:
            data = {
                'accounts': self.accounts,
                'primary_email': self.primary_email
            }
            with open(self.storage_path, 'w') as f:
                json.dump(data, f, indent=2)
            logger.debug("Accounts saved")
        except Exception as e:
            logger.error(f"Error saving accounts: {e}")
    
    def add_account(self, email: str, password: str) -> Dict:
        """
        Add new account and login.
        
        Args:
            email: Account email
            password: Account password
            
        Returns:
            Account info dict
            
        Raises:
            Exception: If login fails
        """
        # Check if already exists
        existing = next((a for a in self.accounts if a['email'] == email), None)
        # If exists, we will update it instead of failing
        
        # Attempt login
        config = FshareConfig(email=email, password=password)
        client = FshareClient.from_config(config)
        
        if not client.login():
            raise Exception("Login failed")
        
        # Create or update account record
        account = {
            'email': email,
            'password': password,
            'premium': client.is_premium,
            'validuntil': getattr(client, 'premium_expiry', None),
            'traffic_left': getattr(client, 'traffic_left', None),
            'account_type': getattr(client, 'account_type', None),
            'is_primary': existing['is_primary'] if existing else (len(self.accounts) == 0),
            'last_refresh': int(datetime.now().timestamp()),
            'cookies': client.get_cookies(),
            'token_expires': client._token_expires.timestamp() if client._token_expires else None
        }
        
        if existing:
            # Update existing in place
            existing.update(account)
            logger.info(f"Updated existing account: {email}")
        else:
            self.accounts.append(account)
            if len(self.accounts) == 1:
                self.primary_email = email
            logger.info(f"Added new account: {email}")
        
        # DEBUG: Log account contents before saving to diagnose missing cookies
        # Mask password for logs
        debug_acc = account.copy()
        if 'password' in debug_acc: debug_acc['password'] = '***'
        if 'cookies' in debug_acc: 
            logger.info(f"Checking cookies for save: {list(debug_acc['cookies'].keys()) if debug_acc['cookies'] else 'EMPTY'}")
        else:
            logger.error("CRITICAL: 'cookies' key MISSING in account dict before save!")
            
        self._save()
        logger.info(f"Added account: {email}")
        
        return self._sanitize_account(account)
    
    def remove_account(self, email: str):
        """Remove account."""
        self.accounts = [a for a in self.accounts if a['email'] != email]
        
        if self.primary_email == email:
            self.primary_email = self.accounts[0]['email'] if self.accounts else None
        
        self._save()
        logger.info(f"Removed account: {email}")
    
    def set_primary(self, email: str):
        """Set primary account."""
        account = next((a for a in self.accounts if a['email'] == email), None)
        if not account:
            raise ValueError(f"Account {email} not found")
        
        # Update all accounts
        for a in self.accounts:
            a['is_primary'] = (a['email'] == email)
        
        self.primary_email = email
        self._save()
        logger.info(f"Set primary account: {email}")
    
    def get_primary(self) -> Optional[Dict]:
        """Get primary account."""
        if not self.primary_email:
            return None
        
        account = next((a for a in self.accounts if a['email'] == self.primary_email), None)
        return self._sanitize_account(account) if account else None
    
    def _handle_session_update(self, client: FshareClient):
        """Handle session update from client and persist to storage."""
        account = next((a for a in self.accounts if a['email'] == client.email), None)
        if not account:
            return
            
        logger.info(f"Persisting updated session for {client.email}")
        account['cookies'] = client.get_cookies()
        if getattr(client, '_token_expires', None):
            account['token_expires'] = client._token_expires.timestamp()
            
        # Update other fields if available
        account['premium'] = client.is_premium
        account['traffic_left'] = client.traffic_left
        account['account_type'] = client.account_type
        account['last_refresh'] = int(datetime.now().timestamp())
        
        self._save()

    def get_primary_client(self) -> Optional[FshareClient]:
        """Get a functional FshareClient for the primary account (cached)."""
        if not self.primary_email:
            return None
        
        # Return from cache if available
        if self.primary_email in self._client_cache:
            return self._client_cache[self.primary_email]
            
        account = next((a for a in self.accounts if a['email'] == self.primary_email), None)
        if not account:
            return None
            
        config = FshareConfig(email=account['email'], password=account['password'])
        client = FshareClient.from_config(config)
        client._on_session_update = self._handle_session_update
        
        # Restore session if available
        if account.get('cookies'):
             client.set_cookies(account['cookies'])
        if account.get('token_expires'):
             try:
                 client._token_expires = datetime.fromtimestamp(account['token_expires'])
                 client._token = "web_session" # Mark as having token
             except:
                 pass
                 
        # Store in cache
        self._client_cache[self.primary_email] = client
        return client
    
    def list_accounts(self) -> List[Dict]:
        """List all accounts (sanitized)."""
        return [self._sanitize_account(a) for a in self.accounts]
    
    def refresh_account(self, email: str) -> Dict:
        """
        Refresh account info by re-logging in.
        
        Args:
            email: Account email
            
        Returns:
            Updated account info
        """
        account = next((a for a in self.accounts if a['email'] == email), None)
        if not account:
            raise ValueError(f"Account {email} not found")
        
        # 1. Init client with existing credentials
        config = FshareConfig(email=email, password=account['password'])
        client = FshareClient.from_config(config)

        # 2. Restore session if available
        if account.get('cookies'):
            client.set_cookies(account['cookies'])
            # Set a dummy token to pass basic auth checks, effectively trusting cookies
            client._token = "web_session" 
        
        # 3. Use ensure_authenticated which handles everything:
        #    - Calls validate_session() to check if session is valid
        #    - If session invalid, calls login() ONCE
        #    - validate_session() also parses profile as side effect
        if not client.ensure_authenticated():
            raise AuthenticationError(f"Failed to authenticate account {email}")

        # Update info from client state
        # Note: If we reused session, client might not have all 'premium' fields populated 
        # unless get_daily_quota does it or we parse it. 
        # The current get_daily_quota ONLY updates _traffic_left.
        # We should probably run the full profile parsing logic even if we don't login.
        # But for now, let's update what we have.
        
        # IMPROVEMENT: If we didn't login, we should probably re-parse the profile to ensure 
        # 'premium' status is current (e.g. if it expired yesterday).
        # Since 'login()' does the heavy lifting of parsing, we might want to call 
        # a dedicated 'refresh_profile()' method on client if available, 
        # or just accept that 'traffic_left' is updated.
        # Given the user request, we just avoid the LOGIN POST. 
        # But we still want accurate data.
        
        # Let's ensure client has updated 'premium' status.
        # If we just called get_daily_quota, we have traffic.
        # We should essentially extract the parsing logic from login() into a separate method
        # or let the client handle it.
        # For this specific step, I will stick to the user's request: Dont force login.
        
        account['premium'] = client.is_premium
        account['validuntil'] = getattr(client, 'premium_expiry', None)
        account['traffic_left'] = getattr(client, 'traffic_left', None)
        account['account_type'] = getattr(client, 'account_type', None)
        account['last_refresh'] = int(datetime.now().timestamp())
        
        # Save session data
        account['cookies'] = client.get_cookies()
        if getattr(client, '_token_expires', None):
            account['token_expires'] = client._token_expires.timestamp()
        
        self._save()
        logger.info(f"Refreshed account: {email}")
        
        return self._sanitize_account(account)

    def refresh_primary_quota(self, force: bool = False) -> Optional[Dict]:
        """
        Get primary account info (returns cached data).
        
        Note: Account data is kept fresh by validate_session() which is called
        on every authentication check. No need for explicit refresh anymore.
        """
        if not self.primary_email:
            return None
        
        return self.get_primary()

    def _sanitize_account(self, account: Optional[Dict]) -> Optional[Dict]:
        """Remove sensitive data from account dict."""
        if not account:
            return None
        
        return {
            'email': account['email'],
            'premium': account.get('premium', False),
            'expiry': account.get('validuntil'),
            'traffic_left': account.get('traffic_left'),
            'account_type': account.get('account_type'),
            'is_primary': account.get('is_primary', False),
            'last_refresh': account.get('last_refresh')
        }

