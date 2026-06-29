## Context

The repository is currently a single Cargo package named `kibana-object-manager` with a `kibob` binary and a public `src/lib.rs`. The code already separates several useful concepts, but the boundaries are not aligned with external consumption:

- `src/client` contains auth, request plumbing, Kibana version parsing, capability gates, concurrency limits, and space-aware path prefixing.
- `src/kibana` contains endpoint-specific extractors/loaders for saved objects, spaces, agents, tools, and workflows.
- `src/kibana/dependencies.rs` contains pure dependency discovery for agents/tools/workflows.
- `src/cli.rs` owns command orchestration, environment loading, version preflight behavior, dependency resolution into files, and project path layout.
- `src/storage`, `src/transform`, and `src/migration.rs` are tied to the `kibob` repository-on-disk format.

The strongest coupling problem is that `KibanaClient::try_new` currently accepts a project directory and reads `spaces.yml` while constructing the HTTP client. That is useful for `kibob`, but wrong for ESDiag and other consumers because it forces implicit CLI project discovery into the reusable HTTP client.

ESDiag already has its own Kibana client and setup flow. It can benefit from the endpoint behavior in this repository, especially multipart saved object import, space management, internal-origin headers for workflows, agent/tool/workflow upsert semantics, capability gates, and dependency discovery. It should be able to adapt its existing host/auth configuration into the new library without being forced into `kibob` project discovery.

ESDiag also needs to bundle and replay Kibana assets as files. That means filesystem support belongs in the reusable library, but as explicit manifest and bundle APIs rather than hidden client-construction behavior. The library can own stable Kibana asset bundle schemas and path-explicit readers/writers while the CLI owns defaults, migration, terminal output, and command policy.

## Goals / Non-Goals

**Goals:**

- Convert the repository to a Cargo workspace with a reusable `kibana-sync` crate and a `kibana-object-manager` CLI crate.
- Make `kibana-sync` consumable by any Rust crate through explicit client configuration rather than implicit CLI project discovery.
- Make `kibana-sync` independently publishable so ESDiag can depend on the crate directly.
- Keep endpoint-specific Kibana behavior in the library, including required methods, paths, headers, multipart handling, and version/capability gates.
- Provide storage-neutral sync operations so consumers can pull or push all supported API families without depending on `kibob` command behavior.
- Provide explicit filesystem-backed manifest and bundle sync APIs so consumers can version-control, ship, and replay Kibana assets.
- Use `tracing` for library instrumentation and avoid binding the library to a concrete logger.
- Introduce a dedicated `kibana-sync` error enum before external publication.
- Preserve existing `kibob` command behavior, file formats, and CLI flags while making `kibob` call into the library crate.
- Keep reusable domain logic independent from `clap`, `dotenvy`, `env_logger`, `owo-colors`, git integration, terminal output, command exit policy, and local migration code.

**Non-Goals:**

- Redesign the `kibob` project directory layout.
- Change CLI command names, flags, exit codes, or default behavior.
- Convert all resource payloads from `serde_json::Value` into fully typed Kibana schemas.
- Add support for Kibana versions or APIs beyond the currently supported capability matrix.
- Make ESDiag depend on the new crate in this repository change.

## Decisions

### 1) Use a workspace with two primary crates

The repository will become a Cargo workspace:

```text
crates/
  kibana-sync/
  kibana-object-manager/
```

`kibana-sync` will expose reusable Kibana API behavior, sync models, manifest schemas, and explicit filesystem bundle helpers. `kibana-object-manager` will own the `kibob` binary, command defaults, project-root selection, migration behavior, and terminal presentation.

Rationale: This makes the dependency direction explicit. The CLI can depend on the library, while the library cannot accidentally depend on CLI-only modules.

Alternative considered: keep a single package with feature flags. Rejected because feature-gating would still leave ambiguous ownership and make external consumers depend on a CLI-named crate with CLI-oriented modules.

### 2) Remove filesystem-bound client construction

`kibana-sync` constructors will accept explicit configuration:

```rust
KibanaClient::builder(url)
    .auth(auth)
    .max_concurrency(8)
    .spaces(space_registry)
    .build()
```

The library may provide convenience constructors for a default single-space registry or for a caller-supplied list/map of spaces. HTTP client construction must not read `spaces.yml` implicitly. Separate filesystem bundle APIs may read a spaces manifest when the caller explicitly provides the path.

Rationale: ESDiag has its own `KnownHost` and settings model. A reusable client should accept values, not discover them from `kibob` project files.

