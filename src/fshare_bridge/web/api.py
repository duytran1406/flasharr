"""
API Blueprint

REST API endpoints for the web UI.
"""

import logging
import time
import psutil
from flask import Blueprint, jsonify, request
from typing import Any, Dict

from ..core.config import get_config
from ..downloader.queue import DownloadQueue
from ..utils.formatters import format_size, format_speed, format_duration
from ..core.settings_store import get_settings_store
from ..clients.pyload import PyLoadClient

logger = logging.getLogger(__name__)

api_bp = Blueprint("api", __name__)

# Lazy-loaded shared instances
_queue: DownloadQueue = None


def get_queue() -> DownloadQueue:
    """Get or create download queue instance."""
    global _queue
    if _queue is None:
        _queue = DownloadQueue()
    return _queue

def get_pyload_client():
    """Get configured PyLoad client."""
    config = get_config()
    # Ensure hot is pyload service name
    host = config.pyload.host if config.pyload.host != "localhost" else "pyload"
    return PyLoadClient(
        f"http://{host}:{config.pyload.port}",
        username=config.pyload.username,
        password=config.pyload.password
    )

@api_bp.route("/stats")
def get_stats() -> Dict[str, Any]:
    """Get system and PyLoad stats."""
    try:
        pyload = get_pyload_client()
        pyload.login()
        status = pyload.get_status() # Expect dict
        
        # Determine stats from PyLoad status response
        # Using safe defaults if keys missing
        total_speed = float(status.get("speed", 0) or 0)
        active_cnt = int(status.get("active", 0) or 0)
        
        # System uptime
        uptime = int(time.time() - psutil.boot_time())
        
        # Fshare account status check
        settings_store = get_settings_store()
        settings = settings_store.get_app_settings()
        has_creds = bool(settings.fshare_email and settings.fshare_password)

        return jsonify({
            "status": "ok",
            "system": {
                "speedtest": format_speed(total_speed),
                "uptime": uptime
            },
            "pyload": {
                "active": active_cnt,
                "total": int(status.get("total", 0) or 0),
                "connected": True, 
                "speed_bytes": total_speed,
                "fshare_account": {
                    "valid": has_creds,
                    "premium": True 
                }
            }
        })
    except Exception as e:
        logger.error(f"Error getting stats: {e}")
        # Fallback response
        return jsonify({
            "status": "ok", 
            "system": {"speedtest": "0 B/s", "uptime": 0},
            "pyload": {"active": 0, "connected": False, "error": str(e)}
        })


@api_bp.route("/downloads")
def get_downloads() -> Dict[str, Any]:
    """Get downloads from PyLoad."""
    try:
        pyload = get_pyload_client()
        pyload.login()
        
        # Get raw queue
        queue_data = pyload.get_queue() 
        
        # If queue_data is None/Error, try to handle
        if queue_data is None:
            queue_data = []

        formatted = []
        
        # Adapt PyLoad structure to UI
        # PyLoad (NG) often returns list of Packages, each has links
        if isinstance(queue_data, list):
            for pkg in queue_data:
                # If it's a package
                if "links" in pkg:
                    for link in pkg.get("links", []):
                        formatted.append(_format_pyload_link(link))
                # If it's a flat file list (unlikely but possible)
                elif "name" in pkg and "url" in pkg:
                     formatted.append(_format_pyload_link(pkg))
                     
        return jsonify({
            "status": "ok",
            "count": len(formatted),
            "downloads": formatted,
        })
    except Exception as e:
        logger.error(f"Error getting downloads: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500

def _format_pyload_link(link):
    """Helper to format a pyload link object."""
    # Mapping PyLoad fields to UI fields
    # status: 0=finished, 1=offline, 2=online, 3=queued, 4=skipped, 5=waiting, 6=temp_offline, 7=starting, 8=downloading...
    # statusmsg: text representation
    
    return {
        "id": str(link.get("id", "")),
        "filename": link.get("name", "Unknown"),
        "url": link.get("url", ""),
        "state": link.get("statusmsg", "Queued"),
        "category": "Uncategorized",
        "progress": link.get("progress", 0),
        "size": {
            "formatted_total": link.get("format_size", "0 B"), 
            "total": link.get("size", 0)
        },
        "speed": {
            "formatted": link.get("format_speed", "0 B/s"),
            "bytes_per_sec": link.get("speed", 0)
        },
        "eta": {
            "formatted": link.get("format_eta", "00:00:00"),
            "seconds": link.get("eta", 0)
        }
    }


@api_bp.route("/downloads", methods=["POST"])
def add_download() -> Dict[str, Any]:
    """Add a new download to PyLoad."""
    try:
        data = request.get_json()
        if not data or "url" not in data:
            return jsonify({"status": "error", "message": "URL required"}), 400
        
        url = data["url"]
        name = data.get("name", "New Download")
        
        pyload = get_pyload_client()
        pyload.login()
        
        # Add package
        result = pyload.add_package(name, [url])
        
        return jsonify({
            "status": "ok",
            "success": True,
            "pyload_id": result
        })
    except Exception as e:
        logger.error(f"Error adding download: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/downloads/<task_id>", methods=["DELETE"])
def delete_download(task_id: str) -> Dict[str, Any]:
    """Delete a download from PyLoad."""
    try:
        pyload = get_pyload_client()
        pyload.login()
        # PyLoad delete expects list of IDs
        pyload.delete_files([int(task_id)])
        return jsonify({"status": "ok", "message": "Deleted", "success": True})
    except Exception as e:
        logger.error(f"Error deleting download: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/downloads/<task_id>/pause", methods=["POST"])
def pause_download(task_id: str) -> Dict[str, Any]:
    """Pause a download."""
    # PyLoad usually supports stopping a file, not pausing individual file in free version easily, but trying stopFile
    try:
        pyload = get_pyload_client()
        pyload.login()
        pyload.stop_file(int(task_id))
        return jsonify({"status": "ok", "message": "Paused", "success": True})
    except Exception as e:
        logger.error(f"Error pausing download: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/downloads/<task_id>/resume", methods=["POST"])
def resume_download(task_id: str) -> Dict[str, Any]:
    """Resume a paused download."""
    try:
        pyload = get_pyload_client()
        pyload.login()
        pyload.restart_file(int(task_id)) # Restart is resume?
        return jsonify({"status": "ok", "message": "Resumed", "success": True})
    except Exception as e:
        logger.error(f"Error resuming download: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/download/toggle/<task_id>", methods=["POST"])
def toggle_download(task_id: str) -> Dict[str, Any]:
    """Toggle download state."""
    # Simple toggle logic if feasible, else just restart/stop
    return resume_download(task_id) # Simplify for now
        
@api_bp.route("/download/delete/<task_id>", methods=["DELETE"])
def legacy_delete_download(task_id: str) -> Dict[str, Any]:
    return delete_download(task_id)

@api_bp.route("/config")
def get_config_endpoint() -> Dict[str, Any]:
    try:
        config = get_config()
        return jsonify({
            "status": "ok",
            "config": {
                "download_dir": config.download.download_dir,
                "max_concurrent": config.download.max_concurrent,
                "server_port": config.server.port,
            },
        })
    except Exception as e:
        logger.error(f"Error getting config: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/version")
def get_version() -> Dict[str, Any]:
    try:
        version = "1.0.0-alpha"
        try:
            with open("/app/VERSION", "r") as f:
                version = f.read().strip()
        except:
            pass
        return jsonify({
            "status": "ok",
            "version": version,
        })
    except Exception as e:
        return jsonify({"status": "error", "message": str(e)}), 500
