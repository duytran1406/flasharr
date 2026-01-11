"""
Indexer Service

Implements Newznab/Torznab-compatible API for *arr suite integration.
Refactored with proper separation of concerns.
"""

import xml.etree.ElementTree as ET
import hashlib
import logging
from datetime import datetime
from typing import List, Dict, Optional, Any
from dataclasses import dataclass, field

from ..clients.timfshare import TimFshareClient, SearchResult
from ..utils.filename_parser import FilenameParser, ParsedFilename
from ..utils.formatters import format_size

logger = logging.getLogger(__name__)


@dataclass
class IndexerConfig:
    """Configuration for the indexer service."""
    title: str = "Fshare Indexer"
    description: str = "Fshare.vn integration for *arr suite"
    email: str = "support@fshare-bridge.local"
    base_url: str = "http://localhost:8484"
    max_results: int = 100
    default_results: int = 50
    
    # Video extensions to filter
    video_extensions: tuple = (
        ".mp4", ".avi", ".mov", ".mkv", ".m4v", 
        ".flv", ".mpeg", ".wav", ".ts", ".m2ts",
    )


@dataclass
class TorznabResponse:
    """Container for Torznab/Newznab XML responses."""
    xml: str
    items_count: int = 0
    
    def to_response(self) -> tuple:
        """Return tuple for Flask response."""
        return self.xml, 200, {"Content-Type": "application/xml"}


