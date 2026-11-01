# Flasharr frontend: shared TypeScript module map (2026-09-02 recovery)

Branch `wip/shared-ts` (base 3fc1018). This file is the contract that the page and component agents code against.
Every path is relative to `frontend/src/`. Import with `$lib/...`.

Chunk names refer to `/work/deployed-pretty/_app/immutable/chunks/*.js`, and `nodes/N` is the page node.
Confidence levels:
- **high**: behaviour is transliterated from the deployed code.
- **med**: behaviour comes from the deployed code, but the file path or export names are inferred.
- **low**: guessed.

Export names are mine wherever the module is new, because the minifier destroys names. Baseline names are kept wherever they existed.

**Verification:** a differential test runs the pure helpers against the deployed chunks in Node. It covers mediaQueue, formatBytes/Speed/ETA, mediaIdentity, torrentFormat, resultUtils (exported subset), fsharePayload.withEpisodeTarget and smartGrabUtils (plan and request builder). It ran 3,569 checks with 0 mismatches. The scripts are in `/root/flasharr-recovery/tools/difftest/` on the host and `/work/wt/shared-ts-tools/` in CT 106.

## Summary

| Path | Status | Deployed source | Confidence |
|---|---|---|---|
| lib/stores/websocket.ts | modified (types only) | BukN4qun | high |
| lib/stores/toasts.ts | modified | CkDX9Xyx | high |
| lib/stores/arr.ts | modified | DhwdlfGU | high |
| lib/stores/system.ts | modified (major) | Cv3rGUaC | high |
| lib/stores/settings.ts | modified | BbNS6lCV (store part) | high |
| lib/stores/account.svelte.ts | modified | BbNS6lCV (class `ze`) | high |
| lib/stores/auth.svelte.ts | **new** | n6-842UH | high (behaviour) / med (path, name) |
| lib/stores/ui.svelte.ts | modified | ACUCnqfv | high |
| lib/stores/smartGrab.ts | modified (types only) | BTaAoaTH (`ta`) | high |
| lib/stores/downloads.svelte.ts | modified (major) | CEn30yCo (after `const T="/api"`) | high |
| lib/stores/downloads.ts | unchanged (compat re-export) | n/a (barrels vanish) | med |
| lib/stores/qbitReducer.ts | **new** | C2Oir40k (lines 35-108) | high (path from notes) |
| lib/stores/qbit.svelte.ts | **new** | C2Oir40k (class `lt`) | high (path from notes) |
| lib/stores/setup.svelte.ts | unchanged | nodes/12 (inlined class `ha`) | high |
| lib/stores/theme.ts | unchanged | nodes/0 (inlined `fr`) | high |
| lib/stores/query.ts | **removed** | none (no `staleTime`/`3e5` anywhere in the build) | high |
| lib/services/tmdb.ts | modified (+resolveTvRoute) | BBDhXcty | high |
| lib/types/tmdb.ts | unchanged | types only | n/a |
| lib/components/downloads/mediaQueue.ts | **new** | CEn30yCo (top, `F`..`X`) | high (path from notes, names mine) |
| lib/components/torrents/torrentFormat.ts | **new** | C2Oir40k (`Re`..`Ht`) | med (path and names mine) |
| lib/components/smart-search/resultUtils.ts | **new** | BTaAoaTH (`je`..`ca`) | med (path from notes; one module because of hoisting order) |
| lib/components/fshare/fsharePayload.ts | **new** | BTaAoaTH (`at`,`tt`,`Rt`) | med (path from notes) |
| lib/components/smart-grab/smartGrabUtils.ts | **new** | ByfGc0pC (`Nt`..`en`) | high (path and `buildSmartGrabPlan` from notes) |
| lib/utils/mediaIdentity.ts | **new** | D0Ri4PtR | med (file name from notes, directory guessed) |
| lib/utils/clipboard.ts | **new** | DDYvk3DQ | med (path and name mine) |
| routes/torrents/+page.ts | **new** | nodes/13 `universal` (`load(){}`) | high |
| routes/setup/+layout.ts | unchanged | nodes/2 universal (`ssr=false, prerender=false`) | high |
| lib/animations/index.ts | unchanged (re-exports `@media-set/core-ui/animations`) | 98pOesfH | high |
| lib/components/index.ts | unchanged; **owned by the component agents** | n/a | n/a |

