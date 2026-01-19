"""
API Routes (AIOHTTP)

REST API endpoints for the web UI, ported to aiohttp.
"""

import logging
import time
import os
import json
import psutil
import re
from typing import Any, Dict, Optional
from datetime import datetime
from pathlib import Path
import asyncio
import asyncio

from aiohttp import web
from ..core.config import get_config
from ..utils.formatters import format_size, format_speed, format_duration, format_eta
from ..services.tmdb import tmdb_client
from ..services.smart_search import get_smart_search_service
from ..clients.timfshare import TimFshareClient
from ..utils.quality_profile import QualityParser, group_by_quality
from ..utils.normalizer import normalize_filename
from ..utils.title_matcher import calculate_unified_similarity

logger = logging.getLogger(__name__)

routes = web.RouteTableDef()

# Helper to get parsed JSON
async def get_json(request: web.Request) -> Dict:
    try:
        return await request.json()
    except Exception:
        return {}

@routes.get("/api/stats")
async def get_stats(request: web.Request) -> web.Response:
    """Get system and engine stats."""
    app = request.app
    try:
        # System uptime
        uptime = int(time.time() - psutil.boot_time())
        
        # Get primary account info
        if 'account_manager' in app:
            primary = app['account_manager'].refresh_primary_quota()
        else:
            primary = None
        
        fshare_data = {
            "valid": primary is not None,
            "premium": primary.get('premium', False) if primary else False
        }
        
        if primary:
            fshare_data['email'] = primary.get('email')
            fshare_data['traffic_left'] = primary.get('traffic_left')
            fshare_data['account_type'] = primary.get('account_type')
            # 'expiry' comes from _sanitize_account (which maps 'validuntil' -> 'expiry')
            expiry_val = primary.get('expiry')
            if expiry_val:
                if expiry_val == -1:
                    fshare_data['expiry'] = "Lifetime"
                elif isinstance(expiry_val, int):
                    try:
                        dt = datetime.fromtimestamp(expiry_val)
                        fshare_data['expiry'] = dt.strftime('%d-%m-%Y')
                    except:
                        fshare_data['expiry'] = "Unknown"
                else:
                    fshare_data['expiry'] = str(expiry_val)

        # Get downloader status from SABnzbd service if available
        sab = app.get('sabnzbd')
        downloader = sab.downloader if sab else None
        
        active_cnt = 0
        total_cnt = 0
        total_speed = 0
        connected = False

        if sab:
            counts = await sab.get_counts()
            active_cnt = counts.get("active", 0)
            queued_cnt = counts.get("queued", 0)
            completed_cnt = counts.get("completed", 0)
            total_cnt = counts.get("total", 0)
            
            # Use safe get_engine_stats if available
            if hasattr(downloader.engine, 'get_engine_stats'):
                 # Ensure proper loop handling for sync method if needed, but get_engine_stats is usually fast
                 eng_stats = downloader.engine.get_engine_stats()
                 total_speed = eng_stats.get('total_speed', 0)
            connected = True
        elif downloader:
            # Fallback direct access (sync)
            status = downloader.get_status() # Legacy sync
            total_speed = status.get("total_speed", 0)
            active_cnt = status.get("active", 0)
            queued_cnt = status.get("queued", 0)
            completed_cnt = status.get("completed", 0)
            total_cnt = status.get("total", 0)
            connected = True

        return web.json_response({
            "status": "ok",
            "system": {
                "speedtest": format_speed(total_speed),
                "uptime": uptime
            },
            "fshare_downloader": {
                "active": active_cnt,
                "queued": queued_cnt,
                "completed": completed_cnt,
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
        return web.json_response({
            "status": "ok", # Return ok to avoid UI crashing, but with error data
            "system": {"speedtest": "0 B/s", "uptime": 0},
            "pyload": {"active": 0, "connected": False, "error": str(e)}
        })


@routes.get("/api/downloads")
async def get_downloads(request: web.Request) -> web.Response:
    """Get downloads from SABnzbd Emulator (Active Queue + History)."""
    app = request.app
    try:
        sab = app.get('sabnzbd')
        if not sab:
            return web.json_response({"status": "error", "message": "Downloader not initialized"}, status=503)
        
        # Trigger sync with engine to update active tasks
        # Assuming get_queue is now thread-safe or async safe
        # In aiohttp, we run in the main loop, so we should be careful about blocking calls.
        # However, SABnzbdEmulator.get_queue IS mostly in-memory operations on the tasks dict.
        
        queue_slots = await sab.get_queue() # returns List[Dict]
        formatted = []
        
        for slot in queue_slots:
            try:
                # Normalize status
                status = slot.get('status', 'Unknown')
                
                # Normalize size/progress
                try:
                    percentage = float(slot.get('percentage', 0))
                except (ValueError, TypeError):
                    percentage = 0.0
                    
                try:
                    mb_total = float(slot.get('mb', 0))
                except (ValueError, TypeError):
                    mb_total = 0.0
                    
                item_dict = {
                    "id": slot.get('nzo_id'),
                    "nzo_id": slot.get('nzo_id'),
                    "filename": slot.get('filename'),
                    "url": slot.get('url', ''),
                    "status": status,
                    "category": slot.get('cat', slot.get('category', 'Uncategorized')),
                    "progress": percentage,
                    "size_bytes": slot.get('total_bytes', 0),
                    "mb_total": mb_total,
                    "speed": slot.get('speed_bytes', 0),
                    "eta": slot.get('eta_seconds', 0),
                    "added": slot.get('added', ''),
                    "completed": slot.get('completed', '')
                }
                
                formatted.append(_format_task(item_dict))
            except Exception as e:
                logger.error(f"Error formatting item {slot.get('nzo_id')}: {e}")
                continue
                     
        return web.json_response({
            "status": "ok",
            "count": len(formatted),
            "downloads": formatted,
        })
    except Exception as e:
        logger.error(f"Error getting downloads: {e}", exc_info=True)
        return web.json_response({"status": "error", "message": str(e)}, status=500)


def _format_task(task):
    """Helper to format a task object for the UI."""
    # Resolve size bytes
    size_raw = task.get("size_bytes") or task.get("size")
    mb_raw = task.get("mb") or task.get("mb_total")
    
    size_bytes = 0
    if size_raw and not isinstance(size_raw, str):
        size_bytes = int(size_raw)
    elif mb_raw:
        try:
            size_bytes = int(float(mb_raw) * 1024 * 1024)
        except (ValueError, TypeError):
            size_bytes = 0
            
    if isinstance(size_raw, str) and not size_bytes:
        try:
            size_bytes = int(float(size_raw))
        except:
            size_bytes = 0
    
    # Safely handle potential None values
    speed = task.get("speed", 0) or 0
    eta = task.get("eta", 0) or 0
    
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
            "formatted": format_speed(speed),
            "bytes_per_sec": speed
        },
        "eta": {
            "formatted": format_eta(eta),
            "seconds": eta
        },
        "added": task.get("added") or task.get("created_at")
    }


@routes.post("/api/downloads")
async def add_download(request: web.Request) -> web.Response:
    """Add a new download to engine."""
    app = request.app
    try:
        data = await get_json(request)
        if not data or "url" not in data:
            return web.json_response({"status": "error", "message": "URL required"}, status=400)
        
        url = data["url"]
        name = data.get("name") or ""
        if name:
            # Sanitize filename
            import re
            name = re.sub(r'[\\/*?:"<>|]', "", name) # Remove unsafe chars
            name = name.replace("..", "") # Prevent path traversal
            
        category = data.get("category", "Manual")
        
        sab = app.get('sabnzbd')
        if not sab:
            return web.json_response({"status": "error", "message": "Downloader not initialized"}, status=503)
            
        # Refined folder logic
        import re
        folder_suffix = ""
        
        # Detect Season patterns
        # S01E01, S1E1, Season 1, Season 01
        season_match = re.search(r'(?:s|season)\s*(\d{1,2})', name, re.IGNORECASE) if name else None
        if season_match:
             season_num = int(season_match.group(1))
             # Extract Series Name (everything before Sxx)
             # This is a basic heuristic
             split_point = season_match.start()
             series_name = name[:split_point].replace(".", " ").replace("-", " ").strip()
             if series_name:
                 folder_suffix = f"{series_name}/Season {season_num}"
        
        # If no series detected, use name as folder to ensure it's not loose
        if not folder_suffix:
             folder_suffix = name
             
        # Override filename to include path
        # Engine supports 'package_name' or path in filename
        # SABnzbd emulator usually takes `filename` as the job name or folder.
        # We can pass `filename` as path relative usage.
        
        # Let's clean the name for folder usage
        folder_suffix = re.sub(r'[\\:*?"<>|]', "", folder_suffix).strip()
        
        # Pass filename argument as the target folder/name structure to Emulator
        # Emulator calls `downloader.add_download(..., filename=normalized_filename, package_name=parsed.title, ...)`
        # We want to force `package_name` or `category` to handle the path?
        # Actually `add_url` takes `filename` argument.
        
        # If we pass a path-like string as filename to add_url, we need to ensure Engine respects it.
        # Engine uses `filename` as destination base if package_name is missing.
        
        # Best approach: Use the derived folder structure as the 'filename' passed to add_url, 
        # but keep the original name for logging/UI if needed.
        # Or better, we should rely on Engine's directory logic.
        
        # Let's pass the calculated folder path as the 'filename' to add_url
        nzo_id = await sab.add_url(url, filename=folder_suffix, category=category)
        
        return web.json_response({
            "status": "ok",
            "success": nzo_id is not None,
            "nzo_id": nzo_id
        })
    except Exception as e:
        logger.error(f"Error adding download: {e}")
        return web.json_response({"status": "error", "message": str(e)}, status=500)


@routes.delete("/api/downloads/{task_id}")
async def delete_download(request: web.Request) -> web.Response:
    """Delete a download."""
    task_id = request.match_info['task_id']
    sab = request.app.get('sabnzbd')
    
    try:
        if not sab:
            return web.json_response({"status": "error", "message": "Downloader not initialized"}, status=503)
            
        success = await sab.delete_item(task_id)
        if not success:
            return web.json_response({"status": "error", "message": "Item not found", "success": False}, status=404)
            
        return web.json_response({"status": "ok", "message": f"Deleted {task_id}", "success": True})
    except Exception as e:
        logger.error(f"Error deleting: {e}")
        return web.json_response({"status": "error", "message": str(e), "success": False}, status=500)

# Alias for frontend quirks
@routes.get("/api/download/delete/{task_id}")
@routes.delete("/api/download/delete/{task_id}")
async def delete_download_alias(request: web.Request) -> web.Response:
    return await delete_download(request)


@routes.post("/api/downloads/{task_id}/pause")
async def pause_download(request: web.Request) -> web.Response:
    task_id = request.match_info['task_id']
    sab = request.app.get('sabnzbd')
    try:
        if not sab:
            return web.json_response({"status": "error", "message": "Downloader not initialized"}, status=503)
            
        success = await sab.toggle_item(task_id)
        return web.json_response({"status": "ok", "message": "Paused/Toggled", "success": success})
    except Exception as e:
        return web.json_response({"status": "error", "message": str(e)}, status=500)

@routes.get("/api/download/pause/{task_id}")
@routes.post("/api/download/pause/{task_id}")
async def pause_alias(request: web.Request) -> web.Response:
    return await pause_download(request)


@routes.post("/api/downloads/{task_id}/resume")
async def resume_download(request: web.Request) -> web.Response:
    task_id = request.match_info['task_id']
    sab = request.app.get('sabnzbd')
    try:
        if not sab:
            return web.json_response({"status": "error", "message": "Downloader not initialized"}, status=503)
            
        success = await sab.toggle_item(task_id)
        return web.json_response({"status": "ok", "message": "Resumed/Toggled", "success": success})
    except Exception as e:
        return web.json_response({"status": "error", "message": str(e)}, status=500)

@routes.get("/api/download/resume/{task_id}")
@routes.post("/api/download/resume/{task_id}")
async def resume_alias(request: web.Request) -> web.Response:
    return await resume_download(request)

# /start aliases (same as resume for paused items)
@routes.post("/api/downloads/{task_id}/start")
@routes.get("/api/download/start/{task_id}")
@routes.post("/api/download/start/{task_id}")
async def start_download(request: web.Request) -> web.Response:
    return await resume_download(request)

@routes.get("/api/download/retry/{task_id}")
@routes.post("/api/download/retry/{task_id}")
async def retry_download(request: web.Request) -> web.Response:
    task_id = request.match_info['task_id']
    sab = request.app.get('sabnzbd')
    try:
        if not sab:
            return web.json_response({"status": "error", "message": "Downloader not initialized"}, status=503)
            
        success = await sab.retry_item(task_id)
        return web.json_response({"status": "ok", "message": "Retried", "success": success})
    except Exception as e:
        return web.json_response({"status": "error", "message": str(e)}, status=500)


@routes.get("/api/search")
async def api_search(request: web.Request) -> web.Response:
    """Search API."""
    app = request.app
    try:
        query = request.query.get('q', '')
        if not query:
            return web.json_response({"results": []})
            
        results = app['indexer'].client.search(query, limit=50)
        
        formatted_results = []
        for item in results:
            parsed = app['indexer'].parser.parse(item.name)
            
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
            
        return web.json_response({"results": formatted_results})
    except Exception as e:
        logger.error(f"Search API error: {e}")
        return web.json_response({"error": str(e)}, status=500)


@routes.get("/api/discovery/trending")
async def get_fshare_trending(request: web.Request) -> web.Response:
    """Fetch trending files from TimFshare as a proxy to avoid CORS."""
    import aiohttp
    try:
        async with aiohttp.ClientSession() as session:
            async with session.get('https://timfshare.com/api/key/data-top', timeout=10) as response:
                if response.status != 200:
                    return web.json_response({"results": []})
                data = await response.json()
                
                raw_items = data.get("dataFile", [])[:12]
                results = []
                for item in raw_items:
                    results.append({
                        "id": item.get("id"),
                        "name": item.get("name"),
                        "url": f"https://www.fshare.vn/file/{item.get('linkcode')}",
                        "size": int(item.get("size", 0)),
                        "quality": "Trending"
                    })
                return web.json_response({"results": results})
    except Exception as e:
        logger.error(f"Trending Proxy error: {e}")
        return web.json_response({"results": []})


@routes.get("/api/accounts")
async def list_accounts(request: web.Request) -> web.Response:
    app = request.app
    try:
        mgr = app['account_manager']
        accounts = mgr.list_accounts()
        primary = mgr.get_primary()
        return web.json_response({
            "status": "ok",
            "accounts": accounts,
            "primary": primary
        })
    except Exception as e:
        return web.json_response({"status": "error", "message": str(e)}, status=500)


@routes.post("/api/accounts/add")
async def add_account(request: web.Request) -> web.Response:
    app = request.app
    try:
        data = await get_json(request)
        email = data.get("email")
        password = data.get("password")
        if not email or not password:
            return web.json_response({"status": "error", "message": "Email and password required"}, status=400)
            
        account = app['account_manager'].add_account(email, password)
        return web.json_response({"status": "ok", "account": account})
    except Exception as e:
        return web.json_response({"status": "error", "message": str(e)}, status=500)


@routes.delete("/api/accounts/{email}")
async def remove_account(request: web.Request) -> web.Response:
    email = request.match_info['email']
    try:
        request.app['account_manager'].remove_account(email)
        return web.json_response({"status": "ok"})
    except Exception as e:
        return web.json_response({"status": "error", "message": str(e)}, status=500)


@routes.post("/api/accounts/{email}/set-primary")
async def set_primary_account(request: web.Request) -> web.Response:
    email = request.match_info['email']
    try:
        request.app['account_manager'].set_primary(email)
        return web.json_response({"status": "ok"})
    except Exception as e:
        return web.json_response({"status": "error", "message": str(e)}, status=500)


@routes.post("/api/accounts/{email}/refresh")
async def refresh_account(request: web.Request) -> web.Response:
    email = request.match_info['email']
    try:
        account = request.app['account_manager'].refresh_account(email)
        return web.json_response({"status": "ok", "account": account})
    except Exception as e:
        return web.json_response({"status": "error", "message": str(e)}, status=500)


@routes.get("/api/settings")
async def get_settings(request: web.Request) -> web.Response:
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
        return web.json_response({"status": "ok", "settings": settings})
    except Exception as e:
        return web.json_response({"status": "error", "message": str(e)}, status=500)

@routes.put("/api/settings")
async def update_settings(request: web.Request) -> web.Response:
    try:
        data = await get_json(request)
        settings_file = Path("data/settings.json")
        settings = {}
        if settings_file.exists():
            with open(settings_file, "r") as f:
                settings = json.load(f)
        
        settings.update(data)
        
        with open(settings_file, "w") as f:
            json.dump(settings, f, indent=4)
            
        if 'max_concurrent_downloads' in data:
            sab = request.app.get('sabnzbd')
            if sab:
                sab.downloader.engine.update_max_concurrent(int(data['max_concurrent_downloads']))
                
        if 'speed_limit_mbps' in data:
             sab = request.app.get('sabnzbd')
             if sab:
                limit = int(data['speed_limit_mbps']) * 1024 * 1024 if int(data['speed_limit_mbps']) > 0 else 0
                sab.downloader.engine.set_speed_limit(limit)

        return web.json_response({"status": "ok"})
    except Exception as e:
        return web.json_response({"status": "error", "message": str(e)}, status=500)


@routes.post("/api/verify-account")
async def verify_account(request: web.Request) -> web.Response:
    app = request.app
    try:
        if 'account_manager' not in app:
             return web.json_response({"status": "error", "message": "Service unavailable"}, status=503)

        mgr = app['account_manager']
        primary = mgr.get_primary()
        if not primary:
             return web.json_response({
                 "status": "error",
                 "code": "NO_ACCOUNT", 
                 "message": "No account login."
             }, status=401)

        # Refresh triggers internal re-login if needed
        # We should run this off the main loop if it does heavy I/O
        # But for now straightforward call
        account = mgr.refresh_account(primary['email'])
        
        is_valid = account is not None and 'email' in account
        
        if not is_valid:
            return web.json_response({
                "status": "error", 
                "message": "Session invalid",
                "account": account
            }, status=401)

        if account.get('validuntil'):
             if account.get('validuntil') == -1:
                 account['expiry'] = "Lifetime"
             else:
                 try:
                     dt = datetime.fromtimestamp(int(account.get('validuntil')))
                     account['expiry'] = dt.strftime('%d-%m-%Y')
                 except:
                     account['expiry'] = "Unknown"

        return web.json_response({
            "status": "ok", 
            "message": "Verified",
            "account": account
        })
    except Exception as e:
        return web.json_response({"status": "error", "message": str(e)}, status=500)

@routes.post("/api/settings/generate-api-key")
async def generate_api_key(request: web.Request) -> web.Response:
    """Generate a new random API key."""
    try:
        import secrets
        new_key = secrets.token_hex(16)
        return web.json_response({"status": "ok", "key": new_key})
    except Exception as e:
        return web.json_response({"status": "error", "message": str(e)}, status=500)


@routes.get("/api/config")
async def get_config_endpoint(request: web.Request) -> web.Response:
    try:
        config = get_config()
        return web.json_response({
            "status": "ok",
            "config": {
                "download_path": config.download.download_dir,
                "concurrent_downloads": config.download.max_concurrent,
                "worker_threads": config.download.worker_threads,
            },
        })
    except Exception as e:
         return web.json_response({"status": "error", "message": str(e)}, status=500)

@routes.post("/api/config")
async def post_config_endpoint(request: web.Request) -> web.Response:
    try:
        data = await request.json()
        config = get_config()
        
        # Update in-memory config
        if 'download_path' in data:
            config.download.download_dir = data['download_path']
        if 'concurrent_downloads' in data:
            config.download.max_concurrent = int(data['concurrent_downloads'])
        if 'worker_threads' in data:
            config.download.worker_threads = int(data['worker_threads'])
        
        # In a real app, we'd persist this to a file here
        # For now, we update the objects in memory
        
        return web.json_response({"status": "ok", "message": "Neural parameters updated"})
    except Exception as e:
        return web.json_response({"status": "error", "message": str(e)}, status=400)

@routes.get("/api/version")
async def get_version(request: web.Request) -> web.Response:
    version = "1.0.0-alpha"
    try:
        if os.path.exists("VERSION"):
            with open("VERSION", "r") as f:
                version = f.read().strip()
    except: pass
    return web.json_response({"status": "ok", "version": version})

@routes.get("/api/logs")
async def get_logs(request: web.Request) -> web.Response:
    try:
        logs = []
        log_file = Path("data/flasharr.log")
        if not log_file.exists():
             alt_paths = [Path("flasharr.log"), Path("/app/data/flasharr.log")]
             for p in alt_paths:
                 if p.exists():
                     log_file = p
                     break
        
        if log_file.exists():
            with open(log_file, "r") as f:
                lines = f.readlines()[-50:]
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
             logs = [{"time": "NOW", "level": "info", "message": "System init."}]
             
        return web.json_response({"status": "ok", "logs": logs})
    except Exception as e:
        return web.json_response({"status": "error", "message": str(e)}, status=500)
@routes.get("/api/tmdb/discover/{media_type}")
async def api_tmdb_discover(request: web.Request) -> web.Response:
    media_type = request.match_info['media_type']
    page = int(request.query.get('page', 1))
    sort_by = request.query.get('sort_by', 'popularity.desc')
    genre = request.query.get('genre')
    year = request.query.get('year')
    
    # Advanced Filters
    date_from = request.query.get('date_from')
    date_to = request.query.get('date_to')
    language = request.query.get('language')
    certification = request.query.get('certification')
    
    runtime_min = request.query.get('runtime_min')
    runtime_max = request.query.get('runtime_max')
    
    score_min = request.query.get('score_min')
    score_max = request.query.get('score_max')
    
    vote_count_min = request.query.get('vote_count_min')

    # Convert numeric params
    g_val = int(genre) if genre and genre.isdigit() else None
    y_val = int(year) if year and year.isdigit() else None
    
    rt_min = int(runtime_min) if runtime_min and runtime_min.isdigit() else None
    rt_max = int(runtime_max) if runtime_max and runtime_max.isdigit() else None
    
    s_min = float(score_min) if score_min else None
    s_max = float(score_max) if score_max else None
    
    vc_min = int(vote_count_min) if vote_count_min and vote_count_min.isdigit() else None

    try:
        if media_type == 'movie':
            data = await tmdb_client.get_discover_movies(
                page=page, sort_by=sort_by, genre=g_val, year=y_val,
                date_from=date_from, date_to=date_to, language=language,
                certification=certification, runtime_min=rt_min, runtime_max=rt_max,
                score_min=s_min, score_max=s_max, vote_count_min=vc_min
            )
        else:
            data = await tmdb_client.get_discover_tv(
                page=page, sort_by=sort_by, genre=g_val, year=y_val,
                date_from=date_from, date_to=date_to, language=language,
                certification=certification, runtime_min=rt_min, runtime_max=rt_max,
                score_min=s_min, score_max=s_max, vote_count_min=vc_min
            )
        return web.json_response(data)
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)

