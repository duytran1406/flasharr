"""
Indexer Routes Blueprint

Implements Newznab/Torznab-compatible API for *arr suite integration.
"""

import logging
from flask import Blueprint, request, Response

from ..services.indexer import IndexerService

logger = logging.getLogger(__name__)

indexer_bp = Blueprint("indexer", __name__)

# Lazy-loaded indexer service
_indexer: IndexerService = None


def get_indexer() -> IndexerService:
    """Get or create indexer service instance."""
    global _indexer
    if _indexer is None:
        _indexer = IndexerService()
    return _indexer


@indexer_bp.route("/indexer/api", methods=["GET"])
def api_endpoint():
    """
    Main Newznab/Torznab API endpoint.
    
    Query params:
        - t: Request type (caps, search, tvsearch, movie)
        - q: Search query
        - season: Season number (for tvsearch)
        - ep: Episode number (for tvsearch)
        - apikey: API key (ignored, for compatibility)
    """
    t = request.args.get("t", "")
    indexer = get_indexer()
    
    if t == "caps":
        # Capabilities request
        response = indexer.get_capabilities()
        return Response(response.xml, mimetype="application/xml")
    
    elif t in ("search", "tvsearch", "movie"):
        # Search request
        query = request.args.get("q", "")
        season = request.args.get("season")
        episode = request.args.get("ep")
        limit = request.args.get("limit", type=int)
        
        if not query:
            # Return empty results
            response = indexer.search("", limit=0)
            return Response(response.xml, mimetype="application/xml")
        
        response = indexer.search(query, season, episode, limit)
        return Response(response.xml, mimetype="application/xml")
    
    else:
        # Unknown request type
        error_xml = '<?xml version="1.0" encoding="UTF-8"?><error>Unknown request type</error>'
        return Response(error_xml, mimetype="application/xml", status=400)


@indexer_bp.route("/nzb/<guid>", methods=["GET"])
def get_nzb(guid: str):
    """
    Generate NZB file for a result.
    
    The NZB contains metadata for the SABnzbd emulator to process.
    """
    indexer = get_indexer()
    nzb_content = indexer.get_nzb(guid)
    
    if nzb_content:
        return Response(
            nzb_content,
            mimetype="application/x-nzb",
            headers={"Content-Disposition": f"attachment; filename={guid}.nzb"},
        )
    
    return Response("NZB not found", status=404)
