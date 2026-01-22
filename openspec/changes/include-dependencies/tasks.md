# Tasks: Include Dependencies

- [x] Update CLI arguments in `src/main.rs` to include `--exclude-dependencies` for add commands.
- [x] Update `add_agents_to_manifest`, `add_tools_to_manifest`, and `add_workflows_to_manifest` in `src/cli.rs` to accept `exclude_dependencies` flag.
- [x] Implement dependency discovery logic for Agents.
- [x] Implement dependency discovery logic for Tools.
- [x] Implement dependency discovery logic for Workflows.
- [x] Implement recursive dependency resolution and manifest addition.
- [x] Add logging to inform the user about added dependencies.
- [x] Verify implementation with manual tests against a live Kibana or mocks.
