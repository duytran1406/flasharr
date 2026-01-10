"""
Web UI Blueprint
Modern dashboard for Fshare-Arr Bridge
"""

from flask import Blueprint, render_template, jsonify, request
import logging
import psutil
import time
from datetime import datetime

logger = logging.getLogger(__name__)

def create_web_ui(timfshare_client, pyload_client, filename_normalizer):
    """Create and configure the Web UI blueprint"""
    
    web_ui_bp = Blueprint('web_ui', __name__, 
                          template_folder='templates')
    
    # Persistent stats (mocked for this session)
    boot_time = time.time()
    
    @web_ui_bp.route('/')
    def index():
        """Dashboard page"""
        pyload_status = pyload_client.logged_in if hasattr(pyload_client, 'logged_in') else False
        return render_template('index.html', pyload_status=pyload_status)
    
    @web_ui_bp.route('/search')
    def search_page():
        """Search interface page"""
        return render_template('search.html')
    
    @web_ui_bp.route('/downloads')
    def downloads_page():
        """Downloads page"""
        return render_template('downloads.html')
    
    # API Endpoints
    
    @web_ui_bp.route('/api/search')
    def api_search():
        """Search API endpoint"""
        query = request.args.get('q', '')
        
        if not query:
            return jsonify({'results': []})
        
        try:
            # Use smart search from TimFshare client
            results = timfshare_client.search(query, limit=40)
            
            # Format results for frontend
            formatted_results = []
            for result in results:
                formatted_results.append({
                    'name': result.get('name', ''),
                    'url': result.get('url', ''),
                    'size': result.get('size', 0),
                    'score': result.get('score', 0),
                    'fcode': result.get('fcode', '')
                })
            
            logger.info(f"Search for '{query}' returned {len(formatted_results)} results")
            return jsonify({'results': formatted_results})
            
        except Exception as e:
            logger.error(f"Search API error: {e}")
            return jsonify({'error': str(e)}), 500
    
    @web_ui_bp.route('/api/autocomplete')
    def api_autocomplete():
        """Autocomplete API endpoint"""
        query = request.args.get('q', '')
        
        if not query or len(query) < 2:
            return jsonify({'suggestions': []})
        
        try:
            suggestions = timfshare_client.autocomplete(query)
            return jsonify({'suggestions': suggestions[:10]})
            
        except Exception as e:
            logger.error(f"Autocomplete API error: {e}")
            return jsonify({'error': str(e)}), 500
    
    @web_ui_bp.route('/api/download', methods=['POST'])
    def api_download():
        """Add download to pyLoad"""
        data = request.get_json()
        url = data.get('url')
        name = data.get('name')
        
        if not url or not name:
            return jsonify({'success': False, 'error': 'Missing url or name'}), 400
        
        try:
            # Normalize filename
            parsed = filename_normalizer.parse(name)
            normalized_name = parsed.normalized_filename
            
            logger.info(f"Adding download: {name}")
            
            # Send to pyLoad
            success = pyload_client.add_download(url, filename=normalized_name)
            
            if success:
                return jsonify({'success': True, 'normalized': normalized_name})
            else:
                return jsonify({'success': False, 'error': 'pyLoad failed'}), 500
                
        except Exception as e:
            logger.error(f"Download API error: {e}")
            return jsonify({'success': False, 'error': str(e)}), 500
    
    @web_ui_bp.route('/api/downloads')
    def api_downloads():
        """Get download queue from pyLoad"""
        try:
            # Get actual queue from pyLoad
            if hasattr(pyload_client, 'get_queue'):
                downloads = pyload_client.get_queue()
            else:
                downloads = []
                
            return jsonify({'downloads': downloads})
            
        except Exception as e:
            logger.error(f"Downloads API error: {e}")
            return jsonify({'error': str(e)}), 500
    
    @web_ui_bp.route('/api/stats')
    def api_stats():
        """Get statistics for the homepage-style dashboard"""
        try:
            # System stats
            cpu_percent = psutil.cpu_percent()
            mem = psutil.virtual_memory()
            mem_used_gb = mem.used / (1024**3)
            
            # pyLoad stats (simplified)
            active_downloads = 0
            speed = "0 B/s"
            total_downloads = 0
            
            if hasattr(pyload_client, 'get_status'):
                status = pyload_client.get_status()
                active_downloads = status.get('active', 0)
                speed = status.get('speed_format', "0 B/s")
                total_downloads = status.get('total', 0)
            
            return jsonify({
                'system': {
                    'cpu': f"{cpu_percent}%",
                    'ram': f"{mem_used_gb:.1f} GiB",
                    'uptime': str(int(time.time() - boot_time))
                },
                'pyload': {
                    'active': active_downloads,
                    'speed': speed,
                    'total': total_downloads,
                    'connected': pyload_client.logged_in if hasattr(pyload_client, 'logged_in') else False
                },
                'bridge': {
                    'searches': 42, # Mock search count
                    'success_rate': '100%'
                }
            })
        except Exception as e:
            logger.error(f"Stats API error: {e}")
            return jsonify({'error': str(e)}), 500
    
    return web_ui_bp
