## MODIFIED Requirements

### Requirement: Idempotent JSON Resource Import
Standalone Tools, Agents, and Workflows imports SHALL use their existing create-or-update loaders and SHALL never delete, prune, or expand dependencies.

#### Scenario: Create or update Tool
- **GIVEN** a validated Tool with an authoritative `id`
- **WHEN** the command imports the Tool
- **THEN** it checks `HEAD /api/agent_builder/tools/{id}`
- **AND** sends `POST /api/agent_builder/tools` when missing or `PUT /api/agent_builder/tools/{id}` when present
- **AND** mutating requests include `kbn-xsrf: true`

#### Scenario: Create or update Agent
- **GIVEN** a validated Agent with an authoritative `id`
- **WHEN** the command imports the Agent
- **THEN** it checks `HEAD /api/agent_builder/agents/{id}`
- **AND** sends `POST /api/agent_builder/agents` when missing or `PUT /api/agent_builder/agents/{id}` when present
- **AND** omits response-only audit fields such as `created_by` from the mutating request
- **AND** mutating requests include `kbn-xsrf: true`

#### Scenario: Create or update Workflow
- **GIVEN** a validated Workflow with an authoritative `id`
- **WHEN** the command imports the Workflow
- **THEN** on Kibana 9.3 it checks `GET /api/workflows/{id}`
- **AND** sends `POST /api/workflows` when missing or `PUT /api/workflows/{id}` when present and writable
- **THEN** on Kibana 9.4 or later it checks `GET /api/workflows/workflow/{id}`
- **AND** sends `POST /api/workflows/workflow` when missing or `PUT /api/workflows/workflow/{id}` when present and writable
- **AND** every Workflow request includes `X-Elastic-Internal-Origin: Kibana`
- **AND** mutating requests include `kbn-xsrf: true`

#### Scenario: Reject readonly Workflow
- **GIVEN** a Workflow item GET returns a Workflow with `readonly: true`
- **WHEN** the command imports the Workflow
- **THEN** it reports failure without sending a PUT request

#### Scenario: Workflow create conflict recovery
- **GIVEN** the Workflow item GET returns 404
- **AND** the following Workflow POST returns 409 Conflict
- **WHEN** the command confirms the item with its version-aware Workflow GET route
- **THEN** it sends PUT only for a readable, non-readonly Workflow
- **AND** reports failure without PUT when the item remains missing, cannot be read, or is readonly

#### Scenario: JSON resource dependencies remain external
- **GIVEN** an imported Agent, Tool, or Workflow references another resource
- **WHEN** the command completes
- **THEN** it does not import the referenced resource implicitly
- **AND** it does not delete remote resources absent from the source
