"""
Smart Search Service

Engine for generating intelligent search queries from TMDB metadata 
and orchestrating batch searches on Fshare.

This is the SMART search used from detail pages - applies strict validation.
For generic/manual search, use IndexerService directly (no filtering).
"""

import logging
import re
from typing import List, Dict, Optional, Any

from ..services.indexer import IndexerService
from ..services.tmdb import tmdb_client
from ..factory import create_indexer_service
from ..utils.normalizer import normalize_filename

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
        Returns a list of validated Fshare file results.
        """
        queries = self.generate_queries(title, season, episode)
        all_results = []
        seen_urls = set()
        
        extensions = self.indexer.config.video_extensions if self.indexer.config else ('.mkv', '.mp4')
        
        # Execute all queries to maximize coverage
        for q in queries:
            logger.info(f"Smart Search: '{q}'")
            try:
                raw_results = self.indexer.client.search(q, extensions=extensions)
                
                for r in raw_results:
                    if r.url not in seen_urls:
                        item = r.to_dict()
                        item['size_bytes'] = r.size
                        all_results.append(item)
                        seen_urls.add(r.url)
            except Exception as e:
                logger.error(f"Smart Search error for query '{q}': {e}")
        
        # Validate and filter results
        return self._filter_episode_results(all_results, title, season, episode)
    
    def search_movie(self, title: str, year: str = None) -> List[Dict]:
        """
        Search for a movie using smart queries.
        Returns a list of validated Fshare file results.
        """
        queries = self.generate_queries(title, year=year)
        all_results = []
        seen_urls = set()
        
        extensions = self.indexer.config.video_extensions if self.indexer.config else ('.mkv', '.mp4')
        
        for q in queries:
            logger.info(f"Smart Search Movie: '{q}'")
            try:
                raw_results = self.indexer.client.search(q, extensions=extensions)
                
                for r in raw_results:
                    if r.url not in seen_urls:
                        item = r.to_dict()
                        item['size_bytes'] = r.size
                        all_results.append(item)
                        seen_urls.add(r.url)
            except Exception as e:
                logger.error(f"Smart Search error for query '{q}': {e}")
        
        # Sort by score then size
        all_results.sort(key=lambda x: (x.get('score', 0), x.get('size_bytes', 0)), reverse=True)
        return all_results
    
    def _filter_episode_results(self, results: List[Dict], title: str, season: int, episode: int) -> List[Dict]:
        """
        Filter results with strict season/episode validation.
        Rejects files that contain wrong S/E markers.
        """
        se_pattern = re.compile(r'S(\d{1,2})E(\d{1,3})', re.IGNORECASE)
        filtered = []
        
        for r in results:
            name = r['name']
            
            # Check for S/E pattern in filename
            match = se_pattern.search(name)
            if match:
                file_season = int(match.group(1))
                file_episode = int(match.group(2))
                
                # Strict validation: must match requested S/E
                if file_season != season or file_episode != episode:
                    logger.debug(f"Rejected S{file_season:02d}E{file_episode:02d} (wanted S{season:02d}E{episode:02d}): {name[:50]}")
                    continue
            
            # Also check for season packs (S01 without E)
            season_pack_pattern = re.compile(rf'S{season:02d}(?!E)', re.IGNORECASE)
            if season_pack_pattern.search(name):
                # Season pack is acceptable
                r['is_season_pack'] = True
            
            filtered.append(r)
        
        # Sort by score then size
        filtered.sort(key=lambda x: (x.get('score', 0), x.get('size_bytes', 0)), reverse=True)
        logger.info(f"Episode validation: {len(results)} -> {len(filtered)} results")
        return filtered

_smart_search_service = None

def get_smart_search_service():
    global _smart_search_service
    if not _smart_search_service:
        _smart_search_service = SmartSearchService()
    return _smart_search_service
