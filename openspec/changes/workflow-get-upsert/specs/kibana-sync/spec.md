## MODIFIED Requirements

### Requirement: Reusable Kibana API Modules
The `kibana-sync` crate SHALL expose reusable API modules for saved objects, spaces, agents, tools, skills, and workflows.

#### Scenario: Saved object import and export
- **WHEN** a consumer exports saved objects
- **THEN** the library sends `POST /api/saved_objects/_export` with a JSON export payload
- **AND** parses the NDJSON response into JSON values
- **WHEN** a consumer imports saved objects
- **THEN** the library sends `POST /api/saved_objects/_import?overwrite=<value>` as multipart form data with `Content-Type: multipart/form-data`

#### Scenario: Space management
- **WHEN** a consumer lists spaces
- **THEN** the library sends `GET /api/spaces/space`
- **WHEN** a consumer fetches a specific space
- **THEN** the library sends `GET /api/spaces/space/{id}`
- **WHEN** a consumer creates or updates a space
- **THEN** the library sends `POST /api/spaces/space` for create operations
- **AND** sends `PUT /api/spaces/space/{id}` for update operations

#### Scenario: Agent management
- **WHEN** a consumer lists agents
- **THEN** the library sends `GET /api/agent_builder/agents`
- **WHEN** a consumer fetches or checks an agent
- **THEN** the library sends `GET /api/agent_builder/agents/{id}` or `HEAD /api/agent_builder/agents/{id}`
- **WHEN** a consumer creates or updates an agent
- **THEN** the library sends `POST /api/agent_builder/agents` for create operations
- **AND** sends `PUT /api/agent_builder/agents/{id}` for update operations

#### Scenario: Tool management
- **WHEN** a consumer lists tools
- **THEN** the library sends `GET /api/agent_builder/tools`
- **WHEN** a consumer fetches or checks a tool
- **THEN** the library sends `GET /api/agent_builder/tools/{id}` or `HEAD /api/agent_builder/tools/{id}`
- **WHEN** a consumer creates or updates a tool
- **THEN** the library sends `POST /api/agent_builder/tools` for create operations
- **AND** sends `PUT /api/agent_builder/tools/{id}` for update operations

#### Scenario: Skill management
- **WHEN** a consumer lists skills
- **THEN** the library sends `GET /api/agent_builder/skills`
- **WHEN** a consumer fetches a skill
- **THEN** the library sends `GET /api/agent_builder/skills/{id}`
- **WHEN** a consumer creates or updates a skill
- **THEN** the library sends `POST /api/agent_builder/skills` for create operations
- **AND** sends `PUT /api/agent_builder/skills/{id}` for update operations
- **WHEN** a consumer deletes a skill
- **THEN** the library sends `DELETE /api/agent_builder/skills/{id}`

#### Scenario: Workflow management uses internal-origin header
- **WHEN** a consumer searches workflows
- **THEN** on Kibana 9.3 the library sends `POST /api/workflows/search`
- **THEN** on Kibana 9.4 or later the library sends `GET /api/workflows`
- **AND** includes `X-Elastic-Internal-Origin: Kibana`
- **WHEN** a consumer checks a workflow before synchronization
- **THEN** on Kibana 9.3 the library sends `GET /api/workflows/{id}`
- **THEN** on Kibana 9.4 or later the library sends `GET /api/workflows/workflow/{id}`
- **AND** includes `X-Elastic-Internal-Origin: Kibana`
- **WHEN** a consumer creates or updates a workflow
- **THEN** on Kibana 9.3 the library sends `POST /api/workflows` for create operations and `PUT /api/workflows/{id}` for writable existing workflows
- **THEN** on Kibana 9.4 or later the library sends `POST /api/workflows/workflow` for create operations and `PUT /api/workflows/workflow/{id}` for writable existing workflows
- **AND** includes `X-Elastic-Internal-Origin: Kibana`

#### Scenario: Readonly Workflow protection
- **WHEN** a workflow lookup returns a Workflow with `readonly: true`
- **THEN** the library reports failure without sending a PUT request

#### Scenario: Workflow create conflict recovery
- **WHEN** a Workflow lookup returns not found and its create request returns 409 Conflict
- **THEN** the library rechecks the Workflow with its version-aware GET item route
- **AND** updates it only when the response is a readable, writable Workflow document
- **AND** reports a failed Create when confirmation fails or the Workflow is readonly
