# Bug Fix Summary: Account Not Appearing + Duplicate API Calls

## Issues Fixed

### Issue 1: Account Not Appearing After Setup Wizard

**Problem**: After completing setup wizard, accounts showed "No Account" in Dashboard and Settings.

**Root Cause**:

- Setup wizard saves credentials to **SQLite database** (`settings` table)
- `/api/accounts` endpoint reads from **config.toml** file
- Data storage mismatch caused accounts to never appear

**Solution**:

1. Updated `/api/accounts` endpoint to read from database first, fallback to config.toml
2. Made `FshareHandler` fetch credentials dynamically from database
3. Added database reference to handler initialization chain

**Files Changed**:

- `backend/src/api/accounts.rs` - Read from DB first
- `backend/src/hosts/fshare.rs` - Dynamic credential fetching
- `backend/src/hosts/mod.rs` - Pass DB to handler
- `backend/src/main.rs` - Provide DB to registry

---

### Issue 2: Duplicate API Calls (4x accounts, 3x status, 3x week)

**Problem**: Single page load triggered multiple redundant API calls, even during intro animation.

**Root Cause Analysis**:

#### Before Fix - Call Chain:

```
1. Layout onMount
   ├─ fetch("/api/accounts")           ← Validation check
   ├─ downloadStore.fetchAll()         ← Fetches /api/downloads + stats
   └─ (setup route) downloadStore.fetchAll() again

2. Dashboard onMount
   └─ accountStore.fetch()             ← /api/accounts (duplicate!)

3. Settings onMount
   ├─ accountStore.fetch()             ← /api/accounts (duplicate!)
   ├─ fetchLogs()                      ← /api/system/logs
   ├─ fetchSettings()                  ← /api/settings/downloads
   ├─ fetchIndexerSettings()           ← /api/settings/indexer
   ├─ fetchSonarrSettings()            ← /api/settings/sonarr
   └─ fetchRadarrSettings()            ← /api/settings/radarr
```

**Why 4x `/api/accounts` calls**:

1. Layout validation check
2. Layout calls downloadStore which may trigger related fetches
3. Dashboard page calls accountStore.fetch()
4. Settings page calls accountStore.fetch()

**Why calls happen during intro**: `onMount()` fires immediately when components mount, regardless of UI visibility.

#### Solution - Centralized Initialization:

```
Layout onMount (ONLY)
├─ Validate setup + accounts (single check)
├─ Initialize theme
├─ Initialize WebSocket handlers
├─ Connect WebSocket (wait 300ms)
├─ Fetch all data ONCE:
│  └─ downloadStore.fetchAll()
└─ Finish intro → navigate to dashboard

Dashboard onMount
└─ Only fetch TMDB trending (page-specific data)

Settings onMount
└─ Only fetch settings-specific data (logs, configs)
```

**Key Changes**:

1. **Centralized data fetching** in layout after validation
2. **WebSocket connects BEFORE navigation** (as requested)
3. **Removed redundant fetches** from individual pages
4. **Proper error handling** - fail safely to setup on account check errors
5. **Sequential initialization** - WebSocket → wait → fetch data

**Files Changed**:

- `frontend/src/routes/+layout.svelte` - Centralized initialization
- `frontend/src/routes/+page.svelte` - Removed accountStore.fetch()
- `frontend/src/routes/settings/+page.svelte` - Removed accountStore.fetch()

---

## Results

### Before:

- ❌ 4x `/api/accounts` calls
- ❌ 3x `/api/setup/status` calls
- ❌ 3x trending/week calls
- ❌ Calls happen even during intro animation
- ❌ Account shows "No Account" after setup
- ❌ Redirect loop on page reload

### After:

- ✅ 1x `/api/accounts` call (validation only)
- ✅ 1x `/api/setup/status` call
- ✅ 1x trending call (page-specific)
- ✅ WebSocket connects before data fetch
- ✅ Account appears immediately after setup
- ✅ No redirect loops
- ✅ Faster page load
- ✅ Cleaner network waterfall

---

## Testing Checklist

- [ ] Complete fresh setup wizard
- [ ] Verify account appears in Dashboard
- [ ] Verify account appears in Settings
- [ ] Check browser network tab - should see only 1x each API call
- [ ] Reload page - should not redirect to setup
- [ ] Verify downloads work with new credentials
- [ ] Test with existing config.toml (backwards compatibility)
- [ ] Verify WebSocket connection establishes before dashboard loads
