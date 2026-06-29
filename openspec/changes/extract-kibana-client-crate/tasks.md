## 1. Workspace Setup

- [x] 1.1 Convert the repository root `Cargo.toml` into a Cargo workspace manifest with `crates/kibana-sync` and `crates/kibana-object-manager` members.
- [x] 1.2 Move the current package manifest, binary target, source tree, tests, docs, and package metadata into `crates/kibana-object-manager` while preserving the `kibob` binary name.
- [x] 1.3 Create the `crates/kibana-sync` package manifest with only reusable library dependencies and publish-ready metadata.
- [x] 1.4 Update workspace-level lockfile and package paths so `cargo metadata` resolves both workspace members.

## 2. Library Extraction

- [x] 2.1 Define the `kibana-sync` public error enum, crate-local `Result<T>` alias, and conversions from HTTP, URL, header, serialization, multipart, semver, space validation, capability, and API response failures.
- [x] 2.2 Move authentication types, request plumbing, version parsing, capability gates, and concurrency limiting into `kibana-sync`.
- [x] 2.3 Replace `KibanaClient::try_new(url, auth, project_dir, max_requests)` with explicit client configuration that accepts URL, auth, max concurrency, and caller-provided space registry.
- [x] 2.4 Implement root and space-bound client types so non-default spaces prefix API requests with `/s/{space}/` and the default space has no `/s/default/` prefix.
- [x] 2.5 Replace library `log` macros with `tracing` instrumentation and avoid subscriber initialization in the library crate.
- [x] 2.6 Move saved objects API behavior into `kibana-sync`, including export NDJSON parsing and multipart import.
- [x] 2.7 Move spaces API behavior into `kibana-sync`, including list, fetch, create, update, and overwrite behavior.
- [x] 2.8 Move agents and tools API behavior into `kibana-sync`, including list, fetch, existence checks, create, update, readonly skips, and payload sanitization.
- [x] 2.9 Move workflows API behavior into `kibana-sync`, including search, fetch, existence checks, create, update, payload sanitization, and `X-Elastic-Internal-Origin: Kibana` headers.
- [x] 2.10 Move pure dependency discovery for agents, tools, and workflows into `kibana-sync`.
- [x] 2.11 Move ETL traits into `kibana-sync` only where they remain useful for API module implementation, and keep direct API methods available for external consumers.

## 3. Storage-Neutral Sync API

- [x] 3.1 Define `SyncSelection`, `SyncOptions`, `SyncBundle`, and `SyncSummary` library models covering spaces, saved objects, workflows, agents, and tools.
- [x] 3.2 Implement pull sync that fetches selected spaces and API families into a `SyncBundle` without writing files.
- [x] 3.3 Implement push sync that applies a `SyncBundle` to Kibana and returns a `SyncSummary`.
- [x] 3.4 Implement dependency expansion that fetches missing dependent agents, tools, and workflows into the bundle without referencing local paths.
- [x] 3.5 Expose capability-gated sync planning so callers can decide whether unsupported APIs are skipped, warned, or forced.
- [x] 3.6 Define an explicit filesystem bundle API in `kibana-sync` for reading and writing manifests plus file-backed saved objects, workflows, agents, and tools from caller-provided paths.
- [x] 3.7 Adapt existing manifest serializers and file-backed resource readers/writers into the filesystem bundle API without using environment variables, current working directory discovery, or CLI command state.
- [x] 3.8 Ensure filesystem bundle reads produce a `SyncBundle` or equivalent resource collection that can be pushed with library sync APIs.
- [x] 3.9 Ensure pulled `SyncBundle` values can be written to a stable filesystem bundle layout and read back losslessly enough for push sync.
- [x] 3.10 Add `kibana-sync` tests for filesystem bundle round trips, explicit path handling, and absence of CLI-only behavior.

## 4. CLI Adaptation

- [x] 4.1 Add a path dependency from `kibana-object-manager` to `kibana-sync`.
- [x] 4.2 Keep dotenv, environment loading, logging setup, colored output, warning exit status, and Clap command definitions in the CLI crate.
- [x] 4.3 Configure CLI tracing/log output so `kibana-sync` tracing events remain visible under existing `kibob` verbosity behavior.
- [x] 4.4 Move `spaces.yml` reading and project space registry construction into the CLI crate before building `KibanaClient`.
- [x] 4.5 Adapt `pull`, `push`, `add`, and dependency-resolution flows to call `kibana-sync` API or sync functions while preserving existing file layout.
- [x] 4.6 Convert `kibana-sync` errors into CLI reports and warning exit behavior at the CLI boundary.
- [x] 4.7 Keep JSON5 formatting, field transforms, NDJSON bundling, migration, gitignore, and path helpers in the CLI crate.
- [x] 4.8 Preserve existing command help text, flags, default behavior, and version warning behavior from the user's perspective.

## 5. Tests and Documentation

- [x] 5.1 Update unit tests for new crate paths and explicit client construction.
- [x] 5.2 Add `kibana-sync` unit tests for explicit space registry behavior, default space path handling, non-default path prefixing, and constructor behavior with no filesystem reads.
- [x] 5.3 Add `kibana-sync` tests for capability matrix boundaries and version parsing.
- [x] 5.4 Add `kibana-sync` tests for public error variants, API response status/body preservation, and CLI conversion boundaries.
- [x] 5.5 Add focused tests for storage-neutral sync models and dependency expansion without filesystem writes.
- [x] 5.6 Update README and docs examples to distinguish `kibob` CLI usage from published `kibana-sync` library usage.
- [x] 5.7 Update any OpenSpec references or documentation that still describe `KibanaClient` as loading spaces from `spaces.yml`.

## 6. Verification

- [x] 6.1 Run `cargo fmt --all`.
- [x] 6.2 Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] 6.3 Run `cargo test --workspace`.
- [x] 6.4 Run existing integration tests for saved objects, spaces, workflows, migration, and ETL behavior.
- [x] 6.5 Run `cargo publish -p kibana-sync --dry-run`.
- [x] 6.6 Manually verify `kibob --help` still lists `init`, `auth`, `pull`, `push`, `add`, `togo`, and `migrate`.
