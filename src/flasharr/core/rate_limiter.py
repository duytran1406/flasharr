"""
Rate Limiter using Token Bucket Algorithm

Provides precise per-second rate limiting for bandwidth control.
"""

import asyncio
import time
from typing import Optional
import logging

logger = logging.getLogger(__name__)


class TokenBucket:
    """
    Token Bucket rate limiter for bandwidth control.
    
    The bucket fills with tokens at a constant rate (rate_bytes_per_sec).
    Each download consumes tokens. If no tokens available, download waits.
    """
    
    def __init__(self, rate_bytes_per_sec: Optional[int] = None, burst_size: Optional[int] = None):
        """
        Initialize token bucket.
        
        Args:
            rate_bytes_per_sec: Maximum bytes per second (None = unlimited)
            burst_size: Maximum burst size in bytes (defaults to 2x rate)
        """
        self.rate = rate_bytes_per_sec
        self.burst_size = burst_size or (rate_bytes_per_sec * 2 if rate_bytes_per_sec else None)
        
        self._tokens = self.burst_size if self.burst_size else float('inf')
        self._last_update = time.monotonic()
        self._lock = asyncio.Lock()
        
        logger.info(f"TokenBucket initialized: rate={rate_bytes_per_sec} B/s, burst={self.burst_size} B")
    
    def set_rate(self, rate_bytes_per_sec: Optional[int]) -> None:
        """Update the rate limit."""
        self.rate = rate_bytes_per_sec
        self.burst_size = rate_bytes_per_sec * 2 if rate_bytes_per_sec else None
        self._tokens = min(self._tokens, self.burst_size if self.burst_size else float('inf'))
        logger.info(f"TokenBucket rate updated: {rate_bytes_per_sec} B/s")
    
    async def consume(self, num_bytes: int) -> None:
        """
        Consume tokens for the given number of bytes.
        Wait if insufficient tokens available.
        """
        if not self.rate:
            return
        
        wait_time = 0.0
        
        async with self._lock:
            now = time.monotonic()
            elapsed = now - self._last_update
            self._last_update = now
            
            # Refill tokens
            # If tokens are negative (debt), we still add refill, effectively working off the debt
            new_tokens = self._tokens + (elapsed * self.rate)
            
            # Only clamp to burst size if we are POSITIVE (not paying off debt)
            # Actually, standard logic: min(current + refill, burst)
            # If current is -5, refill 2, burst 10 -> min(-3, 10) = -3. Correct.
            self._tokens = min(new_tokens, self.burst_size)
            
            # Calculate consumption
            self._tokens -= num_bytes
            
            # If negative, we have debt and must wait
            if self._tokens < 0:
                # Debt amount (positive)
                debt = abs(self._tokens)
                wait_time = debt / self.rate
        
        # Sleep outside the lock!
        if wait_time > 0:
            await asyncio.sleep(wait_time)
    
    def get_stats(self) -> dict:
        """Get current bucket statistics."""
        return {
            "rate_limit": self.rate,
            "burst_size": self.burst_size,
            "available_tokens": int(self._tokens),
            "utilization_pct": ((self.burst_size - self._tokens) / self.burst_size * 100) if self.burst_size else 0
        }


class GlobalRateLimiter:
    """
    Global rate limiter for all downloads.
    
    Provides "Slow Mode" to cap total bandwidth usage.
    """
    
    def __init__(self):
        self._bucket: Optional[TokenBucket] = None
        self._enabled = False
    
    def enable(self, rate_bytes_per_sec: int) -> None:
        """Enable rate limiting with specified rate."""
        self._bucket = TokenBucket(rate_bytes_per_sec)
        self._enabled = True
        logger.info(f"Global rate limiter enabled: {rate_bytes_per_sec} B/s")
    
    def disable(self) -> None:
        """Disable rate limiting."""
        self._enabled = False
        self._bucket = None
        logger.info("Global rate limiter disabled")
    
    def update_rate(self, rate_bytes_per_sec: Optional[int]) -> None:
        """Update rate limit."""
        if rate_bytes_per_sec is None:
            self.disable()
        elif self._bucket:
            self._bucket.set_rate(rate_bytes_per_sec)
        else:
            self.enable(rate_bytes_per_sec)
    
    async def consume(self, num_bytes: int) -> None:
        """Consume bandwidth tokens."""
        if self._enabled and self._bucket:
            await self._bucket.consume(num_bytes)
    
    def get_stats(self) -> dict:
        """Get rate limiter statistics."""
        if self._bucket:
            return {
                "enabled": self._enabled,
                **self._bucket.get_stats()
            }
        return {"enabled": False}
    
    @property
    def is_enabled(self) -> bool:
        return self._enabled
