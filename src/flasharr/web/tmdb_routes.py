"""
TMDB API Routes (AIOHTTP)

Endpoints for TMDB discovery/search.
"""

import logging
import os
from aiohttp import web
from ..clients.tmdb import TMDBClient

logger = logging.getLogger(__name__)

routes = web.RouteTableDef()

def get_tmdb_client() -> TMDBClient:
    """Get TMDB client instance."""
    api_key = os.getenv('TMDB_API_KEY', '')
    if not api_key:
        logger.warning("TMDB_API_KEY not set in environment")
    return TMDBClient(api_key)


@routes.get("/api/tmdb/search/{media_type}")
async def search_media(request: web.Request) -> web.Response:
    """
    Search for movies or TV shows.
    
    Query params:
        q: search query
        page: page number (default: 1)
    """
    media_type = request.match_info['media_type']
    query = request.query.get('q', '')
    page = int(request.query.get('page', 1))
    
    if not query:
        return web.json_response({"results": [], "page": 1, "total_pages": 0})
    
    client = get_tmdb_client()
    if not client.api_key:
        return web.json_response({"error": "TMDB API Key not configured"}, status=500)
    
    if media_type == 'movie':
        data = client.search_movie(query, page)
    elif media_type == 'tv':
        data = client.search_tv(query, page)
    else:
        data = client.search_multi(query, page)
    
    if not data:
        return web.json_response({"error": "Failed to communicate with TMDB"}, status=502)
    
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
            "original_title": item.get("original_title") if mt == "movie" else item.get("original_name"),
            "overview": item.get("overview"),
            "release_date": item.get("release_date") if mt == "movie" else item.get("first_air_date"),
            "poster_url": client.get_poster_url(item.get("poster_path")),
            "backdrop_url": client.get_backdrop_url(item.get("backdrop_path")),
            "score": item.get("vote_average"),
            "vote_count": item.get("vote_count"),
            "genres": item.get("genre_ids", [])
        })
    
    return web.json_response({
        "results": results,
        "page": data.get("page", 1),
        "total_pages": data.get("total_pages", 0)
    })


@routes.get("/api/tmdb/discover/{media_type}")
async def discover_media(request: web.Request) -> web.Response:
    """
    Discover movies or TV shows with filters.
    
    Query params:
        sort_by: popularity.desc, release_date.desc, vote_average.desc, etc.
        page: page number
        with_genres: comma-separated genre IDs
        year: release year
        ... and many more TMDB discover filters
    """
    media_type = request.match_info['media_type']
    filters = dict(request.query)
    
    client = get_tmdb_client()
    if not client.api_key:
        return web.json_response({"error": "TMDB API Key not configured"}, status=500)
    
    if media_type == 'movie':
        data = client.discover_movie(**filters)
    elif media_type == 'tv':
        data = client.discover_tv(**filters)
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
            "poster_url": client.get_poster_url(item.get("poster_path")),
            "backdrop_url": client.get_backdrop_url(item.get("backdrop_path")),
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


@routes.get("/api/tmdb/trending/{media_type}/{time_window}")
async def get_trending(request: web.Request) -> web.Response:
    """
    Get trending items.
    
    Path params:
        media_type: all, movie, tv
        time_window: day, week
    """
    media_type = request.match_info['media_type']
    time_window = request.match_info['time_window']
    
    client = get_tmdb_client()
    if not client.api_key:
        return web.json_response({"error": "TMDB API Key not configured"}, status=500)
    
    data = client.get_trending(media_type, time_window)
    
    if not data:
        return web.json_response({"error": "Failed to communicate with TMDB"}, status=502)
    
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
            "poster_url": client.get_poster_url(item.get("poster_path")),
            "backdrop_url": client.get_backdrop_url(item.get("backdrop_path")),
            "score": item.get("vote_average"),
            "vote_count": item.get("vote_count"),
            "popularity": item.get("popularity")
        })
    
    return web.json_response({"results": results})


