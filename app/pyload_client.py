"""
pyLoad API Client
Handles communication with pyLoad download manager
"""

import requests
import logging
from typing import Optional, Dict, List
from requests.auth import HTTPBasicAuth

logger = logging.getLogger(__name__)


class PyLoadClient:
    """Client for interacting with pyLoad API (v0.5.0+)"""
    
    def __init__(self, host: str, port: int, username: str, password: str):
        self.base_url = f"http://{host}:{port}"
        self.username = username
        self.password = password
        self.session = requests.Session()
        self.session.auth = HTTPBasicAuth(self.username, self.password)
        self.logged_in = True
        self.progress_cache = {}  # Cache to preserve progress for stopped downloads
    
    def login(self) -> bool:
        """Test connection to pyLoad"""
        try:
            response = self.session.get(f"{self.base_url}/api/status_server", timeout=5)
            if response.status_code == 200:
                logger.info("✅ pyLoad API access verified (v0.5.0+)")
                return True
            return False
        except Exception as e:
            logger.error(f"❌ pyLoad connection test failed: {e}")
            return False
    
    def ensure_logged_in(self) -> bool:
        return True
    
    def add_download(self, url: str, filename: Optional[str] = None, package_name: Optional[str] = None) -> bool:
        """Add a download to pyLoad"""
        try:
            pkg_name = package_name or filename or "Fshare Download"
            response = self.session.post(
                f"{self.base_url}/api/add_package",
                json={
                    "name": pkg_name,
                    "links": [url]
                },
                timeout=10
            )
            return response.status_code == 200
        except Exception as e:
            logger.error(f"❌ Error adding download to pyLoad: {e}")
            return False
    
    def get_queue(self) -> List[Dict]:
        """
        Get formatted download queue with detailed info
        """
        try:
            # 1. Get running downloads for ETA/Speed
            active_downloads = {}
            try:
                active_resp = self.session.get(f"{self.base_url}/api/status_downloads", timeout=5)
                if active_resp.status_code == 200:
                    for d in active_resp.json():
                        active_downloads[d['fid']] = d
            except Exception as e:
                logger.warning(f"Failed to get active downloads: {e}")

            # 2. Get full queue data (packages and links)
            response = self.session.get(f"{self.base_url}/api/get_queue_data", timeout=5)
            if response.status_code != 200:
                return []

            formatted_queue = []
            for package in response.json():
                links = package.get('links', [])
                if not links: continue
                
                for link in links:
                    fid = link.get('fid')
                    active = active_downloads.get(fid)
                    
                    # Determine Status Text
                    status_msg = link.get('statusmsg', 'unknown').capitalize()
                    if active:
                        status_text = "Running"
                        eta = active.get('format_eta', '-')
                        speed_raw = active.get('speed', 0)
                        speed_text = self.format_speed(speed_raw)
                        info = f"{eta} @{speed_text}"
                        progress = active.get('percent', 0)
                        # Cache the progress for this file
                        self.progress_cache[fid] = progress
                    else:
                        if status_msg == "Finished":
                            status_text = "Finished"
                        elif status_msg in ["Aborted", "Failed", "Offline"]:
                            status_text = "Stop"
                        else:
                            status_text = "Queue"
                        
                        eta = "-"
                        speed_text = "-"
                        speed_raw = 0
                        info = status_msg
                        
                        if status_msg == "Finished":
                            progress = 100
                        elif fid in self.progress_cache:
                            progress = self.progress_cache[fid]
                        else:
                            progress = 0

                    formatted_queue.append({
                        "fid": fid,
                        "name": link.get('name'),
                        "status": status_text,
                        "info": info,
                        "eta": eta,
                        "speed": speed_text,
                        "speed_raw": speed_raw,
                        "size": link.get('format_size', '0 B'),
                        "size_bytes": link.get('size', 0),
                        "progress": progress
                    })
            
            return formatted_queue
        except Exception as e:
            logger.error(f"Error getting queue: {e}")
            return []
    
    def get_status(self) -> Optional[Dict]:
        """Get pyLoad server status"""
        try:
            response = self.session.get(f"{self.base_url}/api/status_server", timeout=5)
            if response.status_code == 200:
                data = response.json()
                if 'speed' in data:
                    data['speed_format'] = self.format_speed(data['speed'])
                return data
            return None
        except Exception as e:
            logger.error(f"Error getting status: {e}")
            return None

    def format_speed(self, speed_bytes: float) -> str:
        """Format speed bytes/s to human readable"""
        if not speed_bytes or speed_bytes == 0:
            return "0 B/s"
        units = ["B/s", "KB/s", "MB/s", "GB/s"]
        i = 0
        while speed_bytes >= 1024 and i < len(units) - 1:
            speed_bytes /= 1024
            i += 1
        return f"{speed_bytes:.1f} {units[i]}"
