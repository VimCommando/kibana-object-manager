# kibana-client Specification

## Purpose
TBD - created by archiving change refactor-space-into-kibana-client. Update Purpose after archive.
## Requirements
### Requirement: Space-Aware Client Architecture

The Kibana client SHALL be split into two structs:
- `KibanaClient`: Root client that holds shared HTTP client, base URL, and space registry
- `SpaceClient`: Space-bound view that automatically prefixes API paths with space

#### Scenario: Create space-bound client
- **WHEN** calling `kibana.space("marketing")`
- **THEN** return `Ok(SpaceClient)` if "marketing" exists in loaded spaces
- **THEN** return `Err` if "marketing" is not in the spaces manifest

#### Scenario: Default space handling
- **WHEN** calling `kibana.space("default")`
- **THEN** return `SpaceClient` with `space: None`
- **THEN** API paths SHALL NOT be prefixed with `/s/default/`

#### Scenario: Non-default space path prefixing
- **WHEN** `SpaceClient` with `space: Some("marketing")` calls `get("/api/saved_objects")`
- **THEN** the actual request path SHALL be `/s/marketing/api/saved_objects`

### Requirement: Space Registry from Manifest

`KibanaClient` SHALL load the space registry from `spaces.yml` at construction time.

#### Scenario: Load spaces from manifest
- **WHEN** `KibanaClient::try_new(url, auth, project_dir)` is called
- **THEN** read `{project_dir}/spaces.yml` using `SpacesManifest::read()`
- **THEN** store space entries as `HashMap<String, String>` (id -> name)

#### Scenario: No manifest defaults to single space
- **WHEN** `spaces.yml` does not exist in project directory
- **THEN** default to single space: `{"default": "Default"}`

### Requirement: Space Query Methods

`KibanaClient` SHALL provide methods to query available spaces.

#### Scenario: List space IDs
- **WHEN** calling `kibana.space_ids()`
- **THEN** return `Vec<&str>` of all space IDs from the registry

#### Scenario: Get space name
- **WHEN** calling `kibana.space_name("marketing")`
- **THEN** return `Some("Marketing")` if space exists
- **THEN** return `None` if space does not exist

