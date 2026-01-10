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
    
    @web_ui_bp.route('/api/autocomplete')
    def api_autocomplete():
        """Autocomplete API endpoint - returns top 3 suggestions"""
        query = request.args.get('q', '')
        
        if not query or len(query) < 2:
            return jsonify({'suggestions': []})
        
        try:
            results = timfshare_client.search(query, limit=3)
            suggestions = [result.get('name', '') for result in results if result.get('name')]
            return jsonify({'suggestions': suggestions})
        except Exception as e:
            logger.error(f"Autocomplete API error: {e}")
            return jsonify({'suggestions': []})
    
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
            # Get container uptime from Docker
            container_uptime = "0s"
            try:
                import subprocess
                from datetime import datetime
                result = subprocess.run(
                    ['docker', 'inspect', '-f', '{{.State.StartedAt}}', 'fshare-arr-bridge'],
                    capture_output=True, text=True, timeout=2
                )
                if result.returncode == 0:
                    started_at = result.stdout.strip()
                    start_time = datetime.fromisoformat(started_at.replace('Z', '+00:00'))
                    uptime_seconds = int((datetime.now(start_time.tzinfo) - start_time).total_seconds())
                    container_uptime = str(uptime_seconds)
            except Exception as e:
                logger.warning(f"Failed to get container uptime: {e}")
                container_uptime = str(int(time.time() - boot_time))  # Fallback

            # pyLoad stats
            active_downloads = 0
            speed = "0 B/s"
            speed_bytes = 0
            total_downloads = 0
            fshare_account = {'valid': False, 'premium': False}
            
            status = pyload_client.get_status()
            if status:
                active_downloads = status.get('active', 0)
                speed = status.get('speed_format', "0 B/s")
                speed_bytes = status.get('speed', 0)  # Get raw speed in bytes/s
                total_downloads = status.get('total', 0)
            
            # Format speed for header (convert bytes/s to Mbps)
            net_speed = "0 Mbps"
            if speed_bytes > 0:
                mbps = (speed_bytes * 8) / (1024 * 1024)  # Convert bytes/s to Mbps
                net_speed = f"{mbps:.1f} Mbps"
            
            # Get Fshare account status from pyLoad
            try:
                accounts_response = pyload_client.session.get(
                    f"{pyload_client.base_url}/api/get_accounts?refresh=false",
                    timeout=5
                )
                if accounts_response.status_code == 200:
                    accounts = accounts_response.json()
                    for account in accounts:
                        if account.get('type', '').lower() == 'fsharevn':
                            fshare_account = {
                                'valid': account.get('valid', False),
                                'premium': account.get('premium', False),
                                'validuntil': account.get('validuntil', 0),
                                'login': account.get('login', '')
                            }
                            break
            except Exception as e:
                logger.warning(f"Failed to get Fshare account status: {e}")
            
            return jsonify({
                'system': {
                    'uptime': container_uptime,
                    'speedtest': net_speed
                },
                'pyload': {
                    'active': active_downloads,
                    'speed': speed,
                    'total': total_downloads,
                    'connected': pyload_client.logged_in if hasattr(pyload_client, 'logged_in') else False,
                    'fshare_account': fshare_account
                },
                'bridge': {
                    'searches': 42,
                    'success_rate': '100%'
                }
            })
        except Exception as e:
            logger.error(f"Stats API error: {e}")
            return jsonify({'error': str(e)}), 500
    
    @web_ui_bp.route('/api/logs')
    def api_logs():
        """Get recent container logs"""
        try:
            import subprocess
            result = subprocess.run(
                ['docker', 'logs', '--tail', '10', 'fshare-arr-bridge'],
                capture_output=True, text=True, timeout=5
            )
            
            logs = []
            if result.returncode == 0:
                lines = result.stdout.strip().split('\n')
                for line in lines[-5:]:  # Last 5 lines
                    if not line.strip():
                        continue
                    
                    # Parse log level
                    level = 'info'
                    if 'ERROR' in line or '❌' in line:
                        level = 'error'
                    elif 'WARNING' in line or 'WARN' in line:
                        level = 'warning'
                    elif 'SUCCESS' in line or '✅' in line:
                        level = 'success'
                    
                    # Extract timestamp if present
                    timestamp = '[Recent]'
                    if line.startswith('2026-'):
                        parts = line.split(' ', 2)
                        if len(parts) >= 2:
                            timestamp = f"[{parts[1].split(',')[0]}]"
                            line = parts[2] if len(parts) > 2 else line
                    
                    logs.append({
                        'time': timestamp,
                        'message': line[:100],  # Truncate long messages
                        'level': level
                    })
            
            return jsonify({'logs': logs})
        except Exception as e:
            logger.error(f"Logs API error: {e}")
            return jsonify({'logs': []})
    
    @web_ui_bp.route('/api/download/toggle/<int:fid>', methods=['POST'])
    def api_toggle_download(fid):
        """Toggle (pause/resume) a download in pyLoad"""
        try:
            # First check if the download is currently running
            status_response = pyload_client.session.get(
                f"{pyload_client.base_url}/api/status_downloads",
                timeout=5
            )
            
            is_running = False
            if status_response.status_code == 200:
                active_downloads = status_response.json()
                is_running = any(d.get('fid') == fid for d in active_downloads)
            
            if is_running:
                # Stop/Pause the download
                response = pyload_client.session.post(
                    f"{pyload_client.base_url}/api/stop_downloads",
                    json={'file_ids': [fid]},
                    timeout=5
                )
            else:
                # Resume the download
                response = pyload_client.session.post(
                    f"{pyload_client.base_url}/api/restart_file/{fid}",
                    timeout=5
                )
            
            if response.status_code == 200:
                return jsonify({'success': True, 'action': 'stopped' if is_running else 'resumed'})
            else:
                return jsonify({'success': False, 'error': 'pyLoad API failed'}), 500
        except Exception as e:
            logger.error(f"Toggle download error: {e}")
            return jsonify({'success': False, 'error': str(e)}), 500
    
    @web_ui_bp.route('/api/download/delete/<int:fid>', methods=['DELETE'])
    def api_delete_download(fid):
        """Delete a download from pyLoad"""
        try:
            response = pyload_client.session.post(
                f"{pyload_client.base_url}/api/delete_packages",
                json=[fid],
                timeout=5
            )
            if response.status_code == 200:
                return jsonify({'success': True})
            else:
                return jsonify({'success': False, 'error': 'pyLoad API failed'}), 500
        except Exception as e:
            logger.error(f"Delete download error: {e}")
            return jsonify({'success': False, 'error': str(e)}), 500
    
    return web_ui_bp
