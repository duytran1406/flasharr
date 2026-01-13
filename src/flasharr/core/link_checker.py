"""
Smart Link Checker

Pre-verifies link availability before a worker takes a slot.
Prevents wasting worker slots on dead/offline links.
"""

import asyncio
import aiohttp
import logging
from typing import Optional, Dict
from datetime import datetime, timedelta
from enum import Enum

logger = logging.getLogger(__name__)


class LinkStatus(Enum):
    """Link availability status."""
    UNKNOWN = "unknown"
    AVAILABLE = "available"
    OFFLINE = "offline"
    TEMP_OFFLINE = "temp_offline"
    RATE_LIMITED = "rate_limited"
    INVALID = "invalid"


class LinkCheckResult:
    """Result of link check."""
    
    def __init__(
        self,
        status: LinkStatus,
        size_bytes: Optional[int] = None,
        supports_ranges: bool = False,
        error_message: Optional[str] = None
    ):
        self.status = status
        self.size_bytes = size_bytes
        self.supports_ranges = supports_ranges
        self.error_message = error_message
        self.checked_at = datetime.now()
    
    @property
    def is_available(self) -> bool:
        """Check if link is available for download."""
        return self.status == LinkStatus.AVAILABLE
    
    def to_dict(self) -> Dict:
        """Convert to dictionary."""
        return {
            "status": self.status.value,
            "size_bytes": self.size_bytes,
            "supports_ranges": self.supports_ranges,
            "error_message": self.error_message,
            "checked_at": self.checked_at.isoformat()
        }


class LinkChecker:
    """
    Smart link checker with caching.
    
    Features:
    - HEAD request to verify link without downloading
    - Detects file size and range support
    - Caches results to avoid repeated checks
    - Identifies dead links, rate limits, and temp failures
    """
    
    def __init__(self, cache_ttl_seconds: int = 300):
        """
        Initialize link checker.
        
        Args:
            cache_ttl_seconds: How long to cache check results
        """
        self.cache_ttl = timedelta(seconds=cache_ttl_seconds)
        self._cache: Dict[str, LinkCheckResult] = {}
        self._lock = asyncio.Lock()
    
    async def check_link(
        self,
        url: str,
        session: Optional[aiohttp.ClientSession] = None,
        force_recheck: bool = False
    ) -> LinkCheckResult:
        """
        Check if link is available.
        
        Args:
            url: URL to check
            session: Optional aiohttp session (creates temporary one if not provided)
            force_recheck: Force recheck even if cached
            
        Returns:
            LinkCheckResult with availability info
        """
        # Check cache first
        if not force_recheck:
            cached = await self._get_cached(url)
            if cached:
                logger.debug(f"Link check cache hit: {url}")
                return cached
        
        # Perform actual check
        logger.debug(f"Checking link: {url}")
        
        close_session = False
        if not session:
            session = aiohttp.ClientSession()
            close_session = True
        
        try:
            result = await self._perform_check(url, session)
            
            # Cache result
            await self._cache_result(url, result)
            
            return result
        
        finally:
            if close_session:
                await session.close()
    
    async def _perform_check(self, url: str, session: aiohttp.ClientSession) -> LinkCheckResult:
        """Perform actual link check."""
        try:
            # Use HEAD request to check without downloading
            timeout = aiohttp.ClientTimeout(total=10, connect=5)
            
            async with session.head(url, timeout=timeout, allow_redirects=True) as response:
                # Check status code
                if response.status == 200:
                    size_bytes = int(response.headers.get('Content-Length', 0))
                    supports_ranges = response.headers.get('Accept-Ranges', '').lower() == 'bytes'
                    
                    return LinkCheckResult(
                        status=LinkStatus.AVAILABLE,
                        size_bytes=size_bytes if size_bytes > 0 else None,
                        supports_ranges=supports_ranges
                    )
                
                elif response.status == 404:
                    return LinkCheckResult(
                        status=LinkStatus.OFFLINE,
                        error_message="File not found (404)"
                    )
                
                elif response.status == 410:
                    return LinkCheckResult(
                        status=LinkStatus.OFFLINE,
                        error_message="File permanently deleted (410)"
                    )
                
                elif response.status == 429:
                    return LinkCheckResult(
                        status=LinkStatus.RATE_LIMITED,
                        error_message="Rate limited (429)"
                    )
                
                elif response.status in (500, 502, 503, 504):
                    return LinkCheckResult(
                        status=LinkStatus.TEMP_OFFLINE,
                        error_message=f"Server error ({response.status})"
                    )
                
                else:
                    return LinkCheckResult(
                        status=LinkStatus.INVALID,
                        error_message=f"Unexpected status code: {response.status}"
                    )
        
        except asyncio.TimeoutError:
            return LinkCheckResult(
                status=LinkStatus.TEMP_OFFLINE,
                error_message="Connection timeout"
            )
        
        except aiohttp.ClientError as e:
            return LinkCheckResult(
                status=LinkStatus.TEMP_OFFLINE,
                error_message=f"Connection error: {str(e)}"
            )
        
        except Exception as e:
            logger.error(f"Link check failed: {e}")
            return LinkCheckResult(
                status=LinkStatus.INVALID,
                error_message=f"Check failed: {str(e)}"
            )
    
    async def _get_cached(self, url: str) -> Optional[LinkCheckResult]:
        """Get cached result if still valid."""
        async with self._lock:
            if url in self._cache:
                result = self._cache[url]
                age = datetime.now() - result.checked_at
                
                if age < self.cache_ttl:
                    return result
                else:
                    # Expired, remove from cache
                    del self._cache[url]
        
        return None
    
    async def _cache_result(self, url: str, result: LinkCheckResult) -> None:
        """Cache check result."""
        async with self._lock:
            self._cache[url] = result
            
            # Cleanup old entries (simple LRU)
            if len(self._cache) > 1000:
                # Remove oldest 100 entries
                sorted_items = sorted(
                    self._cache.items(),
                    key=lambda x: x[1].checked_at
                )
                for old_url, _ in sorted_items[:100]:
                    del self._cache[old_url]
    
    def clear_cache(self, url: Optional[str] = None) -> None:
        """
        Clear cache.
        
        Args:
            url: Specific URL to clear, or None to clear all
        """
        if url:
            self._cache.pop(url, None)
            logger.debug(f"Cleared cache for {url}")
        else:
            self._cache.clear()
            logger.info("Cleared all link check cache")
    
    def get_stats(self) -> Dict:
        """Get cache statistics."""
        total_cached = len(self._cache)
        
        status_counts = {}
        for result in self._cache.values():
            status = result.status.value
            status_counts[status] = status_counts.get(status, 0) + 1
        
        return {
            "total_cached": total_cached,
            "cache_ttl_seconds": self.cache_ttl.total_seconds(),
            "status_breakdown": status_counts
        }


# Global instance
_link_checker: Optional[LinkChecker] = None


def get_link_checker() -> LinkChecker:
    """Get global link checker instance."""
    global _link_checker
    if _link_checker is None:
        _link_checker = LinkChecker()
    return _link_checker
