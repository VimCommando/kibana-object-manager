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

### Requirement: Reusable Kibana API Modules
The `kibana-sync` crate SHALL expose reusable API modules for saved objects, spaces, agents, tools, and workflows.

#### Scenario: Saved object import and export
- **WHEN** a consumer exports saved objects
- **THEN** the library sends `POST /api/saved_objects/_export` with a JSON export payload
- **AND** parses the NDJSON response into JSON values
- **WHEN** a consumer imports saved objects
- **THEN** the library sends `POST /api/saved_objects/_import?overwrite=<value>` as multipart form data with `Content-Type: multipart/form-data`

#### Scenario: Space management
- **WHEN** a consumer lists spaces
- **THEN** the library sends `GET /api/spaces/space`
- **WHEN** a consumer fetches a specific space
- **THEN** the library sends `GET /api/spaces/space/{id}`
- **WHEN** a consumer creates or updates a space
- **THEN** the library sends `POST /api/spaces/space` for create operations
- **AND** sends `PUT /api/spaces/space/{id}` for update operations

#### Scenario: Agent management
- **WHEN** a consumer lists agents
- **THEN** the library sends `GET /api/agent_builder/agents`
- **WHEN** a consumer fetches or checks an agent
- **THEN** the library sends `GET /api/agent_builder/agents/{id}` or `HEAD /api/agent_builder/agents/{id}`
- **WHEN** a consumer creates or updates an agent
- **THEN** the library sends `POST /api/agent_builder/agents` for create operations
- **AND** sends `PUT /api/agent_builder/agents/{id}` for update operations

#### Scenario: Tool management
- **WHEN** a consumer lists tools
- **THEN** the library sends `GET /api/agent_builder/tools`
- **WHEN** a consumer fetches or checks a tool
- **THEN** the library sends `GET /api/agent_builder/tools/{id}` or `HEAD /api/agent_builder/tools/{id}`
- **WHEN** a consumer creates or updates a tool
- **THEN** the library sends `POST /api/agent_builder/tools` for create operations
- **AND** sends `PUT /api/agent_builder/tools/{id}` for update operations

#### Scenario: Workflow management uses internal-origin header
- **WHEN** a consumer searches workflows
- **THEN** the library sends `POST /api/workflows/search`
- **AND** includes `X-Elastic-Internal-Origin: Kibana`
- **WHEN** a consumer fetches or checks a workflow
- **THEN** the library sends `GET /api/workflows/{id}` or `HEAD /api/workflows/{id}`
- **AND** includes `X-Elastic-Internal-Origin: Kibana`
- **WHEN** a consumer creates or updates a workflow
- **THEN** the library sends `POST /api/workflows` for create operations
- **AND** sends `PUT /api/workflows/{id}` for update operations
- **AND** includes `X-Elastic-Internal-Origin: Kibana`

### Requirement: Space Query Methods

`KibanaClient` SHALL provide methods to query available spaces.

#### Scenario: List space IDs
- **WHEN** calling `kibana.space_ids()`
- **THEN** return `Vec<&str>` of all space IDs from the registry

#### Scenario: Get space name
- **WHEN** calling `kibana.space_name("marketing")`
- **THEN** return `Some("Marketing")` if space exists
- **THEN** return `None` if space does not exist

### Requirement: Storage-Neutral Sync Support
The `kibana-sync` crate SHALL support syncing all supported API families without requiring a `kibob` filesystem project.

#### Scenario: Pull sync returns bundle
- **WHEN** a consumer requests a pull sync for selected spaces and API families
- **THEN** the library returns a bundle containing the fetched spaces, saved objects, workflows, agents, and tools grouped by space where applicable
- **AND** it does not write local files

#### Scenario: Push sync accepts bundle
- **WHEN** a consumer requests a push sync with a bundle of spaces, saved objects, workflows, agents, and tools
- **THEN** the library applies the resources to Kibana using the appropriate API module for each resource family
- **AND** it returns a summary of attempted and applied resources

