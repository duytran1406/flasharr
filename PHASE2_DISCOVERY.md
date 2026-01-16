# Phase 2: Discovery Enhancement Implementation Plan

**Date:** 2026-01-16  
**Status:** 🚀 Ready to Begin  
**Prerequisite:** ✅ UI v2 Complete

---

## 🎯 Objectives

Enhance the discovery features with:
1. **Popular Today** section using TimFshare API
2. **Trending** carousel on dashboard
3. **Enhanced filters** for better discovery
4. **Placeholder images** for media without posters
5. **Improved search** with smart suggestions

---

## 📋 Task Breakdown

### Task 1: Complete TMDB Routes (1 hour)

**File:** `src/flasharr/web/tmdb_routes.py`

**Endpoints to implement:**
```python
@routes.get("/api/tmdb/discover/{media_type}")
async def discover_media(request):
    """
    Discover movies/TV with filters
    Query params: sort_by, page, genre, year, etc.
    """
    pass

@routes.get("/api/tmdb/trending/{media_type}/{time_window}")
async def get_trending(request):
    """
    Get trending items
    time_window: day, week
    """
    pass

@routes.get("/api/tmdb/movie/{tmdb_id}")
@routes.get("/api/tmdb/tv/{tmdb_id}")
async def get_media_details(request):
    """
    Get full details for a movie/TV show
    """
    pass

@routes.get("/api/tmdb/search/{media_type}")
async def search_media(request):
    """
    Search for movies/TV shows
    Query param: q (query string)
    """
    pass

@routes.get("/api/tmdb/genres/{media_type}")
async def get_genres(request):
    """
    Get list of genres for movies or TV
    """
    pass
```

**Implementation:**
```python
from aiohttp import web
from ..clients.tmdb import TMDBClient

routes = web.RouteTableDef()

@routes.get("/api/tmdb/discover/{media_type}")
async def discover_media(request):
    media_type = request.match_info['media_type']
    params = dict(request.query)
    
    client = TMDBClient()
    results = await client.discover(media_type, **params)
    
    return web.json_response(results)

# ... implement other endpoints
```

---

### Task 2: Enhance Discovery Routes (1 hour)

**File:** `src/flasharr/web/discovery_routes.py`

**Endpoints to implement:**
```python
@routes.get("/api/discovery/popular-today")
async def popular_today(request):
    """
    Get popular items from TimFshare
    Combines TMDB trending with Fshare availability
    """
    pass

@routes.get("/api/discovery/recommendations")
async def get_recommendations(request):
    """
    Get personalized recommendations
    Based on download history
    """
    pass
```

**Implementation:**
```python
from aiohttp import web
from ..clients.timfshare import TimFshareClient
from ..clients.tmdb import TMDBClient

routes = web.RouteTableDef()

@routes.get("/api/discovery/popular-today")
async def popular_today(request):
    """
    Get popular items that are available on Fshare
    """
    media_type = request.query.get('type', 'movie')
    
    # Get trending from TMDB
    tmdb = TMDBClient()
    trending = await tmdb.get_trending(media_type, 'day')
    
    # Check availability on Fshare via TimFshare
    timfshare = TimFshareClient()
    results = []
    
    for item in trending['results'][:20]:
        title = item.get('title') or item.get('name')
        # Quick search to check availability
        search_results = await timfshare.search(title, limit=1)
        if search_results:
            item['fshare_available'] = True
            item['fshare_count'] = len(search_results)
        results.append(item)
    
    return web.json_response({
        'status': 'ok',
        'results': results
    })
```

---

### Task 3: Complete TMDB Client (1 hour)

**File:** `src/flasharr/clients/tmdb.py`

