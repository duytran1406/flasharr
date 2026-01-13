"""
Enhanced Download Engine with Beta Features

This module extends the base engine with:
- Dynamic segment scaling (Phase 1)
- Global bandwidth limiting (Phase 2)
- Priority queue system (Phase 2)
- Multi-account load balancing (Phase 3)
- Smart link checking (Phase 4)
"""

import asyncio
import logging
import psutil
from typing import Optional
from datetime import datetime, timedelta

from .engine import DownloadEngine, DownloadTask, DownloadState
from ..core.rate_limiter import GlobalRateLimiter
from ..core.priority_queue import Priority, auto_prioritize
from ..core.link_checker import LinkStatus

logger = logging.getLogger(__name__)


class EnhancedDownloadEngine(DownloadEngine):
    """
    Enhanced download engine with beta features.
    
    Extends base DownloadEngine with:
    - Bandwidth control via token bucket
    - Priority-based queue
    - Multi-account support
    - Smart link pre-checking
    - Dynamic segment scaling
    """
    
    def __init__(self, *args, account_manager=None, **kwargs):
        """Initialize enhanced engine."""
        super().__init__(*args, **kwargs)
        
        # Override queue with priority queue
        from ..core.priority_queue import PriorityQueue
        self._queue = PriorityQueue()
        
        # Phase 2: Bandwidth Control
        self.rate_limiter = GlobalRateLimiter()
        
        # Phase 3: Multi-Account Support
        self.account_manager = account_manager
        self.account_balancer = None
        if account_manager:
            from ..core.account_load_balancer import AccountLoadBalancer
            self.account_balancer = AccountLoadBalancer(account_manager, max_downloads_per_account=2)
        
        # Phase 4: Link Checker
        from ..core.link_checker import get_link_checker
        self.link_checker = get_link_checker()
        
        # Dynamic segment scaling
        self.enable_dynamic_scaling = True
        self.min_segments = 2
        self.max_segments = 8
        
        logger.info("Enhanced Download Engine initialized with beta features")
    
    def set_global_speed_limit(self, bytes_per_sec: Optional[int]) -> None:
        """
        Set global speed limit (Slow Mode).
        
        Args:
            bytes_per_sec: Speed limit in bytes/sec, or None to disable
        """
        if bytes_per_sec:
            self.rate_limiter.enable(bytes_per_sec)
            logger.info(f"Global speed limit enabled: {bytes_per_sec / 1024 / 1024:.1f} MB/s")
        else:
            self.rate_limiter.disable()
            logger.info("Global speed limit disabled")
    
    def set_task_priority(self, task_id: str, priority: Priority) -> bool:
        """
        Set priority for a task.
        
        Args:
            task_id: Task to update
            priority: New priority level
            
        Returns:
            True if successful
        """
        task = self._tasks.get(task_id)
        if task:
            task.priority = priority
            self._queue.set_priority(task_id, priority)
            logger.info(f"Task {task_id} priority set to {priority.name}")
            return True
        return False
    
    async def add_download(
        self,
        url: str,
        filename: str,
        destination: Optional[str] = None,
        task_id: Optional[str] = None,
        category: str = "Uncategorized",
        package_name: Optional[str] = None,
        priority: Optional[Priority] = None,
        check_link: bool = True,
    ) -> DownloadTask:
        """
        Add download with enhanced features.
        
        Args:
            url: Download URL
            filename: Target filename
            destination: Download directory
            task_id: Optional custom task ID
            category: Download category
            package_name: Package/group name
            priority: Priority level (auto-determined if None)
            check_link: Whether to pre-check link availability
            
        Returns:
            DownloadTask object
        """
        import uuid
        from pathlib import Path
        
        # Phase 4: Smart Link Checking
        if check_link:
            logger.info(f"Pre-checking link: {filename}")
            check_result = await self.link_checker.check_link(url, self._session)
            
            if not check_result.is_available:
                logger.warning(f"Link check failed for {filename}: {check_result.status.value}")
                
                # Create task in failed state
                dest_path = Path(destination or self.config.download_dir) / filename
                task = DownloadTask(
                    id=task_id or str(uuid.uuid4()),
                    url=url,
                    filename=filename,
                    destination=dest_path,
                    category=category,
                    package_name=package_name,
                    state=DownloadState.OFFLINE if check_result.status == LinkStatus.OFFLINE else DownloadState.TEMP_OFFLINE,
                    error_message=check_result.error_message
                )
                self._tasks[task.id] = task
                return task
        
        # Create task via parent method
        dest_path = Path(destination or self.config.download_dir) / filename
        task = DownloadTask(
            id=task_id or str(uuid.uuid4()),
            url=url,
            filename=filename,
            destination=dest_path,
            category=category,
            package_name=package_name,
        )
        
        # Phase 2: Auto-prioritize or use provided priority
        if priority is None:
            # Get file size from link check if available
            size_bytes = 0
            if check_link:
                check_result = await self.link_checker.check_link(url, self._session)
                size_bytes = check_result.size_bytes or 0
            
            priority = auto_prioritize(filename, size_bytes, category)
        
        task.priority = priority
        self._tasks[task.id] = task
        
        # Add to priority queue
        await self._queue.put(task.id, priority=priority, size_bytes=task.progress.total_bytes)
        
        logger.info(f"Added download: {filename} (priority: {priority.name})")
        return task
    
    def _calculate_optimal_segments(self, file_size: int) -> int:
        """
        Calculate optimal number of segments based on file size and system resources.
        
        Phase 1: Dynamic Segment Scaling
        
        Args:
            file_size: File size in bytes
            
        Returns:
            Optimal number of segments
        """
        if not self.enable_dynamic_scaling:
            return self.segments_per_download
        
        # Check system resources
        try:
            # Get disk I/O stats
            disk_io = psutil.disk_io_counters()
            
            # Get network stats
            net_io = psutil.net_io_counters()
            
            # Simple heuristic: reduce segments if disk is busy
            # In production, you'd want more sophisticated monitoring
            
            # For now, use file size as primary factor
            if file_size < 10 * 1024 * 1024:  # < 10MB
                return self.min_segments
            elif file_size < 100 * 1024 * 1024:  # < 100MB
                return min(4, self.max_segments)
            else:  # >= 100MB
                return self.max_segments
        
        except Exception as e:
            logger.warning(f"Failed to calculate optimal segments: {e}")
            return self.segments_per_download
    
    async def _download_segment(self, task: DownloadTask, start: int, end: int, lock: asyncio.Lock, start_time: float) -> None:
        """
        Download segment with rate limiting.
        
        Overrides parent to add bandwidth control.
        """
        import aiohttp
        
        headers = {"Range": f"bytes={start}-{end}"}
        timeout = aiohttp.ClientTimeout(total=None, connect=30, sock_read=60)
        
        try:
            async with self._session.get(task.url, headers=headers, timeout=timeout) as response:
                if response.status != 206:
                    from ..core.exceptions import DownloadFailedError
                    raise DownloadFailedError(f"Segment failed with status {response.status}")
                
                current_pos = start
                async for chunk in response.content.iter_chunked(self.config.chunk_size):
                    if task.is_cancelled:
                        return
                    
                    while task.is_paused and not task.is_cancelled:
                        await asyncio.sleep(0.1)
                    
                    # Phase 2: Apply rate limiting
                    await self.rate_limiter.consume(len(chunk))
                    
                    # Async write with lock
                    async with lock:
                        with open(task.destination, "r+b") as f:
                            f.seek(current_pos)
                            f.write(chunk)
                    
                    current_pos += len(chunk)
                    
                    # Update progress
                    import time
                    task.progress.downloaded_bytes += len(chunk)
                    task.progress.update(
                        task.progress.downloaded_bytes,
                        task.progress.total_bytes,
                        time.time() - start_time
                    )
                    
                    if self.progress_callback:
                        self.progress_callback(task)
        
        except Exception as e:
            logger.error(f"Segment {start}-{end} failed: {e}")
            task.state = DownloadState.FAILED
            task.error_message = str(e)
    
    async def _handle_segmented_download(self, task: DownloadTask, total_size: int) -> None:
        """
        Handle segmented download with dynamic scaling.
        
        Overrides parent to add dynamic segment calculation.
        """
        # Phase 1: Calculate optimal segments
        optimal_segments = self._calculate_optimal_segments(total_size)
        logger.info(f"Using {optimal_segments} segments for {task.filename} ({total_size / 1024 / 1024:.1f} MB)")
        
        task.progress.total_bytes = total_size
        task.state = DownloadState.DOWNLOADING
        task.started_at = datetime.now()
        
        # Pre-allocate file
        with open(task.destination, "wb") as f:
            f.truncate(total_size)
        
        segment_size = total_size // optimal_segments
        tasks = []
        lock = asyncio.Lock()
        self._file_locks[task.id] = lock
        
        import time
        start_time = time.time()
        
        for i in range(optimal_segments):
            start = i * segment_size
            end = (i + 1) * segment_size - 1 if i < optimal_segments - 1 else total_size - 1
            
            tasks.append(
                asyncio.create_task(
                    self._download_segment(task, start, end, lock, start_time)
                )
            )
        
        await asyncio.gather(*tasks)
        
        if not task.is_cancelled and task.state != DownloadState.FAILED:
            task.state = DownloadState.COMPLETED
            task.completed_at = datetime.now()
            task.progress.percentage = 100.0
            logger.info(f"Segmented download completed: {task.filename}")
    
    def get_engine_stats(self) -> dict:
        """
        Get comprehensive engine statistics.
        
        Returns:
            Dictionary with engine stats including beta features
        """
        base_stats = {
            "total_tasks": len(self._tasks),
            "active_downloads": self.active_count,
            "queue_size": self._queue.qsize(),
            "max_concurrent": self.max_concurrent,
        }
        
        # Add rate limiter stats
        if self.rate_limiter.is_enabled:
            base_stats["rate_limiter"] = self.rate_limiter.get_stats()
        
        # Add account balancer stats
        if self.account_balancer:
            base_stats["account_balancer"] = self.account_balancer.get_stats()
        
        # Add link checker stats
        base_stats["link_checker"] = self.link_checker.get_stats()
        
        return base_stats
