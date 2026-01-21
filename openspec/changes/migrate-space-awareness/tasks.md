# Tasks: Enhance Migrate Command

- [ ] Update `migrate_to_multispace_unified` in `src/migration.rs` to be `async` and accept an optional `env_path`.
- [ ] Implement space ID detection logic (prefer lowercase `kibana_space`).
- [ ] Add logic to `migrate_to_multispace_unified` to fetch the space definition using `KibanaClient` if possible.
- [ ] Update `migrate_to_multispace_unified` to create `{space_id}/space.json` and update root `spaces.yml`.
- [ ] Create `.env` transformation logic to uppercase keys and comment out `KIBANA_SPACE`.
- [ ] Update `src/main.rs` to pass the `--env` path to the migrate command.
- [ ] Add unit tests for the `.env` transformation logic.
- [ ] Add unit tests for the space-aware migration path.
- [ ] Verify with `cargo check` and `cargo test`.
