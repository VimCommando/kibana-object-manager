## MODIFIED Requirements

### Requirement: Agent API Compliance
The system SHALL interact with the Kibana Agents API using the correct conventions for creation and updates, and SHALL only execute these operations when Kibana version is 9.2.0 or newer.

#### Scenario: Create Agent
- **WHEN** creating a new agent on Kibana 9.2.0 or newer
- **THEN** the system sends a `POST` request to `/api/agent_builder/agents`
- **AND** the request body includes the `id`
- **AND** the request body DOES NOT include `readonly` or `schema` fields

#### Scenario: Update Agent
- **WHEN** updating an existing agent on Kibana 9.2.0 or newer
- **THEN** the system sends a `PUT` request to `/api/agent_builder/agents/{id}`
- **AND** the request body DOES NOT include `id`, `readonly`, or `schema` fields

#### Scenario: Skip agent operations on unsupported Kibana versions
- **WHEN** an agent operation is requested and Kibana version is lower than 9.2.0
- **THEN** the system SHALL skip calls to `/api/agent_builder/agents` endpoints
- **AND** the system SHALL report that agents require Kibana 9.2.0+

## ADDED Requirements

### Requirement: Agent API Version Profile Selection
The system SHALL apply the agent API request profile that matches the connected Kibana version.

#### Scenario: Use tech preview profile on Kibana 9.2.x
- **WHEN** the system executes agent operations against Kibana 9.2.x
- **THEN** the system SHALL use the agent API endpoint/payload profile documented for 9.2 tech preview behavior

#### Scenario: Use GA profile on Kibana 9.3.x and newer
- **WHEN** the system executes agent operations against Kibana 9.3.x or newer
- **THEN** the system SHALL use the agent API endpoint/payload profile documented for GA behavior
