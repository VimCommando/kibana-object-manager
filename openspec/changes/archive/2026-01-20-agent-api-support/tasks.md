## 1. Investigation & Reproduction
- [x] 1.1 Review `src/kibana/agents/loader.rs` to identify current API call patterns.
- [x] 1.2 Verify if `src/cli.rs` is already using `storage::read_json5_file` for agents (this was likely done in the previous tools update, but verify).

## 2. Implementation
- [x] 2.1 Update `create_agent` in `src/kibana/agents/loader.rs`:
    - Use `POST /api/agent_builder/agents`.
    - Strip `readonly` and `schema` from body.
- [x] 2.2 Update `update_agent` in `src/kibana/agents/loader.rs`:
    - Strip `id`, `readonly`, and `schema` from body.

## 3. Verification
- [x] 3.1 Run `cargo test` to ensure no regressions.
- [x] 3.2 Verify existing agent integration tests pass.