Not app code (skip these chunks):
- `BwFNqfKi`: Svelte runtime.
- `VRbCq7-N`, `8geeJZgA`, `BhHRXxJG`, `wbPk3Yxo`, `DsnmJJEf`: SvelteKit client, `$app/stores`, `$app/state`, error page and version marker.
- `98pOesfH`: anime.js v4 plus the core-ui animations.
- `DJYh9pCf`: core-ui Button, Modal, ErrorState and the core-ui toast store. **The frontend's own toasts store is CkDX9Xyx.**
- `CJGNbbzB`: core-ui Badge.
- `nodes/3`: inlines chart.js.

## API contract per module

### lib/stores/websocket.ts
- `wsClient`: a singleton with:
  - `connect()`, `disconnect()`, `send(msg)`
  - `on(type: WSMessageType, handler)` and `off(type, handler)`
  - `status`, `lastMessage` and `error` as `Writable` stores
  - an `isConnected` getter
- `isConnected` is a `Readable<boolean>` and `connectionIndicator` is a `Readable<{text,color,icon}>`. Both are unchanged.
- `WSMessageType` now also includes `QBIT_SYNC_INIT`, `QBIT_TORRENT_UPSERT`, `QBIT_TORRENT_REMOVED`, `QBIT_TRANSFER_INFO`, `QBIT_ERROR`, `QBIT_HEARTBEAT` and `INTEGRATION_SETTINGS_CHANGED`.
- `WSMessage` and `WSStatus` are unchanged.

### lib/stores/toasts.ts
- `toasts` supports `subscribe` plus:
  - `success|error|info|warning(msg, duration?: number|null) => id`. The default duration is 5000. **`null` makes the toast sticky.**
  - **new** `update(id, patch: ToastPatch)`. A numeric `duration` restarts the timer and `null` cancels it.
  - `remove(id)` and `clear()`. Both also clear their timers.
- Types:
  - `Toast` = `{id, type, message, duration: number|null, timestamp}`
  - `ToastType`
  - `ToastPatch`

### lib/stores/arr.ts
Stores:
- `libraryOverview: Writable<LibraryOverview|null>`. `LibraryOverview` gains `series_size_on_disk` and `movies_size_on_disk`.
- `calendarEntries`
- `arrStatus`
- `arrLoading`
- `arrError`
- `diskSpaces`

Functions:
- `fetchLibraryOverview()`, `fetchArrStatus()`, `fetchAllSeries()`, `fetchAllMovies()`, `fetchHistory(pageSize=20)` and `fetchEpisodesBySonarrId(seriesId)` are unchanged.
- **`fetchCalendar(days = 14, startDate: Date = new Date())`**: the end date is `startDate + days` using setDate, and both dates are sent as UTC `YYYY-MM-DD`.
- **`findSeriesInList(allSeries, tvdbId, tmdbId?)`**: matches by TVDB first and falls back to TMDB. **The signature changed:** the baseline took `(list, tmdbId)`.
- `findMovieInList(allMovies, tmdbId)`
- `formatDiskSize(bytes)`, `getSeriesPoster`, `getSeriesBanner`, `getMoviePoster`

Kept but not referenced by the build: `fetchDiskSpace`, `fetchMissing` and `tmdbPosterUrl`.

Types: `LibraryOverview`, `SonarrSeries`, `SonarrStatistics`, `RadarrMovie`, `RadarrCollection`, `MediaImage`, `CalendarEntry`, `DiskSpace`, `ArrServiceStatus`, `SonarrEpisode`.

### lib/stores/system.ts
Stores. Every one is a separate `Writable` unless marked derived.

| Store | Type |
|---|---|
| `downloadSettings` | `Writable<DownloadSettings>` |
| `indexerSettings` | `Writable<IndexerSettings>` (has `enabled?`) |
| `sonarrSettings`, `radarrSettings` | `Writable<SonarrSettings>`, `Writable<RadarrSettings>` |
| `prowlarrSettings` | `Writable<ProwlarrSettings>` |
| `integrationStatus` | `Writable<IntegrationStatusState>` = `{sonarr, radarr, prowlarr: IntegrationHealth\|null, loading, error}` |
| `sonarrConfigured`, `radarrConfigured` | derived `Readable<boolean>`: enabled with a URL and an API key |
| `arrConfigured` | derived `Readable<boolean>`: either one is configured |
| `qbittorrentSettings` | `Writable<QBittorrentSettings>` |
| `torboxSettings` | `Writable<TorBoxSettings>` |
| `torboxProfile` | `Writable<TorBoxProfile\|null>` |
| `metadataProviders` | `Writable<MetadataProvider[]>` (`{provider, configured, source, maskedKey, reachable: boolean\|null, message}`) |
| `systemHealth` | `Writable<SystemHealth\|null>` (`{status, version}`) |
| `storageStatus` | `Writable<StorageStatus\|null>` |
| `systemLogs` | `Writable<LogEntry[]>` |
| `systemActivity` | `Writable<ActivityEvent[]>` |
| `folderSourceUrl` | `Writable<string>` |
| `importedFolderSources` | `Writable<number>` |
| `systemLoading`, `systemError` | baseline stores, never written |

