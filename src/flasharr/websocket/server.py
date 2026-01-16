"""
WebSocket Server

High-performance WebSocket server with:
- Minimal bandwidth usage
- Delta compression
- Event batching
- Selective subscriptions
"""

import asyncio
import logging
from typing import Set, Dict, Optional, Callable
from datetime import datetime
import json

try:
    from aiohttp import web
    import aiohttp
except ImportError:
    raise ImportError("aiohttp required for WebSocket support: pip install aiohttp")

from .events import (
    EventType, WebSocketMessage, EventBatcher, DeltaCompressor,
    TaskEvent, EngineStatsEvent, AccountEvent,
    create_task_event, create_engine_stats_event, create_account_event
)
from ..downloader.engine import DownloadState

logger = logging.getLogger(__name__)


class WebSocketClient:
    """Represents a connected WebSocket client."""
    
    def __init__(self, ws: web.WebSocketResponse, client_id: str):
        self.ws = ws
        self.client_id = client_id
        self.subscriptions: Set[str] = set()  # What events client wants
        self.delta_compressor = DeltaCompressor()
        self.batcher = EventBatcher(max_batch_size=20, flush_interval_ms=100)
        self.last_heartbeat = datetime.now()
    
    async def send(self, message: WebSocketMessage) -> None:
        """Send message to client."""
        try:
            # Add to batch
            batched = self.batcher.add(message)
            if batched:
                await self.ws.send_str(batched)
        except Exception as e:
            logger.error(f"Failed to send to client {self.client_id}: {e}")
    
    async def flush(self) -> None:
        """Flush pending batched messages."""
        try:
            batched = self.batcher.flush()
            if batched:
                await self.ws.send_str(batched)
        except Exception as e:
            logger.error(f"Failed to flush to client {self.client_id}: {e}")
    
    def is_subscribed(self, event_type: str) -> bool:
        """Check if client is subscribed to event type."""
        return not self.subscriptions or event_type in self.subscriptions


