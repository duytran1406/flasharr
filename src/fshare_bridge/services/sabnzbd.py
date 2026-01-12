"""
SABnzbd Emulator Service

Implements SABnzbd-compatible API for *arr suite download client integration.
Refactored with proper separation of concerns and typing.
"""

import xml.etree.ElementTree as ET
import uuid
import logging
from datetime import datetime
from typing import Dict, List, Optional, Any, Protocol
from dataclasses import dataclass, field
from enum import Enum

from ..core.exceptions import DownloadError, APIError
from ..utils.filename_parser import FilenameParser, ParsedFilename

logger = logging.getLogger(__name__)


class DownloadStatus(Enum):
    """Download status enum."""
    QUEUED = "Queued"
    DOWNLOADING = "Downloading"
    PAUSED = "Paused"
    COMPLETED = "Completed"
    FAILED = "Failed"


@dataclass
class QueueItem:
    """Represents an item in the download queue."""
    nzo_id: str
    filename: str
    original_filename: str
    status: DownloadStatus
    percentage: float = 0.0
    mb_left: float = 0.0
    mb_total: float = 0.0
    time_left: str = "0:00:00"
    eta: str = "unknown"
    priority: str = "Normal"
    category: str = "Uncategorized"
    fshare_url: Optional[str] = None
    guid: Optional[str] = None
    added: str = field(default_factory=lambda: datetime.now().isoformat())
    completed: Optional[str] = None
    
    def to_queue_slot(self) -> Dict[str, Any]:
        """Convert to SABnzbd queue slot format."""
        return {
            "nzo_id": self.nzo_id,
            "filename": self.filename,
            "status": self.status.value,
            "percentage": str(int(self.percentage)),
            "mb": f"{self.mb_total:.2f}",
            "mbleft": f"{self.mb_left:.2f}",
            "timeleft": self.time_left,
            "eta": self.eta,
            "priority": self.priority,
            "cat": self.category,
        }
    
    def to_history_slot(self) -> Dict[str, Any]:
        """Convert to SABnzbd history slot format."""
        return {
            "nzo_id": self.nzo_id,
            "name": self.filename,
            "status": self.status.value,
            "fail_message": "" if self.status == DownloadStatus.COMPLETED else "Download failed",
            "category": self.category,
            "size": f"{self.mb_total:.2f}",
            "completed": self.completed or datetime.now().isoformat(),
        }


class DownloadClientProtocol(Protocol):
    """Protocol for download clients (PyLoad, native, etc.)."""
    
    def add_download(
        self,
        url: str,
        filename: Optional[str] = None,
        package_name: Optional[str] = None,
        category: str = "Uncategorized",
    ) -> bool:
        """Add a download to the client."""
        ...
    
    def get_queue(self) -> List[Dict]:
        """Get current download queue."""
        ...
    
    def get_status(self) -> Dict:
        """Get client status."""
        ...


class FshareClientProtocol(Protocol):
    """Protocol for Fshare client."""
    
    def get_file_info(self, url: str) -> Optional[Dict]:
        """Get file information from URL."""
        ...
    
    def get_download_link(self, fcode: str) -> Optional[str]:
        """Get direct download link."""
        ...


