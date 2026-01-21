# Tasks: Enhance Migrate Command

- [x] Update `migrate_to_multispace_unified` in `src/migration.rs` to be `async` and accept an optional `env_path`.
- [x] Implement space ID detection logic (prefer lowercase `kibana_space`).
- [x] Add logic to `migrate_to_multispace_unified` to fetch the space definition using `KibanaClient` if possible.
- [x] Update `migrate_to_multispace_unified` to create `{space_id}/space.json` and update root `spaces.yml`.
- [x] Create `.env` transformation logic to uppercase keys and comment out `KIBANA_SPACE`.
- [x] Update `src/main.rs` to pass the `--env` path to the migrate command.
- [x] Add unit tests for the `.env` transformation logic.
- [x] Add unit tests for the space-aware migration path.
- [x] Verify with `cargo check` and `cargo test`.
