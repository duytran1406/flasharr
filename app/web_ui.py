"""
Web UI Blueprint
Modern dashboard for Fshare-Arr Bridge
"""

from flask import Blueprint, render_template, jsonify, request
import logging
import psutil
import time
import threading
try:
    import speedtest
except ImportError:
    speedtest = None

logger = logging.getLogger(__name__)

# Global cache for speedtest results
speedtest_cache = {
    'download': 0,
    'upload': 0,
    'ping': 0,
    'last_updated': 0
}
speedtest_lock = threading.Lock()

def run_speedtest():
    """Run speedtest in background"""
    global speedtest_cache
    if not speedtest:
        logger.warning("speedtest-cli not installed")
        return

    try:
        logger.info("🎬 Starting background speedtest...")
        st = speedtest.Speedtest()
        st.get_best_server()
        download = st.download() / 1_000_000  # Mbps
        upload = st.upload() / 1_000_000      # Mbps
        results = st.results.dict()
        
        with speedtest_lock:
            speedtest_cache['download'] = round(download, 1)
            speedtest_cache['upload'] = round(upload, 1)
            speedtest_cache['ping'] = results.get('ping', 0)
            speedtest_cache['last_updated'] = time.time()
        
        logger.info(f"✅ Speedtest complete: Down: {speedtest_cache['download']} Mbps")
    except Exception as e:
        logger.error(f"❌ Speedtest failed: {e}")

def create_web_ui(timfshare_client, pyload_client, filename_normalizer):
    """Create and configure the Web UI blueprint"""
    
    web_ui_bp = Blueprint('web_ui', __name__, 
                          template_folder='templates')
    
    # Persistent stats (mocked for this session)
    boot_time = time.time()
    
    # Initial speedtest
    threading.Thread(target=run_speedtest, daemon=True).start()
    
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
            results = timfshare_client.search(query, limit=40)
            formatted_results = []
            for result in results:
                formatted_results.append({
                    'name': result.get('name', ''),
                    'url': result.get('url', ''),
                    'size': result.get('size', 0),
                    'score': result.get('score', 0),
                    'fcode': result.get('fcode', '')
                })
            return jsonify({'results': formatted_results})
        except Exception as e:
            logger.error(f"Search API error: {e}")
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
            parsed = filename_normalizer.parse(name)
            normalized_name = parsed.normalized_filename
            logger.info(f"Adding download: {name}")
            
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
            downloads = pyload_client.get_queue() or []
            return jsonify({'downloads': downloads})
        except Exception as e:
            logger.error(f"Downloads API error: {e}")
            return jsonify({'error': str(e)}), 500
    
    @web_ui_bp.route('/api/stats')
    def api_stats():
        """Get statistics for the homepage-style dashboard"""
        try:
            # Refresh speedtest if older than 1 hour
            if time.time() - speedtest_cache['last_updated'] > 3600:
                threading.Thread(target=run_speedtest, daemon=True).start()

            # pyLoad stats
            active_downloads = 0
            speed = "0 B/s"
            total_downloads = 0
            
            status = pyload_client.get_status()
            if status:
                active_downloads = status.get('active', 0)
                speed = status.get('speed_format', "0 B/s")
                total_downloads = status.get('total', 0)
            
            return jsonify({
                'system': {
                    'uptime': str(int(time.time() - boot_time)),
                    'speedtest': f"{speedtest_cache['download']} Mbps"
                },
                'pyload': {
                    'active': active_downloads,
                    'speed': speed,
                    'total': total_downloads,
                    'connected': pyload_client.logged_in if hasattr(pyload_client, 'logged_in') else False
                },
                'bridge': {
                    'searches': 42,
                    'success_rate': '100%'
                }
            })
        except Exception as e:
            logger.error(f"Stats API error: {e}")
            return jsonify({'error': str(e)}), 500
    
    return web_ui_bp