@routes.get("/api/tmdb/genres")
async def get_tmdb_genres(request: web.Request) -> web.Response:
    """Fetch genres for filtering."""
    media_type = request.query.get('type', 'movie')
    try:
        data = await tmdb_client.get_genres(media_type)
        return web.json_response(data)
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)

@routes.get("/api/tmdb/search")
async def api_tmdb_search(request: web.Request) -> web.Response:
    """Direct TMDB search with pagination."""
    query = request.query.get('q', '')
    page = int(request.query.get('page', 1))
    media_type = request.query.get('type', 'multi')
    if not query:
        return web.json_response({"results": []})
    try:
        data = await tmdb_client.search(query, page, media_type)
        return web.json_response(data)
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)

@routes.get("/api/tmdb/{media_type}/{tmdb_id}/similar")
async def get_tmdb_similar(request: web.Request) -> web.Response:
    media_type = request.match_info['media_type']
    tmdb_id = int(request.match_info['tmdb_id'])
    try:
        if media_type == 'movie':
            data = await tmdb_client.get_similar_movies(tmdb_id)
        else:
            data = await tmdb_client.get_similar_tv(tmdb_id)
        return web.json_response(data)
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)

@routes.get("/api/tmdb/{media_type}/{tmdb_id}/recommendations")
async def get_tmdb_recommendations(request: web.Request) -> web.Response:
    media_type = request.match_info['media_type']
    tmdb_id = int(request.match_info['tmdb_id'])
    try:
        if media_type == 'movie':
            data = await tmdb_client.get_recommendations_movies(tmdb_id)
        else:
            data = await tmdb_client.get_recommendations_tv(tmdb_id)
        return web.json_response(data)
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)


