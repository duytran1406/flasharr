"""
Flasharr Application

Main entry point for the refactored bridge application.
Integrates all services with built-in download engine.
"""

import os
import json
import asyncio
import logging
import threading
from pathlib import Path

from flask import Flask, request, jsonify, send_file, render_template
from flask_cors import CORS

from .factory import create_indexer_service, create_sabnzbd_service
from .core.config import get_config
from .core.account_manager import AccountManager
from .websocket.server import get_websocket_server
from aiohttp import web
import aiohttp

logging.basicConfig(
    level=logging.DEBUG,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)
logging.getLogger('flasharr').setLevel(logging.DEBUG)

# Load version
try:
    VERSION = (Path(__file__).parent.parent.parent / "VERSION").read_text().strip()
except Exception as e:
    logger.warning(f"Could not load VERSION: {e}")
    VERSION = "v0.0.1-beta"


def create_app():
    """Create and configure the Flask application."""
    # Get the directory where this file is located
    base_dir = Path(__file__).parent
    
    app = Flask(__name__,
                template_folder=str(base_dir / 'templates'),
                static_folder=str(base_dir / 'static'))
    CORS(app)
    
    # Create services
    app.indexer = create_indexer_service()
    app.sabnzbd = None  # Will be initialized async
    app.account_manager = AccountManager()
    
    # Inject version into all templates
    @app.context_processor
    def inject_version():
        return dict(version=VERSION)
    
    # Register Blueprints
    from .web.routes import main_bp
    from .web.api import api_bp
    from .web.indexer_routes import indexer_bp
    from .web.sabnzbd_routes import sabnzbd_bp
    
    app.register_blueprint(main_bp)
    app.register_blueprint(api_bp, url_prefix='/api')
    app.register_blueprint(indexer_bp)
    app.register_blueprint(sabnzbd_bp, url_prefix='/sabnzbd')
    
    return app


async def initialize_sabnzbd(app):
    """Initialize SABnzbd service asynchronously."""
    try:
        logger.info("Initializing SABnzbd service...")
        app.sabnzbd = await create_sabnzbd_service(account_manager=app.account_manager)
        logger.info("✅ SABnzbd service initialized")
    except Exception as e:
        app.init_error = str(e)
        logger.error(f"Failed to initialize SABnzbd service: {e}", exc_info=True)


def run_app():
    """Run the application."""
    config = get_config()
    app = create_app()
    app.init_error = None
    
    # Initialize SABnzbd in background thread to keep loop running
    def start_loop(loop):
        asyncio.set_event_loop(loop)
        try:
            loop.run_forever()
        except Exception as e:
            logger.error(f"Async loop exited: {e}")
    
    loop = asyncio.new_event_loop()
    app.async_loop = loop  # Store loop reference for builtin_client access
    t = threading.Thread(target=start_loop, args=(loop,), daemon=True)
    t.start()
    
    # Schedule initialization
    future = asyncio.run_coroutine_threadsafe(initialize_sabnzbd(app), loop)
    
    # Wait briefly for initialization to complete (optional, just for cleaner logs on startup)
    try:
        future.result(timeout=5)
    except Exception as e:
        logger.warning(f"Initialization still in progress or failed: {e}")
    
    # Run Flask app in a separate thread on a secondary port
    logger.info(f"Starting Flask backend on port 8485 (local)")
    flask_thread = threading.Thread(
        target=app.run,
        kwargs={
            'host': '127.0.0.1',
            'port': 8485,
            'debug': False,
            'use_reloader': False
        },
        daemon=True
    )
    flask_thread.start()

    # Create aiohttp proxy server
    async def proxy_handler(request):
        # WebSocket handling
        if request.path == '/ws':
             engine = app.sabnzbd.downloader.engine if app.sabnzbd and hasattr(app.sabnzbd, 'downloader') else None
             ws_server = get_websocket_server(engine)
             if not ws_server:
                 return web.Response(text="WebSocket server not ready", status=503)
             return await ws_server.handle_connection(request)

        # Proxy standard requests to Flask
        target_url = f"http://127.0.0.1:8485{request.path_qs}"
        
        # Prepare headers (exclude hop-by-hop)
        headers = {k: v for k, v in request.headers.items() 
                   if k.lower() not in ('host', 'content-length', 'transfer-encoding', 'connection')}
        
        try:
            async with aiohttp.ClientSession() as session:
                method = request.method
                data = await request.read()
                
                async with session.request(method, target_url, headers=headers, data=data, allow_redirects=False) as resp:
                    resp_headers = {k: v for k, v in resp.headers.items() 
                                   if k.lower() not in ('transfer-encoding', 'content-encoding', 'connection')}
                    
                    body = await resp.read()
                    return web.Response(body=body, status=resp.status, headers=resp_headers)
        except Exception as e:
            logger.error(f"Proxy error: {e}")
            return web.Response(text=f"Proxy error: {e}", status=502)

    # Initialize WebSocket Server and start it
    async def start_aio_app():
        try:
            logger.info("Starting aiohttp WebSocket gateway...")
            
            # Wait for sabnzbd to be initialized
            attempts = 0
            while not app.sabnzbd and attempts < 30:
                await asyncio.sleep(1)
                attempts += 1
            
            engine = app.sabnzbd.downloader.engine if app.sabnzbd and hasattr(app.sabnzbd, 'downloader') else None
            ws_server = get_websocket_server(engine)
            if ws_server:
                await ws_server.start()
                logger.info("WebSocket server background tasks started")
            else:
                logger.error("❌ Could not initialize WebSocket server (SABnzbd engine missing after wait)")
                
            aio_app = web.Application()
            aio_app.router.add_route('*', '/{path_info:.*}', proxy_handler)
            
            runner = web.AppRunner(aio_app)
            await runner.setup()
            site = web.TCPSite(runner, '0.0.0.0', config.server.port)
            await site.start()
            logger.info(f"✅ Real-time WebSocket Gateway started on port {config.server.port}")
        except Exception as e:
            logger.error(f"❌ Failed to start aiohttp gateway: {e}", exc_info=True)
        
    # Schedule aiohttp on the existing loop
    asyncio.run_coroutine_threadsafe(start_aio_app(), loop)
    
    # Wait for the main thread to stay alive
    try:
        while True:
            import time
            time.sleep(1)
    except KeyboardInterrupt:
        logger.info("Stopping...")


if __name__ == '__main__':
    run_app()
