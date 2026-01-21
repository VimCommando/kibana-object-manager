# Change: Agent API Support

## Why
The Agent API implementation needs to align with the recently updated Tools API conventions to ensure consistent behavior and avoid API errors during push/pull operations. Specifically, we need to handle JSON5 parsing correctly, ensure correct HTTP methods and URL structures for creation and updates, and manage read-only fields appropriately.

## What Changes
- **JSON5 Support**: Verify/Ensure agent files are read/written with correct JSON5 support, including triple-quoted strings for multiline fields (like instructions).
- **API Create (POST)**: 
    - Use `POST /api/agent_builder/agents` (without ID in URL).
    - Include ID in the request body.
    - Remove `readonly` and `schema` fields from the body.
- **API Update (PUT)**:
    - Use `PUT /api/agent_builder/agents/{id}`.
    - Remove `id`, `readonly`, and `schema` fields from the request body.

## Impact
- **Affected Specs**: `agents`
- **Affected Code**: `src/kibana/agents/loader.rs`
