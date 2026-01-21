# agents Specification

## Purpose
TBD - created by archiving change agent-api-support. Update Purpose after archive.
## Requirements
### Requirement: Agent API Compliance
The system SHALL interact with the Kibana Agents API using the correct conventions for creation and updates.

#### Scenario: Create Agent
- **WHEN** creating a new agent
- **THEN** the system sends a `POST` request to `/api/agent_builder/agents`
- **AND** the request body includes the `id`
- **AND** the request body DOES NOT include `readonly` or `schema` fields

#### Scenario: Update Agent
- **WHEN** updating an existing agent
- **THEN** the system sends a `PUT` request to `/api/agent_builder/agents/{id}`
- **AND** the request body DOES NOT include `id`, `readonly`, or `schema` fields

