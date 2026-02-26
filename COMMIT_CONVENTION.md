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

---

## Automatic Versioning Flow

This project uses **[release-please](https://github.com/googleapis/release-please)** to automate versioning. Your commit messages directly control what happens:

### How It Works

```
You commit with conventional prefix
        ↓
Push to main (or use /ship)
        ↓
release-please reads ALL commits since last release
        ↓
Creates a "Release PR" with:
  • Auto-calculated version bump
  • Auto-generated CHANGELOG.md
  • Updated version in Cargo.toml + package.json
        ↓
You merge the Release PR (or use /ship-stable)
        ↓
Git tag v1.1.0 created automatically
        ↓
GitHub Actions builds Docker images:
  • ghcr.io/duytran1406/flasharr:v1.1.0
  • ghcr.io/duytran1406/flasharr:stable
  • ghcr.io/duytran1406/flasharr:latest
```

### Version Bump Rules

| Commits since last release         | Result                |
| ---------------------------------- | --------------------- |
| Only `fix:`, `docs:`, `chore:`     | Patch (1.0.0 → 1.0.1) |
| At least one `feat:`               | Minor (1.0.0 → 1.1.0) |
| Any `feat!:` or `BREAKING CHANGE:` | Major (1.0.0 → 2.0.0) |
| Only `docs:`, `style:`, `ci:`      | No release created    |

### Agent Workflows

| Command        | What it does                       | Docker tags                  |
| -------------- | ---------------------------------- | ---------------------------- |
| `/ship`        | Commit + push to main              | `latest`, `nightly`          |
| `/ship-stable` | Commit + version bump + tag + push | `stable`, `vX.Y.Z`, `latest` |
| _(automatic)_  | Nightly build at 2 AM UTC          | `nightly`                    |