Helpers:
- `integrationStatusKind(status, checking=false)` returns one of `'checking'|'disabled'|'unconfigured'|'connected'|'unreachable'|'unknown'`.
- `integrationStatusLabel(status, checking=false)` returns `'Checking'|'Disabled'|'Not configured'|'Connected'|'Unreachable'|'Unknown'`.

Functions. Each is exported on its own and is also a member of `systemStore`, which is what the deployed pages call. `SaveResult` = `{success, message?}`.

**Fetching** (most resolve to `Promise<boolean>`):
- `fetchDownloadSettings`, `fetchIndexerSettings`, `fetchSonarrSettings`, `fetchRadarrSettings` and `fetchProwlarrSettings` resolve to `boolean`.
- `fetchTorBoxSettings`, `fetchTorBoxProfile` and `fetchQBittorrentSettings` resolve to `void`.
- `fetchMetadataProviders`, `fetchSystemHealth`, `fetchStorageStatus` and `fetchFolderSourceConfig` resolve to `boolean`.
- `fetchLogs(lines=50)` resolves to `void`. It hides lines older than the last `clearLogs()`.
- `clearLogs()` runs client-side only.
- `fetchActivity(limit=50)` resolves to `void`.

**Saving and testing:**
- `saveDownloadSettings(s)`
- `saveSonarrSettings(s)` and `saveRadarrSettings(s)`. A 401 produces "Unauthorized — sign in again". Success calls `refreshIntegrationState()`.
- `testSonarrConnection(s)` and `testRadarrConnection(s)`
- `saveProwlarrSettings(s)` and `testProwlarrConnection(s)`
- `saveTorBoxSettings(s)`, `testTorBoxConnection(s)` and `clearTorBoxSettings()` (DELETE)
- `testMetadataProvider(provider, apiKey?)` sends `{apiKey}` and writes `reachable` and `message` onto the matching row.
- `saveMetadataProvider(provider, apiKey)` sends PUT `{apiKey}`.
- `saveQBittorrentSettings(s)` and `testQBittorrentConnection(s)`
- `saveFolderSourceConfig(gistUrl)`
- `importFolderSources(sourceText)` resolves to `FolderSourceImportResult` (`{success, imported, message}`).
- `generateIndexerApiKey()` calls **POST `/settings/indexer/generate`**. The baseline used a GET to `generate-key`. It resolves to `string|null`.

**Integration sync:**
- `fetchIntegrationStatus()`
- `fetchIntegrationConfiguration()`
- `refreshIntegrationState()` fetches the configuration and the status in parallel. Stale responses are dropped by a generation counter.
- `initIntegrationStatusSync()` is idempotent. It refreshes on the `INTEGRATION_SETTINGS_CHANGED` websocket message, on each websocket (re)connect, on window `focus` and on `visibilitychange` to visible.

Types: `DownloadSettings`, `IndexerSettings`, `SonarrSettings`, `RadarrSettings`, `ProwlarrSettings`, `QBittorrentSettings`, `TorBoxSettings`, `TorBoxProfile`, `MetadataProvider`, `IntegrationHealth`, `IntegrationStatusState`, `IntegrationStatusKind`, `SystemHealth`, `StorageStatus`, `LogEntry`, `ActivityEvent`, `SaveResult`, `FolderSourceImportResult`.

### lib/stores/settings.ts
- `settingsStore` has `subscribe`, `fetchSettings()`, `saveSettings(partial)` and `fetchAccounts()`.
- `settingsStore.hasAccounts()` resolves to `boolean`.
- **`addAccount(email, pw)`, `removeAccount(email)` and `refreshAccount(email)` now resolve to `AccountMutationResult`** = `{success, message?, persisted?}`. This is the response body. The baseline resolved to `boolean`.
  - `addAccount` refreshes the list when the request succeeded or when `persisted` is set.