@routes.get("/api/tmdb/movie/{tmdb_id}")
async def get_tmdb_movie_detail(request: web.Request) -> web.Response:
    tmdb_id = request.match_info['tmdb_id']
    try:
        data = await tmdb_client.get_movie_details(int(tmdb_id))
        if not data:
            return web.json_response({"error": "Not found"}, status=404)
        return web.json_response(data)
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)

@routes.get("/api/tmdb/tv/{tmdb_id}")
async def get_tmdb_tv_detail(request: web.Request) -> web.Response:
    tmdb_id = request.match_info['tmdb_id']
    try:
        data = await tmdb_client.get_tv_details(int(tmdb_id))
        if not data:
            return web.json_response({"error": "Not found"}, status=404)
        return web.json_response(data)
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)

@routes.get("/api/tmdb/collection/{id}")
async def get_tmdb_collection(request: web.Request) -> web.Response:
    try:
        collection_id = int(request.match_info['id'])
        data = await tmdb_client.get_collection_details(collection_id)
        return web.json_response(data)
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)

@routes.get("/api/tmdb/tv/{tmdb_id}/season/{season_number}")
async def get_tmdb_season_detail(request: web.Request) -> web.Response:
    tmdb_id = request.match_info['tmdb_id']
    season_number = request.match_info['season_number']
    try:
        data = await tmdb_client.get_season_details(int(tmdb_id), int(season_number))
        if not data:
            return web.json_response({"error": "Not found"}, status=404)
        return web.json_response(data)
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)

