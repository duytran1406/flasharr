"""
pyLoad API Client
Handles communication with pyLoad download manager
"""

import requests
import logging
from typing import Optional, Dict, List

logger = logging.getLogger(__name__)


class PyLoadClient:
    """Client for interacting with pyLoad API"""
    
    def __init__(self, host: str, port: int, username: str, password: str):
        self.base_url = f"http://{host}:{port}"
        self.username = username
        self.password = password
        self.session = requests.Session()
        self.logged_in = False
    
    def login(self) -> bool:
        """
        Login to pyLoad
        
        Returns:
            True if login successful, False otherwise
        """
        try:
            logger.info("Logging into pyLoad...")
            
            response = self.session.post(
                f"{self.base_url}/api/login",
                json={
                    "username": self.username,
                    "password": self.password
                },
                timeout=10
            )
            
            if response.status_code == 200:
                data = response.json()
                if data:
                    self.logged_in = True
                    logger.info("✅ pyLoad login successful")
                    return True
                else:
                    logger.error("❌ pyLoad login failed: Invalid credentials")
                    return False
            else:
                logger.error(f"❌ pyLoad login request failed: {response.status_code}")
                return False
                
        except Exception as e:
            logger.error(f"❌ pyLoad login error: {e}")
            return False
    
    def ensure_logged_in(self) -> bool:
        """Ensure we have a valid session, login if needed"""
        if not self.logged_in:
            return self.login()
        return True
    
    def add_download(self, url: str, filename: Optional[str] = None, package_name: Optional[str] = None) -> bool:
        """
        Add a download to pyLoad
        
        Args:
            url: Download URL (Fshare direct link)
            filename: Optional custom filename
            package_name: Optional package name
            
        Returns:
            True if download added successfully, False otherwise
        """
        if not self.ensure_logged_in():
            logger.error("Cannot add download: not logged in to pyLoad")
            return False
        
        try:
            logger.info(f"Adding download to pyLoad: {filename or url}")
            
            # Create package name
            pkg_name = package_name or filename or "Fshare Download"
            
            # Add package
            response = self.session.post(
                f"{self.base_url}/api/addPackage",
                json={
                    "name": pkg_name,
                    "links": [url]
                },
                timeout=10
            )
            
            if response.status_code == 200:
                data = response.json()
                if data:
                    logger.info(f"✅ Download added to pyLoad: {pkg_name}")
                    return True
                else:
                    logger.error("❌ Failed to add download to pyLoad")
                    return False
            else:
                logger.error(f"❌ pyLoad add download request failed: {response.status_code}")
                return False
                
        except Exception as e:
            logger.error(f"❌ Error adding download to pyLoad: {e}")
            return False
    
    def get_queue(self) -> List[Dict]:
        """
        Get current download queue
        
        Returns:
            List of downloads in queue
        """
        if not self.ensure_logged_in():
            logger.error("Cannot get queue: not logged in to pyLoad")
            return []
        
        try:
            response = self.session.get(
                f"{self.base_url}/api/getQueue",
                timeout=10
            )
            
            if response.status_code == 200:
                data = response.json()
                return data if data else []
            else:
                logger.error(f"Failed to get queue: {response.status_code}")
                return []
                
        except Exception as e:
            logger.error(f"Error getting queue: {e}")
            return []
    
    def get_status(self) -> Optional[Dict]:
        """
        Get pyLoad status
        
        Returns:
            Status dict or None if failed
        """
        if not self.ensure_logged_in():
            logger.error("Cannot get status: not logged in to pyLoad")
            return None
        
        try:
            response = self.session.get(
                f"{self.base_url}/api/getServerStatus",
                timeout=10
            )
            
            if response.status_code == 200:
                return response.json()
            else:
                logger.error(f"Failed to get status: {response.status_code}")
                return None
                
        except Exception as e:
            logger.error(f"Error getting status: {e}")
            return None