class SABnzbdEmulator:
    """
    SABnzbd-compatible API emulator.
    
    Provides SABnzbd API compatibility for *arr suite applications
    while using Fshare as the actual download source.
    
    Example:
        >>> emulator = SABnzbdEmulator(fshare_client, download_client)
        >>> nzo_id = emulator.add_url("https://www.fshare.vn/file/ABC123")
        >>> queue = emulator.get_queue()
    """
    
    VERSION = "3.5.0"  # Emulated SABnzbd version
    
    def __init__(
        self,
        fshare_client: FshareClientProtocol,
        download_client: DownloadClientProtocol,
        parser: Optional[FilenameParser] = None,
        account_manager: Optional[Any] = None,
    ):
        """
        Initialize the SABnzbd emulator.
        
        Args:
            fshare_client: Client for Fshare API operations (fallback)
            download_client: Client for actual downloads (PyLoad or native)
            parser: Filename parser for normalization
            account_manager: Optional AccountManager for dynamic primary account
        """
        self._fshare_fallback = fshare_client
        self.downloader = download_client
        self.parser = parser or FilenameParser()
        self.account_manager = account_manager

        
        # In-memory storage
        self._queue: Dict[str, QueueItem] = {}
        self._history: Dict[str, QueueItem] = {}

    @property
    def fshare(self) -> FshareClientProtocol:
        """Get the current primary Fshare client."""
        if self.account_manager:
            client = self.account_manager.get_primary_client()
            if client:
                return client
        return self._fshare_fallback

    
    def add_file(
        self,
        nzb_data: bytes,
        filename: str = "download.nzb",
        category: Optional[str] = None,
    ) -> Optional[str]:
        """
        Add a download from NZB file data.
        
        Args:
            nzb_data: Raw NZB file content
            filename: Original NZB filename
            category: Download category
            
        Returns:
            NZO ID if successful, None otherwise
        """
        try:
            # Parse NZB to extract Fshare URL
            fshare_url = self._extract_url_from_nzb(nzb_data)
            if not fshare_url:
                # Try extracting GUID and looking up URL
                guid = self._extract_guid_from_nzb(nzb_data)
                if not guid:
                    logger.error("No Fshare URL or GUID found in NZB")
                    return None
                
                # If we have a GUID but no URL, we can't proceed without a lookup
                logger.error(f"NZB contains GUID {guid} but no URL - cannot resolve")
                return None
            
            return self.add_url(fshare_url, category=category)
            
        except ET.ParseError as e:
            logger.error(f"Failed to parse NZB: {e}")
            return None
        except Exception as e:
            logger.error(f"Error adding file: {e}", exc_info=True)
            return None
    
    def add_url(
        self,
        url: str,
        filename: Optional[str] = None,
        category: Optional[str] = None,
    ) -> Optional[str]:
        """
        Add a download from Fshare URL.
        
        Args:
            url: Fshare file URL
            filename: Optional override filename
            category: Download category
            
        Returns:
            NZO ID if successful, None otherwise
        """
        try:
            logger.info(f"Adding URL: {url}")
            
            # Get file info from Fshare
            file_info = self.fshare.get_file_info(url)
            if not file_info:
                logger.error("Failed to get file info from Fshare")
                return None
            
            # Parse and normalize filename
            original_name = file_info.get("name", "Unknown")
            parsed = self.parser.parse(original_name)
            normalized_filename = filename or parsed.normalized_filename
            
            # Get direct download link
            fcode = file_info.get("fcode", "")
            download_url = self.fshare.get_download_link(fcode)
            if not download_url:
                logger.error("Failed to get download link from Fshare")
                return None
            
            # Determine category
            resolved_category = self._resolve_category(category, parsed)
            
            # Send to download client
            success = self.downloader.add_download(
                download_url,
                filename=normalized_filename,
                package_name=parsed.title,
                category=resolved_category,
            )
            
            if not success:
                logger.error("Failed to add download to client")
                return None
            
            # Generate NZO ID and add to queue
            nzo_id = str(uuid.uuid4())
            size_bytes = file_info.get("size", 0)
            
            queue_item = QueueItem(
                nzo_id=nzo_id,
                filename=normalized_filename,
                original_filename=original_name,
                status=DownloadStatus.DOWNLOADING,
                mb_left=size_bytes / (1024 * 1024),
                mb_total=size_bytes / (1024 * 1024),
                category=resolved_category,
                fshare_url=url,
            )
            
            self._queue[nzo_id] = queue_item
            
            logger.info(f"✅ Download started with NZO ID: {nzo_id}")
            return nzo_id
            
        except Exception as e:
            logger.error(f"Error adding URL: {e}", exc_info=True)
            return None
    
    def get_queue(self) -> Dict[str, Any]:
        """
        Get current download queue in SABnzbd format.
        
        Returns:
            Queue data dictionary
        """
        slots = [item.to_queue_slot() for item in self._queue.values()]
        
        total_size = sum(item.mb_total for item in self._queue.values())
        total_left = sum(item.mb_left for item in self._queue.values())
        
        return {
            "queue": {
                "status": "Downloading" if slots else "Idle",
                "speed": "0",
                "size": f"{total_size:.2f}",
                "sizeleft": f"{total_left:.2f}",
                "slots": slots,
                "noofslots": len(slots),
            }
        }
    
    def get_history(self, limit: int = 50) -> Dict[str, Any]:
        """
        Get download history in SABnzbd format.
        
        Args:
            limit: Maximum number of history items
            
        Returns:
            History data dictionary
        """
        items = list(self._history.values())[:limit]
        slots = [item.to_history_slot() for item in items]
        
        return {
            "history": {
                "slots": slots,
                "noofslots": len(slots),
            }
        }
    
    def get_version(self) -> str:
        """Get emulated SABnzbd version."""
        return self.VERSION
    
    def pause_queue(self) -> bool:
        """Pause the download queue."""
        logger.info("Queue paused")
        for item in self._queue.values():
            if item.status == DownloadStatus.DOWNLOADING:
                item.status = DownloadStatus.PAUSED
        return True
    
    def resume_queue(self) -> bool:
        """Resume the download queue."""
        logger.info("Queue resumed")
        for item in self._queue.values():
            if item.status == DownloadStatus.PAUSED:
                item.status = DownloadStatus.DOWNLOADING
        return True
    
    def delete_item(self, nzo_id: str) -> bool:
        """
        Delete an item from the queue.
        
        Args:
            nzo_id: Item identifier
            
        Returns:
            True if deleted, False if not found
        """
        if nzo_id in self._queue:
            del self._queue[nzo_id]
            logger.info(f"Deleted queue item: {nzo_id}")
            return True
        return False
    
    def complete_item(self, nzo_id: str) -> bool:
        """
        Mark an item as completed and move to history.
        
        Args:
            nzo_id: Item identifier
            
        Returns:
            True if completed, False if not found
        """
        if nzo_id in self._queue:
            item = self._queue.pop(nzo_id)
            item.status = DownloadStatus.COMPLETED
            item.completed = datetime.now().isoformat()
            item.percentage = 100.0
            item.mb_left = 0.0
            self._history[nzo_id] = item
            logger.info(f"Completed queue item: {nzo_id}")
            return True
        return False
    
    def _extract_url_from_nzb(self, nzb_data: bytes) -> Optional[str]:
        """Extract Fshare URL from NZB metadata."""
        try:
            root = ET.fromstring(nzb_data)
            
            # Look for URL in meta tags
            for meta in root.findall(".//{http://www.newzbin.com/DTD/2003/nzb}meta"):
                if meta.get("type") == "fshare_url":
                    return meta.text
            
            return None
        except:
            return None
    
    def _extract_guid_from_nzb(self, nzb_data: bytes) -> Optional[str]:
        """Extract GUID from NZB segment."""
        try:
            root = ET.fromstring(nzb_data)
            
            segment = root.find(".//{http://www.newzbin.com/DTD/2003/nzb}segment")
            if segment is not None and segment.text:
                if segment.text.startswith("fshare-"):
                    return segment.text.replace("fshare-", "")
            
            return None
        except:
            return None
    
    def _resolve_category(self, category: Optional[str], parsed: ParsedFilename) -> str:
        """Resolve download category from input or parsed filename."""
        if not category:
            return "Sonarr" if parsed.is_series else "Radarr"
        
        cat_lower = category.lower()
        if "tv" in cat_lower or "sonarr" in cat_lower:
            return "Sonarr"
        elif "movie" in cat_lower or "radarr" in cat_lower:
            return "Radarr"
        
        return category.capitalize()