- **Removed:** `testIntegration`, `saveIntegrations`, the `integrations` derived store and the `IntegrationSettings` type. The deployed build still calls `derived()` for settings, loading and saveStatus but has no integrations call.
- Derived stores: `settings`, `accounts`, `isLoadingSettings` and `saveStatus`.
- Helpers: `formatExpiry(ts)`, `getQuotaPercentage(used, total)` and `formatQuota` (unused).
- Types: `Settings`, `FshareAccount`, `AccountMutationResult`.

### lib/stores/account.svelte.ts
`accountStore` (runes) exposes:
- `accountsList` (`$state`)
- `fetch()`, `refresh(email)`, `logout(email)`
- **`replace(email, password)`**, which wraps `settingsStore.addAccount`. **`switchAccount` was removed.**
- the getters `primary`, `isVip`, `listFormatted` and `primaryFormatted`, all unchanged

### lib/stores/auth.svelte.ts (new)
`authStore` (runes) exposes:
- `session: AuthSession|null`, where `AuthSession` = `{configured, authenticated, username}`
- `loading` (starts `true`)
- `error`

Methods:
- `refresh()` calls GET `/api/auth/setup-status`. On error it treats the user as signed out.
- `submit(mode: 'login'|'register', username, password)` calls POST `/api/auth/{mode}`. It throws `Error('Invalid credentials' | 'Unable to create administrator')`, and calls `wsClient.connect()` on success.
- `changePassword(current, next)` throws with the messages for 401, 400 and other failures. On success it disconnects the websocket and signs out.
- `logout()` disconnects the websocket, calls POST `/api/auth/logout` and signs out.

The root layout's AuthGate uses this store, and the settings page also imports it.

### lib/stores/ui.svelte.ts
`ui` exposes:
- `addDownloadModalOpen`, `smartSearchModalOpen`
- `smartSearchData: SmartSearchData|null`, where `SmartSearchData` = `{tmdbId: string, tvdbId?: string, type, title, year?, season?, episode?}`
- **new** `smartSearchContext: SmartSearchContext|null`
- **new** `smartSearchMode: 'season'|'target'`
- **new** `onSmartSearchDownloadResult`
- `showIntro`, `showTagline`

Methods:
- `startIntroSequence()`, `finishIntro()`, `openAddDownload()`, `closeAddDownload()`
- **`openSmartSearch(data, options?: {mode?, context?, onDownloadResult?})`**. The mode defaults to `target` for a movie or a request with an episode, and to `season` otherwise. Season mode removes `episode`.
- `closeSmartSearch()` resets all of the smart search fields.
- `smartGrabModalOpen`, `smartGrabData`, `openSmartGrab(d)`, `closeSmartGrab()`

Types: `SmartSearchData`, `SmartSearchMode`, `SmartSearchContext`, `SmartSearchOptions`, `SmartSearchDownloadResult` (`{tmdbId:number, success:boolean}`), `SmartGrabData`.

The deployed code sets `onSmartSearchDownloadResult` (from the collection page) but never invokes it.

### lib/stores/smartGrab.ts
- `smartGrabStore` has `subscribe`, `open(data)` and `close()`.
- `SmartGrabData` gains `tvdbId?` and `targetSeason?`.
- The layout opens it with `{tmdbId, tvdbId, type:'tv', title, year, seasons, targetSeason, existingDownloads}`.

### lib/stores/downloads.svelte.ts
**State on `downloadStore` (runes):**
- `downloads: Map<id, DownloadTask>`
- `batches: Map<batch_id, BatchSummary>`
- **`batchItems: Map<"<batch_id>|<sourceFilter>", DownloadTask[]>`**. The cache key changed.
- **new** `torrentRows: Map<"inst:hash", UnifiedTorrentRow>`
- **new** `displayRows: DisplayRowRef[]`
- `stats`, `statusCounts`
- `pagination: PaginationState`. It now has **`sourceFilter?: string|null`, default `'direct'`**.
- `loading`, `error`

**Derived:**
- `downloadList`
- `batchList`
- **`torrentList`**, sorted by `added_on` descending
- `activeDownloadsList`, `queuedDownloadsList`, `completedDownloadsList`, `failedDownloadsList`, `pausedDownloadsList`

