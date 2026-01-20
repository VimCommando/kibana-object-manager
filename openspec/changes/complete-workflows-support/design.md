# Design: Workflows API Integration

## Context
The Kibana Workflows API (`/api/workflows`) has specific requirements for creating and updating workflows that differ from other Saved Objects APIs.

## API Specification

### Create Workflow
- **Method:** `POST`
- **Path:** `/api/workflows`
- **Body:** JSON object including `id`, `name`, `yaml`, `definition`, etc.
- **Constraints:** Must NOT include system fields (`createdAt`, etc.).

### Update Workflow
- **Method:** `PUT`
- **Path:** `/api/workflows/{id}`
- **Body:** JSON object including `id`, `name`, `yaml`, `definition`, etc.
- **Constraints:** Must NOT include system fields.

## Payload Sanitization
We implement a `sanitize_workflow` function to ensure payloads are accepted.

**Stripped Fields:**
- `createdAt`
- `lastUpdatedAt`
- `createdBy`
- `lastUpdatedBy`
- `valid`
- `validationErrors`
- `history`

**Retained Fields:**
- `id`
- `name`
- `description`
- `enabled`
- `yaml`
- `definition`
- `tags`

## Implementation Details
- `upsert_workflow` checks existence using `HEAD`.
- If exists -> `update_workflow` (`PUT`).
- If missing -> `create_workflow` (`POST`).
