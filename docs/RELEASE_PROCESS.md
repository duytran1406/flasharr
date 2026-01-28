# Flasharr Release Process

This document outlines the release process for Flasharr maintainers.

## Quick Release

Use the automated release script:

```bash
./scripts/release.sh
```

This will:

1. Check you're on the main branch
2. Verify no uncommitted changes
3. Prompt for new version number
4. Update version in `Cargo.toml` and `package.json`
5. Update `CHANGELOG.md`
6. Create a git commit and tag
7. Optionally push to GitHub

## Manual Release Process

### 1. Prepare the Release

**Update version numbers:**

```bash
# backend/Cargo.toml
version = "2.1.0"

# frontend/package.json
"version": "2.1.0"
```

**Update CHANGELOG.md:**

```markdown
## [2.1.0] - 2026-01-28

### Added

- New feature X
- New feature Y

### Changed

- Improved Z

### Fixed

- Bug fix A
```

### 2. Commit and Tag

```bash
git add backend/Cargo.toml frontend/package.json CHANGELOG.md
git commit -m "Release v2.1.0"
git tag -a v2.1.0 -m "Release v2.1.0"
```

### 3. Push to GitHub

```bash
git push origin main --tags
```

### 4. GitHub Actions Takes Over

The workflow will automatically:

- Build Docker images for AMD64 and ARM64
- Tag images as:
  - `v2.1.0` (exact version)
  - `v2.1` (minor version)
  - `v2` (major version)
  - `stable` (latest stable)
  - `latest` (latest overall)
- Create a GitHub release with release notes
- Publish to GitHub Container Registry

### 5. Verify Release

```bash
# Check the published image
docker pull ghcr.io/duytran1406/flasharr:v2.1.0

# Verify version
./scripts/check-version.sh
```

## Release Types

### Patch Release (2.0.0 → 2.0.1)

Bug fixes only, no new features.

```bash
./scripts/release.sh
# Enter: 2.0.1
```

### Minor Release (2.0.1 → 2.1.0)

New features, backward compatible.

```bash
./scripts/release.sh
# Enter: 2.1.0
```

### Major Release (2.1.0 → 3.0.0)

Breaking changes, may require migration.

```bash
./scripts/release.sh
# Enter: 3.0.0
```

**Additional steps for major releases:**

1. Create migration guide in `docs/MIGRATION.md`
2. Update documentation for breaking changes
3. Announce in Discord/Reddit/etc.

### Pre-release (Beta/RC)

```bash
./scripts/release.sh
# Enter: 2.1.0-beta.1
```

Pre-releases are automatically marked as such on GitHub.

## Hotfix Process

For urgent fixes to production:

1. **Create hotfix branch:**

   ```bash
   git checkout -b hotfix/2.0.2 v2.0.1
   ```

2. **Make the fix and commit:**

   ```bash
   git commit -m "Fix critical bug X"
   ```

3. **Update version and release:**

   ```bash
   ./scripts/release.sh
   # Enter: 2.0.2
   ```

4. **Merge back to main:**
   ```bash
   git checkout main
   git merge hotfix/2.0.2
   git push origin main --tags
   ```

## Nightly Builds

Nightly builds are automatically created daily at 2 AM UTC from the main branch.

To trigger a manual nightly build:

```bash
# Just push to main
git push origin main
```

The `nightly` tag will be updated automatically.

## Version Checking

### Check local version:

```bash
grep '^version = ' backend/Cargo.toml
```

### Check running instance:

```bash
./scripts/check-version.sh
# or for remote instance:
./scripts/check-version.sh remote-host:8484
```

### Check published images:

```bash
docker pull ghcr.io/duytran1406/flasharr:latest
docker inspect ghcr.io/duytran1406/flasharr:latest | grep -A 5 Labels
```

## Rollback

If a release has issues:

### 1. Revert the tag:

```bash
git tag -d v2.1.0
git push origin :refs/tags/v2.1.0
```

### 2. Delete the GitHub release:

Go to GitHub → Releases → Delete release

### 3. Users can downgrade:

```bash
docker pull ghcr.io/duytran1406/flasharr:v2.0.1
```

## Post-Release Checklist

- [ ] Verify Docker images are published
- [ ] Test installation with `install.sh`
- [ ] Update documentation if needed
- [ ] Announce on:
  - [ ] GitHub Discussions
  - [ ] Discord
  - [ ] Reddit (r/selfhosted, r/homelab)
  - [ ] Twitter/X
- [ ] Monitor for issues

## Troubleshooting

### GitHub Actions failed?

1. Check the Actions tab on GitHub
2. Review build logs
3. Fix the issue
4. Delete the tag and try again:
   ```bash
   git tag -d v2.1.0
   git push origin :refs/tags/v2.1.0
   git tag -a v2.1.0 -m "Release v2.1.0"
   git push origin v2.1.0
   ```

### Image not appearing?

- Check GitHub Packages permissions
- Verify GITHUB_TOKEN has package write access
- Check if the workflow completed successfully

### Wrong version published?

1. Delete the release on GitHub
2. Delete the tag
3. Fix version numbers
4. Re-release with correct version

---

**Questions?** Ask in the maintainers channel on Discord.
