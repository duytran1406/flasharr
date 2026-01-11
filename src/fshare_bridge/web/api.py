"""
API Blueprint

REST API endpoints for the web UI.
"""

import logging
from flask import Blueprint, jsonify, request
from typing import Any, Dict

from ..core.config import get_config
from ..downloader.queue import DownloadQueue
from ..utils.formatters import format_size, format_speed, format_duration

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


@api_bp.route("/stats")
def get_stats() -> Dict[str, Any]:
    """
    Get system statistics.
    
    Returns JSON with download stats, speeds, and account info.
    """
    try:
        queue = get_queue()
        stats = queue.get_statistics()
        active = queue.get_active_tasks()
        
        # Calculate current speed from active downloads
        total_speed = sum(
            task.get("speed", 0) 
            for task in active
        )
        
        return jsonify({
            "status": "ok",
            "downloads": {
                "total": stats["total"],
                "queued": stats["queued"],
                "downloading": stats["downloading"],
                "paused": stats["paused"],
                "completed": stats["completed"],
                "failed": stats["failed"],
            },
            "speed": {
                "bytes_per_sec": total_speed,
                "formatted": format_speed(total_speed),
            },
            "storage": {
                "total_bytes": stats["total_bytes"],
                "downloaded_bytes": stats["downloaded_bytes"],
                "formatted_total": format_size(stats["total_bytes"]),
                "formatted_downloaded": format_size(stats["downloaded_bytes"]),
            },
        })
    except Exception as e:
        logger.error(f"Error getting stats: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/downloads")
def get_downloads() -> Dict[str, Any]:
    """
    Get all downloads (queue + history).
    
    Query params:
        - status: Filter by status (queued, downloading, paused, completed, failed)
        - limit: Maximum results (default 100)
    """
    try:
        queue = get_queue()
        status_filter = request.args.get("status")
        limit = int(request.args.get("limit", 100))
        
        # Get active and pending
        active = queue.get_active_tasks()
        pending = queue.get_pending_tasks(limit)
        history = queue.get_history(limit)
        
        # Combine all
        all_downloads = active + pending + history
        
        # Apply status filter
        if status_filter:
            all_downloads = [
                d for d in all_downloads 
                if d.get("state", "").lower() == status_filter.lower()
            ]
        
        # Format for response
        formatted = []
        for dl in all_downloads[:limit]:
            total_bytes = dl.get("total_bytes", 0)
            downloaded = dl.get("downloaded_bytes", 0)
            speed = dl.get("speed", 0)
            
            # Calculate progress and ETA
            progress = (downloaded / total_bytes * 100) if total_bytes > 0 else 0
            eta = 0
            if speed > 0 and total_bytes > downloaded:
                eta = (total_bytes - downloaded) / speed
            
            formatted.append({
                "id": dl.get("id"),
                "filename": dl.get("filename"),
                "url": dl.get("url"),
                "state": dl.get("state"),
                "category": dl.get("category"),
                "progress": round(progress, 1),
                "size": {
                    "total": total_bytes,
                    "downloaded": downloaded,
                    "formatted_total": format_size(total_bytes),
                    "formatted_downloaded": format_size(downloaded),
                },
                "speed": {
                    "bytes_per_sec": speed,
                    "formatted": format_speed(speed),
                },
                "eta": {
                    "seconds": eta,
                    "formatted": format_duration(eta) if eta > 0 else "--",
                },
                "created_at": dl.get("created_at"),
                "error": dl.get("error_message"),
            })
        
        return jsonify({
            "status": "ok",
            "count": len(formatted),
            "downloads": formatted,
        })
    except Exception as e:
        logger.error(f"Error getting downloads: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/downloads", methods=["POST"])
def add_download() -> Dict[str, Any]:
    """
    Add a new download.
    
    Request body:
        - url: Fshare URL (required)
        - category: Download category (optional)
    """
    try:
        data = request.get_json()
        
        if not data or "url" not in data:
            return jsonify({"status": "error", "message": "URL required"}), 400
        
        url = data["url"]
        category = data.get("category", "Uncategorized")
        
        # Import here to avoid circular imports
        from ..clients.fshare import FshareClient
        from ..downloader.fshare_handler import FshareDownloadHandler
        from ..downloader.engine import DownloadTask, DownloadState
        from ..utils.filename_parser import FilenameParser
        import uuid
        from pathlib import Path
        
        # Initialize clients
        config = get_config()
        fshare_client = FshareClient.from_config()
        handler = FshareDownloadHandler(fshare_client)
        parser = FilenameParser()
        
        # Validate and resolve URL
        success, resolved, error = handler.validate_and_resolve(url)
        if not success:
            return jsonify({"status": "error", "message": error}), 400
        
        # Parse filename
        parsed = parser.parse(resolved.filename)
        normalized_filename = parsed.normalized_filename
        
        # Create task
        task_id = str(uuid.uuid4())
        task = DownloadTask(
            id=task_id,
            url=resolved.direct_url,
            filename=normalized_filename,
            destination=Path(config.download.download_dir) / category / normalized_filename,
            state=DownloadState.QUEUED,
            category=category,
            package_name=parsed.title,
        )
        task.progress.total_bytes = resolved.size_bytes
        
        # Add to queue
        queue = get_queue()
        queue.add_task(task)
        
        logger.info(f"Added download: {normalized_filename}")
        
        return jsonify({
            "status": "ok",
            "id": task_id,
            "filename": normalized_filename,
            "size": format_size(resolved.size_bytes),
        })
    except Exception as e:
        logger.error(f"Error adding download: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/downloads/<task_id>", methods=["DELETE"])
def delete_download(task_id: str) -> Dict[str, Any]:
    """Delete a download from the queue."""
    try:
        queue = get_queue()
        if queue.delete_task(task_id):
            return jsonify({"status": "ok", "message": "Deleted"})
        return jsonify({"status": "error", "message": "Not found"}), 404
    except Exception as e:
        logger.error(f"Error deleting download: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/downloads/<task_id>/pause", methods=["POST"])
def pause_download(task_id: str) -> Dict[str, Any]:
    """Pause a download."""
    try:
        queue = get_queue()
        task = queue.get_task(task_id)
        if task:
            # Update state in DB
            from ..downloader.engine import DownloadState
            task["state"] = DownloadState.PAUSED.value
            # Note: Actual pause logic would be handled by engine
            return jsonify({"status": "ok", "message": "Paused"})
        return jsonify({"status": "error", "message": "Not found"}), 404
    except Exception as e:
        logger.error(f"Error pausing download: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/downloads/<task_id>/resume", methods=["POST"])
def resume_download(task_id: str) -> Dict[str, Any]:
    """Resume a paused download."""
    try:
        queue = get_queue()
        task = queue.get_task(task_id)
        if task:
            return jsonify({"status": "ok", "message": "Resumed"})
        return jsonify({"status": "error", "message": "Not found"}), 404
    except Exception as e:
        logger.error(f"Error resuming download: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/config")
def get_config_endpoint() -> Dict[str, Any]:
    """Get application configuration (safe subset)."""
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
    """Get application version."""
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
