## MODIFIED Requirements

### Requirement: Tools Push Support
The system SHALL support pushing tool definitions to Kibana via the `/api/agent_builder/tools` API, correctly handling JSON5 files with multiline strings, and SHALL only execute tools operations when Kibana version is 9.2.0 or newer.

#### Scenario: Push successfully reads and uploads tools
- **WHEN** the user runs `kibob push` with tools in the project directory on Kibana 9.2.0 or newer
- **THEN** the system reads the tool JSON files, including those with triple-quoted multiline strings
- **AND** successfully creates or updates the tools in Kibana

#### Scenario: Roundtrip preservation of multiline queries
- **GIVEN** a tool with a multiline query (e.g. ES|QL)
- **WHEN** the tool is pulled and then pushed back on Kibana 9.2.0 or newer
- **THEN** the query content is preserved exactly as is, without corruption of newlines or special characters

#### Scenario: Skip tools operations on unsupported Kibana versions
- **WHEN** a tools operation is requested and Kibana version is lower than 9.2.0
- **THEN** the system SHALL skip calls to `/api/agent_builder/tools` endpoints
- **AND** the system SHALL report that tools require Kibana 9.2.0+

## ADDED Requirements

### Requirement: Tools API Version Profile Selection
The system SHALL apply the tools API request profile that matches the connected Kibana version.

#### Scenario: Use tech preview profile on Kibana 9.2.x
- **WHEN** the system executes tools operations against Kibana 9.2.x
- **THEN** the system SHALL use the tools endpoint/payload profile documented for 9.2 tech preview behavior

#### Scenario: Use GA profile on Kibana 9.3.x and newer
- **WHEN** the system executes tools operations against Kibana 9.3.x or newer
- **THEN** the system SHALL use the tools endpoint/payload profile documented for GA behavior
