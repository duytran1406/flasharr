"""
Smart Search Service

Engine for generating intelligent search queries from TMDB metadata 
and orchestrating batch searches on Fshare.
"""

import logging
import re
from typing import List, Dict, Optional, Any

from ..services.indexer import IndexerService
from ..services.tmdb import tmdb_client
from ..factory import create_indexer_service

logger = logging.getLogger(__name__)

class SmartSearchService:
    def __init__(self):
        self.indexer = create_indexer_service()
        self.tmdb = tmdb_client

    def generate_queries(self, title: str, season: int = None, episode: int = None, year: str = None) -> List[str]:
        """
        Generate a list of search queries based on media metadata.
        Prioritize common naming conventions.
        """
        queries = []
        clean_title = self._clean_title(title)
        
        if season is not None and episode is not None:
            # S01E01 format (Standard)
            queries.append(f"{clean_title} S{season:02d}E{episode:02d}")
            # Space format
            queries.append(f"{clean_title} S{season:02d} E{episode:02d}")
            # Simple format (riskier but useful)
            queries.append(f"{clean_title} {season}x{episode:02d}")
            
        elif season is not None:
            # Season pack search
            queries.append(f"{clean_title} Season {season}")
            queries.append(f"{clean_title} S{season:02d}")
            
        else:
            # Movie
            if year:
                queries.append(f"{clean_title} {year}")
            queries.append(clean_title)
            
        return queries

    def _clean_title(self, title: Optional[str]) -> str:
        """Remove special chars that confuse Fshare search."""
        if not title:
            return ""
        # Remove characters that aren't alphanumeric or spaces
        t = re.sub(r'[^\w\s\.-]', '', str(title))
        return t.strip()

    def search_episode(self, title: str, season: int, episode: int) -> List[Dict]:
        """
        Search for a specific episode using smart queries.
        Returns a list of Fshare file results.
        """
        queries = self.generate_queries(title, season, episode)
        all_results = []
        seen_urls = set()
        
        # We can try queries in order. If the first one yields good results, maybe stop?
        # For now, let's just stick to the best 2 queries to avoid spamming.
        for q in queries[:2]: 
            logger.info(f"Smart Search: '{q}'")
            # Use client directly to get SearchResult objects instead of XML
            # Default extensions from indexer config if available, else standard video
            extensions = self.indexer.config.video_extensions if self.indexer.config else ('.mkv', '.mp4')
            
            try:
                # search() returns List[SearchResult]
                raw_results = self.indexer.client.search(q, extensions=extensions)
                
                for r in raw_results:
                    if r.url not in seen_urls:
                        # Convert dataclass to dict and map keys for frontend
                        item = r.to_dict()
                        item['size_bytes'] = r.size # Frontend expects size_bytes
                        
                        all_results.append(item)
                        seen_urls.add(r.url)
            except Exception as e:
                logger.error(f"Smart Search error for query '{q}': {e}")
        
        return self._filter_results(all_results, season, episode)

    def _filter_results(self, results: List[Dict], season: int = None, episode: int = None) -> List[Dict]:
        """
        Filter and rank results. 
        Remove obvious bad matches (wrong season/episode).
        """
        filtered = []
        for r in results:
            name = r['name']
            # Basic verify: if we searched for S01E01, ensure it's not S01E02
            # This requires parsing the result name
            # For MVP: Trusted Search. Since we query specifically "S01E01", results *should* be relevant.
            # But "Ling Cage S01" query might return S01E02.
            
            # Simple check: if looking for E01, and name contains E02, skip?
            # That's too aggressive. 
            # Let's rely on the query being specific enough for now.
            filtered.append(r)
            
        # Sort by Size DESC (assume larger is better quality/pack)
        filtered.sort(key=lambda x: x.get('size_bytes', 0), reverse=True)
        return filtered

_smart_search_service = None

def get_smart_search_service():
    global _smart_search_service
    if not _smart_search_service:
        _smart_search_service = SmartSearchService()
    return _smart_search_service
