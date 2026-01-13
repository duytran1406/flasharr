# Development Changelog

Version history and changes for Flasharr.

---

## [0.0.3-beta] - 2026-01-13

### Added
- **Multi-threaded Downloads**: Segmented downloads for maximum speed
- **Multi-Account Support**: Load balancing across multiple VIP accounts
- **Priority Queue System**: Prioritize downloads (LOW, NORMAL, HIGH, URGENT)
- **Global Speed Limiting**: Configurable bandwidth limits
- **Dynamic Segment Scaling**: Auto-adjust segments based on file size
- **Auto-Cleanup Service**: Automatic history management
- **Link Checker**: Pre-validate links before download
- **WebSocket Support**: Real-time progress updates
- **Enhanced Dashboard**: Modern UI with real-time statistics
- **Account Manager**: Multi-account quota tracking and rotation

### Changed
- Migrated from `/app` to `/src/flasharr` module structure
- Improved error handling and retry logic
- Enhanced logging with structured output
- Optimized database queries for better performance

### Fixed
- Session expiration handling
- Download resume functionality
- Queue persistence across restarts
- Memory leaks in download workers

---

## [0.0.2-beta] - 2026-01-01

### Added
- **SABnzbd API Emulation**: Full download client support
- **Newznab/Torznab API**: Indexer integration
- **Web Dashboard**: Basic UI for download management
- **SQLite Queue**: Persistent download queue
- **Async Download Engine**: Concurrent download support

### Changed
- Refactored client layer for better modularity
- Improved Fshare authentication flow
- Enhanced search result scoring

### Fixed
- Prowlarr integration issues
- NZB file parsing
- Category mapping for *arr apps

---

## [0.0.1-alpha] - 2025-12-28

### Added
- Initial project structure
- Basic Fshare client
- TimFshare search integration
- Simple download functionality
- Docker support

---

## Migration Notes

### From 0.0.2-beta to 0.0.3-beta

**Breaking Changes:**
- Environment variable changes:
  - `PYLOAD_URL` removed (no longer using PyLoad)
  - `FSHARE_ACCOUNTS` added for multi-account support
  - `MAX_CONCURRENT_DOWNLOADS` replaces `MAX_DOWNLOADS`

**Migration Steps:**
1. Update `.env` file with new variables
2. Remove PyLoad-related configuration
3. Rebuild Docker image: `docker-compose build`
4. Restart container: `docker-compose up -d`

**Data Migration:**
- Download queue automatically migrated
- No manual intervention required

---

### From Legacy (fshare-arr-bridge) to Flasharr

**Breaking Changes:**
- Complete codebase restructure
- New API endpoints
- Different configuration format

**Migration Steps:**
See [Migration Guide](../migration/from-legacy.md) for detailed instructions.

---

## Roadmap

### Planned for 0.0.4-beta
- [ ] Scheduled downloads
- [ ] Bandwidth scheduling (slow mode during peak hours)
- [ ] Download categories and organization
- [ ] Advanced filtering in web UI
- [ ] Export/import configuration
- [ ] API authentication

### Planned for 0.1.0 (Stable)
- [ ] Complete test coverage
- [ ] Production-ready error handling
- [ ] Performance optimizations
- [ ] Comprehensive documentation
- [ ] Migration tools from other download managers

### Future Considerations
- [ ] Support for other file hosting services
- [ ] Plugin system for extensibility
- [ ] Mobile app
- [ ] Torrent support
- [ ] Cloud storage integration

---

## Version Numbering

Flasharr follows [Semantic Versioning](https://semver.org/):

- **MAJOR**: Incompatible API changes
- **MINOR**: New functionality (backwards compatible)
- **PATCH**: Bug fixes (backwards compatible)
- **-beta**: Pre-release versions

---

## Contributing

See [Contributing Guide](contributing.md) for how to contribute to Flasharr development.

---

## License

Flasharr is released under the MIT License.
