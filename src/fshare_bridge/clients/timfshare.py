"""
TimFshare API Client

Handles search operations using TimFshare.com API with smart scoring.
Refactored with proper typing, dataclasses, and error handling.
"""

import requests
import logging
import urllib.parse
import re
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass, field

from ..core.exceptions import APIError, SearchError, ConnectionError as BridgeConnectionError

logger = logging.getLogger(__name__)


@dataclass
class SearchResult:
    """Represents a search result from TimFshare."""
    name: str
    url: str
    size: int
    fcode: str
    score: int = 0
    
    @classmethod
    def from_api_response(cls, item: Dict, score: int = 0) -> "SearchResult":
        """Create SearchResult from API response item."""
        url = item.get("url", "")
        fcode = url.split("/file/")[-1] if "/file/" in url else ""
        
        return cls(
            name=item.get("name", "Unknown"),
            url=url,
            size=int(item.get("size", 0)),
            fcode=fcode,
            score=score,
        )
    
    def to_dict(self) -> Dict:
        """Convert to dictionary for JSON serialization."""
        return {
            "name": self.name,
            "url": self.url,
            "size": self.size,
            "fcode": self.fcode,
            "score": self.score,
        }


@dataclass
class ScoringConfig:
    """Configuration for result scoring."""
    keyword_match_points: int = 10
    year_match_points: int = 20
    quality_bonus_points: int = 10
    vietnamese_bonus_points: int = 15
    
    quality_markers: Tuple[str, ...] = ("1080p", "2160p", "4k", "uhd")
    vietnamese_markers: Tuple[str, ...] = (
        "vietsub", "thuyết minh", "thuyet minh", 
        "lồng tiếng", "long tieng", "vie.dub", 
        "vie.sub", "phụ đề", "phu de",
        "tvp", "tmpđ"
    )


