"""
SABnzbd Routes Blueprint

Implements SABnzbd-compatible API for *arr suite download client integration.
"""

import logging
from flask import Blueprint, request, jsonify

from ..services.sabnzbd import SABnzbdEmulator
from ..clients.fshare import FshareClient
from ..core.config import get_config

logger = logging.getLogger(__name__)

sabnzbd_bp = Blueprint("sabnzbd", __name__)

# Lazy-loaded emulator
_emulator: SABnzbdEmulator = None


def get_emulator() -> SABnzbdEmulator:
    """Get or create SABnzbd emulator instance."""
    global _emulator
    if _emulator is None:
        # Create a mock download client for now
        # In production, this would be the DownloadEngine
        class MockDownloader:
            def add_download(self, url, filename=None, package_name=None, category="Uncategorized"):
                logger.info(f"Mock download: {filename}")
                return True
            
            def get_queue(self):
                return []
            
            def get_status(self):
                return {"status": "idle"}
        
        fshare_client = FshareClient.from_config()
        _emulator = SABnzbdEmulator(fshare_client, MockDownloader())
    return _emulator


@sabnzbd_bp.route("/api", methods=["GET", "POST"])
def api_endpoint():
    """
    Main SABnzbd API endpoint.
    
    Query/Form params:
        - mode: API mode (addfile, addurl, queue, history, version, pause, resume)
        - output: Output format (json or text)
        - apikey: API key (ignored, for compatibility)
    """
    mode = request.args.get("mode") or request.form.get("mode")
    output = request.args.get("output", "json")
    emulator = get_emulator()
    
    if not mode:
        return _response({"error": "No mode specified"}, output, 400)
    
    logger.info(f"SABnzbd API request: mode={mode}")
    
    if mode == "addfile":
        # Add download from NZB file
        if "name" not in request.files:
            return _response({"error": "No file provided"}, output, 400)
        
        nzb_file = request.files["name"]
        nzb_data = nzb_file.read()
        category = request.args.get("cat") or request.form.get("cat")
        
        nzo_id = emulator.add_file(nzb_data, nzb_file.filename, category)
        
        if nzo_id:
            return _response({"status": True, "nzo_ids": [nzo_id]}, output)
        return _response({"status": False, "error": "Failed to add download"}, output, 400)
    
    elif mode == "addurl":
        # Add download from URL
        url = request.args.get("name") or request.form.get("name")
        
        if not url:
            return _response({"error": "No URL provided"}, output, 400)
        
        category = request.args.get("cat") or request.form.get("cat")
        nzo_id = emulator.add_url(url, category=category)
        
        if nzo_id:
            return _response({"status": True, "nzo_ids": [nzo_id]}, output)
        return _response({"status": False, "error": "Failed to add download"}, output, 400)
    
    elif mode == "queue":
        # Get queue
        queue_data = emulator.get_queue()
        return _response(queue_data, output)
    
    elif mode == "history":
        # Get history
        limit = int(request.args.get("limit", 50))
        history_data = emulator.get_history(limit)
        return _response(history_data, output)
    
    elif mode == "version":
        # Get version
        version = emulator.get_version()
        return _response({"version": version}, output)
    
    elif mode == "pause":
        # Pause queue
        emulator.pause_queue()
        return _response({"status": True}, output)
    
    elif mode == "resume":
        # Resume queue
        emulator.resume_queue()
        return _response({"status": True}, output)
    
    elif mode == "fullstatus":
        # Full status for *arr compatibility
        queue_data = emulator.get_queue()
        return _response({
            "status": True,
            **queue_data,
        }, output)
    
    else:
        return _response({"error": f"Unknown mode: {mode}"}, output, 400)


def _response(data: dict, output: str, status: int = 200):
    """Format response based on output type."""
    if output == "json":
        return jsonify(data), status
    else:
        # Text format
        if "error" in data:
            return f"error\n{data['error']}", status
        elif "status" in data:
            return "ok" if data.get("status") else "error", status
        else:
            return str(data), status