**Methods:**
- `fetchPage(page?)`
- `fetchBatches(page?)` calls `/api/downloads/batches` and sends `source=` too. It fills batches, downloads, torrentRows and displayRows.
- `fetchBatchItems(batchId)` caches per source filter.
- **`invalidateBatchItemCache()`**
- `fetchAll()`
- `setPage`, `setSort`, `setLimit`, `setStatusFilter`
- **`setSourceFilter(source|null)`**
- **`addDownload(req, {notify?: boolean} = {})`**
- `pauseDownload`, `resumeDownload`, `deleteDownload` (also calls `fetchLibraryOverview`), `retryDownload`, `redownload`
- `redownloadBatch`, `pauseAll`, `resumeAll`, `pauseBatch`, `resumeBatch`, `deleteBatch`
- `initWebSocket()` handles the `TASK_*`, `ENGINE_STATS` and `QBIT_TORRENT_UPSERT/REMOVED/SYNC_INIT` messages for unmanaged torrents, and refetches on reconnect.
- `subscribe(fn)` is the Svelte 4 contract. Its snapshot now includes torrentRows and displayRows.

Error text on failed actions comes from the JSON `error` or `message` field of the response, then from the raw text.

**Other exports:**
- **`onDownloadEvent(type: 'TASK_UPDATED'|'TASK_STARTED'|'TASK_COMPLETED', listener: (e: CustomEvent<DownloadTask>) => void) => unsubscribe`**: an EventTarget side channel. Nodes 5 and 6 and both MediaCard chunks (BHuBWhUK, CrfdV7eE) use it.
- `engineStats`: the only compat readable that the build uses.
- `downloads`, `batches`, `activeDownloads`, `queuedDownloads`, `completedDownloads`, `failedDownloads`, `pausedDownloads`, `statusCounts`, `paginationState`, `isLoading` and `storeError`: kept, but not used by the build.
- `formatBytes(n)` (2 decimals; non-finite or ≤0 gives "0 B"), `formatSpeed(n)`, `formatETA(s)`
- `STATUS_PRIORITY`: kept, but the deployed copy lives in nodes/7.
- `getStateColor`: unused.

**Types:**
- `DownloadState`
- `DownloadTask`. All backend fields are present, including `download_source`, `external_provider`, `external_id`, `info_hash`, `task_state`, `qbit_state`, `sync_health`, `transfer_state`, `tvdb_id`, `tmdb_id`, `media_type`, `tmdb_title`, `tmdb_season`, `tmdb_episode` and `info_path`. `url`, `original_url` and `destination` are optional because the backend no longer serializes them, and `batch_id` is now `string|null|undefined`.
- `BatchSummary` (`source` added), `DisplayRowRef`, `DownloadTorrentRow`
- `EngineStats`, `DbStatusCounts`, `StatusCounts`, `PaginationState`
- `TmdbMetadata` (`tvdb_id` added)
- `AddDownloadRequest`. `url` is optional, and these were added: `download_ref`, `source`, `provider`, `size`, `seeders`, `leechers`, `info_hash`, `file_idx`, `source_filename`, `external_id`, `origin_*`, `strategy`, `force_redownload`, `selected_episodes`, `qbit_instance_id`. `priority` is now `string`.
- `AddDownloadOptions`, `AddDownloadResponse`, `AffectedResponse`, `ApiResponse<T>` (now exported), `DownloadStoreState`, `DownloadEventType`

### lib/components/downloads/mediaQueue.ts (new; pure)
`MediaQueueItem` is task-like, with `id` and an optional `state`, `download_source`, `episode_title` and `is_current`.

**Building the queue:**
- `mergeQueueRows(downloads, batches, batchItems, torrents?, displayRows?) => MediaQueueItem[]`. Unloaded batches become `batch:<id>` placeholders and torrents become `torrent:inst:hash` items. When `displayRows` is given, its order wins.
- `groupQueueItemsByMedia(items) => MediaQueueGroup[]`, where `MediaQueueGroup` = `{key, title, mediaType: 'movie'|'tv'|'unknown', items}`.
- `filterMediaQueueGroups(groups, {query, source, status, mediaType, sort}) => MediaQueueGroup[]`. Sorting is always descending.
- `buildMediaQueueTree(items) => MediaQueueNode[]`. Each node is a movie or unknown leaf, or a `series` with `seasons[{season, episodes[{key, title, item}]}]` and an aggregated `source`.
- `pickCurrentQueueItem(items)` picks `is_current` first, then the newest item.

