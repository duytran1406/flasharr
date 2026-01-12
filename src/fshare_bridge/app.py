"""
Fshare-Arr-Bridge Application

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

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# Load version
VERSION = "v2.0.0-beta.8"  # Fallback
try:
    # Check package root first
    pkg_version = Path(__file__).parent / "VERSION"
    if pkg_version.exists():
        VERSION = pkg_version.read_text().strip()
    else:
        # Check app root
        app_version = Path(__file__).parent.parent.parent / "VERSION"
        if app_version.exists():
            VERSION = app_version.read_text().strip()
except Exception as e:
    logger.warning(f"Could not load VERSION: {e}")


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
    
    def format_size(bytes):
        if bytes == 0: return '0 B'
        import math
        units = ['B', 'KB', 'MB', 'GB', 'TB']
        i = int(math.floor(math.log(bytes) / math.log(1024)))
        p = math.pow(1024, i)
        s = round(bytes / p, 2)
        return f"{s} {units[i]}"
    
    # Temporarily add Web UI stub APIs for compatibility
    # These will eventually be replaced with proper implementations
    @app.route('/api/stats')
    def api_stats():
        """Stats API for Web UI"""
        # Get primary account info
        primary = app.account_manager.get_primary()
        
        fshare_data = {
            "valid": primary is not None,
            "premium": primary.get('premium', False) if primary else False
        }
        
        if primary:
            # Add extra details if available
            if primary.get('validuntil'):
                try:
                    from datetime import datetime
                    if primary.get('validuntil') == -1:
                        fshare_data['expiry'] = "Lifetime"
                    else:
                        dt = datetime.fromtimestamp(primary.get('validuntil'))
                        fshare_data['expiry'] = dt.strftime('%d-%m-%Y')
                except:
                    pass
            
            fshare_data['traffic_left'] = primary.get('traffic_left')
            fshare_data['account_type'] = primary.get('account_type')
            fshare_data['email'] = primary.get('email')

        # Get downloader status
        sab_status = {"active": 0, "speed": 0, "total_size": 0, "connected": False}
        if app.sabnzbd:
            try:
                sab_status = app.sabnzbd.get_status()
            except:
                pass

        total_speed = sab_status.get('speed', 0)
        formatted_speed = "0 B/s"
        if total_speed > 1024 * 1024:
            formatted_speed = f"{total_speed / (1024 * 1024):.1f} MB/s"
        elif total_speed > 1024:
            formatted_speed = f"{total_speed / 1024:.1f} KB/s"
        else:
            formatted_speed = f"{total_speed} B/s"

        return jsonify({
            "fshare_downloader": {
                "active": sab_status.get('active', 0),
                "speed": formatted_speed,
                "speed_bytes": total_speed,
                "total": sab_status.get('queued', 0) + sab_status.get('active', 0),
                "connected": sab_status.get('connected', False),
                "primary_account": fshare_data
            }
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
                speed_bytes = slot.get('speed_bytes', 0)
                formatted_speed = "0 B/s"
                if speed_bytes > 1024 * 1024:
                    formatted_speed = f"{speed_bytes / (1024 * 1024):.1f} MB/s"
                elif speed_bytes > 1024:
                    formatted_speed = f"{speed_bytes / 1024:.1f} KB/s"
                else:
                    formatted_speed = f"{speed_bytes} B/s"

                total_bytes = slot.get('total_bytes', 0)
                formatted_total = format_size(total_bytes) if total_bytes > 0 else slot.get('mb', 'N/A')

                downloads.append({
                    "id": slot.get('nzo_id'),
                    "filename": slot.get('filename'),
                    "state": slot.get('status'),
                    "progress": int(slot.get('percentage', 0)),
                    "size": {"formatted_total": formatted_total, "total": total_bytes},
                    "speed": {"formatted": formatted_speed, "bytes_per_sec": speed_bytes},
                    "eta": {"formatted": slot.get('timeleft', '--:--'), "seconds": slot.get('eta_seconds', 0)},
                    "category": slot.get('cat', 'Unknown')
                })
            return jsonify({"downloads": downloads})
        except:
            return jsonify({"downloads": []})
    
    @app.route('/api/search')
    def api_search():
        """Search API using TimFshareClient via IndexerService"""
        query = request.args.get('q', '')
        if not query:
            return jsonify({"results": []})
            
        try:
            # Use IndexerService's client (TimFshare)
            # This avoids Fshare login requirements and uses the dedicated search engine
            results = app.indexer.client.search(query, limit=50)
            
            # Format for UI
            formatted_results = []
            for item in results:
                # Use the parser from indexer service
                parsed = app.indexer.parser.parse(item.name)
                
                formatted_results.append({
                    "id": item.fcode,
                    "name": parsed.title or item.name,
                    "original_filename": item.name,
                    "size": item.size,
                    "url": item.url,
                    "folder": False,
                    "score": item.score,
                    "is_series": parsed.is_series,
                    "season": parsed.season,
                    "episode": parsed.episode,
                    "quality": parsed.quality,
                    "vietsub": parsed.quality_attrs.viet_sub if parsed.quality_attrs else False,
                    "vietdub": parsed.quality_attrs.viet_dub if parsed.quality_attrs else False,
                    "metadata": parsed.quality_attrs.to_dict() if parsed.quality_attrs else {}
                })
                
            return jsonify({"results": formatted_results})
            
        except Exception as e:
            logger.error(f"Search API error: {e}")
            return jsonify({"error": str(e)}), 500
    
    @app.route('/api/autocomplete')
    def api_autocomplete():
        """Autocomplete API stub"""
        return jsonify({"suggestions": []})
    
    @app.route('/api/download', methods=['POST'])
    def api_download():
        """Download API: Start a download from search results"""
        if not app.sabnzbd:
            error_msg = getattr(app, 'init_error', 'Download engine not initialized')
            return jsonify({"success": False, "error": error_msg}), 503
            
        data = request.get_json()
        url = data.get('url')
        name = data.get('name')
        
        if not url:
            return jsonify({"success": False, "error": "URL is required"}), 400
            
        try:
            # Add to download engine
            # We don't always have a category from search, default to 'manual'
            nzo_id = app.sabnzbd.add_url(url, category='manual')
            
            if nzo_id:
                logger.info(f"Added download from search: {name or url} ({nzo_id})")
                return jsonify({"success": True, "nzo_id": nzo_id})
            else:
                return jsonify({"success": False, "error": "Failed to add to queue"})
                
        except Exception as e:
            logger.error(f"Error adding download: {e}")
            return jsonify({"success": False, "error": str(e)}), 500
    
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
        if app.sabnzbd and app.sabnzbd.toggle_item(nzo_id):
            return jsonify({"success": True, "action": "toggled"})
        return jsonify({"success": False, "error": "Item not found"}), 404
    
    @app.route('/api/download/delete/<nzo_id>', methods=['DELETE'])
    def api_delete_download(nzo_id):
        """Delete a download"""
        if app.sabnzbd and app.sabnzbd.delete_item(nzo_id):
            return jsonify({"success": True})
        return jsonify({"success": False, "error": "Item not found"}), 404
    
    @app.route('/api/settings', methods=['GET'])
    def api_get_settings():
        """Get current settings"""
        config = get_config()
        settings_file = Path("data/settings.json")
        
        # Default settings structure
        settings = {
            "download_path": config.download.download_dir,
            "max_concurrent_downloads": config.download.max_concurrent,
            "speed_limit_kbps": 0,
            "auto_resume": True,
            "category_paths": {
                "radarr": "movies",
                "sonarr": "tv",
                "lidarr": "music"
            },
            "base_url": "http://localhost:8484",
            "indexer_api_key": os.getenv("INDEXER_API_KEY", ""),
            "sabnzbd_api_key": os.getenv("SABNZBD_API_KEY", ""),
            "enable_indexer": True,
            "enable_sabnzbd": True,
            "theme": "dark",
            "language": "en",
            "refresh_interval": 3000
        }
        
        # Load from file if exists
        if settings_file.exists():
            try:
                with open(settings_file, 'r') as f:
                    saved = json.load(f)
                    settings.update(saved)
            except Exception as e:
                logger.error(f"Error loading settings file: {e}")
                
        return jsonify({
            "status": "ok",
            "settings": settings
        })
    
    @app.route('/api/settings', methods=['PUT'])
    def api_save_settings():
        """Save settings"""
        data = request.get_json()
        settings_file = Path("data/settings.json")
        settings_file.parent.mkdir(parents=True, exist_ok=True)
        
        try:
            with open(settings_file, 'w') as f:
                json.dump(data, f, indent=2)
            logger.info("Settings saved to data/settings.json")
            return jsonify({"status": "ok", "message": "Settings saved"})
        except Exception as e:
            logger.error(f"Error saving settings: {e}")
            return jsonify({"status": "error", "message": str(e)})
    
    @app.route('/api/settings/login-fshare', methods=['POST'])
    def api_login_fshare():
        """Login to Fshare and save credentials (legacy)"""
        data = request.get_json()
        email = data.get('email')
        password = data.get('password')
        
        if not email or not password:
            return jsonify({"status": "error", "message": "Email and password required"})
        
        try:
            account = app.account_manager.add_account(email, password)
            return jsonify({
                "status": "ok",
                "message": "Login successful",
                "account": account
            })
        except Exception as e:
            logger.error(f"Fshare login error: {e}")
            return jsonify({"status": "error", "message": str(e)})

    @app.route('/api/accounts', methods=['GET'])
    def api_list_accounts():
        """List all Fshare accounts"""
        try:
            accounts = app.account_manager.list_accounts()
            primary = app.account_manager.get_primary()
            logger.info(f"Returning {len(accounts)} accounts to UI")
            return jsonify({
                "status": "ok",
                "accounts": accounts,
                "primary": primary
            })
        except Exception as e:
            logger.error(f"Error listing accounts: {e}")
            return jsonify({"status": "error", "message": str(e)})

    @app.route('/api/accounts/add', methods=['POST'])
    def api_add_account():
        """Add and login to a new Fshare account"""
        data = request.get_json()
        email = data.get('email')
        password = data.get('password')
        
        if not email or not password:
            return jsonify({"status": "error", "message": "Email and password required"})
        
        try:
            account = app.account_manager.add_account(email, password)
            return jsonify({
                "status": "ok",
                "message": "Account added successfully",
                "account": account
            })
        except Exception as e:
            logger.error(f"Error adding account: {e}")
            return jsonify({"status": "error", "message": str(e)})

    @app.route('/api/accounts/<email>', methods=['DELETE'])
    def api_remove_account(email):
        """Remove an Fshare account"""
        try:
            app.account_manager.remove_account(email)
            return jsonify({"status": "ok", "message": "Account removed"})
        except Exception as e:
            return jsonify({"status": "error", "message": str(e)})

    @app.route('/api/accounts/<email>/set-primary', methods=['POST'])
    def api_set_primary_account(email):
        """Set an account as primary"""
        try:
            app.account_manager.set_primary(email)
            return jsonify({"status": "ok", "message": "Primary account updated"})
        except Exception as e:
            return jsonify({"status": "error", "message": str(e)})

    @app.route('/api/accounts/<email>/refresh', methods=['POST'])
    def api_refresh_account(email):
        """Refresh account credentials/info"""
        try:
            account = app.account_manager.refresh_account(email)
            return jsonify({
                "status": "ok", 
                "message": "Account refreshed",
                "account": account
            })
        except Exception as e:
            return jsonify({"status": "error", "message": str(e)})

    @app.route('/api/settings/logout-fshare', methods=['POST'])
    def api_logout_fshare():
        """Logout from Fshare"""
        app.fshare_account = None
        app.fshare_credentials = None
        return jsonify({"status": "ok", "message": "Logged out"})
    
    @app.route('/api/settings/fshare-status', methods=['GET'])
    def api_fshare_status():
        """Get Fshare login status"""
        primary = app.account_manager.get_primary()
        if primary:
            return jsonify({
                "status": "ok",
                "logged_in": True,
                "account": primary
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
        return render_template('index.html', version=VERSION)
    
    @app.route('/search')
    def search_page():
        """Search interface page"""
        return render_template('search.html', version=VERSION)
    
    @app.route('/downloads')
    def downloads_page():
        """Downloads page"""
        return render_template('downloads.html', version=VERSION)
    
    @app.route('/settings')
    def settings_page():
        """Settings page"""
        return render_template('settings.html', version=VERSION)
    
    @app.route('/tutorial')
    def tutorial_page():
        """Tutorial page"""
        return render_template('tutorial.html', version=VERSION)
    
    @app.route('/about')
    def about_page():
        """About page"""
        return render_template('about.html', version=VERSION)
    
    @app.route('/api/info')
    def api_info():
        """API information endpoint."""
        return jsonify({
            "name": "Fshare-Arr-Bridge",
            "version": VERSION,
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
    
    # Run Flask app
    logger.info(f"Starting Fshare-Arr-Bridge on port {config.server.port}")
    app.run(
        host='0.0.0.0',
        port=config.server.port,
        debug=config.server.debug,
        use_reloader=False # Disable reloader to prevent creating two loops
    )


if __name__ == '__main__':
    run_app()
