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
            storage_path = Path(__file__).parent.parent.parent.parent / "data" / "accounts.json"
        
        self.storage_path = Path(storage_path)
        self.storage_path.parent.mkdir(parents=True, exist_ok=True)
        
        self.accounts: List[Dict] = []
        self.primary_email: Optional[str] = None
        self._client_cache: Dict[str, FshareClient] = {} # Cache client instances by email
        
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
        
        # Re-login
        config = FshareConfig(email=email, password=account['password'])
        client = FshareClient.from_config(config)
        
        if not client.login():
            raise Exception("Login failed")
        
        # Update info
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
    
    def _sanitize_account(self, account: Optional[Dict]) -> Optional[Dict]:
        """Remove sensitive data from account dict."""
        if not account:
            return None
        
        return {
            'email': account['email'],
            'premium': account.get('premium', False),
            'validuntil': account.get('validuntil'),
            'traffic_left': account.get('traffic_left'),
            'account_type': account.get('account_type'),
            'is_primary': account.get('is_primary', False),
            'last_refresh': account.get('last_refresh')
        }
