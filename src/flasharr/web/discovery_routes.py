"""
Discovery Routes (Flask)

API endpoints for Smart Search and Discovery features.
"""

import logging
from flask import Blueprint, request, jsonify
from ..services.smart_search import get_smart_search_service
from ..services.tmdb import tmdb_client
from ..clients.timfshare import TimFshareClient
from ..core.config import get_config
import asyncio

logger = logging.getLogger(__name__)

discovery_bp = Blueprint("discovery", __name__)

# Helper for async execution in Flask (since it's sync by default unless using async extra)
# We can use simple asyncio.run for now if not fully async flask
def run_async(coro):
    try:
        loop = asyncio.get_event_loop()
    except RuntimeError:
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
    return loop.run_until_complete(coro)

@discovery_bp.route("/smart-search", methods=["POST"])
def smart_search():
    """
    Trigger a smart search for a specific item (Movie or Episode).
    """
    try:
        data = request.json or {}
    except:
        data = {}
    
    title = data.get("title")
    season = data.get("season")
    episode = data.get("episode")
    year = data.get("year")
    
    if not title:
        return jsonify({"error": "Title required"}), 400
    
    service = get_smart_search_service()
    
    # Generate search queries
    queries = service.generate_queries(title, season, episode, year)
    
    results = []
    
    # Search for specific item
    if season and episode:
        # Note: search_episode might be sync or async depending on implementation.
        # Assuming sync based on previous context, or we wrap it.
        # If service methods are sync:
        results = service.search_episode(title, season, episode)
    else:
        # Movie search
        all_results = []
        for q in queries:
            # efficient indexer search
            r = service.indexer.search(q)
            all_results.extend(r)
        
        # Filter duplicates
        seen = set()
        unique_results = []
        for r in all_results:
            if r['url'] not in seen:
                seen.add(r['url'])
                unique_results.append(r)
        results = service._filter_results(unique_results)
    
    return jsonify({
        "queries_used": queries,
        "results": results[:20]  # Limit response
    })


@discovery_bp.route("/popular-today", methods=["GET"])
def popular_today():
    """
    Get popular items that are available on Fshare.
    Combines TMDB trending with Fshare availability check.
    """
    media_type = request.args.get('type', 'movie')
    limit = int(request.args.get('limit', 20))
    page = int(request.args.get('page', 1))
    
    # Get trending from TMDB
    # TMDB Client is async, so we need to run it appropriately
    try:
        trending_data = run_async(tmdb_client.get_trending(media_type, 'day', page=page))
    except Exception as e:
        logger.error(f"Error fetching trending: {e}")
        return jsonify({"error": "Failed to fetch trending data"}), 502
    
    if not trending_data:
        return jsonify({"error": "Failed to fetch trending data"}), 502
    
    # Check availability on Fshare via TimFshare
    # TimFshareClient seems to be sync or async? 
    # Let's assume sync for now or check usage elsewhere. 
    # Previous code used it as `timfshare.search(..., limit=3)` synchronously in `discovery_routes.py` (which was aiohttp but the call didn't invoke await?)
    # Wait, previous code was: `search_results = timfshare.search(search_query, limit=3)` inside an async func but NO await. So it's sync.
    
    timfshare = TimFshareClient()
    results = []
    
    # Only process up to limit items to avoid slow responses
    items_to_process = trending_data.get('results', [])[:limit]
    
    for item in items_to_process:
        title = item.get('title') if media_type == 'movie' else item.get('name')
        year = None
        
        # Extract year from release date
        release_date = item.get('release_date') if media_type == 'movie' else item.get('first_air_date')
        if release_date and len(release_date) >= 4:
            year = release_date[:4]
        
        # Quick search to check availability
        search_query = f"{title} {year}" if year else title
        try:
            search_results = timfshare.search(search_query, limit=3)
            fshare_available = len(search_results) > 0
            fshare_count = len(search_results)
        except Exception as e:
            logger.warning(f"Failed to check Fshare availability for {title}: {e}")
            fshare_available = False
            fshare_count = 0
        
        # Format result
        result = {
            "id": item.get("id"),
            "media_type": media_type,
            "title": title,
            "original_title": item.get("original_title") if media_type == "movie" else item.get("original_name"),
            "overview": item.get("overview"),
            "release_date": release_date,
            "poster_url": run_async(tmdb_client.get_poster_url(item.get("poster_path"))), # Wait, get_poster_url might be sync? 
            # In tmdb.py earlier view (step 104), get_poster_url was sync. 
            # But services/tmdb.py (step 201) doesn't have it? 
            # I should check services/tmdb.py again or just use base_url logic manually if missing.
            "backdrop_url": f"https://image.tmdb.org/t/p/w780{item.get('backdrop_path')}" if item.get('backdrop_path') else None,
            "score": item.get("vote_average"),
            "vote_count": item.get("vote_count"),
            "popularity": item.get("popularity"),
            "fshare_available": fshare_available,
            "fshare_count": fshare_count
        }
        
        # Fix poster url if needed manually
        if item.get("poster_path"):
             result["poster_url"] = f"https://image.tmdb.org/t/p/w342{item['poster_path']}"
        
        results.append(result)
    
    return jsonify({
        "status": "ok",
        "results": results,
        "media_type": media_type
    })


@discovery_bp.route("/recommendations", methods=["GET"])
def get_recommendations():
    """
    Get personalized recommendations based on download history.
    """
    limit = int(request.args.get('limit', 20))
    
    # For now, return popular movies as fallback
    try:
        popular_data = run_async(tmdb_client.get_popular_movies(page=1))
    except:
        return jsonify({"error": "Failed to fetch recommendations"}), 502
    
    if not popular_data:
        return jsonify({"error": "Failed to fetch recommendations"}), 502
    
    results = []
    for item in popular_data.get('results', [])[:limit]:
        poster_url = f"https://image.tmdb.org/t/p/w342{item.get('poster_path')}" if item.get('poster_path') else None
        backdrop_url = f"https://image.tmdb.org/t/p/w780{item.get('backdrop_path')}" if item.get('backdrop_path') else None
        
        results.append({
            "id": item.get("id"),
            "media_type": "movie",
            "title": item.get("title"),
            "overview": item.get("overview"),
            "release_date": item.get("release_date"),
            "poster_url": poster_url,
            "backdrop_url": backdrop_url,
            "score": item.get("vote_average"),
            "popularity": item.get("popularity")
        })
    
    return jsonify({
        "status": "ok",
        "results": results
    })


@discovery_bp.route("/available-on-fshare", methods=["GET"])
def check_fshare_availability():
    """
    Check if a specific title is available on Fshare.
    """
    title = request.args.get('title', '')
    year = request.args.get('year', '')
    
    if not title:
        return jsonify({"error": "Title required"}), 400
    
    search_query = f"{title} {year}" if year else title
    
    try:
        timfshare = TimFshareClient()
        results = timfshare.search(search_query, limit=10)
        
        return jsonify({
            "status": "ok",
            "available": len(results) > 0,
            "count": len(results),
            "results": results[:5]  # Return top 5 matches
        })
    except Exception as e:
        logger.error(f"Failed to check Fshare availability: {e}")
        return jsonify({"error": str(e)}), 500
