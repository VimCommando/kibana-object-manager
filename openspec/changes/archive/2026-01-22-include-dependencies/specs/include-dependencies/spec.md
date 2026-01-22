# Capability: Include Dependencies

## ADDED Requirements

### Requirement: Automatic Dependency Inclusion
When adding an object to a manifest via the `add` command, `kibob` MUST automatically identify and add all referenced dependencies to their respective manifests, unless the `--exclude-dependencies` flag is provided.

#### Scenario: Adding an Agent with Tools
- **Given** an agent that references two tools: `tool-a` and `tool-b`.
- **When** the user runs `kibob add agent my-agent`.
- **Then** `my-agent` is added to `agents.yml`.
- **And** `tool-a` and `tool-b` are added to `tools.yml`.
- **And** the JSON files for the agent and both tools are written to the filesystem.

#### Scenario: Adding a Tool with a Workflow
- **Given** a tool that references workflow `wf-1`.
- **When** the user runs `kibob add tools my-tool`.
- **Then** `my-tool` is added to `tools.yml`.
- **And** `wf-1` is added to `workflows.yml`.

#### Scenario: Adding a Workflow with mixed dependencies
- **Given** a workflow that references agent `agent-1` and another workflow `wf-2`.
- **When** the user runs `kibob add workflows my-wf`.
- **Then** `my-wf` is added to `workflows.yml`.
- **And** `agent-1` is added to `agents.yml`.
- **And** `wf-2` is added to `workflows.yml`.

#### Scenario: Opting out with --exclude-dependencies
- **Given** an agent that references tools.
- **When** the user runs `kibob add agent my-agent --exclude-dependencies`.
- **Then** only `my-agent` is added to the manifest.

### Requirement: Transitive Dependencies
Dependency inclusion MUST be transitive. If a workflow is added as a dependency of a tool, any agents referenced by that workflow MUST also be added.

#### Scenario: Multi-level dependencies
- **Given** agent `A` depends on tool `T`.
- `And` tool `T` depends on workflow `W`.
- **When** the user runs `kibob add agent A`.
- **Then** `A`, `T`, and `W` are all added to their respective manifests.
