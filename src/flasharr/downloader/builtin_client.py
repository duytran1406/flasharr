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
        account_manager: Optional[Any] = None,
    ):
        """
        Initialize the built-in download client.
        
        Args:
            fshare_client: Authenticated Fshare client (fallback)
            engine: Optional existing download engine
            download_dir: Optional download directory override
            account_manager: Optional AccountManager for dynamic primary account
        """
        self._fshare_fallback = fshare_client
        self.engine = engine or DownloadEngine(max_concurrent=3)
        self.download_dir = download_dir
        self.account_manager = account_manager
        self._fshare_handler_cache = None
        self._cached_client_id = None
        
        # Track task IDs by NZO ID for lookups
        self._nzo_to_task: Dict[str, str] = {}
        
        # Ensure engine is started
        if not self.engine._running:
            # We'll try to start it, but usually it's started in add_download
            # if we're in a sync context here.
            try:
                loop = asyncio.get_running_loop()
                loop.create_task(self.engine.start())
            except RuntimeError:
                pass
    @property
    def fshare_client(self) -> FshareClient:
        """Get the current primary Fshare client."""
        if self.account_manager:
            client = self.account_manager.get_primary_client()
            if client:
                return client
        return self._fshare_fallback

    @property
    def fshare_handler(self) -> FshareDownloadHandler:
        """Get the Fshare handler for the current client."""
        client = self.fshare_client
        # Use email as client ID for caching
        client_id = getattr(client, 'email', str(id(client)))
        
        if self._fshare_handler_cache is None or self._cached_client_id != client_id:
            self._fshare_handler_cache = FshareDownloadHandler(client)
            self._cached_client_id = client_id
            
        return self._fshare_handler_cache


    
    async def add_download(
        self,
        url: str,
        filename: Optional[str] = None,
        package_name: Optional[str] = None,
        category: str = "Uncategorized",
        task_id: Optional[str] = None,
        skip_resolve: bool = False,
    ) -> bool:
        """
        Add a download to the engine.
        
        Args:
            url: Fshare URL or direct download URL
            filename: Target filename
            package_name: Package/group name
            category: Download category
            skip_resolve: If True, skip Fshare URL resolution (assume direct link)
            
        Returns:
            True if added successfully
        """
        try:
            # Check if this is a Fshare URL that needs resolution
            if not skip_resolve and self.fshare_handler.is_fshare_url(url):
                logger.info(f"Resolving Fshare URL: {url}")
                
                # Resolve to direct download link (Offload to thread to avoid blocking loop)
                resolved = await asyncio.to_thread(self.fshare_handler.resolve_url, url)
                
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
            logger.info(f"✅ Download URL: {download_url}")
            
            try:
                await self.engine.add_download(
                    download_url,
                    filename or "download",
                    destination=self.download_dir,
                    task_id=task_id,
                    category=category,
                    package_name=package_name,
                )
                logger.info(f"✅ Download added: {filename}")
                return True
                
            except Exception as e:
                logger.error(f"Error scheduling download: {e}")
                import traceback
                traceback.print_exc()
                return False
                
        except Exception as e:
            logger.error(f"Error adding download: {e}", exc_info=True)
            return False
    

    async def get_queue(self) -> List[Dict[str, Any]]:
        """
        Get current download queue.
        """
        queue = []
        try:
            # Iterate over a copy of tasks to be thread-safe(ish)
            # engine.tasks is a plain dict, so this is fast and non-blocking
            for task_id, task in self.engine.tasks.copy().items():
                 queue.append({
                    "id": task_id,
                    "filename": task.filename,
                    "status": task.state.value,
                    "progress": task.progress.percentage if task.progress else 0,
                    "size_bytes": task.progress.total_bytes if task.progress else 0,
                    "downloaded_bytes": task.progress.downloaded_bytes if task.progress else 0,
                    "speed": task.progress.speed_bytes_per_sec if task.progress else 0,
                    "eta": task.progress.eta_seconds if task.progress else 0,
                    "category": task.category,
                    "created_at": task.created_at.timestamp() if hasattr(task, 'created_at') and task.created_at else None,
                    "added": task.created_at.timestamp() if hasattr(task, 'created_at') and task.created_at else None,
                })
        except Exception as e:
            logger.warning(f"get_queue error: {e}")
        return queue
    
    async def get_status(self) -> Dict[str, Any]:
        """
        Get client status.
        
        Returns:
            Status dictionary
        """
        tasks_list = list(self.engine.tasks.values())
        
        active = sum(1 for t in tasks_list if t.state in (DownloadState.DOWNLOADING, DownloadState.STARTING, DownloadState.EXTRACTING))
        queued = sum(1 for t in tasks_list if t.state in (DownloadState.QUEUED, DownloadState.WAITING))
        paused = sum(1 for t in tasks_list if t.state in (DownloadState.PAUSED, DownloadState.SKIPPED))
        completed = sum(1 for t in tasks_list if t.state in (DownloadState.COMPLETED, DownloadState.FINISHED))
        failed = sum(1 for t in tasks_list if t.state in (DownloadState.FAILED, DownloadState.OFFLINE))
        
        total_speed = sum(
            t.progress.speed_bytes_per_sec
            for t in tasks_list
            if t.state in (DownloadState.DOWNLOADING, DownloadState.EXTRACTING)
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

    async def delete_download(self, task_id: str) -> bool:
        """Delete/Cancel a download."""
        return self.engine.cancel_task(task_id)

    async def pause_download(self, task_id: str) -> bool:
        """Pause a download."""
        return self.engine.pause_task(task_id)

    async def get_counts(self) -> Dict[str, int]:
        """Get counts for UI stats."""
        s = await self.get_status()
        return {
            "active": s["active"],
            "queued": s["queued"],
            "completed": s["completed"],
            "total": s["active"] + s["queued"] + s["paused"] + s["failed"]
        }

    async def resume_download(self, task_id: str) -> bool:
        """Resume a download."""
        return self.engine.resume_task(task_id)
