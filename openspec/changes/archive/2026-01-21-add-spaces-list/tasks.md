# Tasks: Support Multiple Spaces and Manifest Generation

- [ ] Update `Cli` struct in `src/main.rs` to support comma-separated space lists for `Pull`, `Push`, and `Togo` subcommands.
- [ ] Update `Cli` struct in `src/main.rs` to support `--space` filtering for `Add` subcommand (remove default "default" and handle it in the logic).
- [ ] Refactor `get_target_space_ids` and `get_target_space_ids_from_manifest` in `src/cli.rs` to accept `Option<&[String]>`.
- [ ] Update `pull_saved_objects`, `push_saved_objects`, and `bundle_to_ndjson` to pass the parsed space list.
- [ ] Update `add_spaces_to_manifest` in `src/cli.rs` to support ID filtering and write to root `spaces.yml`.
- [ ] Fix inconsistency in `src/cli.rs` where `add_spaces_to_manifest` was using `manifest/spaces.yml`.
- [ ] Verify changes with `cargo check` and run existing tests.
- [ ] Add unit tests for multiple space filtering logic.
