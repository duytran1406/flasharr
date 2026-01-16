"""
WebSocket Event System

Optimized event types and data structures for minimal bandwidth usage.
Only sends changed data, not full state.
"""

from enum import Enum
from typing import Dict, Any, Optional, List
from dataclasses import dataclass, asdict
import json


class EventType(str, Enum):
    """WebSocket event types - single character for minimal payload."""
    # Task events
    TASK_ADDED = "ta"           # Task added to queue
    TASK_UPDATED = "tu"         # Task state/progress updated
    TASK_REMOVED = "tr"         # Task removed/deleted
    
    # Engine events
    ENGINE_STATS = "es"         # Engine statistics update
    SPEED_CHANGED = "sc"        # Speed limit changed
    
    # Account events
    ACCOUNT_STATUS = "as"       # Account status changed
    
    # System events
    HEARTBEAT = "hb"            # Keep-alive ping
    ERROR = "er"                # Error message
    CONNECTED = "cn"            # Client connected
    SUBSCRIBED = "sb"           # Subscription confirmed
    SYNC_ALL = "sa"             # Full state sync


@dataclass
class TaskEvent:
    """
    Minimal task event data.
    Only includes changed fields to minimize payload.
    """
    i: str                      # id
    n: Optional[str] = None     # name
    s: Optional[str] = None     # state
    p: Optional[int] = None     # progress percentage (0-100)
    d: Optional[int] = None     # downloaded bytes
    t: Optional[int] = None     # total bytes
    sp: Optional[int] = None    # speed bytes/sec
    e: Optional[int] = None     # eta seconds
    er: Optional[str] = None    # error message
    pr: Optional[str] = None    # priority (L/N/H/U)
    a: Optional[str] = None     # created_at (ISO format)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dict, excluding None values."""
        return {k: v for k, v in asdict(self).items() if v is not None}


@dataclass
class EngineStatsEvent:
    """Minimal engine statistics."""
    a: int                      # active downloads
    q: int                      # queue size
    sp: Optional[int] = None    # total download speed (bytes/sec)
    s: Optional[int] = None     # speed limit (bytes/sec), None = unlimited
    aa: Optional[int] = None    # available accounts
    
    def to_dict(self) -> Dict[str, Any]:
        return {k: v for k, v in asdict(self).items() if v is not None}


@dataclass
class AccountEvent:
    """Minimal account status."""
    e: str                      # email
    a: bool                     # available
    p: bool = False             # premium/VIP
    x: Optional[str] = None     # expiry/validuntil
    t: Optional[str] = None     # traffic_left (e.g. "50 GB / 100 GB")
    q: bool = False             # quota exceeded

    
    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


class WebSocketMessage:
    """
    Optimized WebSocket message format.
    
    Format: {"t": "event_type", "d": data}
    - t: 2-char event type
    - d: minimal data payload
    """
    
    def __init__(self, event_type: EventType, data: Any = None):
        self.event_type = event_type
        self.data = data
    
    def to_json(self) -> str:
        """Convert to JSON string."""
        msg = {"t": self.event_type.value}
        if self.data is not None:
            if hasattr(self.data, 'to_dict'):
                msg["d"] = self.data.to_dict()
            else:
                msg["d"] = self.data
        return json.dumps(msg, separators=(',', ':'))  # No spaces for minimal size
    
    @classmethod
    def from_json(cls, json_str: str) -> 'WebSocketMessage':
        """Parse from JSON string."""
        data = json.loads(json_str)
        return cls(
            event_type=EventType(data["t"]),
            data=data.get("d")
        )


class EventBatcher:
    """
    Batches multiple events into a single message to reduce overhead.
    
    Instead of sending 10 separate messages, sends 1 batch message.
    """
    
    def __init__(self, max_batch_size: int = 50, flush_interval_ms: int = 100):
        self.max_batch_size = max_batch_size
        self.flush_interval_ms = flush_interval_ms
        self._batch: List[WebSocketMessage] = []
    
    def add(self, message: WebSocketMessage) -> Optional[str]:
        """
        Add message to batch.
        
        Returns:
            JSON string if batch is full, None otherwise
        """
        self._batch.append(message)
        
        if len(self._batch) >= self.max_batch_size:
            return self.flush()
        
        return None
    
    def flush(self) -> Optional[str]:
        """
        Flush current batch.
        
        Returns:
            JSON string of batched messages, or None if empty
        """
        if not self._batch:
            return None
        
        if len(self._batch) == 1:
            # Single message, send as-is
            result = self._batch[0].to_json()
        else:
            # Multiple messages, send as batch
            result = json.dumps({
                "t": "batch",
                "d": [{"t": msg.event_type.value, "d": msg.data.to_dict() if hasattr(msg.data, 'to_dict') else msg.data} 
                      for msg in self._batch]
            }, separators=(',', ':'))
        
        self._batch.clear()
        return result
    
    def has_pending(self) -> bool:
        """Check if there are pending messages."""
        return len(self._batch) > 0


class DeltaCompressor:
    """
    Compresses data by only sending changes (deltas).
    
    Tracks previous state and only sends what changed.
    """
    
    def __init__(self):
        self._previous_state: Dict[str, Dict[str, Any]] = {}
    
    def compress(self, task_id: str, current_state: Dict[str, Any]) -> Dict[str, Any]:
        """
        Get delta between current and previous state.
        
        Args:
            task_id: Task identifier
            current_state: Current task state
            
        Returns:
            Dictionary with only changed fields
        """
        previous = self._previous_state.get(task_id, {})
        delta = {"i": task_id}  # Always include ID
        
        for key, value in current_state.items():
            if key == "i":  # Skip ID, already included
                continue
            
            # Only include if changed or new
            if key not in previous or previous[key] != value:
                delta[key] = value
        
        # Update previous state
        self._previous_state[task_id] = current_state.copy()
        
        return delta if len(delta) > 1 else None  # Return None if only ID
    
    def clear(self, task_id: Optional[str] = None) -> None:
        """Clear cached state."""
        if task_id:
            self._previous_state.pop(task_id, None)
        else:
            self._previous_state.clear()


def create_task_event(
    task_id: str,
    name: Optional[str] = None,
    state: Optional[str] = None,
    progress_pct: Optional[int] = None,
    downloaded: Optional[int] = None,
    total: Optional[int] = None,
    speed: Optional[int] = None,
    eta: Optional[int] = None,
    error: Optional[str] = None,
    priority: Optional[str] = None
) -> TaskEvent:
    """Helper to create task event with minimal data."""
    return TaskEvent(
        i=task_id,
        n=name,
        s=state,
        p=progress_pct,
        d=downloaded,
        t=total,
        sp=speed,
        e=eta,
        er=error,
        pr=priority
    )


def create_engine_stats_event(
    active: int,
    queue: int,
    speed_limit: Optional[int] = None,
    available_accounts: Optional[int] = None
) -> EngineStatsEvent:
    """Helper to create engine stats event."""
    return EngineStatsEvent(
        a=active,
        q=queue,
        s=speed_limit,
        aa=available_accounts
    )


def create_account_event(
    email: str,
    available: bool,
    premium: bool = False,
    expiry: Optional[str] = None,
    traffic_left: Optional[str] = None,
    quota_exceeded: bool = False
) -> AccountEvent:
    """Helper to create account event."""
    return AccountEvent(
        e=email,
        a=available,
        p=premium,
        x=expiry,
        t=traffic_left,
        q=quota_exceeded
    )

