## ADDED Requirements

### Requirement: Workflows API Version Gating
The system SHALL treat workflows APIs as supported only when Kibana server version is 9.3.0 or newer.

#### Scenario: Skip workflows on unsupported Kibana versions
- **WHEN** the user runs a workflow-related operation and Kibana version is lower than 9.3.0
- **THEN** the system SHALL skip workflow API calls
- **AND** the system SHALL log that workflows require Kibana 9.3.0+

#### Scenario: Execute workflows on supported Kibana versions
- **WHEN** the user runs a workflow-related operation and Kibana version is 9.3.0 or newer
- **THEN** the system SHALL execute workflow API calls against `/api/workflows` endpoints
- **AND** the system SHALL include `X-Elastic-Internal-Origin: Kibana` on internal API requests

### Requirement: Workflows API Profile Evolution
The system SHALL support version-aware workflows API request profiles so documented endpoint or payload differences can be applied across versions.

#### Scenario: Select profile by Kibana version
- **WHEN** the system prepares a workflows API request
- **THEN** the system SHALL resolve the workflows request profile for the detected Kibana version
- **AND** request method/path/body SHALL follow that resolved profile
