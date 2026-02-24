# Flasharr Backend Refactoring - February 2026

## Overview

This document summarizes the architectural refactoring performed on the Flasharr backend to improve code quality, testability, and maintainability.

## Problem Statement

The initial code review identified several critical issues:

- **God Object**: `orchestrator.rs` at 1553 lines with too many responsibilities
- **Monolithic Functions**: `handle_tv_search` at 580+ lines
- **Low Test Coverage**: ~5% estimated coverage
- **Scattered Configuration**: TMDB API key hardcoded in constants

## Changes Summary

### Phase 1: Quick Wins

| Change                        | Impact                                       |
| ----------------------------- | -------------------------------------------- |
| Added `ExternalConfig` struct | TMDB API key now configurable via env var    |
| Deleted `batch_progress.rs`   | Removed 95 lines of dead code                |
| Added `get_tmdb_api_key()`    | Supports `TMDB_API_KEY` environment variable |

### Phase 2: Orchestrator Decomposition

**New modules created:**

```
backend/src/downloader/
├── path_builder.rs       # 110 lines - destination path construction
├── duplicate_detector.rs # 125 lines - Fshare duplicate handling
```

**Key extractions:**

- `TmdbDownloadMetadata` moved to canonical location in `path_builder.rs`
- `PathBuilder::build_destination_path()` - organizes downloads by media type
- `PathBuilder::sanitize_filename()` - safe filename generation
- `DuplicateDetector::extract_fshare_code()` - URL parsing
- `DuplicateDetector::find_task_by_fshare_code()` - duplicate detection

**Result**: `orchestrator.rs` reduced from 1553 → 1458 lines (-95)

### Phase 3: Search Refactoring

**New module created:**

```
backend/src/api/
└── search_pipeline.rs    # 175 lines - reusable search components
```

**Key extractions:**

- `RawFshareResult` struct - search result data model
- `TmdbEnrichment` struct - TMDB metadata container
- `SearchPipeline::execute_fshare_search()` - timfshare.com API client
- `SearchPipeline::fetch_tv_enrichment()` - TV metadata fetching
- `SearchPipeline::fetch_movie_enrichment()` - Movie metadata fetching
- `SearchPipeline::deduplicate_by_fcode()` - result deduplication

**Result**: `smart_search.rs` reduced from 1041 → 991 lines (-50)

## Metrics

| File                     | Before | After | Δ   |
| ------------------------ | ------ | ----- | --- |
| `orchestrator.rs`        | 1553   | 1458  | -95 |
| `smart_search.rs`        | 1041   | 991   | -50 |
| **New modules (P1-3)**   | 0      | 4     | +4  |
| **Lines in new modules** | -      | ~410  | -   |

### Phase 4: Service Layer ✅

**New modules created:**

```
backend/src/services/
├── mod.rs               # 9 lines - module exports
├── download_service.rs  # 307 lines - business logic layer
```

```
backend/src/utils/
├── batch_utils.rs       # 157 lines - batch progress/state aggregation
├── status_utils.rs      # 128 lines - status count utilities
```

**Key extractions:**

- `DownloadService` - abstracts DB operations from API handlers
  - `get_task()`, `pause_task()`, `resume_task()`, `delete_task()`
  - `pause_batch()`, `resume_batch()`, `delete_batch()`
  - `get_status_counts()`, `merge_realtime_progress()`
- `BatchStats::from_tasks()` - reusable batch statistics calculation
- `StatusCounts::from_db_counts()` - reusable status aggregation
- `aggregate_batch_state()` - batch state determination

**Tests added:** 7 unit tests (4 batch_utils, 3 status_utils)

### Phase 5: Domain Errors ✅

**New module created:**

```
backend/src/
├── error.rs             # 145 lines - typed error system
```

**Key additions:**

- `FlasharrError` enum with 15+ variants:
  - `DownloadNotFound`, `DownloadAlreadyExists`, `DownloadInvalidState`
  - `BatchNotFound`, `BatchEmpty`
  - `Database`, `DatabaseConnection`
  - `HostNotFound`, `HostAuthFailed`, `HostRateLimited`
  - `InvalidUuid`, `InvalidRequest`
  - `TmdbError`, `FshareError`, `ArrServiceError`, `Internal`
- Automatic HTTP status code mapping via `IntoResponse`
- Conversions from `rusqlite::Error`, `uuid::Error`
- `FlasharrResult<T>` type alias

## Updated Metrics

| File                    | Lines | Purpose                  |
| ----------------------- | ----- | ------------------------ |
| `error.rs`              | 145   | Domain error types       |
| `services/mod.rs`       | 9     | Service module exports   |
| `download_service.rs`   | 307   | Business logic layer     |
| `utils/batch_utils.rs`  | 157   | Batch utilities + tests  |
| `utils/status_utils.rs` | 128   | Status utilities + tests |
| **Total new lines**     | ~746  | -                        |

## Related Files

- [path_builder.rs](../../backend/src/downloader/path_builder.rs)
- [duplicate_detector.rs](../../backend/src/downloader/duplicate_detector.rs)
- [search_pipeline.rs](../../backend/src/api/search_pipeline.rs)
- [error.rs](../../backend/src/error.rs) - FlasharrError enum
- [download_service.rs](../../backend/src/services/download_service.rs)
- [batch_utils.rs](../../backend/src/utils/batch_utils.rs)
- [status_utils.rs](../../backend/src/utils/status_utils.rs)
