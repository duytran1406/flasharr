"""
SABnzbd-Compatible API
Implements SABnzbd API for *arr suite download client integration
"""

from flask import Blueprint, request, jsonify
import xml.etree.ElementTree as ET
import logging
import hashlib
import uuid
from typing import Dict, List, Optional
from datetime import datetime

from .fshare_client import FshareClient
from .pyload_client import PyLoadClient
from .filename_parser import FilenameNormalizer

logger = logging.getLogger(__name__)

sabnzbd_bp = Blueprint('sabnzbd', __name__)


class SABnzbdAPI:
    """SABnzbd-compatible API for download client integration"""
    
    def __init__(self, fshare_client: FshareClient, pyload_client: PyLoadClient, normalizer: FilenameNormalizer):
        self.fshare = fshare_client
        self.pyload = pyload_client
        self.normalizer = normalizer
        
        # In-memory storage for queue and history
        self.queue = {}  # nzo_id -> download info
        self.history = {}  # nzo_id -> download info
    
    def add_file(self, nzb_data: bytes, filename: str = "download.nzb") -> Optional[str]:
        """
        Add a download from NZB file data
        """
        try:
            # Parse NZB to extract Fshare info
            root = ET.fromstring(nzb_data)
            
            # Find the segment that contains our Fshare GUID
            segment = root.find('.//{http://www.newzbin.com/DTD/2003/nzb}segment')
            if segment is None:
                logger.error("No segment found in NZB")
                return None
            
            segment_text = segment.text
            if not segment_text or not segment_text.startswith('fshare-'):
                logger.error(f"Invalid segment format: {segment_text}")
                return None
            
            # Extract GUID
            guid = segment_text.replace('fshare-', '')
            
            # Get the subject (filename) from NZB
            file_elem = root.find('.//{http://www.newzbin.com/DTD/2003/nzb}file')
            subject = file_elem.get('subject', 'Unknown') if file_elem is not None else 'Unknown'
            
            logger.info(f"Processing download request for: {subject}")
            
            # Generate NZO ID
            nzo_id = str(uuid.uuid4())
            
            # Parse filename for normalization
            parsed = self.normalizer.parse(subject)
            normalized_filename = parsed.normalized_filename
            
            logger.info(f"Normalized filename: {normalized_filename}")
            
            # Add to queue
            self.queue[nzo_id] = {
                'nzo_id': nzo_id,
                'filename': normalized_filename,
                'original_filename': subject,
                'status': 'Queued',
                'percentage': 0,
                'mb_left': 0,
                'mb_total': 0,
                'time_left': '0:00:00',
                'eta': 'unknown',
                'priority': 'Normal',
                'category': 'tv' if parsed.is_series else 'movies',
                'guid': guid,
                'added': datetime.now().isoformat()
            }
            
            logger.info(f"✅ Added to queue with NZO ID: {nzo_id}")
            
            return nzo_id
            
        except Exception as e:
            logger.error(f"Error adding file: {e}", exc_info=True)
            return None
    
    def add_url(self, url: str, filename: Optional[str] = None) -> Optional[str]:
        """
        Add a download from URL
        """
        try:
            logger.info(f"Adding URL: {url}")
            
            # Get file info from Fshare
            file_info = self.fshare.get_file_info(url)
            if not file_info:
                logger.error("Failed to get file info from Fshare")
                return None
            
            # Parse and normalize filename
            parsed = self.normalizer.parse(file_info['name'])
            normalized_filename = parsed.normalized_filename
            
            # Get direct download link
            download_url = self.fshare.get_download_link(file_info['fcode'])
            if not download_url:
                logger.error("Failed to get download link from Fshare")
                return None
            
            # Send to pyLoad
            success = self.pyload.add_download(
                download_url,
                filename=normalized_filename,
                package_name=parsed.title
            )
            
            if not success:
                logger.error("Failed to add download to pyLoad")
                return None
            
            # Generate NZO ID
            nzo_id = str(uuid.uuid4())
            
            # Add to queue
            self.queue[nzo_id] = {
                'nzo_id': nzo_id,
                'filename': normalized_filename,
                'original_filename': file_info['name'],
                'status': 'Downloading',
                'percentage': 0,
                'mb_left': file_info['size'] / (1024 * 1024),
                'mb_total': file_info['size'] / (1024 * 1024),
                'time_left': '0:00:00',
                'eta': 'unknown',
                'priority': 'Normal',
                'category': 'tv' if parsed.is_series else 'movies',
                'fshare_url': url,
                'added': datetime.now().isoformat()
            }
            
            logger.info(f"✅ Download started with NZO ID: {nzo_id}")
            
            return nzo_id
            
        except Exception as e:
            logger.error(f"Error adding URL: {e}", exc_info=True)
            return None
    
    def get_queue(self) -> Dict:
        """Get current download queue"""
        slots = []
        
        for nzo_id, item in self.queue.items():
            slots.append({
                'nzo_id': nzo_id,
                'filename': item['filename'],
                'status': item['status'],
                'percentage': item['percentage'],
                'mb': item['mb_total'],
                'mbleft': item['mb_left'],
                'timeleft': item['time_left'],
                'eta': item['eta'],
                'priority': item['priority'],
                'cat': item['category']
            })
        
        return {
            'queue': {
                'status': 'Downloading' if slots else 'Idle',
                'speed': '0',
                'size': str(sum(item['mb_total'] for item in self.queue.values())),
                'sizeleft': str(sum(item['mb_left'] for item in self.queue.values())),
                'slots': slots,
                'noofslots': len(slots)
            }
        }
    
    def get_history(self, limit: int = 50) -> Dict:
        """Get download history"""
        slots = []
        
        for nzo_id, item in list(self.history.items())[:limit]:
            slots.append({
                'nzo_id': nzo_id,
                'name': item['filename'],
                'status': item.get('status', 'Completed'),
                'fail_message': '',
                'category': item['category'],
                'size': str(item.get('mb_total', 0)),
                'completed': item.get('completed', datetime.now().isoformat())
            })
        
        return {
            'history': {
                'slots': slots,
                'noofslots': len(slots)
            }
        }
    
    def get_version(self) -> str:
        """Get SABnzbd version (fake)"""
        return '3.5.0'
    
    def pause_queue(self) -> bool:
        """Pause download queue"""
        logger.info("Queue paused")
        return True
    
    def resume_queue(self) -> bool:
        """Resume download queue"""
        logger.info("Queue resumed")
        return True


