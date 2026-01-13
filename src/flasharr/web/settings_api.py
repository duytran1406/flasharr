"""
Settings API Blueprint

REST API endpoints for settings management.
"""

import logging
import secrets
from flask import Blueprint, jsonify, request

from ..core.settings_store import get_settings_store, AppSettings
from ..clients.fshare import FshareClient
from ..core.exceptions import AuthenticationError

logger = logging.getLogger(__name__)

# Note: This blueprint is registered with url_prefix='/api/settings' in app.py
settings_bp = Blueprint("settings", __name__)


@settings_bp.route("/", methods=["GET"])
def get_settings():
    """
    Get all application settings.
    
    Returns JSON with all settings (passwords masked).
    """
    try:
        store = get_settings_store()
        settings = store.get_app_settings()
        data = settings.to_dict()
        
        # Mask password
        if data.get("fshare_password"):
            data["fshare_password"] = "••••••••"
        
        return jsonify({
            "status": "ok",
            "settings": data,
        })
    except Exception as e:
        logger.error(f"Error getting settings: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@settings_bp.route("/", methods=["PUT"])
def update_settings():
    """
    Update application settings.
    
    Request body: Partial settings object
    """
    try:
        data = request.get_json()
        if not data:
            return jsonify({"status": "error", "message": "No data provided"}), 400
        
        store = get_settings_store()
        current = store.get_app_settings()
        
        # Update each provided setting
        for key, value in data.items():
            if hasattr(current, key):
                # Skip masked password
                if key == "fshare_password" and value == "••••••••":
                    continue
                setattr(current, key, value)
        
        store.save_app_settings(current)
        
        # Propagate settings to active engine
        try:
            from flask import current_app
            if hasattr(current_app, "sabnzbd") and current_app.sabnzbd:
                engine = current_app.sabnzbd.downloader.engine
                
                # Update Max Concurrent
                engine.update_max_concurrent(current.max_concurrent_downloads)
                
                # Update Speed Limit (MB/s to B/s)
                limit_mbps = current.speed_limit_mbps
                limit_bytes = int(limit_mbps) * 1024 * 1024 if limit_mbps > 0 else None
                engine.set_speed_limit(limit_bytes)
        except Exception as e:
            logger.error(f"Failed to update active engine settings: {e}")
        
        logger.info("Settings updated successfully")
        return jsonify({
            "status": "ok",
            "message": "Settings saved",
        })
    except Exception as e:
        logger.error(f"Error updating settings: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@settings_bp.route("/test-fshare", methods=["POST"])
def test_fshare_connection():
    """
    Test Fshare connection with provided or stored credentials.
    
    Request body (optional):
        - email: Test email
        - password: Test password
    """
    try:
        data = request.get_json() or {}
        store = get_settings_store()
        settings = store.get_app_settings()
        
        # Use provided credentials or stored ones
        email = data.get("email") or settings.fshare_email
        password = data.get("password")
        
        # If password is masked, use stored one
        if not password or password == "••••••••":
            password = settings.fshare_password
        
        if not email or not password:
            return jsonify({
                "status": "error",
                "message": "Email and password required",
            }), 400
        
        # Test connection
        try:
            client = FshareClient(email=email, password=password)
            success = client.login()
            
            if success:
                return jsonify({
                    "status": "ok",
                    "message": "Connection successful",
                    "account": {
                        "email": email,
                        "authenticated": True,
                    },
                })
            else:
                return jsonify({
                    "status": "error",
                    "message": "Login failed",
                }), 401
                
        except AuthenticationError as e:
            return jsonify({
                "status": "error",
                "message": str(e),
            }), 401
            
    except Exception as e:
        logger.error(f"Error testing Fshare: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@settings_bp.route("/generate-api-key", methods=["POST"])
def generate_api_key():
    """
    Generate a new API key.
    
    Request body:
        - type: "indexer" or "sabnzbd"
    """
    try:
        data = request.get_json() or {}
        key_type = data.get("type", "indexer")
        
        # Generate secure random key
        new_key = secrets.token_urlsafe(32)
        
        store = get_settings_store()
        settings = store.get_app_settings()
        
        if key_type == "indexer":
            settings.indexer_api_key = new_key
        elif key_type == "sabnzbd":
            settings.sabnzbd_api_key = new_key
        else:
            return jsonify({
                "status": "error",
                "message": "Invalid key type",
            }), 400
        
        store.save_app_settings(settings)
        
        return jsonify({
            "status": "ok",
            "key": new_key,
            "type": key_type,
        })
    except Exception as e:
        logger.error(f"Error generating API key: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@settings_bp.route("/export", methods=["GET"])
def export_settings():
    """Export all settings as JSON."""
    try:
        store = get_settings_store()
        json_data = store.export_json()
        
        return jsonify({
            "status": "ok",
            "data": json_data,
        })
    except Exception as e:
        logger.error(f"Error exporting settings: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@settings_bp.route("/import", methods=["POST"])
def import_settings():
    """Import settings from JSON."""
    try:
        data = request.get_json()
        if not data or "data" not in data:
            return jsonify({
                "status": "error",
                "message": "No data provided",
            }), 400
        
        store = get_settings_store()
        success = store.import_json(data["data"])
        
        if success:
            return jsonify({
                "status": "ok",
                "message": "Settings imported",
            })
        else:
            return jsonify({
                "status": "error",
                "message": "Invalid JSON format",
            }), 400
            
    except Exception as e:
        logger.error(f"Error importing settings: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500


@settings_bp.route("/reset", methods=["POST"])
def reset_settings():
    """Reset all settings to defaults."""
    try:
        store = get_settings_store()
        defaults = AppSettings()
        store.save_app_settings(defaults)
        
        return jsonify({
            "status": "ok",
            "message": "Settings reset to defaults",
        })
    except Exception as e:
        logger.error(f"Error resetting settings: {e}")
        return jsonify({"status": "error", "message": str(e)}), 500
