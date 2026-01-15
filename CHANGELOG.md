# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Redesigned `add` command with API-agnostic support for workflows, spaces, and objects
- Regex-based filtering with `--include` and `--exclude` options for workflows and spaces
- `--include` option to filter items by name using regex patterns
- `--exclude` option to exclude items by name using regex patterns (applied after include)
- Support for case-insensitive matching using `(?i)` regex flag
- Spaces discovery and addition with `kibob add spaces .`
- File-based space import supporting both `.json` (API response) and `.ndjson` (bundle) formats
- `SpacesExtractor::search_spaces()` method for fetching all spaces via API
- Workflow discovery via search API with `kibob add workflows . --query "term"`
- File-based workflow import supporting both `.json` (API response) and `.ndjson` (bundle) formats
- Duplicate detection when adding workflows and spaces to manifest
- Automatic manifest creation if it doesn't exist when adding workflows or spaces
- `WorkflowsExtractor::search_workflows()` method for discovering workflows via search API
- Kibana Workflows API integration for managing workflow configurations
- `manifest/workflows.yml` for tracking workflows with both ID and name
- Automatic workflow pull/push/bundle when `manifest/workflows.yml` exists
- Individual workflow storage as pretty-printed JSON files in `workflows/` directory (named by workflow name)
- Integration tests for workflows functionality
- Kibana Spaces API integration for managing space configurations
- `manifest/spaces.yml` for tracking spaces in YAML format
- Automatic space pull/push/bundle when `manifest/spaces.yml` exists
- Individual space storage as pretty-printed JSON files in `spaces/` directory
- `bundle/` directory structure for organized NDJSON output files
- Complete spaces documentation in `docs/SPACES.md`
- Integration tests for spaces functionality
- `--space` flag for `pull` and `push` commands to override `KIBANA_SPACE` env var
- `regex` crate dependency for pattern matching

### Changed
- **Breaking**: `add` command now requires API type as first argument: `kibob add <api> [dir]`
- **Breaking**: Replaced `--filter` with `--include` and `--exclude` regex-based filters
- **Breaking**: Legacy `--objects` flag only works with `kibob add objects` command
- Filtering is now regex-based instead of substring matching (more powerful and flexible)
- `add` command syntax: `kibob add <api> .` (search), `--include "regex"` (include matches), `--exclude "regex"` (exclude matches), `--file path` (from file)
- Workflows support `--query "term"` for server-side filtering, spaces fetch all and filter client-side
- **Breaking**: `togo` command now creates `bundle/` directory instead of root-level NDJSON files
- **Breaking**: Renamed `export.ndjson` to `bundle/saved_objects.ndjson`
- **Breaking**: Spaces bundled to `bundle/spaces.ndjson` instead of root-level `spaces.ndjson`
- **Breaking**: Workflows bundled to `bundle/workflows.ndjson`
- **Breaking**: Workflows manifest format now includes both `id` and `name` fields (not just name)
- Bundle directory structure allows easy archiving: `zip -r archive.zip bundle/`

### Fixed
- Fixed double space prefix bug in Kibana API URLs (was `/s/space/s/space/api/...`, now `/s/space/api/...`)
- `WorkflowsManifest::add_workflow()` now returns boolean to indicate if workflow was added or already existed
- `SpacesManifest::add_space()` now returns boolean to indicate if space was added or already existed

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
