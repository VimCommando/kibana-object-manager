## 1. Skills API Module

- [x] 1.1 Add `crates/kibana-sync/src/kibana/skills/` with module, directory storage/projection, extractor, and loader code.
- [x] 1.2 Implement Skill directory read/write support for `SKILL.md` YAML frontmatter, markdown body content, referenced markdown files, Skill ID directory naming, authoritative frontmatter `id`, and round-trip unit tests.
- [x] 1.3 Implement `SkillsExtractor` for list, optional `include_plugins=true`, and selected-id `GET /api/agent_builder/skills/{skillId}` fetches.
- [x] 1.4 Implement tolerant Skills list response parsing with tests for `results` arrays and top-level arrays.
- [x] 1.5 Implement `SkillsLoader` create/update upsert flow using `POST /api/agent_builder/skills` and `PUT /api/agent_builder/skills/{skillId}`.
- [x] 1.6 Add Skill directory to API JSON projection for create/update, including frontmatter fields, markdown content, deterministic generated `referenced_content`, root `relativePath: ""`, stable empty arrays, update removal of `id`, removal of `readonly`, and skipping system Skills with `readonly: true`.
- [x] 1.7 Add a Skills delete helper that supports `DELETE /api/agent_builder/skills/{skillId}` and `force=true`, preserving conflict details.

## 2. Sync Model and Capabilities

- [x] 2.1 Add `ApiCapability::Skills` with minimum version `9.4.0`, experimental-as-of-9.4 maturity note, display name, and capability matrix tests.
- [x] 2.2 Add Skills to `SyncSelection`, `SpaceBundle`, `SyncBundle`, `SyncSummary`, and default constructors.
- [x] 2.3 Wire Skills into `pull_sync`, `push_sync`, `plan_capabilities`, and unsupported API policy handling.
- [x] 2.4 Add `Dependency::Skill`, Skills dependency expansion capabilities, and fetch support for `api/agent_builder/skills/{skillId}`.
- [x] 2.5 Detect Skill dependencies from Agent and Workflow key variants and Tool dependencies from Skill `tool_ids`.
- [x] 2.6 Add unit tests for Skill dependency extraction and transitive Agent -> Skill -> Tool -> Workflow expansion.

## 3. Filesystem and Bundle Storage

- [x] 3.1 Extend `KibanaFsBundle` to read and write per-space `skills/{skill-directory}/SKILL.md` directories and referenced markdown files while preserving markdown content exactly.
- [x] 3.2 Include Skills in filesystem discovery, explicit selection, missing `SKILL.md` handling, missing resource errors, path escape rejection, and symlink escape rejection.
- [x] 3.3 Extend bundle NDJSON read/write paths to project Skill directories into `bundle/{space_id}/skills.ndjson` and hydrate NDJSON records back into Skill directories when writing filesystem bundles.
- [x] 3.4 Add per-space `manifest/skills.yml` tracked Skill manifests and enforce manifest order, filtering, and missing-resource errors when present.
- [x] 3.5 Add filesystem sync tests covering Skills-only bundles and mixed Agent Builder bundles.

## 4. CLI Integration

- [x] 4.1 Add `skills` and `skill` to API filter parsing and default pull/push/togo selections.
- [x] 4.2 Wire Skills into pull, push, add, dependency-enrichment, bundle, and togo command flows.
- [x] 4.3 Add `kibob add skill <skill-id>` behavior that fetches the Skill and writes `skills/{skill-directory}/SKILL.md` plus referenced markdown files.
- [x] 4.4 Add Skills counts and unsupported-skip details to pull, push, and togo summaries.
- [x] 4.5 Update command help and user documentation to describe Skills API support, skill-directory storage layout, frontmatter fields, referenced content projection, and version gating.

## 5. Tests and Verification

- [x] 5.1 Add mocked API tests for Skills list, include-plugins list, fetch, create, update, delete, and conflict handling.
- [x] 5.2 Add CLI-level tests for `--api skills`, singular alias `skill`, mixed API filters, and unsupported version warnings.
- [x] 5.3 Add live Kibana integration coverage for Skills, gated by `ApiCapability::Skills`.
- [x] 5.4 Add a live referenced-content round-trip test that exports `threat-hunting`, renames it to a new Skill id/name, imports the copy, verifies referenced content after re-export, and deletes the test copy with best-effort cleanup on failure.
- [x] 5.5 Run live validation by sourcing `.env.test` for `KIBANA_URL` and `KIBANA_APIKEY` at execution time only, targeting the `esdiag` space, without reading, logging, storing, or committing credentials.
- [x] 5.6 Run `cargo clippy` and fix any warnings introduced by Skills support.
- [x] 5.7 Run `cargo test` and confirm all tests pass.