class TimFshareClient:
    """
    Client for interacting with TimFshare.com search API with smart scoring.
    
    Provides:
    - Smart search with relevance scoring
    - Autocomplete suggestions
    - Extension filtering
    
    Example:
        >>> client = TimFshareClient()
        >>> results = client.search("movie 2024", extensions=(".mkv", ".mp4"))
        >>> for result in results:
        ...     print(f"{result.name}: score={result.score}")
    """
    
    API_BASE = "https://timfshare.com/api/v1"
    SEARCH_ENDPOINT = f"{API_BASE}/string-query-search"
    AUTOCOMPLETE_ENDPOINT = f"{API_BASE}/autocomplete"
    DEFAULT_TIMEOUT = 15
    
    def __init__(
        self,
        timeout: int = DEFAULT_TIMEOUT,
        scoring_config: Optional[ScoringConfig] = None,
    ):
        """
        Initialize TimFshare client.
        
        Args:
            timeout: Request timeout in seconds
            scoring_config: Optional custom scoring configuration
        """
        self.timeout = timeout
        self.scoring = scoring_config or ScoringConfig()
        
        self.session = requests.Session()
        self.session.headers.update({
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "*/*",
        })
    
    def search(
        self,
        query: str,
        limit: int = 50,
        extensions: Optional[Tuple[str, ...]] = None,
    ) -> List[SearchResult]:
        """
        Search for files with smart scoring and filtering.
        
        Args:
            query: Search query string
            limit: Maximum number of results
            extensions: Optional tuple of allowed file extensions (e.g. ('.mp4', '.mkv'))
            
        Returns:
            List of SearchResult objects, sorted by relevance score
            
        Raises:
            SearchError: If search fails
            BridgeConnectionError: If connection fails
        """
        logger.info(f"Smart Search starting for: {query}")
        
        # Execute search
        raw_results = self._execute_search(query)
        
        if not raw_results:
            return []
        
        # Filter by extension if specified
        if extensions:
            original_count = len(raw_results)
            extensions_lower = tuple(ext.lower() for ext in extensions)
            raw_results = [
                r for r in raw_results
                if r.get("name", "").lower().endswith(extensions_lower)
            ]
            logger.info(f"Filtered {original_count} -> {len(raw_results)} results by extension")
        
        if not raw_results:
            return []
        
        # Score and rank results
        scored_results = self._score_results(raw_results, query)
        
        # Sort by score (descending) and convert to SearchResult objects
        scored_results.sort(key=lambda x: x.get("score", 0), reverse=True)
        
        results = [
            SearchResult.from_api_response(item, item.get("score", 0))
            for item in scored_results[:limit]
        ]
        
        logger.info(f"Returning {len(results)} scored results")
        return results
    
    def _execute_search(self, query: str) -> List[Dict]:
        """
        Execute the search API call.
        
        Args:
            query: Search query
            
        Returns:
            Raw list of result dictionaries from API
        """
        try:
            url = f"{self.SEARCH_ENDPOINT}?query={urllib.parse.quote(query)}"
            headers = {
                "Content-Length": "0",
                "Origin": "https://timfshare.com",
                "Referer": f"https://timfshare.com/search?key={urllib.parse.quote(query)}",
            }
            
            response = self.session.post(
                url,
                headers=headers,
                timeout=self.timeout,
            )
            
            if response.status_code != 200:
                logger.error(f"Search API failed: {response.status_code}")
                raise APIError(
                    "Search API request failed",
                    status_code=response.status_code,
                    response=response.text,
                )
            
            data = response.json().get("data", [])
            logger.info(f"Search API returned {len(data)} results")
            return data
            
        except requests.exceptions.RequestException as e:
            logger.error(f"Search API error: {e}")
            raise BridgeConnectionError(f"Failed to connect to TimFshare: {e}")
    
    def _score_results(self, results: List[Dict], query: str) -> List[Dict]:
        """
        Score results based on relevance.
        
        Scoring factors:
        - Keyword match count
        - Year match
        - Resolution quality
        - Vietnamese content markers
        
        Args:
            results: Raw search results
            query: Original search query
            
        Returns:
            Results with 'score' field added
        """
        # Normalize query and extract keywords
        q_norm = query.lower().replace(".", " ")
        keywords = [kw for kw in q_norm.split() if len(kw) > 1]
        
        scored = []
        
        for item in results:
            name = item.get("name", "")
            name_lower = name.lower()
            
            # Base score from keyword matching
            match_count = sum(1 for kw in keywords if kw in name_lower)
            score = match_count * self.scoring.keyword_match_points
            
            # Year matching bonus
            year_match = re.search(r"\b(19[5-9][0-9]|20[0-2][0-9])\b", name_lower)
            if year_match and year_match.group(1) in q_norm:
                score += self.scoring.year_match_points
            
            # Quality bonus
            if any(marker in name_lower for marker in self.scoring.quality_markers):
                score += self.scoring.quality_bonus_points
            
            # Vietnamese content boost
            if any(marker in name_lower for marker in self.scoring.vietnamese_markers):
                score += self.scoring.vietnamese_bonus_points
            
            item["score"] = score
            scored.append(item)
        
        return scored
    
    def autocomplete(self, query: str, limit: int = 10) -> List[str]:
        """
        Get autocomplete suggestions.
        
        Args:
            query: Partial search query
            limit: Maximum number of suggestions
            
        Returns:
            List of suggestion strings
        """
        try:
            url = f"{self.AUTOCOMPLETE_ENDPOINT}?query={urllib.parse.quote(query)}"
            headers = {
                "Referer": f"https://timfshare.com/search?key={urllib.parse.quote(query)}",
            }
            
            response = self.session.get(
                url,
                headers=headers,
                timeout=self.timeout,
            )
            
            if response.status_code != 200:
                logger.error(f"Autocomplete request failed: {response.status_code}")
                return []
            
            data = response.json().get("data", [])
            suggestions = [item["value"] for item in data if "value" in item]
            
            logger.info(f"✅ Found {len(suggestions)} suggestions")
            return suggestions[:limit]
            
        except requests.exceptions.RequestException as e:
            logger.error(f"Autocomplete error: {e}")
            return []
