"""
Flasharr Application (AIOHTTP Native)

Main entry point for the refactored bridge application.
Integrates all services with built-in download engine using a single AsyncIO loop.
"""

import os
import asyncio
import logging
from pathlib import Path
from aiohttp import web

from .factory import create_indexer_service, create_sabnzbd_service
from .core.config import get_config
from .core.account_manager import AccountManager
from .websocket.server import get_websocket_server
from .web import api_aio, routes_aio, tmdb_routes, discovery_routes

# Setup logging
log_dir = Path("data")
log_dir.mkdir(exist_ok=True)

# Configure basic logging if not already done
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

async def init_app() -> web.Application:
    """Create and configure the AIOHTTP application."""
    app = web.Application()
    
    # Context Configuration
    base_dir = Path(__file__).parent

    # Middleware: API Key Authentication
    @web.middleware
    async def auth_middleware(request, handler):
        # Skip auth for non-API routes or if no key configured
        if not request.path.startswith('/api/'):
            return await handler(request)
            
        config = get_config()
        if not config.server.api_key:
            return await handler(request)
            
        # Allow health check without auth
        if request.path.endswith('/health') or request.path.endswith('/version'):
            return await handler(request)
            
        api_key = request.headers.get('X-Api-Key') or request.query.get('apikey')
        if not api_key or api_key != config.server.api_key:
            return web.json_response(
                {"status": "error", "message": "Unauthorized"}, 
                status=401
            )
            
        return await handler(request)
    
    app = web.Application(middlewares=[auth_middleware])
    
    # Initialize Core Services
    app['account_manager'] = AccountManager()
    app['indexer'] = create_indexer_service()
    app['sabnzbd'] = None # Will be init in background
    
    # Setup Jinja2
    routes_aio.setup_jinja(app, str(base_dir / 'templates'))
    
    # Register Routes
    app.add_routes(api_aio.routes)
    app.add_routes(routes_aio.routes)
    app.add_routes(tmdb_routes.routes)
    app.add_routes(discovery_routes.routes)
    
    # Static Files
    app.router.add_static('/static', base_dir / 'static', name='static')
    app.router.add_static('/assets', base_dir / 'static/assets', name='assets', show_index=False)
    
    # WebSocket Route
    async def ws_handler(request):
        sab = app.get('sabnzbd')
        engine = sab.downloader.engine if sab and hasattr(sab, 'downloader') else None
        ws_server = get_websocket_server(engine, sab)
        return await ws_server.handle_connection(request)
        
    app.router.add_get('/ws', ws_handler)
    
    # Background Tasks
    app.on_startup.append(start_background_tasks)
    app.on_cleanup.append(cleanup_background_tasks)
    
    return app

async def start_background_tasks(app: web.Application):
    """Initialize heavy services and background loops."""
    logger.info("Starting background services...")
    
    # Init SABnzbd (Downloader Engine)
    try:
        app['sabnzbd'] = await create_sabnzbd_service(account_manager=app['account_manager'])
        logger.info("✅ SABnzbd service initialized")
        
        # Start WebSocket Server
        engine = app['sabnzbd'].downloader.engine
        ws_server = get_websocket_server(engine, app['sabnzbd'])
        app['ws_server'] = ws_server
        await ws_server.start()
        
    except Exception as e:
        logger.error(f"Failed to start background services: {e}", exc_info=True)

async def cleanup_background_tasks(app: web.Application):
    """Cleanup resources on shutdown."""
    logger.info("Stopping background services...")
    if 'ws_server' in app:
        await app['ws_server'].stop()
    
    if app.get('sabnzbd'):
        # Add shutdown logic for engine if needed
        pass

def run_app():
    """Run the application."""
    config = get_config()
    port = config.server.port
    
    # AIOHTTP native run
    web.run_app(init_app(), port=port)

if __name__ == '__main__':
    run_app()
