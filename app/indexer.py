"""
Prowlarr Indexer API
Implements Newznab/Torznab-compatible API for Prowlarr integration
"""

from flask import Blueprint, request, Response
import xml.etree.ElementTree as ET
from datetime import datetime
import logging
from typing import List, Dict
import hashlib

from .fshare_client import FshareClient
from .filename_parser import FilenameNormalizer

logger = logging.getLogger(__name__)

indexer_bp = Blueprint('indexer', __name__)


class NewznabIndexer:
    """Newznab-compatible indexer for Fshare"""
    
    def __init__(self, fshare_client: FshareClient):
        self.fshare = fshare_client
        self.normalizer = FilenameNormalizer()
    
    def build_caps_response(self) -> str:
        """Build capabilities XML response"""
        root = ET.Element('caps')
        
        # Server info
        server = ET.SubElement(root, 'server', {
            'version': '1.0',
            'title': 'Fshare Indexer',
            'strapline': 'Fshare.vn integration for *arr suite',
            'email': 'support@fshare-bridge.local',
            'url': 'http://localhost:8484',
            'image': 'https://www.fshare.vn/images/logo.png'
        })
        
        # Limits
        limits = ET.SubElement(root, 'limits', {
            'max': '100',
            'default': '50'
        })
        
        # Registration
        registration = ET.SubElement(root, 'registration', {
            'available': 'no',
            'open': 'no'
        })
        
        # Searching
        searching = ET.SubElement(root, 'searching')
        
        # Search types
        search = ET.SubElement(searching, 'search', {
            'available': 'yes',
            'supportedParams': 'q'
        })
        
        tv_search = ET.SubElement(searching, 'tv-search', {
            'available': 'yes',
            'supportedParams': 'q,season,ep'
        })
        
        movie_search = ET.SubElement(searching, 'movie-search', {
            'available': 'yes',
            'supportedParams': 'q'
        })
        
        # Categories
        categories = ET.SubElement(root, 'categories')
        
        # TV category
        tv_cat = ET.SubElement(categories, 'category', {
            'id': '5000',
            'name': 'TV'
        })
        ET.SubElement(tv_cat, 'subcat', {'id': '5030', 'name': 'TV/HD'})
        ET.SubElement(tv_cat, 'subcat', {'id': '5040', 'name': 'TV/SD'})
        
        # Movies category
        movie_cat = ET.SubElement(categories, 'category', {
            'id': '2000',
            'name': 'Movies'
        })
        ET.SubElement(movie_cat, 'subcat', {'id': '2040', 'name': 'Movies/HD'})
        ET.SubElement(movie_cat, 'subcat', {'id': '2030', 'name': 'Movies/SD'})
        
        return ET.tostring(root, encoding='unicode')
    
    def build_search_response(self, results: List[Dict]) -> str:
        """
        Build search results XML response
        
        Args:
            results: List of search results from Fshare
            
        Returns:
            XML string in Newznab format
        """
        root = ET.Element('rss', {'version': '2.0', 'xmlns:atom': 'http://www.w3.org/2005/Atom'})
        channel = ET.SubElement(root, 'channel')
        
        # Channel info
        ET.SubElement(channel, 'title').text = 'Fshare Indexer'
        ET.SubElement(channel, 'description').text = 'Fshare.vn search results'
        ET.SubElement(channel, 'link').text = 'http://localhost:8484'
        ET.SubElement(channel, 'language').text = 'en-us'
        
        # Add items
        for result in results:
            self._add_item(channel, result)
        
        return '<?xml version="1.0" encoding="UTF-8"?>\n' + ET.tostring(root, encoding='unicode')
    
    def _add_item(self, channel: ET.Element, result: Dict):
        """Add a single result item to the channel"""
        item = ET.SubElement(channel, 'item')
        
        # Parse filename for better metadata
        parsed = self.normalizer.parse(result['name'])
        
        # Use normalized filename for title
        title = parsed.normalized_filename
        ET.SubElement(item, 'title').text = title
        
        # Generate GUID from fcode
        guid = hashlib.md5(result['fcode'].encode()).hexdigest()
        ET.SubElement(item, 'guid', {'isPermaLink': 'false'}).text = guid
        
        # Link (Fshare URL)
        ET.SubElement(item, 'link').text = result['url']
        
        # Comments (same as link)
        ET.SubElement(item, 'comments').text = result['url']
        
        # Pub date (use current time)
        pub_date = datetime.utcnow().strftime('%a, %d %b %Y %H:%M:%S +0000')
        ET.SubElement(item, 'pubDate').text = pub_date
        
        # Size
        ET.SubElement(item, 'size').text = str(result.get('size', 0))
        
        # Description
        description = f"{title} - {self._format_size(result.get('size', 0))}"
        ET.SubElement(item, 'description').text = description
        
        # Enclosure (download link - we'll use a special NZB format)
        # The NZB will contain the Fshare URL and metadata
        enclosure_url = f"http://localhost:8484/nzb/{guid}"
        ET.SubElement(item, 'enclosure', {
            'url': enclosure_url,
            'length': str(result.get('size', 0)),
            'type': 'application/x-nzb'
        })
        
        # Newznab attributes
        attrs = ET.SubElement(item, 'newznab:attr')
        
        # Category (determine from parsed data)
        if parsed.is_series:
            category_id = '5000'  # TV
        else:
            category_id = '2000'  # Movies
        
        ET.SubElement(item, 'newznab:attr', {'name': 'category', 'value': category_id})
        ET.SubElement(item, 'newznab:attr', {'name': 'size', 'value': str(result.get('size', 0))})
        
        # Add season/episode if available
        if parsed.season is not None:
            ET.SubElement(item, 'newznab:attr', {'name': 'season', 'value': str(parsed.season)})
        if parsed.episode is not None:
            ET.SubElement(item, 'newznab:attr', {'name': 'episode', 'value': str(parsed.episode)})
        
        # Add year if available
        if parsed.year:
            ET.SubElement(item, 'newznab:attr', {'name': 'year', 'value': str(parsed.year)})
    
    def _format_size(self, size_bytes: int) -> str:
        """Format size in bytes to human-readable string"""
        for unit in ['B', 'KB', 'MB', 'GB', 'TB']:
            if size_bytes < 1024.0:
                return f"{size_bytes:.2f} {unit}"
            size_bytes /= 1024.0
        return f"{size_bytes:.2f} PB"
    
    def search(self, query: str, season: str = None, episode: str = None) -> str:
        """
        Perform search and return XML response
        
        Args:
            query: Search query
            season: Optional season number
            episode: Optional episode number
            
        Returns:
            XML response string
        """
        # Build search query
        search_query = query
        if season and episode:
            search_query = f"{query} S{int(season):02d}E{int(episode):02d}"
        elif season:
            search_query = f"{query} S{int(season):02d}"
        
        logger.info(f"Searching Fshare for: {search_query}")
        
        # Search Fshare
        results = self.fshare.search(search_query)
        
        # Filter results if season/episode specified
        if season or episode:
            filtered_results = []
            for result in results:
                parsed = self.normalizer.parse(result['name'])
                
                # Check if season/episode match
                if season and parsed.season != int(season):
                    continue
                if episode and parsed.episode != int(episode):
                    continue
                
                filtered_results.append(result)
            
            results = filtered_results
        
        logger.info(f"Returning {len(results)} results")
        
        # Build and return XML response
        return self.build_search_response(results)


