## 1. Generic Source Model

- [ ] 1.1 Add `KibanaBundle<S>` with a sealed `BundleSource` contract for existence checks, traversal, and byte reads.
- [ ] 1.2 Add `Filesystem` and generic `Entries<B: AsRef<[u8]>>` source types.
- [ ] 1.3 Add root-relative logical path normalization and a deterministic entry index with implicit directories.
- [ ] 1.4 Reject absolute paths, parent traversal, invalid root components, empty paths, and duplicate normalized entries with path-specific library errors.
- [ ] 1.5 Add unit tests for source operations, path normalization, implicit directories, deterministic listing, invalid paths, and duplicate detection.

## 2. Generic Bundle Reader

- [ ] 2.1 Refactor space discovery, manifest loading, recursive JSON resource loading, JSON5 parsing, selection, and manifest filtering into `KibanaBundle<S: BundleSource>`.
- [ ] 2.2 Implement `read_all` and `read` once for all readable bundle sources.
- [ ] 2.3 Implement `open`, `create`, `root`, and `write` only for `KibanaBundle<Filesystem>` while preserving the stable layout and filesystem safety checks.
- [ ] 2.4 Implement `from_entries` only for `KibanaBundle<Entries<B>>` without filesystem access or temporary materialization.
- [ ] 2.5 Add tests covering generic reads, manifest ordering, missing-resource errors, explicit selections, and source-specific method behavior.

## 3. Public API and Consumer Migration

- [ ] 3.1 Export `KibanaBundle`, `Filesystem`, and `Entries<B>` from `kibana-sync` and remove the `KibanaFsBundle` public type.
- [ ] 3.2 Migrate all `kibana-object-manager` filesystem bundle call sites to `KibanaBundle<Filesystem>`.
- [ ] 3.3 Verify ESDiag can construct `KibanaBundle<Entries<B>>` directly from its in-memory directory entry type.
- [ ] 3.4 Add tests for borrowed slices, owned vectors, shared buffers, empty entry bundles, automatic space discovery, selected resources, invalid UTF-8, and path-specific parse errors.

## 4. Source-Neutral Skill Projection

- [ ] 4.1 Separate skill frontmatter, body, referenced-content, and reference-metadata parsing from operating-system directory traversal.
- [ ] 4.2 Adapt filesystem skill projection to the shared format parser while retaining canonical-root containment and symlink rejection.
- [ ] 4.3 Implement entry-backed skill discovery from immediate skill directories containing `SKILL.md`, including nested referenced Markdown files and reference metadata.
- [ ] 4.4 Add tests for skill manifest filtering, missing `SKILL.md`, nested referenced content, metadata path restoration, deterministic ordering, and filesystem/entry value equality.

## 5. Parity and Documentation

- [ ] 5.1 Create a shared representative bundle fixture covering spaces, every supported resource family, manifests, nested objects, JSON5, skills, and referenced content.
- [ ] 5.2 Add conformance tests asserting `KibanaBundle<Filesystem>` and `KibanaBundle<Entries<&[u8]>>` `read_all` results are equal for the shared fixture.
- [ ] 5.3 Add conformance tests asserting filesystem and entry-backed selected reads and validation failures are behaviorally equivalent.
- [ ] 5.4 Document generic filesystem, `include_bytes!`, and dynamically collected in-memory usage in crate docs or the `kibana-sync` README.

## 6. Verification

- [ ] 6.1 Run `cargo fmt --all` and verify formatting.
- [ ] 6.2 Run `cargo clippy --workspace --all-targets -- -D warnings` and resolve warnings.
- [ ] 6.3 Run `cargo test --workspace` and confirm all unit, integration, conformance, and documentation tests pass.
