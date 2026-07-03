## Context

Kibob already treats Agents, Tools, and Workflows as first-class Kibana API families that can be pulled, pushed, filtered, bundled, and dependency-expanded per space. Kibana now adds Skills under the Agent Builder API surface:

- `GET /api/agent_builder/skills` lists built-in and user-created skills, with `include_plugins=true` enabling plugin-provided skills.
- `POST /api/agent_builder/skills` creates user-defined skills.
- `GET /api/agent_builder/skills/{skillId}` fetches one skill.
- `PUT /api/agent_builder/skills/{skillId}` updates a user-created skill.
- `DELETE /api/agent_builder/skills/{skillId}` deletes a user-created skill, with `force=true` removing agent references first.

The implementation should follow the existing ETL pipeline shape: API-specific extractors and loaders in `kibana-sync`, storage-neutral `SyncBundle` resources, explicit filesystem bundle read/write, and CLI orchestration in `kibana-object-manager`.

## Goals / Non-Goals

**Goals:**

- Add Skills as a peer resource family to Agents, Tools, and Workflows.
- Preserve the documented create/update payload differences: create includes `id`; update uses `skillId` in the path and sends mutable fields in the body.
- Keep user-created Skills portable using per-skill directories with `SKILL.md` frontmatter and markdown content files.
- Avoid pushing or deleting system skills identified by `readonly: true`.
- Add Skills to API filters, capability gating, filesystem sync, bundle/togo flows, summaries, and dependency expansion.
- Detect Skills dependencies on Tools through `tool_ids`, and detect Agent references to Skills when agent payloads expose skill reference keys.
- Validate referenced-content export/import with the built-in `threat-hunting` Skill by cloning it to a new user-created Skill for tests.
- Reuse existing request helpers, error handling, concurrency, and space prefixing.

**Non-Goals:**

- Implement destructive cleanup as part of ordinary push.
- Manage system Skills identified by `readonly: true` as editable project assets.
- Introduce a schema-specific Rust model for every possible Skill field before the Kibana API stabilizes.
- Change Saved Objects, Spaces, Agents, Tools, or Workflows payload behavior except where dependency expansion needs to understand Skills.

## Decisions

### 1) Model Skills as a new API module, not as saved objects

Add `kibana::skills` with `SkillsExtractor`, `SkillsLoader`, and skill directory projection types, mirroring the existing Agents and Tools module boundaries while using a markdown-first storage model.

Rationale: the Skills API has explicit Agent Builder endpoints and payload rules. Treating it as saved objects would bypass the documented API behavior and space-aware Agent Builder conventions.

Alternatives considered:

- Fold Skills into Agents or Tools modules: rejected because Skills have their own collection, item, update, and delete semantics.
- Use a generic Agent Builder resource module immediately: rejected for now because Agents, Tools, and Skills have different readonly filtering and payload sanitization rules. A later refactor can consolidate shared code after behavior is proven.

### 2) Store Skills as skill directories, not JSON blobs

Use a markdown-first per-space layout:

```text
<space>/
  skills/
    <skill-id-or-slug>/
      SKILL.md
      <relativePath>/
        <referenced-file>.md
bundle/<space>/skills.ndjson
```

`SKILL.md` is the source of truth at rest. It contains YAML frontmatter with `id`, `name`, `description`, `tool_ids`, and `experimental`, followed by the markdown body that maps to the API `content` field. Additional markdown files under the skill directory are converted into `referenced_content` entries when generating API or NDJSON JSON.

Referenced content is derived from the directory structure:

- `name`: markdown filename without the `.md` extension
- `relativePath`: subdirectory path relative to the skill directory
- `content`: file contents

The generated `referenced_content` objects contain only those three fields.

Use a sanitized form of the Skill `id` as the default skill directory name, but treat `SKILL.md` frontmatter `id` as authoritative. For markdown files directly under the skill directory, generate `relativePath: ""`. Generate referenced content in deterministic relative-path order. Preserve markdown bytes as text exactly through export/import, including newline content. Reject referenced files that escape the skill directory, including symlink traversal. When writing from JSON, sanitize filesystem names for safety but preserve original JSON `id`, `name`, and `relativePath` values for round trips.

Rationale: Skills are author-facing markdown assets. Keeping them as directories avoids forcing users to edit a JSON blob and mirrors the shape of a standard skill package while still preserving a deterministic JSON projection for Kibana.

Alternatives considered:

- Store the API payload as `skills/*.json5`: rejected because the desired at-rest representation is a standard skill directory, with JSON produced only for NDJSON bundles and API calls.
- Keep `referenced_content` in `SKILL.md` frontmatter: rejected because referenced content should be derived from actual markdown files in the directory tree.

### 3) Project between skill directories and Kibana JSON

Load API responses as tolerant JSON values, then write user-created Skills to the directory representation. Before create/update, build the API JSON payload from the directory representation:

- Require `id` for addressing and create.
- Skip system skills marked with `readonly: true`.
- Read `id`, `name`, `description`, `tool_ids`, and `experimental` from `SKILL.md` frontmatter.
- Read API `content` from the `SKILL.md` markdown body.
- Build `referenced_content` by walking markdown files under the skill directory, excluding `SKILL.md`.
- Remove read-only/system fields such as `readonly`, `schema`, `type`, `built_in`, `source`, `created_at`, and `updated_at` when projecting API responses.
- Remove `id` from update bodies because `skillId` is path-owned.
- Serialize `tool_ids` and `referenced_content` as stable arrays, including empty arrays when absent in local files.

Rationale: the published API is JSON, but the desired filesystem representation is markdown-first. A projection layer keeps API details at the boundary and makes local editing natural.

Alternatives considered:

