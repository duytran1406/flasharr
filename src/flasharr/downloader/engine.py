"""
Download Engine

Core async download engine with concurrent download support and progress tracking.

Beta Features:
- Dynamic segment scaling
- Global bandwidth limiting (Token Bucket)
- Priority queue system
- Multi-account load balancing
- Smart link checking
"""

import asyncio
import aiohttp
import aiofiles
import aiofiles.os
import logging
import time
from pathlib import Path
from typing import Optional, Dict, Callable, Any, List, TYPE_CHECKING
if TYPE_CHECKING:
    from .queue import DownloadQueue
from dataclasses import dataclass, field
from enum import Enum
from datetime import datetime, timedelta

from ..core.config import get_config, DownloadConfig
from ..core.exceptions import DownloadError, DownloadFailedError
from ..core.rate_limiter import GlobalRateLimiter
from ..core.priority_queue import Priority
from ..core.link_checker import get_link_checker, LinkStatus
from ..security.validators import sanitize_filename

logger = logging.getLogger(__name__)


# Smart segment calculation constants
# Assuming ~50 MB/s per connection, we want segments to complete in ~10-30 seconds
MIN_SEGMENT_SIZE = 50 * 1024 * 1024   # 50 MB minimum per segment
MAX_SEGMENT_SIZE = 200 * 1024 * 1024  # 200 MB maximum per segment
MIN_SEGMENTS = 1
MAX_SEGMENTS = 8  # Hard limit (user can configure 1-8)
SMALL_FILE_THRESHOLD = 100 * 1024 * 1024  # Files < 100MB use single stream


def calculate_optimal_segments(file_size: int, user_max: int = 8) -> int:
    """
    Calculate optimal segment count based on file size.
    
    The user's configured max (user_max) acts as the CEILING.
    Smart calculation can return LESS than user's setting but NEVER MORE.
    
    Assumes ~50 MB/s download speed per connection.
    Target: Each segment should take 10-30 seconds to download.
    
    Args:
        file_size: Total file size in bytes
        user_max: User's configured max segments (1-8), acts as ceiling
        
    Returns:
        Optimal number of segments (1 to user_max)
    """
    # Validate user_max
    user_max = max(1, min(user_max, MAX_SEGMENTS))
    
    if file_size <= 0:
        return 1
    
    # Small files: single stream is more efficient
    if file_size < SMALL_FILE_THRESHOLD:
        return 1
    
    # Calculate based on target segment size
    # We want segments between 50MB and 200MB
    optimal = file_size // MIN_SEGMENT_SIZE
    
    # Clamp to reasonable range based on file size
    if file_size < 500 * 1024 * 1024:  # < 500MB
        optimal = min(optimal, 4)
    elif file_size < 1024 * 1024 * 1024:  # < 1GB
        optimal = min(optimal, 8)
    
    # Never exceed user's configured maximum
    optimal = min(optimal, user_max)
    
    # Ensure at least 1
    return max(1, optimal)


class DownloadState(Enum):
    """Download task state."""
    QUEUED = "Queued"
    STARTING = "Starting"
    DOWNLOADING = "Downloading"
    PAUSED = "Paused"
    COMPLETED = "Completed"
    FAILED = "Failed"
    CANCELLED = "Cancelled"
    
    # Extended PyLoad-like states
    WAITING = "Waiting"        # Waiting for retry (IP block, rate limit)
    SKIPPED = "Skipped"        # Skipped by user or logic
    TEMP_OFFLINE = "TempOffline" # Server temporarily unreachable
    EXTRACTING = "Extracting"  # Post-processing/Unpacking
    FINISHED = "Finished"      # Alias for Completed/Post-processed
    OFFLINE = "Offline"        # Link is dead