#### Scenario: Dependency expansion is resource based
- **WHEN** a consumer enables dependency expansion for agents, tools, or workflows
- **THEN** the library discovers dependent agents, tools, and workflows from JSON resource definitions
- **AND** fetches missing dependencies through Kibana APIs
- **AND** returns the expanded resources in the sync bundle rather than writing them to files

### Requirement: Explicit Filesystem Manifest and Bundle Sync

The `kibana-sync` crate SHALL support reusable filesystem-backed sync for Kibana manifests and file-backed assets using caller-provided paths.

#### Scenario: Read filesystem bundle from explicit path
- **WHEN** a consumer asks the library to read a Kibana asset bundle from a provided path
- **THEN** the library reads supported manifest files and file-backed saved objects, workflows, agents, and tools from that path
- **AND** returns a `SyncBundle` or equivalent resource collection that can be pushed to Kibana
- **AND** it does not infer the path from environment variables, process working directory, or CLI command state

#### Scenario: Write filesystem bundle to explicit path
- **WHEN** a consumer asks the library to write a pulled sync bundle to a provided path
- **THEN** the library writes supported manifests and file-backed resources in a stable bundle layout
- **AND** the written bundle can be read back by the library and pushed to another Kibana instance

#### Scenario: CLI project policy remains outside filesystem sync
- **WHEN** `kibob` uses the library filesystem sync APIs
- **THEN** the CLI crate chooses default paths, command behavior, terminal output, gitignore behavior, and migration policy
- **AND** the library only receives explicit paths, sync options, and resource data

### Requirement: Reusable Capability Gates
The `kibana-sync` crate SHALL expose Kibana version detection and API capability support checks for all supported API families.

#### Scenario: Server version detection
- **WHEN** a consumer requests server version information
- **THEN** the library sends `GET /api/status`
- **AND** parses `version.number` into a normalized semantic version value
- **AND** caches the result for cloned clients

#### Scenario: Capability matrix
- **WHEN** a consumer checks supported capabilities for a detected version
- **THEN** `spaces` and `saved_objects` require Kibana `8.0.0` or newer
- **AND** `agents` and `tools` require Kibana `9.2.0` or newer
- **AND** `workflows` requires Kibana `9.3.0` or newer

### Requirement: Public Error Model
The `kibana-sync` crate SHALL expose a dedicated public error enum and crate-local `Result<T>` alias instead of exposing `eyre::Report` in public APIs.

#### Scenario: Consumer matches error variants
- **WHEN** a library operation fails due to invalid configuration, an unknown space, unsupported capability, transport failure, serialization failure, version parsing failure, API response failure, or missing resource identifier
- **THEN** the returned error identifies the failure category with a documented enum variant
- **AND** consumers can match on the error without parsing display strings

#### Scenario: API response failure preserves status and body
- **WHEN** Kibana returns a non-success HTTP response for a library API operation
- **THEN** the error includes the HTTP status code
- **AND** includes the response body or a lossless body excerpt suitable for diagnostics

#### Scenario: CLI converts library errors at boundary
- **WHEN** the `kibob` CLI calls `kibana-sync`
- **THEN** it can convert library errors into CLI error reports and warning exit behavior without requiring the library to depend on `eyre`

### Requirement: Tracing Instrumentation
The `kibana-sync` crate SHALL use `tracing` for diagnostic instrumentation and SHALL NOT initialize global logging or tracing subscribers.

#### Scenario: Library emits tracing events
- **WHEN** the library sends requests, performs sync operations, applies capability gates, or skips resources
- **THEN** it emits diagnostic events through `tracing`
- **AND** does not emit those events through `log` macros

#### Scenario: Application owns subscriber configuration
- **WHEN** a consumer uses `kibana-sync`
- **THEN** the consumer controls whether and how tracing events are recorded by installing its own subscriber
- **AND** the library does not initialize or modify global subscriber state
