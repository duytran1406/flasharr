"""
Fshare-Arr-Bridge Application

Main entry point for the refactored bridge application.
Integrates all services with built-in download engine.
"""

import asyncio
import logging
from pathlib import Path

from flask import Flask, request, jsonify, send_file, render_template
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
    # Get the directory where this file is located
    base_dir = Path(__file__).parent
    
    app = Flask(__name__,
                template_folder=str(base_dir / 'templates'),
                static_folder=str(base_dir / 'static'))
    CORS(app)
    
    # Create services
    app.indexer = create_indexer_service()
    app.sabnzbd = None  # Will be initialized async
    
    # Temporarily add Web UI stub APIs for compatibility
    # These will eventually be replaced with proper implementations
    @app.route('/api/stats')
    def api_stats():
        """Stats API for Web UI"""
        return jsonify({
            "system": {"uptime": "0", "speedtest": "0 Mbps"},
            "pyload": {
                "active": 0,
                "speed": "0 B/s",
                "speed_bytes": 0,
                "total": 0,
                "connected": app.sabnzbd is not None,
                "fshare_account": {"valid": False, "premium": False}
            },
            "bridge": {"searches": 0, "success_rate": "100%"}
        })
    
    @app.route('/api/downloads')
    def api_downloads():
        """Downloads API for Web UI"""
        if not app.sabnzbd:
            return jsonify({"downloads": []})
        try:
            queue = app.sabnzbd.get_queue()
            downloads = []
            for slot in queue.get('queue', {}).get('slots', []):
                downloads.append({
                    "id": slot.get('nzo_id'),
                    "filename": slot.get('filename'),
                    "state": slot.get('status'),
                    "progress": int(slot.get('percentage', 0)),
                    "size": {"formatted_total": slot.get('mb', 'N/A'), "total": 0},
                    "speed": {"formatted": slot.get('speed', '0 B/s'), "bytes_per_sec": 0},
                    "eta": {"formatted": slot.get('eta', '--:--'), "seconds": 0},
                    "category": slot.get('cat', 'Unknown')
                })
            return jsonify({"downloads": downloads})
        except:
            return jsonify({"downloads": []})
    
    @app.route('/api/search')
    def api_search():
        """Search API stub - not implemented yet"""
        return jsonify({"results": []})
    
    @app.route('/api/autocomplete')
    def api_autocomplete():
        """Autocomplete API stub"""
        return jsonify({"suggestions": []})
    
    @app.route('/api/download', methods=['POST'])
    def api_download():
        """Download API stub"""
        return jsonify({"success": False, "error": "Not implemented"})
    
    @app.route('/api/logs')
    def api_logs():
        """Logs API stub"""
        return jsonify({"logs": []})
    
    @app.route('/api/downloads/start_all', methods=['POST'])
    def api_start_all():
        """Start all downloads"""
        if app.sabnzbd:
            app.sabnzbd.resume_queue()
            return jsonify({"success": True})
        return jsonify({"success": False})
    
    @app.route('/api/downloads/pause_all', methods=['POST'])
    def api_pause_all():
        """Pause all downloads"""
        if app.sabnzbd:
            app.sabnzbd.pause_queue()
            return jsonify({"success": True})
        return jsonify({"success": False})
    
    @app.route('/api/downloads/stop_all', methods=['POST'])
    def api_stop_all():
        """Stop all downloads"""
        return jsonify({"success": True})
    
    @app.route('/api/download/toggle/<nzo_id>', methods=['POST'])
    def api_toggle_download(nzo_id):
        """Toggle download pause/resume"""
        return jsonify({"success": True, "action": "toggled"})
    
    @app.route('/api/download/delete/<nzo_id>', methods=['DELETE'])
    def api_delete_download(nzo_id):
        """Delete a download"""
        return jsonify({"success": True})
    
    # Settings API endpoints
    @app.route('/api/settings', methods=['GET'])
    def api_get_settings():
        """Get current settings"""
        return jsonify({
            "status": "ok",
            "settings": {
                "fshare_email": "",
                "fshare_password": "",
                "download_path": "/downloads",
                "max_concurrent_downloads": 3,
                "speed_limit_kbps": 0,
                "auto_resume": True,
                "category_paths": {
                    "radarr": "movies",
                    "sonarr": "tv",
                    "lidarr": "music"
                },
                "base_url": "http://localhost:8484",
                "indexer_api_key": "",
                "sabnzbd_api_key": "",
                "enable_indexer": True,
                "enable_sabnzbd": True,
                "theme": "dark",
                "language": "en",
                "refresh_interval": 3000
            }
        })
    
    @app.route('/api/settings', methods=['PUT'])
    def api_save_settings():
        """Save settings"""
        data = request.get_json()
        # TODO: Actually save settings to config file
        return jsonify({"status": "ok", "message": "Settings saved"})
    
    @app.route('/api/settings/login-fshare', methods=['POST'])
    def api_login_fshare():
        """Login to Fshare and save credentials"""
        data = request.get_json()
        email = data.get('email')
        password = data.get('password')
        
        if not email or not password:
            return jsonify({"status": "error", "message": "Email and password required"})
        
        try:
            # Use the Fshare client from app.sabnzbd if available
            from .clients.fshare import FshareClient
            from .core.config import FshareConfig
            
            # Create temporary client to test login
            config = FshareConfig(email=email, password=password)
            client = FshareClient.from_config(config)
            
            # Attempt login
            try:
                success = client.login()
                
                if success:
                    # Get account info
                    account_info = {
                        "email": email,
                        "premium": client.is_premium,
                        "validuntil": getattr(client, 'premium_expiry', None),
                        "logged_in": True
                    }
                    
                    # TODO: Save credentials to config file for persistence
                    # For now, store in app context
                    app.fshare_account = account_info
                    app.fshare_credentials = {"email": email, "password": password}
                    
                    return jsonify({
                        "status": "ok",
                        "message": "Login successful",
                        "account": account_info
                    })
                else:
                    return jsonify({"status": "error", "message": "Login failed"})
                    
            except Exception as login_error:
                # Log the detailed error
                logger.error(f"Fshare login error: {login_error}", exc_info=True)
                
                # Return user-friendly error message
                error_msg = str(login_error)
                if "AuthenticationError" in type(login_error).__name__:
                    error_msg = "Invalid email or password"
                elif "FshareConnectionError" in type(login_error).__name__:
                    error_msg = "Could not connect to Fshare"
                elif "Login request failed" in error_msg:
                    error_msg = "Invalid credentials or Fshare API error"
                
                return jsonify({"status": "error", "message": error_msg})
                
        except Exception as e:
            logger.error(f"Fshare login error: {e}", exc_info=True)
            return jsonify({"status": "error", "message": f"Login error: {str(e)}"})
    
    @app.route('/api/settings/logout-fshare', methods=['POST'])
    def api_logout_fshare():
        """Logout from Fshare"""
        app.fshare_account = None
        app.fshare_credentials = None
        return jsonify({"status": "ok", "message": "Logged out"})
    
    @app.route('/api/settings/fshare-status', methods=['GET'])
    def api_fshare_status():
        """Get Fshare login status"""
        if hasattr(app, 'fshare_account') and app.fshare_account:
            return jsonify({
                "status": "ok",
                "logged_in": True,
                "account": app.fshare_account
            })
        return jsonify({
            "status": "ok",
            "logged_in": False
        })
    
    @app.route('/api/settings/generate-api-key', methods=['POST'])
    def api_generate_key():
        """Generate API key"""
        import secrets
        data = request.get_json()
        key_type = data.get('type', 'indexer')
        new_key = secrets.token_urlsafe(32)
        return jsonify({"status": "ok", "key": new_key})
    
    @app.route('/api/settings/export', methods=['GET'])
    def api_export_settings():
        """Export settings"""
        import json
        settings = {
            "version": "2.0.0-alpha",
            "settings": {}
        }
        return jsonify({"status": "ok", "data": json.dumps(settings, indent=2)})
    
    @app.route('/api/settings/reset', methods=['POST'])
    def api_reset_settings():
        """Reset settings to defaults"""
        return jsonify({"status": "ok", "message": "Settings reset"})
    
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
    
    @app.route('/')
    def index():
        """Main dashboard."""
        return render_template('index.html')
    
    @app.route('/search')
    def search_page():
        """Search interface page"""
        return render_template('search.html')
    
    @app.route('/downloads')
    def downloads_page():
        """Downloads page"""
        return render_template('downloads.html')
    
    @app.route('/settings')
    def settings_page():
        """Settings page"""
        return render_template('settings.html')
    
    @app.route('/tutorial')
    def tutorial_page():
        """Tutorial page"""
        return render_template('tutorial.html')
    
    @app.route('/about')
    def about_page():
        """About page"""
        return render_template('about.html')
    
    @app.route('/api/info')
    def api_info():
        """API information endpoint."""
        return jsonify({
            "name": "Fshare-Arr-Bridge",
            "version": "2.0.0-alpha",
            "description": "Integration bridge for Fshare.vn with Sonarr/Radarr",
            "endpoints": {
                "health": "/health",
                "indexer_api": "/indexer/api (Newznab compatible)",
                "indexer_caps": "/indexer/api?t=caps",
                "indexer_search": "/indexer/api?t=search&q=query",
                "sabnzbd_api": "/sabnzbd/api (SABnzbd compatible)",
                "nzb_download": "/nzb/{guid}"
            },
            "documentation": {
                "newznab": "https://newznab.readthedocs.io/",
                "sabnzbd": "https://sabnzbd.org/wiki/advanced/api"
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