class WebSocketServer:
    """
    WebSocket server for real-time updates.
    
    Features:
    - Selective subscriptions (clients choose what to receive)
    - Delta compression (only send changes)
    - Event batching (combine multiple events)
    - Heartbeat (keep connections alive)
    """
    
    def __init__(self, engine, sabnzbd=None):
        self.engine = engine
        self.sabnzbd = sabnzbd
        self.clients: Dict[str, WebSocketClient] = {}
        self._running = False
        self._heartbeat_task: Optional[asyncio.Task] = None
        self._flush_task: Optional[asyncio.Task] = None
        self._stats_task: Optional[asyncio.Task] = None
        
        # Track previous states for delta compression
        self._previous_engine_stats: Optional[Dict] = None
        
        logger.info("WebSocket server initialized")
    
    async def start(self) -> None:
        """Start background tasks."""
        if self._running:
            return
        
        self._running = True
        
        # Start heartbeat task (every 30 seconds)
        self._heartbeat_task = asyncio.create_task(self._heartbeat_loop())
        
        # Start flush task (every 100ms)
        self._flush_task = asyncio.create_task(self._flush_loop())
        
        # Start stats loop (every 2 seconds)
        self._stats_task = asyncio.create_task(self._stats_loop())
        
        # Hook into engine progress callback
        if hasattr(self.engine, 'progress_callback'):
            original_callback = self.engine.progress_callback
            
            def combined_callback(task):
                if original_callback:
                    original_callback(task)
                asyncio.create_task(self.broadcast_task_update(task))
            
            self.engine.progress_callback = combined_callback
        
        logger.info("WebSocket server started")
    
    async def stop(self) -> None:
        """Stop background tasks."""
        self._running = False
        
        if self._heartbeat_task:
            self._heartbeat_task.cancel()
        
        if (self._flush_task):
            self._flush_task.cancel()
            
        if (self._stats_task):
            self._stats_task.cancel()
        
        # Close all client connections
        for client in list(self.clients.values()):
            await client.ws.close()
        
        self.clients.clear()
        logger.info("WebSocket server stopped")
    
    async def handle_connection(self, request: web.Request) -> web.WebSocketResponse:
        """Handle new WebSocket connection."""
        ws = web.WebSocketResponse(heartbeat=30)
        await ws.prepare(request)
        
        # Generate client ID
        import uuid
        client_id = str(uuid.uuid4())[:8]
        
        client = WebSocketClient(ws, client_id)
        self.clients[client_id] = client
        
        logger.info(f"Client {client_id} connected (total: {len(self.clients)})")
        
        # Send connection confirmation
        await client.send(WebSocketMessage(EventType.CONNECTED, {"id": client_id}))
        
        # Trigger full sync in background to avoid blocking connection
        asyncio.create_task(self.send_full_sync(client))
        
        try:
            async for msg in ws:
                if msg.type == aiohttp.WSMsgType.TEXT:
                    await self._handle_message(client, msg.data)
                elif msg.type == aiohttp.WSMsgType.ERROR:
                    logger.error(f"WebSocket error: {ws.exception()}")
        finally:
            # Client disconnected
            self.clients.pop(client_id, None)
            logger.info(f"Client {client_id} disconnected (remaining: {len(self.clients)})")
        
        return ws

    async def send_full_sync(self, client: WebSocketClient) -> None:
        """Send full state of all downloads to client."""
        if not self.engine:
            logger.warning("Cannot send sync: Engine not initialized")
            return
            
        try:
            # Fetch fresh list from engine
            tasks_dict = self.engine.tasks
            
            # Convert to minimal format
            sync_data = []
            for task_id, task in tasks_dict.items():
                 sync_data.append({
                     "i": task.id,
                     "n": task.filename,
                     "s": task.state.value if hasattr(task.state, 'value') else str(task.state),
                     "p": int(task.progress.percentage) if task.progress else 0,
                     "d": task.progress.downloaded_bytes if task.progress else 0,
                     "t": task.progress.total_bytes if task.progress else 0,
                     "sp": int(task.progress.speed_bytes_per_sec) if task.progress else 0,
                     "e": int(task.progress.eta_seconds) if task.progress else 0,
                     "er": task.error_message or "",
                     "a": task.created_at.isoformat() if hasattr(task, 'created_at') else None
                 })
                 
            await client.send(WebSocketMessage(EventType.SYNC_ALL, sync_data))
            logger.info(f"Sent full sync with {len(sync_data)} tasks to {client.client_id}")
            
        except Exception as e:
            logger.error(f"Failed to send full sync to {client.client_id}: {e}")
    
    async def _handle_message(self, client: WebSocketClient, data: str) -> None:
        """Handle message from client."""
        try:
            message = WebSocketMessage.from_json(data)
            
            if message.event_type == EventType.HEARTBEAT:
                # Respond to heartbeat
                client.last_heartbeat = datetime.now()
                await client.send(WebSocketMessage(EventType.HEARTBEAT))
            
            elif message.data and isinstance(message.data, dict):
                # Handle subscription updates
                if "subscribe" in message.data:
                    subscriptions = message.data["subscribe"]
                    if isinstance(subscriptions, list):
                        client.subscriptions.update(subscriptions)
                        await client.send(WebSocketMessage(
                            EventType.SUBSCRIBED,
                            {"subscriptions": list(client.subscriptions)}
                        ))
                        logger.debug(f"Client {client.client_id} subscribed to: {subscriptions}")
        
        except Exception as e:
            logger.error(f"Error handling message from {client.client_id}: {e}")
            await client.send(WebSocketMessage(EventType.ERROR, {"error": str(e)}))
    
    async def _heartbeat_loop(self) -> None:
        """Send periodic heartbeats."""
        while self._running:
            try:
                await asyncio.sleep(30)
                
                # Send heartbeat to all clients
                for client in list(self.clients.values()):
                    await client.send(WebSocketMessage(EventType.HEARTBEAT))
            
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Heartbeat error: {e}")
    
    async def _flush_loop(self) -> None:
        """Flush batched messages periodically."""
        while self._running:
            try:
                await asyncio.sleep(0.1)  # 100ms
                
                # Flush all clients
                for client in list(self.clients.values()):
                    if client.batcher.has_pending():
                        await client.flush()
            
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Flush error: {e}")

    async def _stats_loop(self) -> None:
        """Broadcast engine stats periodically."""
        while self._running:
            try:
                await asyncio.sleep(2)
                await self.broadcast_engine_stats()
                await self.broadcast_account_status()

            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Stats loop error: {e}")
    
    async def broadcast_task_update(self, task) -> None:
        """
        Broadcast task update to subscribed clients.
        Uses delta compression to minimize payload.
        """
        if not self.clients:
            return
        # Create minimal task state
        current_state = {
            "i": task.id,
            "n": task.filename,
            "s": task.state.value if hasattr(task.state, 'value') else str(task.state),
            "p": int(task.progress.percentage) if task.progress else 0,
            "d": task.progress.downloaded_bytes if task.progress else 0,
            "t": task.progress.total_bytes if task.progress else 0,
             "sp": int(task.progress.speed_bytes_per_sec) if task.progress else 0,
             "e": int(task.progress.eta_seconds) if task.progress else 0,
             "a": task.created_at.isoformat() if hasattr(task, 'created_at') else None,
         }
        
        if hasattr(task, 'error_message') and task.error_message:
            current_state["er"] = task.error_message
        
        if hasattr(task, 'priority'):
            # Convert priority to single char: L/N/H/U
            priority_map = {"LOW": "L", "NORMAL": "N", "HIGH": "H", "URGENT": "U"}
            current_state["pr"] = priority_map.get(task.priority.name, "N")
        for client in list(self.clients.values()):
            if not client.is_subscribed(EventType.TASK_UPDATED.value):
                continue
            
            # Get delta for this client
            delta = client.delta_compressor.compress(task.id, current_state)
            
            if delta:  # Only send if there are changes
                event = TaskEvent(**delta)
                logger.debug(f"Broadcasting task update to {client.client_id}: {delta}")
                await client.send(WebSocketMessage(EventType.TASK_UPDATED, event))
    
    async def broadcast_engine_stats(self) -> None:
        """Broadcast engine statistics."""
        if not self.clients:
            return
        
        # Get current stats
        stats = self.engine.get_engine_stats() if hasattr(self.engine, 'get_engine_stats') else {}
        
        # Use sabnzbd for counts if available (more accurate for history/queue)
        if self.sabnzbd:
            # Fix: get_counts() is likely async in the service
            if asyncio.iscoroutinefunction(self.sabnzbd.get_counts):
                counts = await self.sabnzbd.get_counts()
            else:
                 # Check if it returns a coroutine even if not typed as such (e.g. mocked or wrapped)
                result = self.sabnzbd.get_counts()
                if asyncio.iscoroutine(result):
                    counts = await result
                else:
                    counts = result
            
            active_val = counts.get("active", 0)
            total_val = counts.get("total", 0)
        else:
            active_val = stats.get("active_downloads", 0)
            total_val = stats.get("queue_size", 0)

        current_stats = {
            "a": active_val,
            "q": total_val,
            "sp": int(stats.get("total_speed", 0))
        }
        
        # Add speed limit if enabled
        if "rate_limiter" in stats and stats["rate_limiter"].get("enabled"):
            current_stats["s"] = stats["rate_limiter"].get("rate_limit")
        
        # Add account info if available
        if "account_balancer" in stats:
            current_stats["aa"] = stats["account_balancer"].get("available_accounts")
        
        # Only send if changed
        if current_stats != self._previous_engine_stats:
            self._previous_engine_stats = current_stats.copy()
            
            event = EngineStatsEvent(**current_stats)
            
            for client in list(self.clients.values()):
                if client.is_subscribed(EventType.ENGINE_STATS.value):
                    logger.debug(f"Broadcasting stats to {client.client_id}: {current_stats}")
                    await client.send(WebSocketMessage(EventType.ENGINE_STATS, event))
    
    async def broadcast_account_status(self) -> None:
        """Broadcast status of the primary account."""
        if not self.clients or not self.sabnzbd:
            return
            
        acc_mgr = getattr(self.sabnzbd, 'account_manager', None)
        if not acc_mgr:
            return
            
        primary = acc_mgr.get_primary()
        if not primary:
            return
            
        event = create_account_event(
            email=primary['email'],
            available=True,
            premium=primary.get('premium', False),
            expiry=primary.get('expiry'),
            traffic_left=primary.get('traffic_left')
        )

        
        for client in list(self.clients.values()):
            if client.is_subscribed(EventType.ACCOUNT_STATUS.value):
                await client.send(WebSocketMessage(EventType.ACCOUNT_STATUS, event))

    
    async def broadcast_task_added(self, task) -> None:
        """Broadcast new task added."""
        event = create_task_event(
            task_id=task.id,
            name=task.filename,
            state=task.state.value if hasattr(task.state, 'value') else str(task.state),
            priority=task.priority.name[0] if hasattr(task, 'priority') else "N"
        )
        
        for client in list(self.clients.values()):
            if client.is_subscribed(EventType.TASK_ADDED.value):
                await client.send(WebSocketMessage(EventType.TASK_ADDED, event))
    
    async def broadcast_task_removed(self, task_id: str) -> None:
        """Broadcast task removed."""
        for client in list(self.clients.values()):
            if client.is_subscribed(EventType.TASK_REMOVED.value):
                # Clear delta cache for this task
                client.delta_compressor.clear(task_id)
                await client.send(WebSocketMessage(EventType.TASK_REMOVED, {"i": task_id}))
    
    def get_stats(self) -> Dict:
        """Get WebSocket server statistics."""
        return {
            "connected_clients": len(self.clients),
            "running": self._running,
            "clients": [
                {
                    "id": client.client_id,
                    "subscriptions": list(client.subscriptions),
                    "last_heartbeat": client.last_heartbeat.isoformat()
                }
                for client in self.clients.values()
            ]
        }


# Global instance
_ws_server: Optional[WebSocketServer] = None


def get_websocket_server(engine=None, sabnzbd=None) -> WebSocketServer:
    """Get or create global WebSocket server."""
    global _ws_server
    if _ws_server is None and engine:
        _ws_server = WebSocketServer(engine, sabnzbd)
    elif _ws_server and sabnzbd and not _ws_server.sabnzbd:
        # Late binding of sabnzbd service
        _ws_server.sabnzbd = sabnzbd
    return _ws_server


def init_websocket_routes(app: web.Application, engine) -> None:
    """Initialize WebSocket routes in aiohttp app."""
    ws_server = get_websocket_server(engine)
    
    async def websocket_handler(request):
        return await ws_server.handle_connection(request)
    
    app.router.add_get('/ws', websocket_handler)
    
    # Start server
    asyncio.create_task(ws_server.start())
    
    logger.info("WebSocket routes initialized at /ws")
