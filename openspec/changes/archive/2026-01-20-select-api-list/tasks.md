## 1. CLI Argument Updates
- [x] 1.1 Update `src/main.rs`: Add `api` argument to `Pull`, `Push`, and `Togo` variants in `Commands` enum.
    - `#[arg(long, value_delimiter = ',')] api: Option<Vec<String>>`

## 2. Logic Updates
- [x] 2.1 Update `src/cli.rs`: Add `api_filter` parameter to `pull_saved_objects`.
- [x] 2.2 Update `src/cli.rs`: Add `api_filter` parameter to `push_saved_objects`.
- [x] 2.3 Update `src/cli.rs`: Add `api_filter` parameter to `bundle_to_ndjson`.
- [x] 2.4 Implement filtering helper function `should_process_api(api_name, filter)`.
- [x] 2.5 Apply filtering in `pull_saved_objects` to skip steps (Spaces, Saved Objects, Workflows, Agents, Tools).
- [x] 2.6 Apply filtering in `push_saved_objects` to skip steps.
- [x] 2.7 Apply filtering in `bundle_to_ndjson` to skip steps.

## 3. Wiring
- [x] 3.1 Update `src/main.rs`: Pass the parsed `api` argument to the CLI functions.

## 4. Verification
- [x] 4.1 Verify `kibob pull` (no args) still pulls everything.
- [x] 4.2 Verify `kibob pull --api tools` only pulls tools.
- [x] 4.3 Verify aliases work (e.g., `--api agent` vs `--api agents`).
