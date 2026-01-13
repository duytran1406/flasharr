"""
Account Load Balancer for Multi-Account Support

Distributes downloads across multiple Fshare VIP accounts with quota awareness.
"""

import asyncio
import logging
from typing import Optional, List, Dict
from datetime import datetime, timedelta
from dataclasses import dataclass, field

from .account_manager import AccountManager

logger = logging.getLogger(__name__)


@dataclass
class AccountStatus:
    """Track account usage and availability."""
    email: str
    is_available: bool = True
    current_downloads: int = 0
    total_downloads: int = 0
    quota_exceeded: bool = False
    quota_reset_at: Optional[datetime] = None
    last_error: Optional[str] = None
    last_used: Optional[datetime] = None
    consecutive_failures: int = 0


class AccountLoadBalancer:
    """
    Load balancer for multiple Fshare accounts.
    
    Features:
    - Round-robin distribution
    - Quota awareness and automatic rotation
    - Account health tracking
    - Automatic failover
    """
    
    def __init__(self, account_manager: AccountManager, max_downloads_per_account: int = 2):
        """
        Initialize load balancer.
        
        Args:
            account_manager: Account manager instance
            max_downloads_per_account: Max concurrent downloads per account
        """
        self.account_manager = account_manager
        self.max_downloads_per_account = max_downloads_per_account
        
        self._account_status: Dict[str, AccountStatus] = {}
        self._current_index = 0
        self._lock = asyncio.Lock()
        
        # Initialize status for all accounts
        self._refresh_accounts()
        
        logger.info(f"AccountLoadBalancer initialized with {len(self._account_status)} accounts")
    
    def _refresh_accounts(self) -> None:
        """Refresh account list from account manager."""
        accounts = self.account_manager.list_accounts()
        
        for account in accounts:
            email = account['email']
            if email not in self._account_status:
                self._account_status[email] = AccountStatus(email=email)
        
        # Remove accounts that no longer exist
        current_emails = {acc['email'] for acc in accounts}
        removed = [email for email in self._account_status if email not in current_emails]
        for email in removed:
            del self._account_status[email]
    
    async def acquire_account(self) -> Optional[tuple[str, any]]:
        """
        Acquire an available account for download.
        
        Returns:
            Tuple of (email, client) or None if no accounts available
        """
        async with self._lock:
            self._refresh_accounts()
            
            if not self._account_status:
                logger.error("No accounts available")
                return None
            
            # Try to find available account (round-robin with health checks)
            accounts = list(self._account_status.values())
            attempts = 0
            
            while attempts < len(accounts):
                account = accounts[self._current_index % len(accounts)]
                self._current_index = (self._current_index + 1) % len(accounts)
                attempts += 1
                
                # Check if account is available
                if not self._is_account_available(account):
                    continue
                
                # Try to get client for this account
                try:
                    # Get client from account manager
                    client = await self._get_account_client(account.email)
                    
                    if client:
                        account.current_downloads += 1
                        account.total_downloads += 1
                        account.last_used = datetime.now()
                        account.consecutive_failures = 0
                        
                        logger.info(f"Acquired account {account.email} ({account.current_downloads}/{self.max_downloads_per_account})")
                        return (account.email, client)
                
                except Exception as e:
                    logger.error(f"Failed to acquire account {account.email}: {e}")
                    account.consecutive_failures += 1
                    account.last_error = str(e)
                    
                    # Disable account after 3 consecutive failures
                    if account.consecutive_failures >= 3:
                        account.is_available = False
                        logger.warning(f"Account {account.email} disabled after 3 failures")
            
            logger.warning("No available accounts found")
            return None
    
    async def release_account(self, email: str, success: bool = True, error: Optional[str] = None) -> None:
        """
        Release account after download completion.
        
        Args:
            email: Account email
            success: Whether download was successful
            error: Error message if failed
        """
        async with self._lock:
            account = self._account_status.get(email)
            if not account:
                return
            
            account.current_downloads = max(0, account.current_downloads - 1)
            
            if not success:
                account.last_error = error
                
                # Check for quota exceeded
                if error and "quota" in error.lower():
                    account.quota_exceeded = True
                    account.quota_reset_at = datetime.now() + timedelta(hours=24)
                    logger.warning(f"Account {email} quota exceeded. Reset at {account.quota_reset_at}")
                
                # Check for other errors
                elif error and any(err in error.lower() for err in ['banned', 'suspended', 'invalid']):
                    account.is_available = False
                    logger.error(f"Account {email} disabled: {error}")
            
            logger.debug(f"Released account {email} (success={success}, active={account.current_downloads})")
    
    def _is_account_available(self, account: AccountStatus) -> bool:
        """Check if account is available for new downloads."""
        # Check if account is enabled
        if not account.is_available:
            return False
        
        # Check quota
        if account.quota_exceeded:
            if account.quota_reset_at and datetime.now() >= account.quota_reset_at:
                # Quota should be reset, re-enable
                account.quota_exceeded = False
                account.quota_reset_at = None
                logger.info(f"Account {account.email} quota reset")
            else:
                return False
        
        # Check concurrent download limit
        if account.current_downloads >= self.max_downloads_per_account:
            return False
        
        return True
    
    async def _get_account_client(self, email: str):
        """Get Fshare client for account."""
        # Get account info from manager
        accounts = self.account_manager.list_accounts()
        account = next((acc for acc in accounts if acc['email'] == email), None)
        
        if not account:
            raise Exception(f"Account {email} not found")
        
        # For now, return the primary client if this is the primary account
        # In a full implementation, we'd create a client per account
        primary = self.account_manager.get_primary()
        if primary and primary['email'] == email:
            return self.account_manager.get_primary_client()
        
        # TODO: Implement per-account client creation
        # For now, we'll use the account manager's client factory
        from ..clients.fshare import FshareClient
        from .config import get_config
        
        config = get_config()
        client = FshareClient(
            email=account['email'],
            password=account.get('password', ''),  # Password should be stored securely
            app_key=config.fshare.app_key,
            user_agent=config.fshare.user_agent
        )
        
        # Login
        await client.login()
        return client
    
    def get_stats(self) -> Dict:
        """Get load balancer statistics."""
        total_accounts = len(self._account_status)
        available_accounts = sum(1 for acc in self._account_status.values() if self._is_account_available(acc))
        total_active_downloads = sum(acc.current_downloads for acc in self._account_status.values())
        
        accounts_detail = [
            {
                "email": acc.email,
                "available": self._is_account_available(acc),
                "current_downloads": acc.current_downloads,
                "total_downloads": acc.total_downloads,
                "quota_exceeded": acc.quota_exceeded,
                "quota_reset_at": acc.quota_reset_at.isoformat() if acc.quota_reset_at else None,
                "last_error": acc.last_error,
                "consecutive_failures": acc.consecutive_failures
            }
            for acc in self._account_status.values()
        ]
        
        return {
            "total_accounts": total_accounts,
            "available_accounts": available_accounts,
            "total_active_downloads": total_active_downloads,
            "max_capacity": total_accounts * self.max_downloads_per_account,
            "accounts": accounts_detail
        }
    
    def reset_account(self, email: str) -> bool:
        """
        Manually reset account status.
        
        Args:
            email: Account to reset
            
        Returns:
            True if account was reset
        """
        account = self._account_status.get(email)
        if account:
            account.is_available = True
            account.quota_exceeded = False
            account.quota_reset_at = None
            account.consecutive_failures = 0
            account.last_error = None
            logger.info(f"Account {email} manually reset")
            return True
        return False
