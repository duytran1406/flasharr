# 🎯 Newznab Smart Search Bridge - Implementation Summary

## 📊 Status: 95% Complete

### ✅ What's Been Implemented:

#### 1. **ID Conversion Functions**

- `tvdb_to_tmdb()` - Converts TVDB IDs to TMDB IDs via TMDB API
- `imdb_to_tmdb()` - Converts IMDB IDs to TMDB IDs via TMDB API
- Both functions handle API calls and error cases gracefully

#### 2. **Enhanced Search Handlers**

- `handle_tv_search()` - Now bridges to smart_search with TMDB ID
- `handle_movie_search()` - Now bridges to smart_search with TMDB ID
- Both pass all metadata: season, episode, year, title

#### 3. **Smart Search Integration**

- Made `handle_tv_search()` and `handle_movie_search()` public in smart_search.rs
- Bridge now leverages existing Vietnamese title resolution
- Reuses TMDB caching and enrichment logic

#### 4. **Enhanced Logging**

- Logs all incoming parameters: tvdbid, imdbid, season, ep, query
- Tracks ID conversion success/failure
- Shows bridge operation flow

---

## 🔄 The Complete Flow:

```
Sonarr Request:
  GET /newznab/api?t=tvsearch&tvdbid=81189&season=1&ep=1&apikey=xxx

↓ Indexer receives request

↓ Bridge converts TVDB → TMDB
  tvdb_to_tmdb("81189") → "1396"

↓ Build SmartSearchRequest
  {
    title: "Breaking Bad",
    tmdb_id: "1396",
    type: "tv",
    season: 1,
    episode: 1
  }

↓ Call smart_search (already has Vietnamese logic!)
  - Fetches TMDB alternative titles
  - Gets "Tội Phạm Hoàn Lương" (Vietnamese)
  - Searches Fshare with Vietnamese + S01E01

↓ Convert SmartSearchResponse → Newznab XML
  - Extract results from response
  - Format as <item> elements
  - Add newznab:attr tags

↓ Return to Sonarr
  <?xml version="1.0"?>
  <rss>...</rss>
```

---

## ⏳ What Remains (5% - 30 minutes):

### **Task: Implement `convert_smart_response_to_xml()`**

Currently this function returns empty results. It needs to:

1. **Deserialize the Response Body:**

   ```rust
   async fn convert_smart_response_to_xml(
       response: axum::response::Response,
       query: &str
   ) -> String {
       // Extract body bytes
       let body_bytes = to_bytes(response.into_body(), usize::MAX).await.ok()?;

       // Deserialize to SmartSearchResponse
       let smart_response: SmartSearchResponse =
           serde_json::from_slice(&body_bytes).ok()?;

       // Convert to IndexerResult format
       let indexer_results = smart_response.results.iter().map(|r| {
           IndexerResult {
               title: r.title.clone(),
               guid: format!("fshare://{}", r.file_id),
               link: r.download_url.clone(),
               size: r.size,
               pubdate: Utc::now(),
               category: determine_category(&r.media_type),
           }
       }).collect();

       generate_search_xml(indexer_results, query)
   }
   ```

2. **Add Category Mapping:**
   ```rust
   fn determine_category(media_type: &str) -> u32 {
       match media_type {
           "tv" => 5040,      // TV/HD
           "movie" => 2040,   // Movies/HD
           _ => 2000,         // Generic
       }
   }
   ```

---

## 🧪 Testing Plan:

### 1. **Local Testing:**

```bash
# Start backend with logging
./target/release/flasharr 2>&1 | tee /tmp/flasharr.log

# Test TVDB conversion
curl "http://localhost:8484/newznab/api?t=tvsearch&tvdbid=81189&season=1&ep=1&apikey=flasharr-default-key"

# Check logs for:
# - "Converted TVDB 81189 → TMDB 1396"
# - "TV Search Bridge: title='', tmdb_id=Some(...)"
# - Vietnamese titles in smart_search output
```

### 2. **Sonarr Integration:**

- Add indexer in Sonarr
- Perform manual search for a series
- Check Flasharr logs for full request flow
- Verify Vietnamese titles are being used

### 3. **Radarr Integration:**

- Add indexer in Radarr
- Search for a movie with IMDB ID
- Verify IMDB → TMDB conversion
- Check Vietnamese title usage

---

## 📈 Performance Optimizations (Future):

1. **Cache ID Conversions:**

   ```rust
   static ID_CACHE: Lazy<Cache<String, String>> = Lazy::new(|| {
       Cache::builder()
           .time_to_live(Duration::from_secs(86400)) // 24 hours
           .build()
   });
   ```

2. **Batch TMDB Requests:**
   - When multiple episodes requested, batch the TMDB calls
   - Reduce API latency

3. **Pre-warm Cache:**
   - Popular series IDs can be pre-cached
   - Reduces first-search latency

---

## 🎯 Expected Benefits:

### **Before (Text-Based Search):**

```
Sonarr → "Breaking Bad S01E01"
Fshare → Search for "Breaking Bad S01E01"
Results → ❌ Few or no Vietnamese releases found
```

### **After (ID-Based Smart Search):**

```
Sonarr → tvdbid=81189, season=1, ep=1
TMDB → "Tội Phạm Hoàn Lương" (Vietnamese title)
Fshare → Search for "Tội Phạm Hoàn Lương S01E01"
Results → ✅ Accurate Vietnamese releases!
```

---

## 🚀 Deployment Status:

- ✅ Code committed to main branch
- ✅ GitHub Action triggered
- ⏳ Docker image building
- ⏳ Deploy to LXC 112 (pending build completion)

---

## 📝 Next Session TODO:

1. Implement `convert_smart_response_to_xml()` properly
2. Test with real Sonarr/Radarr searches
3. Add ID conversion caching
4. Monitor performance and accuracy
5. Document Remote Path Mappings for Sonarr/Radarr

---

**Status:** Bridge architecture complete, awaiting response converter implementation.
**ETA to Full Functionality:** 30 minutes of focused work.
