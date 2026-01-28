---
description: Create and run unit tests for recent changes
---

# Unit Testing Helper

Use this workflow to ensure quality by creating and running tests for your code changes.

## Steps

1. **Identify Changed Components**
   Check git status to see which files have been modified.

   // turbo

   ```bash
   git status -s
   ```

2. **Create/Update Tests**
   - For a Python module `src/flasharr/foo.py`, check/create `tests/test_foo.py`.
   - Ensure you cover:
     - Success paths (happy path)
     - Error handling (edge cases)
     - Logic verification

3. **Run Tests**
   Execute pytest to verify the changes.

   ```bash
   pytest
   ```

   _If you want to run only specific tests:_

   ```bash
   pytest tests/test_specific_file.py
   ```
