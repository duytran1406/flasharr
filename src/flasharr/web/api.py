"""
API Blueprint

REST API endpoints for the web UI.
"""

import logging
import time
import os
import json
import psutil
from flask import Blueprint, jsonify, request, current_app
from typing import Any, Dict
from datetime import datetime
from pathlib import Path

from ..core.config import get_config
from ..utils.formatters import format_size, format_speed, format_duration, format_eta

logger = logging.getLogger(__name__)

api_bp = Blueprint("api", __name__)


@api_bp.route("/stats")
def get_stats() -> Dict[str, Any]:
    """Get system and engine stats."""
    try:
        # System uptime
        uptime = int(time.time() - psutil.boot_time())
        
        # Get primary account info
        primary = current_app.account_manager.get_primary() if hasattr(current_app, 'account_manager') else None
        
        fshare_data = {
            "valid": primary is not None,
            "premium": primary.get('premium', False) if primary else False
        }
        
        if primary:
            fshare_data['email'] = primary.get('email')
            fshare_data['traffic_left'] = primary.get('traffic_left')
            fshare_data['account_type'] = primary.get('account_type')
            if primary.get('validuntil'):
                from datetime import datetime
                if primary.get('validuntil') == -1:
                    fshare_data['expiry'] = "Lifetime"
                else:
                    dt = datetime.fromtimestamp(primary.get('validuntil'))
                    fshare_data['expiry'] = dt.strftime('%d-%m-%Y')

        # Get downloader status from SABnzbd service if available
        # or from the engine directly if needed
        sab = getattr(current_app, 'sabnzbd', None)
        downloader = sab.downloader if sab else None
        
        active_cnt = 0
        total_cnt = 0
        total_speed = 0
        connected = False

        if sab:
            counts = sab.get_counts()
            active_cnt = counts.get("active", 0)
            total_cnt = counts.get("total", 0)
            
            status = downloader.get_status() if downloader else {}
            total_speed = status.get("total_speed", 0)
            connected = True
        elif downloader:
            status = downloader.get_status()
            total_speed = status.get("total_speed", 0)
            active_cnt = status.get("active", 0)
            total_cnt = status.get("total", 0) # Fallback
            connected = True

        return jsonify({
            "status": "ok",
            "system": {
                "speedtest": format_speed(total_speed),
                "uptime": uptime
            },
            "fshare_downloader": { # Unified key for UI
                "active": active_cnt,
                "total": total_cnt,
                "connected": connected, 
                "speed": format_speed(total_speed),
                "speed_bytes": total_speed,
                "primary_account": fshare_data
            },
            "pyload": { # Legacy compatibility
                "active": active_cnt,
                "total": total_cnt,
                "connected": connected,
                "speed_bytes": total_speed,
                "fshare_account": fshare_data
            }
        })
    except Exception as e:
        logger.error(f"Error getting stats: {e}")
        return jsonify({
            "status": "ok", 
            "system": {"speedtest": "0 B/s", "uptime": 0},
            "pyload": {"active": 0, "connected": False, "error": str(e)}
        })


