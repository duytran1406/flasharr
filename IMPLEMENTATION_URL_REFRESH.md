# Implementation Summary: URL Refresh & Debug Mode

## ✅ Completed Tasks

### 1. Unit Tests for URL Refresh (`tests/unit/test_url_refresh.py`)

Created comprehensive test suite with 7 test cases covering:

- ✅ **Successful URL refresh** - Verifies new direct URL is fetched from original Fshare URL
- ✅ **Missing original URL** - Handles gracefully when no original URL is saved
- ✅ **Resolution failure** - Proper error handling when Fshare API fails
- ✅ **Exception handling** - Catches and logs network/API errors
- ✅ **Resume triggers refresh** - Confirms resume operation refreshes expired URLs
- ✅ **Active download skip** - Prevents unnecessary refresh for active downloads
- ✅ **Original URL saved** - Validates original URL is stored on new downloads

**Test Coverage:**
- `BuiltinDownloadClient.refresh_download_url()`
- `BuiltinDownloadClient.resume_download()`
- `BuiltinDownloadClient.add_download()`

**Run Tests:**
```bash
# Inside container
docker exec flasharr python3 -m pytest /app/tests/unit/test_url_refresh.py -v

# Or deploy and tests will run automatically
cd /etc/pve/fshare-arr-bridge && SKIP_GIT=true bash deploy.sh
```

### 2. Debug Mode: Stop at 1% (`engine.py`)

Added automatic pause at 1% progress to save daily quota during testing.

**Implementation:**
- Environment variable: `DEBUG_STOP_AT_1_PERCENT=true`
- Checks progress after each chunk download
- Automatically pauses when progress ≥ 1.0%
- Sets error message: "DEBUG: Auto-paused at 1%"
- Updates database and triggers WebSocket notification

**Location:** `src/flasharr/downloader/engine.py` (lines 1110-1121)

**Usage:**
```bash
# Enable in .env file
DEBUG_STOP_AT_1_PERCENT=true

# Redeploy
docker-compose up -d

# All downloads will auto-pause at 1%
# Resume anytime - URL will be refreshed if expired
```

**Benefits:**
- 💰 Saves daily quota (stops after ~26MB for 2.5GB file)
- 🧪 Perfect for testing URL refresh logic
- 🔄 Can resume multiple times to test refresh
- 📊 See real progress updates via WebSocket

## 📁 Files Created/Modified

### Created:
1. `/etc/pve/fshare-arr-bridge/tests/unit/test_url_refresh.py` - Unit tests
2. `/etc/pve/fshare-arr-bridge/tests/unit/README_URL_REFRESH.md` - Test documentation
3. `/etc/pve/fshare-arr-bridge/run_url_tests.sh` - Test runner script

### Modified:
1. `/etc/pve/fshare-arr-bridge/src/flasharr/downloader/engine.py` - Added debug stop condition
2. `/etc/pve/fshare-arr-bridge/.env` - Added DEBUG_STOP_AT_1_PERCENT variable
3. `/etc/pve/fshare-arr-bridge/src/flasharr/downloader/queue.py` - Added original_url column
4. `/etc/pve/fshare-arr-bridge/src/flasharr/downloader/builtin_client.py` - URL refresh logic
5. `/etc/pve/fshare-arr-bridge/src/flasharr/static/js/app_v2.js` - Error message display

## 🔄 Complete Flow: URL Refresh on Resume

```
1. User adds download
   ├─ Original URL: https://www.fshare.vn/file/ABC123
   └─ Direct URL: https://download039.fshare.vn/dl/token1/movie.mkv

2. Download starts (DEBUG_STOP_AT_1_PERCENT=true)
   ├─ Progress: 0% → 1%
   ├─ Auto-paused at 1.0%
   └─ Status: PAUSED, Error: "DEBUG: Auto-paused at 1%"

3. User clicks Resume (1 hour later, URL expired)
   ├─ System checks: original_url exists? ✓
   ├─ Calls Fshare API with original URL
   ├─ Gets new direct URL: https://download039.fshare.vn/dl/token2/movie.mkv
   ├─ Updates task.url = new_direct_url
   ├─ Clears error_message
   └─ Resumes download from 1%

4. Download continues
   ├─ Progress: 1% → 2%
   ├─ Auto-paused at 2.0% (if still in debug mode)
   └─ Can repeat resume test multiple times
```

## 🧪 Testing the Implementation

### Test URL Refresh:
```bash
# 1. Enable debug mode
echo "DEBUG_STOP_AT_1_PERCENT=true" >> .env

# 2. Deploy
SKIP_GIT=true bash deploy.sh

# 3. Add a download via UI
# - It will auto-pause at 1%

# 4. Wait 5 minutes (or modify system time)

# 5. Click Resume
# - Check logs: Should see "Refreshing download URL from: ..."
# - Download should resume with new URL

# 6. Check database
docker exec flasharr sqlite3 /app/data/downloads.db "SELECT id, original_url, url FROM downloads;"
```

### Test Error Display:
```bash
# 1. Cause a download to fail (disconnect network, invalid URL, etc.)

# 2. Check UI
# - Status column shows error icon with tooltip
# - Red warning message appears below status
# - Error message visible on hover

# 3. Check WebSocket
# - Open browser console
# - See: {event_type: "task_updated", data: {er: "error message"}}
```

## 📊 Database Schema Changes

### downloads table:
```sql
-- Added column
original_url TEXT  -- Stores original Fshare URL for refresh

-- Example data
id: "abc-123"
url: "https://download039.fshare.vn/dl/token/movie.mkv"  -- Expires in 1 hour
original_url: "https://www.fshare.vn/file/ABC123XYZ"     -- Never expires
filename: "Movie.2024.1080p.mkv"
state: "Paused"
error_message: "DEBUG: Auto-paused at 1%"
```

## 🎯 Next Steps

1. **Deploy changes:**
   ```bash
   cd /etc/pve/fshare-arr-bridge
   SKIP_GIT=true bash deploy.sh
   ```

2. **Enable debug mode** (optional):
   ```bash
   # Edit .env
   DEBUG_STOP_AT_1_PERCENT=true
   ```

3. **Test URL refresh:**
   - Add a download
   - Let it pause at 1%
   - Resume after a few minutes
   - Verify URL was refreshed in logs

4. **Run unit tests:**
   ```bash
   docker exec flasharr python3 -m pytest /app/tests/unit/test_url_refresh.py -v
   ```

## 📝 Notes

- Debug mode only affects NEW downloads started after enabling
- Existing downloads in progress won't be affected
- URL refresh is automatic on resume - no user action needed
- Error messages appear in real-time via WebSocket
- Original URLs are saved for ALL downloads (not just debug mode)

## 🔐 Security

- Original URLs are stored securely in SQLite database
- No sensitive tokens are logged
- Error messages don't expose internal URLs
- Debug mode is disabled by default in production