- Strict typed `Skill` struct: rejected until the response schema is stable and fully known.
- Send pulled payloads back verbatim: rejected because built-in/read-only metadata can make create/update fail.

### 4) Add explicit discovery options for plugin skills

Default pull discovery should list user-created and system Skills without `include_plugins=true`, then filter Skills with `readonly: true` out of persisted project files. The list response includes descriptions and the `readonly` marker; `GET /api/agent_builder/skills/{skillId}` includes content and also preserves the `readonly` marker. Add an extractor option for `include_plugins=true` so future add/discovery flows can inspect plugin-provided Skills without treating readonly Skills as pushable assets.

Rationale: the list endpoint can include system skills, but the create/update/delete endpoints operate on user-created skills. Persisting `readonly: true` skills by default would create unpushable project state.

Alternatives considered:

- Always request plugin skills: rejected because it increases noise and the risk of writing definitions that cannot be managed.
- Never support plugin discovery: rejected because agents may reference plugin skills and diagnostics may need to resolve them.

### 5) Extend dependency expansion with Skills

Add `Dependency::Skill(String)` and include `skills` in `DependencyExpansionCapabilities`, `SpaceBundle`, `SyncSelection`, and `SyncSummary`. Dependency detection should:

- Read `tool_ids` from Skill definitions as Tool dependencies.
- Recursively detect `skill_id`, `skillId`, `skill_ids`, and `skillIds` in Agent and Workflow payloads as Skill dependencies.
- Continue transitive traversal across Agent, Skill, Tool, and Workflow resources while respecting selected and supported API families.

Rationale: Skills can reference Tools directly, and Agents are expected to reference Skills as part of Agent Builder configurations. Dependency expansion should keep bundles complete when users add or pull a subset.

Alternatives considered:

- Only support direct Skill to Tool dependencies: rejected because adding an Agent would still miss referenced Skills.
- Add dependencies only at CLI level: rejected because `kibana-sync` already owns storage-neutral dependency expansion.

### 6) Gate Skills with the existing capability matrix

Add `ApiCapability::Skills` and wire it through `plan_capabilities`, `pull_sync`, `push_sync`, CLI filters, and summaries. Skills require Kibana `9.4.0` or newer and should be labeled `experimental as of 9.4` in CLI help and documentation. No earlier Kibana version should be treated as supporting the Skills API.

Rationale: Skills are an Agent Builder API peer and should use the same deterministic skip/warn/force behavior as other versioned APIs.

Alternatives considered:

- Do not gate Skills initially: rejected because unsupported endpoints would cause noisy failures on older Kibana.
- Gate only in the CLI: rejected because library consumers should get the same behavior.

### 7) Validate referenced content by cloning `threat-hunting`

Use the built-in `threat-hunting` Skill as the live validation source because it includes out-of-the-box `referenced_content`. Validation should fetch/export `threat-hunting`, write it to the skill directory format, rename the Skill `id` and `name`, and import the renamed copy as a new user-created Skill. The import projection must not send `readonly` or other system metadata from the source Skill. The renamed test Skill should be deleted at the end, with best-effort cleanup if validation fails after creation.

Live validation may source `KIBANA_URL` and `KIBANA_APIKEY` from `.env.test` at execution time and should target the `esdiag` space. These credentials must not be read into planning artifacts, logged, written to generated files, or committed.

Rationale: this exercises the real API representation and the real referenced-content directory mapping without weakening the default rule that `readonly: true` Skills are not pushable.

Alternatives considered:

- Push `threat-hunting` with a read-only override: rejected because it would blur production behavior and validation-only behavior.
- Use only mocked referenced content: rejected because it would miss shape differences present in Kibana's out-of-the-box Skills.

## Risks / Trade-offs

- [Risk] The Skills response shape may differ from Agents and Tools list responses. -> Mitigation: parse both `results` arrays and top-level arrays where practical, and cover with mocked API tests plus live integration tests.
- [Risk] The `readonly: true` marker may not be enough if future experimental responses add another non-editable Skill class. -> Mitigation: centralize the editable-skill predicate so additional markers can be added without changing orchestration.
- [Risk] The Skills API is experimental in Kibana 9.4 and may change before GA. -> Mitigation: isolate the threshold and maturity label in `ApiCapability::Skills` so request profiles and help text can be revised without touching orchestration.
- [Risk] Agents may expose skill references under fields not known today. -> Mitigation: recursively search common key variants and keep dependency parsing extensible.
- [Trade-off] Projecting Skills at the API boundary adds conversion code, but it keeps local edits in the desired markdown directory format.

## Migration Plan

1. Add the `skills` module in `kibana-sync` with directory parser/writer, extractor, loader, and delete helper tests.
2. Extend `ApiCapability`, `SyncSelection`, `SpaceBundle`, `SyncBundle`, `SyncSummary`, and dependency expansion with Skills.
3. Extend filesystem bundle read/write and bundle NDJSON flows for `skills/<skill-id>/SKILL.md`, referenced markdown files, and `skills.ndjson`.
4. Wire CLI filters, aliases, pull/push/add/togo orchestration, command summaries, and documentation.
5. Add unit tests for skill directory round-trips, payload projection, capability planning, filesystem sync, and dependency extraction.
6. Add mocked API tests for documented Skills endpoints and live integration tests gated by server capability.
7. Add a live validation path that clones `threat-hunting` to a new Skill id/name and verifies referenced-content export/import round trips.
8. Run live validation by sourcing `.env.test` for Kibana credentials at command execution time only, targeting the `esdiag` space, and ensuring no secrets are emitted or persisted.
9. Rollback by removing Skills from capability/filter selection and leaving unrelated resource families unchanged. Existing projects without `skills/` directories continue to read normally.

## Open Questions

- None currently.
