# Change: Tools API Support

## Why
We need to complete the support for Kibana `/api/agent_builder/tools` API. Currently, `pull` and `add` operations are functional, but `push` fails due to JSON5 parsing errors when reading files from disk. This prevents users from version controlling and deploying Kibana tools.

## What Changes
- Investigate and fix JSON5 parsing errors in the `push` operation for tools.
- Ensure robust roundtrip (pull -> push) support for tools, handling complex queries and multiline fields correctly.
- Add comprehensive tests for Tools extractor and loader.

## Impact
- **Affected Specs**: `tools` (new)
- **Affected Code**: 
    - `src/kibana/tools/`
    - `src/storage/json_writer.rs` (potentially)
    - `src/cli.rs`
