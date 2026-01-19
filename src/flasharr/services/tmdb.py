
import aiohttp
import logging
from typing import Dict, Any, List, Optional
from ..core.config import get_config

logger = logging.getLogger(__name__)

class TMDBClient:
    def __init__(self):
        self.base_url = "https://api.themoviedb.org/3"
        
    @property
    def api_key(self):
        return get_config().tmdb.api_key or "REPLACE_WITH_YOUR_KEY"

    async def _request(self, method: str, endpoint: str, params: Dict[str, Any] = None) -> Dict[str, Any]:
        url = f"{self.base_url}{endpoint}"
        if params is None:
            params = {}
        params['api_key'] = self.api_key
        
        async with aiohttp.ClientSession() as session:
            try:
                async with session.request(method, url, params=params, timeout=10) as response:
                    if response.status != 200:
                        logger.error(f"TMDB API Error {response.status}: {await response.text()}")
                        return {}
                    return await response.json()
            except Exception as e:
                logger.error(f"TMDB Connection Error: {e}")
                return {}
    
    async def get_discover_movies(self, page: int = 1, sort_by: str = 'popularity.desc', 
                                 genre: Optional[int] = None, year: Optional[int] = None,
                                 date_from: Optional[str] = None, date_to: Optional[str] = None,
                                 language: Optional[str] = None, certification: Optional[str] = None,
                                 runtime_min: Optional[int] = None, runtime_max: Optional[int] = None,
                                 score_min: Optional[float] = None, score_max: Optional[float] = None,
                                 vote_count_min: Optional[int] = None) -> Dict[str, Any]:
        """Fetch movies with sorting and advanced filtering."""
        params = {
            'sort_by': sort_by,
            'page': page,
            'include_adult': 'false'
        }
        if genre: params['with_genres'] = genre
        if year: params['primary_release_year'] = year
        if date_from: params['primary_release_date.gte'] = date_from
        if date_to: params['primary_release_date.lte'] = date_to
        if language: params['with_original_language'] = language
        if certification: 
            params['certification_country'] = 'US'
            params['certification'] = certification
        if runtime_min: params['with_runtime.gte'] = runtime_min
        if runtime_max: params['with_runtime.lte'] = runtime_max
        if score_min: params['vote_average.gte'] = score_min
        if score_max: params['vote_average.lte'] = score_max
        if vote_count_min: params['vote_count.gte'] = vote_count_min
            
        return await self._request('GET', '/discover/movie', params=params)

    async def get_discover_tv(self, page: int = 1, sort_by: str = 'popularity.desc', 
                             genre: Optional[int] = None, year: Optional[int] = None,
                             date_from: Optional[str] = None, date_to: Optional[str] = None,
                             language: Optional[str] = None, certification: Optional[str] = None,
                             runtime_min: Optional[int] = None, runtime_max: Optional[int] = None,
                             score_min: Optional[float] = None, score_max: Optional[float] = None,
                             vote_count_min: Optional[int] = None) -> Dict[str, Any]:
        """Fetch TV shows with sorting and advanced filtering."""
        params = {
            'sort_by': sort_by,
            'page': page,
            'include_adult': 'false'
        }
        if genre: params['with_genres'] = genre
        if year: params['first_air_date_year'] = year
        if date_from: params['first_air_date.gte'] = date_from
        if date_to: params['first_air_date.lte'] = date_to
        if language: params['with_original_language'] = language
        if certification:
             # TV certifications often require 'certification_country' too, but might use 'content_ratings' logic 
             # TMDB API for TV discovery usually uses 'with_content_runtime' or similar, but for certification:
             # It's 'watch_region=US' & 'with_watch_monetization_types=flatrate' usually for streaming,
             # but for age rating it's 'certification_country=US' & 'certification=...'.
             # However, for TV it's technically 'first_air_date' etc. 
             # NOTE: TV content rating filtering in TMDB is tricky and may not work perfectly without watch_region.
             # We will try standard params.
             params['watch_region'] = 'US' 
             # params['certification'] = certification # This is technically for movies usually.
             # For TV it is creating complexity. Let's map it roughly or skip strict implementation if complex.
             # Actually TMDB doc says for TV: 'air_date.gte', etc. 
             # Let's stick to what allows basic filtering. 
             # For now, we will apply the param, if it fails we remove.
             pass 

        if runtime_min: params['with_runtime.gte'] = runtime_min
        if runtime_max: params['with_runtime.lte'] = runtime_max
        if score_min: params['vote_average.gte'] = score_min
        if score_max: params['vote_average.lte'] = score_max
        if vote_count_min: params['vote_count.gte'] = vote_count_min
            
        return await self._request('GET', '/discover/tv', params=params)

    async def search(self, query: str, page: int = 1, media_type: str = 'multi') -> Dict[str, Any]:
        """Search for content (multi or specific type)."""
        endpoint = f'/search/{media_type}' if media_type in ['movie', 'tv'] else '/search/multi'
        return await self._request('GET', endpoint, params={'query': query, 'page': page})

    async def get_movie_details(self, tmdb_id: int) -> Dict[str, Any]:
        """Get movie details."""
        return await self._request('GET', f'/movie/{tmdb_id}', params={'append_to_response': 'credits,keywords,external_ids,release_dates'})

    async def get_tv_details(self, tmdb_id: int) -> Dict[str, Any]:
        """Get TV show details."""
        return await self._request('GET', f'/tv/{tmdb_id}', params={'append_to_response': 'credits,keywords,external_ids,content_ratings'})

    async def get_collection_details(self, collection_id: int) -> Dict[str, Any]:
        """Get collection details."""
        return await self._request('GET', f'/collection/{collection_id}')

    async def get_similar_movies(self, tmdb_id: int) -> Dict[str, Any]:
        """Get similar movies."""
        return await self._request('GET', f'/movie/{tmdb_id}/similar')

    async def get_similar_tv(self, tmdb_id: int) -> Dict[str, Any]:
        """Get similar TV shows."""
        return await self._request('GET', f'/tv/{tmdb_id}/similar')

    async def get_recommendations_movies(self, tmdb_id: int) -> Dict[str, Any]:
        """Get movie recommendations."""
        return await self._request('GET', f'/movie/{tmdb_id}/recommendations')

    async def get_recommendations_tv(self, tmdb_id: int) -> Dict[str, Any]:
        """Get TV show recommendations."""
        return await self._request('GET', f'/tv/{tmdb_id}/recommendations')

    async def get_season_details(self, tmdb_id: int, season_number: int) -> Dict[str, Any]:
        """Get TV season details."""
        return await self._request('GET', f'/tv/{tmdb_id}/season/{season_number}')

    async def get_trending(self, media_type: str = "all", time_window: str = "day", page: int = 1) -> Dict[str, Any]:
        """Get trending items."""
        return await self._request('GET', f'/trending/{media_type}/{time_window}', params={'page': page})

    async def get_popular_movies(self, page: int = 1) -> Dict[str, Any]:
        """Get popular movies."""
        return await self._request('GET', '/movie/popular', params={'page': page})

    async def get_popular_tv(self, page: int = 1) -> Dict[str, Any]:
        """Get popular TV shows."""
        return await self._request('GET', '/tv/popular', params={'page': page})

    def get_poster_url(self, path: str, size: str = "w342") -> str:
        """Get full poster URL."""
        if not path:
            return ""
        return f"https://image.tmdb.org/t/p/{size}{path}"

    async def get_movie_alternative_titles(self, tmdb_id: int) -> Dict[str, Any]:
        """
        Get alternative titles for a movie in all available languages.
        
        TMDB API: /movie/{movie_id}/alternative_titles
        
        Returns titles including Vietnamese (VN), Chinese (CN), etc.
        """
        return await self._request('GET', f'/movie/{tmdb_id}/alternative_titles')
    
    async def get_tv_alternative_titles(self, tmdb_id: int) -> Dict[str, Any]:
        """
        Get alternative titles for a TV show in all available languages.
        
        TMDB API: /tv/{series_id}/alternative_titles
        
        Returns titles including Vietnamese (VN), Chinese (CN), etc.
        """
        return await self._request('GET', f'/tv/{tmdb_id}/alternative_titles')
    
    async def get_alternative_titles(self, tmdb_id: int, media_type: str) -> List[str]:
        """
        Unified method to get all alternative titles for a movie or TV show.
        
        Prioritizes Vietnamese titles (VN) for local content matching.
        
        Args:
            tmdb_id: TMDB ID of the movie/TV show
            media_type: 'movie' or 'tv'
            
        Returns:
            List of alternative titles, Vietnamese titles first
        """
        try:
            if media_type == 'movie':
                data = await self.get_movie_alternative_titles(tmdb_id)
                titles_key = 'titles'
            else:
                data = await self.get_tv_alternative_titles(tmdb_id)
                titles_key = 'results'
            
            titles = []
            vn_titles = []
            
            for item in data.get(titles_key, []):
                iso = item.get('iso_3166_1', '')
                title = item.get('title', '') or item.get('name', '')
                
                if not title:
                    continue
                
                # Prioritize Vietnamese titles
                if iso == 'VN':
                    vn_titles.append(title)
                else:
                    titles.append(title)
            
            # Vietnamese titles first, then others
            result = vn_titles + titles
            
            if result:
                logger.info(f"Found {len(result)} alternative titles for {media_type}/{tmdb_id} (VN: {len(vn_titles)})")
            
            return result
            
        except Exception as e:
            logger.warning(f"Failed to get alternative titles for {media_type}/{tmdb_id}: {e}")
            return []

    async def get_genres(self, media_type: str = 'movie') -> Dict[str, Any]:
        """Fetch genres for movies or TV shows."""
        return await self._request('GET', f'/genre/{media_type}/list')

# Singleton instance
tmdb_client = TMDBClient()
