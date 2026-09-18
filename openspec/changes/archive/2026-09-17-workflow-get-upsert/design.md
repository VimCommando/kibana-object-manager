## Context

The Workflow loader performs a version-aware create-or-update operation.
It uses an internal Kibana route and reports a `ResourceOutcome` for each input.

## Goals / Non-Goals

**Goals:**

- Select create or update using a reliable readable Workflow response.
- Preserve each route family and space prefix.
- Recover only the create race represented by POST 409.

**Non-Goals:**

- Change Tools, Agents, Skills, or the public client API.
- Add locking, retries beyond conflict confirmation, or rollback.

## Decisions

### Use the item GET as the lookup

GET returns the Workflow document required to enforce readonly protection.
HEAD is not reliable for Workflow existence in Kibana.

### Confirm before conflict recovery

After POST 409, GET the same item path again.
Only PUT a JSON object that is not readonly.

This avoids counting 409 as success when a different condition caused the conflict.

### Preserve outcome semantics

An initial missing result followed by successful POST is Create.
An existing result, or a successful conflict recovery PUT, is Update.
Any failed confirmation or PUT remains a failure.

## Risks / Trade-offs

- GET needs readable workflow JSON → malformed or unauthorized responses fail without mutation.
- A workflow may be deleted after confirmation → the PUT failure remains visible in the resource outcome.
- This does not verify live Kibana behavior → cover the route and response matrix through deterministic HTTP tests.

## Migration Plan

1. Release this compatible behavior correction as a patch release.
2. Consumers continue using the same routes and payloads with no configuration migration.
