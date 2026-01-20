# Change: Select API List

## Why
Users currently have to run `kibob pull` or `kibob push` which operates on all supported object types (Saved Objects, Workflows, Agents, Tools, Spaces). As the number of managed types grows, this becomes slow and inflexible. Users often want to sync only specific types (e.g., "just push my tools" or "just pull the agents").

## What Changes
- Add an optional `--api` argument to `pull`, `push`, and `togo` subcommands.
- This argument accepts a comma-separated list of APIs to operate on (e.g., `--api tools,agents`).
- Supported values: `saved_objects` (alias: `objects`), `workflows`, `agents` (alias: `agent`), `tools` (alias: `tool`), `spaces`.
- If the argument is omitted, the default behavior (operate on all types) is preserved.

## Impact
- **Affected Specs**: `cli` (new)
- **Affected Code**:
    - `src/main.rs`: Add argument to `Commands` enum variants.
    - `src/cli.rs`: Update `pull_saved_objects`, `push_saved_objects`, and `bundle_to_ndjson` to accept and respect the filter.
