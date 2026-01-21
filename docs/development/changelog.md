# Development Changelog

Version history and changes for Flasharr.

---

## [0.2.0-beta] - 2026-01-21

### 🎉 Major Release - Authentication System Overhaul

### Added
- **Single-Instance Authentication** - All auth calls now share same FshareClient with shared lock
- **Improved Session Persistence** - Token expiry restored correctly from saved state
- **Enhanced Logging** - Detailed authentication flow logging with emojis for clarity
- **TMDB Integration** - Direct TMDB API for movie/TV discovery
- **Smart Search** - Enhanced search with quality scoring
- **Mobile UI Foundation** - Bottom navigation, mobile drawer

### Fixed
- **Critical: Multiple Login Sessions** - Fixed issue where 3-4 login sessions were created per download
- **Critical: Session Not Restored** - Fixed token_expires not being restored from cookies
- **Critical: Duplicate ensure_authenticated** - Removed redundant auth calls from FshareHandler and get_download_link
- **Test Account Function** - Fixed missing datetime import in settings API
- **Download Failures** - Downloads now succeed after single authentication

### Changed
- `refresh_account()` now uses cached client instead of creating new instances
- `SABnzbd.add_url()` now handles authentication centrally before Fshare operations
- Removed `_handle_session_expiry` auto-login from `get_download_link`
- Consolidated documentation into single `docs/` folder

### Security
- Single threading lock prevents race conditions during login
- Session cookies properly persisted and restored

---

## [0.1.x-beta] - 2026-01-01 to 2026-01-20

### Added
- **Multi-threaded Downloads**: Segmented downloads for maximum speed
- **Multi-Account Support**: Load balancing across multiple VIP accounts
- **Priority Queue System**: Prioritize downloads (LOW, NORMAL, HIGH, URGENT)
- **Global Speed Limiting**: Configurable bandwidth limits
- **WebSocket Support**: Real-time progress updates
- **Enhanced Dashboard**: Modern glassmorphism UI with Material theme
- **Account Manager**: Multi-account quota tracking and rotation
- **Log Streaming**: WebSocket-based real-time log updates

### Changed
- Migrated from `/app` to `/src/flasharr` module structure
- Improved error handling and retry logic
- Enhanced logging with structured output
- Migrated from Jellyseerr to TMDB for media discovery

### Fixed
- Session expiration handling
- Download resume functionality
- Queue persistence across restarts
- UI status mismatch (state vs progress)

---

## [0.0.2-beta] - 2026-01-01

### Added
- **SABnzbd API Emulation**: Full download client support
- **Newznab/Torznab API**: Indexer integration
- **Web Dashboard**: Basic UI for download management
- **SQLite Queue**: Persistent download queue
- **Async Download Engine**: Concurrent download support

---

## [0.0.1-alpha] - 2025-12-28

### Added
- Initial project structure
- Basic Fshare client
- TimFshare search integration
- Simple download functionality
- Docker support

---

## Version Numbering

Flasharr follows [Semantic Versioning](https://semver.org/):

- **MAJOR**: Incompatible API changes
- **MINOR**: New functionality (backwards compatible)
- **PATCH**: Bug fixes (backwards compatible)
- **-beta**: Pre-release versions

---

## License

Flasharr is released under the MIT License.