@routes.post("/api/discovery/smart-search")
async def api_smart_search(request: web.Request) -> web.Response:
    try:
        data = await request.json()
        title = data.get("title")
        season = data.get("season")
        episode = data.get("episode")
        year = data.get("year")
        
        if not title:
            return web.json_response({"error": "Title required"}, status=400)
            
        service = get_smart_search_service()
        queries = service.generate_queries(title, season, episode, year)
        
        results = []
        if season and episode:
            # Episode search
            results = service.search_episode(title, season, episode)
        else:
            # Movie or Generic search
            all_results = []
            for q in queries:
                r = service.indexer.client.search(q)
                all_results.extend([res.to_dict() for res in r])
            
            # Filter duplicates and format
            seen = set()
            unique_results = []
            for r in all_results:
                if r['url'] not in seen:
                    seen.add(r['url'])
                    # Ensure size_bytes is present for frontend
                    r['size_bytes'] = r.get('size', 0)
                    unique_results.append(r)
            results = service._filter_results(unique_results)
            
        return web.json_response({
            "queries_used": queries,
            "results": results[:20]
        })
    except Exception as e:
        logger.error(f"Smart search API error: {e}")
        return web.json_response({"error": str(e)}, status=500)

@routes.get("/api/discovery/popular-today")
async def api_popular_today(request: web.Request) -> web.Response:
    """Fetch popular items from TimFshare and resolve to TMDB."""
    try:
        import aiohttp
        async with aiohttp.ClientSession() as session:
            async with session.get('https://timfshare.com/api/key/data-top') as resp:
                if resp.status != 200:
                    return web.json_response({"results": []})
                data = await resp.json()
        
        tags = data.get('dataTag', [])
        # Get top 12 names
        top_names = [t.get('name_tag') for t in tags[:12] if t.get('name_tag')]
        
        results = []
        # resolving concurrently
        import asyncio
        tasks = [tmdb_client.search(name, 1) for name in top_names]
        search_results = await asyncio.gather(*tasks, return_exceptions=True)
        
        for res in search_results:
            if isinstance(res, dict) and res.get('results'):
                # Take the first best match
                results.append(res['results'][0])
                
        # Filter duplicates just in case
        seen = set()
        unique_results = []
        for r in results:
            if r['id'] not in seen:
                seen.add(r['id'])
                unique_results.append(r)
                
        return web.json_response({"results": unique_results})
    except Exception as e:
        logger.error(f"Popular Today Error: {e}")
        return web.json_response({"error": str(e)}, status=500)

