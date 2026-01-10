"""
Fshare API Client
Handles authentication and file operations with Fshare.vn
"""

import requests
import logging
from typing import Optional, List, Dict
from datetime import datetime, timedelta

logger = logging.getLogger(__name__)


class FshareClient:
    """Client for interacting with Fshare.vn API"""
    
    API_BASE = "https://www.fshare.vn/api"
    APP_KEY = "L2S7R6ZMagggC5wWkQhX2+aDi467PPuftWUMoxn"
    
    def __init__(self, email: str, password: str):
        self.email = email
        self.password = password
        self.session = requests.Session()
        self.session.headers.update({
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
        })
        self.token = None
        self.session_id = None
        self.token_expires = None
    
    def login(self) -> bool:
        """
        Login to Fshare and obtain session token
        
        Returns:
            True if login successful, False otherwise
        """
        try:
            logger.info("Logging into Fshare...")
            
            response = self.session.post(
                f"{self.API_BASE}/user/login",
                json={
                    "user_email": self.email,
                    "password": self.password,
                    "app_key": self.APP_KEY
                },
                timeout=15
            )
            
            if response.status_code == 200:
                data = response.json()
                if data.get("code") == 200:
                    self.token = data.get("token")
                    self.session_id = data.get("session_id")
                    
                    # Set session cookie
                    self.session.headers.update({
                        'Cookie': f'session_id={self.session_id}'
                    })
                    
                    # Token expires in 24 hours
                    self.token_expires = datetime.now() + timedelta(hours=24)
                    
                    logger.info("✅ Fshare login successful")
                    return True
                else:
                    logger.error(f"❌ Fshare login failed: {data.get('msg', 'Unknown error')}")
                    return False
            else:
                logger.error(f"❌ Fshare login request failed: {response.status_code}")
                return False
                
        except Exception as e:
            logger.error(f"❌ Fshare login error: {e}")
            return False
    
    def ensure_logged_in(self) -> bool:
        """Ensure we have a valid session, login if needed"""
        if not self.token or not self.token_expires:
            return self.login()
        
        if datetime.now() >= self.token_expires:
            logger.info("Token expired, re-logging in...")
            return self.login()
        
        return True
    
    def search(self, query: str, limit: int = 50) -> List[Dict]:
        """
        Search for files on Fshare
        
        Args:
            query: Search query
            limit: Maximum number of results
            
        Returns:
            List of file dictionaries with keys: name, url, size, fcode
        """
        if not self.ensure_logged_in():
            logger.error("Cannot search: not logged in")
            return []
        
        try:
            logger.info(f"Searching Fshare for: {query}")
            
            # Fshare search API endpoint
            response = self.session.post(
                f"{self.API_BASE}/fileops/search",
                json={
                    "search": query,
                    "limit": limit,
                    "token": self.token
                },
                timeout=15
            )
            
            if response.status_code == 200:
                data = response.json()
                if data.get("code") == 200:
                    items = data.get("items", [])
                    
                    results = []
                    for item in items:
                        results.append({
                            'name': item.get('name', 'Unknown'),
                            'url': f"https://www.fshare.vn/file/{item.get('linkcode', '')}",
                            'size': item.get('size', 0),
                            'fcode': item.get('linkcode', ''),
                            'type': item.get('type', 0)  # 0 = file, 1 = folder
                        })
                    
                    logger.info(f"✅ Found {len(results)} results")
                    return results
                else:
                    logger.warning(f"Search returned error: {data.get('msg', 'Unknown')}")
                    return []
            else:
                logger.error(f"Search request failed: {response.status_code}")
                return []
                
        except Exception as e:
            logger.error(f"Search error: {e}")
            return []
    
    def get_download_link(self, fcode: str) -> Optional[str]:
        """
        Get direct download link for a file
        
        Args:
            fcode: Fshare file code
            
        Returns:
            Direct download URL or None if failed
        """
        if not self.ensure_logged_in():
            logger.error("Cannot get download link: not logged in")
            return None
        
        try:
            logger.info(f"Getting download link for: {fcode}")
            
            response = self.session.post(
                f"{self.API_BASE}/session/download",
                json={
                    "url": f"https://www.fshare.vn/file/{fcode}",
                    "token": self.token,
                    "password": ""
                },
                timeout=15
            )
            
            if response.status_code == 200:
                data = response.json()
                if data.get("code") == 200:
                    download_url = data.get("location")
                    logger.info(f"✅ Got download link")
                    return download_url
                else:
                    logger.error(f"Failed to get download link: {data.get('msg', 'Unknown')}")
                    return None
            else:
                logger.error(f"Download link request failed: {response.status_code}")
                return None
                
        except Exception as e:
            logger.error(f"Error getting download link: {e}")
            return None
    
    def get_file_info(self, url: str) -> Optional[Dict]:
        """
        Get file information from Fshare URL
        
        Args:
            url: Fshare file URL
            
        Returns:
            Dict with file info or None if failed
        """
        if not self.ensure_logged_in():
            logger.error("Cannot get file info: not logged in")
            return None
        
        try:
            # Extract fcode from URL
            fcode = url.split('/file/')[-1].split('?')[0]
            
            response = self.session.post(
                f"{self.API_BASE}/fileops/get",
                json={
                    "url": url,
                    "token": self.token
                },
                timeout=15
            )
            
            if response.status_code == 200:
                data = response.json()
                if data.get("code") == 200:
                    item = data.get("item", {})
                    return {
                        'name': item.get('name', 'Unknown'),
                        'size': item.get('size', 0),
                        'fcode': fcode,
                        'url': url
                    }
                else:
                    logger.error(f"Failed to get file info: {data.get('msg', 'Unknown')}")
                    return None
            else:
                logger.error(f"File info request failed: {response.status_code}")
                return None
                
        except Exception as e:
            logger.error(f"Error getting file info: {e}")
            return None