@routes.get("/api/tmdb/genres/{media_type}")
async def get_genres(request: web.Request) -> web.Response:
    """Get list of genres for movies or TV."""
    media_type = request.match_info['media_type']
    
    client = get_tmdb_client()
    if not client.api_key:
        return web.json_response({"error": "TMDB API Key not configured"}, status=500)
    
    if media_type == 'movie':
        data = client.get_movie_genres()
    elif media_type == 'tv':
        data = client.get_tv_genres()
    else:
        return web.json_response({"error": "Invalid media_type"}, status=400)
    
    if not data:
        return web.json_response({"error": "Failed to communicate with TMDB"}, status=502)
    
    return web.json_response({"genres": data.get("genres", [])})


@routes.get("/api/tmdb/movie/{tmdb_id}")
async def get_movie_details(request: web.Request) -> web.Response:
    """Get movie details with credits, videos, similar, and recommendations."""
    tmdb_id = int(request.match_info['tmdb_id'])
    
    client = get_tmdb_client()
    if not client.api_key:
        return web.json_response({"error": "TMDB API Key not configured"}, status=500)
    
    data = client.get_movie_details(tmdb_id)
    
    if not data:
        return web.json_response({"error": "Not found"}, status=404)
    
    # Add full image URLs
    if data.get("poster_path"):
        data["poster_url"] = client.get_poster_url(data["poster_path"], "w500")
    if data.get("backdrop_path"):
        data["backdrop_url"] = client.get_backdrop_url(data["backdrop_path"], "w1280")
    
    return web.json_response(data)


@routes.get("/api/tmdb/tv/{tmdb_id}")
async def get_tv_details(request: web.Request) -> web.Response:
    """Get TV show details with credits, videos, similar, and recommendations."""
    tmdb_id = int(request.match_info['tmdb_id'])
    
    client = get_tmdb_client()
    if not client.api_key:
        return web.json_response({"error": "TMDB API Key not configured"}, status=500)
    
    data = client.get_tv_details(tmdb_id)
    
    if not data:
        return web.json_response({"error": "Not found"}, status=404)
    
    # Add full image URLs
    if data.get("poster_path"):
        data["poster_url"] = client.get_poster_url(data["poster_path"], "w500")
    if data.get("backdrop_path"):
        data["backdrop_url"] = client.get_backdrop_url(data["backdrop_path"], "w1280")
    
    return web.json_response(data)


@routes.get("/api/tmdb/tv/{tmdb_id}/season/{season_number}")
async def get_season_details(request: web.Request) -> web.Response:
    """Get TV season details."""
    tmdb_id = int(request.match_info['tmdb_id'])
    season_number = int(request.match_info['season_number'])
    
    client = get_tmdb_client()
    if not client.api_key:
        return web.json_response({"error": "TMDB API Key not configured"}, status=500)
    
    data = client.get_season_details(tmdb_id, season_number)
    
    if not data:
        return web.json_response({"error": "Not found"}, status=404)
    
    return web.json_response(data)


@routes.get("/api/tmdb/{media_type}/{tmdb_id}/similar")
async def get_similar(request: web.Request) -> web.Response:
    """Get similar movies or TV shows."""
    media_type = request.match_info['media_type']
    tmdb_id = int(request.match_info['tmdb_id'])
    page = int(request.query.get('page', 1))
    
    client = get_tmdb_client()
    if not client.api_key:
        return web.json_response({"error": "TMDB API Key not configured"}, status=500)
    
    if media_type == 'movie':
        data = client.get_similar_movies(tmdb_id, page)
    elif media_type == 'tv':
        data = client.get_similar_tv(tmdb_id, page)
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
            "poster_url": client.get_poster_url(item.get("poster_path")),
            "score": item.get("vote_average")
        })
    
    return web.json_response({"results": results})


@routes.get("/api/tmdb/{media_type}/{tmdb_id}/recommendations")
async def get_recommendations(request: web.Request) -> web.Response:
    """Get recommended movies or TV shows."""
    media_type = request.match_info['media_type']
    tmdb_id = int(request.match_info['tmdb_id'])
    page = int(request.query.get('page', 1))
    
    client = get_tmdb_client()
    if not client.api_key:
        return web.json_response({"error": "TMDB API Key not configured"}, status=500)
    
    if media_type == 'movie':
        data = client.get_movie_recommendations(tmdb_id, page)
    elif media_type == 'tv':
        data = client.get_tv_recommendations(tmdb_id, page)
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
            "poster_url": client.get_poster_url(item.get("poster_path")),
            "score": item.get("vote_average")
        })
    
    return web.json_response({"results": results})