Alternative considered: keep `try_new(url, auth, project_dir)` in the library and add another constructor. Rejected because the manifest-bound constructor would remain the easy path and preserve the wrong default.

### 3) Keep Kibana endpoint semantics in the library

The library will own modules for:

- saved objects: `POST /api/saved_objects/_export`, `POST /api/saved_objects/_import?overwrite=...` with multipart form data.
- spaces: `GET /api/spaces/space`, `GET /api/spaces/space/{id}`, `POST /api/spaces/space`, `PUT /api/spaces/space/{id}`.
- agents: `GET /api/agent_builder/agents`, `GET/HEAD/PUT /api/agent_builder/agents/{id}`, `POST /api/agent_builder/agents`.
- tools: `GET /api/agent_builder/tools`, `GET/HEAD/PUT /api/agent_builder/tools/{id}`, `POST /api/agent_builder/tools`.
- workflows: `POST /api/workflows/search`, `GET/HEAD/PUT /api/workflows/{id}`, `POST /api/workflows`, with `X-Elastic-Internal-Origin: Kibana`.

Rationale: These endpoint details are precisely what ESDiag should not duplicate. The library should be more than a thin `reqwest` wrapper.

Alternative considered: expose only raw request helpers and let consumers implement API modules. Rejected because it would not solve ESDiag's need to sync all supported APIs.

### 4) Introduce storage-neutral sync models and explicit filesystem bundle APIs

The library will expose sync-level models that represent what to transfer, not where to store it:

```rust
pub struct SyncSelection { ... }
pub struct SyncOptions { ... }
pub struct SyncBundle { ... }
pub struct SyncSummary { ... }
```

`SyncBundle` will group spaces, saved objects, workflows, agents, and tools by space where applicable. Pull sync returns a bundle. Push sync accepts a bundle or API-specific collections and applies them to Kibana. Dependency expansion operates on values and IDs, not on local files.

The library will also expose explicit filesystem-backed sync helpers for version-controlled Kibana asset bundles:

```rust
let bundle = KibanaFsBundle::open(path)?.read(selection)?;
push_sync(&client, bundle, options).await?;

let bundle = pull_sync(&client, selection, options).await?;
KibanaFsBundle::create(path)?.write(&bundle)?;
```

These APIs may know the stable bundle layout and manifest schemas, but they must only operate on paths supplied by the caller. They must not infer a project root from the process working directory, read dotenv state, initialize logging, create gitignore entries, perform migrations, or choose CLI warning/exit behavior.

Rationale: This lets `kibob` adapt bundles to its directory layout while ESDiag can adapt them to embedded assets, setup flows, or runtime orchestration.

Alternative considered: move current `pull_saved_objects` / `push_saved_objects` from `src/cli.rs` into the library unchanged. Rejected because those functions mix Kibana API calls, local path conventions, command defaults, terminal presentation, and warning policy. The correct extraction is reusable file formats and path-explicit readers/writers.

### 5) Keep ETL traits only if they improve the library API

The current `Extractor`, `Transformer`, `Loader`, and `Pipeline` traits are useful internally and can move into `kibana-sync` if the API modules continue to use them. They should not force consumers into an ETL-only API. The library should also expose direct methods and sync services.

Data flow after the refactor:

```text
kibob pull
  -> read spaces.yml and manifests from project directory
  -> build KibanaClient from explicit config
  -> create SyncSelection / API manifests
  -> kibana-sync fetches resources and dependencies
  -> kibob writes JSON5/YAML/NDJSON files

ESDiag setup
  -> adapt KnownHost into KibanaClient config
  -> read bundled filesystem assets into SyncBundle or call API modules directly
  -> import saved objects, ensure spaces, sync agents/tools/workflows
```

### 6) Keep CLI policy in the CLI crate

The CLI crate will retain:

- dotenv and environment variable loading.
- default path selection for `spaces.yml`, per-space manifests, objects, API resources, and bundles.
- JSON5 formatting and saved object transforms.
- logging presentation, colored output, and warning exit status mapping.
- migration and `togo` packaging.

Rationale: These behaviors define `kibob`, not a general Kibana client. Reusable manifest schemas, bundle schemas, and path-explicit filesystem readers/writers are not CLI policy and may live in `kibana-sync`.

### 7) Preserve capability gates in the library, apply command policy in the CLI

`kibana-sync` will expose `ApiCapability`, minimum versions, version parsing, server version retrieval, and support checks. The CLI decides whether unsupported requested APIs produce warnings, skipped operations, or status code 2.