@routes.post("/api/search/smart")
async def smart_search(request: web.Request) -> web.Response:
    """
    Perform smart search with quality grouping and scoring.
    
    Body: { "title": str, "year": str, "type": "movie"|"tv", "limit": int }
    """
    try:
        data = await get_json(request)
        title = data.get('title') or data.get('query')  # Accept both parameters for backward compatibility
        year = str(data.get('year', ''))
        media_type = data.get('type', 'movie')
        season = data.get('season')
        episode = data.get('episode')
        
        if not title:
            return web.json_response({"error": "Title is required"}, status=400)
            
        # Initialize components
        client = TimFshareClient()
        parser = QualityParser()
        
        # Normalize title for search (remove special characters like ū → u)
        import unicodedata
        normalized_title = ''.join(
            c for c in unicodedata.normalize('NFD', title) 
            if unicodedata.category(c) != 'Mn'
        ) if title else title
        
        # Construct query using normalized title
        if media_type == 'movie':
            query = f"{normalized_title} {year}" if year else normalized_title
        else:
            # TV Logic
            if season and episode:
                # Specific episode search: "Title SxxExx"
                query = f"{normalized_title} S{int(season):02d}E{int(episode):02d}"
            elif season:
                # Season pack search: "Title Season X" or "Title Sxx"
                query = f"{normalized_title} Season {season}"
            else:
                # General show search
                query = f"{normalized_title}"
        
        limit = data.get('limit', 20)
        tmdb_id = data.get('tmdbId')
        
        # Use TMDB official title and alternative titles for matching
        official_title = title
        aliases = []
        
        if tmdb_id and media_type in ('movie', 'tv'):
            try:
                if media_type == 'movie':
                    movie_data = await tmdb_client.get_movie_details(tmdb_id)
                    official_title = movie_data.get('title', title)
                else:  # tv
                    tv_data = await tmdb_client.get_tv_details(tmdb_id)
                    official_title = tv_data.get('name', title)
                
                logger.info(f"Using TMDB official title: '{official_title}' (tmdbId: {tmdb_id})")
                
                # Fetch alternative titles (Vietnamese, Chinese, etc.)
                aliases = await tmdb_client.get_alternative_titles(tmdb_id, media_type)
                if aliases:
                    logger.info(f"Alternative titles: {aliases[:5]}{'...' if len(aliases) > 5 else ''}")
                    
                    # Also add normalized Vietnamese versions (without diacritics)
                    # Files are often named "Bo Bo Kinh Tam" instead of "Bộ Bộ Kinh Tâm"
                    from ..utils.title_matcher import is_vietnamese_title, normalize_vietnamese
                    normalized_aliases = []
                    for alias in aliases:
                        if is_vietnamese_title(alias):
                            normalized = normalize_vietnamese(alias)
                            if normalized not in [a.lower() for a in aliases]:
                                normalized_aliases.append(normalized)
                    if normalized_aliases:
                        aliases = list(aliases) + normalized_aliases
                        logger.info(f"Added normalized aliases: {normalized_aliases}")
                    
            except Exception as e:
                logger.warning(f"Failed to fetch TMDB data for {tmdb_id}: {e}, using user input")
                official_title = title
        
        logger.info(f"Smart Search Query: {query} (type={media_type})")
        
        # Execute primary search
        results = client.search(query, limit=100, extensions=('.mkv', '.mp4'))
        logger.info(f"Primary search returned {len(results)} results")
        
        # Dual-search: Also search with Vietnamese alias if available and different
        from ..utils.title_matcher import is_vietnamese_title, normalize_vietnamese
        vn_alias = next((a for a in aliases if is_vietnamese_title(a)), None)
        
        if vn_alias and vn_alias.lower() != official_title.lower():
            # Search with original Vietnamese (with diacritics)
            logger.info(f"Performing secondary search with Vietnamese alias: '{vn_alias}'")
            vn_results = client.search(vn_alias, limit=100, extensions=('.mkv', '.mp4'))
            logger.info(f"Vietnamese alias search returned {len(vn_results)} results")
            
            # ALSO search with normalized Vietnamese (without diacritics)
            # Files are often named "Bo Bo Kinh Tam" instead of "Bộ Bộ Kinh Tâm"
            vn_normalized = normalize_vietnamese(vn_alias)
            if vn_normalized != vn_alias.lower():
                logger.info(f"Performing normalized search: '{vn_normalized}'")
                vn_norm_results = client.search(vn_normalized, limit=100, extensions=('.mkv', '.mp4'))
                logger.info(f"Normalized search returned {len(vn_norm_results)} results")
                vn_results.extend(vn_norm_results)
            
            # Merge results, avoiding duplicates
            seen_urls = {r.url for r in results}
            for vr in vn_results:
                if vr.url not in seen_urls:
                    results.append(vr)
                    seen_urls.add(vr.url)
            
            logger.info(f"Total results after merge: {len(results)}")
        
        # ================================================================
        # SMART SNOWBALL SEARCH (TV Series Enhancement)
        # Group by filename pattern, prioritize, deep-dive for missing
        # ================================================================
        if media_type == 'tv' and len(results) > 0 and aliases:
            from ..utils.title_matcher import normalize_vietnamese
            vn_alias = next((a for a in aliases if is_vietnamese_title(a)), None)
            
            if vn_alias:
                # Step 1: Group files by pattern and extract episode numbers
                pattern_groups = {}  # pattern_template -> {eps: set, sample: str}
                
                for r in results:
                    name = r.name
                    # Extract episode number and create pattern template
                    
                    # Pattern type 1: NN_Title or NN.Title (e.g., 01_Bo Bo kinh Tam)
                    m1 = re.match(r'^(\d{1,3})([_\s.].+)$', name)
                    
                    # Pattern type 2: [Group] Title NN.ext or Title NN.ext
                    m2 = re.search(r'^(.+?)[_\s.-](\d{1,3})(\.(?:mkv|mp4))$', name)
                    
                    # Pattern type 3: SxxExx (e.g., Title.S01E01.mkv)
                    m3 = re.search(r'^(.+?)[._\s]S(\d{1,2})[Ee](\d{1,3})(.*)$', name)
                    
                    # Pattern type 4: Title.TapNN or TitleEpNN (flexible separator)
                    m4 = re.search(r'^(.+?)(?:[\s_.-]?(?:Tập|[Tt]ap|[Ee]p?))[\s_.-]*(\d{1,4})(.*)$', name)

                    if m3:  # Priority to SxxExx
                        ep = int(m3.group(3))
                        season = m3.group(2)
                        # Template: Title SxxE{ep} (ignore suffix)
                        template = f"{m3.group(1)} S{season}E{{ep}}"
                        # Base search: Title Sxx
                        base_search = f"{m3.group(1)} S{season}"
                        
                    elif m4:
                        ep = int(m4.group(2))
                        # Template: Title Tap {ep} (ignore suffix)
                        template = f"{m4.group(1)} Tap {{ep}}"
                        base_search = f"{m4.group(1)}"
                        
                    elif m1:
                        ep = int(m1.group(1))
                        template = f"{{ep}}{m1.group(2)}"
                        base_search = m1.group(2).strip('_. ')[:30]
                        
                    elif m2 and not re.search(r'^\d{3,4}p$', m2.group(2)): # Avoid resolution match like 720p
                        ep = int(m2.group(2))
                        template = f"{m2.group(1)} {{ep}}{m2.group(3)}"
                        base_search = m2.group(1).strip()[:40]
                        
                    else:
                        continue
                    
                    if 1 <= ep <= 1000:  # Valid episode range (increased for anime)
                        if template not in pattern_groups:
                            pattern_groups[template] = {'eps': set(), 'sample': name, 'base': base_search}
                        pattern_groups[template]['eps'].add(ep)
                
                # Step 2: Sort patterns by episode count (highest potential first)
                sorted_patterns = sorted(
                    pattern_groups.items(),
                    key=lambda x: len(x[1]['eps']),
                    reverse=True
                )
                
                # Step 3: Deep-dive on top 2 patterns with > 5 episodes
                for template, data in sorted_patterns[:2]:
                    if len(data['eps']) < 5:
                        continue
                    
                    # Find missing episodes (dynamic range based on found episodes)
                    found_eps = data['eps']
                    if not found_eps:
                        continue
                        
                    max_ep = max(found_eps)
                    # If it's a movie/mini-series (typically < 30), check up to 35? 
                    # But for long series, check up to max found.
                    # We'll use max(max_ep, 35) to cover typical season size minimum.
                    check_limit = max(max_ep, 35)
                    missing_eps = [ep for ep in range(1, check_limit + 1) if ep not in found_eps]
                    
                    if not missing_eps:
                        logger.info(f"Pattern complete: {template[:50]} ({len(found_eps)} eps)")
                        continue
                    
                    logger.info(f"Deep-dive: {template[:50]} - found {len(found_eps)}, missing {len(missing_eps)}")
                    
                    # Search for missing episodes using base pattern
                    base_pattern = data['base']
                    
                    # ADAPTIVE PARTITIONING STRATEGY
                    # If we are missing a LOT of episodes (> 50) or have a very long series,
                    # a single search for 'base' will likely hit the 100-item limit and fail.
                    # We partition the search space by digits: "Base 1", "Base 2"...
                    
                    is_large_series = len(missing_eps) > 50 or max_ep > 100
                    snowball_queries = []
                    
                    if is_large_series:
                        logger.info(f"Large series detected ({len(missing_eps)} missing, max {max_ep}). Using partitioned search.")
                        
                        # Phase 1: Partition by single digit (0-9) to check buckets
                        # "Naruto Tap 1" finds 1, 10-19, 100-199
                        for i in range(10):
                            q = f"{base_pattern} {i}"
                            
                            # Execute partition search
                            logger.info(f"Partition search: '{q}'")
                            p_results = client.search(q, limit=100, extensions=('.mkv', '.mp4'))
                            
                            # If saturated, we might need to drill down
                            if len(p_results) >= 95:
                                logger.info(f"Bucket '{q}' saturated ({len(p_results)}). Drilling down...")
                                for j in range(10):
                                    sub_q = f"{base_pattern} {i}{j}"
                                    snowball_queries.append(sub_q)
                            else:
                                # Add valid results from this bucket immediately
                                for sr in p_results:
                                    if sr.url not in seen_urls:
                                        results.append(sr)
                                        seen_urls.add(sr.url)
                    else:
                        # Small series - just search the base pattern
                        snowball_queries.append(base_pattern)

                    # Execute collected queries (Drill-down or Simple)
                    for query in snowball_queries:
                        logger.info(f"Snowball query: '{query}'")
                        snowball_results = client.search(query, limit=100, extensions=('.mkv', '.mp4'))
                        logger.info(f"Query returned {len(snowball_results)} results")
                        
                        # Add only new results
                        new_count = 0
                        for sr in snowball_results:
                            if sr.url not in seen_urls:
                                results.append(sr)
                                seen_urls.add(sr.url)
                                new_count += 1
                        
                        if new_count > 0:
                            logger.info(f"Added {new_count} new results")
                
                logger.info(f"Total results after smart snowball: {len(results)}")
        
        # Process results
        valid_results = []
        for r in results:
            r_dict = r.to_dict()
            
            # Unified similarity check with franchise detection and alias support
            # Uses keyword-based matching requiring ALL search keywords present
            # Also checks against alternative titles (Vietnamese, etc.)
            sim_result = calculate_unified_similarity(official_title, r.name, aliases=aliases)
            
            if not sim_result['is_valid']:
                logger.debug(f"Rejected ({sim_result['match_type']}, {sim_result['score']:.2f}): {r.name[:60]}")
                continue
            
            # STRICT TMDB FILTERING: When TMDB ID provided, only accept alias matches
            # This prevents wrong shows (e.g., Korean "Scarlet Heart Ryeo") from appearing
            # Note: Check for both None and string "null" (from JSON payload)
            if tmdb_id and tmdb_id != "null" and sim_result['match_type'] != 'alias':
                logger.debug(f"Rejected non-alias match (tmdbId={tmdb_id}): {r.name[:60]}")
                continue

            # Relaxed Year Filter (Movies Only)
            # Only skip if file explicitly has a DIFFERENT year
            if year and media_type == 'movie':
                # Find year in filename: 19xx or 20xx
                y_match = re.search(r'\b(19|20)\d{2}\b', r.name)
                if y_match:
                    file_year = y_match.group(0)  # Get full year (e.g., "2018")
                    # If file has a year and it doesn't match requested year -> SKIP
                    # Allow +/- 1 year tolerance for release date variations
                    try:
                        if abs(int(file_year) - int(year)) > 1:
                            logger.debug(f"Skipping {r.name[:50]} - year mismatch: {file_year} vs {year}")
                            continue
                    except ValueError:
                        pass  # If year parsing fails, include the file
                
            # Profile quality
            profile = parser.parse(r.name, r.size)
            
            # Merge profile data into result
            r_dict.update(profile.to_dict())
            r_dict['similarity'] = sim_result['score']
            r_dict['match_type'] = sim_result['match_type']
            
            # Extract Season/Episode info for TV
            if media_type == 'tv':
                s_match = re.search(r'\bS(\d{1,3})', r.name, re.IGNORECASE)
                e_match = re.search(r'\bE(\d{1,4})', r.name, re.IGNORECASE)
                
                # Check for "Season X" pattern
                if not s_match:
                    season_match = re.search(r'\bSeason\s*(\d{1,3})\b', r.name, re.IGNORECASE)
                    if season_match:
                        r_dict['season_number'] = int(season_match.group(1))
                else:
                    r_dict['season_number'] = int(s_match.group(1))
                    
                if e_match:
                    r_dict['episode_number'] = int(e_match.group(1))
                    
            valid_results.append(r_dict)
            
        logger.info(f"Valid results after filtering: {len(valid_results)}")
            
        # TV Specific Grouping
        if media_type == 'tv':
            seasons = {}
            for res in valid_results:
                s_num = res.get('season_number', 0) # 0 = Specials or Unknown
                if s_num not in seasons:
                    seasons[s_num] = {'packs': [], 'episodes': []}
                
                # Determine if Pack or Episode
                # Pack: Has Season, No Episode (or explicit "Complete")
                is_episode = 'episode_number' in res
                
                if is_episode:
                    seasons[s_num]['episodes'].append(res)
                else:
                    seasons[s_num]['packs'].append(res)
                    
            # Format output for frontend with metadata
            sorted_seasons = []
            
            # 1. Fetch metadata if tmdb_id is present (concurrently)
            # tmdb_client is already imported globally
            season_meta_map = {} # s_num -> {ep_num: metadata}
            
            if tmdb_id:
                valid_season_nums = sorted([s for s in seasons.keys() if s > 0])
                if valid_season_nums:
                    try:
                        tasks = [tmdb_client.get_season_details(tmdb_id, s) for s in valid_season_nums]
                        meta_results = await asyncio.gather(*tasks, return_exceptions=True)
                        
                        for s_num, res in zip(valid_season_nums, meta_results):
                            if isinstance(res, dict) and 'episodes' in res:
                                season_meta_map[s_num] = {
                                    ep['episode_number']: {
                                        'name': ep.get('name', ''),
                                        'overview': ep.get('overview', ''),
                                        'air_date': ep.get('air_date', ''),
                                        'still_path': ep.get('still_path', '')
                                    }
                                    for ep in res['episodes']
                                }
                    except Exception as e:
                        logger.error(f"Failed to fetch season metadata: {e}")

            # 2. Build final response structure
            for s_num in sorted(seasons.keys()):
                pk = seasons[s_num]['packs']
                ep_files = seasons[s_num]['episodes']
                
                if not pk and not ep_files:
                    continue
                    
                # Sort packs by score
                pk.sort(key=lambda x: x['total_score'], reverse=True)
                
                # Group episodes by episode number
                grouped_eps = {}
                for f in ep_files:
                    ep_num = f.get('episode_number', 0)
                    if ep_num not in grouped_eps:
                        grouped_eps[ep_num] = []
                    grouped_eps[ep_num].append(f)
                
                # Create detailed episode objects with metadata
                final_episodes = []
                for ep_num in sorted(grouped_eps.keys()):
                    # Get metadata for this episode
                    meta = season_meta_map.get(s_num, {}).get(ep_num, {})
                    
                    # Sort files in this episode by score
                    files = grouped_eps[ep_num]
                    files.sort(key=lambda x: x['total_score'], reverse=True)
                    
                    final_episodes.append({
                        'episode_number': ep_num,
                        'name': meta.get('name', f'Episode {ep_num}'),
                        'overview': meta.get('overview', 'No overview available.'),
                        'air_date': meta.get('air_date', ''),
                        'still_path': meta.get('still_path', ''),
                        'files': files
                    })
                
                sorted_seasons.append({
                    "season": s_num,
                    "packs": pk,
                    "episodes_grouped": final_episodes
                })
                
            return web.json_response({
                "query": query,
                "total_found": len(valid_results),
                "type": "tv",
                "seasons": sorted_seasons
            })

        # Movie Grouping (legacy quality grouping)
        groups = group_by_quality(valid_results)
        logger.info(f"Groups created: {len(groups)} groups from {len(valid_results)} results")
        
        # Format response: Sort groups by highest score
        sorted_groups = []
        for qname, items in groups.items():
            if not items:
                continue
            
            # Sort items in group by total_score
            items.sort(key=lambda x: x['total_score'], reverse=True)
            
            # Group score is max score of items
            group_score = items[0]['total_score']
            
            sorted_groups.append({
                "quality": qname,
                "score": group_score,
                "count": len(items),
                "files": items
            })
            
        # Sort groups descending by score
        sorted_groups.sort(key=lambda x: x['score'], reverse=True)
        
        logger.info(f"Returning {len(sorted_groups)} sorted groups with {len(valid_results)} total results")
        
        return web.json_response({
            "query": query,
            "total_found": len(valid_results),
            "type": "movie",
            "groups": sorted_groups
        })

        
    except Exception as e:
        logger.error(f"Smart Search Error: {e}", exc_info=True)
        return web.json_response({"error": str(e)}, status=500)
