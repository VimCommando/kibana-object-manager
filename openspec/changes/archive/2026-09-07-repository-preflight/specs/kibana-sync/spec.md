## ADDED Requirements

### Requirement: Configurable request deadlines
The library SHALL bound HTTP requests with configurable, nonzero deadlines. The default request deadline SHALL be 300 seconds including response body reads, and the default connection deadline SHALL be 10 seconds. Waiting for a shared concurrency permit is outside the HTTP request deadline.

#### Scenario: A server stalls
- **GIVEN** a server accepts a request but does not respond within the configured request deadline
- **WHEN** the client awaits its response
- **THEN** the library returns a transport timeout error

#### Scenario: Invalid deadline
- **WHEN** a consumer configures a zero request or connection deadline
- **THEN** client construction fails before connecting

#### Scenario: CLI deadline configuration
- **WHEN** the CLI reads KIBANA_REQUEST_TIMEOUT or KIBANA_CONNECT_TIMEOUT
- **THEN** it requires a positive integer number of seconds
- **AND** passes those values to the library without changing the package's default concurrency
