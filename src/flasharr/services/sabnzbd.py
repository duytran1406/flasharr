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
        skip_resolve: bool = False,
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
        queue_manager: Optional[Any] = None,
    ):
        """
        Initialize the SABnzbd emulator.
        
        Args:
            fshare_client: Client for Fshare API operations (fallback)
            download_client: Client for actual downloads (PyLoad or native)
            parser: Filename parser for normalization
            account_manager: Optional AccountManager for dynamic primary account
            queue_manager: Persistence manager
        """
        self._fshare_fallback = fshare_client
        self.downloader = download_client
        self.parser = parser or FilenameParser()
        self.account_manager = account_manager
        self.queue_manager = queue_manager
        
        # No internal state anymore - rely on Engine + DB
        
        # If queue manager provided, ensure engine is synced if needed
        # (Engine handles its own restoration now)

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
        Restore/Sync state.
        Now handled by Engine locally. Kept for factory compatibility.
        """
        pass

    def _restore_item(self, item):
        pass

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
            # Pass skip_resolve=True because we just resolved it manually above
            success = self.downloader.add_download(
                download_url,
                filename=normalized_filename,
                package_name=parsed.title,
                category=resolved_category,
                task_id=nzo_id,
                skip_resolve=True,
            )
            
            if not success:
                logger.error("Failed to add download to client")
                return None
            
            logger.info(f"✅ Download added via SABnzbd API with NZO ID: {nzo_id}")
            return nzo_id
            
        except Exception as e:
            logger.error(f"Error adding URL: {e}", exc_info=True)
            return None
    
    def sync(self):
        """Sync emulator state. Now a no-op as state is live."""
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
                                     self.complete_item(nzo_id, status=DownloadStatus.COMPLETED)
                                 elif engine_task.state in (DownloadState.FAILED, DownloadState.CANCELLED, DownloadState.OFFLINE):
                                     logger.info(f"Emulator: Detected failure/cancel of {nzo_id} via engine check. State: {engine_task.state}")
                                     self.complete_item(nzo_id, status=DownloadStatus.FAILED)
                                 elif engine_task.state == DownloadState.DOWNLOADING:
                                     # Debugging flapping issue
                                     logger.warning(f"Emulator: Item {nzo_id} MISSING from active_ids but DOWNLOADING in engine! engine_tasks count: {len(self.downloader.engine.tasks)}")
                     except: pass

    def get_queue(self) -> List[Dict[str, Any]]:
        """Get current download queue in SABnzbd format."""
        
        # 1. Get active tasks from Engine (Memory)
        engine_queue = self.downloader.get_queue()
        slots = []
        
        for item in engine_queue:
            # Map engine dict to SABnzbd slot
            # Engine item: {'id', 'filename', 'status', 'progress', 'size_bytes', 'downloaded_bytes', 'speed', 'eta', 'category'}
            
            status_map = {
                'Queued': 'Queued',
                'Starting': 'Running',
                'Downloading': 'Running',
                'Extracting': 'Running',
                'Paused': 'Paused',
                'Completed': 'Completed',
                'Failed': 'Failed',
                'Video': 'Running' # Fallback
            }
            
            status_val = status_map.get(item['status'], item['status'])
            
            # Create QueueItem transiently for logic reuse or just map directly
            q_item = QueueItem(
                nzo_id=item['id'],
                filename=item['filename'],
                original_filename=item['filename'],
                status=DownloadStatus.DOWNLOADING, # Dummy
                mb_total=item['size_bytes'] / (1024*1024) if item['size_bytes'] else 0,
                mb_left=(item['size_bytes'] - item['downloaded_bytes']) / (1024*1024) if item['size_bytes'] else 0,
                priority="Normal",
                category=item['category'],
                speed=item['speed'] or 0.0,
                size_bytes=item['size_bytes'] or 0,
                downloaded_bytes=item['downloaded_bytes'] or 0,
                eta_seconds=item['eta'] or 0,
                percentage=item['progress'] or 0.0
            )

            # Manually override status string for slot
            slot = q_item.to_queue_slot()
            slot['status'] = status_val
            slots.append(slot)

        # 2. Get history from DB (Completed/Failed)
        if self.queue_manager:
             history_items = self.queue_manager.get_history(limit=50)
             for row in history_items:
                 # Check if already in slots (active engine tasks might linger as completed for a moment)
                 if any(s['nzo_id'] == row['id'] for s in slots):
                     continue
                     
                 # Convert DB row to QueueItem-like slot
                 # DB Columns: id, filename, status, category, total_bytes, completed_at, etc.
                 
                 # Basic fields for History display in Queue
                 slot = {
                    "nzo_id": row['id'],
                    "filename": row['filename'],
                    "status": "Completed" if row['state'] == 'Completed' else "Failed",
                    "percentage": "100",
                    "mb": f"{(row['total_bytes'] or 0) / (1024*1024):.2f}",
                    "mbleft": "0.00",
                    "timeleft": "0:00:00",
                    "eta": "0:00:00",
                    "priority": "Normal", # or row['priority']
                    "cat": row['category'],
                    "speed_bytes": 0,
                    "total_bytes": row['total_bytes'] or 0,
                    "downloaded": row['downloaded_bytes'] or 0,
                    "eta_seconds": 0,
                }
                 slots.append(slot)
                 
        return slots

    def get_counts(self) -> Dict[str, int]:
        """Get quick counts for dashboard."""
        
        # Count active/queued from engine
        engine_queue = self.downloader.get_queue()
        active = sum(1 for item in engine_queue if item['status'] in ('Running', 'Downloading', 'Starting', 'Extracting'))
        queued = sum(1 for item in engine_queue if item['status'] in ('Queued', 'Paused'))
        
        # Count history from DB
        history = 0
        if self.queue_manager:
            # We need a proper count query, but for now this is approximation or uses get_statistics
            stats = self.queue_manager.get_statistics()
            history = stats.get('completed', 0) + stats.get('failed', 0)

        return {
            "active": active,
            "queued": queued,
            "history": history,
            "total": active + queued + history
        }
        
    def _calculate_total_size(self):
        # Simplified stats
        slots = self.get_queue()
        total_size = sum(float(s['mb']) for s in slots)
        
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
                "status": "Downloading" if status.get('active', 0) > 0 else "Idle",
                "speed": formatted_speed,
                "size": f"{total_size:.2f}",
                "sizeleft": "0.00", # TODO calculation
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
        """
        slots = []
        if self.queue_manager:
            items = self.queue_manager.get_history(limit=limit)
            for row in items:
                slots.append({
                    "nzo_id": row['id'],
                    "name": row['filename'],
                    "status": "Completed" if row['state'] == 'Completed' else "Failed",
                    "fail_message": row['error_message'] or "",
                    "category": row['category'],
                    "size": f"{(row['total_bytes'] or 0) / (1024*1024):.2f} MB",
                    "completed": row['completed_at'] or "",
                    "storage": str(row['destination']),
                     # Extra fields for UI
                    "download_time": 60, # Dummy
                    "path": str(row['destination']),
                    "url": row['url'],
                    "script": "",
                    "action_line": "",
                })
        
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
        Delete an item from the queue or history and cancel download if active.
        """
        # Cancel in engine (this also deletes file)
        self.downloader.delete_download(nzo_id)
        
        # Also remove from DB History if present
        if self.queue_manager:
            self.queue_manager.delete_task(nzo_id)
            
        return True

    def toggle_item(self, nzo_id: str) -> bool:
        """
        Toggle pause/resume for an item.
        """
        # Pass directly to downloader
        status = self.downloader.get_status() # Not efficient but works
        # Better: get task status
        # Assuming downloader keys task actions
        # We try both resume and pause, engine handles invalid state gracefully
        
        if self.downloader.resume_download(nzo_id):
            return True
        if self.downloader.pause_download(nzo_id):
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
        Mark an item as completed/finished.
        Persistence is now handled by Engine updates.
        This method is kept for Hook consistency (factory callback).
        """
        # Engine already updated DB state to COMPLETED/FAILED.
        # We can just log it.
        logger.info(f"Emulator: Item {nzo_id} completed with status: {status.value}")
        return True
    
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
    
            # Migrated to SQLite. Legacy file ignored.
            pass
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