def create_sabnzbd_api(fshare_client: FshareClient, pyload_client: PyLoadClient, filename_normalizer: FilenameNormalizer) -> Blueprint:
    """Create and configure the SABnzbd API blueprint"""
    
    api = SABnzbdAPI(fshare_client, pyload_client, filename_normalizer)
    
    @sabnzbd_bp.route('/api', methods=['GET', 'POST'])
    def api_endpoint():
        """Main SABnzbd API endpoint"""
        mode = request.args.get('mode') or request.form.get('mode')
        output = request.args.get('output', 'json')
        
        if not mode:
            return jsonify({'error': 'No mode specified'}), 400
        
        logger.info(f"SABnzbd API request: mode={mode}")
        
        if mode == 'addfile':
            # Add download from NZB file
            if 'name' not in request.files:
                return jsonify({'error': 'No file provided'}), 400
            
            nzb_file = request.files['name']
            nzb_data = nzb_file.read()
            
            nzo_id = api.add_file(nzb_data, nzb_file.filename)
            
            if nzo_id:
                if output == 'json':
                    return jsonify({'status': True, 'nzo_ids': [nzo_id]})
                else:
                    return f"ok\n{nzo_id}"
            else:
                if output == 'json':
                    return jsonify({'status': False, 'error': 'Failed to add download'}), 400
                else:
                    return "error", 400
        
        elif mode == 'addurl':
            # Add download from URL
            url = request.args.get('name') or request.form.get('name')
            
            if not url:
                return jsonify({'error': 'No URL provided'}), 400
            
            nzo_id = api.add_url(url)
            
            if nzo_id:
                if output == 'json':
                    return jsonify({'status': True, 'nzo_ids': [nzo_id]})
                else:
                    return f"ok\n{nzo_id}"
            else:
                if output == 'json':
                    return jsonify({'status': False, 'error': 'Failed to add download'}), 400
                else:
                    return "error", 400
        
        elif mode == 'queue':
            # Get queue
            queue_data = api.get_queue()
            
            if output == 'json':
                return jsonify(queue_data)
            else:
                return str(queue_data)
        
        elif mode == 'history':
            # Get history
            limit = int(request.args.get('limit', 50))
            history_data = api.get_history(limit)
            
            if output == 'json':
                return jsonify(history_data)
            else:
                return str(history_data)
        
        elif mode == 'version':
            # Get version
            version = api.get_version()
            
            if output == 'json':
                return jsonify({'version': version})
            else:
                return version
        
        elif mode == 'pause':
            # Pause queue
            api.pause_queue()
            
            if output == 'json':
                return jsonify({'status': True})
            else:
                return "ok"
        
        elif mode == 'resume':
            # Resume queue
            api.resume_queue()
            
            if output == 'json':
                return jsonify({'status': True})
            else:
                return "ok"
        
        else:
            return jsonify({'error': f'Unknown mode: {mode}'}), 400
    
    return sabnzbd_bp
