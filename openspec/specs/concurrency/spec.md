# concurrency Specification

## Purpose
This specification defines the concurrency control mechanisms for the Kibana API interaction and asset processing, ensuring high performance while protecting the Kibana server from overload.
## Requirements
### Requirement: Global API Concurrency Limit
`KibanaClient` SHALL implement a global limit on the number of simultaneous "in-flight" HTTP requests across all instances (including space-scoped clones).

#### Scenario: Enforce concurrency limit
- **GIVEN** `KibanaClient` initialized with `MAX_REQUESTS = 2`
- **WHEN** making 3 simultaneous API requests
- **THEN** the first 2 requests SHALL be sent immediately
- **THEN** the 3rd request SHALL wait until one of the first 2 completes

#### Scenario: Configurable limit
- **WHEN** `KibanaClient` is initialized
- **THEN** the limit SHALL be configurable via the constructor
- **AND** SHALL default to 8 if not specified via environment variables

### Requirement: Concurrent Asset Processing
The application SHALL perform asset-level operations concurrently to minimize total execution time.

#### Scenario: Concurrent space processing
- **GIVEN** multiple spaces are being processed during `pull` or `push`
- **THEN** spaces SHALL be processed concurrently up to the global limit

#### Scenario: Concurrent item extraction
- **GIVEN** an extractor is fetching multiple items by ID (e.g. workflows)
- **THEN** the items SHALL be fetched concurrently up to the global limit

