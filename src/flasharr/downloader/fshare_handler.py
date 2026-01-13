"""
Fshare Download Handler

Handles Fshare-specific download logic including URL resolution.
"""

import logging
from typing import Optional, Tuple
from dataclasses import dataclass

from ..clients.fshare import FshareClient, FshareFile
from ..core.exceptions import DownloadError, InvalidURLError

logger = logging.getLogger(__name__)


@dataclass
class ResolvedDownload:
    """Resolved download information from Fshare."""
    direct_url: str
    filename: str
    size_bytes: int
    fcode: str
    original_url: str


class FshareDownloadHandler:
    """
    Handles Fshare-specific download logic.
    
    Responsibilities:
    - URL validation
    - Direct download link resolution
    - File info extraction
    
    Example:
        >>> handler = FshareDownloadHandler(fshare_client)
        >>> resolved = await handler.resolve_url("https://www.fshare.vn/file/ABC123")
        >>> print(resolved.direct_url)
    """
    
    FSHARE_URL_PATTERNS = (
        "fshare.vn/file/",
        "www.fshare.vn/file/",
    )
    
    def __init__(self, fshare_client: FshareClient):
        """
        Initialize the handler.
        
        Args:
            fshare_client: Authenticated Fshare client
        """
        self.client = fshare_client
    
    def is_fshare_url(self, url: str) -> bool:
        """
        Check if URL is a valid Fshare URL.
        
        Args:
            url: URL to check
            
        Returns:
            True if valid Fshare URL
        """
        url_lower = url.lower()
        return any(pattern in url_lower for pattern in self.FSHARE_URL_PATTERNS)
    
    def extract_fcode(self, url: str) -> Optional[str]:
        """
        Extract file code from Fshare URL.
        
        Args:
            url: Fshare URL
            
        Returns:
            File code or None if invalid
        """
        if not self.is_fshare_url(url):
            return None
        
        try:
            # Extract everything after /file/
            parts = url.split("/file/")
            if len(parts) < 2:
                return None
            
            # Remove query parameters
            fcode = parts[1].split("?")[0].split("#")[0]
            return fcode if fcode else None
        except Exception:
            return None
    
    def resolve_url(self, url: str) -> Optional[ResolvedDownload]:
        """
        Resolve Fshare URL to direct download link.
        
        Args:
            url: Fshare file URL
            
        Returns:
            ResolvedDownload with direct URL and file info
            
        Raises:
            InvalidURLError: If URL is not a valid Fshare URL
            DownloadError: If resolution fails
        """
        if not self.is_fshare_url(url):
            raise InvalidURLError(f"Not a valid Fshare URL: {url}")
        
        fcode = self.extract_fcode(url)
        if not fcode:
            raise InvalidURLError(f"Could not extract file code from: {url}")
        
        try:
            # Get file info
            file_info = self.client.get_file_info(url)
            if not file_info:
                raise DownloadError(f"Failed to get file info for: {url}")
            
            # Get direct download link
            direct_url = self.client.get_download_link(fcode)
            if not direct_url:
                raise DownloadError(f"Failed to get download link for: {url}")
            
            return ResolvedDownload(
                direct_url=direct_url,
                filename=file_info.name,
                size_bytes=file_info.size,
                fcode=fcode,
                original_url=url,
            )
            
        except (InvalidURLError, DownloadError):
            raise
        except Exception as e:
            logger.error(f"Failed to resolve Fshare URL: {e}")
            raise DownloadError(f"Failed to resolve URL: {e}")
    
    def validate_and_resolve(self, url: str) -> Tuple[bool, Optional[ResolvedDownload], Optional[str]]:
        """
        Validate URL and resolve if valid.
        
        Args:
            url: URL to validate and resolve
            
        Returns:
            Tuple of (success, resolved_download, error_message)
        """
        if not self.is_fshare_url(url):
            return False, None, "Not a valid Fshare URL"
        
        try:
            resolved = self.resolve_url(url)
            return True, resolved, None
        except InvalidURLError as e:
            return False, None, str(e)
        except DownloadError as e:
            return False, None, str(e)
        except Exception as e:
            return False, None, f"Unexpected error: {e}"
