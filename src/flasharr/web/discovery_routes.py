"""
Discovery Routes (AIOHTTP)

API endpoints for Smart Search and Discovery features.
"""

import logging
from aiohttp import web
from ..services.smart_search import get_smart_search_service
from ..services.tmdb import tmdb_client
from ..clients.timfshare import TimFshareClient
from ..core.config import get_config
import os

logger = logging.getLogger(__name__)

routes = web.RouteTableDef()


@routes.post("/api/discovery/smart-search")
async def smart_search(request: web.Request) -> web.Response:
    """
    Trigger a smart search for a specific item (Movie or Episode).
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
    
    # Search for specific item
    if season and episode:
        # Assuming sync but wrapping could be needed if blocking.
        # However indexer calls are usually blocking HTTP requests if not fully async.
        # But let's assume standard usage as previous files.
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
    """
    media_type = request.query.get('type', 'movie')
    limit = int(request.query.get('limit', 20))
    page = int(request.query.get('page', 1))
    
    # Get trending from TMDB
    # TMDB Client is async, await it
    try:
        trending_data = await tmdb_client.get_trending(media_type, 'day', page=page)
    except Exception as e:
        logger.error(f"Error fetching trending: {e}")
        return web.json_response({"error": "Failed to fetch trending data"}, status=502)
    
    if not trending_data:
        return web.json_response({"error": "Failed to fetch trending data"}, status=502)
    
    # Check availability on Fshare via TimFshare
    # Assuming TimFshareClient is sync (based on previous observations), we run it directly.
    # If it blocks too long, we should run_in_executor.
    # For now, let's keep it simple.
    timfshare = TimFshareClient()
    results = []
    
    # Only process up to limit items to avoid slow responses
    items_to_process = trending_data.get('results', [])[:limit]
    
    for item in items_to_process:
        title = item.get('title') if media_type == 'movie' else item.get('name')
        year = None
        
        release_date = item.get('release_date') if media_type == 'movie' else item.get('first_air_date')
        if release_date and len(release_date) >= 4:
            year = release_date[:4]
        
        search_query = f"{title} {year}" if year else title
        try:
            search_results = timfshare.search(search_query, limit=3)
            fshare_available = len(search_results) > 0
            fshare_count = len(search_results)
        except Exception as e:
            logger.warning(f"Failed to check Fshare availability for {title}: {e}")
            fshare_available = False
            fshare_count = 0
        
        result = {
            "id": item.get("id"),
            "media_type": media_type,
            "title": title,
            "original_title": item.get("original_title") if media_type == "movie" else item.get("original_name"),
            "overview": item.get("overview"),
            "release_date": release_date,
            "poster_url": tmdb_client.get_poster_url(item.get("poster_path")),
            "backdrop_url": f"https://image.tmdb.org/t/p/w780{item.get('backdrop_path')}" if item.get('backdrop_path') else None,
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
    """
    limit = int(request.query.get('limit', 20))
    
    # For now, return popular movies as fallback
    try:
        popular_data = await tmdb_client.get_popular_movies(page=1)
    except:
        return web.json_response({"error": "Failed to fetch recommendations"}, status=502)
    
    if not popular_data:
        return web.json_response({"error": "Failed to fetch recommendations"}, status=502)
    
    results = []
    for item in popular_data.get('results', [])[:limit]:
        results.append({
            "id": item.get("id"),
            "media_type": "movie",
            "title": item.get("title"),
            "overview": item.get("overview"),
            "release_date": item.get("release_date"),
            "poster_url": tmdb_client.get_poster_url(item.get("poster_path")),
            "backdrop_url": tmdb_client.get_poster_url(item.get("backdrop_path"), "w780") if item.get("backdrop_path") else None,
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
            "results": results[:5]
        })
    except Exception as e:
        logger.error(f"Failed to check Fshare availability: {e}")
        return web.json_response({"error": str(e)}, status=500)
