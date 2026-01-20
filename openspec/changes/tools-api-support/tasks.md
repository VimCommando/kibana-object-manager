## 1. Investigation & Reproduction
- [x] 1.1 Create a reproduction test case in `src/storage/json_writer.rs` or a new test file that simulates the failing scenario (Tool with complex query field containing newlines and special characters).
- [x] 1.2 Identify the exact cause of the JSON5 parsing error (e.g., incorrect escaping in `to_string_with_multiline`, `normalize_triple_quotes` bug, or `json5` crate limitation).

## 2. Implementation
- [x] 2.1 Fix the identified issue in `src/storage/json_writer.rs` or `src/transform/multiline_fields.rs`.
- [x] 2.2 Verify the fix with the reproduction test case.
- [x] 2.3 Ensure existing tests (Workflows, Agents) still pass.
- [x] 2.4 Fix Tools API `create_tool` to use correct URL format (POST /api/agent_builder/tools).

## 3. Verification
- [x] 3.1 Verify `pull` and `push` operations for tools work correctly end-to-end (using mocked loader/extractor tests if live Kibana is not available).