@api_bp.route("/downloads")
def get_downloads() -> Dict[str, Any]:
    """Get downloads from SABnzbd Emulator (Active Queue + History)."""
    try:
        if not hasattr(current_app, 'sabnzbd') or not current_app.sabnzbd:
            return jsonify({"status": "error", "message": "Downloader not initialized"}), 503
            
        sab = current_app.sabnzbd
        
        # Trigger sync with engine to update active tasks
        sab.get_queue() 
        
        formatted = []
        
        # Helper to convert QueueItem to compatible dict for _format_task
        def item_to_dict(item):
            return {
                "id": item.nzo_id,
                "nzo_id": item.nzo_id,
                "filename": item.filename,
                "url": item.fshare_url,
                "status": item.status.value if hasattr(item.status, 'value') else str(item.status),
                "category": item.category,
                "progress": item.percentage,
                "size_bytes": item.size_bytes,
                "mb_total": item.mb_total,
                "speed": item.speed,
                "eta": item.eta_seconds,
                "added": item.added
            }

        # 1. Add History Items (Completed/Failed)
        for item in sab._history.values():
            formatted.append(_format_task(item_to_dict(item)))

        # 2. Add Active Queue Items (Running/Paused/Queued)
        # Use a set to track IDs to avoid duplicates
        seen_ids = {item['id'] for item in formatted}
        
        for item in sab._queue.values():
            if item.nzo_id not in seen_ids:
                formatted.append(_format_task(item_to_dict(item)))

        # Sort by added time descending (newest first)
        formatted.sort(key=lambda x: str(x.get('added', '')), reverse=True)
                     
        return jsonify({
            "status": "ok",
            "count": len(formatted),
            "downloads": formatted,
        })
    except Exception as e:
        logger.error(f"Error getting downloads: {e}", exc_info=True)
        return jsonify({"status": "error", "message": str(e)}), 500


def _format_task(task):
    """Helper to format a task object for the UI."""
    # Resolve size bytes
    # Prefer explicit size_bytes, fall back to 'size' (from engine) or 'mb' (from emulator)
    size_raw = task.get("size_bytes") or task.get("size")
    mb_raw = task.get("mb") or task.get("mb_total")
    
    size_bytes = 0
    if size_raw and not isinstance(size_raw, str):
        size_bytes = int(size_raw)
    elif mb_raw:
        # Convert MB to bytes for consistent formatting
        try:
            size_bytes = int(float(mb_raw) * 1024 * 1024)
        except (ValueError, TypeError):
            size_bytes = 0
            
    # Final safety: if we still have a string or weird value
    if isinstance(size_raw, str) and not size_bytes:
        try:
            size_bytes = int(float(size_raw))
        except:
            size_bytes = 0

    return {
        "id": str(task.get("id") or task.get("nzo_id", "")),
        "filename": task.get("filename", "Unknown"),
        "url": task.get("url") or task.get("fshare_url", ""),
        "state": task.get("status") or task.get("state", "Queued"),
        "category": task.get("category", "Uncategorized"),
        "progress": task.get("progress", 0),
        "size": {
            "formatted_total": format_size(size_bytes), 
            "total": size_bytes,
            "mb": f"{size_bytes / (1024*1024):.2f}"
        },
        "speed": {
            "formatted": format_speed(task.get("speed", 0)),
            "bytes_per_sec": task.get("speed", 0)
        },
        "eta": {
            "formatted": format_eta(task.get("eta", 0)),
            "seconds": task.get("eta", 0)
        },
        "added": task.get("added") or task.get("created_at")
    }


@api_bp.route("/downloads", methods=["POST"])
def add_download() -> Dict[str, Any]:
    """Add a new download to engine."""
    try:
        data = request.get_json()
        if not data or "url" not in data:
            return jsonify({"status": "error", "message": "URL required"}), 400
        
        url = data["url"]
        name = data.get("name")
        category = data.get("category", "Manual")
        
        if not hasattr(current_app, 'sabnzbd') or not current_app.sabnzbd:
            return jsonify({"status": "error", "message": "Downloader not initialized"}), 503
            
        # Use SABnzbdEmulator.add_url instead of direct downloader to ensure state tracking
        nzo_id = current_app.sabnzbd.add_url(url, filename=name, category=category)
        
        return jsonify({
            "status": "ok",
            "success": nzo_id is not None,
            "nzo_id": nzo_id
        })
    except Exception as e:
        logger.error(f"Error adding download: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/downloads/<task_id>", methods=["DELETE"])
