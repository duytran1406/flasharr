"""
TMDB API Routes (AIOHTTP)

Endpoints for TMDB discovery/search.
"""

import logging
from aiohttp import web
from ..services.tmdb import tmdb_client

logger = logging.getLogger(__name__)

routes = web.RouteTableDef()

@routes.get("/api/tmdb/search/{media_type}")
async def search_media(request: web.Request) -> web.Response:
    """
    Search for movies or TV shows.
    """
    media_type = request.match_info['media_type']
    query = request.query.get('q', '')
    page = int(request.query.get('page', 1))
    
    if not query:
        return web.json_response({"results": [], "page": 1, "total_pages": 0})
    
    try:
        # Note: services/tmdb.py search is essentially multi-search or we can fallback
        # If media_type is specific, we might want to filter results if search is multi
        # or implement specific search in service.
        # For now, let's use search (multi) and filtered as we did in Flask attempt,
        # OR better: use discover with query if possible? No, search is search.
        
        # Checking services/tmdb.py capabilities again (mental model):
        # We saw get_discover_movies/tv, search(multi). 
        # We did NOT see search_movie/tv in the file content we updated.
        data = await tmdb_client.search(query, page)
        
        if not data:
            return web.json_response({"error": "Failed to communicate with TMDB"}, status=502)
        
        # Format results
        results = []
        for item in data.get("results", []):
            mt = item.get("media_type", media_type)
            if media_type != 'multi' and mt != media_type:
                 # If user asked for 'movie' but result is 'tv', skip it
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
        
        return web.json_response({
            "results": results,
            "page": data.get("page", 1),
            "total_pages": data.get("total_pages", 0)
        })
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)


@routes.get("/api/tmdb/discover/{media_type}")
async def discover_media(request: web.Request) -> web.Response:
    """
    Discover movies or TV shows with filters.
    """
    media_type = request.match_info['media_type']
    
    # Extract query params
    page = int(request.query.get('page', 1))
    sort_by = request.query.get('sort_by', 'popularity.desc')
    
    # Helper to clean params
    def get_int(k): return int(request.query[k]) if request.query.get(k) and request.query[k].isdigit() else None
    def get_float(k): return float(request.query[k]) if request.query.get(k) else None
    
    filters = {
        'page': page,
        'sort_by': sort_by,
        'genre': get_int('genre'),
        'year': get_int('year'),
        'date_from': request.query.get('date_from'),
        'date_to': request.query.get('date_to'),
        'language': request.query.get('language'),
        'certification': request.query.get('certification'),
        'runtime_min': get_int('runtime_min'),
        'runtime_max': get_int('runtime_max'),
        'score_min': get_float('score_min'),
        'score_max': get_float('score_max'),
        'vote_count_min': get_int('vote_count_min')
    }
    
    try:
        if media_type == 'movie':
            data = await tmdb_client.get_discover_movies(**filters)
        elif media_type == 'tv':
            data = await tmdb_client.get_discover_tv(**filters)
        else:
            return web.json_response({"error": "Invalid media_type"}, status=400)
        
        if not data:
            return web.json_response({"error": "Failed to communicate with TMDB"}, status=502)
        
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
        
        return web.json_response({
            "results": results,
            "page": data.get("page", 1),
            "total_pages": data.get("total_pages", 0),
            "total_results": data.get("total_results", 0)
        })
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)


@routes.get("/api/tmdb/trending/{media_type}/{time_window}")
async def get_trending(request: web.Request) -> web.Response:
    """Get trending items."""
    media_type = request.match_info['media_type']
    time_window = request.match_info['time_window']
    page = int(request.query.get('page', 1))
    
    try:
        data = await tmdb_client.get_trending(media_type, time_window, page=page)
        if not data:
            return web.json_response({"error": "Failed to communicate with TMDB"}, status=502)
            
        results = []
        for item in data.get("results", []):
            mt = item.get("media_type", media_type)
            if mt not in ["movie", "tv"]: continue
            
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
            
        return web.json_response({"results": results})
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)


@routes.get("/api/tmdb/genres")
async def get_genres(request: web.Request) -> web.Response:
    """Get list of genres for movies or TV."""
    media_type = request.query.get('type', 'movie')
    try:
        data = await tmdb_client.get_genres(media_type)
        return web.json_response({"genres": data.get("genres", [])})
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)


@routes.get("/api/tmdb/movie/{tmdb_id}")
async def get_movie_details(request: web.Request) -> web.Response:
    """Get movie details."""
    tmdb_id = int(request.match_info['tmdb_id'])
    try:
        data = await tmdb_client.get_movie_details(tmdb_id)
        if not data:
             return web.json_response({"error": "Not found"}, status=404)
             
        if data.get("poster_path"):
            data["poster_url"] = tmdb_client.get_poster_url(data["poster_path"], "w500")
        if data.get("backdrop_path"):
            data["backdrop_url"] = f"https://image.tmdb.org/t/p/w1280{data['backdrop_path']}"
            
        return web.json_response(data)
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)

@routes.get("/api/tmdb/tv/{tmdb_id}")
async def get_tv_details(request: web.Request) -> web.Response:
    """Get TV details."""
    tmdb_id = int(request.match_info['tmdb_id'])
    try:
        data = await tmdb_client.get_tv_details(tmdb_id)
        if not data:
             return web.json_response({"error": "Not found"}, status=404)
        
        if data.get("poster_path"):
            data["poster_url"] = tmdb_client.get_poster_url(data["poster_path"], "w500")
        if data.get("backdrop_path"):
            data["backdrop_url"] = f"https://image.tmdb.org/t/p/w1280{data['backdrop_path']}"
            
        return web.json_response(data)
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)

@routes.get("/api/tmdb/tv/{tmdb_id}/season/{season_number}")
async def get_season_details(request: web.Request) -> web.Response:
    tmdb_id = int(request.match_info['tmdb_id'])
    season_number = int(request.match_info['season_number'])
    try:
        data = await tmdb_client.get_season_details(tmdb_id, season_number)
        return web.json_response(data or {}, status=404 if not data else 200)
    except Exception as e:
        return web.json_response({"error": str(e)}, status=500)
        
@routes.get("/api/tmdb/{media_type}/{tmdb_id}/similar")
async def get_similar(request: web.Request) -> web.Response:
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
async def get_recommendations(request: web.Request) -> web.Response:
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
