## ADDED Requirements
### Requirement: Tools Push Support
The system SHALL support pushing tool definitions to Kibana via the `/api/agent_builder/tools` API, correctly handling JSON5 files with multiline strings.

#### Scenario: Push successfully reads and uploads tools
- **WHEN** the user runs `kibob push` with tools in the project directory
- **THEN** the system reads the tool JSON files, including those with triple-quoted multiline strings
- **AND** successfully creates or updates the tools in Kibana

#### Scenario: Roundtrip preservation of multiline queries
- **GIVEN** a tool with a multiline query (e.g. ES|QL)
- **WHEN** the tool is pulled and then pushed back
- **THEN** the query content is preserved exactly as is, without corruption of newlines or special characters
