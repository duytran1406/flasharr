"""
Flasharr - Main Entry Point

Unified application entry for running the bridge.
"""

import asyncio
import logging
import signal
import sys
from typing import Optional

from .core.config import get_config
from .web.app import create_app
from .downloader.engine import DownloadEngine

logger = logging.getLogger(__name__)

# Global engine instance
_engine: Optional[DownloadEngine] = None


def run_server():
    """Run the Flask web server."""
    config = get_config()
    app = create_app()
    
    # Use gunicorn in production, development server otherwise
    if config.server.debug:
        app.run(
            host=config.server.host,
            port=config.server.port,
            debug=True,
        )
    else:
        from gunicorn.app.base import Application

        class FlaskApplication(Application):
            def __init__(self, app, options=None):
                self.options = options or {}
                self.application = app
                super().__init__()

            def load_config(self):
                for key, value in self.options.items():
                    self.cfg.set(key.lower(), value)

            def load(self):
                return self.application

        options = {
            "bind": f"{config.server.host}:{config.server.port}",
            "workers": 2,
            "threads": 4,
            "timeout": 120,
            "accesslog": "-",
            "errorlog": "-",
            "loglevel": "info",
        }
        
        FlaskApplication(app, options).run()


async def run_engine():
    """Run the download engine."""
    global _engine
    
    config = get_config()
    _engine = DownloadEngine(max_concurrent=config.download.max_concurrent)
    
    # Setup shutdown handler
    def shutdown_handler(sig, frame):
        logger.info("Shutdown signal received")
        asyncio.create_task(_engine.stop())
        sys.exit(0)
    
    signal.signal(signal.SIGINT, shutdown_handler)
    signal.signal(signal.SIGTERM, shutdown_handler)
    
    await _engine.start()
    
    # Keep running until shutdown
    while True:
        await asyncio.sleep(1)


def main():
    """Main entry point."""
    import argparse
    
    parser = argparse.ArgumentParser(description="Flasharr")
    parser.add_argument(
        "--mode",
        choices=["server", "engine", "both"],
        default="server",
        help="Run mode: server (web only), engine (downloader only), or both",
    )
    parser.add_argument(
        "--debug",
        action="store_true",
        help="Enable debug mode",
    )
    
    args = parser.parse_args()
    
    # Configure logging
    log_level = logging.DEBUG if args.debug else logging.INFO
    logging.basicConfig(
        level=log_level,
        format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    )
    
    logger.info(f"Starting Flasharr in {args.mode} mode")
    
    if args.mode == "server":
        run_server()
    elif args.mode == "engine":
        asyncio.run(run_engine())
    else:
        # Run both - server in main thread, engine in background
        import threading
        
        engine_thread = threading.Thread(
            target=lambda: asyncio.run(run_engine()),
            daemon=True,
        )
        engine_thread.start()
        
        run_server()


if __name__ == "__main__":
    main()
