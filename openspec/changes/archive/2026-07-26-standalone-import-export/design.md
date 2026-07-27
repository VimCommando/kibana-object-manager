## Context

The existing `pull` and `push` commands orchestrate multiple API families around a kibob project. Their paths are project roots, space selection may come from `spaces.yml`, Skills may be selected by `manifest/skills.yml`, pull persists project metadata, and dependency expansion may add related resources.

The lower-level pieces required for standalone transfer already exist:

- Extractors list and fetch Skills, Tools, Agents, and Workflows through the space-aware client.
- Loaders create or update each of those resource families.
- Existing project storage represents Skills as `SKILL.md` directories and Tools, Agents, and Workflows as individual JSON/JSON5 files.
- Existing transforms preserve multiline Tool queries, Agent instructions, and Workflow YAML.
- The client exposes version detection and an independent capability gate for each family.

The missing boundary is orchestration that treats a caller-provided path as an artifact rather than a project. That boundary must not call project manifest discovery, project version persistence, dependency expansion, or the manifest-aware Skills directory helpers.

## Goals / Non-Goals

**Goals:**

- Add unambiguous `kibob import <resource> <source>` and `kibob export <resource> <destination>` commands for `skills`, `tools`, `agents`, and `workflows`.
- Transfer exactly one API family and one Kibana space per invocation.
- Reuse each family's existing API, storage representation, transforms, and loader instead of defining new artifact formats.
- Validate complete local import batches before remote mutation.
- Fetch and validate complete export batches before local mutation.
- Preserve useful per-resource outcomes when a remote batch is only partially successful.
- Establish a command and orchestration shape that can add other API families later.
- Support opt-in end-to-end validation against a compatible test node without making an active commercial license a prerequisite for ordinary tests.

**Non-Goals:**

- General multi-family bundle deployment.
- Project manifests, project version files, gitignore management, or project initialization.
- Dependency discovery or installation for referenced Skills, Tools, Agents, or Workflows.
- Remote deletion, pruning, synchronization, or conflict merging.
- Exporting built-in or plugin-provided readonly resources.
- Adding standalone import/export for Saved Objects or Spaces in this change.
- Transactional rollback across multiple remote API calls.
- Provisioning, activating, or managing a test-cluster license.

## Decisions

### Use action-first commands with resource subcommands

Clap will model `Import` and `Export` as top-level commands, each with nested `Skills`, `Tools`, `Agents`, and `Workflows` resource variants.

```text
kibob import <skills|tools|agents|workflows> <source>
             [--space <id>] [--force]
kibob export <skills|tools|agents|workflows> <destination>
             (--id <id>... | --all)
             [--space <id>] [--overwrite] [--force]
```

The canonical resource token is plural because it names an API family and may transfer multiple items. Resource-specific arguments belong to the selected nested parser, avoiding a global union of selectors such as `--api` and format hints such as `--as`.

Alternative considered: `kibob skills push`. Rejected because it introduces a second resource-first command grammar alongside the existing action-first commands.

Alternative considered: `kibob deploy <path>` or `kibob push <path> --direct`. Rejected because both require artifact-type inference or another disambiguation flag, and `import`/`export` aligns with Kibana's existing transfer vocabulary.

### Defer Saved Objects and Spaces

Saved Objects and Spaces are explicitly out of scope for this change. Saved Objects use a different hierarchical/NDJSON artifact model, while Spaces are global resources rather than content scoped within a space. Either family can be added later with its own source format, selectors, collision behavior, and specification without changing the action-first command grammar established here.

### Keep standalone client configuration independent of project paths

Client construction will be split so environment authentication, URL, and concurrency settings can be reused without reading `spaces.yml`. Standalone orchestration will register exactly the requested space, defaulting through the existing configured-space behavior to `default`, then create a space-bound client.

Standalone capability preflight will call the reusable server-version check and the selected family's `ApiCapability` directly. It will not call project preflight that reads or persists `kibana.version`. The existing `--force` spelling may retain its established meaning of bypassing a version gate; it will not mean overwrite, readonly replacement, or dependency forcing.

### Represent transfer work as validated type-state plans

Filesystem and network mutation will only be exposed from a validated plan state:

