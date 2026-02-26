# Commit Message Convention

Flasharr follows **[Conventional Commits](https://www.conventionalcommits.org/)** for all git commits.

## Format

```
<type>(<scope>): <description>

[optional body]
```

## Types

| Type       | When to use                            | Version bump          |
| ---------- | -------------------------------------- | --------------------- |
| `feat`     | New feature or capability              | Minor (1.0.0 → 1.1.0) |
| `fix`      | Bug fix                                | Patch (1.0.0 → 1.0.1) |
| `docs`     | Documentation only                     | None                  |
| `style`    | Formatting, no code change             | None                  |
| `refactor` | Code restructuring, no behavior change | None                  |
| `perf`     | Performance improvement                | Patch                 |
| `test`     | Adding or updating tests               | None                  |
| `build`    | Build system, Docker, CI changes       | None                  |
| `ci`       | GitHub Actions, workflow changes       | None                  |
| `chore`    | Dependencies, maintenance              | None                  |

**Breaking changes**: Add `!` after type → `feat!:` or `fix!:` → Major bump (1.0.0 → 2.0.0)

## Scopes (optional)

| Scope      | Area                      |
| ---------- | ------------------------- |
| `backend`  | Rust backend              |
| `frontend` | SvelteKit frontend        |
| `api`      | REST/WebSocket API        |
| `search`   | Search functionality      |
| `download` | Download engine           |
| `arr`      | Sonarr/Radarr integration |
| `docker`   | Docker/deployment         |
| `docs`     | Documentation             |

## Examples

```bash
feat(search): add infinite scroll to search results
fix(download): resolve retry loop on expired links
docs: update installation guide with TMDB clarification
refactor(backend): extract search pipeline into module
feat!: change API authentication to token-based
build(docker): optimize multi-stage build
ci: add nightly build schedule
perf(api): cache TMDB responses for 24h
```

## Rules

1. **Lowercase** — type and description start lowercase
2. **No period** — don't end the description with `.`
3. **Imperative mood** — "add feature" not "added feature"
4. **Max 72 chars** — first line should be under 72 characters
5. **Body for context** — use body for "why", not "what" (the code shows what)
