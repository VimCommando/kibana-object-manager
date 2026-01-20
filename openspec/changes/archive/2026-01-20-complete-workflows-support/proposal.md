# Complete Workflows API Support

## Problem
The initial Workflows API support was incomplete and failing on `push` operations. Specifically:
1.  **Payload Rejection:** The API rejected payloads containing read-only system fields (e.g., `createdAt`, `createdBy`).
2.  **Incorrect HTTP Methods:** The original implementation used `POST` for updates, then `PUT` for everything (causing 500s on create), and `POST` with incorrect paths for creation.

## Solution
This consolidated change implements a robust `WorkflowsLoader` that correctly interacts with the Kibana Workflows API:

1.  **Payload Sanitization:** A `sanitize_workflow` helper strips read-only system fields (`createdAt`, `lastUpdatedAt`, `createdBy`, `lastUpdatedBy`, `valid`, `validationErrors`, `history`) while retaining core definition fields (`id`, `name`, `yaml`, `definition`, `tags`, `description`, `enabled`).
2.  **Creation Logic:** Uses `POST /api/workflows` with the ID in the body when the workflow does not exist.
3.  **Update Logic:** Uses `PUT /api/workflows/{id}` when the workflow already exists.

## Scope
- `src/kibana/workflows/loader.rs`: Implement sanitization, create, and update logic.
- `src/client/kibana.rs`: Add `put_json_value_internal` helper if missing.

## Risks
- If the API schema changes or new read-only fields are added, the sanitization list might need updating.
- The `definition` field is retained as it appears to be required by the API, despite being derived from `yaml`.
