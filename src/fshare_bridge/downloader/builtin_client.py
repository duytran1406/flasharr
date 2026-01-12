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


    
    def add_download(
        self,
        url: str,
        filename: Optional[str] = None,
        package_name: Optional[str] = None,
        category: str = "Uncategorized",
        task_id: Optional[str] = None,
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
            
            # Add to download engine using thread-safe method
            # This works from any thread, including Flask request threads
            logger.info(f"✅ Download URL: {download_url}")
            
            try:
                # Get the engine's event loop
                loop = None
                
                # Try to get from engine first
                if hasattr(self.engine, '_loop') and self.engine._loop:
                    loop = self.engine._loop
                    logger.debug("Using engine's event loop")
                
                # Try to get from Flask app
                if not loop:
                    try:
                        from flask import current_app
                        if hasattr(current_app, 'async_loop'):
                            loop = current_app.async_loop
                            logger.debug("Using Flask app's event loop")
                    except:
                        pass
                
                # Fallback: try to get the running loop
                if not loop:
                    try:
                        loop = asyncio.get_running_loop()
                        logger.debug("Using running event loop")
                    except RuntimeError:
                        pass
                
                if not loop:
                    logger.error("No event loop available for download engine")
                    return False
                
                # Schedule the coroutine in the engine's event loop
                future = asyncio.run_coroutine_threadsafe(
                    self.engine.add_download(
                        download_url,
                        filename or "download",
                        destination=self.download_dir,
                        task_id=task_id,
                        category=category,
                        package_name=package_name,
                    ),
                    loop
                )
                
                # Wait for the result with a timeout
                future.result(timeout=5.0)
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
    def delete_download(self, task_id: str) -> bool:
        """Delete/Cancel a download."""
        return self.engine.cancel_task(task_id)

    def pause_download(self, task_id: str) -> bool:
        """Pause a download."""
        return self.engine.pause_task(task_id)

    def resume_download(self, task_id: str) -> bool:
        """Resume a download."""
        return self.engine.resume_task(task_id)
