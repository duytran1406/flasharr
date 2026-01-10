"""
TimFshare API Client
Handles search operations using TimFshare.com API
"""

import requests
import logging
from typing import List, Dict, Optional

logger = logging.getLogger(__name__)


class TimFshareClient:
    """Client for interacting with TimFshare.com search API"""
    
    API_BASE = "https://timfshare.com/api/v1"
    
    def __init__(self):
        self.session = requests.Session()
        self.session.headers.update({
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
        })
    
    def autocomplete(self, query: str) -> List[str]:
        """
        Get autocomplete suggestions
        
        Args:
            query: Search query
            
        Returns:
            List of suggested titles
        """
        try:
            logger.info(f"Getting autocomplete suggestions for: {query}")
            
            response = self.session.get(
                f"{self.API_BASE}/autocomplete",
                params={'query': query},
                timeout=10
            )
            
            if response.status_code == 200:
                data = response.json()
                suggestions = [item['value'] for item in data.get('data', [])]
                logger.info(f"✅ Found {len(suggestions)} suggestions")
                return suggestions
            else:
                logger.error(f"Autocomplete request failed: {response.status_code}")
                return []
                
        except Exception as e:
            logger.error(f"Autocomplete error: {e}")
            return []
    
    def search(self, query: str, limit: int = 50) -> List[Dict]:
        """
        Search for files on TimFshare
        
        Args:
            query: Search query
            limit: Maximum number of results
            
        Returns:
            List of file dictionaries with keys: name, url, size, fcode
        """
        try:
            logger.info(f"Searching TimFshare for: {query}")
            
            # First get autocomplete suggestions
            suggestions = self.autocomplete(query)
            
            if not suggestions:
                logger.warning("No suggestions found")
                return []
            
            # For now, we'll use the autocomplete results
            # In a real implementation, you'd need to query each suggestion
            # to get the actual Fshare links
            
            results = []
            for suggestion in suggestions[:limit]:
                # Parse the suggestion to extract metadata
                # The suggestion format is typically: "Title YYYY Quality SxxExx ..."
                
                # For now, we'll create a placeholder result
                # You'll need to implement the actual link retrieval
                results.append({
                    'name': suggestion,
                    'url': f"https://www.fshare.vn/file/PLACEHOLDER",  # TODO: Get actual link
                    'size': 0,  # TODO: Get actual size
                    'fcode': 'PLACEHOLDER',  # TODO: Get actual fcode
                    'type': 0  # 0 = file
                })
            
            logger.info(f"✅ Found {len(results)} results")
            return results
            
        except Exception as e:
            logger.error(f"Search error: {e}")
            return []
    
    def get_fshare_link(self, title: str) -> Optional[str]:
        """
        Get Fshare download link for a specific title
        
        Args:
            title: Full title from autocomplete
            
        Returns:
            Fshare URL or None if not found
        """
        try:
            logger.info(f"Getting Fshare link for: {title}")
            
            # TODO: Implement the actual API call to get Fshare links
            # This might require:
            # 1. Querying the string-query-search endpoint
            # 2. Or scraping the search page
            # 3. Or using another API endpoint
            
            # For now, return None
            logger.warning("get_fshare_link not yet implemented")
            return None
            
        except Exception as e:
            logger.error(f"Error getting Fshare link: {e}")
            return None
