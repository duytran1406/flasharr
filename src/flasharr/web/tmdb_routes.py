"""
TMDB API Routes (Flask)

Endpoints for TMDB discovery/search.
"""

import logging
from flask import Blueprint, request, jsonify
from ..services.tmdb import tmdb_client
import asyncio

logger = logging.getLogger(__name__)

tmdb_bp = Blueprint("tmdb", __name__)

# Async helper
def run_async(coro):
    try:
        loop = asyncio.get_event_loop()
    except RuntimeError:
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
    return loop.run_until_complete(coro)


@tmdb_bp.route("/search/<media_type>", methods=["GET"])
def search_media(media_type):
    """
    Search for movies or TV shows.
    """
    query = request.args.get('q', '')
    page = int(request.args.get('page', 1))
    
    if not query:
        return jsonify({"results": [], "page": 1, "total_pages": 0})
    
    # Use tmdb_client instance
    try:
        # Client search method in services/tmdb.py uses 'search' for multi-search?
        # Step 201 shows: async def search(self, query: str, page: int = 1) -> Dict[str, Any]: return ... /search/multi
        # But we want specific media type probably?
        # The service doesn't have specific search_movie/tv methods exposed in Step 201 clearly?
        # Wait, Step 201 ONLY showed get_discover_movies/tv and search (multi) and details.
        # It did NOT show search_movie/search_tv.
        # So we might fallback to multi search or implement them.
        # For now let's use search (multi) and filter if possible or just use it.
        # Actually TMDB search/multi returns media_type field.
        
        data = run_async(tmdb_client.search(query, page))
    except Exception as e:
        return jsonify({"error": str(e)}), 500

    if not data:
        return jsonify({"error": "Failed to communicate with TMDB"}), 502

    # Format results
    results = []
    for item in data.get("results", []):
        mt = item.get("media_type", media_type) 
        # If multi search, mt might be person/movie/tv. 
        # If we requested /search/movie, we probably want to filter or use specific endpoint?
        # Given service limitations, let's filter results if media_type is specific.
        if media_type != 'multi' and mt != media_type:
            continue
        if mt not in ["movie", "tv"]:
            continue
        
        results.append({
            "id": item.get("id"),
            "media_type": mt,
            "title": item.get("title") if mt == "movie" else item.get("name"),
            "original_title": item.get("original_title") if mt == "movie" else item.get("original_name"),
            "overview": item.get("overview"),
            "release_date": item.get("release_date") if mt == "movie" else item.get("first_air_date"),
            "poster_url": tmdb_client.get_poster_url(item.get("poster_path")),
            "backdrop_url": f"https://image.tmdb.org/t/p/w1280{item.get('backdrop_path')}" if item.get('backdrop_path') else None,
            "score": item.get("vote_average"),
            "vote_count": item.get("vote_count"),
            "genres": item.get("genre_ids", [])
        })
    
    return jsonify({
        "results": results,
        "page": data.get("page", 1),
        "total_pages": data.get("total_pages", 0)
    })


