# Design: Dependency Inclusion Logic

## Dependency Discovery
The dependency discovery logic will inspect the JSON structure of the objects being added.

### Agent Dependencies
- **Tools**: Usually found in `configuration.tools` as a list of tool IDs.

### Tool Dependencies
- **Workflows**: Usually found in `configuration.workflow_id`.

### Workflow Dependencies
- **Agents/Tools/Workflows**: Can be found within the `definition` or `yaml` fields. Since workflows are complex, a recursive search for keys like `agent_id`, `tool_id`, and `workflow_id` (or their CamelCase variants) will be performed.

## Implementation Strategy
1. **Update CLI**: Add `exclude_dependencies: bool` to `AddArgs` for agents, tools, and workflows in `src/main.rs` and update the function signatures in `src/cli.rs`.
2. **Dependency Resolver**: Create a utility to find dependencies in a `serde_json::Value`.
3. **Recursive Addition**:
   - When an object is added, its dependencies are identified.
   - For each dependency:
     - Check if it's already in the relevant manifest.
     - If not, fetch it from the API (or file) and add it to the manifest.
     - Recursively check the dependency for its own dependencies.
4. **Error Handling**: If a dependency cannot be found or fetched, log a warning but continue adding the primary object.

## CLI Changes
- `kibob add agent <query> [--exclude-dependencies]`
- `kibob add tools <query> [--exclude-dependencies]`
- `kibob add workflows <query> [--exclude-dependencies]`
