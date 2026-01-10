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
        # Use Basic Auth for all requests to pyLoad NG
        self.session.auth = HTTPBasicAuth(self.username, self.password)
        self.logged_in = True
    
    def login(self) -> bool:
        """
        Test connection to pyLoad
        """
        try:
            response = self.session.get(f"{self.base_url}/api/status_server", timeout=5)
            if response.status_code == 200:
                logger.info("✅ pyLoad API access verified (v0.5.0+)")
                return True
            else:
                logger.error(f"❌ pyLoad API access failed: {response.status_code}")
                return False
        except Exception as e:
            logger.error(f"❌ pyLoad connection test failed: {e}")
            return False
    
    def ensure_logged_in(self) -> bool:
        return True
    
    def add_download(self, url: str, filename: Optional[str] = None, package_name: Optional[str] = None) -> bool:
        """
        Add a download to pyLoad
        """
        try:
            logger.info(f"Adding download to pyLoad: {filename or url}")
            
            pkg_name = package_name or filename or "Fshare Download"
            
            # pyLoad 0.5.0+ uses snake_case and JSON body
            response = self.session.post(
                f"{self.base_url}/api/add_package",
                json={
                    "name": pkg_name,
                    "links": [url]
                },
                timeout=10
            )
            
            if response.status_code == 200:
                logger.info(f"✅ Download added to pyLoad: {pkg_name}")
                return True
            else:
                logger.error(f"❌ pyLoad add download failed: {response.status_code} - {response.text}")
                return False
                
        except Exception as e:
            logger.error(f"❌ Error adding download to pyLoad: {e}")
            return False
    
    def get_queue(self) -> List[Dict]:
        """
        Get current download queue
        """
        try:
            response = self.session.get(
                f"{self.base_url}/api/get_queue",
                timeout=10
            )
            
            if response.status_code == 200:
                return response.json() or []
            return []
        except Exception as e:
            logger.error(f"Error getting queue: {e}")
            return []
    
    def get_status(self) -> Optional[Dict]:
        """
        Get pyLoad status
        """
        try:
            response = self.session.get(
                f"{self.base_url}/api/status_server",
                timeout=10
            )
            
            if response.status_code == 200:
                data = response.json()
                # Map newer keys to what web_ui expects if necessary
                if 'speed' in data and 'speed_format' not in data:
                    data['speed_format'] = self.format_speed(data['speed'])
                return data
            return None
        except Exception as e:
            logger.error(f"Error getting status: {e}")
            return None

    def format_speed(self, speed_bytes: float) -> str:
        """Format speed in bytes/s to human readable string"""
        if speed_bytes == 0:
            return "0 B/s"
        units = ["B/s", "KB/s", "MB/s", "GB/s"]
        i = 0
        while speed_bytes >= 1024 and i < len(units) - 1:
            speed_bytes /= 1024
            i += 1
        return f"{speed_bytes:.1f} {units[i]}"
