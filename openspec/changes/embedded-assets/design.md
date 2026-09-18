## Context

`KibanaFsBundle` currently combines two responsibilities: access to an operating-system directory tree and interpretation of the Kibana bundle layout. Its read path calls `Path::exists`, `std::fs::read_dir`, and `std::fs::read_to_string` directly, while skill projection expects a real skill directory so it can discover `SKILL.md` and referenced files. This prevents ESDiag and similar consumers from loading assets exposed as relative paths and bytes without first creating a temporary directory or duplicating the layout parser.

The established behavior is broader than parsing JSON files. The reader discovers spaces from `spaces.yml` and space directories, honors per-space manifests as ordering and selection authorities, recursively parses JSON5 resources, validates manifest references, and converts complete skill directory trees into Kibana values. The stable layout and parsing behavior must remain intact, but `KibanaFsBundle` source compatibility is unnecessary because the only two consumers are this workspace and ESDiag, both of which can migrate before the first public release.

Data flow after this change is:

```text
KibanaBundle<Filesystem> --+
                           +-> BundleSource -> manifests / JSON5 / skill projection
KibanaBundle<Entries<B>> --+                    -> SyncBundle
                                                -> push_sync ETL loaders
                                                -> Kibana APIs
```

The change affects only the extraction side of the sync ETL pipeline. `SyncBundle`, dependency expansion, transforms, and Kibana loaders remain storage-neutral and unchanged.

## Goals / Non-Goals

**Goals:**
- Load a complete Kibana asset bundle from relative-path and byte-content entries without filesystem materialization.
- Produce the same `SyncBundle` content, ordering, selection behavior, and validation outcomes as filesystem reads.
- Preserve JSON5 support and complete skill projection, including nested referenced files.
- Provide one generic `KibanaBundle<S>` API with compile-time backend capabilities.
- Keep filesystem bundle layout and read/write behavior while allowing both consumers to migrate to the generic API.
- Reject paths that could escape the entry root or make lookup ambiguous.
- Keep the reader synchronous and free of new runtime dependencies.

**Non-Goals:**
- Add write support for entry-backed sources.
- Model operating-system metadata, permissions, symlinks, or modification times in entry-backed sources.
- Change the stable on-disk bundle layout, manifest schemas, `SyncBundle`, or Kibana API behavior.
- Add archive-format readers, asset compression, proc macros, or build scripts.
- Expose a fully general virtual filesystem abstraction in the first version.

## Decisions

### 1) Make the bundle generic over a source backend

Replace `KibanaFsBundle` with these public types:

```rust
pub struct KibanaBundle<S> {
    source: S,
}

pub struct Filesystem {
    root: PathBuf,
}

pub struct Entries<B> {
    files: BTreeMap<String, B>,
}
```

`KibanaBundle<Filesystem>` provides `open`, `create`, `root`, and `write`. `KibanaBundle<Entries<B>>` provides `from_entries` when `B: AsRef<[u8]>`. Both provide `read_all` and `read` through their `BundleSource` implementation. The source trait is sealed initially so the crate can refine its minimal traversal contract without committing to a general virtual filesystem API.

The byte storage type belongs to the caller. Static embedded assets use `&'static [u8]`, dynamically collected assets use `Vec<u8>`, and consumers needing shared ownership can use `Arc<[u8]>`. A single entry collection uses one byte storage type; callers that need mixed ownership can choose their own uniform enum or shared representation.

Rationale: backend generics make filesystem-only operations unavailable on entry-backed bundles at compile time, avoid lifetime-heavy `Cow` APIs, and let Rust infer content ownership from the supplied collection. Entry pairs match ESDiag's in-memory directory model and can represent every stable bundle asset.

Alternative considered: use a single bundle containing a filesystem-or-entries enum. Rejected because `root` and `write` would then require runtime errors or optional results for entry-backed bundles. Separate `KibanaFsBundle` and `KibanaVirtualBundle` types were also rejected because they duplicate the common API and obscure that both are the same parser over different sources.

### 2) Separate source access from bundle interpretation

Define a sealed `BundleSource` interface with the minimal operations needed by bundle parsing: test for a file or directory prefix, enumerate immediate or recursive entries, and read file bytes. Implement it for `Filesystem` and `Entries<B>`. Move space discovery, manifest loading, JSON resource loading, manifest filtering, and skill tree projection into generic `KibanaBundle<S>` reader methods.

Filesystem-only safety checks such as rejecting symlinked skill directories remain in `Filesystem`. Filesystem construction and writing are specialized inherent implementations on `KibanaBundle<Filesystem>` rather than part of `BundleSource`.

Rationale: a single interpretation path prevents the entry-backed reader from drifting from filesystem behavior. Source backends own access-specific concerns; parsing owns bundle semantics.

Alternative considered: implement a second parser over a `BTreeMap<PathBuf, bytes>`. Rejected because manifest, discovery, JSON5, and skill behavior would immediately be duplicated.

### 3) Treat paths as normalized, root-relative logical bundle paths

