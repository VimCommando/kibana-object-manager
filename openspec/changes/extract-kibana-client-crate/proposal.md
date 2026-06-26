## Why

`kibana-object-manager` already contains reusable Kibana API behavior, but it is packaged and constructed around the `kibob` CLI project layout. ESDiag needs the same saved object, space, agent, tool, workflow, dependency, and sync behavior without inheriting CLI concerns such as dotenv loading, `spaces.yml` discovery, local manifest paths, or command exit policy.

## What Changes

- Convert the repository into a Cargo workspace with separate crates for the reusable library and the `kibob` CLI.
- Extract a standalone `kibana-client` library crate containing Kibana authentication, request plumbing, space-aware clients, version/capability checks, API modules, dependency discovery, and storage-neutral sync operations.
- Prepare `kibana-client` for independent publication so ESDiag and other crates can depend on it directly.
- Use `tracing` for library instrumentation and introduce a dedicated public error enum for the client library.
- Modify `kibob` so its CLI orchestration consumes `kibana-client` instead of owning Kibana API behavior directly.
- Move filesystem project layout, JSON5 formatting, manifests, migration, bundling, environment loading, colored logging, and command exit policy into the CLI crate.
- Replace manifest-bound client construction with explicit client configuration suitable for external consumers such as ESDiag.
- Preserve existing `kibob` command behavior and file layout while changing its internal dependency structure.
- **BREAKING** for current library consumers of `kibana-object-manager`: reusable Kibana APIs move to the new `kibana-client` crate and constructors no longer read `spaces.yml` implicitly.

## Capabilities

### New Capabilities
- `workspace-library-packaging`: Multi-crate workspace packaging, crate boundaries, and public dependency contracts.

### Modified Capabilities
- `kibana-client`: Change the client contract from a manifest-bound internal helper into a standalone reusable library with explicit space configuration, API modules, and storage-neutral sync support.

## Impact

- Affects `Cargo.toml`, crate metadata, module paths, documentation examples, tests, and public exports.
- Moves client/API modules currently under `src/client`, `src/kibana`, and selected pure ETL/dependency code into `crates/kibana-client`.
- Keeps `src/cli.rs`, storage, transforms, migration, and binary entrypoint behavior in the CLI crate, adjusted to call the library crate.
- Enables ESDiag to depend on `kibana-client` directly and adapt from its existing `KnownHost`/auth model.
- Requires publish-ready crate metadata, a stable error surface, and tracing-compatible instrumentation before the library is consumed externally.
- Requires compatibility checks to ensure existing `kibob pull`, `push`, `add`, `togo`, `auth`, `init`, and `migrate` behavior remains unchanged from a user perspective.
