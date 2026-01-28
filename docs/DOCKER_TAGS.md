# Docker Image Tags

Flasharr provides multiple Docker image tags to suit different use cases.

## Available Tags

### Stable Releases

| Tag      | Description                  | Recommended For            | Update Frequency            |
| -------- | ---------------------------- | -------------------------- | --------------------------- |
| `stable` | Latest stable release        | Production                 | On version releases         |
| `latest` | Latest commit on main branch | Production (bleeding edge) | On every main branch commit |
| `v2.0.0` | Specific version             | Production (pinned)        | Never (immutable)           |
| `v2.0`   | Latest patch in v2.0.x       | Production (auto-patch)    | On patch releases           |
| `v2`     | Latest minor in v2.x.x       | Production (auto-minor)    | On minor releases           |

### Development Releases

| Tag            | Description                     | Recommended For     | Update Frequency        |
| -------------- | ------------------------------- | ------------------- | ----------------------- |
| `nightly`      | Nightly build from main         | Testing/Development | Daily at 2 AM UTC       |
| `develop`      | Latest commit on develop branch | Early testing       | On every develop commit |
| `main-abc1234` | Specific commit SHA             | Debugging           | Per commit              |

## Which Tag Should I Use?

### 🏢 **Production (Conservative)**

```yaml
image: ghcr.io/duytran1406/flasharr:stable
```

- ✅ Only updates on official releases
- ✅ Most stable and tested
- ✅ Recommended for most users

### 🚀 **Production (Latest Features)**

```yaml
image: ghcr.io/duytran1406/flasharr:latest
```

- ✅ Gets bug fixes immediately
- ✅ New features as soon as merged
- ⚠️ Slightly less tested than `stable`

### 📌 **Production (Pinned Version)**

```yaml
image: ghcr.io/duytran1406/flasharr:v2.0.0
```

- ✅ Never changes
- ✅ Reproducible deployments
- ✅ Full control over updates
- ⚠️ Manual updates required

### 🔬 **Testing/Development**

```yaml
image: ghcr.io/duytran1406/flasharr:nightly
```

- ✅ Latest features and fixes
- ✅ Help test before release
- ⚠️ May have bugs
- ⚠️ Not recommended for production

### 🧪 **Early Access**

```yaml
image: ghcr.io/duytran1406/flasharr:develop
```

- ✅ Experimental features
- ⚠️ Unstable
- ⚠️ Only for developers/testers

## Tag Behavior Examples

### When you release `v2.1.3`:

```
ghcr.io/duytran1406/flasharr:v2.1.3   ← Created (immutable)
ghcr.io/duytran1406/flasharr:v2.1     ← Updated to v2.1.3
ghcr.io/duytran1406/flasharr:v2       ← Updated to v2.1.3
ghcr.io/duytran1406/flasharr:stable   ← Updated to v2.1.3
ghcr.io/duytran1406/flasharr:latest   ← Updated to v2.1.3
```

### When you push to main branch:

```
ghcr.io/duytran1406/flasharr:latest   ← Updated
ghcr.io/duytran1406/flasharr:nightly  ← Updated
ghcr.io/duytran1406/flasharr:main-abc1234 ← Created
```

### Every night at 2 AM UTC:

```
ghcr.io/duytran1406/flasharr:nightly  ← Rebuilt from main
```

## Auto-Update Strategies

### Using Watchtower (Automatic Updates)

**For stable releases only:**

```yaml
services:
  flasharr:
    image: ghcr.io/duytran1406/flasharr:stable
    labels:
      - "com.centurylinklabs.watchtower.enable=true"

  watchtower:
    image: containrrr/watchtower
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
    environment:
      - WATCHTOWER_LABEL_ENABLE=true
      - WATCHTOWER_SCHEDULE=0 0 4 * * * # 4 AM daily
```

**For nightly builds:**

```yaml
services:
  flasharr:
    image: ghcr.io/duytran1406/flasharr:nightly
    labels:
      - "com.centurylinklabs.watchtower.enable=true"

  watchtower:
    image: containrrr/watchtower
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
    environment:
      - WATCHTOWER_LABEL_ENABLE=true
      - WATCHTOWER_SCHEDULE=0 0 6 * * * # 6 AM daily (after nightly build)
```

## Manual Update Commands

### Update to latest stable:

```bash
docker compose pull
docker compose up -d
```

### Switch to a specific version:

```bash
# Edit docker-compose.yml
image: ghcr.io/duytran1406/flasharr:v2.0.0

docker compose up -d
```

### Switch to nightly:

```bash
# Edit docker-compose.yml
image: ghcr.io/duytran1406/flasharr:nightly

docker compose pull
docker compose up -d
```

## Version Numbering

Flasharr follows [Semantic Versioning](https://semver.org/):

```
v2.1.3
│ │ │
│ │ └─ Patch: Bug fixes, no breaking changes
│ └─── Minor: New features, backward compatible
└───── Major: Breaking changes
```

### Examples:

- `v2.0.0` → `v2.0.1`: Bug fixes only
- `v2.0.1` → `v2.1.0`: New features, no breaking changes
- `v2.1.0` → `v3.0.0`: Breaking changes, migration may be required

## Pre-release Tags

For beta/RC releases:

```
v2.1.0-beta.1   ← Beta release
v2.1.0-rc.1     ← Release candidate
```

These are automatically marked as "pre-release" on GitHub.

## Image Verification

All images include metadata:

```bash
# Check image version
docker inspect ghcr.io/duytran1406/flasharr:latest | grep -A 5 Labels

# View build info
docker run --rm ghcr.io/duytran1406/flasharr:latest --version
```

## Best Practices

1. **Production**: Use `stable` or pinned versions (`v2.0.0`)
2. **Staging**: Use `latest` to test before production
3. **Development**: Use `nightly` or `develop`
4. **Pin versions** in production for reproducibility
5. **Test updates** in staging before production
6. **Subscribe** to GitHub releases for notifications
7. **Read changelogs** before major version updates

## Troubleshooting

### Image not updating?

```bash
# Force pull
docker compose pull --ignore-pull-failures

# Remove old image
docker rmi ghcr.io/duytran1406/flasharr:latest

# Pull fresh
docker compose pull
docker compose up -d
```

### Check what version is running:

```bash
docker exec flasharr cat /app/VERSION
# or
curl http://localhost:8484/api/version
```

---

**Questions?** See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) or open an issue.
