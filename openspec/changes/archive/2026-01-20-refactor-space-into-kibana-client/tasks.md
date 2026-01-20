## 1. Client Module Refactor

- [ ] 1.1 Rename `Kibana` to `KibanaClient` in `src/client/kibana.rs`
- [ ] 1.2 Add `spaces: HashMap<String, String>` field to `KibanaClient`
- [ ] 1.3 Update `try_new()` to accept `project_dir` and load `SpacesManifest`
- [ ] 1.4 Add `space(&self, id: &str) -> Result<SpaceClient>` method with validation
- [ ] 1.5 Add `space_ids(&self)` and `space_name(&self, id: &str)` helper methods
- [ ] 1.6 Create `SpaceClient` struct with `client`, `url`, `space: Option<String>`
- [ ] 1.7 Move HTTP methods to `SpaceClient`, removing `_with_space` suffix
- [ ] 1.8 Keep non-space methods on `KibanaClient` for global operations (test_connection, etc.)
- [ ] 1.9 Update `src/client/mod.rs` to export both structs

## 2. Extractor Updates

- [ ] 2.1 Update `SavedObjectsExtractor` to accept `SpaceClient` instead of `Kibana` + `space_id`
- [ ] 2.2 Update `WorkflowsExtractor` to accept `SpaceClient`
- [ ] 2.3 Update `AgentsExtractor` to accept `SpaceClient`
- [ ] 2.4 Update `ToolsExtractor` to accept `SpaceClient`
- [ ] 2.5 Update `SpacesExtractor` (global, may need `KibanaClient` directly)

## 3. Loader Updates

- [ ] 3.1 Update `SavedObjectsLoader` to accept `SpaceClient`
- [ ] 3.2 Update `WorkflowsLoader` to accept `SpaceClient`, remove `space_path()` helper
- [ ] 3.3 Update `AgentsLoader` to accept `SpaceClient`
- [ ] 3.4 Update `ToolsLoader` to accept `SpaceClient`
- [ ] 3.5 Update `SpacesLoader` (global, may need `KibanaClient` directly)

## 4. CLI Orchestration Updates

- [ ] 4.1 Update `pull_saved_objects` to use `KibanaClient::space()`
- [ ] 4.2 Update `push_saved_objects` to use `KibanaClient::space()`
- [ ] 4.3 Update `bundle_to_ndjson` to use `KibanaClient::space()`
- [ ] 4.4 Update `add_workflows_to_manifest` to use `KibanaClient::space()`
- [ ] 4.5 Update `add_agents_to_manifest` to use `KibanaClient::space()`
- [ ] 4.6 Update `add_tools_to_manifest` to use `KibanaClient::space()`
- [ ] 4.7 Update all `pull_space_*` helper functions
- [ ] 4.8 Update all `push_space_*` helper functions
- [ ] 4.9 Remove `SpaceContext` usage from CLI, use `kibana.space_ids()` for iteration

## 5. Cleanup

- [ ] 5.1 Delete `src/space_context.rs`
- [ ] 5.2 Remove `SpaceContext` from `src/lib.rs` exports
- [ ] 5.3 Update any doc comments referencing old API
- [ ] 5.4 Run `cargo fmt` and `cargo clippy`
- [ ] 5.5 Run `cargo test --all` and fix any failures
- [ ] 5.6 Run `cargo build --release` to verify compilation
