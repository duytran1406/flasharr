"""
PyLoad API Client

Client for interacting with the PyLoad API.
"""

import logging
import requests
from typing import Dict, Any, List, Optional
from urllib.parse import urljoin

logger = logging.getLogger(__name__)


class PyLoadClient:
    """Client for PyLoad API."""
    
    def __init__(self, base_url: str, username: str = "pyload", password: str = "pyload"):
        self.base_url = base_url.rstrip("/")
        self.username = username
        self.password = password
        self.session = requests.Session()
        self._sid = None
    
    def login(self) -> bool:
        """Login to PyLoad."""
        try:
            resp = self.session.post(
                f"{self.base_url}/api/login",
                data={"username": self.username, "password": self.password}
            )
            data = resp.json()
            if isinstance(data, str): # quoted session id
                self._sid = data.strip('"')
            elif isinstance(data, bool) and data:
                # Cookie based?
                pass
            
            # Verify login
            if "pyload_session" in self.session.cookies or self._sid:
                 logger.info("PyLoad login successful")
                 return True
                 
            logger.error(f"PyLoad login failed: {resp.text}")
            return False
        except Exception as e:
            logger.error(f"PyLoad login error: {e}")
            return False

    def get_queue(self) -> List[Dict[str, Any]]:
        """Get download queue."""
        return self._call("getQueue") or []

    def get_history(self) -> List[Dict[str, Any]]:
        """Get processed packages/files (History usually separate or part of queue query)."""
        # PyLoad split queue/collector/history
        # getCollector, getQueue
        # history might need specific call or manual processing
        return [] # Placeholder

    def get_status(self) -> Dict[str, Any]:
        """Get server status."""
        return self._call("statusServer") or {}

    def start_all(self):
        return self._call("unpauseServer")

    def pause_all(self):
        return self._call("pauseServer")
        
    def stop_all(self):
        return self._call("stopAllDownloads") # Might not exist

    def add_package(self, name: str, links: List[str], dest: Optional[str] = None):
        """Add package."""
        return self._call("addPackage", name, links, dest)
        
    def restart_file(self, file_id: int):
        return self._call("restartFile", file_id)

    def stop_file(self, file_id: int):
        return self._call("stopFile", file_id)
        
    def delete_files(self, file_ids: List[int]):
         return self._call("deleteFiles", file_ids)

    def _call(self, method: str, *args):
        """Call API method."""
        try:
            # PyLoad API usually accepts form data or args
            # POST /api/methodName params
            url = f"{self.base_url}/api/{method}"
            resp = self.session.post(url, data=dict(enumerate(args))) 
            # Note: PyLoad API argument passing can be tricky.
            # Using simple query params for simple args often works, or positional data props.
            
            # Try JSON body with args list if this is pyload-ng
            # But earlier curl attempts suggested form-like expectations.
            
            if resp.status_code == 200:
                try:
                    return resp.json()
                except:
                    return resp.text
            return None
        except Exception as e:
            logger.error(f"PyLoad API call {method} failed: {e}")
            return None
