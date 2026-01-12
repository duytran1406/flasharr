"""
Fshare-Arr-Bridge Application

Main entry point for the refactored bridge application.
Integrates all services with built-in download engine.
"""

import asyncio
import logging
from pathlib import Path

from flask import Flask, request, jsonify, send_file
from flask_cors import CORS

from .factory import create_indexer_service, create_sabnzbd_service
from .core.config import get_config

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


def create_app():
    """Create and configure the Flask application."""
    app = Flask(__name__)
    CORS(app)
    
    # Create services
    app.indexer = create_indexer_service()
    app.sabnzbd = None  # Will be initialized async
    
    @app.route('/health')
    def health():
        """Health check endpoint."""
        return jsonify({
            "status": "healthy",
            "services": {
                "indexer": "ready",
                "sabnzbd": "ready" if app.sabnzbd else "initializing",
            }
        })
    
    # Indexer API routes
    @app.route('/indexer/api')
    def indexer_api():
        """Newznab/Torznab API endpoint."""
        t = request.args.get('t', '')
        
        if t == 'caps':
            response = app.indexer.get_capabilities()
            return response.to_response()
        
        elif t in ('search', 'tvsearch', 'movie'):
            query = request.args.get('q', '')
            season = request.args.get('season')
            episode = request.args.get('ep')
            limit = request.args.get('limit', type=int)
            
            response = app.indexer.search(
                query=query,
                season=season,
                episode=episode,
                limit=limit,
            )
            return response.to_response()
        
        else:
            return "Unknown command", 400
    
    @app.route('/nzb/<guid>')
    def get_nzb(guid):
        """Get NZB file for a specific GUID."""
        nzb_content = app.indexer.get_nzb(guid)
        if not nzb_content:
            return "NZB not found", 404
        
        return nzb_content, 200, {
            'Content-Type': 'application/x-nzb',
            'Content-Disposition': f'attachment; filename="{guid}.nzb"'
        }
    
    # SABnzbd API routes
    @app.route('/sabnzbd/api')
    def sabnzbd_api():
        """SABnzbd-compatible API endpoint."""
        if not app.sabnzbd:
            return jsonify({"error": "SABnzbd service not initialized"}), 503
        
        mode = request.args.get('mode', '')
        output = request.args.get('output', 'json')
        
        try:
            if mode == 'version':
                result = {"version": app.sabnzbd.get_version()}
            
            elif mode == 'queue':
                result = app.sabnzbd.get_queue()
            
            elif mode == 'history':
                limit = request.args.get('limit', 50, type=int)
                result = app.sabnzbd.get_history(limit)
            
            elif mode == 'addurl':
                url = request.args.get('name', '')
                category = request.args.get('cat')
                nzo_id = app.sabnzbd.add_url(url, category=category)
                result = {"status": True, "nzo_ids": [nzo_id]} if nzo_id else {"status": False}
            
            elif mode == 'addfile':
                # Handle NZB file upload
                if 'nzbfile' not in request.files:
                    return jsonify({"error": "No file uploaded"}), 400
                
                nzb_file = request.files['nzbfile']
                nzb_data = nzb_file.read()
                category = request.form.get('cat')
                
                nzo_id = app.sabnzbd.add_file(nzb_data, nzb_file.filename, category)
                result = {"status": True, "nzo_ids": [nzo_id]} if nzo_id else {"status": False}
            
            elif mode == 'pause':
                app.sabnzbd.pause_queue()
                result = {"status": True}
            
            elif mode == 'resume':
                app.sabnzbd.resume_queue()
                result = {"status": True}
            
            else:
                return jsonify({"error": f"Unknown mode: {mode}"}), 400
            
            return jsonify(result)
            
        except Exception as e:
            logger.error(f"SABnzbd API error: {e}", exc_info=True)
            return jsonify({"error": str(e)}), 500
    
    return app


async def initialize_sabnzbd(app):
    """Initialize SABnzbd service asynchronously."""
    try:
        logger.info("Initializing SABnzbd service...")
        app.sabnzbd = await create_sabnzbd_service()
        logger.info("✅ SABnzbd service initialized")
    except Exception as e:
        logger.error(f"Failed to initialize SABnzbd service: {e}", exc_info=True)


def run_app():
    """Run the application."""
    config = get_config()
    app = create_app()
    
    # Initialize SABnzbd in background
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    loop.run_until_complete(initialize_sabnzbd(app))
    
    # Run Flask app
    logger.info(f"Starting Fshare-Arr-Bridge on port {config.server.port}")
    app.run(
        host='0.0.0.0',
        port=config.server.port,
        debug=config.server.debug,
    )


if __name__ == '__main__':
    run_app()
