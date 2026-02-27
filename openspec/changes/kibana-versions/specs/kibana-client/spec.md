## ADDED Requirements

### Requirement: Kibana Version Discovery
The `KibanaClient` SHALL discover and expose the Kibana server version for use by command orchestration and API modules.

#### Scenario: Version is available after client initialization
- **WHEN** a `KibanaClient` is created successfully
- **THEN** the client SHALL store a normalized Kibana version value
- **AND** downstream code SHALL be able to query that version without additional HTTP calls

### Requirement: API Capability Matrix
The `KibanaClient` SHALL provide centralized API support checks using minimum-version thresholds.

#### Scenario: Support check for always-supported APIs
- **WHEN** the system checks support for `spaces` or `saved_objects`
- **THEN** the check SHALL return supported for Kibana 8.0.0 and newer

#### Scenario: Support check for agents and tools
- **WHEN** the system checks support for `agents` or `tools`
- **THEN** the check SHALL return unsupported when Kibana version is lower than 9.2.0
- **AND** the check SHALL return supported when Kibana version is 9.2.0 or newer

#### Scenario: Support check for workflows
- **WHEN** the system checks support for `workflows`
- **THEN** the check SHALL return unsupported when Kibana version is lower than 9.3.0
- **AND** the check SHALL return supported when Kibana version is 9.3.0 or newer

### Requirement: Versioned API Request Profiles
The `KibanaClient` SHALL provide version-aware request profile selection for APIs that may differ between tech preview and GA releases.

#### Scenario: Resolve request profile by capability and Kibana version
- **WHEN** an API module requests request profile metadata for a capability
- **THEN** the client SHALL resolve profile data using detected Kibana version and capability
- **AND** the returned profile SHALL include endpoint and payload guidance used by that module

#### Scenario: Use GA profile on or after GA version
- **WHEN** Kibana version is at or above an API's GA threshold
- **THEN** the client SHALL return the GA request profile for that capability
- **AND** API modules SHALL use the GA endpoint/payload mapping for requests