`Entries<B>` construction accepts only non-empty relative paths. It normalizes platform separators to the bundle's logical separator and rejects absolute paths, parent traversal, root/prefix components, empty terminal names, and duplicate normalized paths. Directory entries are implicit from file path prefixes; callers provide files only. Paths are compared and sorted using their normalized logical form.

Case remains significant, matching the stable bundle names and avoiding silent aliasing on case-sensitive consumers. Exact duplicate normalized paths are errors rather than last-write-wins.

Rationale: embedded bundles have no operating-system root to contain traversal. Validating at construction establishes a safe and deterministic namespace before any parsing occurs.

Alternative considered: silently clean `.` and `..` components. Rejected because accepting traversal-shaped input can hide producer mistakes and create ambiguous duplicate paths.

### 4) Parse text from bytes at the source-neutral boundary

Manifest, JSON5, `SKILL.md`, and referenced-content readers decode required text files as UTF-8 and attach the normalized logical path to parse and decoding errors. JSON resources retain the current recursive `.json` discovery, lexicographic ordering, and `json5::from_json5_str` behavior. YAML and JSON manifests continue using their existing schemas and source-of-truth semantics.

Rationale: byte entries support normal embedding APIs while explicit UTF-8 validation preserves the text formats' current expectations and provides useful diagnostics.

Alternative considered: require entry content as `str`. Rejected because common embedding and in-memory directory APIs expose bytes, and rejecting non-UTF-8 should happen only when a selected textual resource is read.

### 5) Project skills directly from their logical directory tree

Extract the format-level portions of `skill_to_value` so they can consume a logical skill directory and its text entries rather than requiring `Path::canonicalize` and `std::fs` traversal. A skill is discoverable only when an immediate child of `<space>/skills/` contains `SKILL.md`. Every other nested file is referenced content: the filesystem-safe filename stem is its API name and its parent directory determines its API relative path. No metadata sidecar or path override is accepted.

Filesystem projection continues to enforce canonical-root containment and reject symlinked skill directories or referenced content. Entry-backed sources cannot contain symlinks, so their equivalent safety guarantee comes from constructor path validation.

Rationale: materializing only skill directories would retain the original problem and introduce a hidden temporary-filesystem dependency.

Alternative considered: represent each skill as a prebuilt Kibana JSON value in the entry API. Rejected because it bypasses the documented bundle layout and would force embedded consumers to duplicate skill parsing.

### 6) Verify parity with source conformance fixtures

Build one representative bundle fixture containing spaces, manifests, nested saved objects, workflows, agents, tools, JSON5 syntax, skills, and nested referenced files. Load it through `KibanaBundle<Filesystem>` and `KibanaBundle<Entries<&[u8]>>`, then assert equal `SyncBundle` results for `read_all` and representative selections. Add entry-only tests for invalid paths, duplicates, invalid UTF-8 in selected text assets, missing manifest resources, deterministic ordering, and empty bundles.

Rationale: parity is the primary compatibility promise and is stronger when exercised against identical logical content.

Alternative considered: maintain separate filesystem and entry-backed test fixtures. Rejected because differences between fixtures can conceal behavioral drift.

## Risks / Trade-offs

- [Risk] Refactoring mature filesystem parsing may regress current reads. -> Mitigation: route both sources through shared functions and retain existing filesystem tests alongside parity fixtures.
- [Risk] A uniform `B` prevents mixing borrowed slices and owned vectors in one entry collection. -> Mitigation: typical producers already expose a uniform content type; callers with mixed ownership can normalize to `Vec<u8>`, `Arc<[u8]>`, or their own enum implementing `AsRef<[u8]>`.
- [Risk] Generic compiler errors could expose internal source bounds. -> Mitigation: keep constructors specialized, document common inferred forms, and seal the source trait.
- [Risk] Filesystem case and symlink behavior cannot be identical across platforms and entry-backed sources. -> Mitigation: define parity at the logical bundle-content level while keeping source-specific safety checks explicit.
- [Risk] Eager indexing adds path metadata overhead for large bundles. -> Mitigation: borrow content where possible, store entries in sorted maps, and avoid copying file bytes during parsing unless required by a parser.
- [Risk] Extracting skill projection could accidentally change API field construction. -> Mitigation: keep one projection implementation and test complete value equality, including referenced content.

## Migration Plan

1. Add the generic `KibanaBundle<S>`, sealed `BundleSource`, `Filesystem`, and `Entries<B>` types.
2. Add normalized entry indexing and path validation, then move bundle discovery plus manifest/resource parsing into the generic reader.
3. Refactor skill projection into source-neutral format parsing with filesystem-specific containment checks at the adapter boundary.
4. Replace workspace `KibanaFsBundle` call sites with `KibanaBundle<Filesystem>` and export the generic API for ESDiag.
5. Add shared parity fixtures and negative entry-source tests.
6. Run formatting, clippy, the full workspace test suite, and crate documentation tests.

Migrate the workspace and ESDiag together before publishing the next crate version. Rollback consists of reverting the generic API and reader refactor; no persistent data or bundle format changes are involved.

## Open Questions

None currently.
