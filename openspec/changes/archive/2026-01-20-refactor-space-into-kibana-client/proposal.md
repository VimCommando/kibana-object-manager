# Change: Refactor Space Context into KibanaClient

## Why

The current architecture separates space management (`SpaceContext`) from HTTP communication (`Kibana` client). This leads to repetitive code where every extractor/loader must store both a client reference AND a `space_id: String`, and all HTTP methods have `*_with_space` variants. Since spaces are a core Kibana concept, the client should be space-aware, enabling a cleaner builder-lite pattern: `kibana.space("shanks")?.push_tools()`.

## What Changes

- **BREAKING**: Rename `Kibana` struct to `KibanaClient`
- **BREAKING**: Split client into `KibanaClient` (root) and `SpaceClient` (space-bound)
- **BREAKING**: Remove all `*_with_space` methods from HTTP helpers
- **BREAKING**: Remove `SpaceContext` module entirely
- **BREAKING**: Update all extractors/loaders to accept `SpaceClient` instead of `Kibana` + `space_id`
- `KibanaClient` loads `spaces.yml` at construction time, storing a `HashMap<id, name>`
- `KibanaClient::space(id)` returns `Result<SpaceClient>`, validating the space exists
- `SpaceClient` stores `space: Option<String>` where `None` means default (no `/s/` prefix)

## Impact

- Affected specs: kibana-client (new capability)
- Affected code:
  - `src/client/kibana.rs` - Major refactor to two structs
  - `src/client/mod.rs` - Update exports
  - `src/space_context.rs` - Delete
  - `src/lib.rs` - Remove SpaceContext export
  - `src/kibana/*/extractor.rs` - All 5 extractors
  - `src/kibana/*/loader.rs` - All 5 loaders
  - `src/cli.rs` - Update orchestration (~15 functions)
