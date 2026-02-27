## 1. Kibana Version and Capability Model

- [x] 1.1 Add a normalized Kibana version type and parser in the Kibana client layer.
- [x] 1.2 Fetch and store Kibana server version during client initialization.
- [x] 1.3 Define centralized API capability minimum versions (`spaces`, `saved_objects`, `agents`, `tools`, `workflows`).
- [x] 1.4 Expose helper methods to check API support and return human-readable unsupported reasons.

## 2. Command Flow Gating

- [x] 2.1 Apply capability checks in `pull` orchestration before invoking per-API extractors.
- [x] 2.2 Apply capability checks in `push` orchestration before invoking per-API loaders.
- [x] 2.3 Apply capability checks in `add` and dependency-enrichment paths (agents/tools/workflows).
- [x] 2.4 Apply capability checks in bundle/togo flows and ensure unsupported APIs are skipped.
- [x] 2.5 Add `--force` flag plumbing to bypass preflight version checks with explicit warning output.

## 3. API Module Integration and Reporting

- [x] 3.1 Ensure agent API calls (`/api/agent_builder/agents`) are only reachable on Kibana 9.2.0+.
- [x] 3.2 Ensure tools API calls (`/api/agent_builder/tools`) are only reachable on Kibana 9.2.0+.
- [x] 3.3 Ensure workflows API calls (`/api/workflows`) are only reachable on Kibana 9.3.0+.
- [x] 3.4 Update logs and command summaries to report skipped APIs with required and detected versions.
- [x] 3.5 Add and persist `kibana.version` in `spaces.yml` on pull.
- [x] 3.6 Enforce push compatibility floor from recorded `kibana.version`, with `--force` bypass behavior.
- [x] 3.7 Update command help/documentation to show per-API minimum versions and tech preview labels.

## 4. Tests and Verification

- [x] 4.1 Add unit tests for version parsing/comparison and capability matrix thresholds.
- [x] 4.2 Add command-level tests covering boundary versions (8.x, 9.2.x, 9.3.x) with mixed API selections and warning exit semantics.
- [x] 4.3 Add tests for `--force` behavior across unsupported API checks and push version-floor enforcement.
- [x] 4.4 Run `cargo clippy` and fix any new warnings introduced by version-gating changes.
- [x] 4.5 Run `cargo test` and confirm all tests pass.
