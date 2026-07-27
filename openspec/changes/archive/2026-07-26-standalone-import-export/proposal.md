## Why

`kibob push` and `kibob pull` are project-oriented operations whose inputs are interpreted through project layout and manifests. Users also need an explicit, one-shot way to move a single Kibana API family between local artifacts and a configured Kibana instance without creating, reading, or maintaining project tracking state.

## What Changes

- Add action-first, resource-explicit standalone commands:
  - `kibob import <resource> <source>`
  - `kibob export <resource> <destination>`
- Initially support the existing file-backed `skills`, `tools`, `agents`, and `workflows` API families.
- Allow Skills import sources to be either one directory containing `SKILL.md` or a collection whose immediate child directories contain `SKILL.md`.
- Allow Tools, Agents, and Workflows import sources to be either one JSON/JSON5 file or a directory of immediate JSON/JSON5 files.
- Require standalone exports to select resources explicitly with one or more `--id` values or `--all`.
- Materialize exports using each API family's existing project representation, but without writing its manifest.
- Keep standalone transfer isolated from project state: it does not read or write manifests, discover dependencies, prune remote resources, or change `push`/`pull` behavior.
- Validate complete local inputs before mutation and make batch failures visible through a non-zero exit status and per-resource results.

## Capabilities

### New Capabilities

- `standalone-resource-transfer`: Explicit, manifest-free import and export of one Kibana API family at a time for Skills, Tools, Agents, and Workflows.

### Modified Capabilities

None.

## Impact

- Adds `import` and `export` command trees to the `kibob` CLI.
- Adds standalone orchestration that reuses each supported family's existing filesystem representation, transforms, extractor, loader, Kibana client, space selection, authentication, and version preflight.
- Requires structured batch outcomes so standalone imports cannot silently succeed when individual resource operations fail.
- Adds license-independent CLI, filesystem, and mocked API tests plus opt-in live validation against a compatible, actively licensed test node.
- Existing project-oriented commands and layouts remain compatible.
