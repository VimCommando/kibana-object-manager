## 1. Generic Source Model

- [x] 1.1 Add `KibanaBundle<S>` with a sealed `BundleSource` contract for existence checks, traversal, and byte reads.
- [x] 1.2 Add `Filesystem` and generic `Entries<B: AsRef<[u8]>>` source types.
- [x] 1.3 Add root-relative logical path normalization and a deterministic entry index with implicit directories.
- [x] 1.4 Reject absolute paths, parent traversal, invalid root components, empty paths, and duplicate normalized entries with path-specific library errors.
- [x] 1.5 Add unit tests for source operations, path normalization, implicit directories, deterministic listing, invalid paths, and duplicate detection.

## 2. Generic Bundle Reader

- [x] 2.1 Refactor space discovery, manifest loading, recursive JSON resource loading, JSON5 parsing, selection, and manifest filtering into `KibanaBundle<S: BundleSource>`.
- [x] 2.2 Implement `read_all` and `read` once for all readable bundle sources.
- [x] 2.3 Implement `open`, `create`, `root`, and `write` only for `KibanaBundle<Filesystem>` while preserving the stable layout and filesystem safety checks.
- [x] 2.4 Implement `from_entries` only for `KibanaBundle<Entries<B>>` without filesystem access or temporary materialization.
- [x] 2.5 Add tests covering generic reads, manifest ordering, missing-resource errors, explicit selections, and source-specific method behavior.

## 3. Public API and Consumer Migration

- [x] 3.1 Export `KibanaBundle`, `Filesystem`, and `Entries<B>` from `kibana-sync` and remove the `KibanaFsBundle` public type.
- [x] 3.2 Migrate all `kibana-object-manager` filesystem bundle call sites to `KibanaBundle<Filesystem>`.
- [x] 3.3 Verify an application-defined byte type can construct `KibanaBundle<Entries<B>>` through `AsRef<[u8]>` without a consumer-specific adapter.
- [x] 3.4 Add tests for borrowed slices, owned vectors, shared buffers, empty entry bundles, automatic space discovery, selected resources, invalid UTF-8, and path-specific parse errors.

## 4. Source-Neutral Skill Projection

- [x] 4.1 Separate skill frontmatter, body, referenced-content, and reference-metadata parsing from operating-system directory traversal.
- [x] 4.2 Adapt filesystem skill projection to the shared format parser while retaining canonical-root containment and symlink rejection.
- [x] 4.3 Implement entry-backed skill discovery from immediate skill directories containing `SKILL.md`, including nested referenced Markdown files and reference metadata.
- [x] 4.4 Add tests for skill manifest filtering, missing `SKILL.md`, nested referenced content, metadata path restoration, deterministic ordering, and filesystem/entry value equality.

## 5. Parity and Documentation

- [x] 5.1 Create a shared representative bundle fixture covering spaces, every supported resource family, manifests, nested objects, JSON5, skills, and referenced content.
- [x] 5.2 Add conformance tests asserting `KibanaBundle<Filesystem>` and `KibanaBundle<Entries<&[u8]>>` `read_all` results are equal for the shared fixture.
- [x] 5.3 Add conformance tests asserting filesystem and entry-backed selected reads and validation failures are behaviorally equivalent.
- [x] 5.4 Document generic filesystem, `include_bytes!`, and dynamically collected in-memory usage in crate docs or the `kibana-sync` README.

## 6. Verification

- [x] 6.1 Run `cargo fmt --all` and verify formatting.
- [x] 6.2 Run `cargo clippy --workspace --all-targets -- -D warnings` and resolve warnings.
- [x] 6.3 Run `cargo test --workspace` and confirm all unit, integration, conformance, and documentation tests pass.
