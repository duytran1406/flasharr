---
name: Rust Backend Development
description: Skills for developing and debugging the Flasharr Rust backend
---

# Rust Backend Development Skill

## Overview

This skill provides guidance for working with the Flasharr Rust backend, including API development, database operations, and performance optimization.

## Key Directories

- `/backend/src/` - Main source code
- `/backend/src/api/` - API routes and handlers
- `/backend/src/models/` - Database models
- `/backend/src/hosts/` - External service integrations (Fshare, TMDB)
- `/backend/src/downloader/` - Download management

## Common Tasks

### Building

```bash
cd backend && cargo build          # Debug build
cd backend && cargo build --release # Release build
```

### Running Tests

```bash
cd backend && cargo test
```

### Checking for Errors

```bash
cd backend && cargo check
cd backend && cargo clippy  # Linting
```

### Database Operations

- SQLite database at `backend/flasharr.db`
- Migrations in `backend/migrations/`

## Best Practices

1. Use `Result<T, Error>` for error handling
2. Implement proper logging with `tracing`
3. Use async/await for I/O operations
4. Keep API handlers thin, business logic in services

## Debug Tips

- Check logs in `debug_log/run.log`
- Use `RUST_LOG=debug cargo run` for verbose output
- Use `cargo expand` to see macro expansions
