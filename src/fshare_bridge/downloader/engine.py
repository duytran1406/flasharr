"""
Download Engine

Core async download engine with concurrent download support and progress tracking.
"""

import asyncio
import aiohttp
import logging
import time
from pathlib import Path
from typing import Optional, Dict, Callable, Any
from dataclasses import dataclass, field
from enum import Enum
from datetime import datetime

from ..core.config import get_config, DownloadConfig
from ..core.exceptions import DownloadError, DownloadFailedError

logger = logging.getLogger(__name__)


class DownloadState(Enum):
    """Download task state."""
    QUEUED = "Queued"
    STARTING = "Starting"
    DOWNLOADING = "Downloading"
    PAUSED = "Paused"
    COMPLETED = "Completed"
    FAILED = "Failed"
    CANCELLED = "Cancelled"


@dataclass
class DownloadProgress:
    """Progress information for a download."""
    downloaded_bytes: int = 0
    total_bytes: int = 0
    speed_bytes_per_sec: float = 0.0
    eta_seconds: float = 0.0
    percentage: float = 0.0
    
    def update(self, downloaded: int, total: int, elapsed_seconds: float) -> None:
        """Update progress calculations."""
        self.downloaded_bytes = downloaded
        self.total_bytes = total
        
        if total > 0:
            self.percentage = (downloaded / total) * 100
        
        if elapsed_seconds > 0:
            self.speed_bytes_per_sec = downloaded / elapsed_seconds
            
            if self.speed_bytes_per_sec > 0 and total > 0:
                remaining = total - downloaded
                self.eta_seconds = remaining / self.speed_bytes_per_sec


