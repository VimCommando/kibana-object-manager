# Proposal: Include Dependencies when Adding Objects

## Problem Statement
When users add agents, tools, or workflows to their manifest using `kibob add`, they often have to manually find and add all dependencies (e.g., tools referenced by an agent, workflows referenced by a tool) to ensure the manifest is complete and doesn't break when pushed to another environment. This is tedious and error-prone.

## Proposed Solution
Update the `kibob add` command for agents, tools, and workflows to automatically detect and add referenced dependencies to their respective manifests.
- Adding an **agent** will automatically add all **tools** it references.
- Adding a **tool** will automatically add all **workflows** it references.
- Adding a **workflow** will automatically add all **agents**, **tools**, and other **workflows** it references.

A new flag `--exclude-dependencies` will be added to allow users to opt out of this automatic inclusion.

## Scope
- CLI: Update `add` commands for agents, tools, and workflows.
- Logic: Implement dependency discovery for each object type.
- Manifest: Ensure dependencies are added to the correct space-specific manifests.
