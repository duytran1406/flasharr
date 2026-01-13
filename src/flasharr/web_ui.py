"""
Web UI Blueprint
Modern dashboard for Flasharr
"""

from flask import Blueprint, render_template, jsonify, request
import logging
import psutil
import time
import threading
import difflib
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
    
    # Load version
    try:
        with open('VERSION', 'r') as f:
            app_version = f.read().strip()
    except:
        app_version = '0.0.0'
    
    # Persistent stats (mocked for this session)
    boot_time = time.time()
    
    # Initial speedtest
    threading.Thread(target=run_speedtest, daemon=True).start()
    
    @web_ui_bp.route('/')
    def index():
        """Dashboard page"""
        pyload_status = pyload_client.logged_in if hasattr(pyload_client, 'logged_in') else False
        return render_template('index.html', pyload_status=pyload_status, version=app_version)
    
    @web_ui_bp.route('/search')
    def search_page():
        """Search interface page"""
        return render_template('search.html', version=app_version)
    
    @web_ui_bp.route('/downloads')
    def downloads_page():
        """Downloads page"""
        return render_template('downloads.html', version=app_version)
    
    @web_ui_bp.route('/settings')
    def settings_page():
        """Settings page"""
        return render_template('settings.html', version=app_version)
    
    # API Endpoints
    
    @web_ui_bp.route('/api/search')
    def api_search():
        """Search API endpoint"""
        query = request.args.get('q', '')
        
        if not query:
            return jsonify({'results': []})
        
        try:
            # Video extensions to filter
            VIDEO_EXTENSIONS = ('.mp4', '.avi', '.mov', '.mkv', '.m4v', '.flv', '.mpeg', '.wav')
            
            # Use client-side filtering and limit
            results = timfshare_client.search(query, limit=100, extensions=VIDEO_EXTENSIONS)
            formatted_results = []
            
            for result in results:
                raw_name = result.get('name', '')
                
                # Parse filename
                parsed = filename_normalizer.parse(raw_name)
                
                # Calculate score based on quality markers
                score = calculate_quality_score(parsed, raw_name, query=query)
                
                # Format file size
                size_bytes = result.get('size', 0)
                size_formatted = format_file_size(size_bytes)
                
                # Detect quality badge
                quality_badge = detect_quality_badge(raw_name)
                
                # Detect vietsub/vietdub
                name_lower = raw_name.lower()
                has_vietsub = any(marker in name_lower for marker in ['vietsub', 'tvp', 'vie.sub', 'phụ đề', 'phu de'])
                has_vietdub = any(marker in name_lower for marker in ['vietdub', 'tmpđ', 'thuyết minh', 'lồng tiếng', 'vie.dub'])
                
                formatted_results.append({
                    'name': parsed.title,  # Use parsed title
                    'url': result.get('url', ''),
                    'size': size_formatted,  # Formatted size
                    'size_bytes': size_bytes,  # Keep for sorting
                    'score': score,  # Quality score
                    'fcode': result.get('fcode', ''),
                    'quality': quality_badge,
                    'vietsub': has_vietsub,
                    'vietdub': has_vietdub,
                    'is_series': parsed.is_series,
                    'season': parsed.season,
                    'episode': parsed.episode
                })
            
            # Separate Series and Movies
            series_results = [r for r in formatted_results if r['is_series']]
            movie_results = [r for r in formatted_results if not r['is_series']]
            
            # Sort Series: Season ASC -> Episode ASC -> Score DESC
            series_results.sort(key=lambda x: (
                x['season'] if x['season'] is not None else 9999,
                x['episode'] if x['episode'] is not None else 9999,
                -x['score']
            ))
            
            # Sort Movies: Score DESC -> Name ASC
            movie_results.sort(key=lambda x: (-x['score'], x['name']))
            
            # Combine: Series first (likely intent if detected), then Movies
            final_results = series_results + movie_results
            
            return jsonify({'results': final_results})
        except Exception as e:
            logger.error(f"Search API error: {e}")
            return jsonify({'error': str(e)}), 500
    
    def calculate_quality_score(parsed, filename, query=''):
        """
        Calculate score using multi-factor matching algorithm.
        
        Score Breakdown:
        - Accuracy (0-90): Multi-factor matching (exact, word-based, fuzzy)
        - Quality/Metadata (0-10): Tie-breaker for quality and language
        """
        if not query:
            return 50  # Default if no query
            
        # 1. Calculate Accuracy Score (0-90) using improved algorithm
        accuracy_score = calculate_accuracy_score(query, parsed.title)
        
        # 2. Tie-Breaker Bonuses (0-10)
        bonus_score = get_quality_profile_score(filename)
            
        # Language Bonus (+2)
        filename_lower = filename.lower()
        if any(marker in filename_lower for marker in ['vietsub', 'tvp', 'vietdub', 'tmpđ', 'thuyết minh', 'lồng tiếng']):
             bonus_score += 2
             
        final_score = accuracy_score + min(bonus_score, 10)
        
        return min(final_score, 100)
    
    def calculate_accuracy_score(query, title):
        """
        Multi-factor accuracy scoring algorithm.
        
        Returns score 0-90 based on how well query matches title.
        Uses hierarchical matching: exact > word-perfect > prefix > substring > token > fuzzy
        """
        # Normalize inputs
        q_norm = query.lower().strip()
        t_norm = title.lower().strip()
        
        # 1. Exact Match (90 points)
        if q_norm == t_norm:
            return 90
        
        # 2. Tokenize for word-based matching
        q_tokens = q_norm.split()
        t_tokens = t_norm.split()
        q_tokens_set = set(q_tokens)
        t_tokens_set = set(t_tokens)
        
        # 3. Word-Perfect Match (80 points)
        # All query words present in title
        if q_tokens_set.issubset(t_tokens_set):
            # Bonus if words appear in same order
            if is_subsequence(q_tokens, t_tokens):
                return 80
            return 75
        
        # 4. Prefix Match (75 points)
        if t_norm.startswith(q_norm):
            return 75
        
        # 5. Substring Match (70 points)
        if q_norm in t_norm:
            return 70
        
        # 6. Token-Based Scoring (0-65 points)
        matched_tokens = len(q_tokens_set & t_tokens_set)
        if matched_tokens > 0:
            # Base score from match ratio
            match_ratio = matched_tokens / len(q_tokens)
            base_score = match_ratio * 65
            
            # Penalty for extra words in title (reduces relevance)
            extra_words = max(0, len(t_tokens) - len(q_tokens))
            penalty = min(extra_words * 2, 15)  # Cap penalty at 15
            
            return int(max(base_score - penalty, 0))
        
        # 7. Fuzzy Fallback (0-40 points)
        # For cases with no word matches, use character-based similarity
        matcher = difflib.SequenceMatcher(None, q_norm, t_norm)
        return int(matcher.ratio() * 40)
    
    def is_subsequence(subseq, seq):
        """Check if subseq is a subsequence of seq (maintains order)."""
        it = iter(seq)
        return all(item in it for item in subseq)

    def get_quality_profile_score(filename):
        """
        Get score (0-8) based on *arr Quality Profile hierarchy.
        
        Hierarchy:
        8: Remux / ISO / 2160p (with Source)
        7: 1080p BluRay
        6: 1080p WEB-DL
        5: 1080p (Generic)
        4: 720p BluRay / WEB-DL
        3: 720p (Generic)
        2: HDTV / PDTV
        1: SD / DVD / 480p
        0: CAM / TS / Unknown
        """
        f = filename.lower()
        
        # 1. Check for Remux/ISO (Top Tier)
        if 'remux' in f or 'iso' in f:
            return 8
            
        # 2. Check Resolution & Source
        is_4k = any(k in f for k in ['2160p', '4k', 'uhd', '8k'])
        is_1080p = '1080p' in f
        is_720p = '720p' in f
        
        is_bluray = any(k in f for k in ['bluray', 'blu-ray', 'bdrip', 'brrip'])
        is_web = any(k in f for k in ['web-dl', 'webdl', 'webrip'])
        is_hdtv = any(k in f for k in ['hdtv', 'pdtv', 'tvrip'])
        is_sd = any(k in f for k in ['480p', '576p', 'dvd', 'sd'])
        
        if is_4k:
            return 8 # 4K matches top tier in this simple scale
            
        if is_1080p:
            if is_bluray: return 7
            if is_web: return 6
            return 5
            
        if is_720p:
            if is_bluray or is_web: return 4
            return 3
            
        if is_hdtv:
            return 2
            
        if is_sd:
            return 1
            
        return 0
    
    def format_file_size(size_bytes):
        """Format file size in human-readable format"""
        if not size_bytes or size_bytes == 0:
            return 'N/A'
        
        # Convert to appropriate unit
        for unit in ['B', 'KB', 'MB', 'GB', 'TB']:
            if size_bytes < 1024.0:
                if unit == 'B':
                    return f"{int(size_bytes)}{unit}"
                return f"{size_bytes:.2f}{unit}"
            size_bytes /= 1024.0
        return f"{size_bytes:.2f}PB"
    
    def detect_quality_badge(filename):
        """Detect quality badge from filename"""
        filename_lower = filename.lower()
        
        # High Def
        if any(k in filename_lower for k in ['4k', '2160p', 'uhd', '8k', '4320p']):
            return '4K'
        if '1080p' in filename_lower:
            return '1080P'
        if '720p' in filename_lower:
            return '720P'
            
        # Sources
        if any(k in filename_lower for k in ['remux', 'iso']):
            return 'BluRay' # Treat Remux/ISO as high quality BluRay category
        if any(k in filename_lower for k in ['bluray', 'blu-ray', 'bdrip', 'brrip']):
            return 'BluRay'
        if any(k in filename_lower for k in ['web-dl', 'webdl', 'webrip']):
            return 'WEB-DL'
        
        # TV
        if any(k in filename_lower for k in ['hdtv', 'pdtv', 'tvrip']):
            return 'HDTV'
            
        # HDR/SDR
        if any(k in filename_lower for k in ['hdr', 'dolby vision', 'dv', 'hdr10']):
            return 'HDR'
        if any(k in filename_lower for k in ['cam', 'ts', 'tc']):
            return 'CAM'
            
        # Standard Def / DVD
        if any(k in filename_lower for k in ['480p', '576p', 'dvd', 'dvdrip', 'sd']):
            return 'SD'
            
        return '1080P'  # Default fallback
    
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
            
            success = pyload_client.add_download(url, filename=normalized_name, category="Uncategorized")
            if success:
                return jsonify({'success': True, 'normalized': normalized_name})
            else:
                return jsonify({'success': False, 'error': 'pyLoad failed'}), 500
        except Exception as e:
            logger.error(f"Download API error: {e}")
            return jsonify({'success': False, 'error': str(e)}), 500

    @web_ui_bp.route('/tutorial')
    def tutorial():
        """Render tutorial page"""
        try:
            # Use README.md as the tutorial content
            root_dir = os.path.dirname(os.path.dirname(__file__))
            readme_path = os.path.join(root_dir, 'README.md')
            
            with open(readme_path, 'r') as f:
                content = f.read()
            return render_template('tutorial.html', content=content, version=app_version)
        except Exception as e:
            logger.error(f"Error loading tutorial: {e}")
            return render_template('tutorial.html', content="# Tutorial\nGuide coming soon.", version=app_version)
    
    @web_ui_bp.route('/about')
    def about():
        """Render about page with README content"""
        try:
            # Use README.md as the documentation content
            root_dir = os.path.dirname(os.path.dirname(__file__))
            readme_path = os.path.join(root_dir, 'README.md')
            
            with open(readme_path, 'r') as f:
                md_content = f.read()
            
            # Convert markdown to HTML
            import markdown
            html_content = markdown.markdown(md_content, extensions=['tables', 'fenced_code'])
            return render_template('about.html', content=html_content, version=app_version)
        except Exception as e:
            logger.error(f"Error loading about page: {e}")
            return render_template('about.html', content="<p>Documentation coming soon.</p>", version=app_version)
    
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
            container_uptime = str(int(time.time() - boot_time))
            """
            try:
                import subprocess
                from datetime import datetime
                result = subprocess.run(
                    ['docker', 'inspect', '-f', '{{.State.StartedAt}}', 'Flasharr'],
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
            """

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
                    'speed_bytes': speed_bytes,
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
                ['docker', 'logs', '--tail', '10', 'Flasharr'],
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

    @web_ui_bp.route('/api/downloads/start_all', methods=['POST'])
    def api_start_all():
        """Unpause pyLoad server - resumes all downloads"""
        success = pyload_client.unpause_server()
        return jsonify({'success': success})

    @web_ui_bp.route('/api/downloads/pause_all', methods=['POST'])
    def api_pause_all():
        """Pause pyLoad server - pauses all downloads"""
        success = pyload_client.pause_server()
        return jsonify({'success': success})

    @web_ui_bp.route('/api/downloads/stop_all', methods=['POST'])
    def api_stop_all():
        """Stop all active downloads"""
        success = pyload_client.stop_all()
        return jsonify({'success': success})
    
    return web_ui_bp
