"""
Download Engine Adapter

Adapts the built-in DownloadEngine to work with SABnzbdEmulator.
Integrates FshareDownloadHandler for direct Fshare downloads.
"""

import asyncio
import logging
from typing import Optional, Dict, List, Any

from ..downloader.engine import DownloadEngine, DownloadTask, DownloadState
from ..downloader.fshare_handler import FshareDownloadHandler
from ..clients.fshare import FshareClient

logger = logging.getLogger(__name__)


class BuiltinDownloadClient:
    """
    Adapter for DownloadEngine to match DownloadClientProtocol.
    
    Integrates FshareDownloadHandler for URL resolution and link expiration handling.
    """
    
    def __init__(
        self,
        fshare_client: FshareClient,
        engine: Optional[DownloadEngine] = None,
        download_dir: Optional[str] = None,
    ):
        """
        Initialize the built-in download client.
        
        Args:
            fshare_client: Authenticated Fshare client
            engine: Optional existing download engine
            download_dir: Optional download directory override
        """
        self.fshare_client = fshare_client
        self.engine = engine or DownloadEngine(max_concurrent=3)
        self.fshare_handler = FshareDownloadHandler(fshare_client)
        self.download_dir = download_dir
        
        # Track task IDs by NZO ID for  lookups
        self._nzo_to_task: Dict[str, str] = {}
        
        # Ensure engine is started
        if not self.engine._running:
            asyncio.create_task(self.engine.start())
    
    def add_download(
        self,
        url: str,
        filename: Optional[str] = None,
        package_name: Optional[str] = None,
        category: str = "Uncategorized",
    ) -> bool:
        """
        Add a download to the engine.
        
        Args:
            url: Fshare URL or direct download URL
            filename: Target filename
            package_name: Package/group name
            category: Download category
            
        Returns:
            True if added successfully
        """
        try:
            # Check if this is a Fshare URL that needs resolution
            if self.fshare_handler.is_fshare_url(url):
                logger.info(f"Resolving Fshare URL: {url}")
                
                # Resolve to direct download link
                resolved = self.fshare_handler.resolve_url(url)
                if not resolved:
                    logger.error(f"Failed to resolve Fshare URL: {url}")
                    return False
                
                download_url = resolved.direct_url
                if not filename:
                    filename = resolved.filename
            else:
                # Already a direct URL
                download_url = url
            
            # Add to download engine
            loop = asyncio.get_event_loop()
            if loop.is_running():
                # If event loop is running, schedule the coroutine
                task = asyncio.create_task(self.engine.add_download(
                    download_url,
                    filename or "download",
                    destination=self.download_dir,
                    category=category,
                    package_name=package_name,
                ))
                # We can't await in sync context, but we can get the task ID
                # For now, just log success
                logger.info(f"Scheduled download: {filename}")
                return True
            else:
                # No event loop running - this shouldn't happen in async context
                logger.error("No event loop running - cannot add download")
                return False
                
        except Exception as e:
            logger.error(f"Error adding download: {e}", exc_info=True)
            return False
    
    def get_queue(self) -> List[Dict[str, Any]]:
        """
        Get current download queue.
        
        Returns:
            List of queue items
        """
        queue = []
        
        for task_id, task in self.engine.tasks.items():
            if task.state in (DownloadState.QUEUED, DownloadState.DOWNLOADING, DownloadState.PAUSED):
                queue.append({
                    "id": task_id,
                    "filename": task.filename,
                    "status": task.state.value,
                    "progress": task.progress.percentage,
                    "size": task.progress.total_bytes,
                    "downloaded": task.progress.downloaded_bytes,
                    "speed": task.progress.speed_bytes_per_sec,
                    "eta": task.progress.eta_seconds,
                    "category": task.category,
                })
        
        return queue
    
    def get_status(self) -> Dict[str, Any]:
        """
        Get client status.
        
        Returns:
            Status dictionary
        """
        tasks_list = list(self.engine.tasks.values())
        
        active = sum(1 for t in tasks_list if t.state == DownloadState.DOWNLOADING)
        queued = sum(1 for t in tasks_list if t.state == DownloadState.QUEUED)
        paused = sum(1 for t in tasks_list if t.state == DownloadState.PAUSED)
        completed = sum(1 for t in tasks_list if t.state == DownloadState.COMPLETED)
        failed = sum(1 for t in tasks_list if t.state == DownloadState.FAILED)
        
        total_speed = sum(
            t.progress.speed_bytes_per_sec
            for t in tasks_list
            if t.state == DownloadState.DOWNLOADING
        )
        
        return {
            "active": active,
            "queued": queued,
            "paused": paused,
            "completed": completed,
            "failed": failed,
            "total_speed": total_speed,
            "running": self.engine._running,
        }
