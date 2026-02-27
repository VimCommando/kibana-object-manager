## Why

Kibana APIs in scope do not exist uniformly across supported versions, but the CLI currently attempts all selected APIs regardless of server version. We need deterministic version gating so operations succeed from Kibana 8.0 onward without unsupported endpoint failures.

## What Changes

- Add Kibana version detection and an API availability matrix keyed by minimum supported version and maturity milestone.
- Require a Kibana version preflight check before command execution and gate API execution for `agents`, `tools`, and `workflows` based on detected Kibana version.
- Keep `saved_objects` and `spaces` enabled across supported versions (8.0+).
- Skip unsupported APIs with clear logs/summaries, and return a warning exit status when any requested API is unsupported for the target Kibana version.
- Add a `--force` CLI argument to bypass version checks (with warning) and attempt API calls anyway.
- Apply gating consistently across pull, push, add, dependency fetch, and bundle/togo flows.
- Persist `kibana.version: <full-semver>` in `spaces.yml` after pull.
- Block push to a target Kibana cluster older than the recorded `kibana.version` minor compatibility floor (equal/newer minor allowed; patch differences allowed).
- Document API minimum versions and tech preview status in CLI help and project documentation for each API.
- Validate and encode version-specific API request profiles where tech preview and GA behavior differ.
- Add tests covering boundary versions and mixed API selections.

## Capabilities

### New Capabilities
- `workflows`: Define version-aware behavior for workflows APIs (introduced as tech preview in 9.3) so unsupported versions are skipped cleanly.

### Modified Capabilities
- `kibana-client`: Add server version discovery and reusable API support checks.
- `agents`: Require version-gated access to `agent_builder/agents` (tech preview in 9.2, GA in 9.3).
- `tools`: Require version-gated access to `agent_builder/tools` (tech preview in 9.2, GA in 9.3).
- `cli`: Require command preflight gating, warning exit behavior for unsupported requests, and `spaces.yml` Kibana version provenance/push guardrails.

## Impact

- Affected code: `src/kibana/client.rs`, API extractors/loaders for agents/tools/workflows, and orchestration in `src/cli.rs`.
- Behavioral impact: mixed-version compatibility improves; unsupported API requests are identified up front with warning exits, and push compatibility is guarded by recorded pull version.
- Test impact: add version matrix tests for 8.x, 9.2, and 9.3 boundary behavior.
