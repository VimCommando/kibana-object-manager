# Design: Multiple Spaces List and Manifest Generation

## CLI Argument Parsing
The `Cli` structure in `src/main.rs` will be updated for `Pull`, `Push`, and `Togo` subcommands:
- Change `space: Option<String>` to `space: Option<Vec<String>>`.
- Add `value_delimiter = ','` to the `#[arg]` attribute.

For the `Add` subcommand:
- Change `space: String` (defaulting to "default") to `space: Option<Vec<String>>` (or similar).
- When `api == "spaces"`, this flag acts as a filter for fetched space IDs.
- When `api == "workflows" | "agents" | "tools"`, it continues to specify the target space(s) to search within. For simplicity in this iteration, we may restrict it to a single target space if multiple aren't supported by those APIs yet, but the user requested supporting multiple values "like we do with the --api flag".

## Spaces Manifest Management
`add_spaces_to_manifest` in `src/cli.rs` will be updated:
- It will accept `space_filter: Option<&[String]>`.
- It will write to `project_dir.join("spaces.yml")` (root) instead of `manifest/spaces.yml`.
- It will fetch all spaces and filter by both regex (existing) and ID list (new).

## Consistency
`pull_saved_objects`, `push_saved_objects`, and `bundle_to_ndjson` already use helper functions that parse comma-separated strings. These functions (`get_target_space_ids` and `get_target_space_ids_from_manifest`) will be updated or bypassed to accept the already-parsed `Vec<String>` from `clap`.

## Backward Compatibility
`spaces.yml` remains optional. If omitted, operations default to the `default` space.
Existing regex filters in `kibob add spaces` (`--include`, `--exclude`) are preserved.