```text
Resource import source
    │
    ▼
ImportPlan<Discovered>
    │ parse every resource, validate paths and IDs, reject duplicates
    ▼
ImportPlan<Validated>
    │ version/capability preflight
    ▼
family-specific batch loader

Export selection
    │
    ▼
ExportPlan<Selected>
    │ fetch every resource, reject readonly, validate output collisions
    ▼
ExportPlan<ReadyToWrite>
    │
    ▼
family-specific artifact writer
```

Marker states prevent the mutating execution functions from being called with an unvalidated local batch. The plans retain source paths, resource family, and resource IDs so diagnostics do not have to recover identity from unstructured errors.

Alternative considered: pass `Vec<Value>` directly between CLI helpers. Rejected because it loses source-path context, permits mutation before batch validation, and cannot express duplicate IDs or output collision planning cleanly.

### Add manifest-free family filesystem adapters

Standalone source discovery will live beside the reusable storage readers rather than reusing private manifest-aware project helpers.

Skills retain their two exclusive source forms:

1. `<source>/SKILL.md` represents one Skill.
2. Immediate child directories containing `SKILL.md` represent a collection.

Tools, Agents, and Workflows accept either one `.json` file or a directory whose immediate `.json` files form the collection. These files use the existing JSON5 reader, so comments, trailing commas, and multiline syntax behave exactly as they do in project push. Directory discovery is deterministic and non-recursive.

Discovery requires at least one resource. Each resource is projected through its existing family reader and transforms. A batch-level ID index rejects duplicates before client creation.

Export always treats the destination as a collection root. Skills write `{destination}/{id}/SKILL.md`; Tools, Agents, and Workflows use their existing project JSON serialization and filename rules. The export planner detects filename collisions before writing. Each resulting destination is directly consumable by the matching import command.

No standalone adapter accepts or returns a manifest type. Malformed `skills.yml`, `tools.yml`, `agents.yml`, or `workflows.yml` files near a source or destination therefore cannot influence the operation.

### Make import idempotent without making it synchronizing

Import uses each family's current upsert behavior:

```text
existence check by ID
    ├─ missing ───────────▶ POST family collection
    ├─ writable resource ▶ PUT family item
    └─ readonly resource ─▶ failed outcome; no mutation
```

Skills use their existing `GET` existence check. Tools and Agents use their existing `HEAD` item checks. Workflows retain the required internal-origin header and select the route family from the detected Kibana version: Kibana 9.3 uses `/api/workflows[/{id}]`, while Kibana 9.4 and later use `/api/workflows/workflow[/{id}]`. Import never enumerates remote collections, deletes absent IDs, or expands dependencies. Repeating the same import converges the selected IDs while leaving every unselected remote resource alone.

Fetched definitions may contain response-only metadata that is not accepted by create or update schemas. Family loaders continue to sanitize those fields before mutation; for example, Agent imports omit `created_by` and related audit timestamps while preserving writable fields such as `visibility`.

Alternative considered: create-only import unless `--overwrite` is present. Deferred because existing project push and all four family loaders already provide create-or-update semantics, and repeatable one-shot delivery is the primary use case. A future conflict policy can be added to the resource-specific subcommand without changing the command grammar.

### Require an explicit export selector

Clap enforces a required, mutually exclusive selector group:

- Repeatable `--id <resource-id>` preserves caller order.
- `--all` uses the selected family's existing list or search endpoint, filters `readonly: true` resources where applicable, sorts IDs, and fetches each full definition. Workflow discovery also follows the detected server contract: Kibana 9.3 uses `POST /api/workflows/search`, while Kibana 9.4 and later use `GET /api/workflows`.

List and search responses are not written directly because they may omit fields present in item responses. An explicitly selected readonly resource is an error, not a silent skip, because the produced artifact would not be safely re-importable with the same ID.

Every selected definition is fetched and transformed before filesystem mutation. Output paths are calculated and checked as a batch. Existing selected output paths cause an error unless `--overwrite` is present. With overwrite enabled, the family writer replaces only selected outputs and leaves unrelated destination entries unchanged.

### Return structured per-item batch outcomes

