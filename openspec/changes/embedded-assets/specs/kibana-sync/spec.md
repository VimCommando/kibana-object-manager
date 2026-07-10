## ADDED Requirements

### Requirement: Generic Bundle Source Reading
The `kibana-sync` crate SHALL expose `KibanaBundle<S>` with a `Filesystem` source for path-backed reads and writes and an `Entries<B>` source for validated, root-relative file entries where `B: AsRef<[u8]>`. Both source types SHALL produce the same storage-neutral resource content, deterministic ordering, selection behavior, manifest validation, JSON5 parsing, and skill projection for equivalent logical bundles.

#### Scenario: Restrict operations by source type
- **GIVEN** a `KibanaBundle<Filesystem>`
- **WHEN** a consumer uses the bundle API
- **THEN** filesystem construction, root access, reads, and writes are available
- **AND** a `KibanaBundle<Entries<B>>` exposes entry construction and reads without filesystem-only operations

#### Scenario: Read all embedded bundle resources
- **GIVEN** a `KibanaBundle<Entries<B>>` containing `spaces.yml` and per-space manifests plus saved objects, workflows, agents, tools, and skills
- **WHEN** a consumer reads all resources from the entry-backed bundle
- **THEN** the library returns a `SyncBundle` containing the discovered spaces and every supported resource family
- **AND** no temporary files or directories are created

#### Scenario: Read selected embedded bundle resources
- **GIVEN** an entry-backed bundle containing multiple spaces and resource families
- **WHEN** a consumer reads it with a `SyncSelection`
- **THEN** the library loads only the selected spaces and resource families
- **AND** applies the same selection semantics as a filesystem-backed `KibanaBundle`

#### Scenario: Preserve manifest authority and resource ordering
- **GIVEN** an entry-backed bundle whose per-space manifest lists a subset of available resource files in a defined order
- **WHEN** the bundle is read
- **THEN** the returned collection contains only matching manifest-listed resources in manifest order
- **AND** a manifest-listed resource with no matching asset produces an error that identifies the resource and logical bundle path

#### Scenario: Parse JSON5 resource entries
- **GIVEN** a selected entry-backed `.json` resource containing supported JSON5 comments, trailing commas, unquoted keys, or multiline strings
- **WHEN** the bundle is read
- **THEN** the library parses it with the same JSON5 behavior as a filesystem bundle resource
- **AND** any decode or parse error identifies the resource's logical relative path

#### Scenario: Load a complete virtual skill directory
- **GIVEN** an entry-backed skill directory containing `SKILL.md`, nested referenced Markdown content, and reference metadata
- **WHEN** the bundle is read with skills enabled
- **THEN** the library projects the skill and its referenced content into the same Kibana value produced from the equivalent filesystem directory
- **AND** preserves referenced-content names, relative paths, content, and deterministic ordering

#### Scenario: Discover spaces from virtual layout
- **GIVEN** an entry-backed bundle with space entries in `spaces.yml` and a space directory containing supported resources but absent from that manifest
- **WHEN** all resources are read
- **THEN** the library discovers both space identifiers using the same rules as a filesystem-backed bundle

#### Scenario: Reject unsafe entry paths
- **GIVEN** an entry whose path is absolute, traverses a parent, has an invalid root component, or normalizes to an empty path
- **WHEN** `KibanaBundle<Entries<B>>` is constructed
- **THEN** construction fails before any resource is parsed
- **AND** the error identifies the invalid entry path

#### Scenario: Reject duplicate entry paths
- **GIVEN** two entries that normalize to the same logical relative path
- **WHEN** `KibanaBundle<Entries<B>>` is constructed
- **THEN** construction fails with an error identifying the duplicate path
- **AND** neither entry silently replaces the other

#### Scenario: Accept caller-selected byte storage
- **GIVEN** entry collections backed uniformly by borrowed byte slices, owned byte vectors, or shared byte buffers implementing `AsRef<[u8]>`
- **WHEN** a consumer constructs `KibanaBundle<Entries<B>>`
- **THEN** the bundle reads each representation through the same generic source API
- **AND** does not require a library-specific borrowed-or-owned content wrapper

#### Scenario: Preserve logical filesystem behavior
- **GIVEN** equivalent logical assets in `KibanaBundle<Filesystem>` and `KibanaBundle<Entries<B>>`
- **WHEN** both bundles are read
- **THEN** they return equivalent `SyncBundle` values
- **AND** the stable filesystem layout and write behavior remain unchanged