def delete_download(task_id: str) -> Dict[str, Any]:
    """Delete a download."""
    try:
        if not hasattr(current_app, 'sabnzbd') or not current_app.sabnzbd:
            return jsonify({"status": "error", "message": "Downloader not initialized"}), 503
            
        success = current_app.sabnzbd.delete_item(task_id)
        if not success:
            return jsonify({"status": "error", "message": "Item not found in queue or history", "success": False}), 404
            
        return jsonify({"status": "ok", "message": f"Deleted {task_id}", "success": True})
    except Exception as e:
        logger.error(f"Error deleting download: {e}")
        return jsonify({"status": "error", "message": str(e), "success": False}), 500

@api_bp.route("/download/delete/<task_id>", methods=["DELETE", "GET"])
def delete_download_alias(task_id: str):
    """Alias for delete_download to support frontend."""
    return delete_download(task_id)


@api_bp.route("/downloads/<task_id>/pause", methods=["POST"])
def pause_download(task_id: str) -> Dict[str, Any]:
    """Pause a download."""
    try:
        if not hasattr(current_app, 'sabnzbd') or not current_app.sabnzbd:
            return jsonify({"status": "error", "message": "Downloader not initialized"}), 503
            
        success = current_app.sabnzbd.toggle_item(task_id) # toggle_item handles both pause/resume in SAB emulator
        return jsonify({"status": "ok", "message": "Paused/Toggled", "success": success})
    except Exception as e:
        logger.error(f"Error pausing download: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500

@api_bp.route("/download/pause/<task_id>", methods=["POST", "GET"])
def pause_download_alias(task_id: str):
    return pause_download(task_id)


@api_bp.route("/downloads/<task_id>/resume", methods=["POST"])
def resume_download(task_id: str) -> Dict[str, Any]:
    """Resume a paused download."""
    try:
        if not hasattr(current_app, 'sabnzbd') or not current_app.sabnzbd:
            return jsonify({"status": "error", "message": "Downloader not initialized"}), 503
            
        success = current_app.sabnzbd.toggle_item(task_id)
        return jsonify({"status": "ok", "message": "Resumed/Toggled", "success": success})
    except Exception as e:
        logger.error(f"Error resuming download: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500

@api_bp.route("/download/resume/<task_id>", methods=["POST", "GET"])
def resume_download_alias(task_id: str):
    return resume_download(task_id)

@api_bp.route("/download/retry/<task_id>", methods=["POST", "GET"])
def retry_download(task_id: str):
    """Retry a failed download."""
    try:
        if not hasattr(current_app, 'sabnzbd') or not current_app.sabnzbd:
            return jsonify({"status": "error", "message": "Downloader not initialized"}), 503
            
        success = current_app.sabnzbd.retry_item(task_id)
        return jsonify({"status": "ok", "message": "Retried", "success": success})
    except Exception as e:
        logger.error(f"Error retrying download: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/search")
def api_search():
    """Search API."""
    try:
        query = request.args.get('q', '')
        if not query:
            return jsonify({"results": []})
            
        # Use IndexerService's client (TimFshare)
        results = current_app.indexer.client.search(query, limit=50)
        
        # Format for UI
        formatted_results = []
        for item in results:
            parsed = current_app.indexer.parser.parse(item.name)
            
            formatted_results.append({
                "id": item.fcode,
                "name": parsed.title or item.name,
                "original_filename": item.name,
                "size": item.size,
                "url": item.url,
                "folder": False,
                "score": item.score,
                "is_series": parsed.is_series,
                "season": parsed.season,
                "episode": parsed.episode,
                "quality": parsed.quality,
                "vietsub": parsed.quality_attrs.viet_sub if parsed.quality_attrs else False,
                "vietdub": parsed.quality_attrs.viet_dub if parsed.quality_attrs else False,
                "metadata": parsed.quality_attrs.to_dict() if parsed.quality_attrs else {}
            })
            
        return jsonify({"results": formatted_results})
    except Exception as e:
        logger.error(f"Search API error: {e}")
        return jsonify({"error": str(e)}), 500


@api_bp.route("/autocomplete")
def api_autocomplete():
    """Autocomplete suggestions."""
    return jsonify({"suggestions": []})


@api_bp.route("/download", methods=["POST"])
def download_from_search():
    """Start a download from search results."""
    try:
        data = request.get_json()
        url = data.get('url')
        name = data.get('name')
        
        if not url:
            return jsonify({"success": False, "error": "URL required"}), 400
            
        if not hasattr(current_app, 'sabnzbd') or not current_app.sabnzbd:
            return jsonify({"success": False, "error": "Downloader not initialized"}), 503
            
        nzo_id = current_app.sabnzbd.add_url(url, category='manual')
        if nzo_id:
            return jsonify({"success": True, "nzo_id": nzo_id})
        return jsonify({"success": False, "error": "Failed to add to queue"})
    except Exception as e:
        logger.error(f"Error adding download: {e}")
        return jsonify({"success": False, "error": str(e)}), 500


@api_bp.route("/accounts", methods=["GET"])
def list_accounts():
    """List Fshare accounts."""
    try:
        accounts = current_app.account_manager.list_accounts()
        primary = current_app.account_manager.get_primary()
        return jsonify({
            "status": "ok",
            "accounts": accounts,
            "primary": primary
        })
    except Exception as e:
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/accounts/add", methods=["POST"])
def add_account():
    """Add Fshare account."""
    try:
        data = request.get_json()
        email = data.get("email")
        password = data.get("password")
        if not email or not password:
            return jsonify({"status": "error", "message": "Email and password required"}), 400
            
        account = current_app.account_manager.add_account(email, password)
        return jsonify({"status": "ok", "account": account})
    except Exception as e:
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/accounts/<email>", methods=["DELETE"])
def remove_account(email: str):
    """Remove account."""
    try:
        current_app.account_manager.remove_account(email)
        return jsonify({"status": "ok"})
    except Exception as e:
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/accounts/<email>/set-primary", methods=["POST"])
def set_primary_account(email: str):
    """Set account as primary."""
    try:
        current_app.account_manager.set_primary(email)
        return jsonify({"status": "ok"})
    except Exception as e:
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/accounts/<email>/refresh", methods=["POST"])
def refresh_account(email: str):
    """Refresh account info."""
    try:
        account = current_app.account_manager.refresh_account(email)
        return jsonify({"status": "ok", "account": account})
    except Exception as e:
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/settings", methods=["GET"])
def get_settings():
    """Get app settings."""
    try:
        config = get_config()
        settings_file = Path("data/settings.json")
        settings = {
            "download_path": config.download.download_dir,
            "max_concurrent_downloads": config.download.max_concurrent,
            "speed_limit_mbps": 0,
            "auto_resume": True,
            "category_paths": {"radarr": "movies", "sonarr": "tv"},
            "theme": "dark"
        }
        if settings_file.exists():
            with open(settings_file, "r") as f:
                settings.update(json.load(f))
        return jsonify({"status": "ok", "settings": settings})
    except Exception as e:
        return jsonify({"status": "error", "message": str(e)}), 500

@api_bp.route("/settings", methods=["PUT"])
def update_settings():
    """Update app settings."""
    try:
        data = request.get_json()
        settings_file = Path("data/settings.json")
        settings = {}
        if settings_file.exists():
            with open(settings_file, "r") as f:
                settings = json.load(f)
        
        settings.update(data)
        
        with open(settings_file, "w") as f:
            json.dump(settings, f, indent=4)
            
        # Apply runtime settings changes
        if 'max_concurrent_downloads' in data:
            if hasattr(current_app, 'sabnzbd') and current_app.sabnzbd:
                current_app.sabnzbd.downloader.engine.update_max_concurrent(int(data['max_concurrent_downloads']))
                
        if 'speed_limit_mbps' in data:
            if hasattr(current_app, 'sabnzbd') and current_app.sabnzbd:
                limit = int(data['speed_limit_mbps']) * 1024 * 1024 if int(data['speed_limit_mbps']) > 0 else 0
                current_app.sabnzbd.downloader.engine.set_speed_limit(limit)

        return jsonify({"status": "ok"})
    except Exception as e:
        logger.error(f"Error saving settings: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500

@api_bp.route("/verify-account", methods=["POST"])
def verify_account():
    """Verify primary account and fetch latest stats."""
    try:
        if not hasattr(current_app, 'account_manager'):
             return jsonify({"status": "error", "message": "Account validation service unavailable"}), 503

        primary = current_app.account_manager.get_primary()
        if not primary:
             return jsonify({"status": "error", "message": "No account configured"}), 404

        # Force refresh which calls login() -> check profile -> updates daily quota
        account = current_app.account_manager.refresh_account(primary['email'])
        
        # If refresh_account succeeded, the account is valid/functional
        is_valid = account is not None and 'email' in account
        
        if not is_valid:
            return jsonify({
                "status": "error", 
                "message": "Account login failed or session invalid",
                "account": account
            }), 401

        return jsonify({
            "status": "ok", 
            "message": "Account verified successfully",
            "account": account
        })
    except Exception as e:
        # Check if it looks like an auth error even if not detailed type
        err_msg = str(e).lower()
        if "session expired" in err_msg or "login failed" in err_msg:
             return jsonify({
                 "status": "error",
                 "code": "SESSION_EXPIRED",
                 "message": "Your Fshare session has expired. Please log in again."
             }), 401
             
        logger.error(f"Account verification failed: {e}")
        return jsonify({"status": "error", "message": f"Verification failed: {str(e)}"}), 500

@api_bp.route("/settings/generate-api-key", methods=["POST"])
def generate_api_key():
    """Generate a new random API key."""
    try:
        import secrets
        new_key = secrets.token_hex(16)
        return jsonify({"status": "ok", "key": new_key})
    except Exception as e:
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/config")
def get_config_endpoint() -> Dict[str, Any]:
    try:
        config = get_config()
        return jsonify({
            "status": "ok",
            "config": {
                "download_dir": config.download.download_dir,
                "max_concurrent": config.download.max_concurrent,
            },
        })
    except Exception as e:
        return jsonify({"status": "error", "message": str(e)}), 500


@api_bp.route("/version")
def get_version() -> Dict[str, Any]:
    version = "1.0.0-alpha"
    try:
        if os.path.exists("VERSION"):
            with open("VERSION", "r") as f:
                version = f.read().strip()
    except:
        pass
    return jsonify({"status": "ok", "version": version})

@api_bp.route("/logs")
def get_logs():
    """Get system logs."""
    try:
        # For now, return some mock logs or try to read from a file if it exists
        # In a real app, we'd use a log collector or read the actual log file
        logs = []
        log_file = Path("data/flasharr.log")
        if not log_file.exists():
             # Check common locations
             alt_paths = [Path("flasharr.log"), Path("/app/data/flasharr.log")]
             for p in alt_paths:
                 if p.exists():
                     log_file = p
                     break
        
        if log_file.exists():
            with open(log_file, "r") as f:
                lines = f.readlines()[-50:] # Get last 50 lines
                for line in lines:
                    parts = line.split(" - ")
                    if len(parts) >= 4:
                        logs.append({
                            "time": parts[0],
                            "level": parts[2].lower(),
                            "message": " - ".join(parts[3:]).strip()
                        })
                    else:
                        logs.append({
                            "time": "NOW",
                            "level": "info",
                            "message": line.strip()
                        })
        
        if not logs:
            logs = [
                {"time": datetime.now().strftime("%H:%M:%S"), "level": "info", "message": "Flasharr Beta system initialized."},
                {"time": datetime.now().strftime("%H:%M:%S"), "level": "success", "message": "API Backend is running and healthy."}
            ]
            
        return jsonify({"status": "ok", "logs": logs})
    except Exception as e:
        return jsonify({"status": "error", "message": str(e)}), 500