**Item details:**
- `episodeCoordinates(item) => {season, episode}`
- `mediaQueueKey(item)` returns `tv:<tvdb>:sNN:eNN`, `movie:<tmdb>` or `unresolved:<id>`.
- `queueItemSource(item)` maps qbittorrent to torrent and flasharr-http to fshare.
- `aggregateQueueSource(items, fallback='unknown')` returns a single source or `mixed`.
- `queueStatusLabel(item) => QueueStatusLabel` returns one of "Downloading", "Waiting for peers", "Seeding", "Sync stale" and similar labels.
- `torrentRowToQueueItem(row)`

**Cache helpers:** `upsertBatchItemCache(cache, task)` and `removeFromBatchItemCache(cache, id)`.

The downloads page uses the pipeline `filterMediaQueueGroups(groupQueueItemsByMedia(mergeQueueRows(store.downloadList, store.batchList, store.batchItems, store.torrentList, store.displayRows)), {...})`, and also `buildMediaQueueTree(groups.map(g => pickCurrentQueueItem(g.items)))`.

### lib/stores/qbitReducer.ts (new; pure)
- `UnifiedTorrentRow`: the full backend row. The numeric fields, `tags`, `category`, `tracker` and `updated_at` are required, and the media metadata is optional.
- `TorrentImportState`
- `QbitSyncInitMessage`
- `normalizeTorrentRow(raw)` returns a row or `null`. It requires non-empty `instance_id`, `hash`, `name` and `state`.
- `normalizeTorrentRows(raw)`
- `applyQbitSyncInit(map, msg)` handles `replaces_instance` and keeps the media fields of rows it replaces.
- `pruneTorrentSelection(selected, torrents, instanceId?)`

### lib/stores/qbit.svelte.ts (new)
**State on `qbitStore` (runes):**
- `instances: QbitInstance[]`
- `selectedInstanceId: string`
- `torrents: Map<"inst:hash", UnifiedTorrentRow>`
- `transferInfo: Map<inst, QbitTransferInfo>`
- `transferHealth: Map<inst, {sync_health, last_sync_at?}>`
- `selectedHashes: Set<"inst:hash">`
- `categoryNames`, `tagNames`
- `filterOptions` = `{status, category, tag, tracker}: string[]`
- `detailCache`
- `loading`, `error`, `search`
- `filters` = `{status, category, tag, tracker}: string`

**Derived:** `torrentList` and `selectedInstance`.

**Methods. Every mutation toasts its result.**

Instances:
- `fetchInstances()`
- `saveInstance(input: QbitInstanceInput) => QbitInstance` (throws)
- `deleteInstance(id)` (throws)
- `testInstance(id)`

Torrent lists:
- `fetchTorrents()` loads at most 2000 rows with server-side filters, then rebuilds `filterOptions`. On failure it sets `error` and rethrows.
- `fetchFilterOptionRows()`
- `fetchTransferInfo()`

Adding:
- `createTorrent(payload)`
- `addTorrent(url, opts)`
- `addTorrentFile(file, opts)`

