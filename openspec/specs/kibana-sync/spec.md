# kibana-sync Specification

## Purpose
TBD - created by archiving change refactor-space-into-kibana-sync. Update Purpose after archive.
## Requirements
### Requirement: Space-Aware Client Architecture

The Kibana client SHALL expose:
- `KibanaClient`: Root client that holds shared HTTP client state, base URL, space registry, concurrency limit, and cached server version information
- `SpaceClient` or an equivalent space-bound client view that automatically prefixes API paths with a non-default space

#### Scenario: Create space-bound client
- **WHEN** calling `kibana.space("marketing")`
- **THEN** return `Ok(SpaceClient)` if "marketing" exists in the caller-provided space registry
- **THEN** return `Err` if "marketing" is not in the caller-provided space registry

#### Scenario: Default space handling
- **WHEN** calling `kibana.space("default")`
- **THEN** return a space-bound client with no `/s/default/` path prefix
- **THEN** API paths SHALL NOT be prefixed with `/s/default/`

#### Scenario: Non-default space path prefixing
- **WHEN** a space-bound client for "marketing" calls `get("/api/saved_objects")`
- **THEN** the actual request path SHALL be `/s/marketing/api/saved_objects`

### Requirement: Explicit Client Configuration

The `kibana-sync` crate SHALL construct clients from explicit configuration values rather than CLI project directories.

#### Scenario: Construct client with default space
- **WHEN** a consumer builds a client with a Kibana URL, auth configuration, and no explicit spaces
- **THEN** the client is created with the `default` space available
- **AND** no filesystem reads are performed during construction

#### Scenario: Construct client with caller-provided spaces
- **WHEN** a consumer builds a client with a caller-provided list or map of spaces
- **THEN** the client validates space-bound clients against that registry
- **AND** it does not read `spaces.yml`

#### Scenario: Construct client with concurrency limit
- **WHEN** a consumer configures a maximum concurrent request count
- **THEN** all cloned root and space-bound clients share that request limit

### Requirement: Space Query Methods

`KibanaClient` SHALL provide methods to query available spaces.

#### Scenario: List space IDs
- **WHEN** calling `kibana.space_ids()`
- **THEN** return `Vec<&str>` of all space IDs from the registry

#### Scenario: Get space name
- **WHEN** calling `kibana.space_name("marketing")`
- **THEN** return `Some("Marketing")` if space exists
- **THEN** return `None` if space does not exist

### Requirement: Explicit Filesystem Manifest and Bundle Sync

The `kibana-sync` crate SHALL support reusable filesystem-backed sync for Kibana manifests and file-backed assets using caller-provided paths.

#### Scenario: Read filesystem bundle from explicit path
- **WHEN** a consumer asks the library to read a Kibana asset bundle from a provided path
- **THEN** the library reads supported manifest files and file-backed saved objects, workflows, agents, and tools from that path
- **AND** returns a sync bundle or equivalent resource collection that can be pushed to Kibana
- **AND** it does not infer the path from environment variables, process working directory, or CLI command state

#### Scenario: Write filesystem bundle to explicit path
- **WHEN** a consumer asks the library to write a pulled sync bundle to a provided path
- **THEN** the library writes supported manifests and file-backed resources in a stable bundle layout
- **AND** the written bundle can be read back by the library and pushed to another Kibana instance

#### Scenario: CLI project policy remains outside filesystem sync
- **WHEN** `kibob` uses the library filesystem sync APIs
- **THEN** the CLI crate chooses default paths, command behavior, terminal output, gitignore behavior, and migration policy
- **AND** the library only receives explicit paths, sync options, and resource data
