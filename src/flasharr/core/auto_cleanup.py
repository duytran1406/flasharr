"""
Auto-Cleanup Service

Automatically purges successfully imported tasks from history after X days.
Phase 4 feature.
"""

import asyncio
import logging
from datetime import datetime, timedelta
from typing import Optional

from ..downloader.engine import DownloadState

logger = logging.getLogger(__name__)


class AutoCleanupService:
    """
    Automatic cleanup service for completed downloads.
    
    Features:
    - Purge completed tasks after configurable retention period
    - Separate retention for successful vs failed downloads
    - Configurable cleanup interval
    - Manual cleanup trigger
    """
    
    def __init__(
        self,
        engine,
        retention_days_success: int = 7,
        retention_days_failed: int = 30,
        cleanup_interval_hours: int = 24
    ):
        """
        Initialize cleanup service.
        
        Args:
            engine: Download engine instance
            retention_days_success: Days to keep successful downloads
            retention_days_failed: Days to keep failed downloads
            cleanup_interval_hours: Hours between automatic cleanups
        """
        self.engine = engine
        self.retention_days_success = retention_days_success
        self.retention_days_failed = retention_days_failed
        self.cleanup_interval = timedelta(hours=cleanup_interval_hours)
        
        self._running = False
        self._task: Optional[asyncio.Task] = None
        
        logger.info(
            f"AutoCleanup initialized: success={retention_days_success}d, "
            f"failed={retention_days_failed}d, interval={cleanup_interval_hours}h"
        )
    
    async def start(self) -> None:
        """Start the cleanup service."""
        if self._running:
            return
        
        self._running = True
        self._task = asyncio.create_task(self._cleanup_loop())
        logger.info("AutoCleanup service started")
    
    async def stop(self) -> None:
        """Stop the cleanup service."""
        self._running = False
        
        if self._task:
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass
        
        logger.info("AutoCleanup service stopped")
    
    async def _cleanup_loop(self) -> None:
        """Main cleanup loop."""
        while self._running:
            try:
                # Perform cleanup
                await self.cleanup()
                
                # Wait for next interval
                await asyncio.sleep(self.cleanup_interval.total_seconds())
            
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Cleanup loop error: {e}")
                await asyncio.sleep(3600)  # Wait 1 hour on error
    
    async def cleanup(self, force: bool = False) -> dict:
        """
        Perform cleanup of old tasks.
        
        Args:
            force: If True, ignore retention periods and clean all completed
            
        Returns:
            Dictionary with cleanup statistics
        """
        logger.info("Starting cleanup...")
        
        now = datetime.now()
        removed_success = 0
        removed_failed = 0
        removed_other = 0
        
        tasks_to_remove = []
        
        for task_id, task in self.engine._tasks.items():
            should_remove = False
            
            # Check if task is in a terminal state
            if task.state not in [
                DownloadState.COMPLETED,
                DownloadState.FINISHED,
                DownloadState.FAILED,
                DownloadState.CANCELLED,
                DownloadState.OFFLINE,
                DownloadState.SKIPPED
            ]:
                continue
            
            # Determine retention period
            if force:
                should_remove = True
            elif task.state in [DownloadState.COMPLETED, DownloadState.FINISHED]:
                # Successful downloads
                if task.completed_at:
                    age = now - task.completed_at
                    if age.days >= self.retention_days_success:
                        should_remove = True
                        removed_success += 1
            
            elif task.state in [DownloadState.FAILED, DownloadState.CANCELLED, DownloadState.OFFLINE]:
                # Failed downloads
                if task.completed_at or task.started_at:
                    ref_time = task.completed_at or task.started_at
                    age = now - ref_time
                    if age.days >= self.retention_days_failed:
                        should_remove = True
                        removed_failed += 1
            
            else:
                # Other terminal states
                if task.created_at:
                    age = now - task.created_at
                    if age.days >= self.retention_days_failed:
                        should_remove = True
                        removed_other += 1
            
            if should_remove:
                tasks_to_remove.append(task_id)
        
        # Remove tasks
        for task_id in tasks_to_remove:
            del self.engine._tasks[task_id]
            logger.debug(f"Removed task: {task_id}")
        
        total_removed = removed_success + removed_failed + removed_other
        
        logger.info(
            f"Cleanup completed: removed {total_removed} tasks "
            f"(success: {removed_success}, failed: {removed_failed}, other: {removed_other})"
        )
        
        return {
            "total_removed": total_removed,
            "removed_success": removed_success,
            "removed_failed": removed_failed,
            "removed_other": removed_other,
            "timestamp": now.isoformat()
        }
    
    def update_config(
        self,
        retention_days_success: Optional[int] = None,
        retention_days_failed: Optional[int] = None,
        cleanup_interval_hours: Optional[int] = None
    ) -> None:
        """
        Update cleanup configuration.
        
        Args:
            retention_days_success: New retention for successful downloads
            retention_days_failed: New retention for failed downloads
            cleanup_interval_hours: New cleanup interval
        """
        if retention_days_success is not None:
            self.retention_days_success = retention_days_success
            logger.info(f"Updated success retention: {retention_days_success} days")
        
        if retention_days_failed is not None:
            self.retention_days_failed = retention_days_failed
            logger.info(f"Updated failed retention: {retention_days_failed} days")
        
        if cleanup_interval_hours is not None:
            self.cleanup_interval = timedelta(hours=cleanup_interval_hours)
            logger.info(f"Updated cleanup interval: {cleanup_interval_hours} hours")
    
    def get_stats(self) -> dict:
        """Get cleanup service statistics."""
        # Count tasks by state
        state_counts = {}
        for task in self.engine._tasks.values():
            state = task.state.value
            state_counts[state] = state_counts.get(state, 0) + 1
        
        # Count cleanable tasks
        now = datetime.now()
        cleanable_success = 0
        cleanable_failed = 0
        
        for task in self.engine._tasks.values():
            if task.state in [DownloadState.COMPLETED, DownloadState.FINISHED]:
                if task.completed_at:
                    age = now - task.completed_at
                    if age.days >= self.retention_days_success:
                        cleanable_success += 1
            
            elif task.state in [DownloadState.FAILED, DownloadState.CANCELLED, DownloadState.OFFLINE]:
                if task.completed_at or task.started_at:
                    ref_time = task.completed_at or task.started_at
                    age = now - ref_time
                    if age.days >= self.retention_days_failed:
                        cleanable_failed += 1
        
        return {
            "running": self._running,
            "retention_days_success": self.retention_days_success,
            "retention_days_failed": self.retention_days_failed,
            "cleanup_interval_hours": self.cleanup_interval.total_seconds() / 3600,
            "total_tasks": len(self.engine._tasks),
            "state_breakdown": state_counts,
            "cleanable_success": cleanable_success,
            "cleanable_failed": cleanable_failed,
            "total_cleanable": cleanable_success + cleanable_failed
        }
