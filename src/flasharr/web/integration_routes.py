"""
Integration Routes

API endpoints for Sonarr/Radarr integration key features.
"""

from flask import Blueprint, jsonify, request
import logging

from ..services.integration_service import get_integration_service

logger = logging.getLogger(__name__)

integration_bp = Blueprint("integration", __name__)


@integration_bp.route("/integration/status", methods=["GET"])
def get_status():
    """Get connection status of integrations."""
    service = get_integration_service()
    return jsonify(service.get_status())


# --- Sonarr Endpoints ---

@integration_bp.route("/integration/sonarr/profiles", methods=["GET"])
def get_sonarr_profiles():
    service = get_integration_service()
    return jsonify(service.get_sonarr_profiles())


@integration_bp.route("/integration/sonarr/folders", methods=["GET"])
def get_sonarr_folders():
    service = get_integration_service()
    return jsonify(service.get_sonarr_folders())


@integration_bp.route("/integration/sonarr/add", methods=["POST"])
def add_series():
    """Add series to Sonarr."""
    data = request.json
    tvdb_id = data.get("tvdb_id")
    profile_id = data.get("quality_profile_id")
    folder = data.get("root_folder_path")
    search = data.get("search", True)
    
    if not tvdb_id:
        return jsonify({"error": "Missing tvdb_id"}), 400
        
    service = get_integration_service()
    success, message = service.add_series(tvdb_id, profile_id, folder, search)
    
    if success:
        return jsonify({"message": message})
    return jsonify({"error": message}), 500


# --- Radarr Endpoints ---

@integration_bp.route("/integration/radarr/profiles", methods=["GET"])
def get_radarr_profiles():
    service = get_integration_service()
    return jsonify(service.get_radarr_profiles())


@integration_bp.route("/integration/radarr/folders", methods=["GET"])
def get_radarr_folders():
    service = get_integration_service()
    return jsonify(service.get_radarr_folders())


@integration_bp.route("/integration/radarr/add", methods=["POST"])
def add_movie():
    """Add movie to Radarr."""
    data = request.json
    tmdb_id = data.get("tmdb_id")
    profile_id = data.get("quality_profile_id")
    folder = data.get("root_folder_path")
    search = data.get("search", True)
    
    if not tmdb_id:
        return jsonify({"error": "Missing tmdb_id"}), 400
        
    service = get_integration_service()
    success, message = service.add_movie(tmdb_id, profile_id, folder, search)
    
    if success:
        return jsonify({"message": message})
    return jsonify({"error": message}), 500
