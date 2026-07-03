## Why

Kibana now exposes Skills as a first-class Agent Builder API alongside Agents, Tools, and Workflows. Kibob needs to manage these definitions directly so projects can round-trip user-created skills and keep agent-building assets portable across spaces and clusters.

## What Changes

- Add first-class Skills API support for pull, push, add, bundle, and togo flows.
- Introduce per-skill directories under each space, with `SKILL.md` as the authoritative at-rest representation.
- Add `SkillsExtractor`, `SkillsLoader`, and skill-directory model/projection types that use `/api/agent_builder/skills` and `/api/agent_builder/skills/{skillId}`.
- Preserve system skills during pull discovery while only writing user-created skills to project files by default.
- Support `GET /api/agent_builder/skills?include_plugins=true` for optional discovery when needed, without attempting to push or delete Skills marked `readonly: true`.
- Convert skill directories to Kibana JSON only when bundling to NDJSON or sending create/update requests to the Skills API.
- Create skills with `POST /api/agent_builder/skills` including `id`, and update existing skills with `PUT /api/agent_builder/skills/{skillId}` excluding path-owned or read-only fields.
- Add deletion semantics for future cleanup support using `DELETE /api/agent_builder/skills/{skillId}` with optional `force=true` when agents still reference the skill.
- Include skill dependencies in dependency enrichment: skills reference tools through `tool_ids`, and agents may reference skills once Kibana definitions expose those references.
- Add Skills to API filtering, version gating, command summaries, documentation, and live integration coverage.

## Capabilities

### New Capabilities

- `skills`: Defines filesystem, extraction, loading, markdown-to-JSON projection, and API behavior for Kibana Skills.

### Modified Capabilities

- `cli`: Add Skills to API filtering, command defaults, aliases, help text, and summaries.
- `kibana-sync`: Add Skills to reusable bundle read/write, capability gating, and filesystem sync contracts.
- `include-dependencies`: Add dependency traversal for skill-to-tool references and agent-to-skill references.

## Impact

- Affected code: `crates/kibana-sync/src/kibana`, `crates/kibana-sync/src/fs.rs`, `crates/kibana-sync/src/client/kibana.rs`, `crates/kibana-object-manager/src/cli.rs`, `crates/kibana-object-manager/src/main.rs`, documentation, and integration tests.
- API impact: uses Kibana Skills endpoints under `/api/agent_builder/skills`, including space-prefixed variants through the existing `SpaceClient`.
- Storage impact: adds per-space `manifest/skills.yml` tracked Skill manifests, `skills/<skill-id>/SKILL.md` directories with referenced markdown content, and `bundle/{space_id}/skills.ndjson` for bundle exports.
- Compatibility impact: Skills become a version-gated Agent Builder capability requiring Kibana `9.4.0` or newer, labeled experimental as of `9.4`, and skipped with the same unsupported-API warning behavior as Agents, Tools, and Workflows.
