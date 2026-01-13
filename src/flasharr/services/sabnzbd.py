"""
SABnzbd Emulator Service

Implements SABnzbd-compatible API for *arr suite download client integration.
Refactored with proper separation of concerns and typing.
"""

import xml.etree.ElementTree as ET
import uuid
import asyncio
import logging
from datetime import datetime
from typing import Dict, List, Optional, Any, Protocol
from dataclasses import dataclass, field
from enum import Enum
import json
from pathlib import Path

from ..core.exceptions import DownloadError, APIError
from ..utils.filename_parser import FilenameParser, ParsedFilename
from ..downloader.engine import DownloadState

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
    speed: float = 0.0
    size_bytes: int = 0
    downloaded_bytes: int = 0
    eta_seconds: float = 0.0
    added: str = field(default_factory=lambda: datetime.now().isoformat())
    completed: Optional[str] = None
    
    def to_queue_slot(self) -> Dict[str, Any]:
        """Convert to SABnzbd queue slot format."""
        return {
            "nzo_id": self.nzo_id,
            "filename": self.filename,
            "status": "Running" if self.status == DownloadStatus.DOWNLOADING else self.status.value,
            "percentage": str(int(self.percentage)),
            "mb": f"{self.mb_total:.2f}",
            "mbleft": f"{self.mb_left:.2f}",
            "timeleft": self.time_left,
            "eta": self.eta,
            "priority": self.priority,
            "cat": self.category,
            "speed_bytes": self.speed,
            "total_bytes": self.size_bytes,
            "downloaded": self.downloaded_bytes,
            "eta_seconds": self.eta_seconds,
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
        
        # Persistence
        self._queue_file = Path("/app/data/queue.json")
        self._queue_file.parent.mkdir(parents=True, exist_ok=True)
        
        # Load persisted queue
        self._load_queue()

    @property
    def fshare(self) -> FshareClientProtocol:
        """Get the current primary Fshare client."""
        if self.account_manager:
            client = self.account_manager.get_primary_client()
            if client:
                return client
        return self._fshare_fallback

    
    async def restore_state(self):
        """
        Restore active downloads to the engine from the loaded queue.
        Must be called after initialization.
        """
        logger.info("Restoring active downloads...")
        count = 0
        for nzo_id, item in list(self._queue.items()):
            if item.status in (DownloadStatus.DOWNLOADING, DownloadStatus.PAUSED, DownloadStatus.QUEUED):
                if not item.fshare_url:
                    logger.warning(f"Cannot restore {nzo_id}: No Fshare URL")
                    continue
                
                try:
                    # Run in thread to avoid blocking loop with URL resolution
                    await asyncio.to_thread(self._restore_item, item)
                    count += 1
                except Exception as e:
                    logger.error(f"Failed to restore item {nzo_id}: {e}")
                    # If restore fails, maybe mark as failed?
                    item.status = DownloadStatus.FAILED
                    self._save_queue()

        logger.info(f"Restored {count} active downloads")

    def _restore_item(self, item):
        """Helper to restore a single item (runs in thread)."""
        # Re-add to downloader ensuring we use the SAME Task ID
        success = self.downloader.add_download(
            item.fshare_url,
            filename=item.filename,
            category=item.category,
            task_id=item.nzo_id
        )
        
        if success:
            # If it was paused, ensure it stays paused
            if item.status == DownloadStatus.PAUSED:
                self.downloader.pause_download(item.nzo_id)
        else:
            logger.error(f"Failed to re-add download {item.nzo_id} to engine")

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
            
            # Parse and normalize filename (file_info is now a FshareFile object)
            original_name = file_info.name
            parsed = self.parser.parse(original_name)
            normalized_filename = filename or parsed.normalized_filename
            
            # Get direct download link
            fcode = file_info.fcode
            download_url = self.fshare.get_download_link(fcode)
            if not download_url:
                logger.error("Failed to get download link from Fshare")
                return None
            
            # Determine category
            resolved_category = self._resolve_category(category, parsed)
            
            # Generate NZO ID
            nzo_id = str(uuid.uuid4())
            
            # Send to download client
            success = self.downloader.add_download(
                download_url,
                filename=normalized_filename,
                package_name=parsed.title,
                category=resolved_category,
                task_id=nzo_id,
            )
            
            if not success:
                logger.error("Failed to add download to client")
                return None
            size_bytes = file_info.size
            
            queue_item = QueueItem(
                nzo_id=nzo_id,
                filename=normalized_filename,
                original_filename=original_name,
                status=DownloadStatus.DOWNLOADING,
                mb_left=size_bytes / (1024 * 1024),
                mb_total=size_bytes / (1024 * 1024),
                category=resolved_category,
                fshare_url=url,
                size_bytes=size_bytes,
            )
            
            self._queue[nzo_id] = queue_item
            
            logger.info(f"✅ Download started with NZO ID: {nzo_id}")
            self._save_queue()
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
        # Sync with downloader
        downloader_queue = self.downloader.get_queue()
        active_ids = []
        
        for item in downloader_queue:
            nzo_id = item.get('id')
            active_ids.append(nzo_id)
            
            if nzo_id in self._queue:
                q_item = self._queue[nzo_id]
                q_item.percentage = item.get('progress', 0.0)
                
                size_bytes = item.get('size', 0)
                downloaded_bytes = item.get('downloaded', 0)
                
                q_item.mb_total = size_bytes / (1024 * 1024)
                q_item.mb_left = max(0, (size_bytes - downloaded_bytes) / (1024 * 1024))
                
                # Sync raw values
                q_item.size_bytes = size_bytes
                q_item.downloaded_bytes = downloaded_bytes
                q_item.speed = item.get('speed', 0.0)
                q_item.eta_seconds = item.get('eta', 0.0)
                
                # Map engine status to SABnzbd status
                status_str = str(item.get('status')).lower()
                if status_str == "downloading":
                    q_item.status = DownloadStatus.DOWNLOADING
                elif status_str == "queued":
                    q_item.status = DownloadStatus.QUEUED
                elif status_str == "paused":
                    q_item.status = DownloadStatus.PAUSED
                elif status_str == "completed":
                    # Correctly move to history so it doesn't get re-queued on restart
                    if nzo_id in self._queue:
                         self.complete_item(nzo_id)
                         continue # Skip further processing for this item since it's now in history
                elif status_str == "failed":
                    q_item.status = DownloadStatus.FAILED
                    # potentially enable move to history for failed items too?
                    self._save_queue()
                
                # Update time info
                eta_seconds = item.get('eta')
                if eta_seconds and eta_seconds > 0:
                    hours, rem = divmod(int(eta_seconds), 3600)
                    minutes, seconds = divmod(rem, 60)
                    q_item.time_left = f"{hours}:{minutes:02d}:{seconds:02d}"
                    q_item.eta = datetime.now().isoformat() # Placeholder for simplified ETA
            else:
                # Item in downloader but not in emulator? 
                # Could happen if added directly to engine.
                pass
        
        # Check for completed/failed items that are no longer in downloader queue
        current_queue_ids = list(self._queue.keys())
        for nzo_id in current_queue_ids:
            if nzo_id not in active_ids:
                # If it was downloading/queued, it might have finished or failed
                item = self._queue[nzo_id]
                if item.status in (DownloadStatus.DOWNLOADING, DownloadStatus.QUEUED):
                     # Verify if it completed or failed by checking the engine directamente
                     # self.downloader is BuiltinDownloadClient which has access to .engine
                     try:
                         engine = getattr(self.downloader, 'engine', None)
                         if engine:
                             engine_task = engine.get_task(nzo_id)
                             if engine_task:
                                 if engine_task.state == DownloadState.COMPLETED:
                                     logger.info(f"Emulator: Detected completion of {nzo_id} via engine check")
                                     self.complete_item(nzo_id)
                                 elif engine_task.state in (DownloadState.FAILED, DownloadState.CANCELLED, DownloadState.OFFLINE):
                                     logger.info(f"Emulator: Detected failure/cancel of {nzo_id} via engine check")
                                     # Move to history but mark as failed? For now move to history
                                     self.complete_item(nzo_id)
                     except Exception as e:
                         logger.error(f"Error in emulator cleanup loop: {e}")

        # Prepare final slots list
        # Start with all active items in the queue
        all_items = list(self._queue.values())
        
        # Add history items that aren't already in the queue display (limit to recent)
        history_list = list(self._history.values())
        history_list.sort(key=lambda x: x.completed or "", reverse=True)
        
        added_count = 0
        for h_item in history_list:
            if h_item.nzo_id not in self._queue:
                all_items.append(h_item)
                added_count += 1
            if added_count >= 50: # Only show last 50 finished items in the main queue
                break

        slots = [item.to_queue_slot() for item in all_items]
        
        total_size = sum(item.mb_total for item in self._queue.values() if item.status not in (DownloadStatus.COMPLETED, DownloadStatus.FAILED))
        total_left = sum(item.mb_left for item in self._queue.values() if item.status not in (DownloadStatus.COMPLETED, DownloadStatus.FAILED))
        
        # Calculate speed
        status = self.downloader.get_status()
        total_speed = status.get('total_speed', 0)
        formatted_speed = "0 B/s"
        if total_speed > 1024 * 1024:
            formatted_speed = f"{total_speed / (1024 * 1024):.1f} MB/s"
        elif total_speed > 1024:
            formatted_speed = f"{total_speed / 1024:.1f} KB/s"
        else:
            formatted_speed = f"{total_speed} B/s"

        return {
            "queue": {
                "status": "Downloading" if any(i.status == DownloadStatus.DOWNLOADING for i in self._queue.values()) else "Idle",
                "speed": formatted_speed,
                "size": f"{total_size:.2f}",
                "sizeleft": f"{total_left:.2f}",
                "slots": slots,
                "noofslots": len(slots),
            }
        }

    def get_status(self) -> Dict[str, Any]:
        """Get emulator status including downloader engine status."""
        status = self.downloader.get_status()
        
        total_size = sum(item.mb_total for item in self._queue.values() if item.status != DownloadStatus.COMPLETED)
        total_left = sum(item.mb_left for item in self._queue.values() if item.status != DownloadStatus.COMPLETED)
        
        return {
            "active": status.get('active', 0),
            "speed": status.get('total_speed', 0),
            "queued": status.get('queued', 0),
            "total_size": total_size,
            "total_left": total_left,
            "connected": status.get('running', False)
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
        success = True
        for item in list(self._queue.values()):
            if item.status in (DownloadStatus.DOWNLOADING, DownloadStatus.QUEUED):
                if self.downloader.pause_download(item.nzo_id):
                    item.status = DownloadStatus.PAUSED
                else:
                    success = False
        self._save_queue()
        return success
    
    def resume_queue(self) -> bool:
        """Resume the download queue."""
        logger.info("Queue resumed")
        success = True
        for item in list(self._queue.values()):
            if item.status == DownloadStatus.PAUSED:
                if self.downloader.resume_download(item.nzo_id):
                    item.status = DownloadStatus.DOWNLOADING
                else:
                    success = False
        self._save_queue()
        return success

    def stop_all_downloads(self) -> bool:
        """Stop/Cancel all active downloads."""
        logger.info("Stopping all downloads")
        success = True
        for item in list(self._queue.values()):
            if item.status in (DownloadStatus.DOWNLOADING, DownloadStatus.PAUSED, DownloadStatus.QUEUED):
                # Call delete_item which handles cancellation and queue removal
                # But here maybe we just want to stop them but keep in history?
                # Usually Stop All means Cancel All active
                if self.delete_item(item.nzo_id):
                     pass
                else:
                     success = False
        self._save_queue()
        return success
    
    def delete_item(self, nzo_id: str) -> bool:
        """
        Delete an item from the queue and cancel download.
        """
        if nzo_id in self._queue:
            # Tell downloader to cancel
            self.downloader.delete_download(nzo_id)
            del self._queue[nzo_id]
            logger.info(f"Deleted queue item: {nzo_id}")
            self._save_queue()
            return True
        return False

    def toggle_item(self, nzo_id: str) -> bool:
        """
        Toggle pause/resume for an item.
        """
        if nzo_id in self._queue:
            item = self._queue[nzo_id]
            if item.status == DownloadStatus.PAUSED:
                success = self.downloader.resume_download(nzo_id)
                if success:
                    item.status = DownloadStatus.DOWNLOADING
                    self._save_queue()
                    return True
            else:
                success = self.downloader.pause_download(nzo_id)
                if success:
                    item.status = DownloadStatus.PAUSED
                    self._save_queue()
                    return True
        return False
    
    def retry_item(self, nzo_id: str) -> bool:
        """
        Retry a failed or missing download item.
        """
        if nzo_id in self._queue:
            item = self._queue[nzo_id]
            logger.info(f"Retrying download {nzo_id} ({item.filename})")
            
            if not item.fshare_url:
                logger.error("Cannot retry: No Fshare URL saved")
                return False

            # Attempt to re-add to downloader (resolves URL again)
            try:
                success = self.downloader.add_download(
                    item.fshare_url,
                    filename=item.filename,
                    category=item.category,
                    task_id=nzo_id
                )
                
                if success:
                    item.status = DownloadStatus.DOWNLOADING
                    self._save_queue()
                    return True
            except Exception as e:
                logger.error(f"Retry failed: {e}")
                
        return False

    def complete_item(self, nzo_id: str, status: DownloadStatus = DownloadStatus.COMPLETED) -> bool:
        """
        Mark an item as completed/finished and move to history.
        
        Args:
            nzo_id: Item identifier
            status: Final status (COMPLETED or FAILED)
            
        Returns:
            True if completed, False if not found
        """
        if nzo_id in self._queue:
            item = self._queue[nzo_id] # Don't pop, keep in queue as requested by user
            item.status = status
            item.completed = datetime.now().isoformat()
            
            if status == DownloadStatus.COMPLETED:
                item.percentage = 100.0
                item.mb_left = 0.0
                
            self._history[nzo_id] = item
            logger.info(f"Emulator: Item {nzo_id} updated to status: {status.value}")
            self._save_queue()
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
    
    def _save_queue(self) -> None:
        """Save queue and history to disk."""
        try:
            data = {
                "version": 1,
                "queue": {
                    nzo_id: {
                        "nzo_id": item.nzo_id,
                        "filename": item.filename,
                        "original_filename": item.original_filename,
                        "status": item.status.value,
                        "percentage": item.percentage,
                        "mb_left": item.mb_left,
                        "mb_total": item.mb_total,
                        "time_left": item.time_left,
                        "eta": item.eta,
                        "priority": item.priority,
                        "category": item.category,
                        "fshare_url": item.fshare_url,
                        "guid": item.guid,
                        "speed": item.speed,
                        "size_bytes": item.size_bytes,
                        "downloaded_bytes": item.downloaded_bytes,
                        "eta_seconds": item.eta_seconds,
                        "added": item.added,
                        "completed": item.completed,
                    }
                    for nzo_id, item in self._queue.items()
                },
                "history": {
                    nzo_id: {
                        "nzo_id": item.nzo_id,
                        "filename": item.filename,
                        "original_filename": item.original_filename,
                        "status": item.status.value,
                        "percentage": item.percentage,
                        "mb_total": item.mb_total,
                        "category": item.category,
                        "added": item.added,
                        "completed": item.completed,
                    }
                    for nzo_id, item in self._history.items()
                }
            }
            
            with open(self._queue_file, 'w') as f:
                json.dump(data, f, indent=2)
            
            logger.debug(f"Queue saved: {len(self._queue)} items, {len(self._history)} history")
        except Exception as e:
            logger.error(f"Failed to save queue: {e}")
    
    def _load_queue(self) -> None:
        """Load queue and history from disk."""
        if not self._queue_file.exists():
            logger.info("No persisted queue found, starting fresh")
            return
        
        try:
            with open(self._queue_file, 'r') as f:
                data = json.load(f)
            
            # Validate version
            if data.get("version") != 1:
                logger.warning("Queue file version mismatch, ignoring")
                return
            
            # Restore queue
            for nzo_id, item_data in data.get("queue", {}).items():
                try:
                    # Convert status string back to enum
                    status_str = item_data.get("status", "Queued")
                    status = DownloadStatus[status_str.upper()] if hasattr(DownloadStatus, status_str.upper()) else DownloadStatus.QUEUED
                    
                    item = QueueItem(
                        nzo_id=item_data["nzo_id"],
                        filename=item_data["filename"],
                        original_filename=item_data["original_filename"],
                        status=status,
                        percentage=item_data.get("percentage", 0.0),
                        mb_left=item_data.get("mb_left", 0.0),
                        mb_total=item_data.get("mb_total", 0.0),
                        time_left=item_data.get("time_left", "0:00:00"),
                        eta=item_data.get("eta", "unknown"),
                        priority=item_data.get("priority", "Normal"),
                        category=item_data.get("category", "Uncategorized"),
                        fshare_url=item_data.get("fshare_url"),
                        guid=item_data.get("guid"),
                        speed=item_data.get("speed", 0.0),
                        size_bytes=item_data.get("size_bytes", 0),
                        downloaded_bytes=item_data.get("downloaded_bytes", 0),
                        eta_seconds=item_data.get("eta_seconds", 0.0),
                        added=item_data.get("added", datetime.now().isoformat()),
                        completed=item_data.get("completed"),
                    )
                    self._queue[nzo_id] = item
                except Exception as e:
                    logger.warning(f"Failed to restore queue item {nzo_id}: {e}")
            
            # Restore history
            for nzo_id, item_data in data.get("history", {}).items():
                try:
                    status_str = item_data.get("status", "Completed")
                    status = DownloadStatus[status_str.upper()] if hasattr(DownloadStatus, status_str.upper()) else DownloadStatus.COMPLETED
                    
                    mb_total = item_data.get("mb_total", 0.0)
                    item = QueueItem(
                        nzo_id=item_data["nzo_id"],
                        filename=item_data["filename"],
                        original_filename=item_data["original_filename"],
                        status=status,
                        percentage=item_data.get("percentage", 100.0),
                        mb_total=mb_total,
                        size_bytes=int(mb_total * 1024 * 1024),
                        category=item_data.get("category", "Uncategorized"),
                        added=item_data.get("added", datetime.now().isoformat()),
                        completed=item_data.get("completed"),
                    )
                    self._history[nzo_id] = item
                except Exception as e:
                    logger.warning(f"Failed to restore history item {nzo_id}: {e}")
            
            logger.info(f"Queue restored: {len(self._queue)} items, {len(self._history)} history")
        except Exception as e:
            logger.error(f"Failed to load queue: {e}")