Actions:
- `action(action, hashes = selection, extra = {})` covers pause, resume, delete, recheck, reannounce, set-category, set-tags and queue/*.
- `fetchLabels()`
- `detail(hash, kind: 'properties'|'files'|'trackers'|'peers'|'piece-states')` caches its results.
- `setFilePriority(hash, id, prio)`
- `preferences()`, `savePreferences(p)`
- `setSpeedLimits(hashes, dl, ul)`, `setShareLimits(hashes, ratio, seedTime)`
- `postTorrentBody(action, body)`
- `renameTorrent(hash, name)`, `renameFile(hash, old, new)`, `renameFolder(hash, old, new)`
- `setLocation(hashes, loc)`
- `addPeers(hash, peers)`, `banPeers(peers)`
- `crossSeed(hash, targetInstanceId, savePath?)`

Local state:
- `clearFilters()`
- `invalidateDetail(hash, kind?)`
- `pruneSelection()`
- `initWebSocket()`, which the root layout calls
- `toggle(hash)`, `selectOnly(hashes)`, `selectedForCurrentInstance()`, `clearSelection()`

**Types:** `QbitInstance`, `QbitInstanceInput`, `QbitTransferInfo`, `QbitTransferHealth`, `QbitActionResult`, `QbitTorrentFilters`, `QbitFilterOptions`, `QbitTorrentAction`, `QbitDetailKind`, `QbitAddTorrentOptions`, `QbitCreateTorrentPayload`, `QbitPreferences`.

The deployed torrents page persists the `qbit-delete-files` and `qbit-delete-files-remember` keys in `localStorage`. That is page code, not part of this module.

### lib/components/torrents/torrentFormat.ts (new; pure)
**Formatting:**
- `formatTorrentBytes(n)` uses 1 decimal and "0 B" for zero.
- `formatTorrentSpeed(n)` returns "—" when idle.
- `formatTorrentEta(s)` returns "—" when the value is ≥ 8640000.
- `formatTorrentProgress(0..1)`
- `formatTorrentDate(unix)`

**Peers:** `pickField(obj, keys, fallback='—')`, `peerList(payload)`, `peerEntries(payload)`, `peerEndpoint(peer)`, `peerFlags(peer)`.

**Trackers:** `trackerStatusLabel(code)`, `trackerStatusTone(code)` (`'ok'|'warn'|'error'|'muted'`), `trackerHost(url)`.

**Pieces:** `downsamplePieces(states, max=160)` and `pieceStateClass(state)` (`'done'|'partial'|'empty'`).

**Paths:** `fileBaseName(path)` and `parentPath(path)`.

### lib/components/smart-search/resultUtils.ts (new; pure)
**Classifying and ranking:**
- `resultSource(raw)`: torbox is `debrid` when cached and `torrent` otherwise.
- `matchesSourceFilter(raw, filter)`
- `collectResultFiles(response)`
- `hasSmartSearchResults(response)`
- `rankReleases(files)`

**Summaries:**
- `releaseSummary(raw|null) => {fit, quality, health, size}`
- `seasonPackageReleases(season, filter, episode?)`
- `seasonsHaveReleases(seasons, filter)`
- `isSingleFileRelease(r)`
- `episodeReleaseRow(seasonNum, group, filter, downloadedKeys) => EpisodeReleaseRow`

**Normalising and building requests:**
- `normalizeSearchResult(raw, kind='movie') => NormalizedSearchResult`
- `debridStrategy(r)`
- `acquisitionFields(r) => AcquisitionFields`: spread this into an `AddDownloadRequest`.
- `downloadUrl(r)`

**Display:**
- `resultSourceMeta(r)`
- `resultBadges(r) => ResultBadge[]`
- `formatResultSize(n)`
- `availableSourceTabs(response)`

Types: `RawSearchResult`, `Availability`, `ResultSource`, `SourceFilter`, `ReleaseSummary`, `EpisodeReleaseRow`, `NormalizedSearchResult`, `AcquisitionFields`, `ResultBadge`.

The deployed chunk exports a subset to other chunks. That subset is `resultSource`, `hasSmartSearchResults`, `formatResultSize`, `acquisitionFields`, `downloadUrl`, `normalizeSearchResult`, `resultBadges` and `resultSourceMeta`. The rest is used by the smart-search components inside BTaAoaTH.

### lib/components/fshare/fsharePayload.ts (new; pure)
- `normalizeFshareFileUrl(url)` returns `https://www.fshare.vn/file/<code>` and throws "Use an HTTPS Fshare file page URL" for anything else.
- `buildFshareDownloadPayload(release, url, filename, category, tmdb?) => AddDownloadRequest`
- `withEpisodeTarget(tmdb, {season, episode})`. Node 0 uses it.

### lib/components/smart-grab/smartGrabUtils.ts (new)
- `buildSmartGrabPlan(seasons, {targetSeason?, existingDownloads?, skipDownloaded?}) => SmartGrabPlan`. A `SmartGrabPlan` contains:
  - `quickGrabScenarios`
  - `qualitySets`, `releaseGroupSets`, `patternSets`
  - `quickGrabInfo`
  - `totalAvailableEpisodes`, `allExpectedEpisodes`, `tmdbEpisodeCount`, `airedEpisodes`, `downloadedCount`
- `buildQuickGrabScenarios(seasons, availableEpisodes, downloadedKeys)`
- `buildEpisodeDownloadRequest(entry, target, priority, batchId, batchName) => AddDownloadRequest`. The category is always `tv`.
- `episodeKey(s, e)` returns a string like "S01E02".
- `detectQuality(name)`, `detectReleaseGroup(name)`, `filenamePattern(name)`
- `coverageGradient(pct)`

Types: `SmartGrabFile`, `SmartGrabSet`, `QuickGrabScenario`, `QuickGrabScenarioKind`, `ScenarioFile`, `QualityTier`, `SmartGrabOptions`, `SmartGrabPlan`, `SmartGrabTarget`.

### lib/utils/mediaIdentity.ts (new; pure)
- `resolveMediaIdentity(item) => MediaIdentity|null`. `MediaIdentity` is either `{type:'movie', tmdbId}` or `{type:'tv', tvdbId, tmdbBridge}`. A TV item without a TVDB id resolves to `null`.
- `mediaIdentityHref(identity)` returns `/tv/:tvdbId`, `/movie/:tmdbId` or `null`.
- `buildTvTmdbMetadata({tmdbId, tvdbId, title, year, season, episode}) => TvTmdbMetadata`

### lib/utils/clipboard.ts (new)
- `copyToClipboard(text) => Promise<boolean>`. It uses the Clipboard API and falls back to a hidden textarea with `execCommand('copy')`. The settings page and the downloads page use it.

### lib/services/tmdb.ts
- **new** `resolveTvRoute(id) => Promise<TvRouteResolution|null>` calls GET `/api/tmdb/tv/resolve/:id`. `TvRouteResolution` = `{canonical_tvdb_id, legacy_tmdb_id, is_canonical_tv_route}`.
- These are unchanged and used: `getMovieDetails`, `getTVShowDetails`, `getSeasonDetails`, `getCollectionDetails`, `getPosterUrl`, `getBackdropUrl`, `getYear`, `getSimilar` and `getRecommendations`.
- These are kept but not referenced: `getTrendingMovies/TV`, the placeholder helpers and `searchTMDB`.

### routes/torrents/+page.ts (new)
`export const load: PageLoad = () => {}`. The deployed node has an empty universal `load`.

## Inlined helpers left to the page agents (noted, not reconstructed)
- **nodes/7 (downloads):** lines 222-536 hold a `STATUS_PRIORITY` copy and these helpers:
  - colour and icon per state (`#00ffa3`, `#FF5252`, `#00f3ff`, `#ffd700`)
  - `normalizeSource` (qbittorrent→torrent, flasharr-http→fshare, torbox→debrid)
  - copy-label helpers
  - the `direct` filter logic
  - batch-group builders that call `aggregateQueueSource`
  - a mixed-batch status reducer

  These are probably a `lib/components/downloads/*.ts` helper module or the page script. The downloads table and the mobile view share them.
- **nodes/11 (settings):** `const Tt` is the settings tab list (general, accounts, download, integrations, sources, logs). The notes mention `lib/components/settings/settingsTabs.ts` and `accountRows.ts` (both have tests), so these are probably those modules inlined.
- **nodes/0 (layout):** the theme store is inlined (unchanged `stores/theme.ts`). The SmartSearchModal logic (season plan status) is inside its component code.
- **nodes/12:** the inlined setup store matches the baseline `stores/setup.svelte.ts` exactly.

## Baseline files that break against this branch (for the page agents)
`svelte-check` reports 11 errors, all in baseline `.svelte` files and none in the TS modules. `vite build` fails first on `routes/search`.
- `routes/search/+page.svelte` imports `$lib/stores/query`, which was removed. The `/search` route is gone in the deployed build, so delete the route.
- `routes/+page.svelte`, `routes/calendar/+page.svelte` and `routes/library/+page.svelte` import `integrations` from `settings`. That store was removed; use `sonarrConfigured`, `radarrConfigured`, `arrConfigured` and `integrationStatus` from `system` instead.
- `lib/components/AccountWarningModal.svelte` and `routes/settings/+page.svelte` call `accountStore.switchAccount`. Use `accountStore.replace(email, pw)`, which returns `AccountMutationResult`.
- `routes/downloads/+page.svelte` passes `batch_id` (now `string|null|undefined`) where a `string` is expected (5 places). The page must be rebuilt around mediaQueue and the source filter anyway.
- These semantic changes produce no type error:
  - `routes/tv/[id]` calls `findSeriesInList(allSeries, tmdbId)`. The second argument is now the TVDB id; the deployed call is `findSeriesInList(series, tvdbId, tmdbBridge)`.
  - Callers of `settingsStore.add/remove/refreshAccount` get an object back instead of a boolean.
  - `batchItems` keys are now `"<batch_id>|<sourceFilter>"`.
