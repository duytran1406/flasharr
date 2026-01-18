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
    size_bonus_per_gb: int = 5  # Points per GB (larger = better quality assumption)
    title_mismatch_penalty: int = 50  # Penalty for low title similarity
    similarity_threshold: float = 0.4  # Minimum title similarity to keep result (0-1)
    
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
        Score results based on relevance with title similarity filtering.
        
        Scoring factors:
        - Title similarity (fuzzy match)
        - Season/Episode match validation
        - Keyword match count
        - Year match
        - Resolution quality
        - Vietnamese content markers
        - File size bonus
        
        Args:
            results: Raw search results
            query: Original search query
            
        Returns:
            Results with 'score' and 'similarity' fields added
        """
        # Extract core title from query (remove year, quality markers)
        q_clean = self._extract_title_from_query(query)
        q_norm = query.lower().replace(".", " ")
        keywords = [kw for kw in q_norm.split() if len(kw) > 1]
        
        # Extract season from query if present
        query_season = self._extract_season_from_query(query)
        
        scored = []
        
        for item in results:
            name = item.get("name", "")
            name_lower = name.lower()
            
            # Calculate title similarity
            similarity = self._calculate_similarity(q_clean, name)
            item["similarity"] = similarity
            
            # Skip results below similarity threshold
            if similarity < self.scoring.similarity_threshold:
                logger.debug(f"Rejected (low similarity {similarity:.2f}): {name[:50]}")
                continue
            
            # Season validation: if query has a season, result must match
            if query_season is not None:
                result_season = self._extract_season_from_filename(name)
                if result_season is not None and result_season != query_season:
                    logger.debug(f"Rejected (wrong season S{result_season:02d} != S{query_season:02d}): {name[:50]}")
                    continue
            
            # Base score from keyword matching
            match_count = sum(1 for kw in keywords if kw in name_lower)
            score = match_count * self.scoring.keyword_match_points
            
            # Similarity bonus (0-50 points based on similarity)
            score += int(similarity * 50)
            
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
            
            # Size bonus (larger files assumed better quality)
            size_gb = item.get("size", 0) / (1024 ** 3)
            score += int(min(size_gb, 10) * self.scoring.size_bonus_per_gb)
            
            item["score"] = score
            scored.append(item)
        
        logger.info(f"Validation filtered {len(results)} -> {len(scored)} results")
        return scored
    
    def _extract_title_from_query(self, query: str) -> str:
        """Extract clean title from query for similarity matching."""
        # Remove common patterns
        clean = query.lower()
        clean = re.sub(r"\s*s\d{1,2}e\d{1,3}", "", clean)  # S01E01
        clean = re.sub(r"\s*season\s*\d+", "", clean)  # Season X
        clean = re.sub(r"\b(19|20)\d{2}\b", "", clean)  # Year
        clean = re.sub(r"(1080p|720p|2160p|4k|uhd)", "", clean)  # Quality
        clean = re.sub(r"[^\w\s]", " ", clean)  # Special chars
        clean = re.sub(r"\s+", " ", clean).strip()
        return clean
    
    def _calculate_similarity(self, query_title: str, filename: str) -> float:
        """
        Calculate title similarity using word tokenization.
        
        Normalizes query title into significant words (removes stop words),
        then checks what percentage exist in the filename.
        Handles apostrophe variants: Grey's → checks 'greys' or 'grey'
        
        Returns 0-1 where 1 means all title words found in filename.
        """
        # Stop words to ignore (common in titles but may not be in filenames)
        stop_words = {'the', 'a', 'an', 'and', 'of', 'in', 'on', 'at', 'to', 'for', 'is', 'it'}
        
        # Tokenize query title - generate word variants
        query_clean = query_title.lower()
        query_clean = re.sub(r"[:\-–—]", " ", query_clean)  # Colons/dashes to spaces
        raw_words = query_clean.split()
        
        # Generate variants for each word (handle apostrophes)
        query_word_variants = []
        for word in raw_words:
            word = re.sub(r"[^\w']", "", word)  # Keep only letters and apostrophe
            if word in stop_words or len(word) < 2:
                continue
            
            variants = set()
            variants.add(word.replace("'", ""))  # Grey's → Greys
            if "'" in word:
                variants.add(word.split("'")[0])  # Grey's → Grey
            
            # 1. Basic normalization (ā -> a)
            import unicodedata
            normalized = unicodedata.normalize('NFKD', word).encode('ascii', 'ignore').decode()
            variants.add(normalized.replace("'", ""))
            
            # 2. Phonetic expansion (ā -> aa, ö -> oe)
            phonetic_map = {
                'ā': 'aa', 'ē': 'ee', 'ī': 'ii', 'ō': 'oo', 'ū': 'uu',
                'ö': 'oe', 'ä': 'ae', 'ü': 'ue', 'ß': 'ss'
            }
            phonetic = word
            has_special = False
            for char, repl in phonetic_map.items():
                if char in word:
                    phonetic = phonetic.replace(char, repl)
                    has_special = True
            
            if has_special:
                # Add normalized version of phonetic variant
                phonetic_norm = unicodedata.normalize('NFKD', phonetic).encode('ascii', 'ignore').decode()
                variants.add(phonetic_norm.replace("'", ""))
            
            query_word_variants.append(variants)
        
        if not query_word_variants:
            return 0.0
        
        # Normalize filename for matching
        fn_clean = filename.lower()
        fn_clean = re.sub(r"\.(mkv|mp4|avi|mov|wmv|flv|webm|ts|m2ts)$", "", fn_clean)
        fn_clean = re.sub(r"[._]", " ", fn_clean)  # Replace separators with spaces
        fn_clean = re.sub(r"[^\w\s]", " ", fn_clean)
        fn_clean = re.sub(r"\s+", " ", fn_clean).strip()
        
        # Count how many query words have at least one variant in filename
        found = 0
        for variants in query_word_variants:
            if any(v in fn_clean for v in variants):
                found += 1
        
        # Return percentage of words found
        return found / len(query_word_variants)
    
    def _extract_season_from_query(self, query: str) -> Optional[int]:
        """
        Extract season number from search query.
        Handles: S1, S01, S001, Season 1, Season 01
        """
        # Match S followed by 1-3 digits (S1, S01, S001)
        match = re.search(r'\bS(\d{1,3})\b', query, re.IGNORECASE)
        if match:
            return int(match.group(1))
        
        # Match "Season" followed by digits
        match = re.search(r'\bSeason\s*(\d{1,3})\b', query, re.IGNORECASE)
        if match:
            return int(match.group(1))
        
        return None
    
    def _extract_season_from_filename(self, filename: str) -> Optional[int]:
        """
        Extract season number from filename.
        Handles: S1E1, S01E01, S001E001, S1.E1, S1 E1, Season.1, Season 1
        """
        # Normalize underscores to spaces for consistent word boundary matching
        fn = filename.replace('_', ' ')
        
        # Match S followed by 1-3 digits, then E (with optional separator)
        # Patterns: S01E01, S1E1, S001E001, S01.E01, S01 E01
        match = re.search(r'\bS(\d{1,3})[\s.]?E\d{1,4}\b', fn, re.IGNORECASE)
        if match:
            return int(match.group(1))
        
        # Match S followed by 1-3 digits without episode
        match = re.search(r'\bS(\d{1,3})\b', fn, re.IGNORECASE)
        if match:
            return int(match.group(1))
        
        # Match "Season" followed by separator and digits (Season.1, Season 1, Season_1)
        match = re.search(r'\bSeason[\s.]?(\d{1,3})\b', fn, re.IGNORECASE)
        if match:
            return int(match.group(1))
        
        return None
    
    def _extract_episode_from_filename(self, filename: str) -> Optional[int]:
        """
        Extract episode number from filename.
        Handles: E1, E01, E001, E14, E014, Ep1, Ep.01, Episode.14
        """
        # Normalize underscores to spaces for consistent word boundary matching
        fn = filename.replace('_', ' ')
        
        # Match S##E## pattern (prioritize)
        match = re.search(r'\bS\d{1,3}[\s.]?E(\d{1,4})\b', fn, re.IGNORECASE)
        if match:
            return int(match.group(1))
        
        # Match "Episode" followed by separator and digits
        match = re.search(r'\bEpisode[\s.]?(\d{1,4})\b', fn, re.IGNORECASE)
        if match:
            return int(match.group(1))
        
        # Match standalone E followed by 1-4 digits
        match = re.search(r'\bE(\d{1,4})\b', fn, re.IGNORECASE)
        if match:
            return int(match.group(1))
        
        # Match "Ep" or "EP." followed by digits
        match = re.search(r'\bEp[\s.]?(\d{1,4})\b', fn, re.IGNORECASE)
        if match:
            return int(match.group(1))
        
        return None
    
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