**Methods to implement:**
```python
class TMDBClient:
    def __init__(self):
        self.api_key = os.getenv('TMDB_API_KEY')
        self.base_url = 'https://api.themoviedb.org/3'
    
    async def discover(self, media_type, **filters):
        """Discover movies/TV with filters"""
        pass
    
    async def get_trending(self, media_type, time_window='day'):
        """Get trending items"""
        pass
    
    async def get_details(self, media_type, tmdb_id):
        """Get full details"""
        pass
    
    async def search(self, media_type, query):
        """Search for items"""
        pass
    
    async def get_genres(self, media_type):
        """Get genre list"""
        pass
    
    async def get_similar(self, media_type, tmdb_id):
        """Get similar items"""
        pass
    
    async def get_recommendations(self, media_type, tmdb_id):
        """Get recommendations"""
        pass
```

---

### Task 4: Add Placeholder Images (30 min)

**Create:** `src/flasharr/static/images/placeholder-poster.png`

Use generate_image tool to create a placeholder:
```
Prompt: "Movie poster placeholder, dark background with film reel icon, 
minimalist design, 2:3 aspect ratio, professional, modern"
```

**Update CSS:**
```css
.poster-image {
    background-image: url('/static/images/placeholder-poster.png');
    background-size: cover;
}

.poster-image[style*="background-image"] {
    /* Override when real image is set */
}
```

---

### Task 5: Implement Popular Today Section (1 hour)

**Update:** `src/flasharr/static/js/app_v2.js`

**Add to Dashboard:**
```javascript
async loadDashboard() {
    // ... existing code ...
    
    // Add Popular Today section
    const popularSection = `
        <div class="glass-panel" style="margin-top: 2rem; padding: 2rem;">
            <h3 class="glow-text" style="margin-bottom: 1.5rem;">
                <span class="material-icons">local_fire_department</span>
                Popular Today
            </h3>
            <div id="popular-today-grid" class="discover-grid">
                <div class="loading-spinner"></div>
            </div>
        </div>
    `;
    
    this.container.insertAdjacentHTML('beforeend', popularSection);
    
    // Fetch popular items
    this.fetchPopularToday();
}

async fetchPopularToday() {
    try {
        const res = await fetch('/api/discovery/popular-today?type=movie');
        const data = await res.json();
        
        const grid = document.getElementById('popular-today-grid');
        grid.innerHTML = data.results.slice(0, 6).map(item => 
            this.renderPosterCard(item)
        ).join('');
    } catch (e) {
        console.error('Failed to fetch popular today:', e);
    }
}
```

---

### Task 6: Implement Trending Carousel (1 hour)

**Add to Dashboard:**
```javascript
async loadDashboard() {
    // ... existing code ...
    
    const trendingSection = `
        <div class="glass-panel" style="margin-top: 2rem; padding: 2rem;">
            <h3 class="glow-text" style="margin-bottom: 1.5rem;">
                <span class="material-icons">trending_up</span>
                Trending This Week
            </h3>
            <div class="carousel-container">
                <button class="carousel-btn prev" onclick="window.router.carouselPrev()">
                    <span class="material-icons">chevron_left</span>
                </button>
                <div id="trending-carousel" class="carousel-track">
                    <div class="loading-spinner"></div>
                </div>
                <button class="carousel-btn next" onclick="window.router.carouselNext()">
                    <span class="material-icons">chevron_right</span>
                </button>
            </div>
        </div>
    `;
    
    this.container.insertAdjacentHTML('beforeend', trendingSection);
    this.fetchTrending();
}

async fetchTrending() {
    try {
        const res = await fetch('/api/tmdb/trending/movie/week');
        const data = await res.json();
        
        const track = document.getElementById('trending-carousel');
        track.innerHTML = data.results.map(item => 
            this.renderPosterCard(item, 'large')
        ).join('');
        
        this.initCarousel();
    } catch (e) {
        console.error('Failed to fetch trending:', e);
    }
}

initCarousel() {
    this.carouselIndex = 0;
    this.carouselMax = document.querySelectorAll('.carousel-track .poster-card').length;
}

carouselNext() {
    if (this.carouselIndex < this.carouselMax - 4) {
        this.carouselIndex++;
        this.updateCarousel();
    }
}

carouselPrev() {
    if (this.carouselIndex > 0) {
        this.carouselIndex--;
        this.updateCarousel();
    }
}

updateCarousel() {
    const track = document.getElementById('trending-carousel');
    const offset = this.carouselIndex * -220; // Card width + gap
    track.style.transform = `translateX(${offset}px)`;
}
```