class IndexerService:
    """
    Newznab/Torznab-compatible indexer service.
    
    Provides search capabilities for *arr suite applications (Radarr, Sonarr, Prowlarr).
    
    Example:
        >>> indexer = IndexerService()
        >>> response = indexer.search("movie name 2024")
        >>> print(response.items_count)
    """
    
    def __init__(
        self,
        search_client: Optional[TimFshareClient] = None,
        parser: Optional[FilenameParser] = None,
        config: Optional[IndexerConfig] = None,
    ):
        """
        Initialize the indexer service.
        
        Args:
            search_client: Client for search operations
            parser: Filename parser for normalization
            config: Indexer configuration
        """
        self.client = search_client or TimFshareClient()
        self.parser = parser or FilenameParser()
        self.config = config or IndexerConfig()
        
        # Cache for URL lookups (GUID -> URL mapping)
        self._url_cache: Dict[str, str] = {}
    
    def get_capabilities(self) -> TorznabResponse:
        """
        Build capabilities XML response.
        
        Returns:
            TorznabResponse with capabilities XML
        """
        root = ET.Element("caps")
        
        # Server info
        ET.SubElement(root, "server", {
            "version": "1.0",
            "title": self.config.title,
            "strapline": self.config.description,
            "email": self.config.email,
            "url": self.config.base_url,
            "image": "https://www.fshare.vn/images/logo.png",
        })
        
        # Limits
        ET.SubElement(root, "limits", {
            "max": str(self.config.max_results),
            "default": str(self.config.default_results),
        })
        
        # Registration
        ET.SubElement(root, "registration", {
            "available": "no",
            "open": "no",
        })
        
        # Searching capabilities
        searching = ET.SubElement(root, "searching")
        ET.SubElement(searching, "search", {
            "available": "yes",
            "supportedParams": "q",
        })
        ET.SubElement(searching, "tv-search", {
            "available": "yes",
            "supportedParams": "q,season,ep",
        })
        ET.SubElement(searching, "movie-search", {
            "available": "yes",
            "supportedParams": "q",
        })
        
        # Categories
        categories = ET.SubElement(root, "categories")
        
        # TV category
        tv_cat = ET.SubElement(categories, "category", {"id": "5000", "name": "TV"})
        ET.SubElement(tv_cat, "subcat", {"id": "5030", "name": "TV/HD"})
        ET.SubElement(tv_cat, "subcat", {"id": "5040", "name": "TV/SD"})
        
        # Movies category
        movie_cat = ET.SubElement(categories, "category", {"id": "2000", "name": "Movies"})
        ET.SubElement(movie_cat, "subcat", {"id": "2040", "name": "Movies/HD"})
        ET.SubElement(movie_cat, "subcat", {"id": "2030", "name": "Movies/SD"})
        
        xml = ET.tostring(root, encoding="unicode")
        return TorznabResponse(xml=xml)
    
    def search(
        self,
        query: str,
        season: Optional[str] = None,
        episode: Optional[str] = None,
        limit: Optional[int] = None,
    ) -> TorznabResponse:
        """
        Perform search and return Torznab XML response.
        
        Args:
            query: Search query
            season: Optional season number
            episode: Optional episode number
            limit: Maximum results to return
            
        Returns:
            TorznabResponse with search results XML
        """
        if not query:
            return self._build_empty_response()
        
        # Build search query with S/E if provided
        search_query = self._build_search_query(query, season, episode)
        logger.info(f"Searching for: {search_query}")
        
        # Execute search
        results = self.client.search(
            search_query,
            limit=limit or self.config.default_results,
            extensions=self.config.video_extensions,
        )
        
        # Filter by season/episode if specified
        if season or episode:
            results = self._filter_by_episode(results, season, episode)
        
        logger.info(f"Returning {len(results)} results")
        
        # Build XML response
        return self._build_search_response(results)
    
    def get_nzb(self, guid: str) -> Optional[str]:
        """
        Generate NZB content for a GUID.
        
        Args:
            guid: Result GUID
            
        Returns:
            NZB XML content or None if not found
        """
        url = self._url_cache.get(guid)
        
        nzb_content = f'''<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE nzb PUBLIC "-//newzBin//DTD NZB 1.1//EN" "http://www.newzbin.com/DTD/nzb/nzb-1.1.dtd">
<nzb xmlns="http://www.newzbin.com/DTD/2003/nzb">
  <head>
    <meta type="fshare_url">{url or "unknown"}</meta>
  </head>
  <file poster="fshare-bridge" date="{int(datetime.now().timestamp())}" subject="Fshare Download">
    <groups>
      <group>alt.binaries.fshare</group>
    </groups>
    <segments>
      <segment bytes="0" number="1">fshare-{guid}</segment>
    </segments>
  </file>
</nzb>'''
        
        return nzb_content
    
    def _build_search_query(
        self,
        query: str,
        season: Optional[str],
        episode: Optional[str],
    ) -> str:
        """Build search query string with S/E suffix."""
        if season and episode:
            return f"{query} S{int(season):02d}E{int(episode):02d}"
        elif season:
            return f"{query} S{int(season):02d}"
        return query
    
    def _filter_by_episode(
        self,
        results: List[SearchResult],
        season: Optional[str],
        episode: Optional[str],
    ) -> List[SearchResult]:
        """Filter results by season/episode."""
        filtered = []
        
        for result in results:
            parsed = self.parser.parse(result.name)
            
            if season and parsed.season != int(season):
                continue
            if episode and parsed.episode != int(episode):
                continue
            
            filtered.append(result)
        
        return filtered
    
    def _build_search_response(self, results: List[SearchResult]) -> TorznabResponse:
        """Build Torznab search results XML."""
        root = ET.Element("rss", {
            "version": "2.0",
            "xmlns:atom": "http://www.w3.org/2005/Atom",
            "xmlns:newznab": "http://www.newznab.com/DTD/2010/feeds/attributes/",
        })
        channel = ET.SubElement(root, "channel")
        
        # Channel info
        ET.SubElement(channel, "title").text = self.config.title
        ET.SubElement(channel, "description").text = self.config.description
        ET.SubElement(channel, "link").text = self.config.base_url
        ET.SubElement(channel, "language").text = "en-us"
        
        # Add items
        for result in results:
            self._add_item(channel, result)
        
        xml = '<?xml version="1.0" encoding="UTF-8"?>\n' + ET.tostring(root, encoding="unicode")
        return TorznabResponse(xml=xml, items_count=len(results))
    
    def _build_empty_response(self) -> TorznabResponse:
        """Build empty search results response."""
        return self._build_search_response([])
    
    def _add_item(self, channel: ET.Element, result: SearchResult) -> None:
        """Add a single result item to the channel."""
        item = ET.SubElement(channel, "item")
        
        # Parse filename for metadata
        parsed = self.parser.parse(result.name)
        title = parsed.normalized_filename
        
        # Generate GUID
        identifier = result.fcode or result.url or result.name
        guid = hashlib.md5(identifier.encode()).hexdigest()
        
        # Cache URL for NZB generation
        self._url_cache[guid] = result.url
        
        # Basic elements
        ET.SubElement(item, "title").text = title
        ET.SubElement(item, "guid", {"isPermaLink": "false"}).text = guid
        ET.SubElement(item, "link").text = result.url
        ET.SubElement(item, "comments").text = result.url
        
        # Publication date
        pub_date = datetime.utcnow().strftime("%a, %d %b %Y %H:%M:%S +0000")
        ET.SubElement(item, "pubDate").text = pub_date
        
        # Size
        ET.SubElement(item, "size").text = str(result.size)
        
        # Description
        description = f"{title} - {format_size(result.size)}"
        if result.score > 0:
            description += f" [Score: {result.score}]"
        ET.SubElement(item, "description").text = description
        
        # Enclosure
        enclosure_url = f"{self.config.base_url}/nzb/{guid}"
        ET.SubElement(item, "enclosure", {
            "url": enclosure_url,
            "length": str(result.size),
            "type": "application/x-nzb",
        })
        
        # Newznab attributes
        category = "5000" if parsed.is_series else "2000"
        ET.SubElement(item, "newznab:attr", {"name": "category", "value": category})
        ET.SubElement(item, "newznab:attr", {"name": "size", "value": str(result.size)})
        
        if parsed.season is not None:
            ET.SubElement(item, "newznab:attr", {"name": "season", "value": str(parsed.season)})
        if parsed.episode is not None:
            ET.SubElement(item, "newznab:attr", {"name": "episode", "value": str(parsed.episode)})
        if parsed.year:
            ET.SubElement(item, "newznab:attr", {"name": "year", "value": str(parsed.year)})
