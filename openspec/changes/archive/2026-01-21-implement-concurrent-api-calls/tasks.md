# Tasks: Implement Concurrent API Calls

## Phase 1: KibanaClient Enhancements
- [x] Add `Arc<Semaphore>` to `KibanaClient` struct <!-- id: 1 -->
- [x] Update `KibanaClient::try_new` to accept `max_requests` <!-- id: 2 -->
- [x] Implement permit acquisition in `KibanaClient::request_raw` <!-- id: 3 -->
- [x] Update `KibanaClient` unit tests to include concurrency checks <!-- id: 4 -->

## Phase 2: CLI Integration
- [x] Update `load_kibana_client` in `src/cli.rs` to read `KIBANA_MAX_REQUESTS` <!-- id: 5 -->
- [x] Refactor `pull_saved_objects` in `src/cli.rs` to process spaces concurrently <!-- id: 6 -->
- [x] Refactor `push_saved_objects` in `src/cli.rs` to process spaces concurrently <!-- id: 7 -->

## Phase 3: Extractor/Loader Refactoring
- [x] Refactor `WorkflowsExtractor::fetch_manifest_workflows` to use concurrency <!-- id: 8 -->
- [x] Refactor `ToolsExtractor::fetch_manifest_tools` to use concurrency <!-- id: 9 -->
- [x] Refactor `AgentsExtractor::fetch_manifest_agents` to use concurrency <!-- id: 10 -->
- [x] Refactor corresponding Loaders if they perform sequential item-level requests <!-- id: 11 -->

## Phase 4: Verification
- [x] Run full sync (pull/push) against a test Kibana instance to verify speedup <!-- id: 12 -->
- [x] Verify that `KIBANA_MAX_REQUESTS=1` still works correctly (sequential behavior) <!-- id: 13 -->
