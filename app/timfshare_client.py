"""
TimFshare API Client
Handles search operations using TimFshare.com API with smart scoring
"""

import requests
import logging
import urllib.parse
import re
from typing import List, Dict, Optional

logger = logging.getLogger(__name__)


class TimFshareClient:
    """Client for interacting with TimFshare.com search API with smart scoring"""
    
    API_BASE = "https://timfshare.com/api/v1"
    SEARCH_API = f"{API_BASE}/string-query-search"
    AUTOCOMPLETE_API = f"{API_BASE}/autocomplete"
    
    def __init__(self):
        self.session = requests.Session()
        self.session.headers.update({
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
        })
    
    def smart_search(self, query: str, limit: int = 50, extensions: tuple = None) -> List[Dict]:
        """
        Smart search with filtering and scoring
        
        Args:
            query: Search query
            limit: Maximum number of results
            extensions: Optional tuple of allowed file extensions (e.g. ('.mp4', '.mkv'))
            
        Returns:
            List of scored and ranked results
        """
        logger.info(f"Smart Search starting for: {query}")
        
        # Execute search directly with query (Removed autocomplete hijacking)
        raw_results = self._execute_search(query)
        
        if not raw_results:
            return []
            
        # Filter by extension if specified
        if extensions:
            original_count = len(raw_results)
            raw_results = [
                r for r in raw_results 
                if r.get('name', '').lower().endswith(extensions)
            ]
            logger.info(f"Filtered {original_count} -> {len(raw_results)} results by extension")
        
        if not raw_results:
            return []
        
        # Score and rank results
        scored_results = self._score_results(raw_results, query)
        
        # Sort by score and return top results
        scored_results.sort(key=lambda x: x.get('score', 0), reverse=True)
        
        logger.info(f"Returning {len(scored_results[:limit])} scored results")
        return scored_results[:limit]
    
    def _execute_search(self, query: str) -> List[Dict]:
        """Execute the actual search API call"""
        try:
            url = f"{self.SEARCH_API}?query={urllib.parse.quote(query)}"
            headers = {
                'content-length': '0',
                'origin': 'https://timfshare.com',
                'referer': f'https://timfshare.com/search?key={urllib.parse.quote(query)}',
                'user-agent': 'Mozilla/5.0'
            }
            
            response = self.session.post(url, headers=headers, timeout=15)
            
            if response.status_code == 200:
                data = response.json().get('data', [])
                logger.info(f"Search API returned {len(data)} results")
                return data
            else:
                logger.error(f"Search API failed: {response.status_code}")
                
        except Exception as e:
            logger.error(f"Search API Error: {e}")
        
        return []
    
    def _score_results(self, results: List[Dict], query: str) -> List[Dict]:
        """
        Score results based on relevance
        
        Scoring factors:
        - Keyword match count (10 points per keyword)
        - Year match (20 points)
        - Resolution quality (10 points for 1080p+)
        - Vietnamese content (extra boost)
        """
        # Normalize query and extract keywords
        q_norm = query.lower().replace('.', ' ')
        keywords = [kw for kw in q_norm.split() if len(kw) > 1]
        
        scored = []
        
        for item in results:
            name = item.get('name', '')
            name_lower = name.lower()
            
            # Base score from keyword matching
            match_count = sum(1 for kw in keywords if kw in name_lower)
            score = match_count * 10
            
            # Year matching bonus
            year_match = re.search(r'\b(19[5-9][0-9]|20[0-2][0-9])\b', name_lower)
            if year_match and year_match.group(1) in q_norm:
                score += 20
            
            # Quality bonus
            if '1080p' in name_lower or '2160p' in name_lower or '4k' in name_lower:
                score += 10
            
            # Vietnamese content boost
            vietnamese_markers = ['vietsub', 'thuyết minh', 'thuyet minh', 'lồng tiếng', 'long tieng']
            if any(marker in name_lower for marker in vietnamese_markers):
                score += 15
            
            # Add score to item
            item['score'] = score
            scored.append(item)
        
        return scored
    
    def autocomplete(self, query: str) -> List[str]:
        """
        Get autocomplete suggestions (for UI)
        
        Args:
            query: Search query
            
        Returns:
            List of suggested titles
        """
        try:
            url = f"{self.AUTOCOMPLETE_API}?query={urllib.parse.quote(query)}"
            headers = {
                'accept': '*/*',
                'user-agent': 'Mozilla/5.0',
                'referer': f'https://timfshare.com/search?key={urllib.parse.quote(query)}'
            }
            
            response = self.session.get(url, headers=headers, timeout=10)
            
            if response.status_code == 200:
                data = response.json().get('data', [])
                suggestions = [item['value'] for item in data]
                logger.info(f"✅ Found {len(suggestions)} suggestions")
                return suggestions
            else:
                logger.error(f"Autocomplete request failed: {response.status_code}")
                return []
                
        except Exception as e:
            logger.error(f"Autocomplete error: {e}")
            return []
    
    def search(self, query: str, limit: int = 50, extensions: tuple = None) -> List[Dict]:
        """
        Main search method (uses smart_search)
        
        Args:
            query: Search query
            limit: Maximum number of results
            extensions: Optional tuple of allowed file extensions
            
        Returns:
            List of file dictionaries with keys: name, url, size, fcode, score
        """
        return self.smart_search(query, limit, extensions)