@dataclass
class DownloadTask:
    """Represents a download task."""
    id: str
    url: str
    filename: str
    destination: Path
    state: DownloadState = DownloadState.QUEUED
    progress: DownloadProgress = field(default_factory=DownloadProgress)
    error_message: Optional[str] = None
    category: str = "Uncategorized"
    package_name: Optional[str] = None
    created_at: datetime = field(default_factory=datetime.now)
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    
    # Internal state
    _cancel_event: asyncio.Event = field(default_factory=asyncio.Event, repr=False)
    _pause_event: asyncio.Event = field(default_factory=asyncio.Event, repr=False)
    
    def __post_init__(self):
        # Ensure pause event is set (not paused) by default
        self._pause_event.set()
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for serialization."""
        return {
            "id": self.id,
            "url": self.url,
            "filename": self.filename,
            "destination": str(self.destination),
            "state": self.state.value,
            "progress": {
                "downloaded_bytes": self.progress.downloaded_bytes,
                "total_bytes": self.progress.total_bytes,
                "speed": self.progress.speed_bytes_per_sec,
                "eta": self.progress.eta_seconds,
                "percentage": self.progress.percentage,
            },
            "error": self.error_message,
            "category": self.category,
            "package_name": self.package_name,
            "created_at": self.created_at.isoformat(),
            "started_at": self.started_at.isoformat() if self.started_at else None,
            "completed_at": self.completed_at.isoformat() if self.completed_at else None,
        }
    
    def cancel(self) -> None:
        """Request task cancellation."""
        self._cancel_event.set()
    
    def pause(self) -> None:
        """Pause the download."""
        self._pause_event.clear()
        self.state = DownloadState.PAUSED
    
    def resume(self) -> None:
        """Resume the download."""
        self._pause_event.set()
        if self.state == DownloadState.PAUSED:
            self.state = DownloadState.DOWNLOADING
    
    @property
    def is_cancelled(self) -> bool:
        return self._cancel_event.is_set()
    
    @property
    def is_paused(self) -> bool:
        return not self._pause_event.is_set()


ProgressCallback = Callable[[DownloadTask], None]


class DownloadEngine:
    """
    Async download engine with concurrent download support.
    
    Features:
    - Concurrent downloads with configurable limit
    - Progress tracking and callbacks
    - Pause/resume support
    - Resume interrupted downloads
    - Automatic retries
    
    Example:
        >>> engine = DownloadEngine(max_concurrent=3)
        >>> task = await engine.add_download("http://example.com/file.mkv", "file.mkv", "/downloads")
        >>> await engine.start()
    """
    
    def __init__(
        self,
        max_concurrent: int = 3,
        config: Optional[DownloadConfig] = None,
        progress_callback: Optional[ProgressCallback] = None,
    ):
        """
        Initialize the download engine.
        
        Args:
            max_concurrent: Maximum concurrent downloads
            config: Download configuration
            progress_callback: Optional callback for progress updates
        """
        self.config = config or get_config().download
        self.max_concurrent = max_concurrent
        self.progress_callback = progress_callback
        
        self._tasks: Dict[str, DownloadTask] = {}
        self._queue: asyncio.Queue = asyncio.Queue()
        self._workers: list = []
        self._running = False
        self._session: Optional[aiohttp.ClientSession] = None
    
    @property
    def tasks(self) -> Dict[str, DownloadTask]:
        """Get all tasks."""
        return self._tasks.copy()
    
    @property
    def active_count(self) -> int:
        """Get count of active downloads."""
        return sum(
            1 for t in self._tasks.values()
            if t.state in (DownloadState.DOWNLOADING, DownloadState.STARTING)
        )
    
    async def start(self) -> None:
        """Start the download engine."""
        if self._running:
            return
        
        self._running = True
        self._session = aiohttp.ClientSession()
        
        # Start worker tasks
        for i in range(self.max_concurrent):
            worker = asyncio.create_task(self._worker(i))
            self._workers.append(worker)
        
        logger.info(f"Download engine started with {self.max_concurrent} workers")
    
    async def stop(self) -> None:
        """Stop the download engine."""
        self._running = False
        
        # Cancel all workers
        for worker in self._workers:
            worker.cancel()
        
        if self._workers:
            await asyncio.gather(*self._workers, return_exceptions=True)
        
        self._workers.clear()
        
        if self._session:
            await self._session.close()
            self._session = None
        
        logger.info("Download engine stopped")
    
    async def add_download(
        self,
        url: str,
        filename: str,
        destination: Optional[str] = None,
        task_id: Optional[str] = None,
        category: str = "Uncategorized",
        package_name: Optional[str] = None,
    ) -> DownloadTask:
        """
        Add a download to the queue.
        
        Args:
            url: Download URL
            filename: Target filename
            destination: Download directory (uses config default if not provided)
            task_id: Optional custom task ID
            category: Download category
            package_name: Package/group name
            
        Returns:
            DownloadTask object
        """
        import uuid
        
        dest_path = Path(destination or self.config.download_dir) / filename
        
        task = DownloadTask(
            id=task_id or str(uuid.uuid4()),
            url=url,
            filename=filename,
            destination=dest_path,
            category=category,
            package_name=package_name,
        )
        
        self._tasks[task.id] = task
        await self._queue.put(task.id)
        
        logger.info(f"Added download: {filename} -> {dest_path}")
        return task
    
    def get_task(self, task_id: str) -> Optional[DownloadTask]:
        """Get a task by ID."""
        return self._tasks.get(task_id)
    
    def pause_task(self, task_id: str) -> bool:
        """Pause a download task."""
        task = self._tasks.get(task_id)
        if task:
            task.pause()
            return True
        return False
    
    def resume_task(self, task_id: str) -> bool:
        """Resume a paused task."""
        task = self._tasks.get(task_id)
        if task:
            task.resume()
            return True
        return False
    
    def cancel_task(self, task_id: str) -> bool:
        """Cancel a download task."""
        task = self._tasks.get(task_id)
        if task:
            task.cancel()
            return True
        return False
    
    async def _worker(self, worker_id: int) -> None:
        """Worker coroutine for processing downloads."""
        logger.debug(f"Worker {worker_id} started")
        
        while self._running:
            try:
                # Get next task from queue
                task_id = await asyncio.wait_for(
                    self._queue.get(),
                    timeout=1.0,
                )
                
                task = self._tasks.get(task_id)
                if not task:
                    continue
                
                # Process the download
                await self._process_download(task)
                
            except asyncio.TimeoutError:
                continue
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Worker {worker_id} error: {e}")
        
        logger.debug(f"Worker {worker_id} stopped")
    
    async def _process_download(self, task: DownloadTask) -> None:
        """Process a single download task."""
        task.state = DownloadState.STARTING
        task.started_at = datetime.now()
        
        try:
            # Ensure destination directory exists
            task.destination.parent.mkdir(parents=True, exist_ok=True)
            
            # Check for existing partial download
            start_byte = 0
            if task.destination.exists():
                start_byte = task.destination.stat().st_size
                logger.info(f"Resuming download from byte {start_byte}")
            
            # Prepare headers for resume
            headers = {}
            if start_byte > 0:
                headers["Range"] = f"bytes={start_byte}-"
            
            # Start download
            async with self._session.get(task.url, headers=headers) as response:
                if response.status not in (200, 206):
                    raise DownloadFailedError(
                        f"HTTP error {response.status}",
                        {"status": response.status},
                    )
                
                # Get total size
                total_size = int(response.headers.get("Content-Length", 0))
                if start_byte > 0:
                    total_size += start_byte
                
                task.progress.total_bytes = total_size
                task.progress.downloaded_bytes = start_byte
                task.state = DownloadState.DOWNLOADING
                
                # Open file for writing
                mode = "ab" if start_byte > 0 else "wb"
                start_time = time.time()
                
                with open(task.destination, mode) as f:
                    async for chunk in response.content.iter_chunked(self.config.chunk_size):
                        # Check for cancellation
                        if task.is_cancelled:
                            task.state = DownloadState.CANCELLED
                            logger.info(f"Download cancelled: {task.filename}")
                            return
                        
                        # Wait if paused
                        while task.is_paused and not task.is_cancelled:
                            await asyncio.sleep(0.1)
                        
                        # Write chunk
                        f.write(chunk)
                        
                        # Update progress
                        task.progress.downloaded_bytes += len(chunk)
                        elapsed = time.time() - start_time
                        task.progress.update(
                            task.progress.downloaded_bytes,
                            task.progress.total_bytes,
                            elapsed,
                        )
                        
                        # Call progress callback
                        if self.progress_callback:
                            self.progress_callback(task)
            
            # Mark as completed
            task.state = DownloadState.COMPLETED
            task.completed_at = datetime.now()
            task.progress.percentage = 100.0
            
            logger.info(f"Download completed: {task.filename}")
            
        except Exception as e:
            task.state = DownloadState.FAILED
            task.error_message = str(e)
            logger.error(f"Download failed: {task.filename} - {e}")
