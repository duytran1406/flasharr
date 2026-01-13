"""WebSocket package for real-time updates."""

from .events import (
    EventType,
    WebSocketMessage,
    TaskEvent,
    EngineStatsEvent,
    AccountEvent,
    create_task_event,
    create_engine_stats_event,
    create_account_event
)

from .server import (
    WebSocketServer,
    WebSocketClient,
    get_websocket_server,
    init_websocket_routes
)

__all__ = [
    'EventType',
    'WebSocketMessage',
    'TaskEvent',
    'EngineStatsEvent',
    'AccountEvent',
    'create_task_event',
    'create_engine_stats_event',
    'create_account_event',
    'WebSocketServer',
    'WebSocketClient',
    'get_websocket_server',
    'init_websocket_routes',
]
