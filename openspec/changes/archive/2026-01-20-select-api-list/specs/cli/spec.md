## ADDED Requirements
### Requirement: API Filtering
The CLI SHALL support filtering operations by API type using the `--api` flag on `pull`, `push`, and `togo` commands.

#### Scenario: Default behavior (no filter)
- **WHEN** the user runs `kibob pull` without the `--api` flag
- **THEN** the system pulls all supported object types (Spaces, Saved Objects, Workflows, Agents, Tools)

#### Scenario: Filter by single API
- **WHEN** the user runs `kibob pull --api tools`
- **THEN** the system ONLY pulls Tools
- **AND** skips Spaces, Saved Objects, Workflows, and Agents

#### Scenario: Filter by multiple APIs
- **WHEN** the user runs `kibob push --api agents,workflows`
- **THEN** the system pushes Agents and Workflows
- **AND** skips Spaces, Saved Objects, and Tools

#### Scenario: API Aliases
- **WHEN** the user uses singular aliases (e.g., `tool`, `agent`, `object`)
- **THEN** the system treats them as their plural counterparts (`tools`, `agents`, `saved_objects`)
