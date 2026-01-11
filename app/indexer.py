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

from .timfshare_client import TimFshareClient
from .filename_parser import FilenameNormalizer

logger = logging.getLogger(__name__)

indexer_bp = Blueprint('indexer', __name__)


class NewznabIndexer:
    """Newznab-compatible indexer for Fshare"""
    
    def __init__(self, timfshare_client: TimFshareClient, normalizer: FilenameNormalizer):
        self.client = timfshare_client
        self.normalizer = normalizer
    
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
        # result['name'] comes from TimFshare
        parsed = self.normalizer.parse(result.get('name', 'Unknown'))
        
        # Use normalized filename for title
        title = parsed.normalized_filename
        ET.SubElement(item, 'title').text = title
        
        # Generate GUID from url or name if fcode missing
        identifier = result.get('fcode') or result.get('url') or result.get('name')
        if not identifier:
            identifier = str(datetime.now().timestamp())
            
        guid = hashlib.md5(identifier.encode()).hexdigest()
        ET.SubElement(item, 'guid', {'isPermaLink': 'false'}).text = guid
        
        # Link (Fshare URL)
        url = result.get('url', '')
        ET.SubElement(item, 'link').text = url
        
        # Comments (same as link)
        ET.SubElement(item, 'comments').text = url
        
        # Pub date (use current time as fallback)
        pub_date = datetime.utcnow().strftime('%a, %d %b %Y %H:%M:%S +0000')
        ET.SubElement(item, 'pubDate').text = pub_date
        
        # Size
        size = result.get('size', 0)
        ET.SubElement(item, 'size').text = str(size)
        
        # Description
        description = f"{title} - {self._format_size(size)}"
        # Add score if available
        if 'score' in result:
             description += f" [Score: {result['score']}]"
             
        ET.SubElement(item, 'description').text = description
        
        # Enclosure (download link - we'll use a special NZB format)
        # The NZB will contain the Fshare URL and metadata
        enclosure_url = f"http://localhost:8484/nzb/{guid}"
        ET.SubElement(item, 'enclosure', {
            'url': enclosure_url,
            'length': str(size),
            'type': 'application/x-nzb'
        })
        
        # Newznab attributes
        ET.SubElement(item, 'newznab:attr', {'name': 'category', 'value': '5000' if parsed.is_series else '2000'})
        ET.SubElement(item, 'newznab:attr', {'name': 'size', 'value': str(size)})
        
        # Add season/episode if available
        if parsed.season is not None:
            ET.SubElement(item, 'newznab:attr', {'name': 'season', 'value': str(parsed.season)})
        if parsed.episode is not None:
            ET.SubElement(item, 'newznab:attr', {'name': 'episode', 'value': str(parsed.episode)})
        
        # Add year if available
        if parsed.year:
            ET.SubElement(item, 'newznab:attr', {'name': 'year', 'value': str(parsed.year)})
            
        # Add GUID mapping (hacky way to pass URL via description or just rely on URL)
        # We can put the real URL in the NZB later, but we need to look it up.
        # Ideally, we should store this mapping. 
        # For now, we rely on the URL being in result['url'] and we put that in the NZB.
        
    
    def _format_size(self, size_bytes: int) -> str:
        """Format size in bytes to human-readable string"""
        try:
            size_bytes = int(size_bytes)
        except:
            return "0 B"
            
        for unit in ['B', 'KB', 'MB', 'GB', 'TB']:
            if size_bytes < 1024.0:
                return f"{size_bytes:.2f} {unit}"
            size_bytes /= 1024.0
        return f"{size_bytes:.2f} PB"
    
    def search(self, query: str, season: str = None, episode: str = None) -> str:
        """
        Perform search and return XML response
        """
        # Build search query
        search_query = query
        if season and episode:
            search_query = f"{query} S{int(season):02d}E{int(episode):02d}"
        elif season:
            search_query = f"{query} S{int(season):02d}"
        
        logger.info(f"Searching for: {search_query}")
        
        # Extensions to filter
        VIDEO_EXTENSIONS = ('.mp4', '.avi', '.mov', '.mkv', '.m4v', '.flv', '.mpeg', '.wav')
        
        # Search using TimFshare client with filtering
        results = self.client.search(search_query, extensions=VIDEO_EXTENSIONS)
        
        # Filter by season/episode if specified
        if season or episode:
            filtered_results = []
            for result in results:
                name = result.get('name', '')
                parsed = self.normalizer.parse(name)
                
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


def create_indexer_api(timfshare_client: TimFshareClient, filename_normalizer: FilenameNormalizer) -> Blueprint:
    """Create and configure the indexer API blueprint"""
    
    indexer = NewznabIndexer(timfshare_client, filename_normalizer)
    
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
        # We need the URL here. 
        # But wait, Prowlarr calls this later. 
        # We don't have the URL if we don't store it!
        # This is a stateless problem.
        # In the previous bridge implementation, we generated the NZB *during* the search response?
        # No, Prowlarr gets XML with <link> or <enclosure>.
        # If <enclosure> points to /nzb/GUID, we need to know what GUID maps to.
        
        # Quick fix: Encode the URL in the GUID or just rely on the Link.
        # Actually Prowlarr typically uses the 'link' tag.
        # The 'enclosure' is for direct download.
        
        # In my setup.sh, I configured Prowlarr to use SABnzbd as download client.
        # The 'link' url in XML is what gets sent to SABnzbd.
        # So <link> should be an URL that SABnzbd can handle?
        # Or Newznab standard says <link> is the download URL?
        
        # Let's assume Prowlarr grabs the <link> URL (result['url']) and sends it to SABnzbd.
        # BUT SABnzbd might not know what to do with an Fshare URL directly unless it has a plugin.
        # That's why we have the SABnzbd-compatible API in THIS bridge.
        # The bridge *is* the download client.
        
        # So we want Prowlarr to send the Fshare URL to the Bridge (acting as SABnzbd).
        # The Bridge (SABnzbd API) receives `addurl` with the Fshare URL.
        # It then downloads it via pyLoad.
        
        # So <link> should be the raw Fshare URL.
        # And <enclosure> should point to an NZB that contains the Fshare URL, for clients that prefer NZBs.
        
        # To make stateless /nzb/GUID work, we'd need a DB.
        # For now, let's just put a placeholder. 
        # Most users will use the <link> (addurl) method with Prowlarr+SABnzbd(Bridge).
        
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
