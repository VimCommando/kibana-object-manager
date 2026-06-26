## 1. Workspace Setup

- [ ] 1.1 Convert the repository root `Cargo.toml` into a Cargo workspace manifest with `crates/kibana-client` and `crates/kibana-object-manager` members.
- [ ] 1.2 Move the current package manifest, binary target, source tree, tests, docs, and package metadata into `crates/kibana-object-manager` while preserving the `kibob` binary name.
- [ ] 1.3 Create the `crates/kibana-client` package manifest with only reusable library dependencies and publish-ready metadata.
- [ ] 1.4 Update workspace-level lockfile and package paths so `cargo metadata` resolves both workspace members.

## 2. Library Extraction

- [ ] 2.1 Define the `kibana-client` public error enum, crate-local `Result<T>` alias, and conversions from HTTP, URL, header, serialization, multipart, semver, space validation, capability, and API response failures.
- [ ] 2.2 Move authentication types, request plumbing, version parsing, capability gates, and concurrency limiting into `kibana-client`.
- [ ] 2.3 Replace `KibanaClient::try_new(url, auth, project_dir, max_requests)` with explicit client configuration that accepts URL, auth, max concurrency, and caller-provided space registry.
- [ ] 2.4 Implement root and space-bound client types so non-default spaces prefix API requests with `/s/{space}/` and the default space has no `/s/default/` prefix.
- [ ] 2.5 Replace library `log` macros with `tracing` instrumentation and avoid subscriber initialization in the library crate.
- [ ] 2.6 Move saved objects API behavior into `kibana-client`, including export NDJSON parsing and multipart import.
- [ ] 2.7 Move spaces API behavior into `kibana-client`, including list, fetch, create, update, and overwrite behavior.
- [ ] 2.8 Move agents and tools API behavior into `kibana-client`, including list, fetch, existence checks, create, update, readonly skips, and payload sanitization.
- [ ] 2.9 Move workflows API behavior into `kibana-client`, including search, fetch, existence checks, create, update, payload sanitization, and `X-Elastic-Internal-Origin: Kibana` headers.
- [ ] 2.10 Move pure dependency discovery for agents, tools, and workflows into `kibana-client`.
- [ ] 2.11 Move ETL traits into `kibana-client` only where they remain useful for API module implementation, and keep direct API methods available for external consumers.

## 3. Storage-Neutral Sync API

- [ ] 3.1 Define `SyncSelection`, `SyncOptions`, `SyncBundle`, and `SyncSummary` library models covering spaces, saved objects, workflows, agents, and tools.
- [ ] 3.2 Implement pull sync that fetches selected spaces and API families into a `SyncBundle` without writing files.
- [ ] 3.3 Implement push sync that applies a `SyncBundle` to Kibana and returns a `SyncSummary`.
- [ ] 3.4 Implement dependency expansion that fetches missing dependent agents, tools, and workflows into the bundle without referencing local paths.
- [ ] 3.5 Expose capability-gated sync planning so callers can decide whether unsupported APIs are skipped, warned, or forced.

## 4. CLI Adaptation

- [ ] 4.1 Add a path dependency from `kibana-object-manager` to `kibana-client`.
- [ ] 4.2 Keep dotenv, environment loading, logging setup, colored output, warning exit status, and Clap command definitions in the CLI crate.
- [ ] 4.3 Configure CLI tracing/log output so `kibana-client` tracing events remain visible under existing `kibob` verbosity behavior.
- [ ] 4.4 Move `spaces.yml` reading and project space registry construction into the CLI crate before building `KibanaClient`.
- [ ] 4.5 Adapt `pull`, `push`, `add`, and dependency-resolution flows to call `kibana-client` API or sync functions while preserving existing file layout.
- [ ] 4.6 Convert `kibana-client` errors into CLI reports and warning exit behavior at the CLI boundary.
- [ ] 4.7 Keep JSON5 formatting, field transforms, NDJSON bundling, migration, gitignore, and path helpers in the CLI crate.
- [ ] 4.8 Preserve existing command help text, flags, default behavior, and version warning behavior from the user's perspective.

## 5. Tests and Documentation

- [ ] 5.1 Update unit tests for new crate paths and explicit client construction.
- [ ] 5.2 Add `kibana-client` unit tests for explicit space registry behavior, default space path handling, non-default path prefixing, and constructor behavior with no filesystem reads.
- [ ] 5.3 Add `kibana-client` tests for capability matrix boundaries and version parsing.
- [ ] 5.4 Add `kibana-client` tests for public error variants, API response status/body preservation, and CLI conversion boundaries.
- [ ] 5.5 Add focused tests for storage-neutral sync models and dependency expansion without filesystem writes.
- [ ] 5.6 Update README and docs examples to distinguish `kibob` CLI usage from published `kibana-client` library usage.
- [ ] 5.7 Update any OpenSpec references or documentation that still describe `KibanaClient` as loading spaces from `spaces.yml`.

## 6. Verification

- [ ] 6.1 Run `cargo fmt --all`.
- [ ] 6.2 Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] 6.3 Run `cargo test --workspace`.
- [ ] 6.4 Run existing integration tests for saved objects, spaces, workflows, migration, and ETL behavior.
- [ ] 6.5 Run `cargo publish -p kibana-client --dry-run`.
- [ ] 6.6 Manually verify `kibob --help` still lists `init`, `auth`, `pull`, `push`, `add`, `togo`, and `migrate`.