@tmdb_bp.route("/discover/<media_type>", methods=["GET"])
def discover_media(media_type):
    """
    Discover movies or TV shows with filters.
    """
    page = int(request.args.get('page', 1))
    sort_by = request.args.get('sort_by', 'popularity.desc')
    
    # Process filters
    filters = {
        'page': page,
        'sort_by': sort_by,
        'genre': int(request.args.get('genre')) if request.args.get('genre') else None,
        'year': int(request.args.get('year')) if request.args.get('year') else None,
        'date_from': request.args.get('date_from'),
        'date_to': request.args.get('date_to'),
        'language': request.args.get('language'),
        'certification': request.args.get('certification'),
        'runtime_min': int(request.args.get('runtime_min')) if request.args.get('runtime_min') else None,
        'runtime_max': int(request.args.get('runtime_max')) if request.args.get('runtime_max') else None,
        'score_min': float(request.args.get('score_min')) if request.args.get('score_min') else None,
        'score_max': float(request.args.get('score_max')) if request.args.get('score_max') else None,
        'vote_count_min': int(request.args.get('vote_count_min')) if request.args.get('vote_count_min') else None
    }

    try:
        if media_type == 'movie':
            data = run_async(tmdb_client.get_discover_movies(**filters))
        elif media_type == 'tv':
            data = run_async(tmdb_client.get_discover_tv(**filters))
        else:
            return jsonify({"error": "Invalid media_type"}), 400
            
        if not data:
             return jsonify({"error": "Failed to communicate with TMDB"}), 502
             
        # Format results
        results = []
        for item in data.get("results", []):
            results.append({
                "id": item.get("id"),
                "media_type": media_type,
                "title": item.get("title") if media_type == "movie" else item.get("name"),
                "original_title": item.get("original_title") if media_type == "movie" else item.get("original_name"),
                "overview": item.get("overview"),
                "release_date": item.get("release_date") if media_type == "movie" else item.get("first_air_date"),
                "poster_url": tmdb_client.get_poster_url(item.get("poster_path")),
                "backdrop_url": f"https://image.tmdb.org/t/p/w1280{item.get('backdrop_path')}" if item.get('backdrop_path') else None,
                "score": item.get("vote_average"),
                "vote_count": item.get("vote_count"),
                "genres": item.get("genre_ids", []),
                "popularity": item.get("popularity")
            })

        return jsonify({
            "results": results,
            "page": data.get("page", 1),
            "total_pages": data.get("total_pages", 0),
            "total_results": data.get("total_results", 0)
        })

    except Exception as e:
        logger.error(f"Discover error: {e}")
        return jsonify({"error": str(e)}), 500


@tmdb_bp.route("/trending/<media_type>/<time_window>", methods=["GET"])
def get_trending(media_type, time_window):
    """
    Get trending items.
    """
    page = int(request.args.get('page', 1))
    
    try:
        data = run_async(tmdb_client.get_trending(media_type, time_window, page=page))
        if not data:
            return jsonify({"error": "Failed to communicate with TMDB"}), 502
            
        # Format results
        results = []
        for item in data.get("results", []):
            mt = item.get("media_type", media_type)
            if mt not in ["movie", "tv"]:
                continue
            
            results.append({
                "id": item.get("id"),
                "media_type": mt,
                "title": item.get("title") if mt == "movie" else item.get("name"),
                "overview": item.get("overview"),
                "release_date": item.get("release_date") if mt == "movie" else item.get("first_air_date"),
                "poster_url": tmdb_client.get_poster_url(item.get("poster_path")),
                "score": item.get("vote_average"),
                "vote_count": item.get("vote_count"),
                "popularity": item.get("popularity")
            })
        
        return jsonify({"results": results})
    except Exception as e:
        return jsonify({"error": str(e)}), 500


@tmdb_bp.route("/genres", methods=["GET"])
def get_genres():
    """Get list of genres for movies or TV."""
    media_type = request.args.get('type', 'movie') # Support query param
    
    try:
        data = run_async(tmdb_client.get_genres(media_type))
        if not data:
            return jsonify({"error": "Failed to communicate with TMDB"}), 502
        return jsonify({"genres": data.get("genres", [])})
    except Exception as e:
        return jsonify({"error": str(e)}), 500


@tmdb_bp.route("/movie/<int:tmdb_id>", methods=["GET"])
def get_movie_details(tmdb_id):
    """Get movie details."""
    try:
        data = run_async(tmdb_client.get_movie_details(tmdb_id))
        if not data:
            return jsonify({"error": "Not found"}), 404
            
        if data.get("poster_path"):
            data["poster_url"] = tmdb_client.get_poster_url(data["poster_path"], "w500")
        if data.get("backdrop_path"):
             data["backdrop_url"] = f"https://image.tmdb.org/t/p/w1280{data['backdrop_path']}"
             
        return jsonify(data)
    except Exception as e:
        return jsonify({"error": str(e)}), 500

@tmdb_bp.route("/tv/<int:tmdb_id>", methods=["GET"])
def get_tv_details(tmdb_id):
    """Get TV details."""
    try:
        data = run_async(tmdb_client.get_tv_details(tmdb_id))
        if not data:
            return jsonify({"error": "Not found"}), 404
            
        if data.get("poster_path"):
            data["poster_url"] = tmdb_client.get_poster_url(data["poster_path"], "w500")
        if data.get("backdrop_path"):
             data["backdrop_url"] = f"https://image.tmdb.org/t/p/w1280{data['backdrop_path']}"
             
        return jsonify(data)
    except Exception as e:
        return jsonify({"error": str(e)}), 500
