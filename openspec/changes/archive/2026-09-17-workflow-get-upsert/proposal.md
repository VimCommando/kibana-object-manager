## Why

Kibana can return 404 to HEAD for an existing Workflow.
The resulting create request then conflicts and prevents repeated synchronization from converging.

## What Changes

- Replace Workflow existence checks with version-aware GET requests.
- Define bounded recovery for a Workflow create conflict.
- Require a readable, writable Workflow document before an update.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `kibana-sync`: Make Workflow lookup and create-or-update synchronization reliable.
- `standalone-resource-transfer`: Define reliable Workflow import lookup and create-conflict recovery.

## Impact

- Affects `kibana-sync` Workflow loader HTTP behavior.
- Preserves Kibana 9.3 and 9.4+ routes, space prefixes, internal-origin headers, and structured outcomes.