# Capability Matrix: {State: {action: allowed}}
STATE_CAPABILITIES = {
    DownloadState.QUEUED:      {"pause": True,  "resume": False, "cancel": True,  "retry": False, "delete": True},
    DownloadState.STARTING:    {"pause": False, "resume": False, "cancel": True,  "retry": False, "delete": False},
    DownloadState.DOWNLOADING: {"pause": True,  "resume": False, "cancel": True,  "retry": False, "delete": False},
    DownloadState.WAITING:     {"pause": True,  "resume": True,  "cancel": True,  "retry": True,  "delete": False}, # Resume=Force, Retry=Reset
    DownloadState.PAUSED:      {"pause": False, "resume": True,  "cancel": True,  "retry": False, "delete": True},
    DownloadState.EXTRACTING:  {"pause": False, "resume": False, "cancel": True,  "retry": False, "delete": False},
    DownloadState.COMPLETED:   {"pause": False, "resume": False, "cancel": False, "retry": True,  "delete": True}, # Retry=Restart
    DownloadState.FINISHED:    {"pause": False, "resume": False, "cancel": False, "retry": True,  "delete": True},
    DownloadState.FAILED:      {"pause": False, "resume": False, "cancel": False, "retry": True,  "delete": True},
    DownloadState.CANCELLED:   {"pause": False, "resume": False, "cancel": False, "retry": True,  "delete": True},
    DownloadState.SKIPPED:     {"pause": False, "resume": True,  "cancel": False, "retry": True,  "delete": True},
    DownloadState.TEMP_OFFLINE:{"pause": True,  "resume": True,  "cancel": True,  "retry": True,  "delete": False},
    DownloadState.OFFLINE:     {"pause": False, "resume": False, "cancel": False, "retry": True,  "delete": True},
}


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
    
    # Extended state fields
    wait_until: Optional[datetime] = None
    retry_count: int = 0
    plugin_name: Optional[str] = None
    priority: Priority = Priority.NORMAL
    segments: int = 1  # Number of segments for this task (assigned at queue time)
    
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
            "wait_until": self.wait_until.isoformat() if self.wait_until else None,
            "retry_count": self.retry_count,
            "plugin_name": self.plugin_name,
            "actions": self.get_available_actions(),
        }
    
    def get_available_actions(self) -> List[str]:
        """Get list of available actions for current state."""
        caps = STATE_CAPABILITIES.get(self.state, {})
        return [action for action, allowed in caps.items() if allowed]
    
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
    - Multi-threaded (segmented) downloads per file
    - Progress tracking and callbacks
    - Pause/resume support
    - Resume interrupted downloads
    - Automatic retries
    """
    
    def __init__(
        self,
        max_concurrent: int = 2,
        segments_per_download: int = 8,
        config: Optional[DownloadConfig] = None,
        progress_callback: Optional[ProgressCallback] = None,
        queue_manager: Optional['DownloadQueue'] = None,
    ):
        """
        Initialize the download engine.
        
        Args:
            max_concurrent: Maximum concurrent files (minimum 1)
            segments_per_download: Default number of connections per file
            config: Download configuration
            progress_callback: Optional callback for progress updates
            queue_manager: Optional persistence manager
        """
        self.config = config or get_config().download
        self.max_concurrent = max(1, max_concurrent)  # Enforce minimum of 1
        self.segments_per_download = max(1, segments_per_download)  # Default for new tasks
        self.progress_callback = progress_callback
        self.queue_manager = queue_manager
        
        self._tasks: Dict[str, DownloadTask] = {}
        self._queue: asyncio.Queue = asyncio.Queue()
        self._workers: list = []
        self._running = False
        self._session: Optional[aiohttp.ClientSession] = None
        self._file_locks: Dict[str, asyncio.Lock] = {}
        self._rate_limiter = GlobalRateLimiter()
        
        # Apply speed limit if configured
        if self.config.speed_limit:
            self._rate_limiter.enable(self.config.speed_limit)
            
        # Executor for file I/O
        self._io_executor = None # Use default executor (ThreadPoolExecutor)
        
        logger.info(f"Engine initialized: max_concurrent={self.max_concurrent}, default_segments={self.segments_per_download}")

    
    @property
    def tasks(self) -> Dict[str, DownloadTask]:
        """Get all tasks."""
        return self._tasks.copy()
    
    @property
    def active_count(self) -> int:
        """Get count of active downloads."""
        return sum(
            1 for t in self._tasks.values()
            if t.state in (DownloadState.DOWNLOADING, DownloadState.STARTING, DownloadState.EXTRACTING)
        )
    
    def get_engine_stats(self) -> Dict[str, Any]:
        """Get current engine statistics."""
        active_tasks = [t for t in self._tasks.values() if t.state in (DownloadState.DOWNLOADING, DownloadState.STARTING, DownloadState.EXTRACTING)]
        total_speed = sum(t.progress.speed_bytes_per_sec for t in active_tasks if t.progress)
        
        stats = {
            "active_downloads": len(active_tasks),
            "queue_size": sum(1 for t in self._tasks.values() if t.state in (DownloadState.QUEUED, DownloadState.WAITING)),
            "total_tasks": len(self._tasks),
            "total_speed": total_speed
        }

        if self._rate_limiter.is_enabled:
            stats["rate_limiter"] = self._rate_limiter.get_stats()
        
        return stats
        
    async def restore_from_repository(self) -> None:
        """Restore queued/interrupted tasks from repository."""
        if not self.queue_manager:
            return
            
        logger.info("Restoring tasks from repository...")
        
        # 1. Get pending tasks (Queued, Paused)
        pending = self.queue_manager.get_pending_tasks(limit=1000)
        count = 0
        
        for row in pending:
            # Reconstruct Task object
            # Note: We need to handle type conversion carefully
            try:
                task_id = row['id']
                if task_id in self._tasks:
                    continue
                    
                # Create base task
                state_enum = DownloadState(row['state']) if row['state'] in [s.value for s in DownloadState] else DownloadState.QUEUED
                
                # If state was transient active, force to Paused so it doesn't get lost
                if state_enum in (DownloadState.DOWNLOADING, DownloadState.STARTING, DownloadState.EXTRACTING):
                    logger.warning(f"Found orphaned active task {task_id} in state {state_enum}. Resetting to PAUSED.")
                    state_enum = DownloadState.PAUSED
                    
                task = DownloadTask(
                    id=task_id,
                    url=row['url'],
                    filename=row['filename'],
                    destination=Path(row['destination']),
                    state=state_enum,
                    category=row['category'],
                    package_name=row['package_name'],
                )
                
                # Restore progress
                task.progress.downloaded_bytes = row['downloaded_bytes'] or 0
                task.progress.total_bytes = row['total_bytes'] or 0
                if task.progress.total_bytes > 0:
                     task.progress.percentage = (task.progress.downloaded_bytes / task.progress.total_bytes) * 100
                
                # Restore metadata
                if row['created_at']: task.created_at = datetime.fromisoformat(row['created_at']) if isinstance(row['created_at'], str) else row['created_at']
                if row['retry_count']: task.retry_count = row['retry_count']
                
                self._tasks[task.id] = task
                
                # Determine where to put it
                if task.state == DownloadState.QUEUED:
                    await self._queue.put(task.id)
                    count += 1
                elif task.state == DownloadState.PAUSED:
                    task.pause() # Ensure event is cleared
                    count += 1
                # If it was 'Downloading' when it crashed, we should probably reset to Queued or Paused?
                # For safety, let's set to Paused so user can verify
                    
            except Exception as e:
                logger.error(f"Failed to restore task {row.get('id')}: {e}")
                
        # 2. Get active tasks (Crashed while Downloading)
        # These are effectively 'pending' but marked as Downloading in DB
        active = self.queue_manager.get_active_tasks()
        for row in active:
             try:
                task_id = row['id']
                if task_id in self._tasks:
                    continue
                
                task = DownloadTask(
                    id=task_id,
                    url=row['url'],
                    filename=row['filename'],
                    destination=Path(row['destination']),
                    state=DownloadState.PAUSED, # Reset to PAUSED for safety
                    category=row['category']
                )
                 # Restore progress
                task.progress.downloaded_bytes = row['downloaded_bytes'] or 0
                task.progress.total_bytes = row['total_bytes'] or 0
                if task.progress.total_bytes > 0:
                     task.progress.percentage = (task.progress.downloaded_bytes / task.progress.total_bytes) * 100
                
                task.pause()
                self._tasks[task.id] = task
                count += 1
                
                logger.info(f"Restored crashed task {task.filename} as PAUSED")
             except Exception as e:
                 logger.error(f"Failed to restore active task: {e}")

        logger.info(f"Engine: Restored {count} tasks from DB")
    
    async def start(self) -> None:
        """Start the download engine."""
        if self._running:
            return
        
        self._running = True
        self._loop = asyncio.get_running_loop()
        logger.info(f"Engine: Starting with {self.max_concurrent} concurrent workers")

        # Use a browser-like User-Agent to avoid speed caps/throttling
        headers = {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            "Accept": "*/*",
            "Accept-Encoding": "identity",
            "Connection": "keep-alive"
        }
        
        # Configure connection pooling
        connector = aiohttp.TCPConnector(
            limit=100,           # Total connections
            limit_per_host=10,   # Connections per host
            ttl_dns_cache=300
        )
        
        self._session = aiohttp.ClientSession(
            headers=headers,
            connector=connector,
            timeout=aiohttp.ClientTimeout(total=None, connect=30, sock_read=30)
        )
        
        # Start worker tasks
        for i in range(self.max_concurrent):
            worker = asyncio.create_task(self._worker(i))
            self._workers.append(worker)
            
        # Start scheduler task
        self._scheduler_task = asyncio.create_task(self._scheduler())
        
        logger.info(f"Download engine started with {len(self._workers)} workers and scheduler")
        
    def set_speed_limit(self, speed_limit_bytes: Optional[int]) -> None:
        """
        Update the global speed limit.
        
        Args:
            speed_limit_bytes: Max speed in Bytes/s, or None for unlimited.
        """
        if speed_limit_bytes and speed_limit_bytes > 0:
            self._rate_limiter.enable(speed_limit_bytes)
        else:
            self._rate_limiter.disable()
        
        # Update config to reflect state (optional, for persistent access in this session)
        self.config.speed_limit = speed_limit_bytes
        logger.info(f"Global speed limit updated to: {speed_limit_bytes} B/s")
        
    def update_max_concurrent(self, new_max: int) -> None:
        """
        Update the maximum concurrent downloads at runtime.
        """
        new_max = max(1, new_max)  # Enforce minimum of 1
        
        if new_max == self.max_concurrent:
            return
        
        old_max = self.max_concurrent
        self.max_concurrent = new_max
        
        if not self._running:
            return
            
        if new_max > old_max:
            # Add more workers
            for i in range(old_max, new_max):
                worker = asyncio.create_task(self._worker(i))
                self._workers.append(worker)
            logger.info(f"Engine: Added {new_max - old_max} workers (Total: {new_max})")
        else:
            # Cancel excess workers
            to_remove = self._workers[new_max:]
            self._workers = self._workers[:new_max]
            for w in to_remove:
                w.cancel()
            logger.info(f"Engine: Removed {old_max - new_max} workers (Total: {new_max})")
    
    async def stop(self) -> None:
        """Stop the download engine."""
        self._running = False
        
        # Cancel all workers
        for worker in self._workers:
            worker.cancel()
            
        # Cancel scheduler
        if hasattr(self, '_scheduler_task'):
            self._scheduler_task.cancel()
            try:
                await self._scheduler_task
            except asyncio.CancelledError:
                pass
        
        if self._workers:
            await asyncio.gather(*self._workers, return_exceptions=True)
        
        self._workers.clear()
        
        if self._session:
            await self._session.close()
            self._session = None
        
        logger.info("Download engine stopped")

    async def _scheduler(self) -> None:
        """Scheduler loop to manage WAITING tasks and other periodic jobs."""
        logger.info("Scheduler started")
        while self._running:
            try:
                now = datetime.now()
                for task_id, task in self._tasks.items():
                    # Check for WAITING tasks that are ready
                    if task.state == DownloadState.WAITING and task.wait_until:
                        if now >= task.wait_until:
                            logger.info(f"Task {task.filename} ready for retry. Re-queueing.")
                            task.state = DownloadState.QUEUED
                            task.wait_until = None
                            await self._queue.put(task.id)
                            
                await asyncio.sleep(1.0)
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Scheduler error: {e}")
                await asyncio.sleep(5.0)
            
            # Periodic Persistence Sync (every second)
            if self.queue_manager and self._running:
                try:
                    # Sync active tasks to DB
                    active_tasks = [
                        t for t in self._tasks.values() 
                        if t.state in (DownloadState.DOWNLOADING, DownloadState.EXTRACTING, DownloadState.STARTING)
                    ]
                    for task in active_tasks:
                        self.queue_manager.update_task(task)
                except Exception as e:
                    logger.error(f"Persistence sync error: {e}")
                    
    async def _sync_to_db(self, task: DownloadTask) -> None:
        """Helper to sync task to DB in executor."""
        if not self.queue_manager:
            return
            
        try:
            # We use the io_executor (ThreadPool) to run the sync DB call
            await self._loop.run_in_executor(
                self._io_executor,
                self.queue_manager.update_task,
                task
            )
        except Exception as e:
            logger.error(f"DB Sync error for {task.id}: {e}")

    async def _add_to_db(self, task: DownloadTask) -> None:
        """Helper to add task to DB in executor."""
        if not self.queue_manager:
            return
            
        try:
             await self._loop.run_in_executor(
                self._io_executor,
                self.queue_manager.add_task,
                task
            )
        except Exception as e:
            logger.error(f"DB Add error for {task.id}: {e}")
    
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
            filename: Target filename (will be sanitized)
            destination: Download directory (uses config default if not provided)
            task_id: Optional custom task ID
            category: Download category
            package_name: Package/group name
            
        Returns:
            DownloadTask object
        """
        import uuid
        
        # Sanitize filename to prevent path traversal
        try:
            safe_filename = sanitize_filename(filename)
        except ValueError as e:
            logger.error(f"Invalid filename '{filename}': {e}")
            raise ValueError(f"Invalid filename: {e}")
        
        dest_path = Path(destination or self.config.download_dir) / safe_filename
        
        task = DownloadTask(
            id=task_id or str(uuid.uuid4()),
            url=url,
            filename=filename,
            destination=dest_path,
            category=category,
            package_name=package_name,
            segments=self.segments_per_download,  # Capture current setting at queue time
        )
        
        self._tasks[task.id] = task
        
        # Create placeholder file immediately
        try:
            if not dest_path.exists():
                dest_path.parent.mkdir(parents=True, exist_ok=True)
                dest_path.touch()
                logger.debug(f"Created placeholder file: {dest_path}")
        except Exception as e:
            logger.error(f"Failed to create placeholder for {filename}: {e}")
            # Non-critical failure, continue

        # Persist to DB
        await self._add_to_db(task)

        await self._queue.put(task.id)
        
        logger.info(f"Added download: {filename} -> {dest_path} (segments={task.segments})")
        return task
    
    def get_task(self, task_id: str) -> Optional[DownloadTask]:
        """Get a task by ID."""
        return self._tasks.get(task_id)
    
    def pause_task(self, task_id: str) -> bool:
        """Pause a download task."""
        task = self._tasks.get(task_id)
        if task:
            if "pause" not in task.get_available_actions():
                logger.warning(f"Cannot pause task {task_id} in state {task.state}")
                return False
            task.pause()
            return True
        return False
    
    def resume_task(self, task_id: str) -> bool:
        """Resume a paused task."""
        task = self._tasks.get(task_id)
        if task:
            if "resume" not in task.get_available_actions():
                logger.warning(f"Cannot resume task {task_id} in state {task.state}")
                return False
                
            # Special handling for WAITING state (Skip Wait / Force Start)
            if task.state == DownloadState.WAITING:
                task.wait_until = None # Clear wait
                task.state = DownloadState.QUEUED
                logger.info(f"Forced resume (Skip Wait) for task {task.filename}")
                asyncio.run_coroutine_threadsafe(self._queue.put(task.id), self._loop or asyncio.get_running_loop())
                return True
                
            task.resume()
            return True
        return False
    
    def cancel_task(self, task_id: str) -> bool:
        """Cancel a download task and delete file from disk."""
        task = self._tasks.get(task_id)
        if task:
            if "cancel" not in task.get_available_actions():
                logger.warning(f"Cannot cancel task {task_id} in state {task.state}")
                # return False # Allow force delete even if state seems weird?
            
            # 1. Stop the task logic
            task.cancel()
            
            # 2. Delete file from disk
            try:
                if task.destination.exists():
                    if task.destination.is_dir():
                         import shutil
                         shutil.rmtree(task.destination)
                    else:
                        task.destination.unlink()
                    logger.info(f"Deleted file for task {task_id}: {task.destination}")
                
                # Cleanup any partial segments if they exist
                # ... (segments handled naturally if they are temp files, but if in main dir, might need cleanup)
            except Exception as e:
                logger.error(f"Failed to delete file for task {task_id}: {e}")
                
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
                    logger.warning(f"Worker {worker_id}: Picked up non-existent task {task_id}")
                    continue
                
                logger.info(f"Worker {worker_id}: Processing task {task.filename} ({task_id})")
                # Process the task based on state
                await self._process_task(task)
                logger.info(f"Worker {worker_id}: Finished task {task.filename}")
                
            except asyncio.TimeoutError:
                continue
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Worker {worker_id} error: {e}")
        
        logger.debug(f"Worker {worker_id} stopped")

    async def _process_task(self, task: DownloadTask) -> None:
        """Dispatch task processing based on state."""
        try:
            if task.state == DownloadState.QUEUED:
                await self._handle_downloading(task)
            elif task.state == DownloadState.DOWNLOADING:
                await self._handle_downloading(task)
            elif task.state == DownloadState.EXTRACTING:
                await self._handle_extracting(task)
            # Add other handlers here
            else:
                logger.warning(f"Unknown state for processing: {task.state}")
                
        except Exception as e:
            logger.error(f"Error processing task {task.id}: {e}")
            task.state = DownloadState.FAILED
            task.error_message = str(e)
        finally:
            # Always notify on state transition/completion
            if self.progress_callback:
                try:
                    self.progress_callback(task)
                except Exception as e:
                    logger.error(f"Error in progress callback: {e}")

    async def _handle_extracting(self, task: DownloadTask) -> None:
        """Handle extraction state."""
        logger.info(f"Starting extraction for {task.filename} (Simulated)")
        # TODO: Implement actual extraction logic
        await asyncio.sleep(1.0)
        task.state = DownloadState.COMPLETED
        task.completed_at = datetime.now()
        logger.info(f"Extraction completed for {task.filename}")

    async def _handle_downloading(self, task: DownloadTask) -> None:
        """Process a single download task."""
        task.state = DownloadState.STARTING
        task.started_at = datetime.now()
        
        try:
            # Ensure destination directory exists (parent only)
            task.destination.parent.mkdir(parents=True, exist_ok=True)
            
            logger.info(f"Using Premium URL: {task.url}")
            logger.debug(f"Premium Link for {task.filename}: {task.url}")
            
            # Check for existing partial download
            start_byte = 0
            if task.destination.exists():
                start_byte = task.destination.stat().st_size
                logger.info(f"Resuming download from byte {start_byte}")
            
            # Prepare headers for resume
            headers = {}
            if start_byte > 0:
                headers["Range"] = f"bytes={start_byte}-"
            
            # Use a ClientTimeout to prevent hanging
            timeout = aiohttp.ClientTimeout(total=None, connect=30, sock_read=60)
            
            # Check file size first (verification)
            try:
                async with self._session.head(task.url, allow_redirects=True, timeout=timeout) as head_resp:
                    if head_resp.status == 200:
                        total_expected = int(head_resp.headers.get("Content-Length", 0))
                        if total_expected > 0:
                            logger.info(f"Verification: Local={start_byte}, Remote={total_expected}")
                            if start_byte == total_expected:
                                logger.info(f"✅ Smart Match: File {task.filename} already exists with correct size. Skipping download.")
                                task.progress.total_bytes = total_expected
                                task.progress.downloaded_bytes = total_expected
                                task.progress.percentage = 100.0
                                task.state = DownloadState.COMPLETED
                                task.completed_at = datetime.now()
                                return
                            elif start_byte > total_expected:
                                logger.warning(f"Local file larger than remote ({start_byte} > {total_expected}). Resetting.")
                                start_byte = 0
                                task.destination.unlink(missing_ok=True)
                                headers = {}
            except Exception as e:
                logger.warning(f"Head request failed: {e}. Proceeding with download.")

            # Start download
            logger.debug(f"Initiating request for {task.url}")
            
            try:
                # Get file information first (size and range support)
                async with self._session.get(task.url, headers=headers, timeout=timeout) as response:
                    if response.status == 416: # Range Not Satisfiable
                         logger.info("Server returned 416. Assuming file is complete.")
                         task.state = DownloadState.COMPLETED
                         task.completed_at = datetime.now()
                         return
                    if response.status in (429, 503):
                        task.retry_count += 1
                        # Use configurable backoff: base_multiplier * retry_count, capped at max_wait
                        wait_seconds = min(
                            self.config.retry_backoff_multiplier * task.retry_count,
                            self.config.retry_max_wait
                        )
                        task.wait_until = datetime.now() + timedelta(seconds=wait_seconds)
                        task.state = DownloadState.WAITING
                        task.error_message = f"Server busy (HTTP {response.status}). Retrying in {wait_seconds}s."
                        return

                    if response.status not in (200, 206):
                        raise DownloadFailedError(f"HTTP error {response.status}", {"status": response.status})

                    content_length = int(response.headers.get("Content-Length", 0))
                    accept_ranges = response.headers.get("Accept-Ranges", "") == "bytes"
                    
                    # Apply smart segment calculation based on file size
                    if content_length > 0:
                        task.segments = calculate_optimal_segments(content_length, task.segments)
                    
                    if content_length > 0 and accept_ranges and task.segments > 1 and start_byte == 0:
                        # Only use segmented download for fresh starts with multi-segment config
                        logger.info(f"Using {task.segments} segments for {task.filename} ({content_length / (1024*1024):.1f} MB)")
                        await self._handle_segmented_download(task, content_length)
                    else:
                        # Fallback to single stream if no ranges, small file, OR RESUMING (start_byte > 0)
                        # Segmented download currently overwrites/truncates file, so safely append with single stream
                        await self._handle_response_stream(response, task, start_byte)
            
            except Exception as e:
                task.state = DownloadState.FAILED
                task.error_message = str(e)
                logger.error(f"Download failed: {task.filename} - {e}")
        
        except Exception as e:
            task.state = DownloadState.FAILED
            task.error_message = str(e)
            logger.error(f"Download failed (system): {task.filename} - {e}")
        finally:
             await self._sync_to_db(task)

    async def _handle_segmented_download(self, task: DownloadTask, total_size: int) -> None:
        """Download file using multiple concurrent segments."""
        task.progress.total_bytes = total_size
        task.state = DownloadState.DOWNLOADING
        task.started_at = datetime.now()
        
        # Use task.segments (assigned at queue time, possibly adjusted by smart calculation)
        num_segments = task.segments
        
        # Pre-allocate file (Blocking I/O offloaded)
        def pre_allocate():
             with open(task.destination, "wb") as f:
                f.truncate(total_size)
        await self._loop.run_in_executor(self._io_executor, pre_allocate)
        
        segment_size = total_size // num_segments
        segment_tasks = []
        lock = asyncio.Lock()
        self._file_locks[task.id] = lock
        
        start_time = time.monotonic()
        
        for i in range(num_segments):
            start = i * segment_size
            end = (i + 1) * segment_size - 1 if i < num_segments - 1 else total_size - 1
            
            logger.debug(f"Starting segment {i+1}/{num_segments} for {task.filename} (bytes {start}-{end})")
            
            segment_tasks.append(
                asyncio.create_task(
                    self._download_segment(task, start, end, lock, start_time, segment_id=i+1)
                )
            )
            
        await asyncio.gather(*segment_tasks)
        
        if not task.is_cancelled and task.state != DownloadState.FAILED:
            task.state = DownloadState.COMPLETED
            task.completed_at = datetime.now()
            task.progress.percentage = 100.0
            # Ensure bytes match total (fix for 97% bug if rounding errors occurred)
            task.progress.downloaded_bytes = task.progress.total_bytes
            logger.info(f"Segmented download completed: {task.filename} ({num_segments} segments, Speed: {task.progress.speed_bytes_per_sec / (1024*1024):.2f} MB/s)")

    async def _download_segment(self, task: DownloadTask, start: int, end: int, lock: asyncio.Lock, start_time: float, segment_id: int) -> None:
        """Download a specific byte range."""
        segment_start_time = time.monotonic()
        headers = {"Range": f"bytes={start}-{end}"}
        # Use a longer timeout for segments
        timeout = aiohttp.ClientTimeout(total=None, connect=60, sock_read=60)
        
        try:
            async with self._session.get(task.url, headers=headers, timeout=timeout) as response:
                if response.status != 206:
                    # Some servers don't support range for small files or specific states
                    # If we already pre-allocated, we can't just stream to top
                    raise DownloadFailedError(f"Segment {segment_id} failed with status {response.status}")
                
                current_pos = start
                
                # Use a larger chunk size for the iterator
                chunk_size = self.config.chunk_size
                
                # Use aiofiles to keep file handle open for entire segment
                # This eliminates thousands of open/close operations
                async with aiofiles.open(task.destination, 'r+b') as f:
                    async for chunk in response.content.iter_chunked(chunk_size):
                        if task.is_cancelled:
                            return
                        
                        while task.is_paused and not task.is_cancelled:
                            await asyncio.sleep(0.5)
                        
                        # Rate Limiting
                        await self._rate_limiter.consume(len(chunk))
                        
                        chunk_len = len(chunk)
                        
                        # Async file I/O - no thread executor needed
                        await f.seek(current_pos)
                        await f.write(chunk)
                            
                        task.progress.downloaded_bytes += chunk_len
                        task.progress.update(
                            task.progress.downloaded_bytes,
                            task.progress.total_bytes,
                            time.monotonic() - start_time
                        )
                        
                        current_pos += chunk_len
                        
                        if self.progress_callback:
                            self.progress_callback(task)
                            
                duration = time.monotonic() - segment_start_time
                speed = (current_pos - start) / duration if duration > 0 else 0
                logger.debug(f"Segment {segment_id} finished for {task.filename} (Speed: {speed / (1024*1024):.2f} MB/s)")
        except Exception as e:
            logger.error(f"Segment {start}-{end} failed: {e}")
            task.state = DownloadState.FAILED
            task.error_message = str(e)

    async def _handle_response_stream(self, response: aiohttp.ClientResponse, task: DownloadTask, start_byte: int) -> None:
        """Handle the response stream and write file."""
        # Log response details
        content_type = response.headers.get("Content-Type", "")
        content_length = response.headers.get("Content-Length", "0")
        logger.info(f"Download started (Single Stream). Type: {content_type}, Size: {content_length}, URL: {response.url}")
        
        # Check for HTML content (indicates error page instead of binary file)
        if "text/html" in content_type:
            # Read partial content to log error details
            error_content = await response.content.read(1000)
            logger.error(f"Download link returned HTML instead of file. Preview: {error_content.decode('utf-8', errors='ignore')}")
            raise DownloadFailedError(f"Server returned HTML (likely error page): {content_type}")
        
        # Get total size
        total_size = int(content_length)
        if start_byte > 0:
            total_size += start_byte
        
        task.progress.total_bytes = total_size
        task.progress.downloaded_bytes = start_byte
        task.state = DownloadState.DOWNLOADING
        
        # Open file for writing
        mode = "ab" if start_byte > 0 else "wb"
        start_time = time.monotonic()
        last_log_time = start_time
        
        logger.info(f"Opening file {task.destination} in mode {mode}")
        
        # Use aiofiles for async I/O - keeps file handle open for entire download
        async with aiofiles.open(task.destination, mode) as f:
            async for chunk in response.content.iter_chunked(self.config.chunk_size):
                # Check for cancellation
                if task.is_cancelled:
                    task.state = DownloadState.CANCELLED
                    logger.info(f"Download cancelled: {task.filename}")
                    return
                
                # Wait if paused
                while task.is_paused and not task.is_cancelled:
                    await asyncio.sleep(0.1)
                
                # Rate Limiting
                await self._rate_limiter.consume(len(chunk))

                # Write chunk asynchronously (no thread executor needed)
                await f.write(chunk)
                
                # Update progress
                task.progress.downloaded_bytes += len(chunk)
                current_time = time.monotonic()
                elapsed = current_time - start_time
                task.progress.update(
                    task.progress.downloaded_bytes,
                    task.progress.total_bytes,
                    elapsed,
                )
                
                # Log progress every 5 seconds
                if current_time - last_log_time >= 5.0:
                    logger.info(f"Download progress: {task.filename} - {task.progress.percentage:.1f}% ({task.progress.downloaded_bytes}/{task.progress.total_bytes})")
                    last_log_time = current_time
                
                # Call progress callback
                if self.progress_callback:
                    self.progress_callback(task)
    
        # Mark as completed
        task.state = DownloadState.COMPLETED
        task.completed_at = datetime.now()
        task.progress.percentage = 100.0
        
        logger.info(f"Download completed: {task.filename}")

