# Flasharr Development Rules

**Version:** 0.0.3-beta  
**Last Updated:** 2026-01-14

This document defines the development rules, standards, and best practices for Flasharr.

---

## Version Control

### Semantic Versioning

Flasharr follows [Semantic Versioning 2.0.0](https://semver.org/):

**Format:** `MAJOR.MINOR.PATCH[-PRERELEASE]`

- **MAJOR:** Incompatible API changes
- **MINOR:** New functionality (backwards compatible)
- **PATCH:** Bug fixes (backwards compatible)
- **PRERELEASE:** `-alpha`, `-beta`, `-rc.1`

**Examples:**
- `0.0.3-beta` - Current beta version
- `0.1.0` - First stable minor release
- `1.0.0` - First major stable release

### Version Increment Rules

| Change Type | Version Bump | Example |
|-------------|--------------|---------|
| Breaking API change | MAJOR | `0.0.3` → `1.0.0` |
| New feature | MINOR | `0.0.3` → `0.1.0` |
| Bug fix | PATCH | `0.0.3` → `0.0.4` |
| Pre-release | PRERELEASE | `0.0.3-beta` → `0.0.4-beta` |

---

## Git Workflow

### Branch Strategy

- **`main`** - Stable production code
- **`develop`** - Integration branch for features
- **`feature/*`** - Feature branches
- **`hotfix/*`** - Urgent fixes for production
- **`release/*`** - Release preparation

### Commit Message Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types:**
- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation only
- `style` - Code style (formatting, no logic change)
- `refactor` - Code refactoring
- `perf` - Performance improvement
- `test` - Adding/updating tests
- `chore` - Maintenance tasks

**Examples:**
```
feat(downloader): add multi-threaded download support

Implemented segmented downloads using asyncio for improved speed.
Each file is split into configurable segments downloaded concurrently.

Closes #42
```

```
fix(auth): handle session expiration gracefully

Previously, expired sessions caused downloads to fail silently.
Now automatically re-authenticates and retries.

Fixes #58
```

---

## Code Standards

### Python Style

- Follow [PEP 8](https://peps.python.org/pep-0008/)
- Use [Black](https://github.com/psf/black) for formatting
- Use [isort](https://pycqa.github.io/isort/) for import sorting
- Use [flake8](https://flake8.pycqa.org/) for linting

### Type Hints

Always use type hints:

```python
from typing import Optional, List

def get_download_link(fcode: str) -> Optional[str]:
    """Get direct download link for Fshare file."""
    pass

async def search(query: str, limit: int = 100) -> List[SearchResult]:
    """Search for files."""
    pass
```

### Docstrings

Use Google-style docstrings:

```python
def add_download(url: str, filename: str, priority: Priority = Priority.NORMAL) -> DownloadTask:
    """Add a new download to the queue.
    
    Args:
        url: Fshare download URL
        filename: Destination filename
        priority: Download priority level
        
    Returns:
        DownloadTask object with task details
        
    Raises:
        InvalidURLError: If URL is not a valid Fshare link
        QuotaExceededError: If all accounts have exceeded quota
    """
    pass
```

---

## File Organization

### Module Structure

```
src/flasharr/
├── __init__.py          # Package initialization
├── __main__.py          # CLI entry point
├── api/                 # API endpoints
├── clients/             # External service clients
├── core/                # Core business logic
├── downloader/          # Download engine
├── services/            # Service layer
├── utils/               # Utility functions
├── web/                 # Web interface
└── websocket/           # WebSocket handlers
```

### Naming Conventions

- **Files:** `snake_case.py`
- **Classes:** `PascalCase`
- **Functions:** `snake_case`
- **Constants:** `UPPER_SNAKE_CASE`
- **Private:** `_leading_underscore`

---

## Testing

### Test Coverage

- Minimum 80% code coverage
- All public APIs must have tests
- Critical paths must have integration tests

### Test Structure

```
tests/
├── unit/                # Unit tests
│   ├── test_clients.py
│   ├── test_downloader.py
│   └── test_services.py
├── integration/         # Integration tests
│   ├── test_api.py
│   └── test_workflow.py
└── conftest.py          # Pytest fixtures
```

### Running Tests

```bash
# All tests
pytest

# With coverage
pytest --cov=flasharr --cov-report=html

# Specific test
pytest tests/unit/test_downloader.py

# Integration tests only
pytest tests/integration/
```

---

## Release Process

### 1. Prepare Release

```bash
# Create release branch
git checkout -b release/v0.0.4-beta

# Update version in VERSION file
echo "v0.0.4-beta" > VERSION

# Update changelog
nano flasharr_docs/development/changelog.md
```

### 2. Update Documentation

- Update version references
- Update changelog with changes
- Review all documentation for accuracy

### 3. Test Release

```bash
# Run full test suite
pytest

# Build Docker image
docker-compose build

# Test deployment
docker-compose up -d
```

### 4. Create Release

```bash
# Commit changes
git add VERSION flasharr_docs/development/changelog.md
git commit -m "chore: bump version to 0.0.4-beta"

# Merge to main
git checkout main
git merge release/v0.0.4-beta

# Tag release
git tag -a v0.0.4-beta -m "Release v0.0.4-beta"

# Push
git push origin main --tags
```

### 5. Deploy

```bash
# Pull on production server
git pull origin main

# Rebuild and restart
docker-compose build
docker-compose up -d
```

---

## Documentation Rules

### Required Documentation

Every feature must include:

1. **Code documentation** - Docstrings and comments
2. **API documentation** - If adding/changing APIs
3. **User guide** - If user-facing feature
4. **Changelog entry** - All changes

### Documentation Location

- **API docs:** `flasharr_docs/api-reference/`
- **User guides:** `flasharr_docs/user-guide/`
- **Architecture:** `flasharr_docs/architecture/`
- **Development:** `flasharr_docs/development/`

---

## Code Review

### Review Checklist

- [ ] Code follows style guidelines
- [ ] All tests pass
- [ ] New tests added for new features
- [ ] Documentation updated
- [ ] No security vulnerabilities
- [ ] Performance acceptable
- [ ] Error handling comprehensive
- [ ] Logging appropriate

### Review Process

1. Create pull request
2. Automated tests run
3. Code review by maintainer
4. Address feedback
5. Approval and merge

---

## Security Rules

### Credentials

- Never hardcode credentials
- Use environment variables
- Never commit `.env` files
- Rotate credentials regularly

### Dependencies

- Keep dependencies updated
- Review security advisories
- Use `pip-audit` for vulnerability scanning

### Input Validation

- Validate all user inputs
- Sanitize file paths
- Validate URLs before processing

---

## Performance Guidelines

### Async/Await

- Use `async`/`await` for I/O operations
- Don't block the event loop
- Use `asyncio.gather()` for concurrent operations

### Database

- Use connection pooling
- Index frequently queried fields
- Vacuum database periodically

### Caching

- Cache expensive operations
- Use TTL for cache entries
- Clear cache on updates

---

## Deprecation Policy

### Marking Deprecated

```python
import warnings

def old_function():
    warnings.warn(
        "old_function is deprecated, use new_function instead",
        DeprecationWarning,
        stacklevel=2
    )
    return new_function()
```

### Deprecation Timeline

1. **Version N:** Mark as deprecated, add warning
2. **Version N+1:** Keep with warning
3. **Version N+2:** Remove deprecated code

---

## Breaking Changes

### Communication

- Document in changelog
- Add migration guide
- Announce in release notes
- Update version (MAJOR bump)

### Migration Support

- Provide migration scripts if possible
- Document manual migration steps
- Support old version for transition period

---

## Continuous Integration

### GitHub Actions (Recommended)

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions/setup-python@v2
        with:
          python-version: '3.9'
      - run: pip install -r requirements.txt
      - run: pytest --cov=flasharr
```

---

## Issue Management

### Issue Labels

- `bug` - Something isn't working
- `enhancement` - New feature request
- `documentation` - Documentation improvement
- `good first issue` - Good for newcomers
- `help wanted` - Extra attention needed
- `priority: high` - High priority
- `wontfix` - Will not be fixed

### Issue Template

```markdown
**Description:**
Clear description of the issue

**Steps to Reproduce:**
1. Step 1
2. Step 2
3. ...

**Expected Behavior:**
What should happen

**Actual Behavior:**
What actually happens

**Environment:**
- Flasharr version:
- Docker version:
- OS:
```

---

## Next Steps

- [Contributing Guide](contributing.md)
- [Deployment Guide](deployment.md)
- [Changelog](changelog.md)
