## 1. Workflow synchronization

- [x] 1.1 Replace Workflow HEAD checks with item GET checks and validate JSON-object readonly state through focused loader tests.
- [x] 1.2 Recover POST 409 only after a writable item GET and successful PUT, verified by create-race and failure-path tests.

## 2. Compatibility and verification

- [x] 2.1 Cover Kibana 9.3 and 9.4+ route families, default and named spaces, missing and readonly Workflows, verified by focused loader tests.
- [x] 2.2 Update Workflow specifications and changelog, then verify with `openspec validate --strict --no-interactive`.
- [x] 2.3 Run `cargo clippy --workspace --all-targets --all-features -- -D warnings` and `cargo test --workspace --all-features`.
