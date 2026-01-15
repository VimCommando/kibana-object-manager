# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-01-14

### Added
- Complete rewrite in Rust from legacy Bash script
- Modern async ETL (Extract-Transform-Load) architecture
- Modular pipeline design with trait-based extractors, transformers, and loaders
- Full CLI with 7 commands: `auth`, `init`, `pull`, `push`, `add`, `togo`, `migrate`
- Support for multiple authentication methods (Basic Auth, API Key)
- Hierarchical object storage in `objects/` directory (type/id.json structure)
- Manifest-based object tracking with `manifest/saved_objects.json`
- Legacy manifest migration tool for existing repositories
- Comprehensive test suite (86 tests including unit, integration, and doctests)

### Changed
- **Breaking**: Replaced Bash implementation with Rust for better performance and maintainability
- **Breaking**: No longer depends on external tools (jsrmx, curl, jq, grep)
- **Breaking**: Manifest format changed from `manifest.json` to `manifest/saved_objects.json` (migration tool provided)
- Object storage structure changed to hierarchical type-based directories
- Environment variable configuration replaces `.env` file sourcing

### Removed
- Dependency on jsrmx, curl, jq, and grep
- Legacy Bash script implementation
- Support for `--env` flag for loading different `.env` files (use shell `source` instead)

### Fixed
- Improved error handling with descriptive messages
- Race conditions in test suite resolved
- Better handling of special characters in object IDs and attributes

### Technical Improvements
- Built with Tokio for async I/O operations
- Uses reqwest for HTTP client with connection pooling
- Proper JSON escaping/unescaping for Kibana compatibility
- GitIgnore integration for cleaner version control
- Comprehensive documentation in `docs/` directory

## [Pre-0.1.0] - Legacy Bash Implementation

The original Kibana Object Manager was implemented as a Bash script with dependencies on external command-line tools. This version was superseded by the Rust rewrite for improved reliability, performance, and user experience.

[0.1.0]: https://github.com/VimCommando/kibana-object-manager/releases/tag/v0.1.0
