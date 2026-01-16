"""
Discovery Routes (AIOHTTP)

API endpoints for Smart Search and Discovery features.
"""

import logging
from aiohttp import web
from ..services.smart_search import get_smart_search_service
from ..clients.tmdb import TMDBClient
from ..clients.timfshare import TimFshareClient
import os

logger = logging.getLogger(__name__)

routes = web.RouteTableDef()


@routes.post("/api/discovery/smart-search")
async def smart_search(request: web.Request) -> web.Response:
    """
    Trigger a smart search for a specific item (Movie or Episode).
    
    Request Body:
        - title: str
        - year: str (optional)
        - season: int (optional)
        - episode: int (optional)
        - tmdb_id: int (optional, for future use)
    """
    try:
        data = await request.json()
    except:
        data = {}
    
    title = data.get("title")
    season = data.get("season")
    episode = data.get("episode")
    year = data.get("year")
    
    if not title:
        return web.json_response({"error": "Title required"}, status=400)
    
    service = get_smart_search_service()
    
    # Generate search queries
    queries = service.generate_queries(title, season, episode, year)
    
    results = []
    
    if season and episode is None:
        # Search for Season Pack
        # TODO: Implement Season Pack logic
        pass
    
    # Search for specific item
    if season and episode:
        results = service.search_episode(title, season, episode)
    else:
        # Movie search
        all_results = []
        for q in queries:
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
    
    return web.json_response({
        "queries_used": queries,
        "results": results[:20]  # Limit response
    })


@routes.get("/api/discovery/popular-today")
async def popular_today(request: web.Request) -> web.Response:
    """
    Get popular items that are available on Fshare.
    
    Combines TMDB trending with Fshare availability check.
    
    Query params:
        type: movie or tv (default: movie)
        limit: number of results (default: 20)
    """
    media_type = request.query.get('type', 'movie')
    limit = int(request.query.get('limit', 20))
    
    # Get TMDB API key
    api_key = os.getenv('TMDB_API_KEY', '')
    if not api_key:
        return web.json_response({"error": "TMDB API Key not configured"}, status=500)
    
    # Get trending from TMDB
    tmdb = TMDBClient(api_key)
    trending_data = tmdb.get_trending(media_type, 'day')
    
    if not trending_data:
        return web.json_response({"error": "Failed to fetch trending data"}, status=502)
    
    # Check availability on Fshare via TimFshare
    timfshare = TimFshareClient()
    results = []
    
    for item in trending_data.get('results', [])[:limit]:
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
            "poster_url": tmdb.get_poster_url(item.get("poster_path")),
            "backdrop_url": tmdb.get_backdrop_url(item.get("backdrop_path")),
            "score": item.get("vote_average"),
            "vote_count": item.get("vote_count"),
            "popularity": item.get("popularity"),
            "fshare_available": fshare_available,
            "fshare_count": fshare_count
        }
        
        results.append(result)
    
    return web.json_response({
        "status": "ok",
        "results": results,
        "media_type": media_type
    })


@routes.get("/api/discovery/recommendations")
async def get_recommendations(request: web.Request) -> web.Response:
    """
    Get personalized recommendations based on download history.
    
    Query params:
        limit: number of results (default: 20)
    """
    limit = int(request.query.get('limit', 20))
    
    # TODO: Implement recommendation logic based on download history
    # For now, return popular movies
    
    api_key = os.getenv('TMDB_API_KEY', '')
    if not api_key:
        return web.json_response({"error": "TMDB API Key not configured"}, status=500)
    
    tmdb = TMDBClient(api_key)
    popular_data = tmdb.get_popular_movies(page=1)
    
    if not popular_data:
        return web.json_response({"error": "Failed to fetch recommendations"}, status=502)
    
    # Format results
    results = []
    for item in popular_data.get('results', [])[:limit]:
        results.append({
            "id": item.get("id"),
            "media_type": "movie",
            "title": item.get("title"),
            "overview": item.get("overview"),
            "release_date": item.get("release_date"),
            "poster_url": tmdb.get_poster_url(item.get("poster_path")),
            "backdrop_url": tmdb.get_backdrop_url(item.get("backdrop_path")),
            "score": item.get("vote_average"),
            "popularity": item.get("popularity")
        })
    
    return web.json_response({
        "status": "ok",
        "results": results
    })


@routes.get("/api/discovery/available-on-fshare")
async def check_fshare_availability(request: web.Request) -> web.Response:
    """
    Check if a specific title is available on Fshare.
    
    Query params:
        title: title to search for
        year: optional year filter
    """
    title = request.query.get('title', '')
    year = request.query.get('year', '')
    
    if not title:
        return web.json_response({"error": "Title required"}, status=400)
    
    search_query = f"{title} {year}" if year else title
    
    try:
        timfshare = TimFshareClient()
        results = timfshare.search(search_query, limit=10)
        
        return web.json_response({
            "status": "ok",
            "available": len(results) > 0,
            "count": len(results),
            "results": results[:5]  # Return top 5 matches
        })
    except Exception as e:
        logger.error(f"Failed to check Fshare availability: {e}")
        return web.json_response({"error": str(e)}, status=500)
