## MODIFIED Requirements

### Requirement: Workflows Loader Payload Sanitization
The `WorkflowsLoader` MUST sanitize workflow JSON payloads before sending them to the Kibana API. It MUST remove read-only system fields to prevent 400 Bad Request errors.

#### Scenario: Push strips system fields
Given a workflow JSON containing system fields (`createdAt`, `lastUpdatedAt`, `createdBy`, `lastUpdatedBy`)
When the loader prepares the payload for `create_workflow` or `update_workflow`
Then the system fields are removed from the JSON payload sent to the API
And the core fields (`id`, `name`, `yaml`, `definition`) are preserved

### Requirement: Workflows Loader Create Method and Path
The `WorkflowsLoader` MUST use the `POST` HTTP method to the `api/workflows` endpoint when creating a new workflow.

#### Scenario: Create uses POST to root collection
Given a workflow ID that does not exist on the server
When `upsert_workflow` calls `create_workflow`
Then the loader sends a `POST` request to `/api/workflows`
And the payload includes the `id` field
And the payload is sanitized

### Requirement: Workflows Loader Update Method
The `WorkflowsLoader` MUST use the `PUT` HTTP method when updating an existing workflow.

#### Scenario: Update uses PUT
Given a workflow ID that exists on the server
When `upsert_workflow` calls `update_workflow`
Then the loader sends a `PUT` request to `/api/workflows/{id}`
And the payload is sanitized