The current family loaders log individual task errors and return only applied counts. Standalone import needs to distinguish applied, skipped, and failed items and must not exit successfully after partial failure.

The library will expose a shared structured batch outcome shape used by family loaders:

```text
ResourceBatchReport
  outcomes:
    - family
    - id
    - status: applied | skipped | failed
    - operation: create | update | none
    - error/detail
```

Each existing `Loader` implementation can adapt its report to the current count-oriented contract so project push compatibility is preserved. Standalone orchestration consumes the full report, prints deterministic per-item results and aggregate counts, and returns an error after reporting when any selected item failed.

Requests continue to use the client's shared concurrency bound. Outcomes are reordered to the validated plan order before presentation so concurrency does not make output nondeterministic.

### ETL data flow

Import composes a filesystem extractor, validation/normalization stage, and Kibana loader:

```text
family artifact source
  → standalone family source extractor
  → existing projection/transforms + batch validation
  → validated import plan
  → space-scoped family batch loader
  → per-item report
```

Export reverses the endpoints:

```text
explicit ID/all selector
  → space-scoped family extractor
  → readonly + completeness validation
  → output collision plan
  → family artifact writer
  → per-item report
```

The first implementation can use the existing `Extractor` and `Loader` components internally, but the CLI layer owns command policy, terminal formatting, exit status, and the prohibition on project state.

### Separate license-independent tests from licensed live validation

Unit tests, filesystem round trips, Clap parsing tests, and mocked HTTP tests are the required workspace coverage for every resource family. They must not depend on a running cluster or license state.

The existing ignored live-test suite will gain standalone import/export cases and support targeting an operator-configured Kibana node. Before running a family case, the harness checks the node version and probes that family through its API. It may skip only when the response explicitly identifies an unsupported version or license condition. Authentication errors, unexpected forbidden responses, malformed responses, and other API failures remain test failures.

Live cases create uniquely named temporary resources, export them, verify the local representation, re-import them, verify the remote result, and perform best-effort cleanup. The harness reports each family as passed, skipped with reason, or failed. A local container without the required license can still run compatible cases; full-family validation can run against a separately supplied licensed node.

Alternative considered: require the default containerized live suite to support every family. Rejected because API availability may depend on both the selected Kibana version and active cluster license, which would make ordinary contributor verification unreliable.

## Risks / Trade-offs

- **Partial remote import is unavoidable because the supported APIs are item-oriented and non-transactional** → Validate the complete local batch first, process the bounded batch, preserve every outcome, and exit non-zero if any item fails.
- **Export can still encounter an I/O failure after writes begin** → Pre-fetch and preflight every selected output before writing; write only selected paths and report exact completed/failed items. Cross-resource atomicity is not claimed.
- **`import` may be read as create-only** → Document create-or-update semantics prominently in command help and summaries, including whether each resource was created or updated.
- **A source directory may accidentally resemble both a Skill and a collection** → Reject ambiguous layouts rather than choosing precedence.
- **Existing name-based JSON filenames can collide even when resource IDs differ** → Calculate all output paths before writing and reject collisions with both IDs in the diagnostic.
- **Ignoring manifests could be mistaken for failing to honor project selection** → Help text will call the commands “standalone” and state that manifests, dependencies, and pruning are not used.
- **New structured loader outcomes can accidentally alter project push behavior** → Add the report as a new API and keep the existing count-oriented `Loader` adapter and project command tests.
- **Readonly resources are useful as templates but cannot be round-tripped under the same ID** → Exclude them in this change; a later explicit template/clone workflow can define safe renaming semantics.
- **License-aware skipping can hide a real regression** → Skip only recognized version/license responses, fail all unexpected authorization or API responses, and retain license-independent mocked coverage for every path.

## Migration Plan

This is additive and requires no data migration.

1. Add reusable standalone discovery and batch outcome APIs without changing existing callers.
2. Add the nested CLI commands and documentation.
3. Verify existing `init`, `pull`, `push`, `add`, `togo`, and `migrate` behavior with regression tests.
4. If the feature must be rolled back, remove the new command variants and standalone orchestration; existing project files and remote resources remain valid because the feature introduces no persistent metadata.
