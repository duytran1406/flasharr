"""
Priority Queue System for Download Management

Enables manual and automatic prioritization of downloads.
Small files and single movies can jump to the front.
"""

import asyncio
from enum import IntEnum
from typing import Optional, List
from dataclasses import dataclass
import logging

logger = logging.getLogger(__name__)


class Priority(IntEnum):
    """Download priority levels."""
    LOW = 0
    NORMAL = 1
    HIGH = 2
    URGENT = 3


@dataclass
class PriorityTask:
    """Wrapper for tasks in priority queue."""
    task_id: str
    priority: Priority
    size_bytes: int
    added_at: float
    
    def __lt__(self, other: 'PriorityTask') -> bool:
        """
        Compare for priority queue ordering.
        Higher priority first, then smaller files, then FIFO.
        """
        if self.priority != other.priority:
            return self.priority > other.priority
        
        # For same priority, prefer smaller files (< 100MB gets boost)
        if self.size_bytes < 100 * 1024 * 1024 and other.size_bytes >= 100 * 1024 * 1024:
            return True
        if self.size_bytes >= 100 * 1024 * 1024 and other.size_bytes < 100 * 1024 * 1024:
            return False
        
        # Otherwise FIFO
        return self.added_at < other.added_at


class PriorityQueue:
    """
    Priority-based download queue.
    
    Features:
    - Manual priority assignment (LOW, NORMAL, HIGH, URGENT)
    - Automatic small-file prioritization
    - FIFO within same priority level
    """
    
    def __init__(self):
        self._queue: asyncio.PriorityQueue = asyncio.PriorityQueue()
        self._task_priorities: dict[str, Priority] = {}
        self._lock = asyncio.Lock()
    
    async def put(
        self, 
        task_id: str, 
        priority: Priority = Priority.NORMAL,
        size_bytes: int = 0
    ) -> None:
        """
        Add task to priority queue.
        
        Args:
            task_id: Task identifier
            priority: Priority level
            size_bytes: File size for auto-prioritization
        """
        import time
        
        async with self._lock:
            self._task_priorities[task_id] = priority
            
            priority_task = PriorityTask(
                task_id=task_id,
                priority=priority,
                size_bytes=size_bytes,
                added_at=time.monotonic()
            )
            
            await self._queue.put(priority_task)
            
            logger.debug(
                f"Task {task_id} queued with priority {priority.name} "
                f"(size: {size_bytes / 1024 / 1024:.1f} MB)"
            )
    
    async def get(self, timeout: Optional[float] = None) -> str:
        """
        Get next task from queue.
        
        Args:
            timeout: Optional timeout in seconds
            
        Returns:
            Task ID
        """
        if timeout:
            priority_task = await asyncio.wait_for(self._queue.get(), timeout=timeout)
        else:
            priority_task = await self._queue.get()
        
        return priority_task.task_id
    
    def set_priority(self, task_id: str, priority: Priority) -> bool:
        """
        Update task priority.
        
        Note: This doesn't reorder already-queued tasks.
        New priority applies on next queue operation.
        
        Args:
            task_id: Task to update
            priority: New priority level
            
        Returns:
            True if task exists
        """
        if task_id in self._task_priorities:
            self._task_priorities[task_id] = priority
            logger.info(f"Task {task_id} priority updated to {priority.name}")
            return True
        return False
    
    def get_priority(self, task_id: str) -> Optional[Priority]:
        """Get current priority for a task."""
        return self._task_priorities.get(task_id)
    
    def qsize(self) -> int:
        """Get queue size."""
        return self._queue.qsize()
    
    def empty(self) -> bool:
        """Check if queue is empty."""
        return self._queue.empty()


def auto_prioritize(filename: str, size_bytes: int, category: str = "") -> Priority:
    """
    Automatically determine priority based on file characteristics.
    
    Rules:
    - Files < 100MB: HIGH priority (quick downloads)
    - Single movies (not in series): NORMAL
    - Large series packs: LOW
    - Everything else: NORMAL
    
    Args:
        filename: File name
        size_bytes: File size
        category: Download category
        
    Returns:
        Suggested priority level
    """
    # Small files get priority
    if size_bytes < 100 * 1024 * 1024:  # < 100MB
        return Priority.HIGH
    
    # Check if it's a series pack (multiple episodes)
    filename_lower = filename.lower()
    if any(indicator in filename_lower for indicator in ['season', 'complete', 'pack', 's01-s', 'batch']):
        return Priority.LOW
    
    # Default to normal
    return Priority.NORMAL