**Add CSS:**
```css
.carousel-container {
    position: relative;
    overflow: hidden;
}

.carousel-track {
    display: flex;
    gap: 1.5rem;
    transition: transform 0.4s cubic-bezier(0.2, 0.8, 0.2, 1);
}

.carousel-btn {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    z-index: 10;
    background: var(--bg-glass);
    border: 1px solid var(--border-glass);
    border-radius: 50%;
    width: 48px;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.2s;
}

.carousel-btn:hover {
    background: var(--color-primary);
    color: #000;
    box-shadow: 0 0 20px var(--color-primary);
}

.carousel-btn.prev {
    left: -24px;
}

.carousel-btn.next {
    right: -24px;
}
```

---

### Task 7: Enhanced Filters (1 hour)

**Update Discover Sidebar:**
```javascript
renderSidebarContent() {
    return `
        <!-- Existing filters -->
        
        <!-- NEW: Availability Filter -->
        <div class="filter-section">
            <span class="filter-label">Availability</span>
            <div class="filter-toggle-group">
                <button class="filter-toggle ${this.discoverState.onlyAvailable ? 'active' : ''}" 
                        onclick="window.router.updateFilter('onlyAvailable', !window.router.discoverState.onlyAvailable)">
                    <span class="material-icons">check_circle</span>
                    Only Available on Fshare
                </button>
            </div>
        </div>
        
        <!-- NEW: Sort by Availability -->
        <div class="filter-section">
            <span class="filter-label">Priority</span>
            <select onchange="window.router.updateFilter('priority', this.value)" class="filter-select">
                <option value="popularity">Popularity</option>
                <option value="availability">Fshare Availability</option>
                <option value="rating">TMDB Rating</option>
                <option value="release">Release Date</option>
            </select>
        </div>
    `;
}
```

---

### Task 8: Testing & Integration (1 hour)

**Test Checklist:**
- [ ] Popular Today loads on dashboard
- [ ] Trending carousel works (prev/next buttons)
- [ ] Discover filters work correctly
- [ ] Placeholder images display when no poster
- [ ] TMDB API calls work
- [ ] TimFshare integration works
- [ ] All routes return correct data
- [ ] Error handling works
- [ ] Loading states display correctly
- [ ] Mobile responsive

---

## 🔧 Environment Setup

**Required:**
```bash
# Add to .env
TMDB_API_KEY=your_tmdb_api_key_here
```

**Get TMDB API Key:**
1. Go to https://www.themoviedb.org/settings/api
2. Request API key (free)
3. Add to .env file

---

## 📊 Success Metrics

- [ ] Popular Today section shows 6 items
- [ ] Trending carousel shows 20+ items
- [ ] Filters reduce results correctly
- [ ] Placeholder images display
- [ ] Page load time < 2 seconds
- [ ] No console errors
- [ ] Mobile responsive works

---

## 🚀 Deployment

```bash
# After testing
git add src/flasharr/web/tmdb_routes.py
git add src/flasharr/web/discovery_routes.py
git add src/flasharr/clients/tmdb.py
git add src/flasharr/static/js/app_v2.js
git add src/flasharr/static/css/style_v2.css
git add src/flasharr/static/images/placeholder-poster.png

git commit -m "feat: Enhanced discovery with Popular Today and Trending

- Added Popular Today section on dashboard
- Implemented trending carousel
- Enhanced filters with availability check
- Added placeholder images for missing posters
- Completed TMDB client integration
- Added discovery routes for recommendations"

git tag v0.3.0-beta
```

---

## 📝 Notes

- Use caching for TMDB API calls (rate limit: 40 req/10 sec)
- Cache Popular Today for 1 hour
- Cache Trending for 6 hours
- Cache genre list indefinitely
- Implement retry logic for API failures

---

**Ready to implement!** Let me know when to start. 🚀