Rationale: Support detection is reusable; command exit policy is not.

### 8) Publish `kibana-sync` independently

`kibana-sync` will be prepared as an independently published crate, not merely a path/git dependency used by sibling crates. Its manifest will include publish-ready package metadata, license, repository, documentation, keywords/categories, and a README or crate-level documentation suitable for docs.rs.

Rationale: ESDiag should be able to depend on the client crate as a normal library dependency, and publication forces a cleaner public API boundary.

Alternative considered: keep the crate private until ESDiag integration validates the API. Rejected because the refactor's primary purpose is external consumption, and the publication boundary should shape the API now.

### 9) Use `tracing` for library instrumentation

The library will emit diagnostic events with `tracing` macros rather than `log` macros. The CLI crate can install a tracing subscriber or bridge to its existing logging behavior. The library will not initialize global logging or tracing subscribers.

Rationale: ESDiag already uses `tracing`, and library crates should emit structured events without choosing the application-level subscriber.

Alternative considered: keep `log` in the library and let ESDiag bridge log records into tracing. Rejected because this is the right time to align the new library with its intended consumer and avoid a second instrumentation migration.

### 10) Introduce a dedicated client error enum

`kibana-sync` will expose a dedicated non-exhaustive error enum and a crate-local `Result<T>` alias. The error enum will cover HTTP transport errors, URL/header construction, JSON/YAML/NDJSON serialization, multipart construction, semver parsing, unsupported capability checks, invalid or unknown spaces, missing resource identifiers, API response failures with status/body, and sync/dependency failures.

The CLI crate may convert these errors into `eyre::Report` at its boundary to preserve ergonomic CLI error handling and warning exit behavior.

Rationale: `eyre` is appropriate for applications, but a published client library should provide matchable, documented error variants that consumers can handle intentionally.

Alternative considered: keep `eyre` in the public API for the first release. Rejected because changing from `eyre` to a public enum later would be a larger breaking change for ESDiag and other consumers.

## Risks / Trade-offs

- [Risk] The split can become a large mechanical refactor with high merge risk. -> Mitigation: move code in layers: workspace first, client construction second, API modules third, CLI orchestration last.
- [Risk] Public API may overfit `kibob` and still feel awkward in ESDiag. -> Mitigation: require constructors and sync APIs to accept explicit values and storage-neutral bundles.
- [Risk] Moving ETL traits into `kibana-sync` may make the library feel too framework-like. -> Mitigation: keep direct API methods public and treat ETL traits as optional building blocks.
- [Risk] `kibob` behavior may regress while internals move. -> Mitigation: run existing integration tests and add focused tests around CLI path adaptation and library constructors.
- [Risk] External consumers may need richer typed models later. -> Mitigation: keep `serde_json::Value` payload support as the stable baseline and add typed wrappers incrementally.
- [Risk] Designing the error enum too narrowly could make early releases painful to consume. -> Mitigation: mark the enum `#[non_exhaustive]`, preserve source errors, and include API status/body context.
- [Risk] Switching to `tracing` can change CLI log output. -> Mitigation: adapt the CLI subscriber configuration and verify command output expectations separately from library event emission.
- [Risk] Independent publication requires metadata and API hygiene earlier. -> Mitigation: treat publish dry-run and docs examples as required verification before release.

## Migration Plan

1. Convert the repository root to a Cargo workspace and move the current binary package to `crates/kibana-object-manager`.
2. Create `crates/kibana-sync` with package metadata and library exports.
3. Add the public error enum, crate-local `Result<T>` alias, and conversion points from request/serialization/version errors.
4. Move auth, client request plumbing, version/capability logic, endpoint modules, dependency discovery, and any required ETL traits into `kibana-sync`.
5. Replace manifest-bound client construction with explicit space registry configuration.
6. Replace library `log` instrumentation with `tracing` events and keep subscriber initialization in the CLI crate.
7. Add storage-neutral sync models and services over the existing API modules.
8. Add explicit filesystem-backed manifest and bundle readers/writers over the sync models.
9. Update `kibana-object-manager` imports to use `kibana-sync`.
10. Keep transforms, migration, default path selection, and CLI helpers in the CLI crate, adapting them to library sync/API types.
11. Update docs and examples to show both `kibob` usage and library usage.
12. Run the full test suite, publish dry-run, and fix tests to reflect new crate paths.

Rollback strategy: since this is a repository-local refactor before external release, rollback is a normal git revert of the workspace split. No persisted user data migration is required.

## Open Questions

- None currently.
