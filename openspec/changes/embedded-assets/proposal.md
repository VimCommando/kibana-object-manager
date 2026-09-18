## Why

Embedded consumers such as ESDiag cannot use `KibanaFsBundle` without first materializing bundled assets to a temporary directory because the reader is coupled to `std::fs`. A storage-neutral reader is needed so those consumers can reuse `kibana-sync` as the single parser and loader while preserving the established bundle layout and behavior.

## What Changes

- **BREAKING** Replace `KibanaFsBundle` with a generic `KibanaBundle<S>` whose backend determines its available operations.
- Add a `Filesystem` backend for path-backed bundle reads and writes and an `Entries<B>` backend for embedded `(relative path, bytes)` assets where `B: AsRef<[u8]>`.
- Share bundle discovery, manifest parsing, resource selection, JSON5 parsing, ordering, and validation across both generic backends.
- Preserve complete skill-directory loading, including `SKILL.md` and referenced-content subdirectories, without requiring real directories. Referenced files use filesystem-safe names and derive their API names and relative paths from their location.
- Reject unsafe, invalid, or ambiguous entry paths with actionable library errors.
- Document the generic API, migrate both consumers, and test behavioral parity between filesystem-backed and entry-backed reads.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `kibana-sync`: Replace the filesystem-specific bundle type with a generic bundle abstraction supporting filesystem and entry-backed sources with equivalent read semantics.

## Impact

- Affected code: `crates/kibana-sync/src/fs.rs`, skill storage projection helpers, public exports, crate documentation, and bundle reader tests.
- Public API: replaces `KibanaFsBundle` with `KibanaBundle<Filesystem>` and adds `KibanaBundle<Entries<B>>`; no compatibility alias is required before the first public release.
- Consumers: embedded applications can load compile-time or in-memory assets directly and pass the resulting `SyncBundle` to existing push sync APIs.
- Dependencies: no new runtime dependency is expected.
