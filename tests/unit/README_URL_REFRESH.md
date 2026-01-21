# URL Refresh Unit Tests

## Overview
This test suite validates the URL refresh functionality that allows the system to regenerate expired Fshare direct download URLs when resuming failed or paused downloads.

## Test Coverage

### 1. **test_refresh_download_url_success**
- **Purpose**: Verify successful URL refresh from original Fshare URL
- **Scenario**: Task has expired direct URL and valid original URL
- **Expected**: New direct URL is fetched and task is updated

### 2. **test_refresh_download_url_no_original_url**
- **Purpose**: Ensure graceful handling when no original URL is saved
- **Scenario**: Task has no `original_url` field
- **Expected**: Refresh fails without attempting API call

### 3. **test_refresh_download_url_resolution_fails**
- **Purpose**: Test error handling when Fshare API fails to resolve URL
- **Scenario**: Fshare API returns None (resolution failed)
- **Expected**: Error message is set and task is updated

### 4. **test_refresh_download_url_exception_handling**
- **Purpose**: Verify exception handling during URL resolution
- **Scenario**: Network error or API exception occurs
- **Expected**: Exception is caught, error message is set

### 5. **test_resume_download_triggers_url_refresh**
- **Purpose**: Confirm that resuming a failed download triggers URL refresh
- **Scenario**: User resumes a failed download with expired URL
- **Expected**: URL is refreshed before resume attempt

### 6. **test_resume_download_skips_refresh_for_active_downloads**
- **Purpose**: Ensure active downloads don't trigger unnecessary URL refresh
- **Scenario**: User resumes an already-downloading task
- **Expected**: No URL refresh is performed

### 7. **test_add_download_saves_original_url**
- **Purpose**: Verify original URL is saved when adding new downloads
- **Scenario**: User adds a new Fshare download
- **Expected**: Both direct URL and original URL are saved

## Running the Tests

### Run all URL refresh tests:
```bash
pytest tests/unit/test_url_refresh.py -v
```

### Run a specific test:
```bash
pytest tests/unit/test_url_refresh.py::TestURLRefresh::test_refresh_download_url_success -v
```

### Run with coverage:
```bash
pytest tests/unit/test_url_refresh.py --cov=flasharr.downloader.builtin_client --cov-report=html
```

## Debug Mode: Stop at 1%

To save daily quota during testing, you can enable automatic pause at 1% progress:

### Enable Debug Mode:
```bash
# In .env file
DEBUG_STOP_AT_1_PERCENT=true
```

### Or set environment variable:
```bash
export DEBUG_STOP_AT_1_PERCENT=true
docker-compose up -d
```

### What happens:
1. Download starts normally
2. When progress reaches ≥1%, download is automatically paused
3. Task state changes to `PAUSED`
4. Error message shows: "DEBUG: Auto-paused at 1% (DEBUG_STOP_AT_1_PERCENT enabled)"
5. You can resume the download later (URL will be refreshed if expired)

### Disable Debug Mode:
```bash
# In .env file
DEBUG_STOP_AT_1_PERCENT=false
```

## Integration with Resume Logic

The URL refresh logic is automatically triggered when:
1. User clicks "Resume" on a failed/paused download
2. Task has an `original_url` saved
3. Task state is `FAILED` or `PAUSED`

The refresh process:
1. Checks if original URL exists
2. Calls Fshare API to get new direct download URL
3. Updates task with fresh URL
4. Clears any previous error messages
5. Persists changes to database
6. Resumes the download

## Mock Objects

The tests use mocks for:
- **FshareClient**: Simulates Fshare API calls
- **DownloadEngine**: Simulates download engine operations
- **FshareDownloadHandler**: Simulates URL resolution
- **DownloadQueue**: Simulates database operations

## Dependencies

- `pytest`: Test framework
- `pytest-asyncio`: Async test support
- `unittest.mock`: Mocking framework

## Expected Behavior

### Successful URL Refresh Flow:
```
1. User resumes failed download
2. System checks: original_url exists? ✓
3. System calls: Fshare API with original_url
4. Fshare returns: New direct download URL
5. System updates: task.url = new_direct_url
6. System clears: task.error_message = None
7. System saves: Update to database
8. Download resumes with fresh URL
```

### Failed URL Refresh Flow:
```
1. User resumes failed download
2. System checks: original_url exists? ✓
3. System calls: Fshare API with original_url
4. Fshare returns: None (failed)
5. System sets: error_message = "Failed to refresh..."
6. System saves: Update to database
7. Resume fails, user sees error message
```

## Notes

- All tests are async and use `pytest.mark.asyncio`
- Mocks are properly configured to avoid real API calls
- Tests verify both success and failure paths
- Error messages are validated for user-facing clarity
