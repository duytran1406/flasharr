---
name: QA Testing
description: Testing strategies and procedures for Flasharr
---

# QA Testing Skill

## Overview

This skill provides testing strategies, procedures, and best practices for quality assurance on Flasharr.

## Testing Types

### 1. Manual Testing

- Feature verification
- User journey testing
- Cross-browser testing
- Responsive testing

### 2. API Testing

```bash
# Health check
curl http://localhost:3000/health

# Test endpoints
curl http://localhost:3000/api/movies
curl http://localhost:3000/api/search?q=test
```

### 3. UI Testing

- Visual regression testing
- Interaction testing
- Accessibility testing

## Bug Report Template

```markdown
## Bug Report: [ID]

**Summary**: Brief description
**Severity**: Critical/High/Medium/Low
**Steps to Reproduce**:

1. Step 1
2. Step 2
3. Step 3

**Expected Result**: What should happen
**Actual Result**: What actually happens
**Screenshots**: [Attach if applicable]
**Environment**: Browser, OS, Version
```

## Test Scenarios

### User Journey Tests

1. Search for a movie
2. View movie details
3. Initiate download
4. Monitor download progress
5. Access downloaded content

### Edge Cases

- Empty search results
- Network failure handling
- Large file downloads
- Concurrent operations

## Log Locations

- Backend: `debug_log/run.log`
- Frontend: `debug_log/frontend-dev.log`
- Build: `debug_log/*-build.log`

## Bug Reporting Location

- File: `QA_BUG_REPORT.md`
- Format: Use template above