def create_indexer_api(fshare_client: FshareClient) -> Blueprint:
    """Create and configure the indexer API blueprint"""
    
    indexer = NewznabIndexer(fshare_client)
    
    @indexer_bp.route('/api', methods=['GET'])
    def api_endpoint():
        """Main API endpoint"""
        t = request.args.get('t', '')
        
        if t == 'caps':
            # Capabilities request
            xml_response = indexer.build_caps_response()
            return Response(xml_response, mimetype='application/xml')
        
        elif t == 'search' or t == 'tvsearch' or t == 'movie':
            # Search request
            query = request.args.get('q', '')
            season = request.args.get('season')
            episode = request.args.get('ep')
            
            if not query:
                # Return empty results
                xml_response = indexer.build_search_response([])
                return Response(xml_response, mimetype='application/xml')
            
            xml_response = indexer.search(query, season, episode)
            return Response(xml_response, mimetype='application/xml')
        
        else:
            # Unknown request type
            return Response(
                '<?xml version="1.0" encoding="UTF-8"?><error>Unknown request type</error>',
                mimetype='application/xml',
                status=400
            )
    
    @indexer_bp.route('/nzb/<guid>', methods=['GET'])
    def get_nzb(guid):
        """
        Generate NZB file for a result
        The NZB will contain the Fshare URL encoded in a special format
        """
        # For now, return a simple NZB structure
        # The SABnzbd API will parse this to extract the Fshare URL
        nzb_content = f'''<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE nzb PUBLIC "-//newzBin//DTD NZB 1.1//EN" "http://www.newzbin.com/DTD/nzb/nzb-1.1.dtd">
<nzb xmlns="http://www.newzbin.com/DTD/2003/nzb">
  <file poster="fshare-bridge" date="1234567890" subject="Fshare Download">
    <groups>
      <group>alt.binaries.fshare</group>
    </groups>
    <segments>
      <segment bytes="0" number="1">fshare-{guid}</segment>
    </segments>
  </file>
</nzb>'''
        
        return Response(nzb_content, mimetype='application/x-nzb')
    
    return indexer_bp
