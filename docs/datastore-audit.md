# Datastore Communication Audit & Fixes

## Rule: Only Datastores Should Communicate with Backend

**Principle**: UI components should NEVER directly call `fetch()`. All backend communication must go through datastores.

---

## Violations Found & Fixed

### ✅ **1. Settings Page** - `/routes/settings/+page.svelte`

**Violations (10 direct fetch calls)**:

- ❌ `fetch("/api/settings/downloads")` - GET & PUT
- ❌ `fetch("/api/settings/indexer")` - GET
- ❌ `fetch("/api/settings/indexer/generate-key")` - GET
- ❌ `fetch("/api/system/logs")` - GET
- ❌ `fetch("/api/settings/sonarr")` - GET, PUT, POST (test)
- ❌ `fetch("/api/settings/radarr")` - GET, PUT, POST (test)

**Fix**: Created `systemStore` (`lib/stores/system.ts`)

**Methods Added**:

```typescript
systemStore.fetchDownloadSettings();
systemStore.saveDownloadSettings(settings);
systemStore.fetchIndexerSettings();
systemStore.generateIndexerApiKey();
systemStore.fetchSonarrSettings();
systemStore.saveSonarrSettings(settings);
systemStore.testSonarrConnection(settings);
systemStore.fetchRadarrSettings();
systemStore.saveRadarrSettings(settings);
systemStore.testRadarrConnection(settings);
systemStore.fetchLogs(lines);
systemStore.clearLogs();
```

**Action Required**: Update `settings/+page.svelte` to use `systemStore` methods

---

### ✅ **2. Search Page** - `/routes/search/+page.svelte`

**Violations (2 direct fetch calls)**:

- ❌ `fetch("/api/discovery/trending")` - Line 86
- ❌ `fetch` for search query - Line 113

**Fix**: Created `discoveryStore` (`lib/stores/discovery.ts`)

**Methods Added**:

```typescript
discoveryStore.fetchTrending();
discoveryStore.search(query);
discoveryStore.clearSearch();
```

**Action Required**: Update `search/+page.svelte` to use `discoveryStore` methods

---

### ✅ **3. Discover Page** - `/routes/discover/+page.svelte`

**Violations (2 direct fetch calls)**:

- ❌ Direct fetch for categories - Line 52
- ❌ Direct fetch for media details - Line 97

**Fix**: Use `discoveryStore` (already created above)

**Methods Available**:

```typescript
discoveryStore.fetchByCategory(mediaType, category);
discoveryStore.getMediaDetails(mediaType, id);
```

**Action Required**: Update `discover/+page.svelte` to use `discoveryStore` methods

---

### ✅ **4. Layout** - `/routes/+layout.svelte`

**Violation**:

- ❌ `fetch("/api/accounts")` for validation

**Fix**: Already fixed! Now uses `settingsStore.hasAccounts()`

---

## New Datastores Created

### 1. **systemStore** (`lib/stores/system.ts`)

Handles all system settings and logs:

- Download settings (path, concurrency, segments)
- Indexer settings (API key, URL)
- Sonarr settings (enabled, URL, API key, auto-import)
- Radarr settings (enabled, URL, API key, auto-import)
- System logs

### 2. **discoveryStore** (`lib/stores/discovery.ts`)

Handles all TMDB/search/discovery:

- Trending content
- Search queries
- Category browsing
- Media details

---

## Implementation Checklist

- [x] Create `systemStore` with all settings methods
- [x] Create `discoveryStore` with search/discovery methods
- [x] Fix layout to use `settingsStore.hasAccounts()`
- [ ] Update `settings/+page.svelte` to use `systemStore`
- [ ] Update `search/+page.svelte` to use `discoveryStore`
- [ ] Update `discover/+page.svelte` to use `discoveryStore`
- [ ] Test all pages to ensure no regressions
- [ ] Verify no direct `fetch()` calls remain in UI components

---

## Benefits of This Architecture

1. **Single Source of Truth**: All data flows through stores
2. **Easier Testing**: Mock stores instead of fetch calls
3. **Better Caching**: Stores can cache responses
4. **Consistent Error Handling**: Centralized in stores
5. **Type Safety**: Store methods have proper TypeScript types
6. **Reusability**: Multiple components can use same store methods
7. **State Management**: Reactive updates across all consumers

---

## Next Steps

1. Refactor Settings page to use `systemStore`
2. Refactor Search page to use `discoveryStore`
3. Refactor Discover page to use `discoveryStore`
4. Run full audit to ensure no violations remain
5. Update documentation
