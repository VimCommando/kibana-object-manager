## Context

Kibob targets multiple Kibana versions, but API surface area differs by release. Saved objects and spaces are broadly available across the supported floor (8.0+), while `agent_builder/agents` and `agent_builder/tools` appeared as tech preview in 9.2 and reached GA in 9.3. Workflows APIs were introduced as tech preview in 9.3. Current pull/push/add flows can attempt unsupported endpoints and fail when users run against older Kibana clusters.

This change is cross-cutting: API orchestration lives in `src/cli.rs`, endpoint implementations live in extractor/loader modules, and request plumbing lives in the Kibana client layer. A single version-gating mechanism is needed so all flows evaluate support consistently.

## Goals / Non-Goals

**Goals:**
- Detect Kibana server version once and reuse it across command execution.
- Define a single API support matrix with minimum version per API family.
- Run a version preflight before command execution and avoid issuing unsupported API requests.
- Keep `saved_objects` and `spaces` always enabled for supported versions (8.0+).
- Make skip decisions visible through logs and command summaries, and return warning exit status when unsupported APIs are requested.
- Persist the source cluster version in `spaces.yml` during pull and enforce push compatibility against that recorded version.
- Surface minimum version + tech preview status for each API in CLI help and documentation.
- Provide an explicit `--force` escape hatch for advanced users to bypass version checks with clear warning output.
- Cover boundary versions in tests (8.x, 9.2.x, 9.3.x).

**Non-Goals:**
- Backport support below Kibana 8.0.
- Change payload schemas for supported endpoints.
- Enforce feature maturity policy (tech preview vs GA) beyond minimum-version gating.
- Add runtime dependency on external semver CLI tooling.

## Decisions

### 1) Add a normalized Kibana version model in client layer
- Decision: introduce a small internal `KibanaVersion` value type (major/minor/patch) parsed from Kibana info endpoint data and stored on `KibanaClient`.
- Rationale: command flows and API modules need consistent comparisons without repeatedly parsing strings.
- Alternatives considered:
  - Parse version strings ad hoc in each module: rejected due to duplication and drift risk.
  - Gate by trial-and-error (call endpoint, treat 404 as unsupported): rejected because it mixes capability detection with transient failures and generates noisy errors.

### 2) Centralize API minimum versions in an enum-backed matrix
- Decision: define an `ApiCapability` (or equivalent) with minimum supported versions:
  - `spaces`: 8.0.0
  - `saved_objects`: 8.0.0
  - `agents`: 9.2.0
  - `tools`: 9.2.0
  - `workflows`: 9.3.0
- Rationale: one authoritative mapping prevents inconsistencies between pull/push/add/bundle/dependency flows.
- Alternatives considered:
  - Hard-code thresholds in each command path: rejected due to maintenance cost.
  - Load thresholds from config: rejected for now because requirements are product-defined and static.

### 3) Perform mandatory preflight gating before API module invocation
- Decision: evaluate support in orchestration paths (`pull`, `push`, `add`, dependency enrichment, and bundle/togo) before creating extractor/loader work for any API.
- Rationale: preflight guarantees consistent behavior, prevents unsupported requests, and allows warning status decisions to be made deterministically.
- Alternatives considered:
  - Gate inside each extractor/loader: rejected because command summaries would require additional plumbing and modules would still be constructed.

### 4) Return warning status for unsupported requested APIs
- Decision: unsupported requested APIs are skipped with structured log lines and completion summaries, and commands return a warning exit status when any requested API is unsupported.
- Rationale: this preserves partial usefulness for supported APIs while still making version mismatch actionable in automation.
- Alternatives considered:
  - Silent skip with success exit status: rejected because unsupported requests can be missed in CI or scripted usage.

### 5) Persist and enforce cluster version provenance via `spaces.yml`
- Decision: write `kibana.version` (full semver) to `spaces.yml` after pull, then require push target version to be compatible with recorded version (same major with equal/newer minor, or newer major; patch differences ignored for compatibility check).
- Rationale: prevents accidental pushes from newer-exported object sets into older clusters with missing capabilities.
- Alternatives considered:
  - Store version in a separate state file: rejected because `spaces.yml` already acts as cluster/project metadata.
  - Require exact semver match for push: rejected because patch-level drift is acceptable and common.

### 6) Use versioned API request profiles where docs indicate behavior drift
- Decision: maintain request profile mappings keyed by capability/version band so tech preview and GA differences can be encoded without scattering conditionals.
- Rationale: keeps API differences explicit, testable, and easy to revise as documentation evolves.
- Alternatives considered:
  - Single request shape for all versions: rejected because tech preview to GA transitions may change payload constraints or endpoints.

### 7) Add `--force` override for version checks
- Decision: introduce `--force` on relevant commands so users can bypass version preflight and push compatibility floor checks, with explicit warnings that behavior is unsupported.
- Rationale: provides an intentional escape hatch for edge cases and debugging without weakening default safety.
- Alternatives considered:
  - No override: rejected because it blocks operators who need to test undocumented compatibility.

## Risks / Trade-offs

- [Risk] Kibana version format may include suffixes (e.g., snapshot labels) that break strict parsing. -> Mitigation: parse numeric components defensively and ignore suffix metadata.
- [Risk] Gating logic could diverge between command paths if some flow bypasses centralized helpers. -> Mitigation: expose one helper API and require all orchestrators to call it.
- [Risk] Users may not realize APIs were skipped. -> Mitigation: print concise skip reason with required vs detected version and include counts in final summary.
- [Risk] `spaces.yml` schema evolution may break backward compatibility with existing projects. -> Mitigation: add backward-compatible parsing where missing `kibana.version` is treated as unknown and triggers safe fallback messaging.
- [Risk] `--force` may cause partial writes or API errors on unsupported versions. -> Mitigation: print high-visibility warnings and preserve normal error handling/reporting.
- [Trade-off] Central matrix increases upfront refactor surface but reduces long-term drift and bug risk.

## Migration Plan

1. Add version retrieval and parsing to Kibana client initialization path.
2. Introduce capability matrix and helper methods (`is_supported`, `unsupported_reason`).
3. Wire gating into command orchestration and dependency resolution paths.
4. Add `spaces.yml` read/write support for `kibana.version` and push compatibility checks.
5. Add `--force` CLI wiring to bypass preflight/push floor checks with explicit warnings.
6. Update logs/summaries/help text to include required minimum versions and warning status semantics.
7. Add tests for version boundaries, warning exits, `--force`, and push compatibility floors.
8. Validate existing supported-version behavior remains unchanged.

Rollback strategy: revert to previous execution paths by removing capability checks while retaining unrelated refactors. No persistent data migration is required.

## Open Questions

- None currently.
