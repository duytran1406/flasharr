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

from flask import Blueprint, request, jsonify, current_app

logger = logging.getLogger(__name__)

sabnzbd_bp = Blueprint("sabnzbd", __name__)


@sabnzbd_bp.route("/api", methods=["GET", "POST"])
def api_endpoint():
    """
    Main SABnzbd API endpoint.
    """
    if not hasattr(current_app, 'sabnzbd') or not current_app.sabnzbd:
        return jsonify({"error": "SABnzbd service not initialized"}), 503

    mode = request.args.get("mode") or request.form.get("mode")
    output = request.args.get("output", "json")
    emulator = current_app.sabnzbd
    
    if not mode:
        return _response({"error": "No mode specified"}, output, 400)
    
    try:
        if mode == "addfile":
            # Handle NZB file upload
            if "name" not in request.files and "nzbfile" not in request.files:
                return _response({"error": "No file provided"}, output, 400)
            
            nzb_file = request.files.get("name") or request.files.get("nzbfile")
            nzb_data = nzb_file.read()
            category = request.args.get("cat") or request.form.get("cat")
            
            nzo_id = emulator.add_file(nzb_data, nzb_file.filename, category)
            if nzo_id:
                return _response({"status": True, "nzo_ids": [nzo_id]}, output)
            return _response({"status": False}, output)

        elif mode == "addurl":
            url = request.args.get("name") or request.form.get("name")
            if not url:
                return _response({"error": "No URL provided"}, output, 400)
            
            category = request.args.get("cat") or request.form.get("cat")
            nzo_id = emulator.add_url(url, category=category)
            if nzo_id:
                return _response({"status": True, "nzo_ids": [nzo_id]}, output)
            return _response({"status": False}, output)

        elif mode == "queue":
            return _response(emulator.get_queue(), output)

        elif mode == "history":
            limit = int(request.args.get("limit", 50))
            return _response(emulator.get_history(limit), output)

        elif mode == "version":
            return _response({"version": emulator.get_version()}, output)

        elif mode == "pause":
            emulator.pause_queue()
            return _response({"status": True}, output)

        elif mode == "resume":
            emulator.resume_queue()
            return _response({"status": True}, output)
            
        elif mode == "fullstatus":
            return _response({"status": True, **emulator.get_queue()}, output)

        else:
            return _response({"error": f"Unknown mode: {mode}"}, output, 400)

    except Exception as e:
        logger.error(f"SABnzbd API error: {e}", exc_info=True)
        return _response({"error": str(e)}, output, 500)


def _response(data: dict, output: str, status: int = 200):
    """Format response based on output type."""
    if output == "json":
        return jsonify(data), status
    else:
        # Simple text responses for legacy mode
        if "error" in data:
            return f"error\n{data['error']}", status
        return "ok", status
